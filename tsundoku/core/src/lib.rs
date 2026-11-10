//! Tsundoku's logic, with no Tauri dependency: downloads, the history, the tools and settings live here, with their
//! tests. The app crate (`src-tauri`) only exposes them to the UI as commands and events.
//!
//! Types the UI reads derive `ts_rs::TS` with `#[ts(export)]`: `cargo test` writes them to the UI's `src/bindings`.

pub mod app_info;
pub mod atomic_file;
pub mod backup;
pub mod browsers;
pub mod bug_report;
pub mod cancel;
pub mod changelog;
#[cfg(windows)]
pub mod clipboard;
pub mod data_folder;
pub mod database;
pub mod downloads;
pub mod first_run;
pub mod gallery;
pub mod history;
pub mod hosts;
pub mod listing;
pub mod metadata;
pub mod pending;
pub mod post_names;
pub mod presets;
pub mod preview;
pub mod process;
pub mod relocate;
pub mod room;
pub mod rules;
pub mod settings;
pub mod site_limits;
pub mod sites;
pub mod supported_sites;
mod sync;
pub mod system;
pub mod thumbnails;
pub mod tools;
pub mod watched;
