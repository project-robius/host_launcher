//! SCRATCH TEST (verification only — delete after running): does a hostile
//! app id inside a legacy launcher_layout.json escape the data dir during
//! migration?

use host_launcher::app_data_dir;
use host_launcher::mini_apps::registry::{LauncherLayout, MiniAppManifest};
use host_launcher::persistence;

fn manifest(id: &str) -> MiniAppManifest {
    MiniAppManifest {
        id: id.into(),
        name: "X".into(),
        icon: "x".into(),
        tint: 0,
        source: "View{}".into(),
        allow_net: false,
        builtin: false,
        widget: None,
        shortcuts: vec![],
    }
}

#[test]
fn legacy_migration_writes_outside_data_dir_for_traversal_ids() {
    // SAFETY: single-threaded at this point; flips app_data_dir to temp root.
    unsafe { std::env::set_var("HOST_LAUNCHER_FRESH", "1") };
    let dir = app_data_dir().to_path_buf();
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let