//! The paged home-screen grid: app icons and live widget tiles laid out on
//! swipeable pages, with long-press drag-to-rearrange and edit mode.
//!
//! This widget owns the entire home-area gesture state machine:
//! * horizontal drag/flick to switch pages (with rubber-banding at the ends),
//! * swipe-up to request the app drawer,
//! * long-press to lift an item, then drag to rearrange (with edge-of-screen
//!   page flips), or release-in-place to request a context menu,
//! * taps to open apps (and to hit remove badges in edit mode).
//!
//! Icons are deliberately passive (no finger handling of their own) so this
//! state machine is the single source of truth for what a gesture means.
//! Widget tiles do receive events (their Splash content is interactive), so
//! the pager watches those gestures via `capture_overload` and cancels the
//! tile's presses with a sweep lock once a pan or drag actually starts.

use std::collections::HashMap;

use makepad_widgets::{widget_tree::CxWidgetExt, *};

use crate::{
    app::AppState,
    mini_apps::registry::{
        GRID_COLS, GRID_ROWS, HomePage, LauncherLayout, MAX_PAGES, MiniAppId, PlacedItem,
        PlacedKind, WidgetInstanceId,
    },
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.HomePagerBase = #(HomePager::register_widget(vm))

    mod.widgets.HomePager = set_type_default() do mod.widgets.HomePagerBase{
        width: Fill
        height: Fill

        AppIcon := View{
            width: Fill
            height: Fill
            flow: Down
            spacing: 5
            align: Align{x: 0.5, y: 0.5}
            tile := LauncherIconTile{
                glyph := LauncherIconGlyph{}
            }
            name := LauncherIconName{}
            badge := RoundedView{
                visible: false
                width: 20
                height: 20
                align: Align{x: 0.5, y: 0.5}
                show_bg: true
                draw_bg +: {
                    color: #x2a3040ee
                    border_color: #xffffff44
                    border_size: 1.0
                    border_radius: 10.0
                }
                Label{
                    text: "×"
                    draw_text +: {
                        color: #xffffffdd
                        text_style: theme.font_bold{font_size: 12}
                    }
                }
            }
        }

        WidgetTile := RoundedView{
            width: Fill
            height: Fill
            flow: Overlay
            show_bg: true
            draw_bg +: {
                color: #xffffff10
                border_color: #xffffff20
                border_size: 1.0
                border_radius: 16.0
            }
            content := View{
                width: Fill
                height: Fill
                flow: Down
                padding: 10
                splash := Splash{
                    width: Fill
                    height: Fit
                }
            }
            badge := RoundedView{
                visible: false
                width: 20
                height: 20
                margin: Inset{top: 4, left: 4}
                align: Align{x: 0.5, y: 0.5}
                show_bg: true
                draw_bg +: {
                    color: #x2a3040ee
                    border_color: #xffffff44
                    border_size: 1.0
                    border_radius: 10.0
                }
                Label{
                    text: "×"
                    draw_text +: {
                        color: #xffffffdd
                        text_style: theme.font_bold{font_size: 12}
                    }
                }
            }
            resize_handle := View{
                visible: false
                width: Fill
                height: Fill
                align: Align{x: 1.0, y: 1.0}
                Label{
                    text: "◢"
                    margin: Inset{right: 5, bottom: 3}
                    draw_text +: {
                        color: #xffffff88
                        text_style: theme.font_regular{font_size: 12}
                    }
                }
            }
        }
    }
}

/// Finger movement below this (in points) still counts as a tap / stationary press.
const TAP_SLOP: f64 = 8.0;
/// How long a press must be held (secs) to count as a long press on desktop.
const LONG_PRESS_SECS: f64 = 0.5;
/// Upward movement past this many points requests the app drawer.
const SWIPE_UP_DISTANCE: f64 = 36.0;
/// Width of the left/right screen-edge zones that flip pages while dragging.
const EDGE_FLIP_ZONE: f64 = 28.0;
/// How long a dragged item must hover in an edge zone before the page flips.
const EDGE_FLIP_SECS: f64 = 0.55;
/// Resistance applied when panning past the first/last page.
const RUBBER_BAND_FACTOR: f64 = 0.35;
/// Size of the remove badge hit target in edit mode.
const BADGE_HIT_SIZE: f64 = 26.0;
/// Size of the widget resize-handle hit target in edit mode.
const RESIZE_HIT_SIZE: f64 = 30.0;

/// Stable identity of a placed home-screen item, used to key position animations
/// and child widgets across layout mutations.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ItemKey {
    App(MiniAppId),
    Widget(WidgetInstanceId),
}

impl PlacedItem {
    fn key(&self) -> ItemKey {
        match &self.kind {
            PlacedKind::App { id } => ItemKey::App(id.clone()),
            PlacedKind::Widget { instance, .. } => ItemKey::Widget(*instance),
        }
    }
}

/// Actions emitted by the HomePager for the app to handle.
#[derive(Clone, Debug, Default)]
pub enum HomePagerAction {
    /// An app icon was tapped; open the app, animating out from `from_rect`.
    OpenApp { app_id: MiniAppId, from_rect: Rect },
    /// The user swiped up on the home screen; open the app drawer.
    OpenDrawer,
    /// A long-press was released in place; show the context menu for this item.
    ShowContextMenu {
        app_id: MiniAppId,
        widget_instance: Option<WidgetInstanceId>,
        anchor: Rect,
    },
    /// The page position or page count changed (continuous during swipes).
    PageChanged { position: f64, count: usize },
    #[default]
    None,
}

