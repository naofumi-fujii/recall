# GPUI prototype

Minimal [GPUI](https://www.gpui.rs/) version of Recall, built to check whether dropping WKWebView reduces memory usage (see issue #204).

It implements only:

1. Menu bar tray icon (`tray-icon`) with Show / Quit, no Dock icon (Accessory activation policy)
2. Option double-tap → open the window at the mouse position (same `NSEvent` monitor logic as `src-tauri/src/lib.rs`)
3. Close the window on focus loss

The window shows a read-only list of the latest 50 entries from the existing `clipboard_history.jsonl`. Clipboard monitoring, copy, pinning, and keyboard navigation are not implemented.

## Run

```bash
cd prototypes/gpui
cargo run --release                     # wait for Option double-tap
cargo run --release -- --show-on-launch # open the window once on launch (for measurement)
```

The `runtime_shaders` feature compiles Metal shaders at runtime, so the Xcode Metal Toolchain is not required.

## Results

Measured with `footprint` on macOS (Apple Silicon, release build, gpui 0.2.1).

| State | GPUI prototype | Tauri (Recall 2.1.24) |
|---|---|---|
| Idle, window never opened | 12 MB | ~75-80 MB (recall 32 MB + WebKit helpers) |
| Window open (50 entries) | 35-43 MB (peak 86 MB) | ~75-80 MB |
| After the window was closed | 33-39 MB | ~75-80 MB (window is only hidden) |

- Single process; no WebContent / GPU / Networking helper processes.
- Memory is not returned after closing the window (GPU atlas, IOSurface, font caches, malloc), so the realistic steady state is ~35-40 MB, about half of the Tauri app.

## Findings

- Tray icon: GPUI has no tray API; `tray-icon` can be created alongside it on the main thread. Clicking the menu and the Option double-tap have not been verified manually yet.
- No Dock icon: GPUI forces `NSApplicationActivationPolicyRegular` on launch; overriding it with `setActivationPolicy(Accessory)` inside `Application::run` works.
- Hide on focus loss: GPUI has no hide-window API, so the window is closed with `Window::remove_window` from `observe_window_activation` and recreated on the next show.
- Window placement: `WindowBounds` are relative to the display passed in `display_id`, so the mouse position from `CGEvent` has to be converted to display-local coordinates.
- Pitfall: children of a scrolling flex column shrink unless marked `flex_none()`.
