//! The registry of installed mini-apps: built-in manifests plus user-installed apps,
//! and the persistable home screen layout (icon/widget placements, recents).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Number of icon columns on each home page.
pub const GRID_COLS: u8 = 4;
/// Number of icon rows on each home page.
pub const GRID_ROWS: u8 = 6;
/// Maximum number of home pages.
pub const MAX_PAGES: usize = 8;

/// A stable identifier for an installed mini-app, e.g. `"weather"`.
pub type MiniAppId = String;

/// A unique instance id for a placed home-screen widget.
/// One app's widget can be placed multiple times, so placements get their own id.
pub type WidgetInstanceId = u64;

/// Everything the launcher knows about one installed mini-app.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MiniAppManifest {
    pub id: MiniAppId,
    /// Display name shown under the icon and in the drawer.
    pub name: String,
    /// Emoji drawn as the icon glyph (rendered via the built-in emoji font).
    pub icon: String,
    /// Tint color of the icon tile, as 0xRRGGBB.
    pub tint: u32,
    /// The Splash source code of the app itself.
    pub source: String,
    /// Whether this app's Splash VM is allowed to use the network sandbox.
    pub allow_net: bool,
    /// Pre-installed apps cannot be uninstalled.
    pub builtin: bool,
    /// The home-screen widget this app provides, if any.
    pub widget: Option<WidgetManifest>,
}

/// A home-screen widget provided by a mini-app: a separate, smaller Splash script
/// that runs continuously in a resizable tile on the home screen.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WidgetManifest {
    /// The Splash source code of the widget form of the app.
    pub source: String,
    /// Default size in grid cells (cols, rows).
    pub default_span: (u8, u8),
    /// Minimum size in grid cells.
    pub min_span: (u8, u8),
}

/// One item placed on a home page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedItem {
    pub kind: PlacedKind,
    /// Leftmost grid column this item occupies.
    pub col: u8,
    /// Topmost grid row this item occupies.
    pub row: u8,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlacedKind {
    /// An app icon shortcut; occupies a single cell.
    App { id: MiniAppId },
    /// A live widget spanning `cols x rows` cells.
    Widget {
        instance: WidgetInstanceId,
        app_id: MiniAppId,
        cols: u8,
        rows: u8,
    },
}

impl PlacedItem {
    pub fn span(&self) -> (u8, u8) {
        match &self.kind {
            PlacedKind::App { .. } => (1, 1),
            PlacedKind::Widget { cols, rows, .. } => (*cols, *rows),
        }
    }

    /// Whether this item covers the given cell.
    pub fn covers(&self, col: u8, row: u8) -> bool {
        let (cols, rows) = self.span();
        col >= self.col && col < self.col + cols && row >= self.row && row < self.row + rows
    }

    pub fn app_id(&self) -> &MiniAppId {
        match &self.kind {
            PlacedKind::App { id } => id,
            PlacedKind::Widget { app_id, .. } => app_id,
        }
    }
}

/// One home page's worth of placed items.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HomePage {
    pub items: Vec<PlacedItem>,
}

impl HomePage {
    /// Whether a `cols x rows` item can be placed with its top-left at (col, row),
    /// optionally ignoring one item (the one currently being dragged).
    pub fn fits(
        &self,
        col: u8,
        row: u8,
        cols: u8,
        rows: u8,
        ignore: Option<usize>,
    ) -> bool {
        if col + cols > GRID_COLS || row + rows > GRID_ROWS {
            return false;
        }
        for (i, item) in self.items.iter().enumerate() {
            if Some(i) == ignore {
                continue;
            }
            let (icols, irows) = item.span();
            let overlap_x = col < item.col + icols && item.col < col + cols;
            let overlap_y = row < item.row + irows && item.row < row + rows;
            if overlap_x && overlap_y {
                return false;
            }
        }
        true
    }

