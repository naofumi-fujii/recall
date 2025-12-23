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

### Using GitHub Actions (Recommended)

1. Go to [Actions](https://github.com/naofumi-fujii/banzai/actions/workflows/release.yml) tab
2. Click "Run workflow"
3. Select the version bump type (patch/minor/major)
4. Click "Run workflow"

The workflow will automatically update versions, create a tag, build universal binaries (x86_64 + arm64), and publish the release.

## License

MIT
