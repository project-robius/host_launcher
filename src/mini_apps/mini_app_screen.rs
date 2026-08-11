//! The host for running mini-apps: fullscreen, or two side by side (split screen).
//!
//! Each opened app gets its own host view (glass header + a scrollable `Splash`
//! instance) which stays alive after the app is "closed", iOS-style: closing
//! just hides the host, so reopening restores the app instantly with its state
//! intact. "Force stop" (from the context menu) drops the host entirely, which
//! tears down the app's Splash VM isolate; the next open starts it fresh.
//!
//! Hosts are drawn at arbitrary rects — the full container, or one pane of an
//! Android-style split. Whenever a host's settled content size changes (open,
//! window resize, divider drag) the app's script is told via its optional
//! `fn on_app_resize(w, h)` hook, the mini-app analogue of the home widgets'
//! `on_widget_resize`, so scripts can toggle pre-declared layout tiers.
//!
//! Split screen behaves like Android (OnePlus Open flavor): a draggable divider
//! resizes both panes live, releasing it near an edge closes the smaller app
//! and fullscreens the other, and tapping it opens a menu (swap panes, rotate
//! the split axis, fullscreen one app). Entering split goes through "pick"
//! mode: one app docks to a pane while the home screen stays live in the rest
//! of the window to choose the second app from.

use std::collections::HashMap;

use makepad_widgets::{widget_tree::CxWidgetExt, *};

use crate::mini_apps::registry::{MiniAppId, MiniAppManifest};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.MiniAppScreenBase = #(MiniAppScreen::register_widget(vm))

    mod.widgets.MiniAppScreen = set_type_default() do mod.widgets.MiniAppScreenBase{
        width: Fill
        height: Fill

        // The hosted-app chrome lives in shared styles: a home-grid tile
        // and this screen instantiate the SAME template, so expanding a tile
        // can hand its live widget straight over (see shared::styles).
        AppHost := mod.widgets.AppHost{}

        // The split divider: a transparent strip between the panes with a
        // centered grip pill. One shader serves both axes — the pill runs
        // along the strip's long dimension. The pill is a NAMED inner child:
        // ids declared inside an instantiated template land in the snapshot
        // (the outer widget's insertion key does not), so tests can find it.
        Divider := View{
            width: Fill
            height: Fill
            divider_pill := View{
                width: Fill
                height: Fill
                show_bg: true
                draw_bg +: {
                    pixel: fn(){
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        let cx0 = self.rect_size.x * 0.5
                        let cy0 = self.rect_size.y * 0.5
                        if self.rect_size.y > self.rect_size.x {
                            sdf.box(cx0 - 2.5, cy0 - 23.0, 5.0, 46.0, 2.5)
                        } else {
                            sdf.box(cx0 - 23.0, cy0 - 2.5, 46.0, 5.0, 2.5)
                        }
                        sdf.fill(#xd9e8ffb4)
                        return sdf.result
                    }
                }
            }
        }

        // The tap-the-divider menu, OnePlus Open-style: swap the panes, rotate
        // the split between left-right and top-bottom, or fullscreen one app.
        DividerMenu := RoundedView{
            width: Fit
            height: Fit
            flow: Right
            spacing: 6
            padding: Inset{top: 7, bottom: 7, left: 8, right: 8}
            show_bg: true
            draw_bg +: {
                color: #x0a1120f2
                border_color: #xffffff28
                border_size: 1.0
                border_radius: 12.0
            }
            // Fixed widths: Fit would collapse in the headless harness, where
            // template label text measures near zero, making the buttons
            // unclickable by rect in tests.
            menu_swap := glass.GlassButton{
                width: 68
                height: 34
                text: "Swap"
                draw_text +: { text_style: theme.font_bold{font_size: 12} }
            }
            menu_rotate := glass.GlassButton{
                width: 76
                height: 34
                text: "Rotate"
                draw_text +: { text_style: theme.font_bold{font_size: 12} }
            }
            menu_full := glass.GlassButton{
                width: 60
                height: 34
                text: "Full"
                draw_text +: { text_style: theme.font_bold{font_size: 12} }
            }
        }

        // Floating hint shown over the free area during pick mode. The
        // full-width transparent wrapper centers the pill via align, so the
        // pill is EXACTLY centered whatever the text measures — positioning
        // the Fit pill by an estimated width kept drifting off-center.
        PickHint := View{
            width: Fill
            height: Fit
            align: Align{x: 0.5}
            hint_pill := RoundedView{
                width: Fit
                height: Fit
                padding: Inset{top: 8, bottom: 8, left: 14, right: 14}
                show_bg: true
                draw_bg +: {
                    color: #x0a1120d8
                    border_color: #xffffff22
                    border_size: 1.0
                    border_radius: 14.0
                }
                hint_label := Label{
                    text: "Open another app in split screen"
                    draw_text +: {
                        color: #xc8d6f0
                        text_style: theme.font_bold{font_size: 12}
                    }
                }
            }
        }
    }
}

/// Gap between the pick hint and the page-indicator dots it sits above.
const HINT_GAP: f64 = 10.0;

/// Duration of the open/close zoom animation, in seconds. (Was 0.42; the
/// closing tail additionally snaps once the window reaches icon scale.)
const ZOOM_SECS: f64 = 0.36;
/// Duration of pane morphs (enter/leave pick, swap, rotate, exit split).
const MORPH_SECS: f64 = 0.30;
/// Duration of the divider settle/collapse animation after a drag.
const SETTLE_SECS: f64 = 0.22;

/// Visual gap between the two panes; the divider pill sits in it.
const DIVIDER_GAP: f64 = 16.0;
/// Extra grab slop on each side of the gap, so the divider is draggable
/// without pixel-hunting (the strip overlaps the pane borders slightly).
const DIVIDER_SLOP: f64 = 6.0;
/// Smallest a settled pane may be, along the split axis.
const MIN_PANE: f64 = 140.0;
/// Mid-drag rubber limit: the divider tracks the finger down to this pane
/// size, letting the user express "close this side" without hitting a wall.
const DRAG_MIN_PANE: f64 = 56.0;
/// Released with a pane smaller than this → that side closes and the other
/// app goes fullscreen (the OnePlus Open drag-to-dismiss).
const COLLAPSE_AT: f64 = 110.0;
/// Magnetic snap points for the released divider, as pane-A fractions.
const SNAP_POINTS: [f64; 3] = [1.0 / 3.0, 0.5, 2.0 / 3.0];
/// How close (in ratio) a release must be to a snap point to stick to it.
const SNAP_EPS: f64 = 0.03;
/// Finger travel below this is a tap on the divider (opens the menu), not a drag.
const DRAG_SLOP: f64 = 6.0;
/// Containers whose split axis is shorter than this can't fit two usable
/// panes; entering split is refused and a window shrink exits it. Public so
/// the context menu can hide its "Split Screen" entry in a tiny window.
pub const MIN_SPLIT_SPAN: f64 = 2.0 * MIN_PANE + DIVIDER_GAP;

/// Horizontal padding the AppHost template puts around the Splash content
/// (host padding 6+6 plus content padding 8+8). Used to derive the width
/// reported to `on_app_resize` from the drawn host rect.
const CONTENT_PAD_X: f64 = 6.0 + 6.0 + 8.0 + 8.0;
/// Vertical: host padding 6+6, header 44, content padding 4+8.
const CONTENT_PAD_Y: f64 = 6.0 + 6.0 + 44.0 + 4.0 + 8.0;

/// How much of the docked app stays on screen during pick mode: the app
/// keeps its fullscreen SIZE and slides almost entirely off the pane-A edge,
/// leaving this many points peeking in — a slim nub, enough to see something
/// is waiting (and to tap to bring it back), small enough that effectively
/// the whole screen belongs to the home grid for picking.
pub const PICK_PEEK: f64 = 28.0;

/// Which way the screen is split.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitAxis {
    /// Two panes side by side (vertical divider).
    LeftRight,
    /// One pane above the other (horizontal divider).
    TopBottom,
}

/// What the screen is showing.
#[derive(Clone, Debug, Default, PartialEq)]
enum Mode {
    #[default]
    Hidden,
    /// One app, (nearly) fullscreen.
    Single { app: MiniAppId },
    /// Split entry: `a` docked to the first pane, home live in the rest of
    /// the window to pick the second app.
    Pick { a: MiniAppId },
    /// Two apps side by side (or stacked), divider between.
    Split { a: MiniAppId, b: MiniAppId },
}

/// An in-flight divider drag.
#[derive(Clone, Copy, Debug)]
struct DividerDrag {
    start_abs: Vec2d,
    moved: bool,
}

/// The one screen-level animation that may be running.
#[derive(Clone, Debug)]
enum Anim {
    /// An app zooming out from (or back into) an icon rect.
    Zoom { app: MiniAppId, from: Rect, closing: bool },
    /// Every visible host lerping from a captured rect to its current target
    /// (enter/leave pick, swap, rotate, exit split). `hide_after` hosts stay
    /// at their captured rect and are hidden when the morph lands.
    Morph { starts: Vec<(MiniAppId, Rect)>, hide_after: Vec<MiniAppId> },
    /// The divider gliding to a released/collapse position. `survivor` set
    /// means the ratio is heading for an edge and the other app closes.
    Ratio { from: f64, to: f64, survivor: Option<MiniAppId> },
}

/// Actions emitted by the mini-app screen.
#[derive(Clone, Debug, Default)]
pub enum MiniAppScreenAction {
    /// The close animation finished; the launcher is fully visible again.
    FullyClosed,
    #[default]
    None,
}

#[derive(Script, Widget)]
pub struct MiniAppScreen {
    #[deref]
    view: View,
    #[rust]
    templates: HashMap<LiveId, ScriptObjectRef>,
    /// Host views for every app that has been opened (and not force-stopped).
    #[rust]
    hosts: HashMap<MiniAppId, WidgetRef>,
    #[rust]
    mode: Mode,
    /// Where the split divider sits: pane A's share of the split axis.
    #[rust(0.5)]
    ratio: f64,
    #[rust(SplitAxis::TopBottom)]
    axis: SplitAxis,
    /// The split-icon orientation last applied to the headers (None = never):
    /// the shader uniform is only re-applied on change.
    #[rust]
    split_icon_horizontal: Option<bool>,
    /// The pane the user last pressed; back/exit act on it.
    #[rust]
    focused: Option<MiniAppId>,
    #[rust]
    anim: Option<Anim>,
    /// Animation progress 0..1 for `anim`.
    #[rust]
    t: f64,
    #[rust]
    drag: Option<DividerDrag>,
    #[rust]
    menu_open: bool,
    /// The icon rect the current open/close zoom animates from/to.
    #[rust]
    anchor_rect: Rect,
    /// Each app's own launch anchor, so closing a split survivor zooms back
    /// into ITS icon, not whichever app was opened last.
    #[rust]
    anchors: HashMap<MiniAppId, Rect>,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    last_frame_time: f64,
    /// Lazily-instantiated chrome (divider, its menu, the pick hint).
    #[rust]
    divider: Option<WidgetRef>,
    #[rust]
    divider_menu: Option<WidgetRef>,
    #[rust]
    pick_hint: Option<WidgetRef>,
    /// Overlay draw-list the hosts render into DURING ZOOMS, so the moving
    /// window composites ABOVE the home widgets' glass (each tile draws its
    /// whole subtree into its own overlay; overlays paint by per-frame begin
    /// order, and this widget draws after the pager, so this layer wins).
    /// Without it the shrinking app would slide UNDER any widget it crosses.
    #[rust]
    zoom_layer: Option<DrawList2d>,
    /// The home screen's page-indicator rect, mirrored from the app each
    /// event: the pick hint parks in the gap right above it (over the home
    /// screen's own furniture rather than floating in the middle of the
    /// grid). Zero until home has drawn — the hint falls back to just past
    /// the sliver then.
    #[rust]
    hint_anchor: Rect,
    /// The app whose host is on loan from a home-grid tile. Its widget is
    /// owned by the pager too, so this screen must never tear it down.
    #[rust]
    adopted: Option<MiniAppId>,
    /// True while the drawer or search overlay covers the screen during pick
    /// mode. Those layers draw BELOW this widget (an app opened from the
    /// drawer must zoom on top of it), so instead of fighting the z-order the
    /// docked sliver simply doesn't draw — and doesn't fence input — until
    /// the covering layer closes. Synced by the app each event.
    #[rust]
    pick_obscured: bool,
    /// Rects from the last draw, for hit-testing and the pick-mode fence.
    #[rust]
    last_container: Rect,
    #[rust]
    last_rect_a: Rect,
    #[rust]
    last_divider_rect: Rect,
    #[rust]
    last_menu_rect: Rect,
    /// Content size each app's script was last told it occupies, so
    /// `on_app_resize` fires only on real changes (0.5px epsilon).
    #[rust]
    host_sizes: HashMap<MiniAppId, Vec2d>,
    /// Size notifications detected during draw, delivered on the next event so
    /// the freshly-evaluated Splash subtree is in the widget tree by then
    /// (script `ui` lookups silently no-op against a stale tree).
    #[rust]
    pending_resize_notify: Vec<(MiniAppId, Vec2d)>,
}

impl ScriptHook for MiniAppScreen {
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
        vm.cx_mut().widget_tree_mark_dirty(self.widget_uid());
    }
}

impl MiniAppScreen {
    // -------------------------------------------------------------------
    // State queries
    // -------------------------------------------------------------------

    /// Whether any app is currently shown (or animating in/out).
    pub fn is_showing(&self) -> bool {
        self.mode != Mode::Hidden
    }

    /// Whether the screen fully covers the launcher home. False in pick mode,
    /// where home stays visible (and interactive) beside the docked pane.
    pub fn covers_home(&self) -> bool {
        matches!(self.mode, Mode::Single { .. } | Mode::Split { .. })
    }

    /// Whether an app fully covers the screen (not mid open/close zoom).
    pub fn is_fully_open(&self) -> bool {
        self.covers_home() && self.anim.is_none()
    }

    /// Whether an open/close ZOOM (the transition between home and an app)
    /// is running. In-place animations (split morphs, ratio glides) are NOT
    /// zooms: home must stay hidden behind a settled-looking split.
    pub fn is_zooming(&self) -> bool {
        matches!(self.anim, Some(Anim::Zoom { .. }))
    }

    /// Whether we're in split-entry pick mode (home live, one pane docked).
    pub fn is_picking(&self) -> bool {
        matches!(self.mode, Mode::Pick { .. })
    }

    /// Whether two apps are on screen.
    pub fn is_split(&self) -> bool {
        matches!(self.mode, Mode::Split { .. })
    }

    /// Where the docked app sits during pick mode: fullscreen SIZE (never
    /// resized, so its layout doesn't churn), slid along the split axis until
    /// only PICK_PEEK points peek in at the pane-A edge. The rest of the
    /// screen belongs to the home grid for choosing the partner.
    fn pick_dock_rect(&self, container: Rect) -> Rect {
        let mut pos = container.pos;
        match self.axis {
            SplitAxis::TopBottom => pos.y = container.pos.y + PICK_PEEK - container.size.y,
            SplitAxis::LeftRight => pos.x = container.pos.x + PICK_PEEK - container.size.x,
        }
        Rect { pos, size: container.size }
    }

    /// See [`MiniAppScreenRef::adopt_host`].
    fn adopt_host(
        &mut self,
        cx: &mut Cx,
        app_id: &MiniAppId,
        host: WidgetRef,
        from_rect: Rect,
    ) {
        self.finish_anim(cx);
        self.ensure_all_chrome(cx);
        // Fullscreen chrome; the lender put the tile bar away.
        host.set_visible(cx, true);
        host.widget(cx, ids!(tile_bar)).set_visible(cx, false);
        host.widget(cx, ids!(header)).set_visible(cx, true);
        // The tile hid this explicitly to kill its glass overlay; restore it.
        host.widget(cx, ids!(back_button)).set_visible(cx, true);
        // Re-parent it in the widget tree to match where it now DRAWS. The
        // tree holds weak refs (both maps own a strong one), so this is just
        // bookkeeping — but without it the host stays filed under the pager,
        // inside a home screen that gets hidden the moment an app covers it,
        // and anything reading the tree sees a fullscreen app as "hidden".
        cx.widget_tree_insert_child_deep(
            self.widget_uid(),
            LiveId::from_str(app_id),
            host.clone(),
        );
        self.hosts.insert(app_id.clone(), host);
        self.adopted = Some(app_id.clone());
        // Its content box is about to change; let the next draw re-notify.
        self.host_sizes.remove(app_id);
        self.split_icon_horizontal = None;
        self.anchors.insert(app_id.clone(), from_rect);
        self.anchor_rect = from_rect;
        self.mode = Mode::Single { app: app_id.clone() };
        self.focused = Some(app_id.clone());
        self.start_anim(
            cx,
            Anim::Zoom { app: app_id.clone(), from: from_rect, closing: false },
        );
        self.sync_host_visibility(cx);
        cx.redraw_all();
    }

    /// See [`MiniAppScreenRef::release_adopted`].
    fn release_adopted(&mut self, cx: &mut Cx) -> Option<MiniAppId> {
        let app_id = self.adopted.take()?;
        // Drop our reference only — the home tile owns one too, so the
        // isolate, its timers and its state all survive the handback.
        self.hosts.remove(&app_id);
        self.host_sizes.remove(&app_id);
        self.anchors.remove(&app_id);
        self.pending_resize_notify.retain(|(id, _)| *id != app_id);
        match self.mode.clone() {
            Mode::Single { app } | Mode::Pick { a: app } if app == app_id => {
                self.mode = Mode::Hidden;
                self.anim = None;
            }
            // Half of a split went home: the other pane keeps the screen
            // rather than being left pointing at a host that's gone.
            Mode::Split { a, b } if a == app_id || b == app_id => {
                let survivor = if a == app_id { b } else { a };
                self.mode = Mode::Single { app: survivor.clone() };
                self.focused = Some(survivor);
                self.menu_open = false;
                self.anim = None;
            }
            _ => {}
        }
        self.sync_host_visibility(cx);
        cx.redraw_all();
        Some(app_id)
    }

    /// See the `hint_anchor` field.
    pub fn set_hint_anchor(&mut self, cx: &mut Cx, rect: Rect) {
        if (self.hint_anchor.pos.y - rect.pos.y).abs() > 0.5
            || (self.hint_anchor.size.y - rect.size.y).abs() > 0.5
        {
            self.hint_anchor = rect;
            if self.is_picking() {
                self.redraw(cx);
            }
        }
    }

    /// See the `pick_obscured` field: the docked sliver yields entirely while
    /// the drawer or search overlay is up.
    pub fn set_pick_obscured(&mut self, cx: &mut Cx, obscured: bool) {
        if self.pick_obscured != obscured {
            self.pick_obscured = obscured;
            if self.is_picking() {
                cx.redraw_all();
            }
        }
    }

    /// The docked sliver's VISIBLE rect (plus a little margin) during pick
    /// mode, in window coords. Home-layer widgets ignore presses inside it,
    /// so taps on the peeking app can't fall through to what it covers. Zero
    /// while the drawer/search overlay is up: the sliver isn't drawn then,
    /// and the covering layer owns every tap.
    pub fn pick_block_rect(&self) -> Rect {
        if !self.is_picking() || self.pick_obscured {
            return Rect::default();
        }
        let r = Rect {
            pos: self.last_rect_a.pos - dvec2(4.0, 4.0),
            size: self.last_rect_a.size + dvec2(8.0, 8.0),
        };
        // Only the on-screen part fences: the rest of the shifted rect hangs
        // off the window.
        let c = self.last_container;
        let x0 = r.pos.x.max(c.pos.x - 4.0);
        let y0 = r.pos.y.max(c.pos.y - 4.0);
        let x1 = (r.pos.x + r.size.x).min(c.pos.x + c.size.x + 4.0);
        let y1 = (r.pos.y + r.size.y).min(c.pos.y + c.size.y + 4.0);
        if x1 <= x0 || y1 <= y0 {
            return Rect::default();
        }
        Rect { pos: dvec2(x0, y0), size: dvec2(x1 - x0, y1 - y0) }
    }

    /// Whether the given app has a live (running) host instance.
    pub fn is_running(&self, app_id: &MiniAppId) -> bool {
        self.hosts.contains_key(app_id)
    }

    /// The split's shape, for tests and debug output: (a, b, axis, ratio).
    pub fn split_state(&self) -> Option<(MiniAppId, MiniAppId, SplitAxis, f64)> {
        match &self.mode {
            Mode::Split { a, b } => Some((a.clone(), b.clone(), self.axis, self.ratio)),
            _ => None,
        }
    }

    /// The app back/exit currently acts on: the last-pressed pane, falling
    /// back to pane B (the most recently added), then pane A.
    fn focused_app(&self) -> Option<MiniAppId> {
        match &self.mode {
            Mode::Hidden => None,
            Mode::Single { app } => Some(app.clone()),
            Mode::Pick { a } => Some(a.clone()),
            Mode::Split { a, b } => {
                if let Some(f) = &self.focused {
                    if f == a || f == b {
                        return Some(f.clone());
                    }
                }
                Some(b.clone())
            }
        }
    }

    // -------------------------------------------------------------------
    // Opening / closing / split transitions
    // -------------------------------------------------------------------

    /// Ensures `app` has a live host (creating it, and its Splash isolate, if
    /// needed) and returns it.
    fn ensure_host(&mut self, cx: &mut Cx, manifest: &MiniAppManifest) -> Option<WidgetRef> {
        if let Some(host) = self.hosts.get(&manifest.id) {
            return Some(host.clone());
        }
        let uid = self.widget_uid();
        let Some(template) = self.templates.get(&live_id!(AppHost)) else {
            error!("BUG: MiniAppScreen is missing its AppHost template");
            return None;
        };
        let template_value: ScriptValue = template.as_object().into();
        let host = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        cx.widget_tree_insert_child_deep(
            uid,
            LiveId::from_str(&manifest.id),
            host.clone(),
        );
        host.label(cx, ids!(glyph)).set_text(cx, &manifest.icon);
        host.label(cx, ids!(title)).set_text(cx, &manifest.name);
        // A fresh header carries the template's default icon orientation;
        // drop the cache so the next sync re-applies to every host.
        self.split_icon_horizontal = None;
        if manifest.allow_net {
            if let Some(mut splash) = host.widget(cx, ids!(splash)).borrow_mut::<Splash>() {
                splash.set_allow_net(true);
            }
        }
        // The app's private storage jail (its `fs` root), assigned BEFORE
        // the source evals so top-level fs.read boot loads see it.
        if let Some(mut splash) = host.widget(cx, ids!(splash)).borrow_mut::<Splash>() {
            splash.set_sandbox_dir(cx, Some(crate::app_sandbox_dir(&manifest.id)));
            // Names this script in the error log. Without it a runtime
            // error from a mini-app reported an EMPTY file and a line
            // number that was really a pointer address, so working out
            // which app misbehaved meant grepping every installed script.
            splash.set_debug_name(&manifest.id);
        }
        // Evaluating the source spins up the app's own isolated Splash VM.
        host.widget(cx, ids!(splash)).set_text(cx, &manifest.source);
        self.hosts.insert(manifest.id.clone(), host.clone());
        Some(host)
    }

    /// Instantiates all the split chrome up front, at event time. Widgets
    /// inserted into the tree during DRAW never make it into the widget-tree
    /// snapshot (the tests' selectors see nothing), so this must run before
    /// the draw that first needs them.
    fn ensure_all_chrome(&mut self, cx: &mut Cx) {
        self.ensure_chrome(cx, live_id!(Divider), live_id!(split_divider));
        self.ensure_chrome(cx, live_id!(DividerMenu), live_id!(split_menu));
        self.ensure_chrome(cx, live_id!(PickHint), live_id!(pick_hint));
    }

    /// Instantiates one of this widget's chrome templates on first use.
    fn ensure_chrome(&mut self, cx: &mut Cx, template: LiveId, child: LiveId) -> Option<WidgetRef> {
        let slot = if child == live_id!(split_divider) {
            &self.divider
        } else if child == live_id!(split_menu) {
            &self.divider_menu
        } else {
            &self.pick_hint
        };
        if let Some(w) = slot {
            return Some(w.clone());
        }
        let uid = self.widget_uid();
        let template = self.templates.get(&template)?;
        let template_value: ScriptValue = template.as_object().into();
        let widget = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        cx.widget_tree_insert_child_deep(uid, child, widget.clone());
        if child == live_id!(split_divider) {
            self.divider = Some(widget.clone());
        } else if child == live_id!(split_menu) {
            self.divider_menu = Some(widget.clone());
        } else {
            self.pick_hint = Some(widget.clone());
        }
        Some(widget)
    }

    /// Keeps the chrome's visibility flags in step with the mode, so the
    /// widget-tree snapshot (which tests select against) tells the truth.
    /// Cheap: `set_visible` no-ops when nothing changed.
    fn sync_chrome_visibility(&mut self, cx: &mut Cx) {
        if let Some(divider) = &self.divider {
            divider.set_visible(cx, self.is_split());
        }
        if let Some(menu) = &self.divider_menu {
            menu.set_visible(cx, self.menu_open && self.is_split());
        }
        if let Some(hint) = &self.pick_hint {
            hint.set_visible(cx, self.is_picking());
        }
    }

    /// Points every header's split icon along the divider a split would
    /// actually use: the live axis while split (Rotate flips it), else the
    /// axis a fresh split would pick from the container aspect — a tall
    /// window splits top/bottom, so its icon shows a horizontal divider.
    fn sync_split_icons(&mut self, cx: &mut Cx) {
        let horizontal = match &self.mode {
            Mode::Split { .. } => self.axis == SplitAxis::TopBottom,
            _ => self.best_axis() == SplitAxis::TopBottom,
        };
        if self.split_icon_horizontal == Some(horizontal) {
            return;
        }
        self.split_icon_horizontal = Some(horizontal);
        // NOT named `v`: script_apply_eval!'s generated values-block also
        // binds `v`, and the interpolation would capture that instead.
        let toward = if horizontal { 1.0 } else { 0.0 };
        for host in self.hosts.values() {
            let mut btn = host.widget(cx, ids!(split_button));
            script_apply_eval!(cx, btn, {
                draw_bg +: { horizontal: #(toward) }
            });
            btn.redraw(cx);
        }
    }

    /// Shows exactly the hosts the current mode calls for.
    fn sync_host_visibility(&mut self, cx: &mut Cx) {
        let shown: Vec<MiniAppId> = match &self.mode {
            Mode::Hidden => vec![],
            Mode::Single { app } => vec![app.clone()],
            Mode::Pick { a } => vec![a.clone()],
            Mode::Split { a, b } => vec![a.clone(), b.clone()],
        };
        for (id, host) in &self.hosts {
            let show = shown.contains(id);
            host.set_visible(cx, show);
            // The × is a GlassButton, which paints into its OWN overlay draw
            // list — and an overlay composites above the whole main pass and
            // only clears once it stops being re-begun. Hiding just the host
            // left the button hanging over the home screen after every close,
            // so it gets hidden (and restored) explicitly here, the one place
            // host visibility is decided.
            host.widget(cx, ids!(back_button)).set_visible(cx, show);
        }
        self.sync_chrome_visibility(cx);
    }

    /// Opens the given app: fullscreen normally; into the free pane while
    /// picking a split partner; replacing the focused pane while split.
    /// Zooms out from `from_rect` in every case.
    pub fn open_app(&mut self, cx: &mut Cx, manifest: &MiniAppManifest, from_rect: Rect) {
        if self.ensure_host(cx, manifest).is_none() {
            return;
        }
        self.ensure_all_chrome(cx);
        // Settle any in-flight transition BEFORE reading the mode: finishing
        // a closing zoom sets Hidden, and doing that inside start_anim (after
        // the new mode was assigned) would clobber it — an app opened during
        // another's close zoom ended up on a dead, empty screen.
        self.finish_anim(cx);
        let id = manifest.id.clone();
        self.menu_open = false;
        match self.mode.clone() {
            Mode::Hidden | Mode::Single { .. } => {
                self.mode = Mode::Single { app: id.clone() };
                self.start_anim(cx, Anim::Zoom { app: id.clone(), from: from_rect, closing: false });
            }
            Mode::Pick { a } => {
                if a == id {
                    // Same app on both sides isn't possible (one host, one VM
                    // isolate per app); the tap just does nothing.
                    return;
                }
                self.mode = Mode::Split { a, b: id.clone() };
                self.start_anim(cx, Anim::Zoom { app: id.clone(), from: from_rect, closing: false });
            }
            Mode::Split { a, b } => {
                if id == a || id == b {
                    self.focused = Some(id);
                    return;
                }
                // Replace the focused pane's app, Android-style.
                let target = self.focused_app().unwrap_or_else(|| b.clone());
                let (na, nb) = if target == a { (id.clone(), b) } else { (a, id.clone()) };
                self.mode = Mode::Split { a: na, b: nb };
                self.start_anim(cx, Anim::Zoom { app: id.clone(), from: from_rect, closing: false });
            }
        }
        self.anchor_rect = from_rect;
        self.anchors.insert(id.clone(), from_rect);
        self.focused = Some(id);
        self.sync_host_visibility(cx);
        cx.redraw_all();
    }

    /// Docks `app` to the first pane and enters pick mode, where the home
    /// screen stays live to choose the second app. From fullscreen this
    /// morphs the open app aside; from home it zooms out of `from_rect`.
    /// Refused when the window is too small to ever fit two panes.
    /// `screen` is the window's inner size, needed because the container
    /// rect is only learned by drawing — which hasn't happened when split
    /// is entered straight from the home screen.
    pub fn enter_split_pick(
        &mut self,
        cx: &mut Cx,
        manifest: &MiniAppManifest,
        from_rect: Option<Rect>,
        screen: Vec2d,
    ) {
        // The container is only learned by drawing, which doesn't happen
        // while Hidden — the live window size is always at least as fresh,
        // so reseed whenever it's known (a resize-while-closed otherwise
        // leaves a stale size that mis-refuses the split or picks the axis
        // from an old aspect ratio).
        if screen.x > 1.0 {
            self.last_container = Rect {
                pos: dvec2(4.0, 4.0),
                size: screen - dvec2(8.0, 8.0),
            };
        }
        // Refuse only when the container is KNOWN to be too small. At startup
        // (debug states) neither a draw nor the window geometry has happened
        // yet; entering anyway is safe — the WindowGeomChange guard exits a
        // split that turns out not to fit.
        if self.last_container.size.x >= 1.0
            && self.split_span(self.best_axis()) < MIN_SPLIT_SPAN
        {
            return;
        }
        if self.ensure_host(cx, manifest).is_none() {
            return;
        }
        self.ensure_all_chrome(cx);
        // Same stale-anim rule as open_app: settle before reading the mode.
        self.finish_anim(cx);
        let id = manifest.id.clone();
        self.menu_open = false;
        self.axis = self.best_axis();
        self.ratio = 0.5;
        let was_fullscreen = matches!(&self.mode, Mode::Single { app } if *app == id);
        self.mode = Mode::Pick { a: id.clone() };
        if was_fullscreen {
            let starts = vec![(id.clone(), self.last_container)];
            self.start_anim(cx, Anim::Morph { starts, hide_after: vec![] });
        } else {
            let from = from_rect.unwrap_or(Rect {
                pos: self.last_container.pos + self.last_container.size * 0.5
                    - dvec2(28.0, 28.0),
                size: dvec2(56.0, 56.0),
            });
            self.anchor_rect = from;
            self.anchors.insert(id.clone(), from);
            self.start_anim(cx, Anim::Zoom { app: id.clone(), from, closing: false });
        }
        self.focused = Some(id);
        self.sync_host_visibility(cx);
        cx.redraw_all();
    }

    /// Leaves pick mode, restoring the docked app to fullscreen.
    fn cancel_pick(&mut self, cx: &mut Cx) {
        self.finish_anim(cx);
        let Mode::Pick { a } = self.mode.clone() else { return };
        let starts = vec![(a.clone(), self.last_rect_a)];
        self.mode = Mode::Single { app: a };
        self.start_anim(cx, Anim::Morph { starts, hide_after: vec![] });
        cx.redraw_all();
    }

    /// Exits split with `winner` fullscreen; the other pane hides in place.
    fn exit_split_to(&mut self, cx: &mut Cx, winner: MiniAppId) {
        self.finish_anim(cx);
        let Mode::Split { a, b } = self.mode.clone() else { return };
        let loser = if winner == a { b.clone() } else { a.clone() };
        let (ra, rb, _) = self.pane_rects(self.last_container, self.draw_ratio());
        let rect_of = |id: &MiniAppId| if *id == a { ra } else { rb };
        let starts = vec![(winner.clone(), rect_of(&winner)), (loser.clone(), rect_of(&loser))];
        self.menu_open = false;
        self.mode = Mode::Single { app: winner.clone() };
        self.focused = Some(winner);
        self.start_anim(cx, Anim::Morph { starts, hide_after: vec![loser] });
        cx.redraw_all();
    }

    /// Collapses the divider toward one edge, closing `loser`'s pane; the
    /// other app goes fullscreen when the glide lands.
    fn collapse_pane(&mut self, cx: &mut Cx, loser: MiniAppId) {
        self.finish_anim(cx);
        let Mode::Split { a, b } = self.mode.clone() else { return };
        let survivor = if loser == a { b } else { a.clone() };
        let to = if loser == a { 0.0 } else { 1.0 };
        self.menu_open = false;
        self.start_anim(cx, Anim::Ratio { from: self.ratio, to, survivor: Some(survivor) });
        cx.redraw_all();
    }

    /// Swaps which app is in which pane.
    fn swap_panes(&mut self, cx: &mut Cx) {
        self.finish_anim(cx);
        let Mode::Split { a, b } = self.mode.clone() else { return };
        let (ra, rb, _) = self.pane_rects(self.last_container, self.draw_ratio());
        let starts = vec![(a.clone(), ra), (b.clone(), rb)];
        self.mode = Mode::Split { a: b, b: a };
        self.menu_open = false;
        self.start_anim(cx, Anim::Morph { starts, hide_after: vec![] });
        cx.redraw_all();
    }

    /// Toggles the split between left-right and top-bottom.
    fn rotate_axis(&mut self, cx: &mut Cx) {
        self.finish_anim(cx);
        let Mode::Split { a, b } = self.mode.clone() else { return };
        // Refuse when the other axis can't fit two panes (very squat/narrow
        // windows); the menu press just does nothing.
        let other = match self.axis {
            SplitAxis::LeftRight => SplitAxis::TopBottom,
            SplitAxis::TopBottom => SplitAxis::LeftRight,
        };
        if self.split_span(other) < MIN_SPLIT_SPAN {
            return;
        }
        let (ra, rb, _) = self.pane_rects(self.last_container, self.draw_ratio());
        let starts = vec![(a, ra), (b, rb)];
        self.axis = other;
        // The other axis has a different span; a ratio that was legal
        // left-right can leave a sliver pane top-bottom.
        let (lo, hi) = self.ratio_bounds();
        self.ratio = self.ratio.clamp(lo, hi);
        self.menu_open = false;
        self.start_anim(cx, Anim::Morph { starts, hide_after: vec![] });
        // The headers' split icons track the live axis.
        self.sync_split_icons(cx);
        cx.redraw_all();
    }

    /// Starts the zoom-back-to-icon close animation for a fullscreen app.
    pub fn close_active(&mut self, cx: &mut Cx) {
        match self.mode.clone() {
            Mode::Single { app } => {
                if matches!(self.anim, Some(Anim::Zoom { closing: true, .. })) {
                    return;
                }
                // Closing mid-open reverses the zoom from (roughly) where it
                // is, rather than snapping to fullscreen first. Any OTHER
                // in-flight transition is settled properly (a replaced Morph
                // would leak its hide_after losers as visible hosts).
                let resume_t = match &self.anim {
                    Some(Anim::Zoom { app: za, closing: false, .. }) if *za == app => {
                        Some(1.0 - self.t)
                    }
                    _ => None,
                };
                if resume_t.is_none() {
                    self.finish_anim(cx);
                }
                let from = self
                    .anchors
                    .get(&app)
                    .copied()
                    .unwrap_or(self.anchor_rect);
                self.anim = Some(Anim::Zoom { app, from, closing: true });
                self.t = resume_t.unwrap_or(0.0);
                self.last_frame_time = 0.0;
                self.next_frame = cx.new_next_frame();
                self.redraw(cx);
            }
            Mode::Pick { .. } => self.cancel_pick(cx),
            Mode::Split { .. } => {
                if let Some(focused) = self.focused_app() {
                    self.collapse_pane(cx, focused);
                }
            }
            Mode::Hidden => {}
        }
    }

    /// Dismisses whatever is showing, back to the bare home screen: a
    /// fullscreen app or a pick's docked pane zooms away; a split hides
    /// instantly (two panes have no single anchor to zoom into).
    pub fn close_to_home(&mut self, cx: &mut Cx) {
        self.finish_anim(cx);
        match self.mode.clone() {
            Mode::Hidden => {}
            Mode::Single { .. } => self.close_active(cx),
            Mode::Pick { a } => {
                let from = self.anchors.get(&a).copied().unwrap_or(self.anchor_rect);
                self.start_anim(cx, Anim::Zoom { app: a, from, closing: true });
                self.redraw(cx);
            }
            Mode::Split { .. } => {
                self.mode = Mode::Hidden;
                self.anim = None;
                self.menu_open = false;
                self.sync_host_visibility(cx);
                cx.widget_action(self.widget_uid(), MiniAppScreenAction::FullyClosed);
                cx.redraw_all();
            }
        }
    }

    /// One step of "back": close the divider menu, cancel pick mode, close
    /// the focused split pane, or close the fullscreen app. Returns whether
    /// the press was consumed.
    pub fn handle_back(&mut self, cx: &mut Cx) -> bool {
        if self.mode == Mode::Hidden {
            return false;
        }
        if self.menu_open {
            self.menu_open = false;
            self.redraw(cx);
            return true;
        }
        self.close_active(cx);
        true
    }

    /// Tears down an app's host entirely, killing its Splash VM isolate.
    /// The next open starts the app from scratch. A split survives with the
    /// other app fullscreen; pick mode just closes.
    pub fn force_stop(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        if self.hosts.remove(app_id).is_none() {
            return;
        }
        self.host_sizes.remove(app_id);
        // A host on loan from a home tile is gone from this screen too; the
        // pager drops its own reference separately (drop_app_widget_tiles).
        if self.adopted.as_ref() == Some(app_id) {
            self.adopted = None;
        }
        self.anchors.remove(app_id);
        self.pending_resize_notify.retain(|(id, _)| id != app_id);
        match self.mode.clone() {
            Mode::Single { app } if app == *app_id => {
                self.mode = Mode::Hidden;
                self.anim = None;
                cx.widget_action(self.widget_uid(), MiniAppScreenAction::FullyClosed);
            }
            Mode::Pick { a } if a == *app_id => {
                self.mode = Mode::Hidden;
                self.anim = None;
            }
            Mode::Split { a, b } if a == *app_id || b == *app_id => {
                let survivor = if a == *app_id { b } else { a };
                // Abrupt by design: force stop is a kill, not a close. The
                // survivor snaps to fullscreen with no glide.
                self.mode = Mode::Single { app: survivor.clone() };
                self.focused = Some(survivor);
                self.anim = None;
                self.menu_open = false;
            }
            _ => {}
        }
        self.sync_host_visibility(cx);
        cx.widget_tree_mark_dirty(self.widget_uid());
        // Reclaim the dropped host's Splash isolate promptly, and repaint
        // fully so no stale overlay draw lists linger.
        makepad_widgets::widget_async::gc_dead_splash_isolates(cx);
        cx.redraw_all();
    }

    // -------------------------------------------------------------------
    // Geometry
    // -------------------------------------------------------------------

    /// The container's extent along a given split axis.
    fn split_span(&self, axis: SplitAxis) -> f64 {
        match axis {
            SplitAxis::LeftRight => self.last_container.size.x,
            SplitAxis::TopBottom => self.last_container.size.y,
        }
    }

    /// The axis a fresh split should use: side by side in a wide window,
    /// stacked in a tall one — like a phone rotating.
    fn best_axis(&self) -> SplitAxis {
        if self.last_container.size.x > self.last_container.size.y {
            SplitAxis::LeftRight
        } else {
            SplitAxis::TopBottom
        }
    }

    /// The ratio to draw with right now: the live ratio, overridden by an
    /// in-flight settle/collapse glide.
    fn draw_ratio(&self) -> f64 {
        if let Some(Anim::Ratio { from, to, .. }) = &self.anim {
            let eased = ease_out(self.t);
            return from + (to - from) * eased;
        }
        self.ratio
    }

    /// The two pane rects and the divider strip for a container and ratio.
    fn pane_rects(&self, container: Rect, ratio: f64) -> (Rect, Rect, Rect) {
        let gap = DIVIDER_GAP;
        match self.axis {
            SplitAxis::LeftRight => {
                let span = (container.size.x - gap).max(0.0);
                let a_w = span * ratio;
                let a = Rect { pos: container.pos, size: dvec2(a_w, container.size.y) };
                let b = Rect {
                    pos: container.pos + dvec2(a_w + gap, 0.0),
                    size: dvec2(span - a_w, container.size.y),
                };
                let strip = Rect {
                    pos: container.pos + dvec2(a_w - DIVIDER_SLOP, 0.0),
                    size: dvec2(gap + DIVIDER_SLOP * 2.0, container.size.y),
                };
                (a, b, strip)
            }
            SplitAxis::TopBottom => {
                let span = (container.size.y - gap).max(0.0);
                let a_h = span * ratio;
                let a = Rect { pos: container.pos, size: dvec2(container.size.x, a_h) };
                let b = Rect {
                    pos: container.pos + dvec2(0.0, a_h + gap),
                    size: dvec2(container.size.x, span - a_h),
                };
                let strip = Rect {
                    pos: container.pos + dvec2(0.0, a_h - DIVIDER_SLOP),
                    size: dvec2(container.size.x, gap + DIVIDER_SLOP * 2.0),
                };
                (a, b, strip)
            }
        }
    }

    /// Ratio bounds that keep both settled panes at least `MIN_PANE`.
    fn ratio_bounds(&self) -> (f64, f64) {
        let span = (self.split_span(self.axis) - DIVIDER_GAP).max(1.0);
        let min = (MIN_PANE / span).min(0.5);
        (min, 1.0 - min)
    }

    /// Maps a dragged finger position to a pane-A ratio, rubber-limited so
    /// each pane keeps `DRAG_MIN_PANE` on screen.
    fn ratio_for_finger(&self, abs: Vec2d) -> f64 {
        let container = self.last_container;
        let (along, start, span_full) = match self.axis {
            SplitAxis::LeftRight => (abs.x, container.pos.x, container.size.x),
            SplitAxis::TopBottom => (abs.y, container.pos.y, container.size.y),
        };
        let span = (span_full - DIVIDER_GAP).max(1.0);
        let a_px = (along - start - DIVIDER_GAP * 0.5)
            .clamp(DRAG_MIN_PANE, (span - DRAG_MIN_PANE).max(DRAG_MIN_PANE));
        a_px / span
    }

    /// Applies the release rules to the ratio the finger left behind:
    /// collapse past the edge thresholds, magnet onto the snap points,
    /// otherwise settle within the min-pane bounds.
    fn settle_divider(&mut self, cx: &mut Cx) {
        let Mode::Split { a, b } = self.mode.clone() else { return };
        let span = (self.split_span(self.axis) - DIVIDER_GAP).max(1.0);
        let a_px = span * self.ratio;
        let b_px = span - a_px;
        if a_px < COLLAPSE_AT {
            self.start_anim(cx, Anim::Ratio { from: self.ratio, to: 0.0, survivor: Some(b) });
            return;
        }
        if b_px < COLLAPSE_AT {
            self.start_anim(cx, Anim::Ratio { from: self.ratio, to: 1.0, survivor: Some(a) });
            return;
        }
        let (lo, hi) = self.ratio_bounds();
        let mut target = self.ratio.clamp(lo, hi);
        for snap in SNAP_POINTS {
            if (target - snap).abs() < SNAP_EPS && snap >= lo && snap <= hi {
                target = snap;
                break;
            }
        }
        if (target - self.ratio).abs() > 0.0005 {
            self.start_anim(cx, Anim::Ratio { from: self.ratio, to: target, survivor: None });
        }
    }

    // -------------------------------------------------------------------
    // Animation
    // -------------------------------------------------------------------

    fn start_anim(&mut self, cx: &mut Cx, anim: Anim) {
        // Starting a new transition snaps the previous one to its end state.
        if self.anim.is_some() {
            self.finish_anim(cx);
        }
        self.anim = Some(anim);
        self.t = 0.0;
        self.last_frame_time = 0.0;
        self.next_frame = cx.new_next_frame();
    }

    fn anim_secs(anim: &Anim) -> f64 {
        match anim {
            Anim::Zoom { .. } => ZOOM_SECS,
            Anim::Morph { .. } => MORPH_SECS,
            Anim::Ratio { .. } => SETTLE_SECS,
        }
    }

    /// Applies an animation's end state and clears it.
    fn finish_anim(&mut self, cx: &mut Cx) {
        let Some(anim) = self.anim.take() else { return };
        // Every arm below settles `mode` and/or hides hosts by hand. Run the
        // one visibility authority afterwards so nothing is left half-hidden:
        // hiding a host directly used to leave its glass × flagged visible,
        // and a GlassButton paints from its own overlay draw list, so it kept
        // hanging over the home screen after the app had closed.
        match anim {
            Anim::Zoom { app, closing: true, .. } => {
                if let Some(host) = self.hosts.get(&app) {
                    host.set_visible(cx, false);
                }
                self.mode = Mode::Hidden;
                cx.widget_action(self.widget_uid(), MiniAppScreenAction::FullyClosed);
            }
            Anim::Zoom { closing: false, .. } => {}
            Anim::Morph { hide_after, .. } => {
                for id in hide_after {
                    if let Some(host) = self.hosts.get(&id) {
                        host.set_visible(cx, false);
                    }
                }
            }
            Anim::Ratio { to, survivor, .. } => {
                self.ratio = to;
                if let Some(survivor) = survivor {
                    self.mode = Mode::Single { app: survivor.clone() };
                    self.focused = Some(survivor);
                    self.ratio = 0.5;
                    self.sync_host_visibility(cx);
                }
            }
        }
        self.sync_host_visibility(cx);
    }

    fn step_anim(&mut self, cx: &mut Cx, time: f64) {
        let Some(anim) = &self.anim else { return };
        let dt = if self.last_frame_time == 0.0 {
            1.0 / 60.0
        } else {
            (time - self.last_frame_time).clamp(0.0, 0.1)
        };
        self.last_frame_time = time;
        self.t += dt / Self::anim_secs(anim);
        // A closing zoom's tail reads as lag: once the shrinking window is
        // down to icon scale it's visually gone — snap the rest.
        if let Some(Anim::Zoom { from, closing: true, .. }) = &self.anim {
            let eased = ease_out(self.t.min(1.0));
            let src = self.last_rect_a;
            let w = src.size.x + (from.size.x - src.size.x) * eased;
            let h = src.size.y + (from.size.y - src.size.y) * eased;
            if w <= from.size.x + 16.0 && h <= from.size.y + 16.0 {
                self.t = 1.0;
            }
        }
        if self.t >= 1.0 {
            self.t = 1.0;
            self.finish_anim(cx);
        } else {
            self.next_frame = cx.new_next_frame();
        }
        // Transitions reveal/cover launcher content behind the panes.
        cx.redraw_all();
    }

    // -------------------------------------------------------------------
    // Resize notifications (the on_app_resize hook)
    // -------------------------------------------------------------------

    /// Queues an `on_app_resize` call if the app's content box changed. Called
    /// during draw with the host's drawn rect; delivery happens on the next
    /// event (see `pending_resize_notify`).
    fn note_host_size(&mut self, cx: &mut Cx, app_id: &MiniAppId, host_rect: Rect) {
        let content = dvec2(
            (host_rect.size.x - CONTENT_PAD_X).max(0.0),
            (host_rect.size.y - CONTENT_PAD_Y).max(0.0),
        );
        if content.x <= 1.0 || content.y <= 1.0 {
            return;
        }
        let unchanged = self.host_sizes.get(app_id).is_some_and(|prev| {
            (prev.x - content.x).abs() < 0.5 && (prev.y - content.y).abs() < 0.5
        });
        if unchanged {
            return;
        }
        self.host_sizes.insert(app_id.clone(), content);
        self.pending_resize_notify.retain(|(id, _)| id != app_id);
        self.pending_resize_notify.push((app_id.clone(), content));
        self.next_frame = cx.new_next_frame();
    }

    /// Delivers queued `on_app_resize` calls. Runs at event time so the
    /// freshly-evaluated Splash subtree is in the widget tree (a draw-time
    /// call would silently no-op its `ui` lookups).
    fn flush_resize_notifications(&mut self, cx: &mut Cx) {
        for (app_id, content) in std::mem::take(&mut self.pending_resize_notify) {
            let Some(host) = self.hosts.get(&app_id) else { continue };
            let splash = host.widget(cx, ids!(splash));
            if let Some(mut splash) = splash.borrow_mut::<Splash>() {
                splash.call_script_fn(
                    cx,
                    live_id!(on_app_resize),
                    &[content.x.into(), content.y.into()],
                );
            }
        }
    }

    // -------------------------------------------------------------------
    // Divider + chrome interaction
    // -------------------------------------------------------------------

    /// Handles presses on the divider (drag to resize, tap for the menu) and
    /// on each visible header's split button. Returns whether the event was
    /// consumed (an in-flight divider drag swallows finger moves).
    fn handle_chrome_hits(&mut self, cx: &mut Cx, event: &Event) -> bool {
        // The divider, when split and settled. The open menu sits ON the
        // divider; a press inside it must not even be hit-TESTED against the
        // divider — `hits()` captures the digit as a side effect, which would
        // starve the menu buttons of their click.
        let menu_shields_press = match event {
            Event::MouseDown(e) => {
                self.menu_open && self.last_menu_rect.contains(e.abs)
            }
            Event::TouchUpdate(e) => {
                self.menu_open
                    && e.touches.iter().any(|t| self.last_menu_rect.contains(t.abs))
            }
            _ => false,
        };
        if !self.is_split() {
            // A drag held while the split ended (back key, geometry exit)
            // must not survive into the next split session.
            self.drag = None;
        }
        if self.is_split() && !menu_shields_press {
            if let Some(divider) = self.divider.clone() {
                match event.hits(cx, divider.area()) {
                    Hit::FingerHoverIn(_) => {
                        let cursor = match self.axis {
                            SplitAxis::LeftRight => MouseCursor::EwResize,
                            SplitAxis::TopBottom => MouseCursor::NsResize,
                        };
                        cx.set_cursor(cursor);
                    }
                    Hit::FingerDown(fe) => {
                        if self.anim.is_none() {
                            self.drag = Some(DividerDrag { start_abs: fe.abs, moved: false });
                            return true;
                        }
                    }
                    Hit::FingerMove(fe) => {
                        if let Some(mut drag) = self.drag {
                            if (fe.abs - drag.start_abs).length() > DRAG_SLOP {
                                drag.moved = true;
                            }
                            self.drag = Some(drag);
                            if drag.moved {
                                self.menu_open = false;
                                self.sync_chrome_visibility(cx);
                                self.ratio = self.ratio_for_finger(fe.abs);
                                cx.redraw_all();
                            }
                            return true;
                        }
                    }
                    Hit::FingerUp(fe) => {
                        if let Some(drag) = self.drag.take() {
                            if drag.moved {
                                self.ratio = self.ratio_for_finger(fe.abs);
                                self.settle_divider(cx);
                            } else {
                                // A tap: toggle the divider menu.
                                self.menu_open = !self.menu_open;
                                self.sync_chrome_visibility(cx);
                            }
                            cx.redraw_all();
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Each visible header's split button.
        let visible: Vec<MiniAppId> = match &self.mode {
            Mode::Hidden => vec![],
            Mode::Single { app } => vec![app.clone()],
            Mode::Pick { a } => vec![a.clone()],
            Mode::Split { a, b } => vec![a.clone(), b.clone()],
        };
        for id in visible {
            let Some(host) = self.hosts.get(&id).cloned() else { continue };
            let area = host.widget(cx, ids!(split_button)).area();
            match event.hits(cx, area) {
                Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
                Hit::FingerUp(fe) => {
                    if fe.is_over {
                        // A press mid-zoom means the user wants the action,
                        // not a dropped click; snap the transition first.
                        self.finish_anim(cx);
                        self.split_button_pressed(cx, &id);
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// The header split button: dock to a pane (fullscreen), cancel the pick
    /// (pick mode), or fullscreen this pane's app (split).
    fn split_button_pressed(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        match self.mode.clone() {
            Mode::Single { app } if app == *app_id => {
                if self.split_span(self.best_axis()) < MIN_SPLIT_SPAN {
                    return;
                }
                self.axis = self.best_axis();
                self.ratio = 0.5;
                let starts = vec![(app.clone(), self.last_container)];
                self.mode = Mode::Pick { a: app };
                self.start_anim(cx, Anim::Morph { starts, hide_after: vec![] });
                cx.redraw_all();
            }
            Mode::Pick { a } if a == *app_id => self.cancel_pick(cx),
            Mode::Split { .. } => self.exit_split_to(cx, app_id.clone()),
            _ => {}
        }
    }

    /// Routes divider-menu button clicks; closes the menu on outside presses.
    fn handle_menu_actions(&mut self, cx: &mut Cx, event: &Event) {
        if !self.menu_open {
            return;
        }
        if let Some(menu) = self.divider_menu.clone() {
            if let Event::Actions(actions) = event {
                if menu.glass_button(cx, ids!(menu_swap)).clicked(actions) {
                    self.swap_panes(cx);
                    return;
                }
                if menu.glass_button(cx, ids!(menu_rotate)).clicked(actions) {
                    self.rotate_axis(cx);
                    return;
                }
                if menu.glass_button(cx, ids!(menu_full)).clicked(actions) {
                    // The larger pane wins: predictable, and matches what the
                    // divider position already says the user cares about.
                    if let Mode::Split { a, b } = self.mode.clone() {
                        let winner = if self.ratio >= 0.5 { a } else { b };
                        self.exit_split_to(cx, winner);
                    }
                    return;
                }
            }
        }
        // A press anywhere outside the menu (and off the divider, which
        // handles its own tap-toggle) dismisses it.
        let outside = |abs: Vec2d, this: &Self| {
            !this.last_menu_rect.contains(abs) && !this.last_divider_rect.contains(abs)
        };
        let dismissed = match event {
            Event::MouseDown(e) => outside(e.abs, self),
            Event::TouchUpdate(e) => e.touches.iter().any(|t| outside(t.abs, self)),
            _ => false,
        };
        if dismissed {
            self.menu_open = false;
            self.sync_chrome_visibility(cx);
            self.redraw(cx);
        }
    }

    /// Tracks which pane the user last pressed, without disturbing the hit
    /// system (raw event inspection, no capture).
    fn note_focus_press(&mut self, abs: Vec2d) {
        if let Mode::Split { a, b } = &self.mode {
            let (ra, rb, _) = self.pane_rects(self.last_container, self.draw_ratio());
            if ra.contains(abs) {
                self.focused = Some(a.clone());
            } else if rb.contains(abs) {
                self.focused = Some(b.clone());
            }
        }
    }
}

impl Widget for MiniAppScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Deliver queued on_app_resize calls FIRST, before any early return:
        // a close racing a queued notify must not strand the queue (stale
        // sizes would suppress the next real notification).
        if !self.pending_resize_notify.is_empty() {
            self.flush_resize_notifications(cx);
        }

        // Step the screen-level animation.
        if let Some(ne) = self.next_frame.is_event(event) {
            if self.anim.is_some() {
                self.step_anim(cx, ne.time);
            }
        }

        // Keep chrome visibility flags truthful for the snapshot/tests.
        self.sync_chrome_visibility(cx);
        self.sync_split_icons(cx);

        // A window shrink can leave a split without room for two panes;
        // fall back to the focused app fullscreen (or abandon a pick). The
        // event's own size is used — the drawn container is a frame stale.
        if let Event::WindowGeomChange(e) = event {
            let inner = e.new_geom.inner_size;
            let span = match self.axis {
                SplitAxis::LeftRight => inner.x - 8.0,
                SplitAxis::TopBottom => inner.y - 8.0,
            };
            if span < MIN_SPLIT_SPAN {
                if self.is_split() {
                    if let Some(focused) = self.focused_app() {
                        self.exit_split_to(cx, focused);
                    }
                } else if self.is_picking() {
                    self.cancel_pick(cx);
                }
            } else if self.is_split() {
                // Still fits, but a shrink can leave a settled pane below
                // MIN_PANE; clamp against the NEW span (last_container is a
                // frame stale here).
                let usable = (span - DIVIDER_GAP).max(1.0);
                let lo = (MIN_PANE / usable).min(0.5);
                self.ratio = self.ratio.clamp(lo, 1.0 - lo);
            }
        }

        if self.mode == Mode::Hidden {
            // Backgrounded apps still receive network responses so in-flight
            // requests complete; everything else is frozen while hidden.
            if let Event::NetworkResponses(_) = event {
                let hosts: Vec<WidgetRef> = self.hosts.values().cloned().collect();
                for host in hosts {
                    host.handle_event(cx, event, scope);
                }
            }
            return;
        }

        if let Event::MouseDown(e) = event {
            self.note_focus_press(e.abs);
        } else if let Event::TouchUpdate(e) = event {
            for t in &e.touches {
                self.note_focus_press(t.abs);
            }
        }

        // Divider drag / tap and split buttons come before the hosts so an
        // in-flight drag doesn't leak finger moves into pane content.
        if self.handle_chrome_hits(cx, event) {
            return;
        }
        self.handle_menu_actions(cx, event);
        if self.menu_open {
            if let Some(menu) = self.divider_menu.clone() {
                menu.handle_event(cx, event, scope);
            }
            // The menu floats over the panes; a press in its padding or the
            // gaps between its buttons must not fall through to whatever app
            // content happens to be underneath.
            let press_on_menu = match event {
                Event::MouseDown(e) => self.last_menu_rect.contains(e.abs),
                Event::TouchUpdate(e) => {
                    e.touches.iter().any(|t| self.last_menu_rect.contains(t.abs))
                }
                _ => false,
            };
            if press_on_menu {
                return;
            }
        }

        // Pick mode: the peeking sliver is a "bring me back" handle, not an
        // interactive app surface — a tap on it cancels the pick and restores
        // the app to fullscreen (its header, split button included, hangs
        // offscreen while docked). Raw event matching, not hits(): hits()
        // CAPTURES the digit on FingerDown even when the result is ignored.
        if self.is_picking() && !self.pick_obscured && self.anim.is_none() {
            let block = self.pick_block_rect();
            let press = match event {
                Event::MouseDown(e) => block.contains(e.abs),
                Event::TouchUpdate(e) => e.touches.iter().any(|t| block.contains(t.abs)),
                _ => false,
            };
            if press {
                self.cancel_pick(cx);
                return;
            }
        }

        // Forward events to the visible hosts only; backgrounded apps stay
        // frozen (except for network responses, delivered to all).
        let visible: Vec<MiniAppId> = match &self.mode {
            Mode::Hidden => vec![],
            Mode::Single { app } => vec![app.clone()],
            Mode::Pick { a } => vec![a.clone()],
            Mode::Split { a, b } => vec![a.clone(), b.clone()],
        };
        if let Event::NetworkResponses(_) = event {
            let hosts: Vec<WidgetRef> = self.hosts.values().cloned().collect();
            for host in hosts {
                host.handle_event(cx, event, scope);
            }
        } else {
            for id in &visible {
                if let Some(host) = self.hosts.get(id).cloned() {
                    host.handle_event(cx, event, scope);
                }
            }
        }

        // The close (×) in each visible header. An SDF view now (see the
        // AppHost template for why), so it's hit-tested like split_button
        // rather than reported through Button actions.
        for id in &visible {
            let Some(host) = self.hosts.get(id).cloned() else { continue };
            let area = host.widget(cx, ids!(back_button)).area();
            match event.hits(cx, area) {
                Hit::FingerHoverIn(_) => cx.set_cursor(MouseCursor::Hand),
                Hit::FingerUp(fe) => {
                    if !fe.is_over {
                        continue;
                    }
                    self.finish_anim(cx);
                    match self.mode.clone() {
                        Mode::Single { .. } => self.close_active(cx),
                        // Closing the docked app abandons the pick and returns
                        // to the (already visible) home screen.
                        Mode::Pick { .. } => self.close_to_home(cx),
                        Mode::Split { .. } => self.collapse_pane(cx, id.clone()),
                        Mode::Hidden => {}
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.mode == Mode::Hidden {
            return DrawStep::done();
        }
        cx.begin_turtle(walk, Layout::flow_overlay());
        let rect = cx.turtle().rect();

        // The fully-open container leaves a small margin so the backdrop peeks
        // through, keeping the glass-card look from the reference screenshots.
        let container = Rect {
            pos: rect.pos + dvec2(4.0, 4.0),
            size: rect.size - dvec2(8.0, 8.0),
        };
        self.last_container = container;

        // Target rect per visible app for the current mode.
        let ratio = self.draw_ratio();
        let mut targets: Vec<(MiniAppId, Rect)> = Vec::new();
        let mut divider_strip = None;
        match &self.mode {
            Mode::Hidden => {}
            Mode::Single { app } => targets.push((app.clone(), container)),
            Mode::Pick { a } => {
                // The docked app is SHIFTED offscreen (not resized): its
                // fullscreen-sized rect slides along the axis until a small
                // sliver peeks in, leaving home free for picking. While the
                // drawer/search overlay (drawn BELOW this widget) is up, the
                // sliver doesn't draw at all so the covering layer truly
                // fronts everything.
                if !self.pick_obscured {
                    targets.push((a.clone(), self.pick_dock_rect(container)));
                }
            }
            Mode::Split { a, b } => {
                let (ra, rb, strip) = self.pane_rects(container, ratio);
                targets.push((a.clone(), ra));
                targets.push((b.clone(), rb));
                divider_strip = Some(strip);
            }
        }
        if let Some((_, ra)) = targets.first() {
            self.last_rect_a = *ra;
        }

        // Apply the running transition on top of the targets.
        let eased = ease_out(self.t);
        let mut extra: Vec<(MiniAppId, Rect)> = Vec::new();
        match &self.anim {
            Some(Anim::Zoom { app, from, closing }) => {
                for (id, target) in &mut targets {
                    if id == app {
                        let (src, dst) = if *closing { (*target, *from) } else { (*from, *target) };
                        *target = Rect {
                            pos: src.pos + (dst.pos - src.pos) * eased,
                            size: src.size + (dst.size - src.size) * eased,
                        };
                    }
                }
            }
            Some(Anim::Morph { starts, hide_after }) => {
                for (id, target) in &mut targets {
                    if let Some((_, start)) = starts.iter().find(|(sid, _)| sid == id) {
                        *target = Rect {
                            pos: start.pos + (target.pos - start.pos) * eased,
                            size: start.size + (target.size - start.size) * eased,
                        };
                    }
                }
                // Panes leaving the screen (exit split) keep their captured
                // rect under the winner until the morph lands and hides them.
                for id in hide_after {
                    if targets.iter().all(|(tid, _)| tid != id) {
                        if let Some((_, start)) = starts.iter().find(|(sid, _)| sid == id) {
                            extra.push((id.clone(), *start));
                        }
                    }
                }
            }
            _ => {}
        }

        // Zooming hosts draw into their own overlay so the moving window
        // stays ABOVE the home widgets' glass tiles (see `zoom_layer`).
        let in_zoom_layer = self.is_zooming();
        if in_zoom_layer {
            if self.zoom_layer.is_none() {
                self.zoom_layer = Some(DrawList2d::new(cx));
            }
            self.zoom_layer.as_mut().unwrap().begin_overlay_reuse(cx);
        }

        // Losers first so the surviving/incoming pane composites on top.
        for (id, host_rect) in extra.iter().chain(targets.iter()) {
            let Some(host) = self.hosts.get(id) else { continue };
            let host = host.clone();
            let host_walk = Walk {
                abs_pos: Some(host_rect.pos),
                margin: Default::default(),
                width: Size::Fixed(host_rect.size.x),
                height: Size::Fixed(host_rect.size.y),
                metrics: Default::default(),
            };
            let mut scope = Scope::empty();
            host.draw_walk_all(cx, &mut scope, host_walk);
            // Tell the app's script about its new content box, but only once
            // settled: mid-zoom/morph every frame has a different size and
            // would burn the script's instruction budget on throwaway tiers.
            if self.anim.is_none() {
                self.note_host_size(cx, id, *host_rect);
            }
        }

        if in_zoom_layer {
            self.zoom_layer.as_mut().unwrap().end(cx);
        }

        // The divider (only while actually split; the pick gap needs no grip).
        if let Some(strip) = divider_strip {
            self.last_divider_rect = strip;
            if let Some(divider) =
                self.ensure_chrome(cx, live_id!(Divider), live_id!(split_divider))
            {
                let walk = Walk {
                    abs_pos: Some(strip.pos),
                    margin: Default::default(),
                    width: Size::Fixed(strip.size.x),
                    height: Size::Fixed(strip.size.y),
                    metrics: Default::default(),
                };
                let mut scope = Scope::empty();
                divider.draw_walk_all(cx, &mut scope, walk);
            }
        } else {
            self.last_divider_rect = Rect::default();
        }

        // Pick-mode hint, floating just past the docked sliver.
        if self.is_picking() && self.anim.is_none() && !self.pick_obscured {
            if let Some(hint) = self.ensure_chrome(cx, live_id!(PickHint), live_id!(pick_hint)) {
                // The hint spans the full width; its wrapper centers the pill,
                // so no width estimating here — only the height matters, for
                // sitting the pill ON TOP of the anchor.
                let h = hint.area().rect(cx).size.y;
                let h = if h > 1.0 { h } else { 34.0 };
                // Home's page-indicator dots: the hint tucks into the gap just
                // above them, clear of the grid and of the docked sliver.
                let (pos, span) = if self.hint_anchor.size.y > 0.5 {
                    (
                        dvec2(container.pos.x, self.hint_anchor.pos.y - h - HINT_GAP),
                        container.size.x,
                    )
                } else {
                    match self.axis {
                        SplitAxis::TopBottom => (
                            dvec2(container.pos.x, container.pos.y + PICK_PEEK + 14.0),
                            container.size.x,
                        ),
                        SplitAxis::LeftRight => (
                            dvec2(container.pos.x + PICK_PEEK, container.pos.y + 18.0),
                            container.size.x - PICK_PEEK,
                        ),
                    }
                };
                let walk = Walk {
                    abs_pos: Some(pos),
                    margin: Default::default(),
                    width: Size::Fixed(span),
                    height: Size::fit(),
                    metrics: Default::default(),
                };
                let mut scope = Scope::empty();
                hint.draw_walk_all(cx, &mut scope, walk);
            }
        }

        // The divider menu, floating beside the divider's center.
        if self.menu_open && self.is_split() {
            if let Some(menu) = self.ensure_chrome(cx, live_id!(DividerMenu), live_id!(split_menu))
            {
                let strip = self.last_divider_rect;
                let center = strip.center();
                let size = menu.area().rect(cx).size;
                let est = if size.x > 1.0 { size } else { dvec2(232.0, 48.0) };
                let pos = dvec2(
                    (center.x - est.x * 0.5)
                        .clamp(container.pos.x + 8.0, container.pos.x + container.size.x - est.x - 8.0),
                    (center.y - est.y * 0.5)
                        .clamp(container.pos.y + 8.0, container.pos.y + container.size.y - est.y - 8.0),
                );
                self.last_menu_rect = Rect { pos, size: est };
                let walk = Walk {
                    abs_pos: Some(pos),
                    margin: Default::default(),
                    width: Size::fit(),
                    height: Size::fit(),
                    metrics: Default::default(),
                };
                let mut scope = Scope::empty();
                menu.draw_walk_all(cx, &mut scope, walk);
            }
        } else {
            self.last_menu_rect = Rect::default();
        }

        cx.end_turtle();
        DrawStep::done()
    }
}

/// The cubic ease-out shared by every transition here.
fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

impl MiniAppScreenRef {
    pub fn is_showing(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_showing())
    }

    pub fn covers_home(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.covers_home())
    }

    pub fn is_zooming(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_zooming())
    }

    pub fn is_fully_open(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_fully_open())
    }

    pub fn is_picking(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_picking())
    }

    pub fn is_split(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_split())
    }

    pub fn pick_block_rect(&self) -> Rect {
        self.borrow()
            .map(|inner| inner.pick_block_rect())
            .unwrap_or_default()
    }

    /// Takes over a live host that is already running elsewhere (a home-grid
    /// tile) and shows it fullscreen. The widget is NOT rebuilt: the same
    /// isolate keeps running, mid-animation, mid-state — which is the whole
    /// point of expanding a tile rather than launching a copy. The lender
    /// keeps its own reference, so releasing this one can't kill the isolate.
    pub fn adopt_host(
        &self,
        cx: &mut Cx,
        app_id: &MiniAppId,
        host: WidgetRef,
        from_rect: Rect,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.adopt_host(cx, app_id, host, from_rect);
        }
    }

    /// Gives an adopted host back: drops it from this screen without any
    /// teardown (the lender's reference keeps the isolate alive).
    pub fn release_adopted(&self, cx: &mut Cx) -> Option<MiniAppId> {
        self.borrow_mut().and_then(|mut inner| inner.release_adopted(cx))
    }

    /// The app id of the host currently on loan from a home tile, if any.
    pub fn adopted(&self) -> Option<MiniAppId> {
        self.borrow().and_then(|inner| inner.adopted.clone())
    }

    pub fn set_pick_obscured(&self, cx: &mut Cx, obscured: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_pick_obscured(cx, obscured);
        }
    }

    pub fn set_hint_anchor(&self, cx: &mut Cx, rect: Rect) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_hint_anchor(cx, rect);
        }
    }

    pub fn is_running(&self, app_id: &MiniAppId) -> bool {
        self.borrow().is_some_and(|inner| inner.is_running(app_id))
    }

    pub fn split_state(&self) -> Option<(MiniAppId, MiniAppId, SplitAxis, f64)> {
        self.borrow().and_then(|inner| inner.split_state())
    }

    pub fn open_app(&self, cx: &mut Cx, manifest: &MiniAppManifest, from_rect: Rect) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open_app(cx, manifest, from_rect);
        }
    }

    pub fn enter_split_pick(
        &self,
        cx: &mut Cx,
        manifest: &MiniAppManifest,
        from_rect: Option<Rect>,
        screen: Vec2d,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.enter_split_pick(cx, manifest, from_rect, screen);
        }
    }

    pub fn close_active(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close_active(cx);
        }
    }

    pub fn close_to_home(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close_to_home(cx);
        }
    }

    pub fn handle_back(&self, cx: &mut Cx) -> bool {
        if let Some(mut inner) = self.borrow_mut() {
            inner.handle_back(cx)
        } else {
            false
        }
    }

    pub fn force_stop(&self, cx: &mut Cx, app_id: &MiniAppId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.force_stop(cx, app_id);
        }
    }
}
