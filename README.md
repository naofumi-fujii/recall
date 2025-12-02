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

## Build App Bundle

```bash
cargo install cargo-bundle
cargo bundle --release
```

The app bundle is created at `target/release/bundle/osx/Banzai.app`.

To install to Applications:
```bash
cp -r target/release/bundle/osx/Banzai.app /Applications/
```

## Usage

```bash
# Run directly
cargo run

# Or open the app bundle
open target/release/bundle/osx/Banzai.app
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