    /// Finds the first free cell that fits a `cols x rows` item, scanning row-major.
    pub fn first_fit(&self, cols: u8, rows: u8) -> Option<(u8, u8)> {
        for row in 0 ..= GRID_ROWS.saturating_sub(rows) {
            for col in 0 ..= GRID_COLS.saturating_sub(cols) {
                if self.fits(col, row, cols, rows, None) {
                    return Some((col, row));
                }
            }
        }
        None
    }
}

/// The persistable state of the launcher: home layout, recents, and user-installed apps.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LauncherLayout {
    pub pages: Vec<HomePage>,
    /// Unix timestamp (secs) of when each app was last opened, for "recents" sorting.
    #[serde(default)]
    pub recents: HashMap<MiniAppId, u64>,
    /// Apps installed by the user (built-ins are constructed in code, not persisted).
    #[serde(default)]
    pub user_apps: Vec<MiniAppManifest>,
    /// Ids of user apps that have been uninstalled (so we don't re-seed the samples).
    #[serde(default)]
    pub uninstalled_user_apps: Vec<MiniAppId>,
    /// Monotonic counter for allocating `WidgetInstanceId`s.
    #[serde(default)]
    pub next_widget_instance: WidgetInstanceId,
}

impl LauncherLayout {
    /// Removes trailing empty pages, always keeping at least one.
    pub fn prune_empty_pages(&mut self) {
        while self.pages.len() > 1 && self.pages.last().is_some_and(|p| p.items.is_empty()) {
            self.pages.pop();
        }
    }

    /// Allocates a fresh widget-instance id that isn't already placed, keeping the
    /// monotonic counter ahead of every id currently in use.
    pub fn alloc_widget_instance(&mut self) -> WidgetInstanceId {
        let max_used = self
            .pages
            .iter()
            .flat_map(|p| &p.items)
            .filter_map(|it| match &it.kind {
                PlacedKind::Widget { instance, .. } => Some(*instance),
                PlacedKind::App { .. } => None,
            })
            .max()
            .unwrap_or(0);
        let id = self.next_widget_instance.max(max_used + 1).max(1);
        self.next_widget_instance = id + 1;
        id
    }

    /// Removes every placement matching `pred` across all pages, prunes emptied
    /// trailing pages, and reports whether anything was removed.
    pub fn remove_items(&mut self, mut pred: impl FnMut(&PlacedItem) -> bool) -> bool {
        let mut removed = false;
        for page in &mut self.pages {
            let before = page.items.len();
            page.items.retain(|it| !pred(it));
            removed |= page.items.len() != before;
        }
        if removed {
            self.prune_empty_pages();
        }
        removed
    }
}

/// The full app registry: every installed app, in a stable order.
#[derive(Default)]
pub struct AppRegistry {
    apps: Vec<MiniAppManifest>,
    index: HashMap<MiniAppId, usize>,
}

impl AppRegistry {
    pub fn new(apps: Vec<MiniAppManifest>) -> Self {
        let mut registry = Self::default();
        for app in apps {
            registry.insert(app);
        }
        registry
    }

    pub fn insert(&mut self, app: MiniAppManifest) {
        if let Some(&i) = self.index.get(&app.id) {
            self.apps[i] = app;
        } else {
            self.index.insert(app.id.clone(), self.apps.len());
            self.apps.push(app);
        }
    }

    pub fn remove(&mut self, id: &str) -> Option<MiniAppManifest> {
        let i = self.index.remove(id)?;
        let app = self.apps.remove(i);
        // Reindex everything after the removed entry.
        for (j, a) in self.apps.iter().enumerate().skip(i) {
            self.index.insert(a.id.clone(), j);
        }
        Some(app)
    }

    pub fn get(&self, id: &str) -> Option<&MiniAppManifest> {
        self.index.get(id).map(|&i| &self.apps[i])
    }

    pub fn contains(&self, id: &str) -> bool {
        self.index.contains_key(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &MiniAppManifest> {
        self.apps.iter()
    }

}
