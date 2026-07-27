//! Saving and restoring the launcher's persistent state, modularly — one
//! massive JSON was fragile (a single corrupt byte lost everything, and every
//! app's source code rode along with every icon drag). The layout now is:
//!
//! ```text
//! <data_dir>/
//!   layout.json               placements, grid, dock, recents, tombstones
//!   apps/<id>/manifest.json   one user/generated app's metadata
//!   apps/<id>/app.splash      ...its source code, a real editable file
//!   apps/<id>/widget.splash   ...its widget's source, if it has one
//!   apps/<id>/versions/       ...timestamped snapshots to revert to
//!   app_data/<id>/            ...its private storage (the Splash fs jail)
//!   window_geom_state.json    window geometry
//! ```
//!
//! Built-in apps live in the binary/repo (`apps/*.splash`), never here.
//! A legacy single-file `launcher_layout.json` is migrated on first load.

use std::path::PathBuf;

use anyhow::Result;
use makepad_widgets::*;
use serde::{Deserialize, Serialize};

use crate::{
    app_data_dir,
    mini_apps::{
        registry::{LauncherLayout, MiniAppManifest, WidgetManifest},
        versions::{AppVersion, MAX_VERSIONS},
    },
};

const LAYOUT_FILE_NAME: &str = "layout.json";
const LEGACY_LAYOUT_FILE_NAME: &str = "launcher_layout.json";
const WINDOW_GEOM_STATE_FILE_NAME: &str = "window_geom_state.json";

fn layout_path() -> PathBuf {
    app_data_dir().join(LAYOUT_FILE_NAME)
}

fn apps_dir() -> PathBuf {
    app_data_dir().join("apps")
}

/// Whether an app id is safe to use as a single directory name. Generated ids
/// are kebab-case, but a legacy/imported layout could carry anything — reject
/// path separators, `..`, absolute markers, and NUL so an id can never point a
/// write outside `apps/`/`app_data/`. Defense in depth; the id sources are all
/// trusted today.
fn is_safe_app_id(id: &str) -> bool {
    !id.is_empty()
        && id != "."
        && id != ".."
        && !id.contains(['/', '\\', '\0'])
        && !std::path::Path::new(id)
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
}

fn app_dir(id: &str) -> PathBuf {
    apps_dir().join(id)
}

