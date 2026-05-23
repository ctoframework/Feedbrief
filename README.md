# Feedbrief

Feedbrief is a native desktop app that turns RSS feeds into a daily briefing you can actually read. It fetches stories in parallel, scores them with a local Ollama model, summarizes the important ones, and saves each day's result so you can revisit it later.

## What It Does

- Pulls from a configurable set of RSS feeds
- Scores and tags stories with a local LLM
- Generates a single executive-style briefing from the top items
- Keeps a local history of briefs by date
- Lets you switch between personas with different feed sets and briefing prompts
- Opens the original article when you want the source material
- Runs entirely on your machine, with no webview or hosted backend

## Key Features

- **Daily briefing view** - see today's synthesized summary at a glance
- **Live pipeline feedback** - watch feeds load, articles score, and summaries complete in real time
- **Topic filtering** - narrow the finished briefing by topic tags
- **History navigation** - move through previous days or jump from saved history pills
- **Persona management** - create and edit persona profiles from inside the app
- **Local persistence** - store briefs in SQLite so they are available across launches
- **Offline-friendly workflow** - the app checks whether Ollama is available and uses only local inference

## How It Works

1. RSS feeds are fetched in parallel.
2. Articles are scored and tagged by the selected Ollama model.
3. The strongest stories are summarized into a concise briefing.
4. The final brief is saved locally and can be reopened later.

## Prerequisites

1. **Rust 1.75+** - install from <https://rustup.rs>
2. **Ollama** - install from <https://ollama.com>, then pull at least one supported model:
   ```bash
   ollama pull llama3.1:8b
   ```
3. **Platform graphics dependencies**:
   - macOS: nothing extra needed (Metal is built in)
   - Windows: nothing extra needed (DirectX is built in)
   - Linux: `sudo apt install libxkbcommon-x11-0 libgtk-3-dev libwayland-dev libxkbcommon-dev`

## Build And Run

```bash
# Run in development mode
cargo run

# Build a release binary
cargo build --release
./target/release/feedbrief        # macOS/Linux
.\target\release\feedbrief.exe    # Windows
```

The first build takes longer because `eframe`, `winit`, `wgpu`, and the platform graphics stack need to compile. After that, incremental builds are much faster.

## Data Storage

Feedbrief stores briefs locally in SQLite.

- **macOS**: `~/Library/Application Support/com.feedbrief.Feedbrief/briefs.db`
- **Linux**: `~/.local/share/Feedbrief/briefs.db`
- **Windows**: `%APPDATA%\feedbrief\Feedbrief\data\briefs.db`

You can open the database with any SQLite browser if you want to inspect past briefs or export them.

## Customization

- **Add or remove feeds**: edit `src/feeds.rs`
- **Adjust scoring or briefing prompts**: edit `src/llm.rs`
- **Change the UI palette**: edit the color constants near the top of `src/app.rs`
- **Tune the default window size**: edit `src/main.rs`
- **Manage personas**: use the in-app persona editor

## Technical Notes

Feedbrief is a fully native Rust desktop app. The main pieces are:

- `egui` + `eframe` for the UI
- `tokio` for async orchestration
- `reqwest` and `feed-rs` for RSS fetching
- `ollama` over HTTP for local scoring and summarization
- `rusqlite` for persistence

There is no browser shell involved; the app renders directly through the platform graphics stack.

## File Layout

```
feedbrief/
├── Cargo.toml
├── README.md
├── assets/                       ← optional .ttf font files
└── src/
    ├── main.rs                   ← entry point, opens window
    ├── app.rs                    ← egui UI and views
    ├── feeds.rs                  ← feed source list and personas
    ├── fetcher.rs                ← parallel RSS fetch
    ├── llm.rs                    ← Ollama scoring and summarization
    ├── progress.rs               ← progress event types
    ├── pipeline.rs               ← orchestration
    └── storage.rs                ← SQLite persistence and day navigation
```

## Future Enhancements

- Source-level weight overrides
- Embedding-based dedupe
- Export a day brief as markdown or PDF
- Background scheduled fetches
- Bookmark or star articles across days
