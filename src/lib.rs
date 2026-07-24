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
/// Functions for loading and saving persistent launcher state.
pub mod persistence;
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

pub fn app_data_dir() -> &'static Path {
    project_dir().data_dir()
}