/// Writes `bytes` to `path` atomically: a sibling temp file renamed over the
/// target (same-dir rename is atomic on the platforms we target). A crash
/// mid-write leaves the OLD file intact rather than a truncated one — the
/// whole point of splitting the store into per-file pieces.
fn atomic_write(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// The on-disk manifest: everything about an app EXCEPT its id (the directory
/// name) and its sources (their own files beside it).
#[derive(Serialize, Deserialize)]
struct AppManifestFile {
    name: String,
    icon: String,
    tint: u32,
    #[serde(default)]
    allow_net: bool,
    /// True for a user-modified copy of a BUILT-IN app: the override shadows
    /// the stock manifest at load, and keeping the flag means it stays
    /// non-uninstallable (you revert it via version history instead).
    #[serde(default)]
    builtin: bool,
    #[serde(default)]
    shortcuts: Vec<String>,
    /// Spans for the widget whose source is `widget.splash`, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    widget: Option<WidgetSpans>,
}

#[derive(Serialize, Deserialize)]
struct WidgetSpans {
    default_span: (u8, u8),
    min_span: (u8, u8),
}

/// Writes one user app's directory: manifest + source file(s). Called on
/// install/refine — NOT on every layout save, so dragging an icon never
/// rewrites app code.
pub fn save_user_app(manifest: &MiniAppManifest) -> Result<()> {
    if !is_safe_app_id(&manifest.id) {
        anyhow::bail!("refusing to persist app with unsafe id '{}'", manifest.id);
    }
    let dir = app_dir(&manifest.id);
    std::fs::create_dir_all(&dir)?;
    let file = AppManifestFile {
        name: manifest.name.clone(),
        icon: manifest.icon.clone(),
        tint: manifest.tint,
        allow_net: manifest.allow_net,
        builtin: manifest.builtin,
        shortcuts: manifest.shortcuts.clone(),
        widget: manifest.widget.as_ref().map(|w| WidgetSpans {
            default_span: w.default_span,
            min_span: w.min_span,
        }),
    };
    // Order matters for crash safety: write the SOURCES first, and
    // manifest.json (the file load keys off) LAST, each atomically. A crash
    // between them leaves either the old complete app or the new complete app
    // — never a manifest pointing at a half-written source.
    atomic_write(&dir.join("app.splash"), manifest.source.as_bytes())?;
    match &manifest.widget {
        Some(w) => atomic_write(&dir.join("widget.splash"), w.source.as_bytes())?,
        None => {
            let _ = std::fs::remove_file(dir.join("widget.splash"));
        }
    }
    atomic_write(&dir.join("manifest.json"), &serde_json::to_vec_pretty(&file)?)?;
    Ok(())
}

fn versions_dir(id: &str) -> PathBuf {
    app_dir(id).join("versions")
}

/// Snapshots an app's CURRENT state into its history, so a modification (or a
/// restore) can be undone. `note` records why — the request that superseded
/// it, or a marker like "Before restore". A stamp collision (two changes in
/// the same second) gets a `-2`, `-3`… suffix rather than silently clobbering
/// the earlier snapshot.
pub fn snapshot_version(manifest: &MiniAppManifest, mut version: AppVersion) -> Result<()> {
    if !is_safe_app_id(&manifest.id) {
        anyhow::bail!("refusing to snapshot app with unsafe id '{}'", manifest.id);
    }
    let dir = versions_dir(&manifest.id);
    std::fs::create_dir_all(&dir)?;

    let base = version.stamp.clone();
    let mut n = 2;
    while dir.join(format!("{}.json", version.stamp)).exists() {
        version.stamp = format!("{base}-{n}");
        n += 1;
        if n > 60 {
            anyhow::bail!("too many snapshots in the same second");
        }
    }

    // Source first, metadata (the file listing keys off) last.
    atomic_write(
        &dir.join(format!("{}.splash", version.stamp)),
        manifest.source.as_bytes(),
    )?;
    atomic_write(
        &dir.join(format!("{}.json", version.stamp)),
        &serde_json::to_vec_pretty(&version)?,
    )?;

    prune_versions(&manifest.id, MAX_VERSIONS);
    Ok(())
}

/// Every snapshot of an app, newest first.
pub fn list_versions(id: &str) -> Vec<AppVersion> {
    if !is_safe_app_id(id) {
        return Vec::new();
    }
    let Ok(read) = std::fs::read_dir(versions_dir(id)) else {
        return Vec::new();
    };
    let mut out: Vec<AppVersion> = read
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok())
        .filter_map(|bytes| serde_json::from_slice::<AppVersion>(&bytes).ok())
        // A metadata file whose source went missing is not restorable.
        .filter(|v| versions_dir(id).join(format!("{}.splash", v.stamp)).exists())
        .collect();
    // Newest first. Same-second snapshots carry a "-2", "-3"… suffix, which
    // sorts wrong as text ("-10" < "-2"), so compare that index numerically.
    out.sort_by(|a, b| {
        b.at_unix
            .cmp(&a.at_unix)
            .then(collision_index(&b.stamp).cmp(&collision_index(&a.stamp)))
    });
    out
}

/// The `-N` suffix a same-second snapshot carries (0 when there is none).
fn collision_index(stamp: &str) -> u32 {
    // Stamps look like `20260727-105412` or `20260727-105412-3`; only the
    // THIRD segment is a collision counter.
    stamp
        .splitn(3, '-')
        .nth(2)
        .and_then(|n| n.parse().ok())
        .unwrap_or(0)
}

/// Whether an app has any restorable history (drives the menu entry).
pub fn has_versions(id: &str) -> bool {
    !list_versions(id).is_empty()
}

