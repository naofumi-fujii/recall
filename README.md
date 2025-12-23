# Banzai

A macOS menu bar clipboard history manager built with Tauri 2.

## Features

- **Clipboard Monitoring**: Automatically detects and saves clipboard changes (polling every 500ms)
- **Menu Bar App**: Runs quietly in the menu bar as a background application
- **Quick Access**: Double-tap the Option key to instantly open the history window
- **Search**: Filter clipboard history with real-time search
- **Theme Support**: System, Light, and Dark themes
- **Auto Launch**: Option to start automatically at login
- **Persistent Storage**: History saved in JSONL format (max 100 entries)
- **Duplicate Removal**: Automatically removes duplicates, keeping the most recent

## Installation

### Homebrew (recommended)
- https://github.com/naofumi-fujii/homebrew-banzai

```bash
brew tap naofumi-fujii/banzai
brew install --cask banzai
```

### Build from Source

Prerequisites:
- Node.js
- Rust

```bash
npm install
npm run tauri build
```

The app bundle is created at `src-tauri/target/release/bundle/macos/Banzai.app`.

## Usage

1. Launch Banzai - a clipboard icon appears in the menu bar
2. Copy text as usual - it's automatically saved to history
3. Double-tap the **Option** key to open the history window
4. Click any entry to copy it back to clipboard (window closes automatically)
5. Use the search box to filter entries
6. Right-click the tray icon for options (Clear History, Auto Launch, Quit)

## Development

```bash
npm install
npm run tauri dev
```

## Data Storage

History location: `~/Library/Application Support/banzai/clipboard_history.jsonl`

## Release

Run the release script with the desired bump type:

```bash
./scripts/release.sh patch   # 0.12.0 -> 0.12.1
./scripts/release.sh minor   # 0.12.0 -> 0.13.0
./scripts/release.sh major   # 0.12.0 -> 1.0.0
```

This updates the version in all required files, creates a git commit and tag, and pushes to the repository. GitHub Actions will automatically build and publish the release.

## License

MIT
