//! The manifests of all pre-installed mini-apps (and the deletable user samples).
//!
//! Splash sources live in the `apps/` directory. They're baked into the binary,
//! but in a dev checkout we prefer reading them from disk so `.splash` edits
//! show up on the next app launch without a rebuild.

use std::sync::OnceLock;

use crate::mini_apps::registry::{MiniAppManifest, WidgetManifest};

fn load_source(file: &str, baked: &'static str) -> String {
    let dev_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("apps")
        .join(file);
    std::fs::read_to_string(dev_path).unwrap_or_else(|_| baked.to_string())
}

macro_rules! app_source {
    ($file:literal) => {
        load_source($file, include_str!(concat!("../../apps/", $file)))
    };
}

fn app(
    id: &str,
    name: &str,
    icon: &str,
    tint: u32,
    source: String,
    builtin: bool,
) -> MiniAppManifest {
    let mut manifest = MiniAppManifest {
        id: id.to_string(),
        name: name.to_string(),
        icon: icon.to_string(),
        tint,
        source,
        allow_net: false,
        permissions: permissions_for(id),
        builtin,
        widget: None,
        shortcuts: shortcuts_for(id),
    };
    manifest.normalize_permissions();
    manifest
}

/// What each stock app DECLARES (docs/PERMISSIONS.md). Declaring is not
/// granting: runtime-tier entries still prompt on first use. Apps absent here
/// are fully sandboxed on purpose — don't add "just in case" entries.
fn permissions_for(id: &str) -> Vec<String> {
    let p: &[&str] = match id {
        "weather" => &["network", "location"],
        "news" => &["network", "open-url", "notifications"],
        "clock" => &["network"],
        "notes" => &["clipboard-write", "share", "ipc"],
        "todo" => &["ipc"],
        "calculator" => &["clipboard-write"],
        _ => &[],
    };
    p.iter().map(|x| x.to_string()).collect()
}

/// A couple of quick-action shortcuts per app for the long-press menu.
fn shortcuts_for(id: &str) -> Vec<String> {
    let s: &[&str] = match id {
        "notes" => &["＋  New Note", "🔍  Search Notes"],
        "todo" => &["＋  Add Task", "✓  Clear Completed"],
        "weather" => &["📍  My Location", "＋  Add City"],
        "clock" => &["⏰  Add Alarm", "⏱  Start Timer"],
        "calculator" => &["🧮  Scientific", "🕘  History"],
        "music" => &["▶  Play", "🔀  Shuffle All"],
        "calendar" => &["＋  New Event", "📅  Today"],
        "gallery" => &["📷  Camera", "⭐  Favorites"],
        "news" => &["🔖  Saved", "🔄  Refresh"],
        _ => &[],
    };
    s.iter().map(|x| x.to_string()).collect()
}

/// The pre-installed apps: always present, can't be uninstalled.
pub fn builtin_apps() -> Vec<MiniAppManifest> {
    vec![
        MiniAppManifest {
            widget: Some(WidgetManifest {
                source: app_source!("weather_widget.splash"),
                default_span: (2, 2),
                min_span: (2, 1),
            }),
            ..app(
                "weather",
                "Weather",
                "🌤",
                0x4A90D9,
                app_source!("weather.splash"),
                true,
            )
        },
        MiniAppManifest {
            widget: Some(WidgetManifest {
                source: app_source!("clock_widget.splash"),
                default_span: (2, 1),
                min_span: (2, 1),
            }),
            ..app("clock", "Clock", "🕐", 0x4AC2C2, app_source!("clock.splash"), true)
        },
        app("news", "News", "📰", 0xE8734A, app_source!("news.splash"), true),
        app("todo", "To-Do", "✅", 0x4AC274, app_source!("todo.splash"), true),
        app("notes", "Notes", "📝", 0xE8C84A, app_source!("notes.splash"), true),
        app(
            "calculator",
            "Calculator",
            "🧮",
            0x8E7CE8,
            app_source!("calculator.splash"),
            true,
        ),
        app(
            "settings",
            "Settings",
            "⚙️",
            0x9AA5B5,
            app_source!("settings.splash"),
            true,
        ),
        app(
            "calendar",
            "Calendar",
            "📅",
            0xE84A6F,
            app_source!("calendar.splash"),
            true,
        ),
        app("music", "Music", "🎵", 0xD94A90, app_source!("music.splash"), true),
        app(
            "gallery",
            "Gallery",
            "🖼",
            0x4A6FE8,
            app_source!("gallery.splash"),
            true,
        ),
        app(
            "isolation_probe",
            "Sandbox",
            "🛡",
            0x6FD98F,
            app_source!("isolation_probe.splash"),
            true,
        ),
    ]
}

/// Sample "user-installed" apps, seeded on first run. Unlike the built-ins,
/// these can be uninstalled — and, via the App Store, reinstalled.
///
/// Memoized: each manifest's Splash source is read from disk once per process
/// (like the built-ins, which are constructed once at startup), so the store and
/// install paths don't re-hit the disk on every open / Get / Remove.
pub fn user_sample_apps() -> Vec<MiniAppManifest> {
    static CACHE: OnceLock<Vec<MiniAppManifest>> = OnceLock::new();
    CACHE.get_or_init(build_user_sample_apps).clone()
}

fn build_user_sample_apps() -> Vec<MiniAppManifest> {
    vec![
        MiniAppManifest {
            // The counter's home-screen widget bumps the count in place — the
            // reference "interactive widget".
            widget: Some(WidgetManifest {
                source: app_source!("counter_widget.splash"),
                default_span: (2, 2),
                min_span: (2, 1),
            }),
            ..app(
                "counter",
                "Counter",
                "🔢",
                0x6FA6FF,
                app_source!("counter.splash"),
                false,
            )
        },
        app(
            "stopwatch",
            "Stopwatch",
            "⏱",
            0xFFA64A,
            app_source!("stopwatch.splash"),
            false,
        ),
    ]
}

/// Apps offered in the in-launcher "App Store" but NOT seeded on first run:
/// they exist only once the user installs one (which copies the manifest into
/// the persisted `user_apps`). Like the samples, installed catalog apps can be
/// uninstalled. Keep ids distinct from every built-in and sample id. Memoized
/// (see `user_sample_apps`).
pub fn installable_catalog() -> Vec<MiniAppManifest> {
    static CACHE: OnceLock<Vec<MiniAppManifest>> = OnceLock::new();
    CACHE.get_or_init(build_installable_catalog).clone()
}

fn build_installable_catalog() -> Vec<MiniAppManifest> {
    vec![
        MiniAppManifest {
            // Dice ships a tap-to-roll interactive widget.
            widget: Some(WidgetManifest {
                source: app_source!("dice_widget.splash"),
                default_span: (2, 2),
                min_span: (2, 1),
            }),
            ..app("dice", "Dice", "🎲", 0x8E7CE8, app_source!("dice.splash"), false)
        },
        app("tip", "Tip", "💵", 0x4AC274, app_source!("tip.splash"), false),
    ]
}

/// Every app the App Store can install or remove: the on-demand catalog plus the
/// seeded samples. Listing the samples here means a removed sample shows a "Get"
/// button and can be reinstalled, exactly like a catalog app (no dead-end
/// removals). Ids are distinct across the two, so no de-dup is needed.
pub fn store_catalog() -> Vec<MiniAppManifest> {
    let mut v = installable_catalog();
    v.extend(user_sample_apps());
    v
}