/// What the current finger gesture means so far.
#[derive(Clone, Debug, Default)]
enum Gesture {
    #[default]
    Idle,
    /// Finger is down but the gesture's meaning isn't decided yet.
    Pending {
        start: Vec2d,
        item: Option<ItemKey>,
    },
    /// Horizontal page pan in progress.
    PanningX {
        last_x: f64,
        samples: Vec<(f64, f64)>,
    },
    /// A long press fired; the item is lifted but hasn't moved yet.
    Lifted { item: ItemKey, start: Vec2d },
    /// An item is being dragged to a new slot.
    DraggingItem,
    /// A widget tile is being resized via its corner handle.
    ResizingTile {
        instance: WidgetInstanceId,
        start_span: (u8, u8),
        start_abs: Vec2d,
    },
    /// The gesture was recognized and fully handled (e.g. swipe-up); ignore
    /// further movement until the finger lifts.
    Consumed,
}

/// The item currently being dragged, held out of the layout until dropped.
struct DragState {
    item: PlacedItem,
    /// Offset from the finger to the item's top-left, so it doesn't jump on grab.
    grab_offset: Vec2d,
    /// Current top-left position of the dragged item, in absolute coords.
    pos: Vec2d,
    /// Where the item came from, to return it on an invalid drop.
    from: (usize, u8, u8),
    /// The currently hovered drop target, if valid.
    target: Option<(usize, u8, u8)>,
    /// Which edge zone the finger is hovering in (-1 left, 1 right), if any.
    edge: Option<i8>,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum PageAnim {
    #[default]
    Idle,
    /// Finger-driven; page_pos follows the drag directly.
    Dragging,
    /// Animating toward `page_target` after release.
    Settling,
}

#[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
pub struct HomePager {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,
    #[rust]
    area: Area,

    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    /// Instantiated icon widgets, one per app currently placed on the home screen.
    #[rust]
    icons: HashMap<MiniAppId, WidgetRef>,
    /// Instantiated widget tiles, one per placed widget instance.
    #[rust]
    tiles: HashMap<WidgetInstanceId, WidgetRef>,

    /// Continuous page position: 0.0 = first page fully visible.
    #[rust]
    page_pos: f64,
    #[rust]
    page_target: f64,
    #[rust]
    page_anim: PageAnim,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    last_frame_time: f64,

    #[rust]
    gesture: Gesture,
    #[rust]
    long_press_timer: Timer,
    #[rust]
    edge_flip_timer: Timer,
    #[rust]
    drag: Option<DragState>,
    /// Animated on-screen positions of items, keyed by identity, in page-local
    /// coords (relative to the page's origin). Lerped toward layout targets.
    #[rust]
    anim_pos: HashMap<ItemKey, Vec2d>,
    /// Whether edit mode visuals (badges/handles) were applied to children.
    #[rust]
    edit_visuals_applied: bool,

    #[rust]
    last_rect: Rect,
    #[rust]
    last_reported_page: f64,
    #[rust]
    sweep_locked: bool,
}

impl ScriptHook for HomePager {
    fn on_before_apply(
        &mut self,
        _vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        _value: ScriptValue,
    ) {
        if apply.is_reload() {
            self.templates.clear();
        }
    }

    fn on_after_apply(
        &mut self,
        vm: &mut ScriptVm,
        apply: &Apply,
        _scope: &mut Scope,
        value: ScriptValue,
    ) {
        // Collect the AppIcon/WidgetTile templates from the object's vec children.
        if !apply.is_eval() {
            if let Some(obj) = value.as_object() {
                vm.vec_with(obj, |vm, vec| {
                    for kv in vec {
                        if let Some(id) = kv.key.as_id() {
                            if let Some(template_obj) = kv.value.as_object() {
                                self.templates
                                    .insert(id, vm.bx.heap.new_object_ref(template_obj));
                            }
                        }
                    }
                });
            }
        }
        vm.cx_mut().widget_tree_mark_dirty(self.uid);
    }
}

impl WidgetNode for HomePager {
    fn widget_uid(&self) -> WidgetUid {
        self.uid
    }

    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.walk
    }

    fn area(&self) -> Area {
        self.area
    }

    fn children(&self, visit: &mut dyn FnMut(LiveId, WidgetRef)) {
        for (id, child) in self.icons.iter() {
            visit(LiveId::from_str(id), child.clone());
        }
        for (instance, child) in self.tiles.iter() {
            visit(LiveId::from_str_num("wtile", *instance), child.clone());
        }
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.area.redraw(cx);
    }
}

/// Grid geometry for one draw/event pass, derived from the pager's rect.
#[derive(Clone, Copy)]
struct Geom {
    rect: Rect,
    cell: Vec2d,
}

impl Geom {
    /// The rect of a cell span on the given page, in absolute coords,
    /// offset by the current continuous page position.
    fn cell_rect(&self, page: usize, page_pos: f64, col: u8, row: u8, span: (u8, u8)) -> Rect {
        let page_x = self.rect.pos.x + (page as f64 - page_pos) * self.rect.size.x;
        Rect {
            pos: dvec2(
                page_x + col as f64 * self.cell.x,
                self.rect.pos.y + row as f64 * self.cell.y,
            ),
            size: dvec2(self.cell.x * span.0 as f64, self.cell.y * span.1 as f64),
        }
    }

    /// Which (col, row) cell contains the given absolute position on the
    /// currently-centered page, if any.
    fn cell_at(&self, page_pos: f64, abs: Vec2d) -> Option<(u8, u8)> {
        let page = page_pos.round();
        let page_x = self.rect.pos.x + (page - page_pos) * self.rect.size.x;
        let local = abs - dvec2(page_x, self.rect.pos.y);
        if local.x < 0.0 || local.y < 0.0 {
            return None;
        }
        let col = (local.x / self.cell.x) as i64;
        let row = (local.y / self.cell.y) as i64;
        if col < 0 || col >= GRID_COLS as i64 || row < 0 || row >= GRID_ROWS as i64 {
            return None;
        }
        Some((col as u8, row as u8))
    }
}

