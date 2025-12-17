# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Banzai is a macOS menu bar clipboard history manager built with Tauri v2. It monitors the system clipboard, stores history entries in a local JSONL file, and provides a Spotlight-like popup UI triggered by double-tapping the Option key.

## Development Commands

```bash
# Development
npm run dev              # Start frontend dev server only
npm run tauri dev        # Start full Tauri development (frontend + Rust backend)

# Build
npm run build            # Build frontend only
npm run tauri build      # Build production app bundle (.app, .dmg)

# Linting & Formatting
npx tsc --noEmit                      # TypeScript type check
cd src-tauri && cargo fmt --check     # Check Rust formatting
cd src-tauri && cargo clippy          # Rust linting
cd src-tauri && cargo test            # Run Rust tests
```

## Architecture

### Tech Stack
- **Frontend**: React 19 + TypeScript + Vite
- **Backend**: Rust + Tauri v2
- **Platform**: macOS only (uses private APIs for global hotkey detection)

### Key Components

**Rust Backend (`src-tauri/src/lib.rs`)**
- `ClipboardEntry` struct with timestamp, content, and pinned flag
- Clipboard monitoring thread polls every 500ms using `arboard` crate
- History stored in `~/Library/Application Support/banzai/clipboard_history.jsonl`
- Global hotkey detection using `NSEvent` monitors for Option key double-tap
- Window positioning logic handles multi-monitor setups via `core-graphics`

**React Frontend (`src/App.tsx`)**
- Single-page UI with keyboard navigation (j/k/arrows, Enter to copy)
- Theme switching (system/light/dark)
- Pin functionality to prevent items from being trimmed
- Listens for `clipboard-changed` and `show-window-at-mouse` events from Rust

### Tauri Commands
- `get_history()` - Returns clipboard history (newest first)
- `copy_to_clipboard(content)` - Copies text and hides window
- `toggle_pin(timestamp, pinned)` - Toggles pin state
- `clear_all_history()` - Clears unpinned entries

### Important Behaviors
- Window hides on focus loss (Spotlight-like)
- Close button hides instead of quitting
- History limited to 100 entries (pinned items preserved)
- Double-tap Option key shows window at mouse cursor position
