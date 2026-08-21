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

/// What a policy can say about an app, in container terms.
///
/// One WEIGHT decides who yields when apps compete for anything, and the
/// rest are absolute ceilings that are off (or set to runaway backstops) by
/// default. Nothing here limits an app on a system with room to spare — see
/// makepad's `splash_limits` for the mechanism.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Resource {
    /// This app's pull on every contended resource. The only knob that does
    /// anything on an idle system: nothing.
    Priority,
    /// Absolute cap on processor time per second, whatever else is running.
    CpuMax,
    /// Absolute cap on memory held.
    MemoryMax,
    /// Live timers it may hold.
    Timers,
    /// The fastest timer it may ask for, in milliseconds.
    TimerFloor,
    /// Concurrent downloads.
    Http,
    /// Bytes in its private storage jail. Not on the share model: disk is not
    /// given back when pressure passes, so a share of it would be a share of
    /// something nobody returns.
    Storage,
}

impl Resource {
    pub const ALL: [Resource; 7] = [
        Resource::Priority,
        Resource::CpuMax,
        Resource::MemoryMax,
        Resource::Timers,
        Resource::TimerFloor,
        Resource::Http,
        Resource::Storage,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Resource::Priority => "priority",
            Resource::CpuMax => "cpu-max",
            Resource::MemoryMax => "memory-max",
            Resource::Timers => "timers",
            Resource::TimerFloor => "timer-floor",
            Resource::Http => "http",
            Resource::Storage => "storage",
        }
    }

    pub fn from_str(s: &str) -> Option<Resource> {
        Resource::ALL.into_iter().find(|r| r.id() == s)
    }

    pub fn title(self) -> &'static str {
        match self {
            Resource::Priority => "Priority",
            Resource::CpuMax => "Processor limit",
            Resource::MemoryMax => "Memory limit",
            Resource::Timers => "Timers",
            Resource::TimerFloor => "Fastest timer",
            Resource::Http => "Downloads at once",
            Resource::Storage => "Storage",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Resource::Priority => "⚖️",
            Resource::CpuMax => "⚡",
            Resource::MemoryMax => "🧠",
            Resource::Timers => "⏰",
            Resource::TimerFloor => "🏃",
            Resource::Http => "📶",
            Resource::Storage => "💾",
        }
    }

    /// What this actually does to the app, in the user's terms. A number with
    /// no consequence attached is not a decision anyone can make.
    pub fn blurb(self) -> &'static str {
        match self {
            Resource::Priority => "Who gets more when apps compete. On its own, an app is never held back.",
            Resource::CpuMax => "A hard limit on processor time, even when nothing else is running.",
            Resource::MemoryMax => "A hard limit on memory. Over it, the app is stopped.",
            Resource::Timers => "Repeating jobs it may keep going at once.",
            Resource::TimerFloor => "How often its fastest job may repeat. Anything quicker is slowed to this.",
            Resource::Http => "How many things it may download at the same time.",
            Resource::Storage => "How much it may keep in its private folder.",
        }
    }

    /// The exact amounts offered, smallest first. `0` means "no limit" for
    /// the ceilings, which is what they ship as.
    pub fn choices(self) -> &'static [(u64, &'static str)] {
        match self {
            Resource::Priority => &[
                (1, "Low — yields to everything"),
                (4, "Normal"),
                (16, "High — wins most contests"),
                (64, "Highest"),
            ],
            Resource::CpuMax => &[
                (0, "No limit — just its share"),
                (100, "10% of each second"),
                (250, "25%"),
                (500, "50%"),
            ],
            Resource::MemoryMax => &[
                (0, "No limit — just its share"),
                (2_000_000, "2M slots"),
                (8_000_000, "8M slots"),
                (24_000_000, "24M slots"),
            ],
            Resource::Timers => &[(8, "8"), (64, "64"), (256, "256 — normal"), (1024, "1024")],
            Resource::TimerFloor => &[
                (16, "16ms — every frame"),
                (100, "100ms"),
                (1000, "1 second"),
            ],
            Resource::Http => &[(2, "2"), (4, "4"), (16, "16 — normal"), (64, "64")],
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
            Resource::Priority => match value {
                0..=1 => "Low".to_string(),
                2..=8 => "Normal".to_string(),
                9..=32 => "High".to_string(),
                _ => "Highest".to_string(),
            },
            Resource::CpuMax if value == 0 => "No limit".to_string(),
            Resource::CpuMax => format!("{}% of each second", (value as f64 / 10.0).round() as u64),
            Resource::MemoryMax if value == 0 => "No limit".to_string(),
            Resource::MemoryMax => format!("{:.0}M slots", value as f64 / 1_000_000.0),
            Resource::TimerFloor => {
                if value >= 1000 {
                    format!("{:.1}s", value as f64 / 1000.0)
                } else {
                    format!("{value}ms")
                }
            }
            Resource::Timers | Resource::Http => value.to_string(),
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
        let base = match surface {
            Surface::Foreground => SplashLimits::default(),
            Surface::Background => SplashLimits::background(),
        };
        let zero_is_none = |v: u64| (v != 0).then_some(v);
        SplashLimits {
            weight: get(Resource::Priority) as u32,
            cpu_max_ms: zero_is_none(get(Resource::CpuMax)),
            mem_max_slots: zero_is_none(get(Resource::MemoryMax))
                .map(|v| v as usize)
                .unwrap_or(base.mem_max_slots),
            timers_max: get(Resource::Timers) as u32,
            min_timer_interval_s: get(Resource::TimerFloor) as f64 / 1000.0,
            http_max: get(Resource::Http) as u32,
            ..base
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
        Resource::Priority => l.weight as u64,
        // The ceilings ship OFF: an absolute cap on an idle system is a tax
        // with no beneficiary, which is the whole point of the share model.
        Resource::CpuMax => l.cpu_max_ms.unwrap_or(0),
        Resource::MemoryMax => 0,
        Resource::Timers => l.timers_max as u64,
        Resource::TimerFloor => (l.min_timer_interval_s * 1000.0).round() as u64,
        Resource::Http => l.http_max as u64,
        // Storage is a quota, not a share (docs/PERMISSIONS.md): disk is not
        // handed back when pressure passes.
        Resource::Storage => DEFAULT_JAIL_BYTES,
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

    /// The shape of the whole model: an app on its own is limited by nothing.
    /// Only PRIORITY ships with a value, and priority does nothing until apps
    /// compete.
    #[test]
    fn the_ceilings_ship_off() {
        for surface in [Surface::Foreground, Surface::Background] {
            assert_eq!(default_amount(surface, Resource::CpuMax), 0, "no processor cap");
            assert_eq!(default_amount(surface, Resource::MemoryMax), 0, "no memory cap");
        }
        assert!(default_amount(Surface::Foreground, Resource::Priority) > 0);
    }

    /// A tile yields to a foreground app when they compete, and is not
    /// otherwise second-class.
    #[test]
    fn a_tile_has_less_pull_not_a_smaller_cage() {
        assert!(
            default_amount(Surface::Background, Resource::Priority)
                < default_amount(Surface::Foreground, Resource::Priority)
        );
        assert_eq!(
            default_amount(Surface::Background, Resource::CpuMax),
            default_amount(Surface::Foreground, Resource::CpuMax),
            "no extra ceiling just for being a tile"
        );
    }

    /// One resource can be changed without disturbing the others.
    #[test]
    fn an_override_is_per_resource() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::Timers, 1024);
        assert_eq!(p.amount("t", Surface::Foreground, Resource::Timers), 1024);
        assert_eq!(
            p.amount("t", Surface::Foreground, Resource::Priority),
            default_amount(Surface::Foreground, Resource::Priority),
            "an untouched resource keeps its default"
        );
        assert_eq!(p.override_count("t"), 1);

        p.clear("t", Resource::Timers);
        assert_eq!(p.override_count("t"), 0);
    }

    /// An override applies on every surface: the user asked for that.
    #[test]
    fn an_override_beats_both_defaults() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::Priority, 64);
        assert_eq!(p.amount("t", Surface::Foreground, Resource::Priority), 64);
        assert_eq!(p.amount("t", Surface::Background, Resource::Priority), 64);
    }

    /// What the launcher hands makepad carries the policy across, in
    /// makepad's units, with "no limit" surviving as no limit.
    #[test]
    fn limits_carry_the_policy_across() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::Priority, 16);
        p.set("t", Resource::TimerFloor, 1000);
        let l = p.limits_for("t", Surface::Foreground);
        assert_eq!(l.weight, 16);
        assert_eq!(l.min_timer_interval_s, 1.0, "ms in the store, seconds in the VM");
        assert!(l.cpu_max_ms.is_none(), "untouched ceilings stay off");

        p.set("t", Resource::CpuMax, 250);
        assert_eq!(p.limits_for("t", Surface::Foreground).cpu_max_ms, Some(250));
    }

    /// Storage is the one resource with a capability behind it as well as an
    /// amount: the user's number wins, then what storage-large buys, then the
    /// standard jail.
    #[test]
    fn storage_respects_both_the_grant_and_the_override() {
        let mut p = ResourcePolicy::default();
        assert_eq!(p.storage_bytes("t", Surface::Foreground, false), None);
        assert_eq!(
            p.storage_bytes("t", Surface::Foreground, true),
            Some(crate::permissions::LARGE_JAIL_BYTES)
        );
        p.set("t", Resource::Storage, 256 * 1024 * 1024);
        assert_eq!(
            p.storage_bytes("t", Surface::Foreground, true),
            Some(256 * 1024 * 1024),
            "an amount the user chose is not overruled by a capability"
        );
    }

    /// Every preset renders, and the lists ascend so the UI can show a ladder.
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
        assert_eq!(Resource::CpuMax.format(0), "No limit");
        assert_eq!(Resource::Priority.format(4), "Normal");
    }

    /// Overrides survive a restart; they are the user's decision.
    #[test]
    fn overrides_persist() {
        let mut p = ResourcePolicy::default();
        p.set("t", Resource::Http, 64);
        let json = serde_json::to_string(&p).unwrap();
        let back: ResourcePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back.override_for("t", Resource::Http), Some(64));
    }
}
