use arboard::Clipboard;
use auto_launch::AutoLaunchBuilder;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Listener, Manager, PhysicalPosition,
};

#[cfg(target_os = "macos")]
use block2::StackBlock;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags};
#[cfg(target_os = "macos")]
use objc2_foundation::NSRunLoop;
#[cfg(target_os = "macos")]
use std::ptr::NonNull;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub timestamp: DateTime<Local>,
    pub content: String,
}

const MAX_HISTORY_ENTRIES: usize = 100;
const DOUBLE_TAP_THRESHOLD_MS: u128 = 400;

fn get_data_dir() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("banzai");
    fs::create_dir_all(&data_dir).ok();
    data_dir
}

fn get_history_path() -> PathBuf {
    get_data_dir().join("clipboard_history.jsonl")
}

fn get_app_path() -> Option<String> {
    std::env::current_exe().ok().map(|exe_path| {
        let path_str = exe_path.to_string_lossy().to_string();
        if let Some(pos) = path_str.find(".app/") {
            path_str[..pos + 4].to_string()
        } else {
            path_str
        }
    })
}

fn create_auto_launch() -> Option<auto_launch::AutoLaunch> {
    let app_path = get_app_path()?;
    AutoLaunchBuilder::new()
        .set_app_name("Banzai")
        .set_app_path(&app_path)
        .set_use_launch_agent(true)
        .build()
        .ok()
}

fn is_auto_launch_enabled() -> bool {
    create_auto_launch()
        .map(|auto| auto.is_enabled().unwrap_or(false))
        .unwrap_or(false)
}

fn set_auto_launch(enabled: bool) -> Result<(), String> {
    let auto = create_auto_launch().ok_or("Failed to create auto launch")?;
    if enabled {
        auto.enable().map_err(|e| e.to_string())
    } else {
        auto.disable().map_err(|e| e.to_string())
    }
}

