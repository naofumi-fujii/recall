// Minimal GPUI prototype of Recall, used to measure idle memory without a WebView.
// Implements only: tray icon, Option double-tap -> show window at mouse position,
// and hide (close) the window on focus loss. Clipboard monitoring is out of scope.

use std::cell::RefCell;
use std::fs;
use std::io::{BufRead, BufReader};
use std::ptr::NonNull;
use std::time::Instant;

use block2::StackBlock;
use core_graphics::display::CGDisplay;
use core_graphics::event::CGEvent;
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::StreamExt;
use gpui::{
    div, point, prelude::*, px, rgb, size, App, Application, Bounds, Context, SharedString,
    Subscription, Window, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSEvent, NSEventMask, NSEventModifierFlags,
};
use serde::Deserialize;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const DOUBLE_TAP_THRESHOLD_MS: u128 = 400;
const WINDOW_WIDTH: f32 = 500.;
const WINDOW_HEIGHT: f32 = 600.;
const MAX_DISPLAYED_ENTRIES: usize = 50;

/// Events delivered from AppKit callbacks (NSEvent monitors, tray menu) to the GPUI main loop.
enum AppEvent {
    Show,
    Quit,
}

/// Subset of `ClipboardEntry` in src-tauri/src/lib.rs, read-only for this prototype.
#[derive(Deserialize)]
struct ClipboardEntry {
    content: String,
    #[serde(default)]
    pinned: bool,
}

/// Reads the history written by the Tauri app (src-tauri/src/lib.rs get_history_path), newest first.
fn load_history() -> Vec<ClipboardEntry> {
    let path = dirs::data_local_dir()
        .unwrap_or_default()
        .join("recall/clipboard_history.jsonl");
    let Ok(file) = fs::File::open(path) else {
        return Vec::new();
    };
    let mut entries: Vec<ClipboardEntry> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect();
    entries.reverse();
    entries.truncate(MAX_DISPLAYED_ENTRIES);
    entries
}

/// Root view of the popup window: a read-only list of history entries.
struct HistoryView {
    entries: Vec<(SharedString, bool)>,
    _activation: Option<Subscription>,
}

impl HistoryView {
    /// Builds the view and closes the window as soon as it loses focus (Spotlight-like).
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let activation = cx.observe_window_activation(window, |_, window, _| {
            if !window.is_window_active() {
                println!("[Recall] focus lost, closing window");
                window.remove_window();
            }
        });
        let entries = load_history()
            .into_iter()
            .map(|e| {
                let line = e.content.lines().next().unwrap_or_default().to_string();
                (SharedString::from(line), e.pinned)
            })
            .collect();
        Self {
            entries,
            _activation: Some(activation),
        }
    }
}

impl Render for HistoryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("history")
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .bg(rgb(0x1e1e1e))
            .text_color(rgb(0xe0e0e0))
            .text_sm()
            .children(self.entries.iter().enumerate().map(|(i, (text, pinned))| {
                div()
                    .id(i)
                    // Keep row height; otherwise rows shrink to fit the scroll container
                    .flex_none()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(rgb(0x333333))
                    .hover(|s| s.bg(rgb(0x2d2d2d)))
                    .truncate()
                    .child(if *pinned {
                        format!("📌 {text}")
                    } else {
                        text.to_string()
                    })
            }))
    }
}

/// Computes the window origin near the mouse cursor, clamped to the display containing it.
/// Mirrors show_window_at_mouse in src-tauri/src/lib.rs. Returns (display id, display-local origin).
fn window_origin_at_mouse() -> Option<(u32, f32, f32)> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let location = CGEvent::new(source).ok()?.location();
    let display = CGDisplay::active_displays()
        .ok()?
        .into_iter()
        .map(CGDisplay::new)
        .find(|d| {
            let b = d.bounds();
            location.x >= b.origin.x
                && location.x < b.origin.x + b.size.width
                && location.y >= b.origin.y
                && location.y < b.origin.y + b.size.height
        })
        .unwrap_or_else(CGDisplay::main);
    let b = display.bounds();

    // Work in display-local coordinates since GPUI's WindowBounds are relative to the display
    let (w, h) = (WINDOW_WIDTH as f64, WINDOW_HEIGHT as f64);
    let (mx, my) = (location.x - b.origin.x, location.y - b.origin.y);
    let (menu_bar, margin) = (25., 10.);
    let x = (mx - w / 2.).clamp(margin, (b.size.width - w - margin).max(margin));
    let mut y = my + 10.;
    if y + h > b.size.height - margin {
        y = my - h - 10.;
    }
    let y = y.max(menu_bar + margin);
    Some((display.id, x as f32, y as f32))
}

