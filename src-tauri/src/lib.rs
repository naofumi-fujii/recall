use arboard::Clipboard;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Listener, Manager, PhysicalPosition};

#[cfg(target_os = "macos")]
use block2::StackBlock;
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSEvent, NSEventMask, NSEventModifierFlags, NSRunningApplication, NSWorkspace,
};
#[cfg(target_os = "macos")]
use objc2_foundation::NSRunLoop;
#[cfg(target_os = "macos")]
use std::ptr::NonNull;
#[cfg(target_os = "macos")]
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub timestamp: DateTime<Local>,
    pub content: String,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub entries: Vec<ClipboardEntry>,
    pub max_entries: usize,
}

const MAX_HISTORY_ENTRIES: usize = 200;
const DOUBLE_TAP_THRESHOLD_MS: u128 = 400;

#[cfg(target_os = "macos")]
static PREVIOUS_APP: Mutex<Option<objc2::rc::Retained<NSRunningApplication>>> = Mutex::new(None);

fn get_data_dir() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("recall");
    fs::create_dir_all(&data_dir).ok();
    data_dir
}

fn get_history_path() -> PathBuf {
    get_data_dir().join("clipboard_history.jsonl")
}

fn save_entry(entry: &ClipboardEntry) -> std::io::Result<()> {
    let mut history = load_history();

    // Check if the same content exists and preserve its pinned state
    let existing_pinned = history
        .iter()
        .find(|e| e.content == entry.content)
        .map(|e| e.pinned)
        .unwrap_or(false);

    history.retain(|e| e.content != entry.content);

    history.push(ClipboardEntry {
        timestamp: entry.timestamp,
        content: entry.content.clone(),
        pinned: existing_pinned,
    });

    // Trim history while preserving pinned items
    if history.len() > MAX_HISTORY_ENTRIES {
        // Separate pinned and unpinned items
        let pinned: Vec<_> = history.iter().filter(|e| e.pinned).cloned().collect();
        let mut unpinned: Vec<_> = history.iter().filter(|e| !e.pinned).cloned().collect();

        // Calculate how many unpinned items we can keep
        let unpinned_limit = MAX_HISTORY_ENTRIES.saturating_sub(pinned.len());

        // Keep only the newest unpinned items
        if unpinned.len() > unpinned_limit {
            let start = unpinned.len() - unpinned_limit;
            unpinned = unpinned.split_off(start);
        }

        // Rebuild history: unpinned first (older), then pinned
        history = unpinned;
        history.extend(pinned);

        // Sort by timestamp to maintain chronological order
        history.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    }

    save_history(&history)
}

fn save_history(history: &[ClipboardEntry]) -> std::io::Result<()> {
    let path = get_history_path();
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;
    for e in history {
        let json = serde_json::to_string(e)?;
        writeln!(file, "{}", json)?;
    }
    Ok(())
}