impl HomePager {
    fn geom(&self) -> Geom {
        Geom {
            rect: self.last_rect,
            cell: dvec2(
                self.last_rect.size.x / GRID_COLS as f64,
                self.last_rect.size.y / GRID_ROWS as f64,
            ),
        }
    }

    fn page_count(layout: &LauncherLayout) -> usize {
        layout.pages.len().max(1)
    }

    fn current_page(&self) -> usize {
        self.page_pos.round().max(0.0) as usize
    }

    /// The rect of the icon's visual tile (not the whole cell), used as the
    /// source rect for the open-app zoom animation.
    fn icon_tile_rect(cell: Rect) -> Rect {
        let tile = 56.0;
        Rect {
            pos: dvec2(
                cell.pos.x + (cell.size.x - tile) * 0.5,
                // The icon column centers tile+label; the tile sits a bit above center.
                cell.pos.y + (cell.size.y - tile) * 0.5 - 8.0,
            ),
            size: dvec2(tile, tile),
        }
    }

    fn start_next_frame(&mut self, cx: &mut Cx) {
        self.next_frame = cx.new_next_frame();
    }

    fn set_sweep_lock(&mut self, cx: &mut Cx, locked: bool) {
        if locked && !self.sweep_locked {
            cx.sweep_lock(self.area);
            self.sweep_locked = true;
        } else if !locked && self.sweep_locked {
            cx.sweep_unlock(self.area);
            self.sweep_locked = false;
        }
    }

    /// Finds which placed item (if any) is under the given absolute position
    /// on the current page.
    fn item_at(&self, layout: &LauncherLayout, abs: Vec2d) -> Option<(usize, usize)> {
        let geom = self.geom();
        let page = self.current_page();
        let (col, row) = geom.cell_at(self.page_pos, abs)?;
        let items = &layout.pages.get(page)?.items;
        items
            .iter()
            .position(|item| item.covers(col, row))
            .map(|idx| (page, idx))
    }

    /// Emits the PageChanged action if the position or count changed.
    fn report_page(&mut self, cx: &mut Cx, layout: &LauncherLayout) {
        if (self.page_pos - self.last_reported_page).abs() > 0.001 {
            self.last_reported_page = self.page_pos;
            cx.widget_action(
                self.uid,
                HomePagerAction::PageChanged {
                    position: self.page_pos,
                    count: Self::page_count(layout),
                },
            );
        }
    }

