//! The registry of installed mini-apps: built-in manifests plus user-installed apps,
//! and the persistable home screen layout (icon/widget placements, recents).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Default number of icon columns on each home page.
pub const DEFAULT_GRID_COLS: u8 = 4;
/// Default number of icon rows on each home page.
pub const DEFAULT_GRID_ROWS: u8 = 6;
/// User-adjustable grid bounds (via the edit-mode layout steppers).
pub const MIN_GRID_COLS: u8 = 3;
pub const MAX_GRID_COLS: u8 = 5;
pub const MIN_GRID_ROWS: u8 = 4;
pub const MAX_GRID_ROWS: u8 = 8;
/// Maximum number of home pages.
pub const MAX_PAGES: usize = 8;

fn default_grid_cols() -> u8 {
    DEFAULT_GRID_COLS
}
fn default_grid_rows() -> u8 {
    DEFAULT_GRID_ROWS
}

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
    /// Quick-action shortcuts shown in the app's long-press menu (like Android's
    /// app shortcuts / iOS quick actions). Display-only in this demo: picking one
    /// opens the app.
    #[serde(default)]
    pub shortcuts: Vec<String>,
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
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlacedItem {
    pub kind: PlacedKind,
    /// Leftmost grid column this item occupies.
    pub col: u8,
    /// Topmost grid row this item occupies.
    pub row: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlacedKind {
    /// An app icon shortcut; occupies a single cell. `instance` is a per-placement
    /// unique id (like a widget's) so the SAME app can be placed multiple times —
    /// each duplicate icon is a distinct item. Defaulted for layouts saved before
    /// app icons had instances; `LauncherLayout::renumber_instances` fixes those up.
    App {
        id: MiniAppId,
        #[serde(default)]
        instance: WidgetInstanceId,
        /// Cells this placement spans. 1x1 is a plain icon; ANY larger span
        /// runs the real app live in that block of cells, with a title bar
        /// carrying expand/shrink. Defaulted for layouts saved before app
        /// icons could grow.
        #[serde(default = "one_cell")]
        cols: u8,
        #[serde(default = "one_cell")]
        rows: u8,
    },
    /// A live widget spanning `cols x rows` cells.
    Widget {
        instance: WidgetInstanceId,
        app_id: MiniAppId,
        cols: u8,
        rows: u8,
    },
}

/// serde default for a placement's span dimensions (an app icon is 1x1).
fn one_cell() -> u8 {
    1
}

impl PlacedItem {
    pub fn span(&self) -> (u8, u8) {
        match &self.kind {
            PlacedKind::App { cols, rows, .. } => (*cols, *rows),
            PlacedKind::Widget { cols, rows, .. } => (*cols, *rows),
        }
    }

    /// Whether this placement occupies real space. A zero in either axis
    /// makes an item that draws nothing and covers no cell, so it can never
    /// be tapped, selected or removed — a ghost holding a slot. The UI can't
    /// produce one (resize clamps to a floor of 1), but serde will read one
    /// straight out of a hand-edited or corrupt layout.json.
    pub fn has_valid_span(&self) -> bool {
        let (cols, rows) = self.span();
        cols > 0 && rows > 0
    }

    /// Whether this placement hosts a LIVE Splash isolate rather than a
    /// static icon: every widget, and any app grown past a single cell.
    pub fn is_live(&self) -> bool {
        match &self.kind {
            PlacedKind::App { cols, rows, .. } => *cols > 1 || *rows > 1,
            PlacedKind::Widget { .. } => true,
        }
    }

    /// Whether this item covers the given cell.
    pub fn covers(&self, col: u8, row: u8) -> bool {
        let (cols, rows) = self.span();
        col >= self.col && col < self.col + cols && row >= self.row && row < self.row + rows
    }

    pub fn app_id(&self) -> &MiniAppId {
        match &self.kind {
            PlacedKind::App { id, .. } => id,
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
    /// Whether a `cols x rows` item can be placed with its top-left at (col, row)
    /// on a `grid`-sized page, optionally ignoring one item (the dragged one).
    pub fn fits(
        &self,
        grid: (u8, u8),
        col: u8,
        row: u8,
        cols: u8,
        rows: u8,
        ignore: Option<usize>,
    ) -> bool {
        if col + cols > grid.0 || row + rows > grid.1 {
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
    pub fn first_fit(&self, grid: (u8, u8), cols: u8, rows: u8) -> Option<(u8, u8)> {
        for row in 0 ..= grid.1.saturating_sub(rows) {
            for col in 0 ..= grid.0.saturating_sub(cols) {
                if self.fits(grid, col, row, cols, rows, None) {
                    return Some((col, row));
                }
            }
        }
        None
    }
}

/// The persistable state of the launcher: home layout, recents, and user-installed apps.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LauncherLayout {
    pub pages: Vec<HomePage>,
    /// Grid columns per page (user-adjustable, MIN..=MAX_GRID_COLS).
    #[serde(default = "default_grid_cols")]
    pub cols: u8,
    /// Grid rows per page (user-adjustable, MIN..=MAX_GRID_ROWS).
    #[serde(default = "default_grid_rows")]
    pub rows: u8,
    /// App ids pinned to the bottom dock, shown on every page.
    #[serde(default)]
    pub dock: Vec<MiniAppId>,
    /// Unix timestamp (secs) of when each app was last opened, for "recents" sorting.
    #[serde(default)]
    pub recents: HashMap<MiniAppId, u64>,
    /// Apps installed by the user (built-ins are constructed in code, not persisted).
    #[serde(default)]
    pub user_apps: Vec<MiniAppManifest>,
    /// Ids of user apps that have been uninstalled (so we don't re-seed the samples).
    #[serde(default)]
    pub uninstalled_user_apps: Vec<MiniAppId>,
    /// Manifests of uninstalled apps that aren't in the store catalog — the
    /// ones the user made or imported.
    ///
    /// A catalog app can always be fetched back from the catalog, but a
    /// generated or imported app exists nowhere else: uninstalling it used to
    /// destroy the only copy, and a prompt you can't reproduce is real work
    /// gone. Keeping the manifest costs a few KB and makes uninstall
    /// reversible, which is what the App Store's "Get" is for.
    #[serde(default)]
    pub archived_user_apps: Vec<MiniAppManifest>,
    /// Monotonic counter for allocating `WidgetInstanceId`s.
    #[serde(default)]
    pub next_widget_instance: WidgetInstanceId,
}

impl Default for LauncherLayout {
    fn default() -> Self {
        Self {
            pages: Vec::new(),
            cols: DEFAULT_GRID_COLS,
            rows: DEFAULT_GRID_ROWS,
            dock: Vec::new(),
            recents: HashMap::new(),
            user_apps: Vec::new(),
            uninstalled_user_apps: Vec::new(),
            archived_user_apps: Vec::new(),
            next_widget_instance: 0,
        }
    }
}

impl LauncherLayout {
    /// Silently drops placements that can't be interacted with: unknown apps
    /// and zero-span ghosts. Called on every load, so a corrupt or
    /// hand-edited layout heals itself instead of haunting the grid.
    pub fn prune_unusable(&mut self, known: impl Fn(&MiniAppId) -> bool) {
        for page in &mut self.pages {
            page.items.retain(|it| known(it.app_id()) && it.has_valid_span());
        }
    }

    /// The current page grid size as (cols, rows).
    pub fn grid(&self) -> (u8, u8) {
        (
            self.cols.clamp(MIN_GRID_COLS, MAX_GRID_COLS),
            self.rows.clamp(MIN_GRID_ROWS, MAX_GRID_ROWS),
        )
    }

    /// After the grid shrinks, clamps widget spans to the new size and re-places
    /// any item that no longer fits (same page first, then later/new pages).
    pub fn clamp_items_to_grid(&mut self) {
        let grid = self.grid();
        for page in &mut self.pages {
            for it in &mut page.items {
                if let PlacedKind::Widget { cols, rows, .. } = &mut it.kind {
                    *cols = (*cols).min(grid.0);
                    *rows = (*rows).min(grid.1);
                }
            }
        }
        let mut displaced = Vec::new();
        for page in &mut self.pages {
            let mut kept = Vec::new();
            for it in page.items.drain(..) {
                let (c, r) = it.span();
                if it.col + c <= grid.0 && it.row + r <= grid.1 {
                    kept.push(it);
                } else {
                    displaced.push(it);
                }
            }
            page.items = kept;
        }
        'place: for mut it in displaced {
            let (c, r) = it.span();
            for i in 0 .. self.pages.len() {
                if let Some((col, row)) = self.pages[i].first_fit(grid, c, r) {
                    it.col = col;
                    it.row = row;
                    self.pages[i].items.push(it);
                    continue 'place;
                }
            }
            if self.pages.len() < MAX_PAGES {
                it.col = 0;
                it.row = 0;
                let mut page = HomePage::default();
                page.items.push(it);
                self.pages.push(page);
            }
        }
        self.prune_empty_pages();
    }

    /// Removes trailing empty pages, always keeping at least one.
    pub fn prune_empty_pages(&mut self) {
        while self.pages.len() > 1 && self.pages.last().is_some_and(|p| p.items.is_empty()) {
            self.pages.pop();
        }
    }

    /// Allocates a fresh per-placement instance id that isn't already in use by any
    /// placed item (app icon OR widget), keeping the monotonic counter ahead of
    /// every id currently placed. Shared by app icons and widgets so duplicates of
    /// either never collide.
    pub fn alloc_instance(&mut self) -> WidgetInstanceId {
        let max_used = self
            .pages
            .iter()
            .flat_map(|p| &p.items)
            .map(|it| match &it.kind {
                PlacedKind::Widget { instance, .. } => *instance,
                PlacedKind::App { instance, .. } => *instance,
            })
            .max()
            .unwrap_or(0);
        let id = self.next_widget_instance.max(max_used + 1).max(1);
        self.next_widget_instance = id + 1;
        id
    }

    /// Assigns a fresh unique instance id to every placed item (apps and widgets),
    /// so duplicate placements never share a key. Used to migrate layouts saved
    /// before app icons carried instances (their `instance` defaults to 0 and would
    /// otherwise all collide). Instance values need not be stable across runs —
    /// nothing cross-session keys off them.
    pub fn renumber_instances(&mut self) {
        let mut n: WidgetInstanceId = 0;
        for page in &mut self.pages {
            for it in &mut page.items {
                n += 1;
                match &mut it.kind {
                    PlacedKind::App { instance, .. } => *instance = n,
                    PlacedKind::Widget { instance, .. } => *instance = n,
                }
            }
        }
        self.next_widget_instance = n + 1;
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

#[cfg(test)]
mod span_tests {
    use super::*;

    /// A zero span can't come from the UI, but it CAN come off disk — serde
    /// reads `cols: 0` without complaint. It must not survive the load.
    #[test]
    fn zero_span_placements_are_dropped_on_load() {
        let json = r#"{
            "pages": [{"items": [
                {"kind": {"App": {"id": "a", "instance": 1, "cols": 0, "rows": 1}}, "col": 0, "row": 0},
                {"kind": {"Widget": {"instance": 2, "app_id": "a", "cols": 2, "rows": 0}}, "col": 1, "row": 0},
                {"kind": {"App": {"id": "a", "instance": 3, "cols": 1, "rows": 1}}, "col": 2, "row": 0}
            ]}],
            "dock": [], "user_apps": [], "archived_user_apps": []
        }"#;
        let mut layout: LauncherLayout = serde_json::from_str(json).expect("parse");
        assert_eq!(layout.pages[0].items.len(), 3, "all three deserialize");
        layout.prune_unusable(|_| true);
        let kept = &layout.pages[0].items;
        assert_eq!(kept.len(), 1, "both zero-span ghosts dropped, got {kept:?}");
        assert!(kept[0].has_valid_span());
    }

    /// An app icon saved before spans existed must still load as 1x1.
    #[test]
    fn legacy_app_placement_defaults_to_one_cell() {
        let json = r#"{"kind": {"App": {"id": "a", "instance": 1}}, "col": 0, "row": 0}"#;
        let item: PlacedItem = serde_json::from_str(json).expect("parse");
        assert_eq!(item.span(), (1, 1));
        assert!(item.has_valid_span());
    }
}
