use arboard::Clipboard;
use chrono::{DateTime, Local};
use mouse_position::mouse_position::Mouse;
use rdev::{listen, Event as RdevEvent, EventType, Key};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use wry::WebViewBuilder;

#[derive(Debug, Serialize, Deserialize)]
struct ClipboardEntry {
    timestamp: DateTime<Local>,
    content: String,
}

fn get_history_path() -> PathBuf {
    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("banzai");
    fs::create_dir_all(&data_dir).ok();
    data_dir.join("clipboard_history.jsonl")
}

fn save_entry(entry: &ClipboardEntry) -> std::io::Result<()> {
    let path = get_history_path();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    let json = serde_json::to_string(entry)?;
    writeln!(file, "{}", json)?;
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
        .filter_map(|line| line.ok())
        .filter_map(|line| serde_json::from_str(&line).ok())
        .collect()
}

fn truncate_for_display(s: &str, max_len: usize) -> String {
    // 改行を除去して、指定長で切り詰め
    let single_line: String = s.chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    if single_line.chars().count() > max_len {
        let truncated: String = single_line.chars().take(max_len).collect();
        format!("{}...", truncated)
    } else {
        single_line
    }
}

fn create_tray_menu(history: &[ClipboardEntry]) -> (Menu, tray_icon::menu::MenuId, Vec<(tray_icon::menu::MenuId, String)>) {
    let menu = Menu::new();

    // 履歴件数表示
    let status_item = MenuItem::new(format!("履歴: {} 件", history.len()), false, None);
    menu.append(&status_item).unwrap();

    // 区切り線
    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // 最新10件の履歴をメニューに追加
    let mut history_items: Vec<(tray_icon::menu::MenuId, String)> = Vec::new();
    for entry in history.iter().rev().take(10) {
        let display_text = format!(
            "[{}] {}",
            entry.timestamp.format("%H:%M"),
            truncate_for_display(&entry.content, 40)
        );
        let item = MenuItem::new(&display_text, true, None);
        let id = item.id().clone();
        history_items.push((id, entry.content.clone()));
        menu.append(&item).unwrap();
    }

    // 区切り線
    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // 終了ボタン
    let quit_item = MenuItem::new("終了", true, None);
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item).unwrap();

    (menu, quit_id, history_items)
}

fn rebuild_tray_icon(history: &[ClipboardEntry]) -> (TrayIcon, tray_icon::menu::MenuId, Vec<(tray_icon::menu::MenuId, String)>) {
    let (menu, quit_id, history_items) = create_tray_menu(history);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Banzai - Clipboard Monitor")
        .with_icon(create_icon())
        .build()
        .expect("Failed to create tray icon");

    (tray_icon, quit_id, history_items)
}

fn create_icon() -> Icon {
    // 16x16 simple clipboard icon (RGBA)
    let width = 16u32;
    let height = 16u32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];

    // Draw a simple clipboard shape
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let in_clip = x >= 5 && x <= 10 && y <= 3;
            let in_board = x >= 2 && x <= 13 && y >= 2 && y <= 14;
            let in_paper = x >= 4 && x <= 11 && y >= 4 && y <= 12;

            if in_clip {
                // Clip part - dark gray
                rgba[idx] = 80;
                rgba[idx + 1] = 80;
                rgba[idx + 2] = 80;
                rgba[idx + 3] = 255;
            } else if in_paper {
                // Paper - white
                rgba[idx] = 255;
                rgba[idx + 1] = 255;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            } else if in_board {
                // Board - brown
                rgba[idx] = 139;
                rgba[idx + 1] = 90;
                rgba[idx + 2] = 43;
                rgba[idx + 3] = 255;
            } else {
                // Transparent
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }

    Icon::from_rgba(rgba, width, height).expect("Failed to create icon")
}

fn get_mouse_position() -> (i32, i32) {
    match Mouse::get_mouse_position() {
        Mouse::Position { x, y } => (x, y),
        Mouse::Error => (100, 100), // フォールバック位置
    }
}