    /// Ensures a child icon widget exists and is configured for the given app.
    fn ensure_icon(&mut self, cx: &mut Cx, state: &AppState, app_id: &MiniAppId) -> Option<WidgetRef> {
        if let Some(icon) = self.icons.get(app_id) {
            return Some(icon.clone());
        }
        let template = self.templates.get(&live_id!(AppIcon))?;
        let template_value: ScriptValue = template.as_object().into();
        let icon = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        let manifest = state.registry.get(app_id)?;
        icon.label(cx, ids!(glyph)).set_text(cx, &manifest.icon);
        icon.label(cx, ids!(name)).set_text(cx, &manifest.name);
        let tint = tile_tint_color(manifest.tint);
        let mut tile = icon.widget(cx, ids!(tile));
        script_apply_eval!(cx, tile, {
            draw_bg +: { color: #(tint) }
        });
        cx.widget_tree_insert_child_deep(self.uid, LiveId::from_str(app_id), icon.clone());
        self.icons.insert(app_id.clone(), icon.clone());
        Some(icon)
    }

    /// Ensures a widget tile exists for the given placed widget instance,
    /// evaluating its Splash source on first creation.
    fn ensure_tile(
        &mut self,
        cx: &mut Cx,
        state: &AppState,
        instance: WidgetInstanceId,
        app_id: &MiniAppId,
    ) -> Option<WidgetRef> {
        if let Some(tile) = self.tiles.get(&instance) {
            return Some(tile.clone());
        }
        let template = self.templates.get(&live_id!(WidgetTile))?;
        let template_value: ScriptValue = template.as_object().into();
        let tile = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        cx.widget_tree_insert_child_deep(
            self.uid,
            LiveId::from_str_num("wtile", instance),
            tile.clone(),
        );
        if let Some(widget_source) = state
            .registry
            .get(app_id)
            .and_then(|m| m.widget.as_ref())
            .map(|w| w.source.clone())
        {
            tile.widget(cx, ids!(splash)).set_text(cx, &widget_source);
        }
        self.tiles.insert(instance, tile.clone());
        Some(tile)
    }

    /// Drops child widgets whose items are no longer on the home screen.
    fn prune_children(&mut self, cx: &mut Cx, layout: &LauncherLayout) {
        let mut live_apps = Vec::new();
        let mut live_widgets = Vec::new();
        for page in &layout.pages {
            for item in &page.items {
                match &item.kind {
                    PlacedKind::App { id } => live_apps.push(id.clone()),
                    PlacedKind::Widget { instance, .. } => live_widgets.push(*instance),
                }
            }
        }
        if let Some(drag) = &self.drag {
            match &drag.item.kind {
                PlacedKind::App { id } => live_apps.push(id.clone()),
                PlacedKind::Widget { instance, .. } => live_widgets.push(*instance),
            }
        }
        let before = self.icons.len() + self.tiles.len();
        self.icons.retain(|id, _| live_apps.contains(id));
        self.tiles.retain(|inst, _| live_widgets.contains(inst));
        self.anim_pos.retain(|key, _| match key {
            ItemKey::App(id) => live_apps.contains(id),
            ItemKey::Widget(inst) => live_widgets.contains(inst),
        });
        if before != self.icons.len() + self.tiles.len() {
            cx.widget_tree_mark_dirty(self.uid);
            // Dropped tiles may own glass overlays and Splash isolates; a full
            // repaint avoids stuck overlay draw lists.
            cx.redraw_all();
        }
    }

    /// Starts settling toward the given page.
    fn settle_to(&mut self, cx: &mut Cx, layout: &LauncherLayout, page: f64) {
        let max_page = (Self::page_count(layout) - 1) as f64;
        self.page_target = page.clamp(0.0, max_page);
        self.page_anim = PageAnim::Settling;
        self.last_frame_time = 0.0;
        self.start_next_frame(cx);
    }

    /// Removes trailing empty pages (always keeps at least one page).
    fn prune_empty_pages(layout: &mut LauncherLayout) {
        while layout.pages.len() > 1 && layout.pages.last().is_some_and(|p| p.items.is_empty()) {
            layout.pages.pop();
        }
    }

    /// Handles a completed drop of the dragged item.
    fn drop_dragged_item(&mut self, cx: &mut Cx, state: &mut AppState) {
        let Some(drag) = self.drag.take() else { return };
        let layout = &mut state.layout;
        let (span_cols, span_rows) = drag.item.span();

        let (page, col, row) = match drag.target {
            Some(target) => target,
            None => drag.from,
        };
        while layout.pages.len() <= page {
            layout.pages.push(HomePage::default());
        }

        // If an icon is dropped onto another icon, swap: the occupant moves to the
        // dragged item's original slot.
        let occupant = layout.pages[page]
            .items
            .iter()
            .position(|it| {
                let (icols, irows) = it.span();
                col < it.col + icols
                    && it.col < col + span_cols
                    && row < it.row + irows
                    && it.row < row + span_rows
            });
        if let Some(occ_idx) = occupant {
            let is_simple_swap = span_cols == 1
                && span_rows == 1
                && layout.pages[page].items[occ_idx].span() == (1, 1)
                && layout.pages[page].fits(drag.from.1, drag.from.2, 1, 1, Some(occ_idx))
                && page == drag.from.0;
            if is_simple_swap {
                layout.pages[page].items[occ_idx].col = drag.from.1;
                layout.pages[page].items[occ_idx].row = drag.from.2;
            } else {
                // Occupied and not swappable: return the dragged item home.
                let (fp, fc, fr) = drag.from;
                while layout.pages.len() <= fp {
                    layout.pages.push(HomePage::default());
                }
                layout.pages[fp].items.push(PlacedItem {
                    kind: drag.item.kind,
                    col: fc,
                    row: fr,
                });
                Self::prune_empty_pages(layout);
                state.layout_dirty = true;
                self.redraw(cx);
                return;
            }
        }

        layout.pages[page].items.push(PlacedItem {
            kind: drag.item.kind,
            col,
            row,
        });
        Self::prune_empty_pages(layout);
        state.layout_dirty = true;
        self.redraw(cx);
    }

    /// Computes the currently hovered drop target for the drag, if it's valid.
    fn update_drag_target(&mut self, layout: &LauncherLayout, abs: Vec2d) {
        let geom = self.geom();
        let page = self.current_page();
        let Some(drag) = &mut self.drag else { return };
        let (span_cols, span_rows) = drag.item.span();
        // Target the cell under the *center* of the dragged item's first cell,
        // so drops land where the item visually sits.
        let probe = drag.pos + geom.cell * 0.5;
        let target = geom.cell_at(self.page_pos, probe).and_then(|(col, row)| {
            let col = col.min(GRID_COLS.saturating_sub(span_cols));
            let row = row.min(GRID_ROWS.saturating_sub(span_rows));
            let empty_page = HomePage::default();
            let page_items = layout.pages.get(page).unwrap_or(&empty_page);
            if page_items.fits(col, row, span_cols, span_rows, None) {
                Some((page, col, row))
            } else if span_cols == 1 && span_rows == 1 {
                // Icons may drop onto another 1x1 icon (swap).
                let occ = page_items.items.iter().find(|it| it.covers(col, row));
                match occ {
                    Some(it) if it.span() == (1, 1) => Some((page, col, row)),
                    _ => None,
                }
            } else {
                None
            }
        });
        drag.target = target;

        // Edge zones flip pages while dragging.
        let rect = geom.rect;
        let edge = if abs.x < rect.pos.x + EDGE_FLIP_ZONE {
            Some(-1)
        } else if abs.x > rect.pos.x + rect.size.x - EDGE_FLIP_ZONE {
            Some(1)
        } else {
            None
        };
        drag.edge = edge;
    }

    /// Called when the edge-flip timer fires while dragging near a screen edge.
    fn do_edge_flip(&mut self, cx: &mut Cx, state: &mut AppState) {
        let Some(edge) = self.drag.as_ref().and_then(|d| d.edge) else {
            return;
        };
        let layout = &mut state.layout;
        let page = self.current_page();
        if edge < 0 && page > 0 {
            self.settle_to(cx, layout, page as f64 - 1.0);
        } else if edge > 0 {
            if page + 1 < layout.pages.len() {
                self.settle_to(cx, layout, page as f64 + 1.0);
            } else if page + 1 < MAX_PAGES
                && layout.pages.get(page).is_some_and(|p| !p.items.is_empty())
            {
                // Dragging past the last page creates a fresh one.
                layout.pages.push(HomePage::default());
                self.settle_to(cx, layout, page as f64 + 1.0);
            }
        }
        // Re-arm so holding at the edge keeps flipping.
        self.edge_flip_timer = cx.start_timeout(EDGE_FLIP_SECS);
    }

    /// Applies/removes edit-mode visuals (remove badges, resize handles) on children.
    fn sync_edit_visuals(&mut self, cx: &mut Cx, edit_mode: bool) {
        if self.edit_visuals_applied == edit_mode {
            return;
        }
        self.edit_visuals_applied = edit_mode;
        for icon in self.icons.values() {
            icon.widget(cx, ids!(badge)).set_visible(cx, edit_mode);
        }
        for tile in self.tiles.values() {
            tile.widget(cx, ids!(badge)).set_visible(cx, edit_mode);
            tile.widget(cx, ids!(resize_handle)).set_visible(cx, edit_mode);
        }
        self.redraw(cx);
    }

    /// Applies a resize drag to a placed widget: clamps the new span to the
    /// widget's minimum and the grid, and commits it if the space is free.
    /// Returns true if the span changed.
    fn resize_tile_to(
        &mut self,
        state: &mut AppState,
        page: usize,
        instance: WidgetInstanceId,
        start_span: (u8, u8),
        dcols: i32,
        drows: i32,
    ) -> bool {
        let Some(page_items) = state.layout.pages.get_mut(page) else {
            return false;
        };
        let Some(idx) = page_items.items.iter().position(
            |it| matches!(&it.kind, PlacedKind::Widget { instance: i, .. } if *i == instance),
        ) else {
            return false;
        };
        let min_span = match &page_items.items[idx].kind {
            PlacedKind::Widget { app_id, .. } => state
                .registry
                .get(app_id)
                .and_then(|m| m.widget.as_ref())
                .map(|w| w.min_span)
                .unwrap_or((1, 1)),
            PlacedKind::App { .. } => return false,
        };
        let (col, row) = (page_items.items[idx].col, page_items.items[idx].row);
        let new_cols =
            (start_span.0 as i32 + dcols).clamp(min_span.0 as i32, (GRID_COLS - col) as i32) as u8;
        let new_rows =
            (start_span.1 as i32 + drows).clamp(min_span.1 as i32, (GRID_ROWS - row) as i32) as u8;
        if !page_items.fits(col, row, new_cols, new_rows, Some(idx)) {
            return false;
        }
        if let PlacedKind::Widget { cols, rows, .. } = &mut page_items.items[idx].kind {
            if (*cols, *rows) != (new_cols, new_rows) {
                *cols = new_cols;
                *rows = new_rows;
                state.layout_dirty = true;
                return true;
            }
        }
        false
    }

    /// Removes the item at (page, idx) from the home screen.
    fn remove_item(&mut self, cx: &mut Cx, state: &mut AppState, page: usize, idx: usize) {
        if let Some(page_items) = state.layout.pages.get_mut(page) {
            if idx < page_items.items.len() {
                page_items.items.remove(idx);
                Self::prune_empty_pages(&mut state.layout);
                state.layout_dirty = true;
                self.prune_children(cx, &state.layout);
                self.redraw(cx);
            }
        }
    }
}

/// Converts a 0xRRGGBB tint into a translucent icon-tile fill color.
pub fn tile_tint_color(tint: u32) -> Vec4f {
    let r = ((tint >> 16) & 0xff) as f32 / 255.0;
    let g = ((tint >> 8) & 0xff) as f32 / 255.0;
    let b = (tint & 0xff) as f32 / 255.0;
    // Blend toward white a bit and keep it translucent so the backdrop shows through.
    Vec4f {
        x: r * 0.75 + 0.1,
        y: g * 0.75 + 0.1,
        z: b * 0.75 + 0.1,
        w: 0.42,
    }
}

impl Widget for HomePager {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Step page + item animations.
        if let Some(ne) = self.next_frame.is_event(event) {
            let dt = if self.last_frame_time == 0.0 {
                1.0 / 60.0
            } else {
                (ne.time - self.last_frame_time).clamp(0.0, 0.1)
            };
            self.last_frame_time = ne.time;
            let mut still_animating = false;

            if self.page_anim == PageAnim::Settling {
                let diff = self.page_target - self.page_pos;
                if diff.abs() < 0.0015 {
                    self.page_pos = self.page_target;
                    self.page_anim = PageAnim::Idle;
                } else {
                    self.page_pos += diff * (1.0 - (-dt * 14.0).exp());
                    still_animating = true;
                }
                self.redraw(cx);
            }

            // Item shuffle animation runs whenever an anim pos differs from its target;
            // targets are recomputed during draw, so just keep redrawing while dragging
            // or settling into place.
            if self.drag.is_some() {
                still_animating = true;
            }

            if still_animating {
                self.start_next_frame(cx);
            } else {
                self.last_frame_time = 0.0;
            }
            if let Some(state) = scope.data.get_mut::<AppState>() {
                let layout = state.layout.clone();
                self.report_page(cx, &layout);
            }
        }

