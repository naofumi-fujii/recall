# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Banzai is a macOS menu bar clipboard history manager built with Tauri 2 (Rust backend + React frontend).

## Common Commands

### Development
```bash
npm run tauri dev     # Start development server with hot reload
```

### Build
```bash
npm run tauri build   # Build production app bundle
```

### Linting & Type Checking
```bash
npx tsc --noEmit                      # TypeScript type check
cd src-tauri && cargo fmt --check     # Rust formatting check
cd src-tauri && cargo clippy -- -D warnings  # Rust linting
```

### Testing
```bash
cd src-tauri && cargo test            # Run Rust tests
```

### Release
GitHub Actions → "Release" workflow → "Run workflow" → patch/minor/major を選択して実行

ローカルでのデバッグ:
```bash
./scripts/release.sh patch   # 0.12.0 -> 0.12.1
./scripts/release.sh minor   # 0.12.0 -> 0.13.0
./scripts/release.sh major   # 0.12.0 -> 1.0.0
```

自動的に以下を行う:
- `src-tauri/Cargo.toml`, `package.json`, `src-tauri/tauri.conf.json` のバージョン更新
- コミット & タグ作成
- ビルド & GitHubリリース作成

## Architecture

### Backend (Rust - src-tauri/)
- `src/lib.rs`: Main application logic
  - Clipboard monitoring (polling every 500ms using arboard)
  - Tray icon and menu management
  - Global hotkey listener (Option key double-tap via NSEvent)
  - JSONL-based history persistence (max 100 entries)
  - Tauri commands: `get_history`, `copy_to_clipboard`, `clear_all_history`, `get_auto_launch_status`, `toggle_auto_launch`

### Frontend (React - src/)
- `App.tsx`: Single-page clipboard history viewer with search and theme support
- Communicates with backend via Tauri invoke API
- Listens for `clipboard-changed` and `history-cleared` events

### Data Storage
- History location: `~/Library/Application Support/banzai/clipboard_history.jsonl`
- Each entry: `{ timestamp: DateTime, content: String }`

### Key Dependencies
- Tauri 2 with `tray-icon` and `macos-private-api` features
- arboard for cross-platform clipboard access
- cocoa/objc for macOS-specific NSEvent handling
