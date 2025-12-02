use arboard::Clipboard;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

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

fn main() {
    println!("Banzai - Clipboard Monitor");
    println!("履歴保存先: {:?}", get_history_path());
    println!("Ctrl+C で終了\n");

    let mut clipboard = Clipboard::new().expect("Failed to access clipboard");
    let mut last_content: Option<String> = None;

    loop {
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
}