        // Long-press timer: lift the pressed item.
        if self.long_press_timer.is_event(event).is_some() {
            if let Gesture::Pending { start, item: Some(item) } = self.gesture.clone() {
                self.gesture = Gesture::Lifted { item, start };
                self.redraw(cx);
            }
        }

        // Edge-flip timer while dragging.
        if self.edge_flip_timer.is_event(event).is_some() {
            if matches!(self.gesture, Gesture::DraggingItem) {
                if let Some(state) = scope.data.get_mut::<AppState>() {
                    self.do_edge_flip(cx, state);
                }
            }
        }

        // Forward events to children: visibility-gated events only go to items on
        // (or adjacent to) the visible pages; everything else goes to all children.
        if event.requires_visibility() {
            let visible_range = (self.page_pos - 1.2, self.page_pos + 1.2);
            if let Some(state) = scope.data.get::<AppState>() {
                let mut to_event = Vec::new();
                for (page_idx, page) in state.layout.pages.iter().enumerate() {
                    let p = page_idx as f64;
                    if p < visible_range.0 || p > visible_range.1 {
                        continue;
                    }
                    for item in &page.items {
                        match &item.kind {
                            PlacedKind::App { id } => {
                                if let Some(w) = self.icons.get(id) {
                                    to_event.push(w.clone());
                                }
                            }
                            PlacedKind::Widget { instance, .. } => {
                                if let Some(w) = self.tiles.get(instance) {
                                    to_event.push(w.clone());
                                }
                            }
                        }
                    }
                }
                for w in to_event {
                    w.handle_event(cx, event, scope);
                }
            }
        } else {
            let children: Vec<WidgetRef> = self
                .icons
                .values()
                .chain(self.tiles.values())
                .cloned()
                .collect();
            for w in children {
                w.handle_event(cx, event, scope);
            }
        }

