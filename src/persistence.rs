//! Functions for saving and restoring the launcher's persistent state:
//! the home screen layout, recents list, user-installed apps, and window geometry.

use std::path::PathBuf;

use anyhow::Result;
use makepad_widgets::*;
use serde::{Deserialize, Serialize};

use crate::{app_data_dir, mini_apps::registry::LauncherLayout};

const LAYOUT_FILE_NAME: &str = "launcher_layout.json";
const WINDOW_GEOM_STATE_FILE_NAME: &str = "window_geom_state.json";

fn layout_path() -> PathBuf {
    app_data_dir().join(LAYOUT_FILE_NAME)
}

/// Saves the launcher layout (home pages, recents, user apps) to persistent storage.
pub fn save_launcher_layout(layout: &LauncherLayout) -> Result<()> {
    std::fs::create_dir_all(app_data_dir())?;
    let json = serde_json::to_vec_pretty(layout)?;
    std::fs::write(layout_path(), json)?;
    Ok(())
}

/// Loads the launcher layout from persistent storage.
///
/// Returns `Ok(None)` if no saved layout exists yet (first run).
/// A corrupt file is backed up and treated as a first run rather than an error.
pub fn load_launcher_layout() -> Result<Option<LauncherLayout>> {
    let path = layout_path();
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    match serde_json::from_slice(&bytes) {
        Ok(layout) => Ok(Some(layout)),
        Err(e) => {
            error!("Failed to deserialize launcher layout, backing it up: {e}");
            let _ = std::fs::rename(&path, path.with_extension("json.bak"));
            Ok(None)
        }
    }
}

/// Persistable state of the window's size, position, and fullscreen status.
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowGeomState {
    /// A tuple containing the window's width and height.
    pub inner_size: (f64, f64),
    /// A tuple containing the window's x and y position.
    pub position: (f64, f64),
    /// Maximise fullscreen if true.
    pub is_fullscreen: bool,
}

/// Save the current state of the given window's geometry to persistent storage.
pub fn save_window_state(window_ref: WindowRef, cx: &Cx) -> Result<()> {
    let inner_size = window_ref.get_inner_size(cx);
    let position = window_ref.get_position(cx);
    let window_geom = WindowGeomState {
        inner_size: (inner_size.x, inner_size.y),
        position: (position.x, position.y),
        is_fullscreen: window_ref.is_fullscreen(cx),
    };
    std::fs::create_dir_all(app_data_dir())?;
    std::fs::write(
        app_data_dir().join(WINDOW_GEOM_STATE_FILE_NAME),
        serde_json::to_string(&window_geom)?,
    )?;
    Ok(())
}

/// Loads the window geometry's state from persistent storage.
pub fn load_window_state(window_ref: WindowRef, cx: &mut Cx) -> Result<()> {
    let file = match std::fs::File::open(app_data_dir().join(WINDOW_GEOM_STATE_FILE_NAME)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.into()),
    };
    let window_geom: WindowGeomState =
        serde_json::from_reader(file).map_err(|e| anyhow::anyhow!(e))?;
    let WindowGeomState {
        inner_size,
        position,
        is_fullscreen,
    } = window_geom;
    window_ref.configure_window(
        cx,
        dvec2(inner_size.0, inner_size.1),
        dvec2(position.0, position.1),
        is_fullscreen,
        "Host Launcher".to_string(),
    );
    Ok(())
}
