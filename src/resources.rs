//! How much of the machine a mini-app may use, and who decides.
//!
//! Permissions answer "may this app do X" (docs/PERMISSIONS.md) and the
//! request budget in `services/limits.rs` answers "how often may it ask the
//! host for X". Neither says anything about what an app does entirely inside
//! its own isolate: a tight timer burning the frame, a structure that grows
//! every tick, forty HTTP requests in flight. That is this module.
//!
//! The mechanisms live in makepad (`splash_limits`), because only the VM can
//! meter its own execution. What lives here is POLICY: the numbers, where they
//! differ by surface, and the user's ability to change them per app.
//!
//! Three layers, in order:
//!
//! 1. **Defaults by surface.** A foreground app the user is looking at gets a
//!    generous share; a home-screen tile gets a fraction of it, because a tile
//!    competes with every other tile for one frame and is by definition not
//!    what the user is waiting on.
//! 2. **Per-app overrides**, persisted, set by the user. Any single resource
//!    can be raised or lowered without touching the others.
//! 3. **Whatever the app then does** — measured by makepad, reported back as
//!    limit events, and fed into the same strike ladder that handles request
//!    flooding.

use std::collections::BTreeMap;

use makepad_widgets::splash_limits::SplashLimits;
use serde::{Deserialize, Serialize};

use crate::mini_apps::registry::MiniAppId;

/// The resources a policy can name. One enum so the UI, the store and the
/// defaults cannot drift apart: adding a resource is one variant plus one
/// match arm in each impl, and the compiler finds the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    /// Milliseconds of script execution per second, summed across every entry.
    Cpu,
    /// Milliseconds any single entry into the app's script may run.
    EntryTime,
    /// How many live timers the app may hold.
    Timers,
    /// The fastest timer it may ask for, in milliseconds.
    TimerFloor,
    /// Live heap slots it may hold after a collection.
    Heap,
    /// Concurrent HTTP requests.
    Http,
    /// Bytes in its private storage jail.
    Storage,
}

impl Resource {
    pub const ALL: [Resource; 7] = [
        Resource::Cpu,
        Resource::EntryTime,
        Resource::Timers,
        Resource::TimerFloor,
        Resource::Heap,
        Resource::Http,
        Resource::Storage,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Resource::Cpu => "cpu",
            Resource::EntryTime => "entry-time",
            Resource::Timers => "timers",
            Resource::TimerFloor => "timer-floor",
            Resource::Heap => "heap",
            Resource::Http => "http",
            Resource::Storage => "storage",
        }
    }

    pub fn from_str(s: &str) -> Option<Resource> {
        Resource::ALL.into_iter().find(|r| r.id() == s)
    }

    pub fn title(self) -> &'static str {
        match self {
            Resource::Cpu => "Processor time",
            Resource::EntryTime => "Single run",
            Resource::Timers => "Timers",
            Resource::TimerFloor => "Fastest timer",
            Resource::Heap => "Memory",
            Resource::Http => "Downloads at once",
            Resource::Storage => "Storage",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Resource::Cpu => "⚡",
            Resource::EntryTime => "⏱️",
            Resource::Timers => "⏰",
            Resource::TimerFloor => "🏃",
            Resource::Heap => "🧠",
            Resource::Http => "📶",
            Resource::Storage => "💾",
        }
    }

    /// What running out of this one actually does to the app, in the user's
    /// terms. Shown under the value, because a number with no consequence
    /// attached is not a decision anyone can make.
    pub fn blurb(self) -> &'static str {
        match self {
            Resource::Cpu => "How much of each second it may spend running. Over this, it's paused until the next second.",
            Resource::EntryTime => "How long one piece of its work may take before it's cut off.",
            Resource::Timers => "Repeating jobs it can keep going at once.",
            Resource::TimerFloor => "How often its fastest job may repeat. Anything quicker is slowed to this.",
            Resource::Heap => "How much it may keep in memory. Over this, it's stopped.",
            Resource::Http => "How many things it may download at the same time.",
            Resource::Storage => "How much it may keep in its private folder.",
        }
    }

    /// The exact amounts offered for this resource, smallest first. Presets
    /// rather than a free-text field: these are real units with real
    /// consequences, and a typo'd "5" for milliseconds-per-second would make
    /// an app look broken with no way to tell why.
    pub fn choices(self) -> &'static [(u64, &'static str)] {
        match self {
            Resource::Cpu => &[
                (60, "6% — barely"),
                (250, "25% — normal"),
                (500, "50% — generous"),
                (900, "90% — nearly all of it"),
            ],
            Resource::EntryTime => &[
                (16, "16ms — one frame"),
                (64, "64ms — normal"),
                (250, "250ms — long jobs"),
            ],
            Resource::Timers => &[(4, "4"), (8, "8"), (32, "32 — normal"), (128, "128")],
            Resource::TimerFloor => &[
                (16, "16ms — every frame"),
                (100, "100ms"),
                (1000, "1 second"),
            ],
            Resource::Heap => &[
                (250_000, "250k — small"),
                (500_000, "500k"),
                (2_000_000, "2M — normal"),
                (8_000_000, "8M — large"),
            ],
            Resource::Http => &[(1, "1"), (2, "2"), (8, "8 — normal"), (32, "32")],
            Resource::Storage => &[
                (16 * 1024 * 1024, "16 MB — normal"),
                (64 * 1024 * 1024, "64 MB"),
                (256 * 1024 * 1024, "256 MB"),
            ],
        }
    }

    /// Renders an amount in this resource's own units.
    pub fn format(self, value: u64) -> String {
        match self {
            Resource::Cpu => format!("{}% of each second", (value as f64 / 10.0).round() as u64),
            Resource::EntryTime | Resource::TimerFloor => {
                if value >= 1000 {
                    format!("{:.1}s", value as f64 / 1000.0)
                } else {
                    format!("{value}ms")
                }
            }
            Resource::Timers | Resource::Http => value.to_string(),
            Resource::Heap => {
                if value >= 1_000_000 {
                    format!("{:.1}M slots", value as f64 / 1_000_000.0)
                } else {
                    format!("{}k slots", value / 1000)
                }
            }
            Resource::Storage => format!("{} MB", value / (1024 * 1024)),
        }
    }
}

