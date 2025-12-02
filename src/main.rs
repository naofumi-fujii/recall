use arboard::Clipboard;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

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

const MAX_HISTORY_ENTRIES: usize = 100;

fn save_entry(entry: &ClipboardEntry) -> std::io::Result<()> {
    let path = get_history_path();

    // 既存の履歴を読み込み、重複を削除
    let mut history = load_history();
    history.retain(|e| e.content != entry.content);

    // 新しいエントリを追加
    history.push(ClipboardEntry {
        timestamp: entry.timestamp,
        content: entry.content.clone(),
    });

    // 履歴件数が上限を超えた場合、古いエントリを削除
    if history.len() > MAX_HISTORY_ENTRIES {
        let start = history.len() - MAX_HISTORY_ENTRIES;
        history = history.split_off(start);
    }

    // ファイルを書き直す
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

struct MenuIds {
    quit_id: tray_icon::menu::MenuId,
    clear_id: tray_icon::menu::MenuId,
    history_items: Vec<(tray_icon::menu::MenuId, String)>,
}

fn create_tray_menu(history: &[ClipboardEntry]) -> (Menu, MenuIds) {
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
            entry.timestamp.format("%Y/%m/%d %H:%M:%S"),
            truncate_for_display(&entry.content, 40)
        );
        let item = MenuItem::new(&display_text, true, None);
        let id = item.id().clone();
        history_items.push((id, entry.content.clone()));
        menu.append(&item).unwrap();
    }

    // 区切り線
    menu.append(&PredefinedMenuItem::separator()).unwrap();

    // 履歴クリアボタン
    let clear_item = MenuItem::new("履歴をクリア", !history.is_empty(), None);
    let clear_id = clear_item.id().clone();
    menu.append(&clear_item).unwrap();

    // 終了ボタン
    let quit_item = MenuItem::new("終了", true, None);
    let quit_id = quit_item.id().clone();
    menu.append(&quit_item).unwrap();

    (menu, MenuIds { quit_id, clear_id, history_items })
}

fn rebuild_tray_icon(history: &[ClipboardEntry]) -> (TrayIcon, MenuIds) {
    let (menu, menu_ids) = create_tray_menu(history);

    let tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Banzai - Clipboard Monitor")
        .with_icon(create_icon())
        .build()
        .expect("Failed to create tray icon");

    (tray_icon, menu_ids)
}

fn clear_history() -> std::io::Result<()> {
    let path = get_history_path();
    if path.exists() {
        fs::remove_file(&path)?;
    }
    Ok(())
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


fn start_clipboard_monitor(running: Arc<AtomicBool>, history_changed_sender: mpsc::Sender<()>) {
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

                        if let Err(e) = save_entry(&entry) {
                            eprintln!("保存エラー: {}", e);
                        } else {
                            // 履歴変更を通知
                            let _ = history_changed_sender.send(());
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
    println!("メニューバーに常駐中...\n");

    let running = Arc::new(AtomicBool::new(true));

    // 履歴変更通知用チャネル
    let (history_changed_sender, history_changed_receiver) = mpsc::channel();

    // Start clipboard monitoring in background thread
    start_clipboard_monitor(running.clone(), history_changed_sender);

    // Create event loop for tray icon
    let event_loop = EventLoopBuilder::<()>::with_user_event().build();

    // Create tray icon with history menu
    let history = load_history();
    let (tray_icon, mut menu_ids) = rebuild_tray_icon(&history);
    let mut tray_icon: Option<TrayIcon> = Some(tray_icon);
    let menu_channel = MenuEvent::receiver();

    event_loop.run(move |_event, _event_loop_window_target, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(500));

        // Handle menu events
        if let Ok(menu_event) = menu_channel.try_recv() {
            if menu_event.id == menu_ids.quit_id {
                running.store(false, Ordering::Relaxed);
                *control_flow = ControlFlow::Exit;
            } else if menu_event.id == menu_ids.clear_id {
                // 履歴をクリア
                if let Err(e) = clear_history() {
                    eprintln!("履歴クリアエラー: {}", e);
                }
                // メニューを更新
                tray_icon.take();
                let result = rebuild_tray_icon(&[]);
                tray_icon = Some(result.0);
                menu_ids = result.1;
            } else {
                // Check if it's a history item click
                for (id, content) in &menu_ids.history_items {
                    if menu_event.id == *id {
                        // Copy content to clipboard
                        if let Ok(mut clipboard) = Clipboard::new() {
                            if let Err(e) = clipboard.set_text(content.clone()) {
                                eprintln!("クリップボードへのコピーに失敗: {}", e);
                            }
                        }
                        break;
                    }
                }
            }
        }

        // 履歴変更通知を受け取ったときだけメニューを更新
        if let Ok(()) = history_changed_receiver.try_recv() {
            let current_history = load_history();
            // Rebuild tray icon with updated menu
            tray_icon.take(); // Drop the old tray icon
            let result = rebuild_tray_icon(&current_history);
            tray_icon = Some(result.0);
            menu_ids = result.1;
        }

    });
}
