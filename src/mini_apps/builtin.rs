//! The manifests of all pre-installed mini-apps (and the deletable user samples).
//!
//! Splash sources live in the `apps/` directory. They're baked into the binary,
//! but in a dev checkout we prefer reading them from disk so `.splash` edits
//! show up on the next app launch without a rebuild.

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
    MiniAppManifest {
        id: id.to_string(),
        name: name.to_string(),
        icon: icon.to_string(),
        tint,
        source,
        allow_net: false,
        builtin,
        widget: None,
        shortcuts: shortcuts_for(id),
    }
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
/// these can be uninstalled (and stay gone).
pub fn user_sample_apps() -> Vec<MiniAppManifest> {
    vec![
        app(
            "counter",
            "Counter",
            "🔢",
            0x6FA6FF,
            app_source!("counter.splash"),
            false,
        ),
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