/// Which surface an isolate is, for picking defaults. The same app gets
/// different numbers depending on where it's running, which is the whole
/// reason these are per-isolate rather than per-app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// The app on screen, which the user is waiting on.
    Foreground,
    /// A home-screen tile or preview, running beside eleven others.
    Background,
}

/// The user's per-app amounts. Absent = whatever the surface default says, so
/// changing a default later reaches every app that never overrode it.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct ResourcePolicy {
    #[serde(default)]
    overrides: BTreeMap<MiniAppId, BTreeMap<String, u64>>,
}

impl ResourcePolicy {
    /// The amount in force for one app and surface.
    pub fn amount(&self, app_id: &str, surface: Surface, resource: Resource) -> u64 {
        self.override_for(app_id, resource)
            .unwrap_or_else(|| default_amount(surface, resource))
    }

    /// The user's amount, if they set one.
    pub fn override_for(&self, app_id: &str, resource: Resource) -> Option<u64> {
        self.overrides
            .get(app_id)
            .and_then(|m| m.get(resource.id()))
            .copied()
    }

    /// Sets one resource for one app, leaving the rest alone.
    pub fn set(&mut self, app_id: &str, resource: Resource, amount: u64) {
        self.overrides
            .entry(app_id.to_string())
            .or_default()
            .insert(resource.id().to_string(), amount);
    }

    /// Puts one resource back to the surface default.
    pub fn clear(&mut self, app_id: &str, resource: Resource) {
        if let Some(m) = self.overrides.get_mut(app_id) {
            m.remove(resource.id());
            if m.is_empty() {
                self.overrides.remove(app_id);
            }
        }
    }

    /// Puts every resource for one app back to defaults.
    pub fn clear_app(&mut self, app_id: &str) {
        self.overrides.remove(app_id);
    }

    /// How many resources this app has overridden, for the App Info summary.
    pub fn override_count(&self, app_id: &str) -> usize {
        self.overrides.get(app_id).map(|m| m.len()).unwrap_or(0)
    }

