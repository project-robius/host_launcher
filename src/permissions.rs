//! The mini-app permission model: what an app may ask for, what the user has
//! answered, and what that nets out to. Design notes in docs/PERMISSIONS.md.
//!
//! Deny-by-default, Android-style declaration, iOS-style runtime prompts:
//! a capability is reachable only when the app DECLARES it (manifest) AND the
//! user's stored answer (or the tier default) allows it. Grants are launcher
//! state in `<data_dir>/permissions.json`, deliberately outside every app's
//! own storage jail.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::mini_apps::registry::{MiniAppId, MiniAppManifest};

/// Every capability a mini-app can declare. Kebab-case ids are the manifest /
/// wire form; keep them stable, exported bundles carry them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Permission {
    Network,
    Location,
    Notifications,
    ClipboardRead,
    Ipc,
    ClipboardWrite,
    OpenUrl,
    Files,
    Share,
    Auth,
    /// Keep this app's home-screen tiles (and their timers) alive while you
    /// are not in the app. Enforced by the launcher: revoke it and the tiles
    /// stop running rather than merely promising to.
    Background,
    /// Store more than the standard 16 MB in the app's private jail.
    /// Enforced in the storage jail itself, per isolate.
    StorageLarge,
}

/// Runtime permissions prompt the user on first use; normal ones auto-grant
/// on declaration (both stay user-revocable at any time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Normal,
    Runtime,
}

impl Permission {
    pub const ALL: [Permission; 12] = [
        Permission::Network,
        Permission::Location,
        Permission::Notifications,
        Permission::ClipboardRead,
        Permission::Ipc,
        Permission::ClipboardWrite,
        Permission::OpenUrl,
        Permission::Files,
        Permission::Share,
        Permission::Auth,
        Permission::Background,
        Permission::StorageLarge,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Permission::Network => "network",
            Permission::Location => "location",
            Permission::Notifications => "notifications",
            Permission::ClipboardRead => "clipboard-read",
            Permission::Ipc => "ipc",
            Permission::ClipboardWrite => "clipboard-write",
            Permission::OpenUrl => "open-url",
            Permission::Files => "files",
            Permission::Share => "share",
            Permission::Auth => "auth",
            Permission::Background => "background",
            Permission::StorageLarge => "storage-large",
        }
    }

    pub fn from_str(s: &str) -> Option<Permission> {
        Permission::ALL.into_iter().find(|p| p.as_str() == s)
    }

    pub fn tier(self) -> Tier {
        match self {
            Permission::Network
            | Permission::Location
            | Permission::Notifications
            | Permission::ClipboardRead
            | Permission::Ipc => Tier::Runtime,
            Permission::ClipboardWrite
            | Permission::OpenUrl
            | Permission::Files
            | Permission::Share
            | Permission::Auth
            // Both default ON when declared: a widget that silently stopped
            // updating, or a write that silently failed, would be a worse
            // experience than a switch the user can find and flip.
            | Permission::Background
            | Permission::StorageLarge => Tier::Normal,
        }
    }

    /// Short human name for rows and prompts.
    pub fn title(self) -> &'static str {
        match self {
            Permission::Network => "Network",
            Permission::Location => "Location",
            Permission::Notifications => "Notifications",
            Permission::ClipboardRead => "Read Clipboard",
            Permission::Ipc => "App Messaging",
            Permission::ClipboardWrite => "Write Clipboard",
            Permission::OpenUrl => "Open Links",
            Permission::Files => "Files",
            Permission::Share => "Share",
            Permission::Auth => "Authentication",
            Permission::Background => "Background Updates",
            Permission::StorageLarge => "Extra Storage",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Permission::Network => "🌐",
            Permission::Location => "📍",
            Permission::Notifications => "🔔",
            Permission::ClipboardRead => "📋",
            Permission::Ipc => "✉️",
            Permission::ClipboardWrite => "📋",
            Permission::OpenUrl => "🔗",
            Permission::Files => "📁",
            Permission::Share => "📤",
            Permission::Auth => "🔒",
            Permission::Background => "🔄",
            Permission::StorageLarge => "💾",
        }
    }

    /// What granting actually means, in the user's terms; shown on the prompt
    /// and under App Info rows.
    pub fn blurb(self) -> &'static str {
        match self {
            Permission::Network => "Connect to the internet to fetch and send data.",
            Permission::Location => "See your approximate location.",
            Permission::Notifications => "Show notification badges on its icon.",
            Permission::ClipboardRead => "Read whatever is on your clipboard.",
            Permission::Ipc => "Send messages to your other mini-apps.",
            Permission::ClipboardWrite => "Put text on your clipboard.",
            Permission::OpenUrl => "Open web links in your browser.",
            Permission::Files => "Open and save files you pick in the system dialog.",
            Permission::Share => "Open the system share sheet.",
            Permission::Auth => "Ask you to authenticate (Touch ID / password).",
            Permission::Background => {
                "Keep its home-screen widgets running while you're elsewhere."
            }
            Permission::StorageLarge => "Store more than 16 MB of its own data.",
        }
    }
}