        // Don't react to gestures when an overlay (mini-app, drawer, menu) is on
        // top; otherwise the pager, still live behind it, would steal taps and
        // spuriously open apps. Any in-progress gesture is also cancelled.
        if scope
            .data
            .get::<AppState>()
            .is_some_and(|s| !s.home_input_enabled)
        {
            if !matches!(self.gesture, Gesture::Idle) {
                cx.stop_timer(self.long_press_timer);
                cx.stop_timer(self.edge_flip_timer);
                self.set_sweep_lock(cx, false);
                self.gesture = Gesture::Idle;
            }
            return;
        }

        // The pager's own gesture handling.
        let hit = event.hits_with_options(
            cx,
            self.area,
            HitOptions::new()
                .with_capture_overload(true)
                .with_sweep_area(self.area),
        );
        let Some(state) = scope.data.get_mut::<AppState>() else {
            return;
        };

        match hit {
            Hit::FingerDown(fe) => {
                if !fe.device.is_primary_hit() {
                    return;
                }
                // Touching during a page settle catches the pages.
                if self.page_anim == PageAnim::Settling {
                    self.page_anim = PageAnim::Idle;
                }
                let item = self
                    .item_at(&state.layout, fe.abs)
                    .map(|(page, idx)| state.layout.pages[page].items[idx].key());

                if state.edit_mode {
                    // In edit mode, badges and resize handles take precedence,
                    // and items lift immediately without a long press.
                    if let Some((page, idx)) = self.item_at(&state.layout, fe.abs) {
                        let placed = state.layout.pages[page].items[idx].clone();
                        let geom = self.geom();
                        let cell = geom.cell_rect(
                            page,
                            self.page_pos,
                            placed.col,
                            placed.row,
                            placed.span(),
                        );
                        // Remove badge in the top-left corner?
                        let badge_center = match &placed.kind {
                            PlacedKind::App { .. } => Self::icon_tile_rect(cell).pos,
                            PlacedKind::Widget { .. } => cell.pos + dvec2(14.0, 14.0),
                        };
                        if (fe.abs - badge_center).length() < BADGE_HIT_SIZE {
                            self.remove_item(cx, state, page, idx);
                            self.gesture = Gesture::Consumed;
                            return;
                        }
                        // Resize handle in the bottom-right corner of widget tiles?
                        if let PlacedKind::Widget { instance, cols, rows, .. } = &placed.kind {
                            let corner = cell.pos + cell.size;
                            if (fe.abs - corner).length() < RESIZE_HIT_SIZE {
                                self.gesture = Gesture::ResizingTile {
                                    instance: *instance,
                                    start_span: (*cols, *rows),
                                    start_abs: fe.abs,
                                };
                                self.set_sweep_lock(cx, true);
                                return;
                            }
                        }
                        self.gesture = Gesture::Lifted {
                            item: placed.key(),
                            start: fe.abs,
                        };
                        return;
                    }
                    self.gesture = Gesture::Pending { start: fe.abs, item: None };
                    return;
                }

                self.gesture = Gesture::Pending { start: fe.abs, item };
                if matches!(&self.gesture, Gesture::Pending { item: Some(_), .. }) {
                    self.long_press_timer = cx.start_timeout(LONG_PRESS_SECS);
                }
            }

            Hit::FingerMove(fe) => {
                match self.gesture.clone() {
                    Gesture::Pending { start, .. } => {
                        let delta = fe.abs - start;
                        // A platform long-press (mobile) lifts without waiting for our timer.
                        if fe.has_long_press_occurred && delta.length() < TAP_SLOP {
                            if let Gesture::Pending { item: Some(item), .. } = self.gesture.clone() {
                                self.gesture = Gesture::Lifted { item, start };
                                self.redraw(cx);
                                return;
                            }
                        }
                        if delta.length() > TAP_SLOP {
                            cx.stop_timer(self.long_press_timer);
                            if delta.x.abs() > delta.y.abs() {
                                self.gesture = Gesture::PanningX {
                                    last_x: fe.abs.x,
                                    samples: vec![(fe.abs.x, fe.time)],
                                };
                                self.page_anim = PageAnim::Dragging;
                                self.set_sweep_lock(cx, true);
                            } else if delta.y < -TAP_SLOP {
                                // Upward: hand off to the app drawer. No sweep lock here:
                                // opening the drawer hides the home screen, so the pager
                                // may never see the finger-up that would release the lock,
                                // which would then wedge all later hits (e.g. an open
                                // mini-app's buttons).
                                if delta.y < -SWIPE_UP_DISTANCE {
                                    cx.widget_action(self.uid, HomePagerAction::OpenDrawer);
                                    self.gesture = Gesture::Consumed;
                                }
                            } else {
                                self.gesture = Gesture::Consumed;
                            }
                        }
                    }
                    Gesture::PanningX { last_x, mut samples } => {
                        let dx = fe.abs.x - last_x;
                        let width = self.last_rect.size.x.max(1.0);
                        let mut new_pos = self.page_pos - dx / width;
                        let max_page = (Self::page_count(&state.layout) - 1) as f64;
                        // Rubber-band past the ends.
                        if new_pos < 0.0 {
                            new_pos *= RUBBER_BAND_FACTOR;
                        } else if new_pos > max_page {
                            new_pos = max_page + (new_pos - max_page) * RUBBER_BAND_FACTOR;
                        }
                        self.page_pos = new_pos;
                        samples.push((fe.abs.x, fe.time));
                        if samples.len() > 5 {
                            samples.remove(0);
                        }
                        self.gesture = Gesture::PanningX {
                            last_x: fe.abs.x,
                            samples,
                        };
                        let layout = state.layout.clone();
                        self.report_page(cx, &layout);
                        self.redraw(cx);
                    }
                    Gesture::Lifted { item, start } => {
                        if (fe.abs - start).length() > TAP_SLOP - 2.0 {
                            // Begin the actual drag: pull the item out of the layout.
                            let mut found = None;
                            'outer: for (p, page) in state.layout.pages.iter().enumerate() {
                                for (i, it) in page.items.iter().enumerate() {
                                    if it.key() == item {
                                        found = Some((p, i));
                                        break 'outer;
                                    }
                                }
                            }
                            if let Some((p, i)) = found {
                                let placed = state.layout.pages[p].items.remove(i);
                                let geom = self.geom();
                                let cell = geom.cell_rect(
                                    p,
                                    self.page_pos,
                                    placed.col,
                                    placed.row,
                                    placed.span(),
                                );
                                if !state.edit_mode {
                                    state.edit_mode = true;
                                    self.sync_edit_visuals(cx, true);
                                }
                                self.drag = Some(DragState {
                                    from: (p, placed.col, placed.row),
                                    item: placed,
                                    grab_offset: fe.abs - cell.pos,
                                    pos: cell.pos,
                                    target: None,
                                    edge: None,
                                });
                                self.gesture = Gesture::DraggingItem;
                                self.set_sweep_lock(cx, true);
                                self.edge_flip_timer = cx.start_timeout(EDGE_FLIP_SECS);
                                self.start_next_frame(cx);
                                self.redraw(cx);
                            } else {
                                self.gesture = Gesture::Idle;
                            }
                        }
                    }
                    Gesture::DraggingItem => {
                        if let Some(drag) = &mut self.drag {
                            drag.pos = fe.abs - drag.grab_offset;
                        }
                        self.update_drag_target(&state.layout, fe.abs);
                        self.redraw(cx);
                    }
                    Gesture::ResizingTile { instance, start_span, start_abs } => {
                        let geom = self.geom();
                        let delta = fe.abs - start_abs;
                        let dcols = (delta.x / geom.cell.x).round() as i32;
                        let drows = (delta.y / geom.cell.y).round() as i32;
                        let page = self.current_page();
                        if self.resize_tile_to(state, page, instance, start_span, dcols, drows) {
                            self.redraw(cx);
                        }
                    }
                    _ => (),
                }
            }