    /// The limits to hand makepad for one isolate of this app.
    pub fn limits_for(&self, app_id: &str, surface: Surface) -> SplashLimits {
        let get = |r: Resource| self.amount(app_id, surface, r);
        SplashLimits {
            entry_time_ms: get(Resource::EntryTime),
            cpu_per_window_ms: get(Resource::Cpu),
            max_timers: get(Resource::Timers) as u32,
            min_timer_interval_s: get(Resource::TimerFloor) as f64 / 1000.0,
            live_heap_slots: get(Resource::Heap) as usize,
            max_inflight_http: get(Resource::Http) as u32,
            ..SplashLimits::default()
        }
    }

    /// The storage jail cap for this app, which makepad takes separately.
    ///
    /// Storage is the one resource with a PERMISSION behind it as well as an
    /// amount: `storage-large` raises the baseline (docs/PERMISSIONS.md). So
    /// the order is the user's explicit amount first, then what the grant
    /// buys, then the standard jail — an override the user typed should not
    /// be silently overruled by a capability, and a capability they granted
    /// should not be ignored because they never touched this row.
    pub fn storage_bytes(&self, app_id: &str, surface: Surface, granted_large: bool) -> Option<u64> {
        if let Some(exact) = self.override_for(app_id, Resource::Storage) {
            return Some(exact);
        }
        if granted_large {
            return Some(crate::permissions::LARGE_JAIL_BYTES);
        }
        let default = default_amount(surface, Resource::Storage);
        // None = "leave makepad's own default alone", which is what the jail
        // already enforces; saying it again would just be a second place to
        // keep the same number.
        (default != DEFAULT_JAIL_BYTES).then_some(default)
    }
}

/// The jail size makepad applies when no host says otherwise. Mirrored here
/// only so `storage_bytes` can tell "the default" from "an amount the user
/// chose that happens to equal it".
const DEFAULT_JAIL_BYTES: u64 = 16 * 1024 * 1024;

thread_local! {
    /// The policy the isolate-creation sites read. Those sites live inside
    /// widgets that have no `AppState` in reach, so the launcher publishes
    /// the current policy the same way it publishes the grant snapshot — one
    /// source of truth, pushed on every change.
    static POLICY_SNAPSHOT: std::cell::RefCell<ResourcePolicy> =
        std::cell::RefCell::new(ResourcePolicy::default());
}

/// Publishes the current policy for the isolate-creation sites to read.
pub fn publish_policy(policy: &ResourcePolicy) {
    POLICY_SNAPSHOT.with(|p| *p.borrow_mut() = policy.clone());
}

/// The limits an isolate of this app should be created with, per the last
/// published policy. Defaults when nothing has been published yet, which is
/// the right answer for a preview or a validation run.
pub fn snapshot_limits_for(app_id: &str, surface: Surface) -> SplashLimits {
    POLICY_SNAPSHOT.with(|p| p.borrow().limits_for(app_id, surface))
}

/// The storage cap for one isolate, per the last published policy and this
/// app's grants. Replaces `permissions::storage_quota_for` at the isolate
/// sites so the amount the user sees in App Info is the one that applies.
pub fn snapshot_storage_bytes(app_id: &str, surface: Surface, caps: &[String]) -> Option<u64> {
    let granted_large = caps
        .iter()
        .any(|c| c == crate::permissions::Permission::StorageLarge.as_str());
    POLICY_SNAPSHOT.with(|p| p.borrow().storage_bytes(app_id, surface, granted_large))
}