fn generate_popup_html(history: &[ClipboardEntry]) -> String {
    let mut items_html = String::new();

    for (idx, entry) in history.iter().rev().take(10).enumerate() {
        let display_text = truncate_for_display(&entry.content, 60);
        let escaped_content = entry
            .content
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        let time_str = entry.timestamp.format("%H:%M").to_string();

        items_html.push_str(&format!(
            r#"<div class="item" onclick="selectItem('{}')" data-index="{}">
                <span class="time">[{}]</span>
                <span class="content">{}</span>
            </div>"#,
            escaped_content,
            idx,
            time_str,
            html_escape(&display_text)
        ));
    }

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            font-size: 13px;
            background: #2d2d2d;
            color: #e0e0e0;
            overflow: hidden;
            border-radius: 8px;
        }}
        .header {{
            padding: 8px 12px;
            background: #3d3d3d;
            border-bottom: 1px solid #4d4d4d;
            font-weight: 600;
            font-size: 12px;
            color: #aaa;
        }}
        .list {{
            max-height: 300px;
            overflow-y: auto;
        }}
        .item {{
            padding: 8px 12px;
            cursor: pointer;
            border-bottom: 1px solid #3d3d3d;
            display: flex;
            align-items: center;
            gap: 8px;
            transition: background 0.1s;
        }}
        .item:hover {{
            background: #4a4a4a;
        }}
        .item:active {{
            background: #5a5a5a;
        }}
        .time {{
            color: #888;
            font-size: 11px;
            flex-shrink: 0;
        }}
        .content {{
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}
        .empty {{
            padding: 20px;
            text-align: center;
            color: #888;
        }}
        .footer {{
            padding: 6px 12px;
            background: #3d3d3d;
            border-top: 1px solid #4d4d4d;
            font-size: 11px;
            color: #666;
            text-align: center;
        }}
    </style>
</head>
<body>
    <div class="header">クリップボード履歴</div>
    <div class="list">
        {}
    </div>
    <div class="footer">クリックでコピー / Escで閉じる</div>
    <script>
        function selectItem(content) {{
            window.ipc.postMessage('copy:' + content);
        }}
        document.addEventListener('keydown', function(e) {{
            if (e.key === 'Escape') {{
                window.ipc.postMessage('close');
            }}
        }});
        // フォーカスを受け取る
        window.focus();
    </script>
</body>
</html>"#,
        if items_html.is_empty() {
            r#"<div class="empty">履歴がありません</div>"#.to_string()
        } else {
            items_html
        }
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// カスタムイベント（IPCからメインループへの通知用）
#[derive(Debug, Clone)]
enum UserEvent {
    ClosePopup,
    CopyAndClose(String),
}

fn start_hotkey_listener(hotkey_sender: mpsc::Sender<()>) {
    thread::spawn(move || {
        let mut last_alt_release: Option<Instant> = None;
        let double_tap_threshold = Duration::from_millis(400);

        let callback = move |event: RdevEvent| {
            // Altキー（左右両方）のリリースを検出
            if let EventType::KeyRelease(key) = event.event_type {
                if matches!(key, Key::Alt | Key::AltGr) {
                    let now = Instant::now();

                    if let Some(last_time) = last_alt_release {
                        if now.duration_since(last_time) < double_tap_threshold {
                            // ダブルタップ検出！
                            println!("Alt double-tap detected!");
                            let _ = hotkey_sender.send(());
                            last_alt_release = None;
                            return;
                        }
                    }
                    last_alt_release = Some(now);
                }
            }
        };

        if let Err(e) = listen(callback) {
            eprintln!("ホットキーリスナーエラー: {:?}", e);
        }
    });
}

fn start_clipboard_monitor(running: Arc<AtomicBool>) {
    thread::spawn(move || {
        let mut clipboard = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to access clipboard: {}", e);
                return;
            }
        };
        let mut last_content: Option<String> = None;

        while running.load(Ordering::Relaxed) {
            match clipboard.get_text() {
                Ok(current) => {
                    let is_new = match &last_content {
                        Some(last) => last != &current,
                        None => true,
                    };

                    if is_new && !current.is_empty() {
                        let entry = ClipboardEntry {
                            timestamp: Local::now(),
                            content: current.clone(),
                        };

                        println!(
                            "[{}] {}",
                            entry.timestamp.format("%H:%M:%S"),
                            if current.len() > 50 {
                                format!("{}...", &current[..50])
                            } else {
                                current.clone()
                            }
                        );

                        if let Err(e) = save_entry(&entry) {
                            eprintln!("保存エラー: {}", e);
                        }

                        last_content = Some(current);
                    }
                }
                Err(_) => {}
            }

            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn main() {
    println!("Banzai - Clipboard Monitor");
    println!("履歴保存先: {:?}", get_history_path());
    println!("ショートカット: Altキー2回タップで起動");
    println!("メニューバーに常駐中...\n");

    let running = Arc::new(AtomicBool::new(true));

    // Start clipboard monitoring in background thread
    start_clipboard_monitor(running.clone());

    // Start hotkey listener for Alt double-tap
    let (hotkey_sender, hotkey_receiver) = mpsc::channel();
    start_hotkey_listener(hotkey_sender);

    // Create event loop for tray icon (with custom UserEvent)
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let event_loop_proxy = event_loop.create_proxy();

    // Create tray icon with history menu
    let history = load_history();
    let (tray_icon, mut quit_id, mut history_items) = rebuild_tray_icon(&history);
    let mut tray_icon: Option<TrayIcon> = Some(tray_icon);
    let mut last_history_count = history.len();

    let menu_channel = MenuEvent::receiver();

    // ポップアップウィンドウ用の状態
    let mut popup_window: Option<tao::window::Window> = None;
    let mut popup_webview: Option<wry::WebView> = None;

    event_loop.run(move |event, event_loop_window_target, control_flow| {
        *control_flow = ControlFlow::Poll;

        if let Event::NewEvents(StartCause::Init) = event {
            // Tray icon is already created
        }

        // ウィンドウイベントの処理
        if let Event::WindowEvent {
            event: window_event,
            window_id,
            ..
        } = &event
        {
            // ポップアップウィンドウのイベント処理
            if let Some(ref window) = popup_window {
                if window.id() == *window_id {
                    match window_event {
                        WindowEvent::CloseRequested | WindowEvent::Focused(false) => {
                            // フォーカスを失った時またはクローズ時にウィンドウを閉じる
                            popup_webview.take();
                            popup_window.take();
                        }
                        _ => {}
                    }
                }
            }
        }

        // Handle menu events
        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == quit_id {
                running.store(false, Ordering::Relaxed);
                *control_flow = ControlFlow::Exit;
            } else {
                // Check if it's a history item click
                for (id, content) in &history_items {
                    if menu_event.id == *id {
                        // Copy content to clipboard
                        if let Ok(mut clipboard) = Clipboard::new() {
                            if let Err(e) = clipboard.set_text(content.clone()) {
                                eprintln!("クリップボードへのコピーに失敗: {}", e);
                            } else {
                                println!("コピーしました: {}", truncate_for_display(content, 50));
                            }
                        }
                        break;
                    }
                }
            }
        }

        // Periodically refresh menu when history changes
        let current_history = load_history();
        if current_history.len() != last_history_count {
            // Rebuild tray icon with updated menu
            tray_icon.take(); // Drop the old tray icon
            let result = rebuild_tray_icon(&current_history);
            tray_icon = Some(result.0);
            quit_id = result.1;
            history_items = result.2;
            last_history_count = current_history.len();
        }

        // Handle hotkey events (Alt double-tap)
        if let Ok(()) = hotkey_receiver.try_recv() {
            println!("Hotkey activated! Showing clipboard history popup...");

            // 既存のポップアップがあれば閉じる
            popup_webview.take();
            popup_window.take();

            // マウス位置を取得
            let (mouse_x, mouse_y) = get_mouse_position();

            // ポップアップウィンドウを作成
            let window = WindowBuilder::new()
                .with_title("クリップボード履歴")
                .with_inner_size(LogicalSize::new(350.0, 400.0))
                .with_position(LogicalPosition::new(mouse_x as f64, mouse_y as f64))
                .with_decorations(false)
                .with_always_on_top(true)
                .with_resizable(false)
                .build(event_loop_window_target)
                .expect("Failed to create popup window");

            // 履歴を読み込んでHTMLを生成
            let history = load_history();
            let html = generate_popup_html(&history);

            // WebViewを作成
            let proxy = event_loop_proxy.clone();
            let webview = WebViewBuilder::new()
                .with_html(&html)
                .with_ipc_handler(move |message| {
                    let msg = message.body();
                    if msg == "close" {
                        // ウィンドウを閉じるリクエスト
                        let _ = proxy.send_event(UserEvent::ClosePopup);
                    } else if let Some(content) = msg.strip_prefix("copy:") {
                        // コンテンツをクリップボードにコピー＆閉じる
                        let content = content
                            .replace("\\n", "\n")
                            .replace("\\r", "\r")
                            .replace("\\'", "'")
                            .replace("\\\\", "\\");
                        let _ = proxy.send_event(UserEvent::CopyAndClose(content));
                    }
                })
                .build(&window)
                .expect("Failed to create webview");

            popup_window = Some(window);
            popup_webview = Some(webview);
        }

        // Handle custom UserEvent (from IPC handler)
        if let Event::UserEvent(user_event) = &event {
            match user_event {
                UserEvent::ClosePopup => {
                    popup_webview.take();
                    popup_window.take();
                }
                UserEvent::CopyAndClose(content) => {
                    // クリップボードにコピー
                    if let Ok(mut clipboard) = Clipboard::new() {
                        if let Err(e) = clipboard.set_text(content.clone()) {
                            eprintln!("クリップボードへのコピーに失敗: {}", e);
                        } else {
                            println!("コピーしました: {}", truncate_for_display(content, 50));
                        }
                    }
                    // ウィンドウを閉じる
                    popup_webview.take();
                    popup_window.take();
                }
            }
        }

        // Small sleep to prevent busy loop
        thread::sleep(Duration::from_millis(100));
    });
}
