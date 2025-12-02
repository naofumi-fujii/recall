# banzai

A macOS menu bar clipboard history manager.

## Features

- Automatically detects and saves clipboard changes
- Runs in the menu bar as a background application
- Persists history in JSONL format
- Removes duplicate entries (keeps the latest)
- Clear history from menu

## Installation

```bash
cargo build --release
```

## Usage

```bash
cargo run
```

After launching, a clipboard icon appears in the menu bar.
Copied content is automatically saved to history.

## History Location

- macOS: `~/Library/Application Support/banzai/clipboard_history.jsonl`

## Dependencies

- `arboard` - Clipboard access
- `chrono` - Timestamps
- `serde` / `serde_json` - Serialization
- `dirs` - Platform-specific directory paths
- `tao` / `tray-icon` - System tray
