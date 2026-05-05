# TechBrief — native edition

Pure native cross-platform desktop app. No webview, no HTML, no JavaScript. Pulls ~28 RSS feeds in parallel, scores them with a local Ollama model, summarizes the top stories, and produces a daily executive briefing. Saves each day to a local SQLite database so you can flip back through history.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  TechBrief (single ~15 MB native binary)                │
│                                                         │
│  ┌────────────────┐         ┌─────────────────────────┐ │
│  │  egui + eframe │         │  Rust async pipeline    │ │
│  │  (GPU-rendered │◄───────►│  - tokio runtime        │ │
│  │   native UI)   │         │  - reqwest + feed-rs    │ │
│  └────────────────┘         │  - ollama HTTP client   │ │
│         │                   │  - rusqlite storage     │ │
│         │                   └────────────┬────────────┘ │
└─────────┼────────────────────────────────┼──────────────┘
          │                                │
          ▼                                ▼
   ┌────────────┐    ┌──────────────┐    ┌──────────────┐
   │ winit/wgpu │    │ ~28 RSS feeds │    │ Ollama local │
   │ Vulkan/    │    │ (arXiv, DM,   │    │ (llama3.1,   │
   │ Metal/DX   │    │  TC, HN, ...) │    │  qwen2.5...) │
   └────────────┘    └──────────────┘    └──────────────┘
```

**No webview involved.** eframe draws every pixel through your platform's native graphics API (Metal on macOS, DirectX/Vulkan on Windows, Vulkan/OpenGL on Linux). Same APIs games use.

## What's new in this version

1. **Pure native UI** — egui + eframe. No HTML, no JS, no Tauri.
2. **Granular live progress** — see each RSS feed completing in real time, each scoring batch ticking through, each article summary in flight. A scrolling LIVE LOG below the progress bar shows the last 15 events.
3. **Date history** — every fetch is saved to SQLite (one brief per day, overwrites if re-fetched). Navigate with `← previous` / `next →` buttons or tap any history pill on the idle screen to jump to a specific day.
4. **Auto-restore** — on launch, if today already has a brief saved, it shows immediately. No fetch needed.

## Prerequisites

1. **Rust 1.75+** — install from <https://rustup.rs>
2. **Ollama** — install from <https://ollama.com>, then:
   ```
   ollama pull llama3.1:8b
   ```
3. **Platform graphics deps**:
   - macOS: nothing extra needed (Metal is built-in)
   - Windows: nothing extra needed (DirectX is built-in)
   - Linux: `sudo apt install libxkbcommon-x11-0 libgtk-3-dev libwayland-dev libxkbcommon-dev`

## Build & run

```bash
cd tech-brief-egui

# (Optional) Download fonts for the editorial look
# See assets/README.md — drop 4 .ttf files into assets/

# Run in dev (debug build, slow startup, fast compile)
cargo run

# Build release (~5min first time, single optimized binary)
cargo build --release
./target/release/tech-brief        # macOS/Linux
.\target\release\tech-brief.exe    # Windows
```

The first build takes a while because eframe pulls in `winit`, `wgpu`, and the platform graphics stack. After that, incremental builds are seconds.

## Where data is stored

- **macOS**: `~/Library/Application Support/com.techbrief.TechBrief/briefs.db`
- **Linux**: `~/.local/share/TechBrief/briefs.db`
- **Windows**: `%APPDATA%\techbrief\TechBrief\data\briefs.db`

Plain SQLite — open it with any SQLite browser if you want to grep through old briefs or export to markdown.

## File layout

```
tech-brief-egui/
├── Cargo.toml
├── README.md
├── assets/                       ← optional .ttf font files
└── src/
    ├── main.rs                   ← entry point, opens window
    ├── app.rs                    ← egui UI (idle/loading/results views)
    ├── feeds.rs                  ← feed source list
    ├── fetcher.rs                ← parallel RSS fetch with per-feed progress
    ├── llm.rs                    ← Ollama scoring/summarization
    ├── progress.rs               ← progress event types
    ├── pipeline.rs               ← orchestration
    └── storage.rs                ← SQLite persistence + day navigation
```

## Customization

- **Add/remove feeds**: edit `src/feeds.rs`
- **Change LLM persona / scoring**: edit prompts in `src/llm.rs`
- **Tweak colors**: top of `src/app.rs` — all colors are `const Color32`
- **Window size**: edit `main.rs` (`with_inner_size`)

## Why egui instead of Iced/Slint/Dioxus

- **egui**: shipped today with great GPU rendering, immediate-mode is easy to reason about, dashboard/tool aesthetic looks great on it.
- **Iced**: better for typography-heavy UIs, but ~3x more code for the same result.
- **Slint**: requires learning the .slint DSL and dual GPL/commercial licensing.
- **Dioxus**: pure-native renderer is still experimental; default is webview.

For a single-window information dashboard with progress reporting and history, egui is the pragmatic choice.

## Future enhancements

- Source-level weight overrides ("always show me SemiAnalysis even if scored low")
- Embedding-based dedupe (currently URL-only)
- Export day's brief as markdown or PDF
- Background scheduled fetches (e.g. every morning at 7am)
- Bookmark/star articles across days