/// The shipped amount for a surface, when the user hasn't said otherwise.
/// Derived from makepad's own defaults so there is one source of truth for
/// "normal", with the background surface deliberately tighter.
pub fn default_amount(surface: Surface, resource: Resource) -> u64 {
    let l = match surface {
        Surface::Foreground => SplashLimits::default(),
        Surface::Background => SplashLimits::background(),
    };
    match resource {
        Resource::Cpu => l.cpu_per_window_ms,
        Resource::EntryTime => l.entry_time_ms,
        Resource::Timers => l.max_timers as u64,
        Resource::TimerFloor => (l.min_timer_interval_s * 1000.0).round() as u64,
        Resource::Heap => l.live_heap_slots as u64,
        Resource::Http => l.max_inflight_http as u64,
        // Storage is the one resource that predates this module; its default
        // is the jail's own, and `storage-large` raises it (docs/PERMISSIONS.md).
        Resource::Storage => match surface {
            Surface::Foreground => 16 * 1024 * 1024,
            Surface::Background => 16 * 1024 * 1024,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for r in Resource::ALL {
            assert_eq!(Resource::from_str(r.id()), Some(r));
        }
        assert_eq!(Resource::from_str("nope"), None);
    }

    /// A tile gets a smaller share than the same app on screen.
    #[test]
    fn background_defaults_are_tighter() {
        for r in [Resource::Cpu, Resource::Timers, Resource::Heap, Resource::Http] {
            assert!(
                default_amount(Surface::Background, r) < default_amount(Surface::Foreground, r),
                "{} should be tighter in the background",
                r.id()
            );
        }
        // The timer floor is the one that goes UP when tightening: a slower
        // minimum interval is the stricter setting.
        assert!(
            default_amount(Surface::Background, Resource::TimerFloor)
                > default_amount(Surface::Foreground, Resource::TimerFloor)
        );
    }

    /// One resource can be changed without disturbing the others.
    #[test]
    fn an_override_is_per_resource() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::Timers, 128);
        assert_eq!(p.amount("t", Surface::Foreground, Resource::Timers), 128);
        assert_eq!(
            p.amount("t", Surface::Foreground, Resource::Cpu),
            default_amount(Surface::Foreground, Resource::Cpu),
            "an untouched resource keeps its default"
        );
        assert_eq!(p.override_count("t"), 1);

        p.clear("t", Resource::Timers);
        assert_eq!(
            p.amount("t", Surface::Foreground, Resource::Timers),
            default_amount(Surface::Foreground, Resource::Timers)
        );
        assert_eq!(p.override_count("t"), 0);
    }

    /// An override applies on every surface: the user asked for that number.
    #[test]
    fn an_override_beats_both_defaults() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::Cpu, 900);
        assert_eq!(p.amount("t", Surface::Foreground, Resource::Cpu), 900);
        assert_eq!(p.amount("t", Surface::Background, Resource::Cpu), 900);
    }

    /// What the launcher hands makepad reflects the policy, in makepad's units.
    #[test]
    fn limits_carry_the_policy_across() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::TimerFloor, 1000);
        p.set("t", Resource::Heap, 250_000);
        let l = p.limits_for("t", Surface::Foreground);
        assert_eq!(l.min_timer_interval_s, 1.0, "ms in the store, seconds in the VM");
        assert_eq!(l.live_heap_slots, 250_000);
        assert_eq!(
            l.cpu_per_window_ms,
            default_amount(Surface::Foreground, Resource::Cpu)
        );
    }

    /// Every preset is a value the formatter can render, and the lists are
    /// ordered so the UI can show them as a ladder.
    #[test]
    fn presets_are_ordered_and_printable() {
        for r in Resource::ALL {
            let choices = r.choices();
            assert!(!choices.is_empty(), "{} has no presets", r.id());
            for pair in choices.windows(2) {
                assert!(pair[0].0 < pair[1].0, "{} presets must ascend", r.id());
            }
            for (v, _) in choices {
                assert!(!r.format(*v).is_empty());
            }
        }
    }

    /// Storage answers to a permission as well as an amount, in that order:
    /// the user's explicit number wins, then what `storage-large` buys, then
    /// the standard jail.
    #[test]
    fn storage_respects_both_the_grant_and_the_override() {
        let mut p = ResourcePolicy::default();
        assert_eq!(
            p.storage_bytes("t", Surface::Foreground, false),
            None,
            "no grant, no override: makepad's own default stands"
        );
        assert_eq!(
            p.storage_bytes("t", Surface::Foreground, true),
            Some(crate::permissions::LARGE_JAIL_BYTES),
            "storage-large raises the baseline on its own"
        );
        p.set("t", Resource::Storage, 256 * 1024 * 1024);
        assert_eq!(
            p.storage_bytes("t", Surface::Foreground, false),
            Some(256 * 1024 * 1024),
            "an amount the user chose applies without any grant"
        );
        assert_eq!(
            p.storage_bytes("t", Surface::Foreground, true),
            Some(256 * 1024 * 1024),
            "and is not overruled by the capability"
        );
    }

    /// Overrides survive a restart; they are the user's decision, not a
    /// session preference.
    #[test]
    fn overrides_persist() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::Http, 32);
        let json = serde_json::to_string(&p).unwrap();
        let back: ResourcePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back.override_for("t", Resource::Http), Some(32));
    }
}
