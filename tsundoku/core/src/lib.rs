//! Tsundoku's logic, with no Tauri dependency: downloads, the history, the tools and settings live here, with their
//! tests. The app crate (`src-tauri`) only exposes them to the UI as commands and events.
//!
//! Types the UI reads derive `ts_rs::TS` with `#[ts(export)]`: `cargo test` writes them to the UI's `src/bindings`.

pub mod app_info;