fn load_history() -> Vec<ClipboardEntry> {
    let path = get_history_path();
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let reader = BufReader::new(file);
    reader
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

#[tauri::command]
fn get_history() -> HistoryResponse {
    let mut history = load_history();
    history.reverse();
    HistoryResponse {
        entries: history,
        max_entries: MAX_HISTORY_ENTRIES,
    }
}

#[tauri::command]
fn copy_to_clipboard(content: String) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(&content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn toggle_pin(timestamp: String, pinned: bool) -> Result<(), String> {
    let mut history = load_history();

    // Find the entry by timestamp and update its pinned state
    if let Some(entry) = history
        .iter_mut()
        .find(|e| e.timestamp.to_rfc3339() == timestamp)
    {
        entry.pinned = pinned;
    } else {
        return Err("Entry not found".to_string());
    }

    save_history(&history).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_all_history() -> Result<(), String> {
    let history = load_history();
    let pinned: Vec<_> = history.into_iter().filter(|e| e.pinned).collect();

    if pinned.is_empty() {
        let path = get_history_path();
        if path.exists() {
            fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
    } else {
        save_history(&pinned).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn restore_previous_app() -> Result<(), String> {
    let previous_app = PREVIOUS_APP.lock().unwrap().clone();
    if let Some(app) = previous_app {
        // Use empty options set - the default behavior will activate the app
        app.activateWithOptions(objc2_app_kit::NSApplicationActivationOptions::empty());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
#[tauri::command]
fn restore_previous_app() -> Result<(), String> {
    Ok(())
}

fn start_clipboard_monitor(app_handle: AppHandle, running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to access clipboard: {}", e);
                return;
            }
        };
        let mut last_content: Option<String> = None;

        while running.load(Ordering::Relaxed) {
            if let Ok(current) = clipboard.get_text() {
                let is_new = match &last_content {
                    Some(last) => last != &current,
                    None => true,
                };

                if is_new && !current.is_empty() {
                    let entry = ClipboardEntry {
                        timestamp: Local::now(),
                        content: current.clone(),
                        pinned: false,
                    };

                    if let Err(e) = save_entry(&entry) {
                        log::error!("保存エラー: {}", e);
                    } else {
                        let _ = app_handle.emit("clipboard-changed", &entry);
                    }

                    last_content = Some(current);
                }
            }

            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn show_window_at_mouse(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        // Capture the currently active application before showing our window
        #[cfg(target_os = "macos")]
        {
            let workspace = NSWorkspace::sharedWorkspace();
            if let Some(active_app) = workspace.frontmostApplication() {
                // Only store if it's not our own app
                let bundle_id = active_app.bundleIdentifier();
                if let Some(id) = bundle_id {
                    let id_str = id.to_string();
                    if id_str != "com.recall.clipboard" {
                        *PREVIOUS_APP.lock().unwrap() = Some(active_app.clone());
                    }
                }
            }
        }

        // Only hide if already visible (to trigger re-show)
        // Skip hide on initial show to prevent flicker
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            let _ = window.hide();
        }

        // Get the current mouse position using CGEvent (macOS)
        // CGEvent returns coordinates in the global display coordinate system (top-left origin)
        // which works correctly with multiple monitors
        #[cfg(target_os = "macos")]
        {
            use core_graphics::display::CGDisplay;
            use core_graphics::event::CGEvent;
            use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

            if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
                let event = CGEvent::new(source);
                if let Ok(event) = event {
                    let location = event.location();
                    let mouse_x = location.x as i32;
                    let mouse_y = location.y as i32;

                    // Get actual window size
                    let (window_width, window_height) = if let Ok(size) = window.outer_size() {
                        (size.width as i32, size.height as i32)
                    } else {
                        (500, 600) // fallback
                    };

                    // Find the display containing the mouse cursor
                    let (screen_x, screen_y, screen_width, screen_height) = {
                        let mut found_bounds = None;

                        // Get all active displays and find the one containing the mouse
                        if let Ok(display_ids) = CGDisplay::active_displays() {
                            for display_id in display_ids {
                                let display = CGDisplay::new(display_id);
                                let bounds = display.bounds();
                                let x = bounds.origin.x;
                                let y = bounds.origin.y;
                                let w = bounds.size.width;
                                let h = bounds.size.height;

                                // Check if mouse is within this display
                                if location.x >= x
                                    && location.x < x + w
                                    && location.y >= y
                                    && location.y < y + h
                                {
                                    found_bounds = Some((x as i32, y as i32, w as i32, h as i32));
                                    break;
                                }
                            }
                        }

                        // Fallback to main display if not found
                        found_bounds.unwrap_or_else(|| {
                            let main = CGDisplay::main();
                            let bounds = main.bounds();
                            (
                                bounds.origin.x as i32,
                                bounds.origin.y as i32,
                                bounds.size.width as i32,
                                bounds.size.height as i32,
                            )
                        })
                    };

                    // Calculate initial position (centered horizontally on cursor, slightly below)
                    let mut new_x = mouse_x - window_width / 2;
                    let mut new_y = mouse_y + 10;

                    // Clamp to screen bounds with margins
                    let menu_bar_height = 25;
                    let edge_margin = 10; // margin from screen edges
                    let screen_left = screen_x + edge_margin;
                    let screen_right = screen_x + screen_width - window_width - edge_margin;
                    let screen_top = screen_y + menu_bar_height + edge_margin;
                    let screen_bottom = screen_y + screen_height - window_height - edge_margin;

                    // Clamp X position
                    if new_x < screen_left {
                        new_x = screen_left;
                    } else if new_x > screen_right {
                        new_x = screen_right;
                    }

                    // Clamp Y position
                    if new_y < screen_top {
                        new_y = screen_top;
                    } else if new_y > screen_bottom {
                        // If window would go below screen, show it above the cursor instead
                        new_y = mouse_y - window_height - 10;
                        if new_y < screen_top {
                            new_y = screen_top;
                        }
                    }

                    let _ = window.set_position(PhysicalPosition::new(new_x, new_y));
                }
            }
        }

        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(target_os = "macos")]
fn start_hotkey_listener(app_handle: AppHandle) {
    use std::sync::Mutex;

    println!("[Recall] Starting hotkey listener with NSEvent...");

    // Use static variables wrapped in Mutex for thread safety
    static LAST_OPTION_RELEASE: Mutex<Option<Instant>> = Mutex::new(None);
    static LAST_TRIGGER: Mutex<Option<Instant>> = Mutex::new(None);
    static OPTION_WAS_PRESSED: Mutex<bool> = Mutex::new(false);

    // Store app_handle in a thread-safe way
    static APP_HANDLE: Mutex<Option<AppHandle>> = Mutex::new(None);
    *APP_HANDLE.lock().unwrap() = Some(app_handle);

    // Run on separate thread since NSRunLoop blocks
    thread::spawn(move || {
        // Global monitor for when other apps are focused
        let global_block = StackBlock::new(|event: NonNull<NSEvent>| {
            let event = unsafe { event.as_ref() };
            let modifier_flags = event.modifierFlags();
            let option_pressed = modifier_flags.contains(NSEventModifierFlags::Option);

            let mut was_pressed = OPTION_WAS_PRESSED.lock().unwrap();
            let mut last_release = LAST_OPTION_RELEASE.lock().unwrap();
            let mut last_trigger = LAST_TRIGGER.lock().unwrap();

            if option_pressed {
                *was_pressed = true;
            } else if *was_pressed {
                *was_pressed = false;
                let now = Instant::now();

                if let Some(last) = *last_release {
                    let elapsed = now.duration_since(last).as_millis();
                    if elapsed < DOUBLE_TAP_THRESHOLD_MS {
                        println!("[Recall] Option double tap detected!");
                        if let Some(ref handle) = *APP_HANDLE.lock().unwrap() {
                            let _ = handle.emit("show-window-at-mouse", ());
                        }
                        *last_release = None;
                        *last_trigger = Some(now);
                        return;
                    }
                }
                *last_release = Some(now);
            }
        });

        let _ = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
            NSEventMask::FlagsChanged,
            &global_block,
        );

        // Local monitor for when our app is focused
        let local_block = StackBlock::new(|event: NonNull<NSEvent>| -> *mut NSEvent {
            let event_ref = unsafe { event.as_ref() };
            let modifier_flags = event_ref.modifierFlags();
            let option_pressed = modifier_flags.contains(NSEventModifierFlags::Option);

            let mut was_pressed = OPTION_WAS_PRESSED.lock().unwrap();
            let mut last_release = LAST_OPTION_RELEASE.lock().unwrap();
            let mut last_trigger = LAST_TRIGGER.lock().unwrap();

            if option_pressed {
                *was_pressed = true;
            } else if *was_pressed {
                *was_pressed = false;
                let now = Instant::now();

                if let Some(last) = *last_release {
                    let elapsed = now.duration_since(last).as_millis();
                    if elapsed < DOUBLE_TAP_THRESHOLD_MS {
                        println!("[Recall] Option double tap detected (local)!");
                        if let Some(ref handle) = *APP_HANDLE.lock().unwrap() {
                            let _ = handle.emit("show-window-at-mouse", ());
                        }
                        *last_release = None;
                        *last_trigger = Some(now);
                    }
                } else {
                    *last_release = Some(now);
                }
            }
            // Return the event as-is
            event.as_ptr()
        });

        // Safety: NSEvent::addLocalMonitorForEventsMatchingMask_handler requires unsafe
        // because it returns a nullable pointer
        unsafe {
            let _ = NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::FlagsChanged,
                &local_block,
            );
        }

        println!("[Recall] NSEvent global and local monitors registered");

        // Keep the thread alive and run the event loop
        let run_loop = NSRunLoop::currentRunLoop();
        run_loop.run();
    });
}

#[cfg(not(target_os = "macos"))]
fn start_hotkey_listener(_app_handle: AppHandle) {
    // No-op on non-macOS platforms
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {
            // Don't show window on second instance launch
            // Window should only be shown via Option key double-tap
        }))
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_history,
            copy_to_clipboard,
            toggle_pin,
            clear_all_history,
            restore_previous_app
        ])
        .setup(move |app| {
            // Start clipboard monitoring
            start_clipboard_monitor(app.handle().clone(), running_clone.clone());

            // Start hotkey listener for Option key double-tap
            start_hotkey_listener(app.handle().clone());

            // Listen for show-window-at-mouse event from hotkey listener
            let app_handle = app.handle().clone();
            app.listen("show-window-at-mouse", move |_| {
                show_window_at_mouse(&app_handle);
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Hide window instead of closing
                    let _ = window.hide();
                    api.prevent_close();
                }
                tauri::WindowEvent::Focused(false) => {
                    // Hide window when it loses focus (Spotlight-like behavior)
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen { .. } = event {
                // Dock icon clicked
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.center();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