/// The user's stored answer for one (app, permission).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GrantState {
    /// Never asked (or reset): runtime tiers prompt, normal tiers auto-grant.
    #[default]
    Ask,
    Granted,
    Denied,
}

/// What a request nets out to right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effective {
    Granted,
    Denied,
    /// Declared, runtime-tier, still Ask: park the request and prompt.
    NeedsPrompt,
    /// Not in the app's manifest: refuse without ever prompting.
    Undeclared,
}

/// One recorded use of a capability, for the "recent access" line App Info
/// and the manager show. Cheap and bounded — this is a privacy receipt, not
/// telemetry: it never leaves the device and holds no request contents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessRecord {
    pub app_id: MiniAppId,
    /// Permission id (string so an unknown-to-this-build entry survives).
    pub perm: String,
    /// Unix seconds.
    pub at: u64,
}

/// Most access records kept. A few hundred covers "what has this thing been
/// doing lately" without turning permissions.json's sibling into a log file.
pub const MAX_ACCESS_RECORDS: usize = 240;

/// All grants, keyed app id -> permission id. Owned by `AppState`, persisted
/// whole-file on every change (it is tiny, and a lost file just re-asks).
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PermissionStore {
    grants: BTreeMap<MiniAppId, BTreeMap<String, GrantState>>,
    /// Newest-last ring of capability uses (see [`AccessRecord`]).
    #[serde(default)]
    access: Vec<AccessRecord>,
    /// One-time grants: live for this launcher session only and never hit
    /// disk, so "Allow Once" cannot silently become forever. Cleared when the
    /// app's isolates are torn down, exactly like a phone dropping a one-shot
    /// grant when the app closes.
    #[serde(skip)]
    once: std::collections::HashSet<(MiniAppId, Permission)>,
    /// Grants that expire on the clock ("Allow for 1 hour"), as unix seconds.
    /// Persisted: an expiry survives a restart precisely because it ends by
    /// itself, so nothing is silently extended.
    #[serde(default)]
    until: BTreeMap<MiniAppId, BTreeMap<String, u64>>,
    /// How many times each app has actually exercised each capability.
    #[serde(default)]
    uses: BTreeMap<MiniAppId, BTreeMap<String, u64>>,
    /// Strict mode: normal-tier permissions stop auto-granting, so EVERY
    /// capability has to be allowed explicitly. Off by default — it trades
    /// convenience for control, which should be the user's choice.
    #[serde(default)]
    strict: bool,
}

impl PermissionStore {
    pub fn state(&self, app_id: &str, perm: Permission) -> GrantState {
        self.grants
            .get(app_id)
            .and_then(|m| m.get(perm.as_str()))
            .copied()
            .unwrap_or_default()
    }

    pub fn set(&mut self, app_id: &str, perm: Permission, state: GrantState) {
        // A durable answer supersedes any one-time grant for the same pair.
        self.once.remove(&(app_id.to_string(), perm));
        self.grants
            .entry(app_id.to_string())
            .or_default()
            .insert(perm.as_str().to_string(), state);
    }

    pub fn strict(&self) -> bool {
        self.strict
    }