            Hit::FingerUp(fe) => {
                cx.stop_timer(self.long_press_timer);
                cx.stop_timer(self.edge_flip_timer);
                self.set_sweep_lock(cx, false);
                match self.gesture.clone() {
                    Gesture::Pending { start, item } => {
                        let is_tap = (fe.abs - start).length() < TAP_SLOP
                            && !fe.has_long_press_occurred;
                        if is_tap {
                            if state.edit_mode {
                                // Tapping empty space exits edit mode.
                                if item.is_none() {
                                    state.edit_mode = false;
                                    self.sync_edit_visuals(cx, false);
                                }
                            } else if let Some(ItemKey::App(app_id)) = item {
                                // Find the icon's rect for the zoom-out animation.
                                if let Some((page, idx)) = self.item_at(&state.layout, fe.abs) {
                                    let placed = &state.layout.pages[page].items[idx];
                                    let geom = self.geom();
                                    let cell = geom.cell_rect(
                                        page,
                                        self.page_pos,
                                        placed.col,
                                        placed.row,
                                        placed.span(),
                                    );
                                    cx.widget_action(
                                        self.uid,
                                        HomePagerAction::OpenApp {
                                            app_id,
                                            from_rect: Self::icon_tile_rect(cell),
                                        },
                                    );
                                }
                            }
                        }
                    }
                    Gesture::PanningX { samples, .. } => {
                        // Velocity in pages/sec from the recent samples.
                        let velocity = if samples.len() >= 2 {
                            let (x0, t0) = samples[0];
                            let (x1, t1) = *samples.last().unwrap();
                            let dt = (t1 - t0).max(0.001);
                            -((x1 - x0) / dt) / self.last_rect.size.x.max(1.0)
                        } else {
                            0.0
                        };
                        let target = if velocity.abs() > 1.2 {
                            if velocity > 0.0 {
                                self.page_pos.floor() + 1.0
                            } else {
                                self.page_pos.ceil() - 1.0
                            }
                        } else {
                            self.page_pos.round()
                        };
                        let layout = state.layout.clone();
                        self.settle_to(cx, &layout, target);
                    }
                    Gesture::Lifted { item, .. } => {
                        // Long press released in place: show the context menu.
                        let mut found = None;
                        'outer: for (p, page) in state.layout.pages.iter().enumerate() {
                            for it in page.items.iter() {
                                if it.key() == item {
                                    found = Some((p, it.clone()));
                                    break 'outer;
                                }
                            }
                        }
                        if let Some((p, placed)) = found {
                            let geom = self.geom();
                            let cell = geom.cell_rect(
                                p,
                                self.page_pos,
                                placed.col,
                                placed.row,
                                placed.span(),
                            );
                            let (app_id, widget_instance) = match &placed.kind {
                                PlacedKind::App { id } => (id.clone(), None),
                                PlacedKind::Widget { app_id, instance, .. } => {
                                    (app_id.clone(), Some(*instance))
                                }
                            };
                            cx.widget_action(
                                self.uid,
                                HomePagerAction::ShowContextMenu {
                                    app_id,
                                    widget_instance,
                                    anchor: cell,
                                },
                            );
                        }
                        self.redraw(cx);
                    }
                    Gesture::DraggingItem => {
                        self.drop_dragged_item(cx, state);
                        let layout = state.layout.clone();
                        self.report_page(cx, &layout);
                    }
                    Gesture::ResizingTile { .. } | Gesture::Consumed | Gesture::Idle => (),
                }
                self.gesture = Gesture::Idle;
            }

