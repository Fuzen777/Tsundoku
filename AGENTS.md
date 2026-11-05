---
type: Agent Instructions
title: AGENTS
---

## Introduction

- Tsundoku is a Windows desktop app (Tauri 2: a React + TypeScript UI over a Rust core) that downloads videos with yt-dlp and keeps a history of everything it has downloaded, so it can warn before a duplicate.
- [PROJECT.md](./PROJECT.md) holds the original spec and its decisions, [TODO.md](./TODO.md) the features planned after v1.

## Rules

- All UI text lives in `tsundoku/src/strings.ts`, in English. Never write UI text in a component.
- UI labels use Title Case: buttons, toggles, navigation, headings, dialog titles and buttons, placeholders, column headers and short names. Minor words (a, an, the, and, or, of, to, in, on, for, with, as, at, by, from) stay lowercase unless first or last, and hyphenated words take both capitals. Sentences (descriptions, messages, empty-state explanations, status texts) stay in sentence case. See [DESIGN](./DESIGN.md).
- `tsundoku-core` (`tsundoku/core`) has no Tauri dependency. Downloads, the history, the tools, presets and settings go there, with unit tests; the app crate (`tsundoku/src-tauri`) only exposes them as commands and events, and the UI only presents.
- Types the UI reads are Rust types in the core deriving `ts_rs::TS` with `#[ts(export)]`; `cargo test` writes them to `tsundoku/src/bindings`. Never hand-write a TypeScript type that mirrors a Rust one, and commit the regenerated files with the change.
- `tsundoku/src/api.ts` is the only place that calls `invoke` or another Tauri API.
- Before a visual change, read [DESIGN](./DESIGN.md). If something differs notably from it, tell the user.
- Never suppress, ignore or work around an ESLint, Clippy or TypeScript error or warning; fix the cause.
- Meta files (docs, scripts) go at the project root; `tsundoku/` holds code only.
- After implementing a feature, update [FEATURES](./FEATURES.md) and remove the item from [TODO](./TODO.md) if it is listed.
- Every change bumps the patch version and gets an entry in [CHANGELOG](./CHANGELOG.md); a commit that bumps the version is tagged `vX.Y.Z`. A release bumps the minor or major version and folds in the patch entries since the previous release.

## Layout

| Path | Contents |
| --- | --- |
| `tsundoku/src` | The React UI: `views/`, `components/`, `strings.ts`, `api.ts`, `queries.ts` (React Query hooks), `bindings/` (generated) |
| `tsundoku/core` | `tsundoku-core`: the UI-free Rust logic and its tests |
| `tsundoku/src-tauri` | The Tauri app: window, plugins, commands, `tauri.conf.json`, `capabilities/` |
| `Scripts` | Dev, check and build scripts |

## Building and Testing

| Script | What it does |
| --- | --- |
| `Scripts\Dev.cmd` | Starts the app with `tauri dev`: the UI reloads live, and Rust changes rebuild and restart the app |
| `Scripts\Check.cmd` | `cargo fmt --check`, Clippy with warnings as errors, `cargo test` (which also regenerates `src/bindings`), then `tsc`, ESLint with no warnings allowed, and Vitest. Run it before every commit. |
| `Scripts\Build.cmd` | Builds the release app and its NSIS installer |

- First setup: `npm install` in `tsundoku/`. Needs Rust (MSVC toolchain), Node and Visual Studio's C++ build tools.
- **This PC:** the git-ignored `.cargo/config.toml` at the project root sends Rust's build output to `D:\Dev\tsundoku\target`, since Google Drive would sync it and lock files mid-build, and sets the dev data folder (below). Cargo reads it from any folder below the project root. Without it, Cargo builds into `tsundoku/target`. The installer lands in `<build folder>\release\bundle\nsis`.
- `tsundoku/.cargo/config.toml` (committed) tells ts-rs to write to `src/bindings`.
- The version lives in `tsundoku/package.json`, which `tauri.conf.json` reads, and in `[workspace.package]` of `tsundoku/Cargo.toml`. Bump both together.
- The UI alone runs in a browser with `npm run dev` in `tsundoku/` (port 1420), which helps check layouts: outside Tauri, `api.ts` returns stand-ins. In this workspace, the Coding root's `.claude/launch.json` has it as `tsundoku-ui`.
- The project is its own git repository (`github.com/Fuzen777/Tsundoku`, private until it's ready to share); the Coding monorepo ignores this folder. Commits use the GitHub noreply address set in this repository's git config. Commit subjects follow `Title | description`. `README.md`, `CHANGELOG.md` and the other files GitHub shows stay plain GFM, without Obsidian admonitions.

## Data and Testing

- The data folder (the app stores nothing in it until settings and tools land): dev builds use their own, `TSUNDOKU_DATA_DIR` when set (this PC's `.cargo/config.toml` sets `D:\Dev\tsundoku\data`), otherwise `%LocalAppData%\Tsundoku Dev`. Release builds use `%LocalAppData%\Tsundoku` and ignore the variable.
- `D:\Dev\tsundoku\` is the disposable sandbox: `data\` (the dev data folder), `downloads\` (test downloads) and `target\`. Anything under it can be changed or deleted freely for a test. Never test against an installed copy's `%LocalAppData%\Tsundoku`.

## Technical Notes

- The main window starts hidden and transparent (`tauri.conf.json`). `src-tauri/src/lib.rs` applies Mica, which needs Windows 11, and records whether it worked. `main.tsx` reads that through the `app_info` command before the first render and sets `data-backdrop` on `<html>`: `mica` makes the page transparent, `solid` paints `--color-bg`. `App` shows the window after its first frame, and Rust shows it anyway after 3 seconds if the UI never does.
- `single-instance` is the first plugin registered, as it requires: a second launch focuses the running window. `window-state` restores size, position and maximized state, never visibility.
- Every Tauri permission the UI uses is listed in `src-tauri/capabilities/default.json`; `core:window:allow-show` is there for the hidden start.
- The CSP in `tauri.conf.json` allows `https:` images, for video thumbnails.