    pub fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
    }

    /// Grants a capability until `until_unix` (the sheet's "Allow for 1 hour").
    pub fn grant_until(&mut self, app_id: &str, perm: Permission, until_unix: u64) {
        self.once.remove(&(app_id.to_string(), perm));
        self.grants
            .entry(app_id.to_string())
            .or_default()
            .remove(perm.as_str());
        self.until
            .entry(app_id.to_string())
            .or_default()
            .insert(perm.as_str().to_string(), until_unix);
    }

    /// When a timed grant for this pair runs out, if one is live.
    pub fn timed_until(&self, app_id: &str, perm: Permission, now: u64) -> Option<u64> {
        self.until
            .get(app_id)
            .and_then(|m| m.get(perm.as_str()))
            .copied()
            .filter(|until| *until > now)
    }

    /// Drops expired timed grants; returns whether anything changed (so the
    /// caller can republish the snapshot and re-apply to running isolates).
    pub fn expire_timed(&mut self, now: u64) -> Vec<(MiniAppId, Permission)> {
        let mut expired = Vec::new();
        for (app, perms) in self.until.iter_mut() {
            perms.retain(|id, until| {
                if *until > now {
                    return true;
                }
                if let Some(perm) = Permission::from_str(id) {
                    expired.push((app.clone(), perm));
                }
                false
            });
        }
        self.until.retain(|_, perms| !perms.is_empty());
        expired
    }

    /// Blocks every capability an app declares — App Info's "Block all".
    pub fn block_all(&mut self, manifest: &MiniAppManifest) {
        for perm in Permission::ALL {
            if manifest.declares(perm) {
                self.set(&manifest.id, perm, GrantState::Denied);
            }
        }
    }

    /// Forgets every answer for every app: back to first-run.
    pub fn reset_all(&mut self) {
        self.grants.clear();
        self.until.clear();
        self.once.clear();
    }

    /// How many times an app has used a capability.
    pub fn use_count(&self, app_id: &str, perm: Permission) -> u64 {
        self.uses
            .get(app_id)
            .and_then(|m| m.get(perm.as_str()))
            .copied()
            .unwrap_or(0)
    }

    /// Grants a capability for this session only (the prompt's "Allow Once").
    pub fn grant_once(&mut self, app_id: &str, perm: Permission) {
        self.once.insert((app_id.to_string(), perm));
    }

    pub fn has_once(&self, app_id: &str, perm: Permission) -> bool {
        self.once.contains(&(app_id.to_string(), perm))
    }

    /// Drops every one-time grant for an app — called when its isolates go
    /// away (force stop, uninstall, restart), which ends "this once".
    /// Reports whether anything was actually dropped, so callers can skip a
    /// snapshot republish when nothing changed.
    pub fn clear_once_for(&mut self, app_id: &str) -> bool {
        let before = self.once.len();
        self.once.retain(|(id, _)| id != app_id);
        before != self.once.len()
    }

    /// Forget an app entirely (uninstall). A reinstall starts from Ask.
    pub fn remove_app(&mut self, app_id: &str) {
        self.grants.remove(app_id);
        self.until.remove(app_id);
        self.uses.remove(app_id);
        self.clear_once_for(app_id);
        self.access.retain(|r| r.app_id != app_id);
    }

    /// Records that an app actually exercised a capability.
    pub fn record_access(&mut self, app_id: &str, perm: Permission, at: u64) {
        *self
            .uses
            .entry(app_id.to_string())
            .or_default()
            .entry(perm.as_str().to_string())
            .or_insert(0) += 1;
        // Collapse a burst: repeated use within a minute updates the last
        // record instead of filling the ring with near-identical rows.
        if let Some(last) = self.access.last_mut() {
            if last.app_id == app_id && last.perm == perm.as_str() && at.saturating_sub(last.at) < 60
            {
                last.at = at;
                return;
            }
        }
        self.access.push(AccessRecord {
            app_id: app_id.to_string(),
            perm: perm.as_str().to_string(),
            at,
        });
        if self.access.len() > MAX_ACCESS_RECORDS {
            let cut = self.access.len() - MAX_ACCESS_RECORDS;
            self.access.drain(..cut);
        }
    }

    /// When an app last used a capability, if ever.
    pub fn last_access(&self, app_id: &str, perm: Permission) -> Option<u64> {
        self.access
            .iter()
            .rev()
            .find(|r| r.app_id == app_id && r.perm == perm.as_str())
            .map(|r| r.at)
    }

    /// Recent uses, newest first.
    pub fn recent_access(&self, limit: usize) -> Vec<AccessRecord> {
        self.access.iter().rev().take(limit).cloned().collect()
    }

    pub fn effective(&self, manifest: &MiniAppManifest, perm: Permission) -> Effective {
        if !manifest.declares(perm) {
            return Effective::Undeclared;
        }
        // A one-time grant outranks Ask but never a stored Denied: saying
        // "just this once" must not resurrect something you turned off.
        let stored = self.state(&manifest.id, perm);
        // Session and timed grants outrank Ask but never a stored Denied:
        // "just this once" must not resurrect something you turned off.
        if stored != GrantState::Denied {
            if self.has_once(&manifest.id, perm) {
                return Effective::Granted;
            }
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if self.timed_until(&manifest.id, perm, now).is_some() {
                return Effective::Granted;
            }
        }
        match (stored, perm.tier()) {
            (GrantState::Granted, _) => Effective::Granted,
            (GrantState::Denied, _) => Effective::Denied,
            // Strict mode stops normal tiers auto-granting: everything has to
            // be allowed on purpose.
            (GrantState::Ask, Tier::Normal) if !self.strict => Effective::Granted,
            (GrantState::Ask, _) => Effective::NeedsPrompt,
        }
    }

    /// Whether the capability is usable right now (prompt-pending counts as no).
    pub fn is_granted(&self, manifest: &MiniAppManifest, perm: Permission) -> bool {
        self.effective(manifest, perm) == Effective::Granted
    }

    /// The capability names currently usable, in `Permission::ALL` order —
    /// what `host.capabilities()` reports inside the app's isolate.
    pub fn granted_caps(&self, manifest: &MiniAppManifest) -> Vec<String> {
        Permission::ALL
            .into_iter()
            .filter(|p| self.is_granted(manifest, *p))
            .map(|p| p.as_str().to_string())
            .collect()
    }

    /// Declared permissions with their stored states, in declaration order,
    /// for the App Info rows. Unknown declared ids are skipped (a newer
    /// bundle's permission this build doesn't know can't be granted anyway),
    /// and duplicates collapse so a malformed manifest can't overflow the
    /// fixed row budget.
    pub fn declared_states(&self, manifest: &MiniAppManifest) -> Vec<(Permission, GrantState)> {
        let mut seen = Vec::new();
        for perm in manifest.permissions.iter().filter_map(|s| Permission::from_str(s)) {
            if !seen.iter().any(|(p, _)| *p == perm) {
                seen.push((perm, self.state(&manifest.id, perm)));
            }
        }
        seen
    }

    /// Every app that declares `perm`, with what it nets out to — the data
    /// behind the per-permission view ("who can see my location?").
    pub fn apps_declaring(
        &self,
        registry: &crate::mini_apps::registry::AppRegistry,
        perm: Permission,
    ) -> Vec<(MiniAppManifest, Effective)> {
        let mut out: Vec<(MiniAppManifest, Effective)> = registry
            .iter()
            .filter(|m| m.declares(perm))
            .map(|m| (m.clone(), self.effective(m, perm)))
            .collect();
        // Allowed first, then the ones still asking, then blocked; name-sorted
        // inside each group so the list doesn't reshuffle as grants change.
        out.sort_by(|a, b| {
            let rank = |e: &Effective| match e {
                Effective::Granted => 0,
                Effective::NeedsPrompt => 1,
                Effective::Denied => 2,
                Effective::Undeclared => 3,
            };
            rank(&a.1)
                .cmp(&rank(&b.1))
                .then_with(|| a.0.name.to_lowercase().cmp(&b.0.name.to_lowercase()))
        });
        out
    }

    /// How many apps declare `perm`, and how many of those can use it now.
    pub fn permission_tally(
        &self,
        registry: &crate::mini_apps::registry::AppRegistry,
        perm: Permission,
    ) -> (usize, usize) {
        let apps = self.apps_declaring(registry, perm);
        let allowed = apps.iter().filter(|(_, e)| *e == Effective::Granted).count();
        (allowed, apps.len())
    }
}