/// The archived source of one version.
pub fn load_version_source(id: &str, stamp: &str) -> Option<String> {
    if !is_safe_app_id(id) || !is_safe_app_id(stamp) {
        return None;
    }
    std::fs::read_to_string(versions_dir(id).join(format!("{stamp}.splash"))).ok()
}

/// Keeps the newest `keep` snapshots, deleting older ones (both files).
fn prune_versions(id: &str, keep: usize) {
    let versions = list_versions(id);
    for old in versions.into_iter().skip(keep) {
        // The stamp comes from a file on disk; never let it steer a delete
        // outside the versions dir (same guard the read path uses).
        if !is_safe_app_id(&old.stamp) {
            continue;
        }
        let dir = versions_dir(id);
        let _ = std::fs::remove_file(dir.join(format!("{}.json", old.stamp)));
        let _ = std::fs::remove_file(dir.join(format!("{}.splash", old.stamp)));
    }
}

/// Bytes an app has stored in its private jail (`app_data/<id>/`), for the
/// App Info page's storage line.
pub fn app_data_bytes(id: &str) -> u64 {
    fn walk(dir: &std::path::Path) -> u64 {
        let Ok(read) = std::fs::read_dir(dir) else {
            return 0;
        };
        read.flatten()
            .map(|e| match e.metadata() {
                Ok(m) if m.is_dir() => walk(&e.path()),
                Ok(m) => m.len(),
                Err(_) => 0,
            })
            .sum()
    }
    if !is_safe_app_id(id) {
        return 0;
    }
    walk(&app_data_dir().join("app_data").join(id))
}

/// Empties an app's private storage but keeps the app installed — the
/// "Clear data" of a phone's app-info page.
pub fn clear_app_data(id: &str) {
    if !is_safe_app_id(id) {
        return;
    }
    let dir = app_data_dir().join("app_data").join(id);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
}

/// Removes an uninstalled app's code directory AND its private data — the OS
/// convention: uninstalling an app deletes its storage.
pub fn remove_user_app(id: &str) {
    if !is_safe_app_id(id) {
        return;
    }
    let _ = std::fs::remove_dir_all(app_dir(id));
    let _ = std::fs::remove_dir_all(app_data_dir().join("app_data").join(id));
}

/// Reads one app directory back into a manifest. Skips (with a log) rather
/// than fails: one broken app must not take the launcher down.
fn load_user_app(id: &str) -> Option<MiniAppManifest> {
    let dir = app_dir(id);
    let manifest_bytes = std::fs::read(dir.join("manifest.json")).ok()?;
    let file: AppManifestFile = match serde_json::from_slice(&manifest_bytes) {
        Ok(f) => f,
        Err(e) => {
            error!("Skipping app '{id}': bad manifest.json: {e}");
            return None;
        }
    };
    let source = match std::fs::read_to_string(dir.join("app.splash")) {
        Ok(s) => s,
        Err(e) => {
            error!("Skipping app '{id}': missing app.splash: {e}");
            return None;
        }
    };
    let widget = match file.widget {
        Some(spans) => match std::fs::read_to_string(dir.join("widget.splash")) {
            Ok(widget_source) => Some(WidgetManifest {
                source: widget_source,
                default_span: spans.default_span,
                min_span: spans.min_span,
            }),
            Err(_) => {
                error!("App '{id}': manifest promises a widget but widget.splash is missing");
                None
            }
        },
        None => None,
    };
    Some(MiniAppManifest {
        id: id.to_string(),
        name: file.name,
        icon: file.icon,
        tint: file.tint,
        source,
        allow_net: file.allow_net,
        builtin: file.builtin,
        widget,
        shortcuts: file.shortcuts,
    })
}

/// Every user app on disk, by scanning `apps/*/`. Public so init can recover
/// apps even when `layout.json` is missing/corrupt (the code outlives the
/// placement file).
pub fn load_user_apps() -> Vec<MiniAppManifest> {
    let mut apps = Vec::new();
    let Ok(read) = std::fs::read_dir(apps_dir()) else {
        return apps;
    };
    let mut ids: Vec<String> = read
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|id| is_safe_app_id(id))
        .collect();
    ids.sort();
    for id in ids {
        if let Some(app) = load_user_app(&id) {
            apps.push(app);
        }
    }
    apps
}

