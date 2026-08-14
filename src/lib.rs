#![recursion_limit = "256"]

use std::{path::Path, sync::OnceLock};

use robius_directories::ProjectDirs;

pub use makepad_widgets;

/// The top-level main application module.
pub mod app;
/// AI app generation: the create-bar's ACP agent client + pipeline.
pub mod generate;
/// The home screen: paged app grid, drawer, context menu, and edit chrome.
pub mod launcher;
/// Mini-app registry, manifests, and the fullscreen mini-app host.
pub mod mini_apps;
/// The mini-app permission model and grant store.
pub mod permissions;
/// Functions for loading and saving persistent launcher state.
pub mod persistence;
/// Host-service brokers behind the permission system (docs/PERMISSIONS.md).
pub mod services;
/// Shared UI components and styling.
pub mod shared;

pub const APP_QUALIFIER: &str = "rs";
pub const APP_ORGANIZATION: &str = "robius";
pub const APP_NAME: &str = "host_launcher";

pub fn project_dir() -> &'static ProjectDirs {
    static PROJECT_DIRS: OnceLock<ProjectDirs> = OnceLock::new();

    PROJECT_DIRS.get_or_init(|| {
        ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
            .expect("Failed to obtain host_launcher project directory")
    })
}

/// Whether this run should use throwaway state instead of the real profile:
/// explicitly requested (`HOST_LAUNCHER_FRESH=1`), OR running under the
/// headless test harness (`MAKEPAD=headless`). The latter is a safety net —
/// a bare `cargo test --test ui` that forgot the FRESH var must still never
/// write to (or delete from) a developer's real data dir.
pub fn is_fresh_env() -> bool {
    std::env::var("HOST_LAUNCHER_FRESH").is_ok_and(|v| v == "1")
        || std::env::var("MAKEPAD").is_ok_and(|v| v == "headless")
}

pub fn app_data_dir() -> &'static Path {
    // HOST_LAUNCHER_FRESH=1 means "pretend first run" — and that must include
    // WRITES: a fresh instance (the UI test harness runs dozens) may not
    // pollute the real user data dir with its layouts, installed test apps,
    // or app-storage files. Redirect the whole data root to a per-process
    // temp dir instead.
    static FRESH_DIR: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();
    if let Some(dir) = FRESH_DIR.get_or_init(|| {
        is_fresh_env()
            .then(|| {
                let dir = std::env::temp_dir()
                    .join(format!("host_launcher_fresh_{}", std::process::id()));
                // Pids get recycled and TMPDIR isn't cleaned between runs, so a
                // leftover dir from a dead process with this pid would leak its
                // layout/apps/app_data into this "pretend first run". Clear it
                // first. Safe: OnceLock runs this once, and no LIVE process can
                // share our pid-keyed path.
                let _ = std::fs::remove_dir_all(&dir);
                let _ = std::fs::create_dir_all(&dir);
                dir
            })
    }) {
        return dir;
    }
    project_dir().data_dir()
}

/// The private storage directory for one mini-app — the root of its jailed
/// Splash `fs` module, like an Android app's internal storage. Created on
/// demand; deleted when the app is uninstalled.
pub fn app_sandbox_dir(app_id: &str) -> std::path::PathBuf {
    // The id becomes a directory name (and a jail root). Reject anything with
    // path structure so it can't escape app_data/ — falls back to a fixed
    // bucket rather than ever resolving above the data dir. Ids are trusted
    // today (kebab-generated); this is belt-and-braces.
    let safe = !app_id.is_empty()
        && app_id != "."
        && app_id != ".."
        && !app_id.contains(['/', '\\', '\0']);
    let name = if safe { app_id } else { "_unsafe" };
    let dir = app_data_dir().join("app_data").join(name);
    let _ = std::fs::create_dir_all(&dir);
    dir
}