std::thread_local! {
    /// app id -> currently granted capability names. Widgets that create
    /// Splash isolates (fullscreen host, home tiles, widget tiles) read this
    /// instead of threading `AppState` through every open/split path; `App`
    /// republishes it every event pass, so it can never go stale.
    static GRANT_SNAPSHOT: std::cell::RefCell<std::collections::HashMap<MiniAppId, Vec<String>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

impl PermissionStore {
    /// The full app -> granted-caps map for [`publish_snapshot`].
    pub fn snapshot(
        &self,
        registry: &crate::mini_apps::registry::AppRegistry,
    ) -> std::collections::HashMap<MiniAppId, Vec<String>> {
        registry
            .iter()
            .map(|m| (m.id.clone(), self.granted_caps(m)))
            .collect()
    }
}

pub fn publish_snapshot(map: std::collections::HashMap<MiniAppId, Vec<String>>) {
    GRANT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
}

/// The capability names currently granted to an app, per the last published
/// snapshot. Empty for unknown apps: deny-by-default extends to any isolate
/// created before the first publish.
pub fn snapshot_grants_for(app_id: &str) -> Vec<String> {
    GRANT_SNAPSHOT.with(|s| s.borrow().get(app_id).cloned().unwrap_or_default())
}

/// The whole-jail byte cap for an app: the storage default unless it holds
/// `storage-large`. `None` means "leave the default alone".
pub fn storage_quota_for(caps: &[String]) -> Option<u64> {
    caps.iter()
        .any(|c| c == Permission::StorageLarge.as_str())
        .then_some(LARGE_JAIL_BYTES)
}

/// What `storage-large` buys: 4x the standard jail. Big enough to matter for
/// an app that keeps real content, small enough that a runaway app still
/// hits a wall.
pub const LARGE_JAIL_BYTES: u64 = 64 * 1024 * 1024;

/// Whether an app may keep home-screen isolates running while it is not the
/// app on screen. Apps that never declare `background` are unaffected — the
/// switch only exists for those that asked for it.
pub fn may_run_in_background(manifest: &MiniAppManifest, caps: &[String]) -> bool {
    !manifest.declares(Permission::Background)
        || caps.iter().any(|c| c == Permission::Background.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(perms: &[&str]) -> MiniAppManifest {
        MiniAppManifest {
            id: "t".into(),
            name: "T".into(),
            icon: "t".into(),
            tint: 0,
            source: String::new(),
            allow_net: false,
            permissions: perms.iter().map(|s| s.to_string()).collect(),
            permission_reasons: Default::default(),
            builtin: false,
            widget: None,
            shortcuts: vec![],
        }
    }

    #[test]
    fn undeclared_is_never_grantable() {
        let mut store = PermissionStore::default();
        let m = manifest(&[]);
        assert_eq!(store.effective(&m, Permission::Network), Effective::Undeclared);
        // Even a (stray) stored grant can't override a missing declaration.
        store.set("t", Permission::Network, GrantState::Granted);
        assert_eq!(store.effective(&m, Permission::Network), Effective::Undeclared);
        assert!(store.granted_caps(&m).is_empty());
    }

    #[test]
    fn tiers_default_correctly_and_answers_stick() {
        let mut store = PermissionStore::default();
        let m = manifest(&["network", "open-url"]);
        // Runtime tier prompts; normal tier auto-grants.
        assert_eq!(store.effective(&m, Permission::Network), Effective::NeedsPrompt);
        assert_eq!(store.effective(&m, Permission::OpenUrl), Effective::Granted);
        store.set("t", Permission::Network, GrantState::Granted);
        assert_eq!(store.effective(&m, Permission::Network), Effective::Granted);
        assert_eq!(store.granted_caps(&m), vec!["network", "open-url"]);
        // The user can shut off a normal-tier permission too.
        store.set("t", Permission::OpenUrl, GrantState::Denied);
        assert_eq!(store.effective(&m, Permission::OpenUrl), Effective::Denied);
        assert_eq!(store.granted_caps(&m), vec!["network"]);
    }

    #[test]
    fn normalize_translates_legacy_allow_net_both_ways() {
        let mut m = manifest(&[]);
        m.allow_net = true;
        m.normalize_permissions();
        assert!(m.declares(Permission::Network));
        assert!(m.allow_net);

        let mut m2 = manifest(&["network"]);
        m2.allow_net = false;
        m2.normalize_permissions();
        assert!(m2.allow_net, "declaration backfills the legacy flag");
    }

    #[test]
    fn uninstall_resets_to_ask() {
        let mut store = PermissionStore::default();
        let m = manifest(&["location"]);
        store.set("t", Permission::Location, GrantState::Granted);
        assert!(store.is_granted(&m, Permission::Location));
        store.remove_app("t");
        assert_eq!(store.effective(&m, Permission::Location), Effective::NeedsPrompt);
    }


    /// Strict mode is the answer to "an imported app got open-url for free":
    /// with it on, even a normal tier has to be allowed on purpose.
    #[test]
    fn strict_mode_stops_normal_tiers_auto_granting() {
        let mut store = PermissionStore::default();
        let m = manifest(&["open-url"]);
        assert_eq!(store.effective(&m, Permission::OpenUrl), Effective::Granted);
        store.set_strict(true);
        assert_eq!(store.effective(&m, Permission::OpenUrl), Effective::NeedsPrompt);
        // An explicit answer still wins over the default either way.
        store.set("t", Permission::OpenUrl, GrantState::Granted);
        assert_eq!(store.effective(&m, Permission::OpenUrl), Effective::Granted);
    }

    /// A timed grant works until its clock runs out, then simply stops —
    /// without turning into a stored "denied" the user never chose.
    #[test]
    fn timed_grants_expire_on_their_own() {
        let mut store = PermissionStore::default();
        let m = manifest(&["location"]);
        let now = 1_000_000;
        store.grant_until("t", Permission::Location, now + 3600);
        assert!(store.timed_until("t", Permission::Location, now).is_some());
        let expired = store.expire_timed(now + 10);
        assert!(expired.is_empty(), "not due yet");
        let expired = store.expire_timed(now + 3601);
        assert_eq!(expired, vec![("t".to_string(), Permission::Location)]);
        assert!(store.timed_until("t", Permission::Location, now + 3601).is_none());
        assert_eq!(store.state("t", Permission::Location), GrantState::Ask);
    }

    /// Bulk actions: one tap to shut an app out, one to start over.
    #[test]
    fn block_all_and_reset_all() {
        let mut store = PermissionStore::default();
        let m = manifest(&["network", "open-url"]);
        store.block_all(&m);
        assert_eq!(store.effective(&m, Permission::Network), Effective::Denied);
        assert_eq!(store.effective(&m, Permission::OpenUrl), Effective::Denied);
        store.reset_all();
        assert_eq!(store.effective(&m, Permission::Network), Effective::NeedsPrompt);
        assert_eq!(store.effective(&m, Permission::OpenUrl), Effective::Granted);
    }

    /// `background` only constrains apps that asked for it; everything else
    /// keeps running exactly as before.
    #[test]
    fn background_gates_only_apps_that_declare_it() {
        let plain = manifest(&[]);
        assert!(may_run_in_background(&plain, &[]));
        let bg = manifest(&["background"]);
        assert!(!may_run_in_background(&bg, &[]));
        assert!(may_run_in_background(&bg, &["background".to_string()]));
    }

    /// Extra storage is a real quota, not a label.
    #[test]
    fn storage_quota_follows_the_grant() {
        assert_eq!(storage_quota_for(&[]), None);
        assert_eq!(
            storage_quota_for(&["storage-large".to_string()]),
            Some(LARGE_JAIL_BYTES)
        );
    }

    /// Uses are counted per capability, and an uninstall forgets them.
    #[test]
    fn use_counts_accumulate_and_reset_with_the_app() {
        let mut store = PermissionStore::default();
        store.record_access("t", Permission::Location, 100);
        store.record_access("t", Permission::Location, 500);
        assert_eq!(store.use_count("t", Permission::Location), 2);
        store.remove_app("t");
        assert_eq!(store.use_count("t", Permission::Location), 0);
    }

    #[test]
    fn ids_round_trip() {
        for p in Permission::ALL {
            assert_eq!(Permission::from_str(p.as_str()), Some(p));
        }
        assert_eq!(Permission::from_str("nope"), None);
    }
}