/// Saves the layout (placements, grid, dock, recents, tombstones). App
/// manifests/sources are NOT embedded — they live in `apps/<id>/`, written by
/// `save_user_app` at install time.
pub fn save_launcher_layout(layout: &LauncherLayout) -> Result<()> {
    std::fs::create_dir_all(app_data_dir())?;
    let mut slim = layout.clone();
    slim.user_apps = Vec::new();
    atomic_write(&layout_path(), &serde_json::to_vec_pretty(&slim)?)?;
    Ok(())
}

/// Loads the full launcher state: the slim layout plus every user app from
/// its own directory. Returns `Ok(None)` on a true first run. A corrupt
/// layout file is backed up and treated as a first run; a single corrupt APP
/// is skipped without harming the rest — the point of the modular format.
pub fn load_launcher_layout() -> Result<Option<LauncherLayout>> {
    migrate_legacy_if_needed();

    let path = layout_path();
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut layout: LauncherLayout = match serde_json::from_slice(&bytes) {
        Ok(layout) => layout,
        Err(e) => {
            error!("Failed to deserialize launcher layout, backing it up: {e}");
            let _ = std::fs::rename(&path, path.with_extension("json.bak"));
            return Ok(None);
        }
    };
    layout.user_apps = load_user_apps();
    Ok(Some(layout))
}