/// Opens the popup window at the mouse position, replacing any window already shown.
fn show_window_at_mouse(current: &RefCell<Option<WindowHandle<HistoryView>>>, cx: &mut App) {
    if let Some(handle) = current.borrow_mut().take() {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }

    let (display_id, x, y) = window_origin_at_mouse().unwrap_or((0, 100., 100.));
    let display_id = cx
        .displays()
        .into_iter()
        .find(|d| u32::from(d.id()) == display_id)
        .map(|d| d.id());

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(x), px(y)),
            size: size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)),
        })),
        titlebar: None,
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_movable: true,
        display_id,
        ..Default::default()
    };
    match cx.open_window(options, |window, cx| {
        cx.new(|cx| HistoryView::new(window, cx))
    }) {
        Ok(handle) => *current.borrow_mut() = Some(handle),
        Err(e) => eprintln!("[Recall] failed to open window: {e}"),
    }
    // Accessory apps are not activated automatically; bring the window to the front
    cx.activate(true);
}

/// Registers global/local NSEvent monitors that send `AppEvent::Show` on an Option double-tap.
/// Same detection logic as start_hotkey_listener in src-tauri/src/lib.rs, but on the main thread.
fn start_hotkey_listener(tx: UnboundedSender<AppEvent>) {
    // State shared by both monitors: (option currently pressed, time of last release)
    thread_local! {
        static STATE: RefCell<(bool, Option<Instant>)> = const { RefCell::new((false, None)) };
    }
    let on_flags_changed = move |event: &NSEvent| {
        let option_pressed = event.modifierFlags().contains(NSEventModifierFlags::Option);
        STATE.with_borrow_mut(|(was_pressed, last_release)| {
            if option_pressed {
                *was_pressed = true;
                return;
            }
            if !*was_pressed {
                return;
            }
            *was_pressed = false;
            let now = Instant::now();
            match last_release.take() {
                Some(last) if now.duration_since(last).as_millis() < DOUBLE_TAP_THRESHOLD_MS => {
                    let _ = tx.unbounded_send(AppEvent::Show);
                }
                _ => *last_release = Some(now),
            }
        });
    };
    let on_flags_changed_local = on_flags_changed.clone();

    let global_block = StackBlock::new(move |event: NonNull<NSEvent>| {
        on_flags_changed(unsafe { event.as_ref() });
    });
    let local_block = StackBlock::new(move |event: NonNull<NSEvent>| -> *mut NSEvent {
        on_flags_changed_local(unsafe { event.as_ref() });
        event.as_ptr()
    });

    // Monitors live for the whole process; leak them so they are never removed
    let global = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(
        NSEventMask::FlagsChanged,
        &global_block,
    );
    let local = unsafe {
        NSEvent::addLocalMonitorForEventsMatchingMask_handler(
            NSEventMask::FlagsChanged,
            &local_block,
        )
    };
    std::mem::forget(global);
    std::mem::forget(local);
}

/// Creates the menu bar tray icon with "Show" and "Quit" items forwarded as `AppEvent`s.
fn create_tray_icon(tx: UnboundedSender<AppEvent>) -> TrayIcon {
    let show = MenuItem::new("Show", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let (show_id, quit_id) = (show.id().clone(), quit.id().clone());
    let menu = Menu::new();
    menu.append_items(&[&show, &quit])
        .expect("failed to build tray menu");

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id == show_id {
            let _ = tx.unbounded_send(AppEvent::Show);
        } else if event.id == quit_id {
            let _ = tx.unbounded_send(AppEvent::Quit);
        }
    }));

    TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(tray_icon_image())
        .with_icon_as_template(true)
        .with_tooltip("Recall (GPUI prototype)")
        .build()
        .expect("failed to create tray icon")
}

/// Draws a simple filled circle as a template tray icon, to avoid depending on image assets.
fn tray_icon_image() -> Icon {
    const N: u32 = 32;
    let r = N as f32 / 2. - 3.;
    let rgba = (0..N * N)
        .flat_map(|i| {
            let (x, y) = (
                (i % N) as f32 + 0.5 - N as f32 / 2.,
                (i / N) as f32 + 0.5 - N as f32 / 2.,
            );
            let alpha = if (x * x + y * y).sqrt() <= r { 255 } else { 0 };
            [0, 0, 0, alpha]
        })
        .collect();
    Icon::from_rgba(rgba, N, N).expect("invalid tray icon")
}

fn main() {
    Application::new().run(|cx: &mut App| {
        // GPUI sets the Regular activation policy on launch; switch to Accessory to hide the Dock icon
        let mtm = MainThreadMarker::new().expect("must run on the main thread");
        NSApplication::sharedApplication(mtm)
            .setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        let (tx, mut rx) = unbounded::<AppEvent>();
        start_hotkey_listener(tx.clone());
        let tray = create_tray_icon(tx.clone());

        // For memory measurement: open the window once without needing the hotkey
        if std::env::args().any(|a| a == "--show-on-launch") {
            let _ = tx.unbounded_send(AppEvent::Show);
        }

        cx.spawn(async move |cx| {
            let _tray = tray;
            let current = RefCell::new(None);
            while let Some(event) = rx.next().await {
                let result = cx.update(|cx| match event {
                    AppEvent::Show => show_window_at_mouse(&current, cx),
                    AppEvent::Quit => cx.quit(),
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    });
}