            Hit::FingerHoverOver(fe) | Hit::FingerHoverIn(fe) => {
                if self.item_at(&state.layout, fe.abs).is_some() {
                    cx.set_cursor(MouseCursor::Hand);
                }
            }

            _ => (),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::flow_overlay());
        self.last_rect = cx.turtle().rect();

        let Some(state) = scope.data.get::<AppState>() else {
            cx.end_turtle_with_area(&mut self.area);
            return DrawStep::done();
        };
        let layout = state.layout.clone();
        let edit_mode = state.edit_mode;

        self.prune_children(cx, &layout);
        self.sync_edit_visuals(cx, edit_mode);

        let geom = self.geom();
        let mut any_anim = false;

        // Draw items on pages within one page of the current position.
        for (page_idx, page) in layout.pages.iter().enumerate() {
            let p = page_idx as f64;
            if (p - self.page_pos).abs() >= 1.0 {
                continue;
            }
            for item in &page.items {
                let key = item.key();
                let span = item.span();
                let target_rect =
                    geom.cell_rect(page_idx, self.page_pos, item.col, item.row, span);
                // Item positions animate in page-local space so page panning
                // doesn't fight the shuffle animation.
                let local_target = target_rect.pos
                    - dvec2(
                        geom.rect.pos.x + (p - self.page_pos) * geom.rect.size.x,
                        geom.rect.pos.y,
                    );
                let anim = self
                    .anim_pos
                    .entry(key.clone())
                    .or_insert(local_target);
                let diff = local_target - *anim;
                if diff.length() > 0.5 {
                    *anim += diff * 0.25;
                    any_anim = true;
                } else {
                    *anim = local_target;
                }
                let draw_pos = dvec2(
                    geom.rect.pos.x + (p - self.page_pos) * geom.rect.size.x + anim.x,
                    geom.rect.pos.y + anim.y,
                );

                let child_walk = Walk {
                    abs_pos: Some(draw_pos),
                    margin: Default::default(),
                    width: Size::Fixed(target_rect.size.x),
                    height: Size::Fixed(target_rect.size.y),
                    metrics: Default::default(),
                };
                let child = match &item.kind {
                    PlacedKind::App { id } => {
                        let state = scope.data.get::<AppState>().unwrap();
                        self.ensure_icon(cx, state, id)
                    }
                    PlacedKind::Widget { instance, app_id, .. } => {
                        let state = scope.data.get::<AppState>().unwrap();
                        self.ensure_tile(cx, state, *instance, app_id)
                    }
                };
                if let Some(child) = child {
                    child.draw_walk_all(cx, scope, child_walk);
                }
            }
        }

        // Draw the dragged item last so it floats above everything.
        if let Some(drag) = &self.drag {
            let span = drag.item.span();
            let size = dvec2(geom.cell.x * span.0 as f64, geom.cell.y * span.1 as f64);
            let pos = drag.pos;
            let child_walk = Walk {
                abs_pos: Some(pos),
                margin: Default::default(),
                width: Size::Fixed(size.x),
                height: Size::Fixed(size.y),
                metrics: Default::default(),
            };
            let widget = match &drag.item.kind {
                PlacedKind::App { id } => self.icons.get(id).cloned(),
                PlacedKind::Widget { instance, .. } => self.tiles.get(instance).cloned(),
            };
            if let Some(widget) = widget {
                widget.draw_walk_all(cx, scope, child_walk);
            }
        }

        if any_anim {
            self.start_next_frame(cx);
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl HomePagerRef {
    /// Returns the current continuous page position and page count.
    pub fn page_state(&self, layout: &LauncherLayout) -> (f64, usize) {
        if let Some(inner) = self.borrow() {
            (inner.page_pos, HomePager::page_count(layout))
        } else {
            (0.0, 1)
        }
    }

    /// Snaps the pager back to the first page (used by back navigation).
    pub fn go_to_first_page(&self, cx: &mut Cx, layout: &LauncherLayout) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.settle_to(cx, layout, 0.0);
        }
    }

    /// Whether the pager is on (or settling to) the first page.
    pub fn is_on_first_page(&self) -> bool {
        self.borrow().is_none_or(|inner| inner.page_pos < 0.5)
    }
}