/// One-time migration from the legacy single-file format: split the embedded
/// user apps into `apps/<id>/` directories, write the slim layout, and park
/// the old file as `.migrated.bak`. Runs only when the new layout.json
/// doesn't exist yet, so it can never clobber post-migration state.
fn migrate_legacy_if_needed() {
    let legacy = app_data_dir().join(LEGACY_LAYOUT_FILE_NAME);
    if !legacy.exists() {
        return;
    }
    // Both files present = a downgrade/upgrade cycle regenerated the legacy
    // file. The modular state is authoritative (it may hold NEWER data);
    // never remigrate over it — just note it so the stray file isn't a mystery.
    if layout_path().exists() {
        error!(
            "Both {LAYOUT_FILE_NAME} and legacy {LEGACY_LAYOUT_FILE_NAME} exist \
             (a downgrade/upgrade cycle?); using {LAYOUT_FILE_NAME}. Delete the \
             legacy file to silence this."
        );
        return;
    }
    let Ok(bytes) = std::fs::read(&legacy) else {
        return;
    };
    let Ok(layout) = serde_json::from_slice::<LauncherLayout>(&bytes) else {
        error!("Legacy layout unreadable; leaving it in place");
        return;
    };
    log!(
        "Migrating legacy launcher_layout.json ({} user apps) to the modular format",
        layout.user_apps.len()
    );
    // Abort the whole migration on ANY per-app write failure, BEFORE writing
    // layout.json or renaming the legacy file — so a partial migration retries
    // cleanly next boot instead of stranding apps behind a renamed source.
    let mut all_ok = true;
    for app in &layout.user_apps {
        if let Err(e) = save_user_app(app) {
            error!("Migration: couldn't write app '{}': {e}", app.id);
            all_ok = false;
        }
    }
    if !all_ok {
        error!("Migration incomplete; leaving launcher_layout.json in place to retry next boot");
        return;
    }
    if let Err(e) = save_launcher_layout(&layout) {
        error!("Migration: couldn't write layout.json: {e}");
        return;
    }
    let _ = std::fs::rename(&legacy, legacy.with_extension("json.migrated.bak"));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The legacy single-file format splits cleanly into the modular one:
    /// apps land in their own dirs with real .splash files, the slim layout
    /// drops the embedded sources, and the old file is parked as a backup.
    /// (Uses the FRESH temp data dir — set by the test harness env — so this
    /// never touches a real profile; see app_data_dir().)
    #[test]
    fn legacy_layout_migrates_to_modular_format() {
        // SAFETY: single-threaded at this point in the test; the var only
        // flips app_data_dir() to its per-process temp root.
        unsafe { std::env::set_var("HOST_LAUNCHER_FRESH", "1") };
        let dir = app_data_dir().to_path_buf();
        // app_data_dir() latches its FRESH decision in a process-global
        // OnceLock. If another test in this binary initialized it BEFORE our
        // set_var, `dir` would be the developer's REAL profile — never delete
        // that. Only proceed when we got the expected per-process temp root.
        let expected =
            std::env::temp_dir().join(format!("host_launcher_fresh_{}", std::process::id()));
        assert_eq!(
            dir, expected,
            "app_data_dir() was initialized before this test set HOST_LAUNCHER_FRESH; \
             refusing to touch a possibly-real data dir"
        );
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // A legacy file with one embedded user app.
        let mut layout = LauncherLayout::default();
        layout.user_apps.push(MiniAppManifest {
            id: "legacy-app".into(),
            name: "Legacy".into(),
            icon: "🧪".into(),
            tint: 0x123456,
            source: "View{}".into(),
            allow_net: false,
            builtin: false,
            widget: Some(WidgetManifest {
                source: "View{height:Fit}".into(),
                default_span: (2, 2),
                min_span: (1, 1),
            }),
            shortcuts: vec!["Open".into()],
        });
        std::fs::write(
            dir.join(LEGACY_LAYOUT_FILE_NAME),
            serde_json::to_vec(&layout).unwrap(),
        )
        .unwrap();

        let loaded = load_launcher_layout().unwrap().expect("migrated layout");
        assert_eq!(loaded.user_apps.len(), 1);
        let app = &loaded.user_apps[0];
        assert_eq!(app.id, "legacy-app");
        assert_eq!(app.source, "View{}");
        assert_eq!(app.widget.as_ref().unwrap().default_span, (2, 2));

        // Modular files exist; legacy parked; slim layout has no sources.
        assert!(dir.join("apps/legacy-app/app.splash").exists());
        assert!(dir.join("apps/legacy-app/widget.splash").exists());
        assert!(dir.join("apps/legacy-app/manifest.json").exists());
        assert!(dir.join("launcher_layout.json.migrated.bak").exists());
        let slim = std::fs::read_to_string(dir.join(LAYOUT_FILE_NAME)).unwrap();
        assert!(!slim.contains("View{}"));

        // Uninstall removes code AND data dirs.
        std::fs::create_dir_all(dir.join("app_data/legacy-app")).unwrap();
        remove_user_app("legacy-app");
        assert!(!dir.join("apps/legacy-app").exists());
        assert!(!dir.join("app_data/legacy-app").exists());
    }

    #[test]
    fn collision_index_orders_same_second_snapshots() {
        // Plain stamps have no counter; suffixed ones sort numerically, so a
        // 10th snapshot in one second still comes after the 2nd.
        assert_eq!(collision_index("20260727-105412"), 0);
        assert_eq!(collision_index("20260727-105412-2"), 2);
        assert_eq!(collision_index("20260727-105412-10"), 10);
        assert!(collision_index("20260727-105412-10") > collision_index("20260727-105412-2"));
    }

    #[test]
    fn app_id_safety_rejects_path_structure() {
        assert!(is_safe_app_id("tip-calc"));
        assert!(is_safe_app_id("Weather2"));
        for bad in ["", ".", "..", "a/b", "../x", "/etc", "a\\b", "x\0y"] {
            assert!(!is_safe_app_id(bad), "{bad:?} should be rejected");
        }
        // save refuses an unsafe id outright.
        let m = MiniAppManifest {
            id: "../escape".into(),
            name: "X".into(),
            icon: "x".into(),
            tint: 0,
            source: "View{}".into(),
            allow_net: false,
            builtin: false,
            widget: None,
            shortcuts: vec![],
        };
        assert!(save_user_app(&m).is_err());
    }
}
