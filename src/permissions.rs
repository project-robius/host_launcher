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
}

/// Runtime permissions prompt the user on first use; normal ones auto-grant
/// on declaration (both stay user-revocable at any time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Normal,
    Runtime,
}

impl Permission {
    pub const ALL: [Permission; 10] = [
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
            | Permission::Auth => Tier::Normal,
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

/// All grants, keyed app id -> permission id. Owned by `AppState`, persisted
/// whole-file on every change (it is tiny, and a lost file just re-asks).
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct PermissionStore {
    grants: BTreeMap<MiniAppId, BTreeMap<String, GrantState>>,
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
        self.grants
            .entry(app_id.to_string())
            .or_default()
            .insert(perm.as_str().to_string(), state);
    }

    /// Forget an app entirely (uninstall). A reinstall starts from Ask.
    pub fn remove_app(&mut self, app_id: &str) {
        self.grants.remove(app_id);
    }

    pub fn effective(&self, manifest: &MiniAppManifest, perm: Permission) -> Effective {
        if !manifest.declares(perm) {
            return Effective::Undeclared;
        }
        match (self.state(&manifest.id, perm), perm.tier()) {
            (GrantState::Granted, _) => Effective::Granted,
            (GrantState::Denied, _) => Effective::Denied,
            (GrantState::Ask, Tier::Normal) => Effective::Granted,
            (GrantState::Ask, Tier::Runtime) => Effective::NeedsPrompt,
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
    /// bundle's permission this build doesn't know can't be granted anyway).
    pub fn declared_states(&self, manifest: &MiniAppManifest) -> Vec<(Permission, GrantState)> {
        manifest
            .permissions
            .iter()
            .filter_map(|s| Permission::from_str(s))
            .map(|p| (p, self.state(&manifest.id, p)))
            .collect()
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

    #[test]
    fn ids_round_trip() {
        for p in Permission::ALL {
            assert_eq!(Permission::from_str(p.as_str()), Some(p));
        }
        assert_eq!(Permission::from_str("nope"), None);
    }
}
