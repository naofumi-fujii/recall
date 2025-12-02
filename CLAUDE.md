# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Banzai is a macOS menu bar clipboard history monitor written in Rust. It runs as a system tray application that automatically detects and saves clipboard changes to a JSONL file.

## Build Commands

```bash
# Build for development
cargo build

# Build for release
cargo build --release

# Run the application
cargo run
```

## Architecture

Single-file application (`src/main.rs`) with these components:

- **Clipboard monitoring**: Background thread polls clipboard every 500ms using `arboard` crate
- **Persistence**: History stored as JSONL at `~/Library/Application Support/banzai/clipboard_history.jsonl`
- **System tray**: Uses `tao` event loop with `tray-icon` for menu bar integration
- **Inter-thread communication**: `mpsc` channel notifies main thread when history changes to trigger menu rebuild

Key constraints:
- Maximum 200 history entries (older entries trimmed automatically)
- Menu displays latest 10 entries with timestamps
- Clicking a history item copies it back to clipboard
