# Cloneable Workflow Recorder

Desktop workflow recorder built with Tauri (Rust) + Vite React (TypeScript). Records global mouse/keyboard events and captures three screenshots per event (full, window fallback, click crop). Saves sessions locally as JSON + PNGs.

## Setup

Prereqs:
- Rust toolchain (stable)
- Bun (recommended)

Install deps:
```bash
bun install
```

Run desktop app (dev):
```bash
bun run tauri dev
```

Build desktop app:
```bash
bun run tauri build
```

Frontend only:
```bash
bun run dev
```

Typecheck + build:
```bash
bun run build
```

## What shipped vs cut

Shipped:
- Start/Stop recording UI with state feedback
- Tauri command bridge for start/stop
- Global mouse + keyboard capture (rdev)
- 3 images per event: full-screen, window image (fallback), click crop
- Local persistence: `recordings/<session>/recording.json` + `recordings/<session>/shots/*.png`

Cut / deferred:
- Loading recordings for review UI
- Step parsing UI model + annotation UI
- Action type classification UI
- Text input grouping and global hotkeys

## Tradeoffs and known limitations

- Window bounds are not yet detected; window image is currently a second full-screen capture and is flagged as `windowCropFallback: true`.
- Screenshots use primary display only; multi-monitor mapping is best-effort.
- rdev 0.5.3 does not expose printable text; keyboard events are stored as key debug strings.
- JSON storage favors simplicity; a future SQLite migration remains feasible.
- Global input capture may require OS permissions (macOS accessibility, Windows security prompts).

## Recording data format

Each session writes to:
```
recordings/<session>/recording.json
recordings/<session>/shots/
```

Events store timestamp, type, coordinates (mouse), key data (keyboard), and paths for the three screenshots. Errors are recorded per image if capture fails.

## Walkthrough (written)

1) Run `bun run tauri dev`.
2) Click Start Recording.
3) Perform clicks/typing in any app.
4) Click Stop Recording.
5) Inspect `recordings/<session>/recording.json` and the `shots/` folder to confirm captured events and images.

## AI usage notes

- Used an LLM to draft Rust Tauri command scaffolding and iterate on capture/persistence logic.
- All code was reviewed and adapted to project conventions; no generated code was committed without verification.

## Recommended IDE setup

- VS Code + Tauri + rust-analyzer
  - https://code.visualstudio.com/
  - https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode
  - https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer
