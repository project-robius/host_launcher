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
    launcher::notif_badge::NotifBadgeWidgetRefExt,
    mini_apps::registry::{
        HomePage, LauncherLayout, MAX_PAGES, MiniAppId, PlacedItem, PlacedKind, WidgetInstanceId,
    },
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // Rounded outline marking the cell a dragged item would drop into.
    set_type_default() do #(DrawTargetCell::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn(){
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(2.5, 2.5, self.rect_size.x - 5.0, self.rect_size.y - 5.0, 14.0)
            sdf.stroke(vec4(1.0, 1.0, 1.0, 0.4), 1.5)
            return sdf.result
        }
    }

    // Soft shadow drawn under a lifted (dragged) item.
    set_type_default() do #(DrawDragShadow::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn(){
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.blur = 8.0
            sdf.box(8.0, 8.0, self.rect_size.x - 16.0, self.rect_size.y - 16.0, 16.0)
            sdf.fill(vec4(0.0, 0.0, 0.0, 0.35))
            return sdf.result
        }
    }

    // The resize grip on a widget tile's bottom-right corner in edit mode: a
    // glassy disc with a two-headed diagonal arrow (drawn in SDF because the
    // theme font has no resize-arrow glyph). Sized for a 22x22 quad.
    set_type_default() do #(DrawResizeGrip::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn(){
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let c = self.rect_size * 0.5
            sdf.circle(c.x, c.y, c.x - 1.0)
            sdf.fill(vec4(0.102, 0.141, 0.212, 0.88))
            sdf.circle(c.x, c.y, c.x - 1.0)
            sdf.stroke(vec4(1.0, 1.0, 1.0, 0.7), 1.0)
            // Diagonal double-headed arrow (top-right to bottom-left).
            sdf.move_to(8.0, 14.0)
            sdf.line_to(14.0, 8.0)
            sdf.move_to(14.0, 11.0)
            sdf.line_to(14.0, 8.0)
            sdf.line_to(11.0, 8.0)
            sdf.move_to(8.0, 11.0)
            sdf.line_to(8.0, 14.0)
            sdf.line_to(11.0, 14.0)
            sdf.stroke(vec4(1.0, 1.0, 1.0, 1.0), 1.1)
            return sdf.result
        }
    }

    mod.widgets.LauncherGripBase = #(LauncherGrip::register_widget(vm))

    // The widget resize grip as a real child widget: glass.Card tiles render a
    // lens overlay above pager-drawn quads, so the grip must be inside the tile.
    mod.widgets.LauncherGrip = set_type_default() do mod.widgets.LauncherGripBase{
        width: 22
        height: 22
    }

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
            // Don't clip (cut off) the notification badge overhanging the tile.
            clip_x: false, clip_y: false
            tile := LauncherIconTile{
                flow: Overlay
                clip_x: false, clip_y: false
                View{
                    width: Fill
                    height: Fill
                    align: Align{x: 0.5, y: 0.5}
                    glyph := LauncherIconGlyph{}
                }
                // Pin the remove badge to the tile's top-left corner (the tile
                // itself centers overlay children, so it needs its own holder).
                View{
                    width: Fill
                    height: Fill
                    align: Align{x: 0.0, y: 0.0}
                    clip_x: false, clip_y: false
                    badge := LauncherRemoveBadge{
                        margin: Inset{top: -8, left: -8}
                    }
                }
                // Notification count on the top-right corner, iOS-style.
                View{
                    width: Fill
                    height: Fill
                    align: Align{x: 1.0, y: 0.0}
                    clip_x: false, clip_y: false
                    notif := NotifBadge{
                        margin: Inset{top: -5, right: -7}
                    }
                }
            }
            name := LauncherIconName{}
        }

        // A frosted glass card (like aichat's message bubbles): refracts the
        // vector backdrop at its edges and tints it cool blue for contrast.
        WidgetTile := glass.Card{
            width: Fill
            height: Fill
            flow: Overlay
            padding: 0
            clip_x: false
            clip_y: false
            draw_bg +: {
                corner_radius: 16.0
                tint_color: #x6fa6ff
                tint_alpha: 0.14
                lensing_effect: 0.6
                border_alpha: 0.55
                shadow_radius: 9.0
                shadow_offset: vec2(0.0, 3.0)
            }
            content := View{
                width: Fill
                height: Fill
                flow: Down
                padding: 10
                align: Align{x: 0.5, y: 0.5}
                splash := Splash{
                    width: Fill
                    height: Fit
                }
            }
            // Pin the remove badge to the tile's top-left corner.
            View{
                width: Fill
                height: Fill
                align: Align{x: 0.0, y: 0.0}
                clip_x: false, clip_y: false
                badge := LauncherRemoveBadge{
                    margin: Inset{top: -8, left: -8}
                }
            }
            // The resize grip on the bottom-right corner (SDF-drawn: the theme
            // font has no resize-arrow glyph).
            View{
                width: Fill
                height: Fill
                align: Align{x: 1.0, y: 1.0}
                padding: Inset{right: 6, bottom: 6}
                grip := mod.widgets.LauncherGrip{
                    visible: false
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
/// Inset applied to multi-cell widget tiles so neighbours have breathing room.
const WIDGET_GAP: f64 = 6.0;
/// Padding the WidgetTile template puts around its Splash content (keep in sync
/// with the `content` view's `padding` in the DSL below).
const TILE_CONTENT_PAD: f64 = 10.0;

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawTargetCell {
    #[deref]
    draw_super: DrawQuad,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawDragShadow {
    #[deref]
    draw_super: DrawQuad,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawResizeGrip {
    #[deref]
    draw_super: DrawQuad,
}

/// The widget-tile resize grip: an empty view that paints the SDF grip disc
/// over its own rect once drawn.
#[derive(Script, ScriptHook, Widget)]
pub struct LauncherGrip {
    #[deref]
    view: View,
    #[live]
    draw_grip: DrawResizeGrip,
}

impl Widget for LauncherGrip {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        let step = self.view.draw_walk(cx, scope, walk);
        if self.visible {
            let rect = self.view.area().rect(cx);
            self.draw_grip.draw_abs(cx, rect);
        }
        step
    }
}

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
    /// An upward drag is in progress; drive the drawer to this open fraction (0..1).
    DragDrawer { progress: f64 },
    /// The upward drag ended; snap the drawer open or closed.
    ReleaseDrawer { open: bool },
    /// The user swiped down on the home screen; open Spotlight search.
    OpenSearch,
    /// A long-press landed on an item; show its shortcut menu at `anchor`.
    ShowContextMenu {
        app_id: MiniAppId,
        widget_instance: Option<WidgetInstanceId>,
        anchor: Rect,
    },
    /// The finger started moving after a long-press; dismiss any open menu so the
    /// gesture can become a drag.
    HidePopups,
    /// The user right-clicked empty home-screen space; show the background menu.
    ShowBackgroundMenu { abs: Vec2d },
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
    /// Vertical drag opening the app drawer, following the finger.
    PanningY {
        start_y: f64,
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
    /// The (cols, rows, content px) each widget's Splash script was last told it
    /// occupies, so `on_widget_resize` fires only on actual size changes.
    #[rust]
    tile_sizes: HashMap<WidgetInstanceId, (u8, u8, f64, f64)>,
    /// Size notifications detected during draw, delivered on the next event so
    /// the freshly-evaluated Splash subtree is in the widget tree by then
    /// (script `ui` lookups silently no-op against a stale tree).
    #[rust]
    pending_resize_notify: Vec<(WidgetInstanceId, (u8, u8), Vec2d)>,

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
    /// While dragging, the temporary (col,row) each non-dragged item on the drag's
    /// page shifts to so it opens a gap for the dragged item (Android/iOS reflow).
    /// The lerp animation drives icons to these previewed slots in real time.
    #[rust]
    preview_moves: HashMap<ItemKey, (u8, u8)>,
    #[live]
    draw_target: DrawTargetCell,
    #[live]
    draw_shadow: DrawDragShadow,
    /// Animated on-screen positions of items, keyed by identity, in page-local
    /// coords (relative to the page's origin). Lerped toward layout targets.
    #[rust]
    anim_pos: HashMap<ItemKey, Vec2d>,
    /// Whether edit mode visuals (badges/handles) were applied to children.
    #[rust]
    edit_visuals_applied: bool,
    /// Set during draw when any icon is still lerping to its slot, so the
    /// next-frame handler keeps the redraw loop alive until they land.
    #[rust]
    items_animating: bool,
    /// Free-running clock (secs) driving the edit-mode jiggle wobble.
    #[rust]
    jiggle_time: f64,

    /// The layout's grid dimensions, synced from AppState each draw/event pass.
    #[rust((4, 6))]
    grid: (u8, u8),
    #[rust]
    last_rect: Rect,
    #[rust]
    last_reported_page: f64,
    #[rust(usize::MAX)]
    last_reported_count: usize,
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

/// Grid geometry for one draw/event pass, derived from the pager's rect and the
/// layout's current (user-adjustable) grid dimensions.
#[derive(Clone, Copy)]
struct Geom {
    rect: Rect,
    cell: Vec2d,
    grid: (u8, u8),
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
        if col < 0 || col >= self.grid.0 as i64 || row < 0 || row >= self.grid.1 as i64 {
            return None;
        }
        Some((col as u8, row as u8))
    }
}

impl HomePager {
    fn geom(&self) -> Geom {
        let grid = (self.grid.0.max(1), self.grid.1.max(1));
        Geom {
            rect: self.last_rect,
            cell: dvec2(
                self.last_rect.size.x / grid.0 as f64,
                self.last_rect.size.y / grid.1 as f64,
            ),
            grid,
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

    /// Builds the `ShowContextMenu` action for the given item key, anchoring the
    /// menu to the item's on-screen cell. Returns `None` if the item isn't found.
    fn menu_action_for(&self, layout: &LauncherLayout, item: ItemKey) -> Option<HomePagerAction> {
        let (p, placed) = layout.pages.iter().enumerate().find_map(|(p, page)| {
            page.items
                .iter()
                .find(|it| it.key() == item)
                .map(|it| (p, it.clone()))
        })?;
        let geom = self.geom();
        let anchor = geom.cell_rect(p, self.page_pos, placed.col, placed.row, placed.span());
        let (app_id, widget_instance) = match &placed.kind {
            PlacedKind::App { id } => (id.clone(), None),
            PlacedKind::Widget { app_id, instance, .. } => (app_id.clone(), Some(*instance)),
        };
        Some(HomePagerAction::ShowContextMenu {
            app_id,
            widget_instance,
            anchor,
        })
    }

    /// Emits the PageChanged action if the position or page count changed, so the
    /// page indicator tracks both swipes and layout edits (adding/removing pages).
    fn report_page(&mut self, cx: &mut Cx, count: usize) {
        let count = count.max(1);
        if (self.page_pos - self.last_reported_page).abs() > 0.001
            || count != self.last_reported_count
        {
            self.last_reported_page = self.page_pos;
            self.last_reported_count = count;
            cx.widget_action(
                self.uid,
                HomePagerAction::PageChanged {
                    position: self.page_pos,
                    count,
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
        // New children need the current edit-mode chrome applied explicitly;
        // sync_edit_visuals only touches children on a mode *change*.
        icon.widget(cx, ids!(badge)).set_visible(cx, self.edit_visuals_applied);
        icon.notif_badge(cx, ids!(notif))
            .set_count(state.notifications.get(app_id).copied().unwrap_or(0));
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
        tile.widget(cx, ids!(badge)).set_visible(cx, self.edit_visuals_applied);
        tile.widget(cx, ids!(grip)).set_visible(cx, self.edit_visuals_applied);
        self.tiles.insert(instance, tile.clone());
        Some(tile)
    }

    /// Tells a widget's Splash script how much room it has, by calling the
    /// script's optional `on_widget_resize(cols, rows, w, h)` hook. Fires on
    /// first draw and whenever the span or the tile's pixel size changes
    /// (grid-resize, window-resize), so content can reflow to fit.
    fn notify_widget_size(
        &mut self,
        cx: &mut Cx,
        instance: WidgetInstanceId,
        span: (u8, u8),
        content: Vec2d,
    ) {
        let unchanged = self.tile_sizes.get(&instance).is_some_and(|&(c, r, w, h)| {
            (c, r) == span && (w - content.x).abs() < 0.5 && (h - content.y).abs() < 0.5
        });
        if unchanged {
            return;
        }
        if !self.tiles.contains_key(&instance) {
            return;
        }
        self.tile_sizes.insert(instance, (span.0, span.1, content.x, content.y));
        self.pending_resize_notify.retain(|(i, ..)| *i != instance);
        self.pending_resize_notify.push((instance, span, content));
        self.start_next_frame(cx);
    }

    /// Delivers queued `on_widget_resize` calls (see `pending_resize_notify`).
    fn flush_resize_notifications(&mut self, cx: &mut Cx) {
        for (instance, span, content) in std::mem::take(&mut self.pending_resize_notify) {
            let Some(tile) = self.tiles.get(&instance) else { continue };
            let splash = tile.widget(cx, ids!(splash));
            if let Some(mut splash) = splash.borrow_mut::<Splash>() {
                splash.call_script_fn(
                    cx,
                    live_id!(on_widget_resize),
                    &[
                        (span.0 as f64).into(),
                        (span.1 as f64).into(),
                        content.x.into(),
                        content.y.into(),
                    ],
                );
            }
        }
    }

    /// Drops child widgets whose items are no longer on the home screen.
    fn prune_children(&mut self, cx: &mut Cx, pages: &[HomePage]) {
        let mut live_apps = Vec::new();
        let mut live_widgets = Vec::new();
        for page in pages {
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
        self.tile_sizes.retain(|inst, _| live_widgets.contains(inst));
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

    /// Handles a completed drop of the dragged item.
    fn drop_dragged_item(&mut self, cx: &mut Cx, state: &mut AppState) {
        let Some(drag) = self.drag.take() else { return };
        let preview = std::mem::take(&mut self.preview_moves);
        let layout = &mut state.layout;

        let (page, col, row) = match drag.target {
            Some(target) => target,
            None => {
                // No valid target: return the item to where it came from.
                let (fp, fc, fr) = drag.from;
                while layout.pages.len() <= fp {
                    layout.pages.push(HomePage::default());
                }
                layout.pages[fp].items.push(PlacedItem {
                    kind: drag.item.kind,
                    col: fc,
                    row: fr,
                });
                layout.prune_empty_pages();
                state.layout_dirty = true;
                self.redraw(cx);
                return;
            }
        };
        while layout.pages.len() <= page {
            layout.pages.push(HomePage::default());
        }

        // Commit the swap preview: the displaced icon takes the dragged icon's old cell.
        for it in &mut layout.pages[page].items {
            if let Some(&(pc, pr)) = preview.get(&it.key()) {
                it.col = pc;
                it.row = pr;
            }
        }
        // If an icon still sits on the drop cell (e.g. the drag crossed pages, so
        // no swap-back was possible), bump it to the first free slot so nothing
        // overlaps.
        if let Some(i) = layout.pages[page]
            .items
            .iter()
            .position(|it| it.span() == (1, 1) && it.covers(col, row))
        {
            if let Some((nc, nr)) = layout.pages[page].first_fit(layout.grid(), 1, 1) {
                layout.pages[page].items[i].col = nc;
                layout.pages[page].items[i].row = nr;
            }
        }
        // Drop the dragged item into its cell.
        layout.pages[page].items.push(PlacedItem {
            kind: drag.item.kind,
            col,
            row,
        });
        layout.prune_empty_pages();
        state.layout_dirty = true;
        self.redraw(cx);
    }

    /// Computes the currently hovered drop target for the drag, and the live
    /// reflow preview positions of the other icons.
    fn update_drag_target(&mut self, cx: &mut Cx, layout: &LauncherLayout, abs: Vec2d) {
        let geom = self.geom();
        let page = self.current_page();
        self.preview_moves.clear();
        let Some(drag) = &mut self.drag else { return };
        let (span_cols, span_rows) = drag.item.span();
        // Target the cell under the *center* of the dragged item's first cell,
        // so drops land where the item visually sits.
        let probe = drag.pos + geom.cell * 0.5;
        let is_icon = span_cols == 1 && span_rows == 1;
        let empty_page = HomePage::default();
        let page_items = layout.pages.get(page).unwrap_or(&empty_page);

        let from_cell = (drag.from.1, drag.from.2);
        let from_same_page = drag.from.0 == page;
        let dragged_key = drag.item.key();
        let target = geom.cell_at(self.page_pos, probe).and_then(|(col, row)| {
            let col = col.min(geom.grid.0.saturating_sub(span_cols));
            let row = row.min(geom.grid.1.saturating_sub(span_rows));
            if is_icon {
                // Never drop onto a cell a multi-cell widget occupies.
                let on_widget = page_items.items.iter().any(|it| {
                    matches!(it.kind, PlacedKind::Widget { .. }) && it.covers(col, row)
                });
                if on_widget {
                    return None;
                }
                // If another icon already sits here, it swaps back into the cell the
                // dragged icon came from (a clean, predictable grid swap). An empty
                // cell just accepts the drop.
                if from_same_page {
                    if let Some(occ) = page_items.items.iter().find(|it| {
                        it.span() == (1, 1) && it.covers(col, row) && it.key() != dragged_key
                    }) {
                        self.preview_moves.insert(occ.key(), from_cell);
                    }
                }
                Some((page, col, row))
            } else if page_items.fits(geom.grid, col, row, span_cols, span_rows, None) {
                Some((page, col, row))
            } else {
                None
            }
        });
        let Some(drag) = &mut self.drag else { return };
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
        // (Re)arm the one-shot flip timer on entering an edge, stop it on leaving,
        // so the flip fires no matter when during the drag the finger reaches an edge.
        if edge != drag.edge {
            drag.edge = edge;
            if edge.is_some() {
                self.edge_flip_timer = cx.start_timeout(EDGE_FLIP_SECS);
            } else {
                cx.stop_timer(self.edge_flip_timer);
            }
        }
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
            tile.widget(cx, ids!(grip)).set_visible(cx, edit_mode);
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
        let grid = state.layout.grid();
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
            (start_span.0 as i32 + dcols)
                .clamp(min_span.0 as i32, (grid.0 - col) as i32) as u8;
        let new_rows =
            (start_span.1 as i32 + drows)
                .clamp(min_span.1 as i32, (grid.1 - row) as i32) as u8;
        if !page_items.fits(grid, col, row, new_cols, new_rows, Some(idx)) {
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

    /// The per-icon jiggle offset for edit mode: a tiny two-axis wobble with a
    /// per-item phase (derived from its key) so no two icons move in sync.
    fn jiggle_offset(key: &ItemKey, t: f64) -> Vec2d {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        let phase = (h.finish() % 1000) as f64 / 1000.0 * std::f64::consts::TAU;
        let w = 13.0;
        dvec2(
            0.8 * (w * t + phase).sin(),
            0.8 * (w * 1.11 * t + phase + 1.7).sin(),
        )
    }

    /// Removes the item at (page, idx) from the home screen.
    fn remove_item(&mut self, cx: &mut Cx, state: &mut AppState, page: usize, idx: usize) {
        if let Some(page_items) = state.layout.pages.get_mut(page) {
            if idx < page_items.items.len() {
                page_items.items.remove(idx);
                state.layout.prune_empty_pages();
                state.layout_dirty = true;
                self.prune_children(cx, &state.layout.pages);
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
        // Deliver widget-size notifications queued during the previous draw; by
        // now the widget tree includes any subtree that draw created.
        if !self.pending_resize_notify.is_empty() {
            self.flush_resize_notifications(cx);
        }

        // Step page + item animations.
        if let Some(ne) = self.next_frame.is_event(event) {
            let dt = if self.last_frame_time == 0.0 {
                1.0 / 60.0
            } else {
                (ne.time - self.last_frame_time).clamp(0.0, 0.1)
            };
            self.last_frame_time = ne.time;
            self.jiggle_time += dt;
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
            }

            // The icon-shuffle lerp advances during draw; keep redrawing (which re-runs
            // draw and re-evaluates whether anything still needs to move) while a page is
            // settling, an item is being dragged, or icons are still sliding to new slots.
            // Edit mode also runs continuously to drive the jiggle wobble.
            let edit_mode = scope.data.get::<AppState>().is_some_and(|s| s.edit_mode);
            if self.drag.is_some() || self.items_animating || edit_mode {
                still_animating = true;
            }

            if still_animating {
                self.redraw(cx);
                self.start_next_frame(cx);
            } else {
                self.last_frame_time = 0.0;
            }
            if let Some(state) = scope.data.get_mut::<AppState>() {
                self.report_page(cx, state.layout.pages.len());
            }
        }

        // Long-press timer (iOS semantics): on an item, lift it and pop its
        // shortcut menu (no jiggle yet; sliding into a drag starts that); on
        // empty space, enter jiggle/edit mode.
        if self.long_press_timer.is_event(event).is_some() {
            match self.gesture.clone() {
                Gesture::Pending { start, item: Some(item) } => {
                    self.gesture = Gesture::Lifted { item: item.clone(), start };
                    if let Some(state) = scope.data.get::<AppState>() {
                        if !state.edit_mode {
                            if let Some(action) = self.menu_action_for(&state.layout, item) {
                                cx.widget_action(self.uid, action);
                            }
                        }
                    }
                    self.redraw(cx);
                }
                Gesture::Pending { item: None, .. } => {
                    if let Some(state) = scope.data.get_mut::<AppState>() {
                        state.edit_mode = true;
                    }
                    self.gesture = Gesture::Consumed;
                    self.start_next_frame(cx);
                    self.redraw(cx);
                }
                _ => (),
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
        // spuriously open apps. The one exception is a gesture that's already in
        // flight (Lifted/dragging): long-pressing an icon opens the shortcut menu
        // while the finger stays down, and sliding it must slip out of the menu
        // into a drag, so we keep feeding those finger events to the pager.
        if scope
            .data
            .get::<AppState>()
            .is_some_and(|s| !s.home_input_enabled)
        {
            let hold = matches!(
                self.gesture,
                Gesture::Lifted { .. } | Gesture::DraggingItem { .. }
            );
            if !hold && !matches!(self.gesture, Gesture::Idle) {
                cx.stop_timer(self.long_press_timer);
                cx.stop_timer(self.edge_flip_timer);
                self.set_sweep_lock(cx, false);
                self.gesture = Gesture::Idle;
            }
            if !hold {
                return;
            }
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
        self.grid = state.layout.grid();

        match hit {
            Hit::FingerDown(fe) => {
                // Right-click (desktop) is the long-press equivalent: open the app's
                // shortcut menu over an icon, or the home-screen menu on empty space.
                // It also arms the jiggle, but never turns into a drag.
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    if let Some((page, idx)) = self.item_at(&state.layout, fe.abs) {
                        let key = state.layout.pages[page].items[idx].key();
                        if let Some(action) = self.menu_action_for(&state.layout, key) {
                            cx.widget_action(self.uid, action);
                        }
                    } else {
                        cx.widget_action(
                            self.uid,
                            HomePagerAction::ShowBackgroundMenu { abs: fe.abs },
                        );
                    }
                    self.gesture = Gesture::Consumed;
                    self.redraw(cx);
                    return;
                }
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
                self.long_press_timer = cx.start_timeout(LONG_PRESS_SECS);
            }

            Hit::FingerMove(fe) => {
                match self.gesture.clone() {
                    Gesture::Pending { start, .. } => {
                        let delta = fe.abs - start;
                        // A platform long-press (mobile) fires without waiting for our timer.
                        if fe.has_long_press_occurred && delta.length() < TAP_SLOP {
                            match self.gesture.clone() {
                                Gesture::Pending { item: Some(item), .. } => {
                                    self.gesture = Gesture::Lifted { item: item.clone(), start };
                                    if !state.edit_mode {
                                        if let Some(action) =
                                            self.menu_action_for(&state.layout, item)
                                        {
                                            cx.widget_action(self.uid, action);
                                        }
                                    }
                                    self.redraw(cx);
                                    return;
                                }
                                Gesture::Pending { item: None, .. } => {
                                    state.edit_mode = true;
                                    self.gesture = Gesture::Consumed;
                                    self.start_next_frame(cx);
                                    self.redraw(cx);
                                    return;
                                }
                                _ => (),
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
                                // Upward: open the drawer, following the finger. No sweep
                                // lock: while dragging, the drawer reports itself not-yet-open
                                // so the home screen stays visible and the pager keeps seeing
                                // the finger until it lifts (then the drawer settles).
                                self.gesture = Gesture::PanningY {
                                    start_y: fe.abs.y,
                                    samples: vec![(fe.abs.y, fe.time)],
                                };
                                cx.widget_action(
                                    self.uid,
                                    HomePagerAction::DragDrawer { progress: 0.0 },
                                );
                            } else if delta.y > TAP_SLOP {
                                // Downward: drop the Spotlight search overlay (iOS-style).
                                if delta.y > SWIPE_UP_DISTANCE {
                                    cx.widget_action(self.uid, HomePagerAction::OpenSearch);
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
                        self.report_page(cx, state.layout.pages.len());
                        self.redraw(cx);
                    }
                    Gesture::PanningY { start_y, mut samples } => {
                        // Open fraction grows as the finger rises; the drawer opens
                        // over roughly the lower half of the screen.
                        let travel = (self.last_rect.size.y * 0.55).max(1.0);
                        let progress = ((start_y - fe.abs.y) / travel).clamp(0.0, 1.0);
                        samples.push((fe.abs.y, fe.time));
                        if samples.len() > 5 {
                            samples.remove(0);
                        }
                        self.gesture = Gesture::PanningY { start_y, samples };
                        cx.widget_action(self.uid, HomePagerAction::DragDrawer { progress });
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
                                // Slide out of the shortcut menu into the drag.
                                cx.widget_action(self.uid, HomePagerAction::HidePopups);
                                self.set_sweep_lock(cx, true);
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
                        self.update_drag_target(cx, &state.layout, fe.abs);
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
                    Gesture::PanningY { start_y, samples } => {
                        // Snap open on an upward flick or past the halfway point.
                        let travel = (self.last_rect.size.y * 0.55).max(1.0);
                        let progress = ((start_y - fe.abs.y) / travel).clamp(0.0, 1.0);
                        let velocity = if samples.len() >= 2 {
                            let (y0, t0) = samples[0];
                            let (y1, t1) = *samples.last().unwrap();
                            let dt = (t1 - t0).max(0.001);
                            (y0 - y1) / dt
                        } else {
                            0.0
                        };
                        let open = progress > 0.35 || velocity > 300.0;
                        cx.widget_action(self.uid, HomePagerAction::ReleaseDrawer { open });
                    }
                    Gesture::Lifted { .. } => {
                        // Released in place: the shortcut menu (if any) already opened
                        // when the long press fired, and in jiggle mode a stationary
                        // press-and-release on an item does nothing (iOS-style).
                        self.redraw(cx);
                    }
                    Gesture::DraggingItem => {
                        self.drop_dragged_item(cx, state);
                        self.report_page(cx, state.layout.pages.len());
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
        // Clone only the placements (cheap); the heavy fields (user-app sources,
        // recents) aren't needed to draw and would churn allocations every frame.
        let pages = state.layout.pages.clone();
        self.grid = state.layout.grid();
        let edit_mode = state.edit_mode;
        // Sync the page indicator on the first draw and after layout edits that
        // change the page count (report_page self-gates on an actual change).
        self.report_page(cx, pages.len());

        self.prune_children(cx, &pages);
        self.sync_edit_visuals(cx, edit_mode);

        let geom = self.geom();
        let mut any_anim = false;

        // Drop-target outline: the cell the dragged item would land in.
        if let Some(drag) = &self.drag {
            if let Some((page, col, row)) = drag.target {
                let span = drag.item.span();
                let rect = geom.cell_rect(page, self.page_pos, col, row, span);
                let inset = 6.0;
                self.draw_target.draw_abs(
                    cx,
                    Rect {
                        pos: rect.pos + dvec2(inset, inset),
                        size: rect.size - dvec2(inset * 2.0, inset * 2.0),
                    },
                );
            }
        }

        // Draw items on pages within one page of the current position.
        for (page_idx, page) in pages.iter().enumerate() {
            let p = page_idx as f64;
            if (p - self.page_pos).abs() >= 1.0 {
                continue;
            }
            for item in &page.items {
                let key = item.key();
                let span = item.span();
                // While dragging, non-dragged icons slide to their previewed slots.
                let (cell_col, cell_row) = self
                    .preview_moves
                    .get(&key)
                    .copied()
                    .unwrap_or((item.col, item.row));
                let target_rect =
                    geom.cell_rect(page_idx, self.page_pos, cell_col, cell_row, span);
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
                let mut draw_pos = dvec2(
                    geom.rect.pos.x + (p - self.page_pos) * geom.rect.size.x + anim.x,
                    geom.rect.pos.y + anim.y,
                );
                // Jiggle every icon in edit mode (except the one being dragged),
                // each with its own phase so the whole grid wobbles like iOS.
                if edit_mode && self.drag.as_ref().map(|d| d.item.key()) != Some(key.clone()) {
                    draw_pos += Self::jiggle_offset(&key, self.jiggle_time);
                    any_anim = true;
                }

                // Inset multi-cell widgets so neighbours don't touch (icons already
                // float a small tile inside a larger cell, so they need no gap).
                let gap = if span == (1, 1) { 0.0 } else { WIDGET_GAP };
                let child_walk = Walk {
                    abs_pos: Some(draw_pos + dvec2(gap, gap)),
                    margin: Default::default(),
                    width: Size::Fixed(target_rect.size.x - gap * 2.0),
                    height: Size::Fixed(target_rect.size.y - gap * 2.0),
                    metrics: Default::default(),
                };
                let child = match &item.kind {
                    PlacedKind::App { id } => {
                        let state = scope.data.get::<AppState>().unwrap();
                        self.ensure_icon(cx, state, id)
                    }
                    PlacedKind::Widget { instance, app_id, .. } => {
                        let state = scope.data.get::<AppState>().unwrap();
                        let tile = self.ensure_tile(cx, state, *instance, app_id);
                        // The Splash content sits inside the tile's padded content
                        // view; tell the script its usable size when it changes.
                        let content = dvec2(
                            target_rect.size.x - gap * 2.0 - TILE_CONTENT_PAD * 2.0,
                            target_rect.size.y - gap * 2.0 - TILE_CONTENT_PAD * 2.0,
                        );
                        self.notify_widget_size(cx, *instance, span, content);
                        tile
                    }
                };
                if let Some(child) = child {
                    child.draw_walk_all(cx, scope, child_walk);
                }
            }
        }

        // Draw the dragged item last so it floats above everything, lifted with a
        // soft shadow and scaled up slightly (the classic "picked up" feel).
        if let Some(drag) = &self.drag {
            let span = drag.item.span();
            let base = dvec2(geom.cell.x * span.0 as f64, geom.cell.y * span.1 as f64);
            let scale = 1.08;
            let size = base * scale;
            // Grow around the item's center so it doesn't jump on lift.
            let pos = drag.pos - (size - base) * 0.5;
            self.draw_shadow.draw_abs(
                cx,
                Rect {
                    pos: pos + dvec2(0.0, 4.0),
                    size,
                },
            );
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

        self.items_animating = any_anim;
        if any_anim {
            self.start_next_frame(cx);
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl HomePagerRef {
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