fn save_entry(entry: &ClipboardEntry) -> std::io::Result<()> {
    let path = get_history_path();

    let mut history = load_history();
    history.retain(|e| e.content != entry.content);

    history.push(ClipboardEntry {
        timestamp: entry.timestamp,
        content: entry.content.clone(),
    });

    if history.len() > MAX_HISTORY_ENTRIES {
        let start = history.len() - MAX_HISTORY_ENTRIES;
        history = history.split_off(start);
    }

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)?;
    for e in &history {
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

fn clear_history() -> std::io::Result<()> {
    let path = get_history_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
}

#[tauri::command]
fn get_history() -> Vec<ClipboardEntry> {
    let mut history = load_history();
    history.reverse();
    history
}

#[tauri::command]
fn copy_to_clipboard(content: String) -> Result<(), String> {
    let mut clipboard = Clipboard::new().map_err(|e| e.to_string())?;
    clipboard.set_text(&content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn clear_all_history() -> Result<(), String> {
    clear_history().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_auto_launch_status() -> bool {
    is_auto_launch_enabled()
}

#[tauri::command]
fn toggle_auto_launch(enabled: bool) -> Result<(), String> {
    set_auto_launch(enabled)
}

fn create_tray_menu(
    app: &AppHandle,
    history: &[ClipboardEntry],
) -> tauri::Result<Menu<tauri::Wry>> {
    let version = env!("CARGO_PKG_VERSION");

    let version_item = MenuItem::with_id(
        app,
        "version",
        format!("Banzai v{}", version),
        false,
        None::<&str>,
    )?;
    let separator1 = PredefinedMenuItem::separator(app)?;

    let auto_launch_enabled = is_auto_launch_enabled();
    let auto_launch = CheckMenuItem::with_id(
        app,
        "auto_launch",
        "ログイン時に起動",
        true,
        auto_launch_enabled,
        None::<&str>,
    )?;
    let clear = MenuItem::with_id(
        app,
        "clear",
        "履歴をクリア",
        !history.is_empty(),
        None::<&str>,
    )?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "終了", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &version_item,
            &separator1,
            &auto_launch,
            &clear,
            &separator2,
            &quit,
        ],
    )?;

    Ok(menu)
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
                    };

                    if let Err(e) = save_entry(&entry) {
                        log::error!("保存エラー: {}", e);
                    } else {
                        let _ = app_handle.emit("clipboard-changed", &entry);

                        // Update tray menu
                        if let Some(tray) = app_handle.tray_by_id("main") {
                            let history = load_history();
                            if let Ok(menu) = create_tray_menu(&app_handle, &history) {
                                let _ = tray.set_menu(Some(menu));
                            }
                        }
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
        // Only hide if already visible (to trigger re-show)
        // Skip hide on initial show to prevent flicker
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            let _ = window.hide();
        }

        // Get the current mouse position using AppleScript (macOS)
        #[cfg(target_os = "macos")]
        {
            use std::process::Command;
            if let Ok(output) = Command::new("osascript")
                .args([
                    "-e",
                    r#"
                    use framework "Foundation"
                    use framework "AppKit"
                    set mouseLocation to current application's NSEvent's mouseLocation()
                    set screenHeight to (current application's NSScreen's mainScreen()'s frame()'s |size|()'s height) as integer
                    set x to (mouseLocation's x) as integer
                    set y to (screenHeight - (mouseLocation's y as integer))
                    return (x as text) & "," & (y as text)
                    "#,
                ])
                .output()
            {
                if let Ok(pos_str) = String::from_utf8(output.stdout) {
                    let pos_str = pos_str.trim();
                    let parts: Vec<&str> = pos_str.split(',').collect();
                    if parts.len() == 2 {
                        if let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                            // Window size
                            let window_width = 400;

                            // Position window centered horizontally on cursor, slightly below
                            let new_x = x - window_width / 2;
                            let new_y = y + 10;

                            let _ = window.set_position(PhysicalPosition::new(new_x, new_y));
                        }
                    }
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

    println!("[Banzai] Starting hotkey listener with NSEvent...");

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
                        println!("[Banzai] Option double tap detected!");
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
                        println!("[Banzai] Option double tap detected (local)!");
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

        println!("[Banzai] NSEvent global and local monitors registered");

        // Keep the thread alive and run the event loop
        let run_loop = NSRunLoop::currentRunLoop();
        run_loop.run();
    });
}

#[cfg(not(target_os = "macos"))]
fn start_hotkey_listener(_app_handle: AppHandle) {
    // No-op on non-macOS platforms
}

fn create_icon() -> Image<'static> {
    let width = 22u32;
    let height = 22u32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let in_clip = (8..=13).contains(&x) && y <= 4;
            let in_board = (4..=17).contains(&x) && (3..=19).contains(&y);
            let in_paper = (6..=15).contains(&x) && (5..=17).contains(&y);

            if in_clip {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 255;
            } else if in_paper {
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            } else if in_board {
                rgba[idx] = 80;
                rgba[idx + 1] = 80;
                rgba[idx + 2] = 80;
                rgba[idx + 3] = 255;
            } else {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }

    Image::new_owned(rgba, width, height)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            get_history,
            copy_to_clipboard,
            clear_all_history,
            get_auto_launch_status,
            toggle_auto_launch
        ])
        .setup(move |app| {
            let history = load_history();
            let menu = create_tray_menu(app.handle(), &history)?;

            let _tray = TrayIconBuilder::with_id("main")
                .icon(create_icon())
                .menu(&menu)
                .tooltip("Banzai - Clipboard Monitor (Option×2で表示)")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "auto_launch" => {
                        let current = is_auto_launch_enabled();
                        if let Err(e) = set_auto_launch(!current) {
                            log::error!("自動起動設定エラー: {}", e);
                        }
                        // Update menu
                        if let Some(tray) = app.tray_by_id("main") {
                            let history = load_history();
                            if let Ok(menu) = create_tray_menu(app, &history) {
                                let _ = tray.set_menu(Some(menu));
                            }
                        }
                    }
                    "clear" => {
                        if let Err(e) = clear_history() {
                            log::error!("履歴クリアエラー: {}", e);
                        }
                        let _ = app.emit("history-cleared", ());
                        // Update menu
                        if let Some(tray) = app.tray_by_id("main") {
                            if let Ok(menu) = create_tray_menu(app, &[]) {
                                let _ = tray.set_menu(Some(menu));
                            }
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.center();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
