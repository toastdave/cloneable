# AGENTS.md
# Guidance for agentic coding in this repo.
# Keep instructions practical, minimal, and repo-specific.

## Project Snapshot
- Stack: Vite + React + TypeScript frontend, Tauri (Rust) backend.
- Package manager: not enforced, but Tauri config uses `bun` in build hooks.
- Source roots: `src/` (frontend), `src-tauri/` (Rust backend).

## Build / Lint / Test Commands
### Frontend (Vite + React)
- Install deps: `bun install` (or `npm install` if you choose npm).
- Dev server: `bun run dev` (same as `vite`).
- Build: `bun run build` (runs `tsc` then `vite build`).
- Preview: `bun run preview`.

### Tauri (Rust)
- Dev app: `bun run tauri dev` (runs Vite dev and Tauri shell).
- Build app: `bun run tauri build`.

### Linting
- No ESLint/Prettier scripts found in `package.json`.
- Type checking is enforced via `tsc` in `bun run build`.

### Tests
- No JS test runner configured in `package.json`.
- No Rust test runner configured beyond default Cargo support.
- If you add tests later, document a single-test command here.

### Running a Single Test
- Not currently available (no test framework configured).
- For future setup, include examples like:
  - JS: `bun run test -- <pattern>` (if Vitest/Jest added)
  - Rust: `cargo test <test_name>` (from `src-tauri/`)

## Code Style Guidelines
These reflect the current codebase conventions. Keep changes consistent.

### TypeScript / React
- Use functional components with hooks; no class components are present.
- Prefer named functions for handlers (e.g., `async function greet()`),
  or inline lambdas for short handlers.
- Imports use double quotes and semicolons; keep that formatting.
- Import order (seen in `src/App.tsx`):
  1) React/TS imports, 2) local assets, 3) third-party libs, 4) CSS.
- Keep a blank line between import groups.
- JSX formatting uses 2-space indentation and trailing commas.
- Prefer `e.currentTarget` over `e.target` for typed events.
- Use `useState` for local component state; keep state names descriptive
  (`greetMsg`, `setGreetMsg`, `name`, `setName`).
- Keep components default-exported from their file when they are the main
  component (e.g., `export default App`).

### TypeScript Configuration Expectations
- Strict mode enabled in `tsconfig.json`:
  - `strict`, `noUnusedLocals`, `noUnusedParameters`.
- Avoid unused variables; clean up dead code and props.
- Prefer explicit typing only when inference is unclear or public API
  boundaries require it.

### CSS / Assets
- CSS is imported via side-effect (`import "./App.css";`).
- Static assets are imported as modules (e.g., `reactLogo`).
- Keep asset references consistent: use Vite public assets for root paths
  and local imports for module assets.

### Error Handling
- Frontend: use async/await; handle errors at boundaries when adding
  new async calls (e.g., `try/catch` around `invoke` if user-facing).
- Backend: panic-style failure uses `.expect("...")` for fatal startup
  errors in `src-tauri/src/lib.rs`.
- Avoid swallowing errors silently; surface actionable messages.

## Rust (Tauri) Guidelines
- Use Rust 2021 edition; keep `src-tauri/src/lib.rs` as the primary
  entry point and `src-tauri/src/main.rs` minimal.
- Tauri commands are annotated with `#[tauri::command]` and
  registered via `tauri::generate_handler![...]`.
- Function naming: snake_case (`greet`).
- Keep `run()` responsible for Tauri builder setup.
- Prefer `tauri::Builder::default()` chaining for plugins and handlers.
- Formatting: follow `rustfmt` defaults (4-space indentation).

## Imports, Formatting, and Naming
- Use double quotes in TS/TSX imports and strings.
- Use semicolons consistently in TS/TSX.
- Keep line length reasonable; break JSX props onto new lines.
- Use descriptive names: avoid `data`, `temp`, `foo` in final code.
- Boolean vars should read as predicates (`isReady`, `hasError`).

## File Organization
- Frontend entry: `src/main.tsx` renders `src/App.tsx`.
- Tauri entry: `src-tauri/src/main.rs` calls `cloneable_lib::run()`.
- Tauri config: `src-tauri/tauri.conf.json` defines build hooks.

## Build Integration Notes
- Tauri config uses:
  - `beforeDevCommand`: `bun run dev`
  - `beforeBuildCommand`: `bun run build`
- If you change the package manager, update `src-tauri/tauri.conf.json`.

## Rule Files
- No Cursor rules found in `.cursor/rules/` or `.cursorrules`.
- No Copilot instructions found in `.github/copilot-instructions.md`.

## Updating This File
- Keep it around ~150 lines.
- Add new commands when tooling is introduced.
- If you add a formatter or linter, document exact CLI invocations.
