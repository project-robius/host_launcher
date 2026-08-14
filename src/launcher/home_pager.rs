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

use makepad_widgets::makepad_platform::event::TouchState;
use makepad_widgets::{widget_tree::CxWidgetExt, *};

use crate::{
    app::AppState,
    launcher::notif_badge::NotifBadgeWidgetRefExt,
    mini_apps::registry::{
        HomePage, LauncherLayout, MAX_PAGES, MiniAppId, PlacedItem, PlacedKind,
        WidgetInstanceId,
    },
};

/// A resize drag started by grabbing a widget's Android resize handle while its
/// context menu is open. Tracked with raw pointer events so it works even while
/// the (modal) menu is on top; the widget snaps to whole cells while the white
/// border follows the finger.
#[derive(Clone)]
struct MenuResize {
    instance: WidgetInstanceId,
    page: usize,
    col: u8,
    row: u8,
    /// The widget's span when the drag began (basis for the cell-snap delta).
    start_span: (u8, u8),
    /// The bottom-right corner the finger grabbed (basis for the cell-snap delta).
    handle_start: Vec2d,
    /// Live finger position, so the border corner tracks the finger continuously.
    finger: Vec2d,
}

/// The pointer phase of a raw mouse/touch event, unified across desktop + touch.
#[derive(Clone, Copy, PartialEq)]
enum PointerPhase {
    Down,
    Move,
    Up,
}

/// Extracts a unified (position, phase) from a raw pointer event, ignoring
/// non-primary mouse buttons. Used for the menu resize drag, which must see
/// events even while a modal captures the normal hit-tested input.
fn pointer_event(event: &Event) -> Option<(Vec2d, PointerPhase)> {
    match event {
        Event::MouseDown(e) if e.button.is_primary() => Some((e.abs, PointerPhase::Down)),
        Event::MouseMove(e) => Some((e.abs, PointerPhase::Move)),
        Event::MouseUp(e) if e.button.is_primary() => Some((e.abs, PointerPhase::Up)),
        Event::TouchUpdate(e) => e.touches.first().map(|t| {
            let phase = match t.state {
                TouchState::Start => PointerPhase::Down,
                TouchState::Stop => PointerPhase::Up,
                TouchState::Move | TouchState::Stable => PointerPhase::Move,
            };
            (t.abs, phase)
        }),
        _ => None,
    }
}

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

    // The resize drag handle on a widget tile's bottom-right corner in edit
    // mode: a big, glassy accent-blue disc with a bold two-headed diagonal
    // arrow, Android-style, sized generously so it's easy to touch and drag.
    // Coordinates are relative to the quad size, so the handle scales cleanly.
    set_type_default() do #(DrawResizeGrip::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn(){
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let c = self.rect_size * 0.5
            let r = c.x - 3.0
            // Soft drop shadow so the handle floats above the widget.
            sdf.blur = 3.5
            sdf.circle(c.x, c.y + 1.5, r)
            sdf.fill(vec4(0.0, 0.0, 0.0, 0.35))
            sdf.blur = 0.0
            // Glassy accent-blue disc with a crisp light rim.
            sdf.circle(c.x, c.y, r)
            sdf.fill(vec4(0.14, 0.40, 0.78, 0.96))
            sdf.circle(c.x, c.y, r)
            sdf.stroke(vec4(1.0, 1.0, 1.0, 0.9), 1.4)
            // Bold double-headed diagonal arrow (bottom-left <-> top-right).
            let a = r * 0.44
            let h = r * 0.30
            sdf.move_to(c.x - a, c.y + a)
            sdf.line_to(c.x + a, c.y - a)
            // Top-right arrowhead.
            sdf.move_to(c.x + a - h, c.y - a)
            sdf.line_to(c.x + a, c.y - a)
            sdf.line_to(c.x + a, c.y - a + h)
            // Bottom-left arrowhead.
            sdf.move_to(c.x - a + h, c.y + a)
            sdf.line_to(c.x - a, c.y + a)
            sdf.line_to(c.x - a, c.y + a - h)
            sdf.stroke(vec4(1.0, 1.0, 1.0, 1.0), 2.0)
            return sdf.result
        }
    }

    // The Android-style widget resize indicator, drawn around a widget while its
    // context menu is open: a rounded selection outline just outside the widget plus
    // an obvious curved white grab handle bulging out of the bottom-right corner.
    // Drawn as a quad inflated around the widget rect (see RESIZE_FRAME_INFLATE);
    // coordinates are pixels within that quad.
    set_type_default() do #(DrawResizeFrame::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn(){
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let sz = self.rect_size
            // The quad is the widget rect inflated by RESIZE_FRAME_INFLATE on every
            // side; the outline sits `pad` in from the quad edge, leaving a visible
            // gap between the widget and the outline (Android-style), and leaving
            // room outside the outline for the corner handle to bulge into.
            let pad = 11.0
            let x0 = pad
            let y0 = pad
            let w = sz.x - pad * 2.0
            let h = sz.y - pad * 2.0
            // Light, thin rounded selection outline (sdf.box doubles the radius, so
            // 11 -> ~22px visual corners like the reference).
            let vr = 22.0
            sdf.box(x0, y0, w, h, vr * 0.5)
            sdf.stroke(vec4(1.0, 1.0, 1.0, 0.55), 1.5)
            // Curved grab handle: a thick white quarter-circle arc bulging out of the
            // bottom-right corner, traced as a polyline over the rounded corner's
            // centre (from east round to south) with a soft shadow so it reads on any
            // wallpaper. Endpoints land on the right/bottom edges; the belly pushes
            // past the 45-degree corner.
            let ccx = x0 + w - vr
            let ccy = y0 + h - vr
            let ar = vr + 2.0
            // Shadow pass (blurred, dark), then the crisp white arc over it.
            sdf.blur = 3.0
            sdf.move_to(ccx + ar * 1.000, ccy + ar * 0.000)
            sdf.line_to(ccx + ar * 0.966, ccy + ar * 0.259)
            sdf.line_to(ccx + ar * 0.866, ccy + ar * 0.500)
            sdf.line_to(ccx + ar * 0.707, ccy + ar * 0.707)
            sdf.line_to(ccx + ar * 0.500, ccy + ar * 0.866)
            sdf.line_to(ccx + ar * 0.259, ccy + ar * 0.966)
            sdf.line_to(ccx + ar * 0.000, ccy + ar * 1.000)
            sdf.stroke(vec4(0.0, 0.0, 0.0, 0.30), 6.0)
            sdf.blur = 0.0
            sdf.move_to(ccx + ar * 1.000, ccy + ar * 0.000)
            sdf.line_to(ccx + ar * 0.966, ccy + ar * 0.259)
            sdf.line_to(ccx + ar * 0.866, ccy + ar * 0.500)
            sdf.line_to(ccx + ar * 0.707, ccy + ar * 0.707)
            sdf.line_to(ccx + ar * 0.500, ccy + ar * 0.866)
            sdf.line_to(ccx + ar * 0.259, ccy + ar * 0.966)
            sdf.line_to(ccx + ar * 0.000, ccy + ar * 1.000)
            sdf.stroke(vec4(1.0, 1.0, 1.0, 1.0), 5.0)
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
            // Both Fill because this box IS the grid cell, not the icon: the
            // pager draws it with an explicit `Walk` sized to the cell
            // (`draw_walk_all`), which overrides whatever is written here — so
            // `height: Fit` would read as a promise the layout never keeps.
            // Filling the cell is also what makes `align.y` below mean
            // anything: shrink this box to its content and there is no slack
            // left to align within, and the offset silently does nothing.
            //
            // The `Fit` box is `icon_group` inside — that's the icon-shaped
            // thing whose rect is worth measuring, and it's what hit-testing
            // and the context menu read.
            width: Fill
            height: Fill
            flow: Down
            // Above centre, but not flush. Centring (0.5) balances the
            // [tile + label] block in the cell, so a name that wraps to two
            // lines makes the block taller and shoves ITS icon upward — in a
            // row of one-line names, that icon then sits higher than its
            // neighbours. Pinning it flush to the top fixed that but left the
            // icons sitting hard against the cell edge. A fraction keeps every
            // tile on the same line regardless of label height, with air above.
            //
            // Keep `ICON_GROUP_ALIGN_Y` in step with this number — the hit test
            // and the menu anchor locate the group with it.
            align: Align{x: 0.5, y: 0.25}
            // Don't clip (cut off) the notification badge overhanging the tile.
            clip_x: false, clip_y: false
            // The icon and its label in a `Fit` box of their own, so the group
            // has a real rect that already accounts for a one- OR two-line
            // name. Hit-testing and the context-menu anchor just read it —
            // reconstructing the group from the tile and the label meant
            // guessing at wrapped text, because a Label's drawn rect reports a
            // single line's height either way.
            icon_group := View{
                width: Fill
                height: Fit
                flow: Down
                spacing: 5
                align: Align{x: 0.5}
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
        }

        // A live app running in its grid cells. The SAME template the app
        // screen uses, so expanding a tile hands the running widget straight
        // over (same isolate, same state) instead of starting a second one.
        AppTile := mod.widgets.AppHost{}

        // Shown in the cells while that app is expanded to fullscreen. The
        // running widget is on loan to the app screen and can't draw in two
        // places at once, so the block keeps the app's face (icon + name)
        // under a grey "disabled" scrim, with the way back written on it.
        // Tapping anywhere on the card returns the app to its cells.
        AppTileAway := RoundedView{
            width: Fill
            height: Fill
            flow: Overlay
            show_bg: true
            draw_bg +: {
                color: #x070c16f4
                border_color: #xffffff1e
                border_size: 1.0
                border_radius: 20.0
            }
            // The app's own face, dimmed: what the tile was showing.
            View{
                width: Fill
                height: Fill
                flow: Down
                spacing: 6
                align: Align{x: 0.5, y: 0.5}
                away_glyph := Label{
                    text: ""
                    padding: 0
                    margin: 0
                    draw_text +: {
                        color: #xffffff66
                        text_style: theme.font_regular{font_size: 34}
                    }
                }
                away_name := Label{
                    text: ""
                    padding: 0
                    margin: 0
                    draw_text +: {
                        color: #xffffff55
                        text_style: theme.font_bold{font_size: 12}
                    }
                }
            }
            // The disabled scrim, over the app's face.
            View{
                width: Fill
                height: Fill
                show_bg: true
                draw_bg +: { color: #x10151ecc }
            }
            // ...and the explanation on top of that.
            View{
                width: Fill
                height: Fill
                align: Align{x: 0.5, y: 0.5}
                padding: 12
                away_label := Label{
                    width: Fill
                    height: Fit
                    align: Align{x: 0.5}
                    margin: 0
                    padding: 0
                    text: "Open full screen — tap to bring it back here"
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 11}
                    }
                }
            }
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
                padding: Inset{right: 4, bottom: 4}
                clip_x: false, clip_y: false
                grip := mod.widgets.LauncherGrip{
                    visible: false
                    width: 34
                    height: 34
                }
            }
        }
    }
}

/// Finger movement below this (in points) still counts as a tap / stationary press.
const TAP_SLOP: f64 = 8.0;
/// How far outside the pager's own rect a drag in flight keeps its finger, so the
/// sweep hit-test doesn't cut the drag short at the pager's edge. Comfortably
/// larger than any screen, since the drag only ends on a real release.
const DRAG_HIT_SLACK: f64 = 10_000.0;
/// Side margin the grid lays its cells out within, so a full-width widget stops
/// short of the screen edge instead of running into it — a little further in than
/// the dock's own glass pill (8pt, see `HomeScreen`), which reads as the visual
/// margin for the whole home screen. Only *placement* is inset: the pager's own
/// rect stays full-width, so drags and the resize indicator can still use the
/// margins.
const GRID_EDGE_INSET: f64 = 12.0;
/// How long a press must be held (secs) to count as a long press on desktop.
const LONG_PRESS_SECS: f64 = 0.5;
/// Upward movement past this many points requests the app drawer.
const SWIPE_UP_DISTANCE: f64 = 36.0;
/// Width of the left/right screen-edge zones that flip pages while dragging. Kept
/// well inside a single column: a wider zone swallows most of the outermost cell,
/// so simply placing an item in the first or last column starts a page turn.
const EDGE_FLIP_ZONE: f64 = 26.0;
/// How long a dragged item must hover in an edge zone before the page flips (and
/// the interval between repeats while it's held there). Long enough that brushing
/// the edge on the way to a cell doesn't turn the page — it takes a deliberate
/// pause to ask for one.
const EDGE_FLIP_SECS: f64 = 0.75;
/// Resistance applied when panning past the first/last page.
const RUBBER_BAND_FACTOR: f64 = 0.35;
/// Height of an app's icon+label group — measured, not assumed: the 56px tile
/// plus the 5px spacing plus the label (two lines of 11pt bold since the
/// label bump; was 80.0 at 9.5pt). It is what the context menu anchors to AND
/// what counts as a hit on the icon; everything else in the cell is
/// background.
const ICON_GROUP_H: f64 = 86.0;
/// Where that group sits in its cell, as a fraction of the leftover space.
/// MUST match the `AppIcon` DSL's `align.y` — the hit test and the menu anchor
/// both locate the group with it, and a mismatch puts the tappable band off
/// the icon by however far the two disagree.
const ICON_GROUP_ALIGN_Y: f64 = 0.25;
/// Size of the remove badge hit target in edit mode.
const BADGE_HIT_SIZE: f64 = 26.0;
/// Size of the widget resize-handle hit target in edit mode. Generous so the
/// big corner drag handle is easy to grab with a fingertip.
const RESIZE_HIT_SIZE: f64 = 40.0;
/// Hit radius (from the widget's bottom-right corner) for grabbing the Android
/// resize handle shown alongside a widget's context menu. Generous for touch.
const MENU_RESIZE_HIT: f64 = 52.0;
/// Inset applied to multi-cell widget tiles so neighbours have breathing room.
const WIDGET_GAP: f64 = 6.0;
/// Padding the WidgetTile template puts around its Splash content (keep in sync
/// with the `content` view's `padding` in the DSL below).
const TILE_CONTENT_PAD: f64 = 10.0;
/// Chrome eaten by a live app tile, mirroring `mod.widgets.AppHost`: the host's
/// own 6pt padding on both sides plus the content view's 8pt sides...
const APP_TILE_PAD_X: f64 = 6.0 + 6.0 + 8.0 + 8.0;
/// ...and vertically the same 6+6 plus the 26pt tile bar and the content's
/// 4pt top / 8pt bottom. Keep both in step with the template.
const APP_TILE_PAD_Y: f64 = 6.0 + 6.0 + 26.0 + 4.0 + 8.0;

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

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawResizeFrame {
    #[deref]
    draw_super: DrawQuad,
}

/// How far the resize-indicator quad is inflated beyond the widget's rect on each
/// side (room for the outline gap + the overhanging corner handle).
const RESIZE_FRAME_INFLATE: f64 = 17.0;
/// Taken off the frame's BOTTOM inflate only, so it doesn't hang into the row
/// underneath (the top keeps the full inflate — there's slack up there).
const RESIZE_FRAME_BOTTOM_TRIM: f64 = 5.0;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKey {
    /// Keyed by per-placement instance (NOT app id) so duplicate icons of the same
    /// app are distinct items.
    App(WidgetInstanceId),
    Widget(WidgetInstanceId),
}

impl PlacedItem {
    fn key(&self) -> ItemKey {
        match &self.kind {
            PlacedKind::App { instance, .. } => ItemKey::App(*instance),
            PlacedKind::Widget { instance, .. } => ItemKey::Widget(*instance),
        }
    }
}

/// Actions emitted by the HomePager for the app to handle.
#[derive(Clone, Debug, Default)]
pub enum HomePagerAction {
    /// An app icon was tapped; open the app, animating out from `from_rect`.
    OpenApp { app_id: MiniAppId, from_rect: Rect },
    /// A dragged icon was released over the dock; put it in the dock at `index`
    /// (the item has already been lifted out of the grid).
    DropIntoDock { app_id: MiniAppId, index: usize },
    /// The user swiped up on the home screen; open the app drawer.
    OpenDrawer,
    /// An upward drag is in progress; drive the drawer to this open fraction (0..1).
    DragDrawer { progress: f64 },
    /// The upward drag ended; snap the drawer open or closed.
    ReleaseDrawer { open: bool },
    /// The user swiped down on the home screen; open Spotlight search.
    OpenSearch,
    /// A long-press landed on an item; show its shortcut menu at `anchor`.
    /// `home_instance` is the placement instance of the specific home icon the menu
    /// was opened on, so "Remove from Home" removes that one icon (not every copy).
    ShowContextMenu {
        app_id: MiniAppId,
        widget_instance: Option<WidgetInstanceId>,
        home_instance: Option<WidgetInstanceId>,
        anchor: Rect,
    },
    /// A live home tile's ⤢ button: hand this app's RUNNING widget to the app
    /// screen so it fills the display without restarting. `from_rect` is the
    /// tile's rect, for the zoom.
    ExpandAppTile {
        instance: WidgetInstanceId,
        app_id: MiniAppId,
        from_rect: Rect,
    },
    /// A live home tile's shrink button: back to a plain 1x1 icon.
    ShrinkAppTile { instance: WidgetInstanceId },
    /// The stand-in's "Bring back" button: pull the app out of fullscreen and
    /// return it to its cells.
    ReturnAppTile { instance: WidgetInstanceId },
    /// The finger started moving after a long-press; dismiss any open menu so the
    /// gesture can become a drag.
    HidePopups,
    /// The user right-clicked empty home-screen space; show the background menu.
    ShowBackgroundMenu { abs: Vec2d },
    /// The page position or page count changed (continuous during swipes).
    PageChanged { position: f64, count: usize },
    /// The remove (×) badge was tapped in edit mode; the app should confirm
    /// before actually removing `item` (labelled `label` for the prompt).
    RequestRemove { item: ItemKey, label: String },
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
    /// True when this is a fresh app being dragged in from the drawer (not moved
    /// from an existing cell): an invalid drop discards it, a valid drop adds it.
    is_new: bool,
    /// Set when the drag was lifted out of the dock, holding the slot it came
    /// from. An invalid drop puts it back there instead of discarding it — the
    /// app was already removed from the dock when the drag began, so dropping it
    /// on nothing would otherwise silently un-favourite it.
    from_dock: Option<usize>,
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
    /// Instantiated icon widgets, keyed by placement instance (so duplicate icons
    /// of the same app are separate widgets).
    #[rust]
    icons: HashMap<WidgetInstanceId, WidgetRef>,
    /// Overlay draw-list the lifted/dragged item renders into, so it floats ABOVE
    /// the widget tiles' glass lens overlays instead of being occluded when dragged
    /// over a widget.
    #[rust]
    drag_layer: Option<DrawList2d>,
    /// Instantiated widget tiles, one per placed widget instance.
    #[rust]
    tiles: HashMap<WidgetInstanceId, WidgetRef>,
    /// Live app hosts running in grid cells, keyed by PLACEMENT instance (so
    /// two placements of one app are two independent instances). These are
    /// full AppHost widgets — the very same template the app screen hosts —
    /// which is what lets `expand` hand one over without restarting it.
    #[rust]
    app_tiles: HashMap<WidgetInstanceId, WidgetRef>,
    /// The "running full screen" stand-ins, keyed the same way.
    #[rust]
    app_aways: HashMap<WidgetInstanceId, WidgetRef>,
    /// The placement whose widget is currently on loan to the app screen.
    /// Its cells draw the stand-in instead.
    #[rust]
    expanded_app: Option<WidgetInstanceId>,
    /// Content size each live app tile was last told it has, so
    /// `on_app_resize` only fires on real changes (0.5px epsilon).
    #[rust]
    app_tile_sizes: HashMap<WidgetInstanceId, Vec2d>,
    /// Tile-bar rects from the last draw, for grab/long-press hit-testing.
    #[rust]
    app_bar_rects: HashMap<WidgetInstanceId, Rect>,
    /// The resize frame's un-inflated rect from the last draw, so the corner
    /// grab region lines up with the border the user can see.
    #[rust]
    last_resize_rect: Option<Rect>,
    /// Expand-button rects from the last draw. (Shrinking lives in the
    /// long-press menu instead — a second bar button crowded the title.)
    #[rust]
    app_btn_rects: HashMap<WidgetInstanceId, Rect>,
    /// Stand-in card rects from the last draw; tapping one brings its app back.
    #[rust]
    app_away_rects: HashMap<WidgetInstanceId, Rect>,
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
    #[live]
    draw_resize: DrawResizeFrame,
    /// The widget instance to show the Android resize indicator around (set while its
    /// context menu is open); the indicator also shows during an active resize.
    #[rust]
    resize_hint: Option<WidgetInstanceId>,
    /// Where each app icon was actually DRAWN this frame: (page, item index,
    /// icon+label rect). Hit-testing reads this instead of recomputing the
    /// group's position from the cell, so the tappable area is exactly the
    /// pixels the user can see and can't drift from the layout.
    #[rust]
    icon_hits: Vec<(usize, usize, Rect)>,
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
    /// The resized page's items as they were when the current resize gesture
    /// began. Each frame restores from this before applying the reflow, so
    /// shrinking a widget releases the neighbours it pushed while growing.
    #[rust]
    resize_pristine: Option<(usize, Vec<PlacedItem>)>,
    /// An in-progress resize started by grabbing a widget's Android resize handle
    /// from its context menu (see `handle_menu_resize`).
    #[rust]
    menu_resize: Option<MenuResize>,
    /// One-shot latch per widget press: set once we've handed the finger up from
    /// an interactive widget child to the pager so a pan/drag can proceed (see
    /// `handle_widget_takeover`). Reset on each new press.
    #[rust]
    widget_takeover_done: bool,
    /// The dock's on-screen rect, mirrored from AppState each event so drag
    /// targeting can tell "over the dock" from "over the grid".
    #[rust]
    dock_rect: Rect,
    /// While a drag hovers the dock, the slot it would drop into. Published back
    /// to the dock (via AppState) so it can open a gap there.
    #[rust]
    dock_hover: Option<usize>,
    /// The floating create bar's rect (zero when hidden), mirrored from
    /// AppState; presses starting inside it belong to the bar, not the grid.
    #[rust]
    create_rect: Rect,
    /// The split-pick docked pane's rect (zero unless picking), mirrored from
    /// AppState; presses inside it belong to the docked app, not the grid.
    #[rust]
    split_block_rect: Rect,
    /// Mirrored from AppState: tiles aren't drawn while a mini-app pane
    /// exists, so their cells must not hit-test either.
    #[rust]
    hide_widget_tiles: bool,
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
        for (instance, child) in self.icons.iter() {
            visit(LiveId::from_str_num("appicon", *instance), child.clone());
        }
        for (instance, child) in self.tiles.iter() {
            visit(LiveId::from_str_num("wtile", *instance), child.clone());
        }
        for (instance, child) in self.app_tiles.iter() {
            visit(LiveId::from_str_num("atile", *instance), child.clone());
        }
        for (instance, child) in self.app_aways.iter() {
            visit(LiveId::from_str_num("aaway", *instance), child.clone());
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
    /// The cell area: the pager's rect inset by `GRID_EDGE_INSET` on each side.
    rect: Rect,
    /// Distance between consecutive pages — the pager's *full* width, not the
    /// inset one, so the side margins don't eat into the gap between pages.
    page_stride: f64,
    cell: Vec2d,
    grid: (u8, u8),
}

impl Geom {
    /// Top-left of the given page's cell area, offset by the current continuous
    /// page position. Drawing and hit-testing both go through this so a page's
    /// contents and its cell math can't drift apart.
    fn page_origin(&self, page: f64, page_pos: f64) -> Vec2d {
        dvec2(
            self.rect.pos.x + (page - page_pos) * self.page_stride,
            self.rect.pos.y,
        )
    }

    /// The rect of a cell span on the given page, in absolute coords,
    /// offset by the current continuous page position.
    fn cell_rect(&self, page: usize, page_pos: f64, col: u8, row: u8, span: (u8, u8)) -> Rect {
        let origin = self.page_origin(page as f64, page_pos);
        Rect {
            pos: dvec2(
                origin.x + col as f64 * self.cell.x,
                origin.y + row as f64 * self.cell.y,
            ),
            size: dvec2(self.cell.x * span.0 as f64, self.cell.y * span.1 as f64),
        }
    }

    /// Which (col, row) cell contains the given absolute position on the
    /// currently-centered page, if any.
    fn cell_at(&self, page_pos: f64, abs: Vec2d) -> Option<(u8, u8)> {
        let local = abs - self.page_origin(page_pos.round(), page_pos);
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
        // Cells live inside the side margins; the pager's own rect (used for the
        // swipe physics and for clamping the resize indicator) stays full-width.
        let rect = Rect {
            pos: dvec2(self.last_rect.pos.x + GRID_EDGE_INSET, self.last_rect.pos.y),
            size: dvec2(
                (self.last_rect.size.x - 2.0 * GRID_EDGE_INSET).max(1.0),
                self.last_rect.size.y,
            ),
        };
        Geom {
            rect,
            page_stride: self.last_rect.size.x,
            cell: dvec2(rect.size.x / grid.0 as f64, rect.size.y / grid.1 as f64),
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
        let idx = items.iter().position(|item| item.covers(col, row))?;
        // An app icon owns exactly the pixels it drew — its tile and its label
        // — NOT the whole cell. A widget does fill its cell block, so it keeps
        // the cell. Treating the cell as the icon meant the empty band beneath
        // a label belonged to the icon, which is most of the grid, and made
        // long-pressing the background to reach jiggle mode all but impossible.
        // Only a 1x1 icon owns just its drawn glyph+label; a grown (running)
        // app fills its cell block, so it keeps the cell like a widget does.
        if matches!(items[idx].kind, PlacedKind::App { cols: 1, rows: 1, .. }) {
            let point = dvec2(abs.x, abs.y);
            return self
                .icon_hits
                .iter()
                .find(|(p, i, r)| *p == page && *i == idx && r.contains(point))
                .map(|_| (page, idx));
        }
        // A hidden tile (mini-app pane up) is not there to be tapped or
        // long-pressed; its cell reads as background.
        if self.hide_widget_tiles {
            return None;
        }
        Some((page, idx))
    }

    /// The icon+label group inside an app's cell — the part that is actually
    /// the icon, for both hit-testing and menu anchoring so the two agree.
    fn icon_group_rect(cell: Rect) -> Rect {
        let h = ICON_GROUP_H.min(cell.size.y);
        Rect {
            pos: dvec2(
                cell.pos.x,
                cell.pos.y + (cell.size.y - h) * ICON_GROUP_ALIGN_Y,
            ),
            size: dvec2(cell.size.x, h),
        }
    }

    /// Builds the `ShowContextMenu` action for the given item key, anchoring the
    /// menu to the item's on-screen cell. Returns `None` if the item isn't found.
    fn menu_action_for(&self, layout: &LauncherLayout, item: ItemKey) -> Option<HomePagerAction> {
        let (p, idx, placed) = layout.pages.iter().enumerate().find_map(|(p, page)| {
            page.items
                .iter()
                .position(|it| it.key() == item)
                .map(|idx| (p, idx, page.items[idx].clone()))
        })?;
        let geom = self.geom();
        let cell = geom.cell_rect(p, self.page_pos, placed.col, placed.row, placed.span());
        let (app_id, widget_instance) = match &placed.kind {
            PlacedKind::App { id, .. } => (id.clone(), None),
            PlacedKind::Widget { app_id, instance, .. } => (app_id.clone(), Some(*instance)),
        };
        // The specific placement instance, so an app icon's "Remove from Home"
        // removes just this icon rather than every duplicate of the app.
        let home_instance = match &placed.kind {
            PlacedKind::App { instance, .. } => Some(*instance),
            PlacedKind::Widget { .. } => None,
        };
        // Anchor the menu to the tight icon+label group (centred in the cell), not
        // the padded cell, so the menu sits directly against it and the callout
        // lines up with the icon. Widgets fill their whole cell block, so they use
        // it as-is.
        let anchor = match &placed.kind {
            // The rect the icon ACTUALLY drew into, captured during the last
            // frame — so the menu hangs off the real bottom of the label
            // whether that label is one line or two. A fixed group height
            // assumed one line, and a wrapped name ("Fitness Tracker") then
            // ran straight through the menu's callout.
            PlacedKind::App { .. } => {
                // The group's own rect — no slack, no cell arithmetic.
                self.icon_hits
                    .iter()
                    .find(|(hp, hi, _)| *hp == p && *hi == idx)
                    .map(|(_, _, r)| *r)
                    .unwrap_or_else(|| Self::icon_group_rect(cell))
            }
            PlacedKind::Widget { .. } => cell,
        };
        Some(HomePagerAction::ShowContextMenu {
            app_id,
            widget_instance,
            home_instance,
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

    /// Ensures a child icon widget exists (keyed by placement `instance`, so the
    /// same app can have several icons) and is configured for the given app.
    fn ensure_icon(
        &mut self,
        cx: &mut Cx,
        state: &AppState,
        instance: WidgetInstanceId,
        app_id: &MiniAppId,
    ) -> Option<WidgetRef> {
        if let Some(icon) = self.icons.get(&instance) {
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
        cx.widget_tree_insert_child_deep(
            self.uid,
            LiveId::from_str_num("appicon", instance),
            icon.clone(),
        );
        // New children need the current edit-mode chrome applied explicitly;
        // sync_edit_visuals only touches children on a mode *change*.
        icon.widget(cx, ids!(badge)).set_visible(cx, self.edit_visuals_applied);
        icon.notif_badge(cx, ids!(notif))
            .set_count(state.notifications.get(app_id).copied().unwrap_or(0));
        self.icons.insert(instance, icon.clone());
        Some(icon)
    }

    /// Ensures a widget tile exists for the given placed widget instance,
    /// evaluating its Splash source on first creation.
    /// Instantiates (or returns) the live app host for a grown app placement.
    /// It is a full `AppHost` — the same template the app screen uses — with
    /// the fullscreen header swapped for the compact tile bar.
    fn ensure_app_tile(
        &mut self,
        cx: &mut Cx,
        state: &AppState,
        instance: WidgetInstanceId,
        app_id: &MiniAppId,
    ) -> Option<WidgetRef> {
        if let Some(tile) = self.app_tiles.get(&instance) {
            return Some(tile.clone());
        }
        let manifest = state.registry.get(app_id)?;
        let template = self.templates.get(&live_id!(AppTile))?;
        let template_value: ScriptValue = template.as_object().into();
        let tile = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        cx.widget_tree_insert_child_deep(
            self.uid,
            LiveId::from_str_num("atile", instance),
            tile.clone(),
        );
        // Tile chrome, not fullscreen chrome. Both live in the one template so
        // the widget can move between presentations without being rebuilt.
        tile.widget(cx, ids!(header)).set_visible(cx, false);
        // The × is a GlassButton with its own overlay draw list, so hiding the
        // header alone would let it paint over the grid; hide it outright.
        tile.widget(cx, ids!(back_button)).set_visible(cx, false);
        tile.widget(cx, ids!(tile_bar)).set_visible(cx, true);
        tile.label(cx, ids!(tile_glyph)).set_text(cx, &manifest.icon);
        tile.label(cx, ids!(tile_title)).set_text(cx, &manifest.name);
        // Fill the FULLSCREEN header too: expanding lends this very widget to
        // the app screen, which never runs it through `ensure_host` — so if we
        // skip these it arrives fullscreen with a blank title.
        tile.label(cx, ids!(glyph)).set_text(cx, &manifest.icon);
        tile.label(cx, ids!(title)).set_text(cx, &manifest.name);
        if let Some(mut splash) = tile
            .widget(cx, ids!(splash))
            .borrow_mut::<makepad_widgets::Splash>()
        {
            // The net runtime follows the user's GRANT, not the manifest,
            // and the host tag/caps make `host.request` work from home tiles
            // exactly as it does fullscreen.
            let grants = state.permissions.granted_caps(manifest);
            splash.set_allow_net(grants.iter().any(|g| g == "network"));
            // Same private storage jail as every other instance of this app —
            // same app, same container.
            splash.set_sandbox_dir(cx, Some(crate::app_sandbox_dir(app_id)));
            splash.set_host_tag(cx, Some(app_id.clone()));
            splash.set_host_caps(cx, grants);
            // Home surfaces never pop consent dialogs (they boot with the
            // launcher); prompting is the fullscreen surface's job. Flipped
            // true while this very widget is lent out fullscreen.
            splash.set_host_prompts(cx, false);
            splash.set_debug_name(&format!("{app_id} on home"));
        }
        // The REAL app source, not the widget script: a home tile runs the app.
        tile.widget(cx, ids!(splash)).set_text(cx, &manifest.source);
        self.app_tiles.insert(instance, tile.clone());
        Some(tile)
    }

    /// The stand-in drawn in a placement's cells while its app is expanded.
    fn ensure_app_away(
        &mut self,
        cx: &mut Cx,
        state: &AppState,
        instance: WidgetInstanceId,
        app_id: &MiniAppId,
    ) -> Option<WidgetRef> {
        if let Some(w) = self.app_aways.get(&instance) {
            return Some(w.clone());
        }
        let template = self.templates.get(&live_id!(AppTileAway))?;
        let template_value: ScriptValue = template.as_object().into();
        let away = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        cx.widget_tree_insert_child_deep(
            self.uid,
            LiveId::from_str_num("aaway", instance),
            away.clone(),
        );
        if let Some(manifest) = state.registry.get(app_id) {
            away.label(cx, ids!(away_glyph)).set_text(cx, &manifest.icon);
            away.label(cx, ids!(away_name)).set_text(cx, &manifest.name);
        }
        self.app_aways.insert(instance, away.clone());
        Some(away)
    }

    /// Tells a live app tile how much room its script has, via the same
    /// `on_app_resize(w, h)` hook the fullscreen host uses — so an app reflows
    /// into a grid cell exactly the way it reflows into a split pane.
    fn notify_app_tile_size(
        &mut self,
        cx: &mut Cx,
        instance: WidgetInstanceId,
        content: Vec2d,
    ) {
        let changed = self
            .app_tile_sizes
            .get(&instance)
            .is_none_or(|prev| (prev.x - content.x).abs() > 0.5 || (prev.y - content.y).abs() > 0.5);
        if !changed {
            return;
        }
        self.app_tile_sizes.insert(instance, content);
        let Some(tile) = self.app_tiles.get(&instance).cloned() else {
            return;
        };
        let splash = tile.widget(cx, ids!(splash));
        if let Some(mut splash) = splash.borrow_mut::<makepad_widgets::Splash>() {
            splash.call_script_fn(
                cx,
                live_id!(on_app_resize),
                &[content.x.into(), content.y.into()],
            );
        }
    }

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
        if let Some((manifest, widget_source)) = state
            .registry
            .get(app_id)
            .and_then(|m| m.widget.as_ref().map(|w| (m, w.source.clone())))
        {
            // A widget shares its app's private storage jail — same app, same
            // container, the OS convention — and now its grants too (widgets
            // never prompt; an Ask-state capability just isn't there yet).
            // Assigned before eval so top-level boot loads see all of it.
            if let Some(mut splash) = tile
                .widget(cx, ids!(splash))
                .borrow_mut::<makepad_widgets::Splash>()
            {
                let grants = state.permissions.granted_caps(manifest);
                splash.set_allow_net(grants.iter().any(|g| g == "network"));
                splash.set_sandbox_dir(cx, Some(crate::app_sandbox_dir(app_id)));
                splash.set_host_tag(cx, Some(app_id.clone()));
                splash.set_host_caps(cx, grants);
                // Widgets NEVER prompt (docs/PERMISSIONS.md): an Ask-state
                // request from here fails cleanly and the script falls back.
                splash.set_host_prompts(cx, false);
                // "<app> widget", not just the app id: an app and its home
                // widget are separate scripts that fail in different ways, and
                // an error naming only the app sends you to the wrong file.
                splash.set_debug_name(&format!("{app_id} widget"));
            }
            tile.widget(cx, ids!(splash)).set_text(cx, &widget_source);
        }
        tile.widget(cx, ids!(badge)).set_visible(cx, self.edit_visuals_applied);
        // The grip belongs to the dedicated resize mode (long-press → Resize),
        // which draws its own outline and corner handle. In jiggle mode it was
        // a second, differently-styled resize affordance for the same job.
        tile.widget(cx, ids!(grip)).set_visible(cx, false);
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
    /// Drops the cached icon widgets of `app_id` so the next draw rebuilds
    /// them from the (possibly changed) manifest. Cached icons bake in
    /// name/glyph/tint at creation (see `ensure_icon`), so swapping an app's
    /// manifest in place — an AI refine renaming/restyling it — must
    /// invalidate them or the grid keeps showing the old identity.
    fn refresh_app_icons(&mut self, cx: &mut Cx, layout: &LauncherLayout, app_id: &MiniAppId) {
        let mut stale: Vec<WidgetInstanceId> = layout
            .pages
            .iter()
            .flat_map(|p| p.items.iter())
            .filter_map(|it| match &it.kind {
                PlacedKind::App { id, instance, .. } if id == app_id => Some(*instance),
                _ => None,
            })
            .collect();
        // A dragged icon is lifted OUT of the layout while in flight — without
        // this it would keep its old identity forever after the drop.
        if let Some(drag) = &self.drag {
            if let PlacedKind::App { id, instance, .. } = &drag.item.kind {
                if id == app_id {
                    stale.push(*instance);
                }
            }
        }
        let before = self.icons.len();
        self.icons.retain(|inst, _| !stale.contains(inst));
        if before != self.icons.len() {
            cx.widget_tree_mark_dirty(self.uid);
            self.redraw(cx);
        }
    }

    /// Drops `app_id`'s cached WIDGET tiles so the next draw rebuilds them from
    /// the current source (and rebinds the sandbox). A refined app's widget
    /// script otherwise keeps running the old source; and on uninstall the
    /// tile's isolate must die before its data dir is deleted, or its timers
    /// keep firing against a removed jail. Marks the freed isolates dead —
    /// the caller runs the GC to actually reclaim them.
    fn drop_app_widget_tiles(&mut self, cx: &mut Cx, layout: &LauncherLayout, app_id: &MiniAppId) {
        let stale: Vec<WidgetInstanceId> = layout
            .pages
            .iter()
            .flat_map(|p| p.items.iter())
            .filter_map(|it| match &it.kind {
                PlacedKind::Widget { instance, app_id: aid, .. } if aid == app_id => Some(*instance),
                _ => None,
            })
            .collect();
        // ...and the app's LIVE home tiles: those run the real app, so a force
        // stop / uninstall / rewrite has to take them down too or their
        // isolates keep ticking against a jail that may be gone.
        let stale_apps: Vec<WidgetInstanceId> = layout
            .pages
            .iter()
            .flat_map(|p| p.items.iter())
            .filter_map(|it| match &it.kind {
                PlacedKind::App { instance, id, .. } if id == app_id => Some(*instance),
                _ => None,
            })
            .collect();
        let before_apps = self.app_tiles.len();
        self.app_tiles.retain(|inst, _| !stale_apps.contains(inst));
        self.app_tile_sizes.retain(|inst, _| !stale_apps.contains(inst));
        self.app_bar_rects.retain(|inst, _| !stale_apps.contains(inst));
        if self.expanded_app.is_some_and(|i| stale_apps.contains(&i)) {
            self.expanded_app = None;
        }
        let before = self.tiles.len();
        self.tiles.retain(|inst, _| !stale.contains(inst));
        self.tile_sizes.retain(|inst, _| !stale.contains(inst));
        if before != self.tiles.len() || before_apps != self.app_tiles.len() {
            cx.widget_tree_mark_dirty(self.uid);
            cx.redraw_all();
        }
    }

    /// Pushes a fresh capability list into an app's home isolates (grant
    /// changes that don't need a restart).
    fn update_app_caps(&mut self, cx: &mut Cx, layout: &LauncherLayout, app_id: &MiniAppId, caps: &[String]) {
        for (instance, tile) in self.app_tiles.iter().chain(self.tiles.iter()) {
            let owner = app_of_instance(layout, *instance)
                .or_else(|| widget_app_of_instance(layout, *instance));
            if owner.as_deref() == Some(app_id.as_str()) {
                if let Some(mut splash) = tile.widget(cx, ids!(splash)).borrow_mut::<Splash>() {
                    splash.set_host_caps(cx, caps.to_vec());
                }
            }
        }
    }

    /// Calls `fn on_ipc_message(from, data)` in every home isolate of an app
    /// (live tiles and widget tiles). Returns how many took the message.
    /// `skip_heap` is the sender's isolate — self-broadcasts never echo home.
    fn deliver_ipc(
        &mut self,
        cx: &mut Cx,
        layout: &LauncherLayout,
        app_id: &MiniAppId,
        from: &str,
        data: &str,
        skip_heap: usize,
    ) -> usize {
        let mut delivered = 0;
        for (instance, tile) in self.app_tiles.iter().chain(self.tiles.iter()) {
            // The lent-out tile IS the fullscreen host; the app screen already
            // delivered there, and a second call would double-deliver.
            if self.expanded_app == Some(*instance) {
                continue;
            }
            let owner = app_of_instance(layout, *instance)
                .or_else(|| widget_app_of_instance(layout, *instance));
            if owner.as_deref() != Some(app_id.as_str()) {
                continue;
            }
            if let Some(mut splash) = tile.widget(cx, ids!(splash)).borrow_mut::<Splash>() {
                if splash.isolate_heap_key(cx) == Some(skip_heap) {
                    continue;
                }
                if splash.call_script_fn_with_strings(cx, live_id!(on_ipc_message), &[from, data]) {
                    delivered += 1;
                }
            }
        }
        delivered
    }

    fn prune_children(&mut self, cx: &mut Cx, pages: &[HomePage]) {
        let mut live_app_instances = Vec::new();
        let mut live_widgets = Vec::new();
        for page in pages {
            for item in &page.items {
                match &item.kind {
                    PlacedKind::App { instance, .. } => live_app_instances.push(*instance),
                    PlacedKind::Widget { instance, .. } => live_widgets.push(*instance),
                }
            }
        }
        if let Some(drag) = &self.drag {
            match &drag.item.kind {
                PlacedKind::App { instance, .. } => live_app_instances.push(*instance),
                PlacedKind::Widget { instance, .. } => live_widgets.push(*instance),
            }
        }
        let before = self.icons.len() + self.tiles.len();
        self.icons.retain(|inst, _| live_app_instances.contains(inst));
        self.tiles.retain(|inst, _| live_widgets.contains(inst));
        self.tile_sizes.retain(|inst, _| live_widgets.contains(inst));
        self.app_tiles.retain(|inst, _| live_app_instances.contains(inst));
        self.app_aways.retain(|inst, _| live_app_instances.contains(inst));
        self.app_tile_sizes.retain(|inst, _| live_app_instances.contains(inst));
        self.app_bar_rects.retain(|inst, _| live_app_instances.contains(inst));
        if self.expanded_app.is_some_and(|i| !live_app_instances.contains(&i)) {
            self.expanded_app = None;
        }
        self.anim_pos.retain(|key, _| match key {
            ItemKey::App(inst) => live_app_instances.contains(inst),
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
        // The drag is over: no slot stays held open in the dock.
        self.dock_hover = None;

        // Released over the dock? Hand it to the dock instead of the grid. Only
        // single-cell app icons can live there (widgets stay on the grid). The item
        // was already lifted out of the layout when the drag began, so simply not
        // re-placing it here completes the move.
        if drag.item.span() == (1, 1) {
            if let PlacedKind::App { id, .. } = &drag.item.kind {
                let probe = drag.pos + self.geom().cell * 0.5;
                if let Some(index) =
                    Self::dock_slot_at(state.dock_rect, state.layout.dock.len(), probe)
                {
                    cx.widget_action(
                        self.uid,
                        HomePagerAction::DropIntoDock { app_id: id.clone(), index },
                    );
                    state.layout.prune_empty_pages();
                    state.layout_dirty = true;
                    self.redraw(cx);
                    return;
                }
            }
        }

        // A favourite dragged out of the dock and released where nothing can hold
        // it (over the edit bar, the page dots, a full page) goes back to its old
        // slot — it left the dock the moment the drag began, so discarding it here
        // would quietly un-favourite the app instead of cancelling the drag.
        if drag.target.is_none() {
            if let (Some(index), PlacedKind::App { id, .. }) = (drag.from_dock, &drag.item.kind) {
                cx.widget_action(
                    self.uid,
                    HomePagerAction::DropIntoDock { app_id: id.clone(), index },
                );
                self.redraw(cx);
                return;
            }
        }

        let layout = &mut state.layout;

        let (page, col, row) = match drag.target {
            Some(target) => target,
            None => {
                if drag.is_new {
                    // A fresh drawer app dropped on no valid cell: just discard it
                    // (the previewed reflow, if any, snaps back on the next draw).
                    self.redraw(cx);
                    return;
                }
                // No valid target: return the item to where it came from.
                let (fp, fc, fr) = drag.from;
                while layout.pages.len() <= fp {
                    layout.pages.push(HomePage::default());
                }
                let key = drag.item.key();
                layout.pages[fp].items.push(PlacedItem {
                    kind: drag.item.kind,
                    col: fc,
                    row: fr,
                });
                layout.prune_empty_pages();
                state.layout_dirty = true;
                self.seed_drop_anim(key, fp, drag.pos);
                self.redraw(cx);
                return;
            }
        };
        while layout.pages.len() <= page {
            layout.pages.push(HomePage::default());
        }

        // (A fresh drawer app keeps its own new instance id, so dropping it always
        // ADDS a distinct icon — duplicates of the same app are allowed.)

        // Commit the swap preview: the displaced icon takes the dragged icon's old cell.
        for it in &mut layout.pages[page].items {
            if let Some(&(pc, pr)) = preview.get(&it.key()) {
                it.col = pc;
                it.row = pr;
            }
        }
        // Clear the dropped item's whole footprint: bump any single-cell icon
        // still sitting under it (e.g. a cross-page drag with no reflow, or a
        // stray after a widget reflow) to the first free cell *outside* the
        // footprint so nothing overlaps. For a 1x1 icon this is just its cell.
        let (dc, dr) = drag.item.span();
        let (gc, gr) = layout.grid();
        loop {
            let Some(i) = layout.pages[page].items.iter().position(|it| {
                it.span() == (1, 1)
                    && it.col >= col
                    && it.col < col + dc
                    && it.row >= row
                    && it.row < row + dr
            }) else {
                break;
            };
            let mut moved = false;
            'free: for fr in 0 .. gr {
                for fc in 0 .. gc {
                    let in_footprint = fc >= col && fc < col + dc && fr >= row && fr < row + dr;
                    if in_footprint {
                        continue;
                    }
                    let occupied = layout.pages[page]
                        .items
                        .iter()
                        .enumerate()
                        .any(|(j, it)| j != i && it.covers(fc, fr));
                    if !occupied {
                        layout.pages[page].items[i].col = fc;
                        layout.pages[page].items[i].row = fr;
                        moved = true;
                        break 'free;
                    }
                }
            }
            if !moved {
                break;
            }
        }
        // Drop the dragged item into its cell.
        let key = drag.item.key();
        layout.pages[page].items.push(PlacedItem {
            kind: drag.item.kind,
            col,
            row,
        });
        layout.prune_empty_pages();
        state.layout_dirty = true;
        self.seed_drop_anim(key, page, drag.pos);
        self.redraw(cx);
    }

    /// Starts a just-dropped item's slide-into-place from where the finger let go.
    /// Item positions animate from an `anim_pos` entry keyed by item, and the entry
    /// survives the drag still holding the item's *pre-drag* position — so without
    /// this the item snaps back to the cell it was picked up from and slides in from
    /// there, which reads as a glitch. Seeding it with the release position (in the
    /// same page-local space the animation runs in) makes it travel the short way,
    /// from the drop point to the slot. A fresh item dragged in from the drawer or
    /// dock has no entry at all, and gets the same treatment.
    fn seed_drop_anim(&mut self, key: ItemKey, page: usize, released_at: Vec2d) {
        let origin = self.geom().page_origin(page as f64, self.page_pos);
        self.anim_pos.insert(key, released_at - origin);
    }

    /// Finds the first free `w x h` area (row-major) for the item `exclude`
    /// being relocated, treating `exclude`'s own cells as free and never landing
    /// on the reserved cell (avoid_col, avoid_row) that something else is taking.
    fn first_fit_excluding(
        grid: (u8, u8),
        page: &HomePage,
        exclude: &ItemKey,
        also_exclude: Option<&ItemKey>,
        w: u8,
        h: u8,
        avoid_col: u8,
        avoid_row: u8,
    ) -> Option<(u8, u8)> {
        for row in 0 ..= grid.1.saturating_sub(h) {
            for col in 0 ..= grid.0.saturating_sub(w) {
                let covers_avoid = col <= avoid_col
                    && avoid_col < col + w
                    && row <= avoid_row
                    && avoid_row < row + h;
                if covers_avoid {
                    continue;
                }
                let clash = page.items.iter().any(|it| {
                    let k = it.key();
                    if k == *exclude || also_exclude.is_some_and(|a| k == *a) {
                        return false;
                    }
                    let (ic, ir) = it.span();
                    col < it.col + ic && it.col < col + w && row < it.row + ir && it.row < row + h
                });
                if !clash {
                    return Some((col, row));
                }
            }
        }
        None
    }

    /// For a widget landing at (col,row) with the given span, plans where each
    /// overlapped single-cell icon slides to — the same live move-preview an
    /// icon drag produces, generalised to a multi-cell footprint. Returns None
    /// if a multi-cell widget is in the way, or a displaced icon has nowhere to
    /// go (so the drop is rejected rather than shown as valid).
    /// Plans how to clear a widget's landing footprint: every item overlapping
    /// it — single-cell icons AND other widgets — is given a new home elsewhere
    /// on the same page. Returns `None` when something can't be re-placed, which
    /// is what makes the drop invalid (no outline, no drop).
    ///
    /// Displaced items are placed largest-first: a 2x2 widget needs a real gap,
    /// and packing the 1x1s first would fragment the page out from under it.
    fn plan_widget_reflow(
        grid: (u8, u8),
        page_items: &HomePage,
        dragged_key: &ItemKey,
        col: u8,
        row: u8,
        span_cols: u8,
        span_rows: u8,
    ) -> Option<Vec<(ItemKey, (u8, u8))>> {
        let overlaps_target = |c: u8, r: u8, w: u8, h: u8| {
            col < c + w && c < col + span_cols && row < r + h && r < row + span_rows
        };
        // Everything sitting under the target footprint has to move.
        let mut displaced: Vec<(ItemKey, (u8, u8))> = Vec::new();
        for it in &page_items.items {
            if it.key() == *dragged_key {
                continue;
            }
            let (iw, ih) = it.span();
            if overlaps_target(it.col, it.row, iw, ih) {
                displaced.push((it.key(), it.span()));
            }
        }
        // Biggest first — the hardest to fit.
        displaced.sort_by_key(|(_, (w, h))| std::cmp::Reverse((*w as u16) * (*h as u16)));

        let displaced_keys: Vec<ItemKey> = displaced.iter().map(|(k, _)| k.clone()).collect();
        let mut plan: Vec<(ItemKey, (u8, u8))> = Vec::new();
        for (key, (w, h)) in &displaced {
            let mut spot = None;
            'scan: for r in 0 ..= grid.1.saturating_sub(*h) {
                for c in 0 ..= grid.0.saturating_sub(*w) {
                    // Not back under the dragged widget...
                    if overlaps_target(c, r, *w, *h) {
                        continue;
                    }
                    // ...nor onto an item that's staying put...
                    let blocked_by_staying = page_items.items.iter().any(|it| {
                        if it.key() == *dragged_key || displaced_keys.contains(&it.key()) {
                            return false;
                        }
                        let (iw, ih) = it.span();
                        c < it.col + iw && it.col < c + *w && r < it.row + ih && it.row < r + *h
                    });
                    if blocked_by_staying {
                        continue;
                    }
                    // ...nor onto somewhere we already promised to another item.
                    let blocked_by_plan = plan.iter().any(|(pk, (pc, pr))| {
                        let (pw, ph) = displaced
                            .iter()
                            .find(|(k, _)| k == pk)
                            .map(|(_, s)| *s)
                            .unwrap_or((1, 1));
                        c < pc + pw && *pc < c + *w && r < pr + ph && *pr < r + *h
                    });
                    if blocked_by_plan {
                        continue;
                    }
                    spot = Some((c, r));
                    break 'scan;
                }
            }
            plan.push((key.clone(), spot?));
        }
        Some(plan)
    }

    /// Begins dragging a brand-new app icon in from the drawer: seeds a drag
    /// state centred on the finger, takes the finger capture over from
    /// `from_area` (the long-pressed drawer cell), and runs the normal item-drag
    /// machinery so the app can be dropped anywhere on the grid.
    /// Returns whether the drag actually started — false if the finger was already
    /// released, so the caller can leave the drawer open instead of sliding it away
    /// for a drag that never began.
    fn begin_external_drag(
        &mut self,
        cx: &mut Cx,
        layout: &LauncherLayout,
        app_id: MiniAppId,
        instance: WidgetInstanceId,
        abs: Vec2d,
        from_area: Area,
        from_dock: Option<usize>,
    ) -> bool {
        // Hand the still-down finger over from the drawer cell to the pager first.
        // If the finger was already released (e.g. a long-press that only fired on
        // FingerUp), there is nothing to hand off — bail out rather than wedge the
        // pager in a DraggingItem state with an engaged sweep-lock and no finger to
        // ever end it.
        if !cx.switch_finger_capture(from_area, self.area, self.area) {
            return false;
        }
        let geom = self.geom();
        let grab_offset = geom.cell * 0.5;
        self.drag = Some(DragState {
            item: PlacedItem {
                kind: PlacedKind::App { id: app_id, instance, cols: 1, rows: 1 },
                col: 0,
                row: 0,
            },
            grab_offset,
            pos: abs - grab_offset,
            from: (0, 0, 0),
            target: None,
            edge: None,
            is_new: true,
            from_dock,
        });
        self.gesture = Gesture::DraggingItem;
        self.set_sweep_lock(cx, true);
        self.update_drag_target(cx, layout, abs);
        self.start_next_frame(cx);
        self.redraw(cx);
        true
    }

    /// Computes the currently hovered drop target for the drag, and the live
    /// reflow preview positions of the other items.
    /// Which dock slot a drop at `probe` would insert into, or None if the probe
    /// isn't over the dock. Shared by the live hover preview and the drop itself so
    /// the gap the icons open up is always the slot the item actually lands in.
    fn dock_slot_at(dock_rect: Rect, dock_len: usize, probe: Vec2d) -> Option<usize> {
        if dock_rect.size.x <= 0.0 || !dock_rect.contains(probe) {
            return None;
        }
        // One more insertion point than there are favourites: the item can go
        // before the first, between any two, or after the last.
        let slots = dock_len + 1;
        let frac =
            ((probe.x - dock_rect.pos.x) / dock_rect.size.x.max(1.0)).clamp(0.0, 1.0);
        Some(((frac * slots as f64) as usize).min(slots - 1))
    }

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
        // Hovering the dock: no grid cell is being targeted, so drop the outline
        // and the reflow preview (the drop itself goes to the dock). Only single-
        // cell icons can live there, so a widget never opens a dock slot.
        let dock_hover = if is_icon {
            Self::dock_slot_at(self.dock_rect, layout.dock.len(), probe)
        } else {
            None
        };
        if dock_hover != self.dock_hover {
            self.dock_hover = dock_hover;
        }
        if dock_hover.is_some() {
            drag.target = None;
            return;
        }
        let empty_page = HomePage::default();
        let page_items = layout.pages.get(page).unwrap_or(&empty_page);

        let from_cell = (drag.from.1, drag.from.2);
        let from_same_page = drag.from.0 == page;
        let is_new = drag.is_new;
        let dragged_key = drag.item.key();
        let target = if let Some((col, row)) = geom.cell_at(self.page_pos, probe) {
            let col = col.min(geom.grid.0.saturating_sub(span_cols));
            let row = row.min(geom.grid.1.saturating_sub(span_rows));
            if is_icon {
                // If a widget covers this cell, push it to the first spot that
                // fits it (clear of the icon's target cell) — a live reflow, so
                // icons and widgets shove each other out of the way symmetrically.
                let covering_widget = page_items.items.iter().find(|it| {
                    matches!(it.kind, PlacedKind::Widget { .. }) && it.covers(col, row)
                });
                if let Some(w) = covering_widget {
                    // Shove the widget to the first spot that fits it, so icons and
                    // widgets push each other aside symmetrically — consistently on
                    // every page and for a fresh drawer app too (not just same-page
                    // moves). The dragged icon's own cell, if it has one on this page,
                    // is freed at commit, so treat it as available for the widget.
                    let (wc, wr) = w.span();
                    match Self::first_fit_excluding(
                        geom.grid, page_items, &w.key(), Some(&dragged_key), wc, wr, col, row,
                    ) {
                        Some(spot) => {
                            self.preview_moves.insert(w.key(), spot);
                            Some((page, col, row))
                        }
                        None => None,
                    }
                } else {
                    let occ = page_items.items.iter().find(|it| {
                        it.span() == (1, 1) && it.covers(col, row) && it.key() != dragged_key
                    });
                    match occ {
                        // Empty cell: just accept the drop.
                        None => Some((page, col, row)),
                        Some(occ) => {
                            if from_same_page && !is_new {
                                // Same-page move: the occupant swaps back into the
                                // cell the dragged icon is vacating.
                                self.preview_moves.insert(occ.key(), from_cell);
                                Some((page, col, row))
                            } else if Self::first_fit_excluding(
                                geom.grid, page_items, &dragged_key, None, 1, 1, col, row,
                            )
                            .is_some()
                            {
                                // Cross-page move or a fresh drawer app: the occupant
                                // is bumped to a free cell on commit — accept only if
                                // one exists (else the drop would overlap). Treat the
                                // dragged app's own cell as free (a fresh drawer app
                                // that's already placed here is removed first at commit)
                                // and never the occupant's target cell (avoid col,row).
                                Some((page, col, row))
                            } else {
                                None
                            }
                        }
                    }
                }
            } else {
                // Widget: fit the whole footprint, ignoring its own current cells.
                // If it lands on empty space, accept; otherwise reflow any
                // overlapped single-cell icons out of the way (the same live
                // move-preview icons get) and accept if they all find a home.
                let ignore = page_items.items.iter().position(|it| it.key() == dragged_key);
                if page_items.fits(geom.grid, col, row, span_cols, span_rows, ignore) {
                    Some((page, col, row))
                } else {
                    // Reflow whatever is in the way on the page we're OVER —
                    // icons and other widgets alike. Which page the widget came
                    // from is irrelevant, so dragging one across pages displaces
                    // there too.
                    match Self::plan_widget_reflow(
                        geom.grid, page_items, &dragged_key, col, row, span_cols, span_rows,
                    ) {
                        Some(plan) => {
                            for (k, cell) in plan {
                                self.preview_moves.insert(k, cell);
                            }
                            Some((page, col, row))
                        }
                        None => None,
                    }
                }
            }
        } else {
            None
        };
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
        // Recompute the reflow from the layout as it was when the resize began, so
        // that shrinking the widget releases neighbours it pushed while growing
        // (rather than stranding them from an earlier, larger frame). Work on a COPY
        // and commit only if the whole reflow is feasible: an infeasible frame must
        // leave the live layout at its last valid size, never collapse the widget
        // back to pristine mid-drag.
        let mut work: Vec<PlacedItem> = match self.resize_pristine.as_ref() {
            Some((pp, items)) if *pp == page => items.clone(),
            _ => match state.layout.pages.get(page) {
                Some(p) => p.items.clone(),
                None => return false,
            },
        };
        let Some(idx) = work.iter().position(|it| {
            let i = match &it.kind {
                PlacedKind::Widget { instance, .. } => *instance,
                PlacedKind::App { instance, .. } => *instance,
            };
            i == instance
        }) else {
            return false;
        };
        let min_span = match &work[idx].kind {
            PlacedKind::Widget { app_id, .. } => state
                .registry
                .get(app_id)
                .and_then(|m| m.widget.as_ref())
                .map(|w| w.min_span)
                .unwrap_or((1, 1)),
            // An app always shrinks back to a single cell — that IS the
            // gesture for turning a running tile back into a plain icon.
            PlacedKind::App { .. } => (1, 1),
        };
        let (col, row) = (work[idx].col, work[idx].row);
        let new_cols =
            (start_span.0 as i32 + dcols)
                .clamp(min_span.0 as i32, (grid.0 - col) as i32) as u8;
        let new_rows =
            (start_span.1 as i32 + drows)
                .clamp(min_span.1 as i32, (grid.1 - row) as i32) as u8;
        // Relocate any items the grown footprint would overlap to free cells, so
        // the resize pushes neighbours (icons and widgets) out of the way instead
        // of being blocked. Bail only if something genuinely has nowhere to go.
        let hits = |c: u8, r: u8, w: u8, h: u8| -> bool {
            col < c + w && c < col + new_cols && row < r + h && r < row + new_rows
        };
        let overlapping: Vec<usize> = work
            .iter()
            .enumerate()
            .filter(|(i, it)| {
                *i != idx && {
                    let (ic, ir) = it.span();
                    hits(it.col, it.row, ic, ir)
                }
            })
            .map(|(i, _)| i)
            .collect();
        let mut plan: Vec<(usize, (u8, u8))> = Vec::new();
        'reloc: for &i in &overlapping {
            let (ic, ir) = work[i].span();
            for r in 0 ..= grid.1.saturating_sub(ir) {
                for c in 0 ..= grid.0.saturating_sub(ic) {
                    if hits(c, r, ic, ir) {
                        continue;
                    }
                    let clashes = work.iter().enumerate().any(|(j, jt)| {
                        if j == idx || overlapping.contains(&j) {
                            return false;
                        }
                        let (jc, jr) = jt.span();
                        c < jt.col + jc && jt.col < c + ic && r < jt.row + jr && jt.row < r + ir
                    }) || plan.iter().any(|(pj, (pc, pr))| {
                        let (jc, jr) = work[*pj].span();
                        c < pc + jc && *pc < c + ic && r < pr + jr && *pr < r + ir
                    });
                    if !clashes {
                        plan.push((i, (c, r)));
                        continue 'reloc;
                    }
                }
            }
            // No room to relocate this neighbour: block the resize and leave the
            // live layout untouched (holding its last valid size).
            return false;
        }
        // The frame is feasible: bake the span + relocations into the working copy.
        for (i, (c, r)) in plan {
            work[i].col = c;
            work[i].row = r;
        }
        match &mut work[idx].kind {
            PlacedKind::Widget { cols, rows, .. } | PlacedKind::App { cols, rows, .. } => {
                *cols = new_cols;
                *rows = new_rows;
            }
        }
        // Commit only if it actually differs from what's already shown, so an
        // unchanged frame (finger moved within the same cell) doesn't churn/redraw.
        let Some(live) = state.layout.pages.get_mut(page) else {
            return false;
        };
        if live.items == work {
            return false;
        }
        live.items = work;
        state.layout_dirty = true;
        // Shrunk back to a single cell: it's a plain icon again, so the live
        // host (and its isolate) goes away.
        if (new_cols, new_rows) == (1, 1) {
            self.app_tiles.remove(&instance);
            self.app_aways.remove(&instance);
            self.app_tile_sizes.remove(&instance);
            self.app_bar_rects.remove(&instance);
            if self.expanded_app == Some(instance) {
                self.expanded_app = None;
            }
        }
        true
    }

    /// Intersects `r` with `bounds`, returning a non-negative rect. Used to keep the
    /// resize indicator's inflated quad from drawing outside the pager (off-screen).
    fn clamp_rect(r: Rect, bounds: Rect) -> Rect {
        let x0 = r.pos.x.max(bounds.pos.x);
        let y0 = r.pos.y.max(bounds.pos.y);
        let x1 = (r.pos.x + r.size.x).min(bounds.pos.x + bounds.size.x);
        let y1 = (r.pos.y + r.size.y).min(bounds.pos.y + bounds.size.y);
        Rect {
            pos: dvec2(x0, y0),
            size: dvec2((x1 - x0).max(0.0), (y1 - y0).max(0.0)),
        }
    }

    /// Finds the (page, col, row, span) of the widget with the given instance.
    fn widget_placement(
        layout: &LauncherLayout,
        instance: WidgetInstanceId,
    ) -> Option<(usize, u8, u8, (u8, u8))> {
        for (page, p) in layout.pages.iter().enumerate() {
            for it in &p.items {
                // Either kind: an app icon resizes exactly like a widget now
                // (past 1x1 it starts running in the cells it claims).
                let i = match &it.kind {
                    PlacedKind::Widget { instance, .. } => *instance,
                    PlacedKind::App { instance, .. } => *instance,
                };
                if i == instance {
                    return Some((page, it.col, it.row, it.span()));
                }
            }
        }
        None
    }

    /// Drives the widget resize started by grabbing the Android resize handle while
    /// a widget's context menu is open. Reads RAW pointer events (not hit-tested
    /// ones) so the grab and drag keep working even while the modal menu is on top
    /// and the pager's normal input is gated off. While dragging, the widget snaps
    /// to whole cells and the white border corner tracks the finger (drawn in the
    /// draw pass). Returns true if the event was consumed.
    /// The live tiles' own chrome: ⤢ / shrink on a running app, and tapping a
    /// stand-in to bring its app back.
    ///
    /// Matched against RAW pointer positions and rects captured during the
    /// draw, NOT `event.hits()`, for the same reason `handle_menu_resize` does
    /// it: this runs while the item's context menu may still be up, and a
    /// modal on top swallows hit-tests before the pager ever sees them.
    /// Returns true when the event was consumed.
    fn handle_app_tile_chrome(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        scope: &mut Scope,
    ) -> bool {
        let Some((pos, phase)) = pointer_event(event) else {
            return false;
        };
        if phase != PointerPhase::Down {
            return false;
        }
        let Some(state) = scope.data.get::<AppState>() else {
            return false;
        };
        if state.edit_mode {
            return false;
        }

        // A stand-in: tap anywhere on it to pull the app back into its cells.
        // Checked BEFORE the hide-tiles gate, because the whole point of the
        // stand-in is the case where a pane exists and home is still on show
        // (split-screen pick): it draws then, so it must be tappable then.
        let away_hit = self
            .app_away_rects
            .iter()
            .find(|(instance, r)| self.expanded_app == Some(**instance) && r.contains(pos))
            .map(|(instance, _)| *instance);
        if let Some(instance) = away_hit {
            cx.widget_action(self.uid, HomePagerAction::ReturnAppTile { instance });
            self.redraw(cx);
            return true;
        }

        // The live tiles' own buttons, on the other hand, are only pressable
        // when those tiles are actually drawn. `home_input_enabled` is
        // deliberately NOT consulted: an open menu must not disable the very
        // buttons the user can see (that gate is for the grid's gestures).
        if state.hide_widget_tiles {
            return false;
        }

        let hit = self
            .app_btn_rects
            .iter()
            .filter(|(instance, _)| self.expanded_app != Some(**instance))
            .find_map(|(instance, expand)| expand.contains(pos).then_some(*instance));
        let Some(instance) = hit else {
            return false;
        };
        let Some(app_id) = self.app_id_of(scope, instance) else {
            return false;
        };
        let from_rect = self
            .app_tiles
            .get(&instance)
            .map(|t| t.area().rect(cx))
            .unwrap_or_default();
        let action = HomePagerAction::ExpandAppTile { instance, app_id, from_rect };
        cx.widget_action(self.uid, action);
        // Claim the press so the menu (if any) doesn't also treat it as a
        // dismiss tap, and so the grid takes no gesture from it.
        match event {
            Event::MouseDown(e) => e.handled.set(self.area),
            Event::TouchUpdate(e) => {
                if let Some(t) = e.touches.first() {
                    t.handled.set(self.area);
                }
            }
            _ => {}
        }
        cx.widget_action(self.uid, HomePagerAction::HidePopups);
        self.redraw(cx);
        true
    }

    /// The app id behind a placement instance, from the live layout.
    fn app_id_of(&self, scope: &mut Scope, instance: WidgetInstanceId) -> Option<MiniAppId> {
        let state = scope.data.get::<AppState>()?;
        state.layout.pages.iter().find_map(|p| {
            p.items.iter().find_map(|it| match &it.kind {
                PlacedKind::App { id, instance: i, .. } if *i == instance => Some(id.clone()),
                _ => None,
            })
        })
    }

    fn handle_menu_resize(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) -> bool {
        let Some((pos, phase)) = pointer_event(event) else {
            return false;
        };

        // Not yet dragging: begin only when a widget's menu is open (resize_hint
        // set) and the press lands on that widget's bottom-right corner handle.
        if self.menu_resize.is_none() {
            if phase != PointerPhase::Down {
                return false;
            }
            let Some(instance) = self.resize_hint else {
                return false;
            };
            let Some(state) = scope.data.get::<AppState>() else {
                return false;
            };
            let Some((page, col, row, span)) = Self::widget_placement(&state.layout, instance)
            else {
                return false;
            };
            let cell = self.geom().cell_rect(page, self.page_pos, col, row, span);
            let corner = self
                .last_resize_rect
                .map(|r| r.pos + r.size)
                .unwrap_or(cell.pos + cell.size);
            if (pos - corner).length() > MENU_RESIZE_HIT {
                return false;
            }
            self.menu_resize = Some(MenuResize {
                instance,
                page,
                col,
                row,
                start_span: span,
                handle_start: corner,
                finger: pos,
            });
            self.resize_pristine = Some((page, state.layout.pages[page].items.clone()));
            // Mark the press handled so the (overlapping) menu doesn't also treat it
            // as a button click or dismiss tap.
            match event {
                Event::MouseDown(e) => e.handled.set(self.area),
                Event::TouchUpdate(e) => {
                    if let Some(t) = e.touches.first() {
                        t.handled.set(self.area);
                    }
                }
                _ => {}
            }
            // Grabbing a handle dismisses the popup, Android-style; the border keeps
            // following the finger via `menu_resize` even after the menu closes.
            cx.widget_action(self.uid, HomePagerAction::HidePopups);
            self.set_sweep_lock(cx, true);
            self.redraw(cx);
            return true;
        }

        // A drag is in progress: update on move, commit + finish on up.
        let mr = self.menu_resize.clone().unwrap();
        match phase {
            PointerPhase::Down => true,
            PointerPhase::Move => {
                self.menu_resize.as_mut().unwrap().finger = pos;
                let geom = self.geom();
                let delta = pos - mr.handle_start;
                let dcols = (delta.x / geom.cell.x).round() as i32;
                let drows = (delta.y / geom.cell.y).round() as i32;
                if let Some(state) = scope.data.get_mut::<AppState>() {
                    self.resize_tile_to(state, mr.page, mr.instance, mr.start_span, dcols, drows);
                }
                self.redraw(cx);
                true
            }
            PointerPhase::Up => {
                self.menu_resize = None;
                self.resize_pristine = None;
                self.resize_hint = None;
                self.set_sweep_lock(cx, false);
                self.redraw(cx);
                true
            }
        }
    }

    /// When a press begins over an interactive widget, a button inside its Splash
    /// can grab the finger on FingerDown, which blinds the pager's hit-tested move
    /// detection (the child's capture is found first) — so a page-swipe or drag
    /// begun on that button would be swallowed. On the first raw-pointer movement
    /// past the tap slop, hand the finger up to the pager so its normal
    /// pan/drag path drives the gesture. A no-op when the press was over a
    /// non-interactive part of the widget (nothing grabbed the finger). Reads raw
    /// pointer events (which bypass the capture system) and runs before the pager's
    /// own hit-test in the same event, so that hit-test then sees the FingerMove.
    fn handle_widget_takeover(&mut self, cx: &mut Cx, event: &Event) {
        if self.widget_takeover_done {
            return;
        }
        let start = match &self.gesture {
            Gesture::Pending { start, item: Some(ItemKey::Widget(_)) } => *start,
            Gesture::Lifted { item: ItemKey::Widget(_), start } => *start,
            _ => return,
        };
        let Some((pos, PointerPhase::Move)) = pointer_event(event) else {
            return;
        };
        if (pos - start).length() <= TAP_SLOP {
            return;
        }
        // One attempt per press, whether or not a child was actually in the way.
        self.widget_takeover_done = true;
        cx.promote_finger_capture_over(self.area);
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

    /// Removes the item with the given key (from whichever page holds it), used
    /// once the remove is confirmed. Prunes emptied pages and dropped tiles.
    fn remove_by_key(&mut self, cx: &mut Cx, state: &mut AppState, key: &ItemKey) {
        for page in &mut state.layout.pages {
            if let Some(idx) = page.items.iter().position(|it| it.key() == *key) {
                page.items.remove(idx);
                state.layout.prune_empty_pages();
                state.layout_dirty = true;
                self.prune_children(cx, &state.layout.pages);
                self.redraw(cx);
                return;
            }
        }
    }

    /// Deletes the whole page at `page` (with any items on it) and snaps the pager
    /// to a still-valid page. At least one page always remains — deleting the last
    /// page leaves a single empty one.
    fn delete_page(&mut self, cx: &mut Cx, state: &mut AppState, page: usize) {
        if page >= state.layout.pages.len() {
            return;
        }
        state.layout.pages.remove(page);
        if state.layout.pages.is_empty() {
            state.layout.pages.push(HomePage::default());
        }
        state.layout_dirty = true;
        self.prune_children(cx, &state.layout.pages);
        // Land on the page that slid into this slot, or the new last page.
        let target = page.min(state.layout.pages.len() - 1) as f64;
        self.settle_to(cx, &state.layout, target);
        self.redraw(cx);
    }
}

/// The app owning a placed LIVE-APP instance, from the layout.
fn app_of_instance(layout: &LauncherLayout, instance: WidgetInstanceId) -> Option<MiniAppId> {
    layout.pages.iter().flat_map(|p| p.items.iter()).find_map(|it| match &it.kind {
        PlacedKind::App { instance: i, id, .. } if *i == instance => Some(id.clone()),
        _ => None,
    })
}

/// The app owning a placed WIDGET instance, from the layout.
fn widget_app_of_instance(layout: &LauncherLayout, instance: WidgetInstanceId) -> Option<MiniAppId> {
    layout.pages.iter().flat_map(|p| p.items.iter()).find_map(|it| match &it.kind {
        PlacedKind::Widget { instance: i, app_id, .. } if *i == instance => Some(app_id.clone()),
        _ => None,
    })
}

/// Converts a 0xRRGGBB tint into a translucent icon-tile fill color.
pub fn tile_tint_color(tint: u32) -> Vec4f {
    let r = ((tint >> 16) & 0xff) as f32 / 255.0;
    let g = ((tint >> 8) & 0xff) as f32 / 255.0;
    let b = (tint & 0xff) as f32 / 255.0;
    // The vibrant, opaque base colour for an iOS-style icon tile. The tile shader
    // (LauncherIconTile) turns this into a top-lit gradient + glassy sheen, so keep
    // the raw saturated colour here for contrast rather than washing it toward white.
    Vec4f { x: r, y: g, z: b, w: 1.0 }
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
                    self.page_pos += diff * (1.0 - (-dt * 8.0).exp());
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
                            // Both kinds: long-pressing an app icon offers the
                            // same resize frame a widget gets, which is how you
                            // grow it into a live tile.
                            let widget = match &item {
                                ItemKey::Widget(i) => Some(*i),
                                ItemKey::App(i) => Some(*i),
                            };
                            if let Some(action) = self.menu_action_for(&state.layout, item) {
                                // Only once the menu actually opens: suppress the
                                // widget's in-VM buttons for the rest of this press
                                // synchronously (the app also sets this a frame later),
                                // so the release ending the long-press can't trip a
                                // button underneath.
                                self.resize_hint = widget;
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
                // Edit mode: the item was lifted on touch-down; a hold with no move
                // pops its context menu (a move would have stopped this timer and
                // started a drag). Sliding after still turns into a drag.
                Gesture::Lifted { item, .. } => {
                    if let Some(state) = scope.data.get::<AppState>() {
                        if let Some(action) = self.menu_action_for(&state.layout, item) {
                            cx.widget_action(self.uid, action);
                        }
                    }
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

        // Widget resize started by grabbing the Android resize handle from a
        // widget's context menu. Handled before child forwarding and the overlay
        // input gate (which would otherwise swallow the drag), using raw pointer
        // events so it works while the modal menu is still on top.
        if self.handle_menu_resize(cx, event, scope) {
            return;
        }

        // The live tiles' own chrome: ⤢ / shrink on a running app, and the
        // stand-in's "Bring back". SDF views, so they're hit-tested here the
        // way the app screen hit-tests its split button.
        if self.handle_app_tile_chrome(cx, event, scope) {
            return;
        }

        // Hand a finger up from an interactive widget child so a page-swipe/drag
        // begun on a widget button isn't swallowed by the button's capture. Runs
        // before the pager's own hit-test below (which then sees the FingerMove);
        // does not consume the event.
        self.handle_widget_takeover(cx, event);

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
                            PlacedKind::App { instance, cols, rows, .. } => {
                                if *cols == 1 && *rows == 1 {
                                    if let Some(w) = self.icons.get(instance) {
                                        to_event.push(w.clone());
                                    }
                                } else if self.expanded_app == Some(*instance) {
                                    if let Some(w) = self.app_aways.get(instance) {
                                        to_event.push(w.clone());
                                    }
                                } else {
                                    // A running tile IS the app: it takes taps
                                    // like any other app surface, under the same
                                    // rules that gate widget tiles.
                                    let suppress = state.edit_mode
                                        || !state.home_input_enabled
                                        || state.hide_widget_tiles
                                        || Some(*instance) == self.resize_hint;
                                    if !suppress {
                                        if let Some(w) = self.app_tiles.get(instance) {
                                            to_event.push(w.clone());
                                        }
                                    }
                                }
                            }
                            PlacedKind::Widget { instance, .. } => {
                                // In edit mode a widget is a drag handle, not an
                                // interactive surface: withhold finger events from its
                                // Splash so a press/drag moves the tile and its in-VM
                                // buttons can't fire (iOS/Android jiggle behaviour).
                                // Same while its context menu is open, so the long-press
                                // that opened the menu doesn't also trip a button.
                                // ...and while the create panel is expanded: a
                                // press outside it is a DISMISSAL, so it must
                                // not also trip a button inside a widget.
                                // ...and while tiles are hidden for a
                                // mini-app pane: an invisible widget must not
                                // react to taps landing where it used to be.
                                let suppress = state.edit_mode
                                    || !state.home_input_enabled
                                    || state.hide_widget_tiles
                                    || Some(*instance) == self.resize_hint;
                                if !suppress {
                                    if let Some(w) = self.tiles.get(instance) {
                                        to_event.push(w.clone());
                                    }
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
                .chain(self.app_tiles.values())
                .chain(self.app_aways.values())
                .cloned()
                .collect();
            for w in children {
                w.handle_event(cx, event, scope);
            }
        }

        // Reconcile a leaked resize hint. The hint (and its Android resize
        // indicator) is only valid while a widget's context menu is open, which
        // gates home input off. If home input is back on (the menu closed) with no
        // resize drag in flight, the hint lingered — most commonly because the menu
        // was dismissed by a tap outside it, which closes the modal without routing
        // through close_context_menu — so drop it here rather than leave the
        // indicator stuck on screen.
        if self.resize_hint.is_some()
            && self.menu_resize.is_none()
            && scope
                .data
                .get::<AppState>()
                .is_some_and(|s| s.home_input_enabled)
        {
            self.resize_hint = None;
            self.redraw(cx);
        }

        // Mirror the dock's rect so drag targeting can tell "over the dock" apart
        // from "over the grid" — and the activity panel's, so presses on it
        // don't reach the grid.
        if let Some(state) = scope.data.get::<AppState>() {
            self.dock_rect = state.dock_rect;
            self.create_rect = state.create_rect;
            self.split_block_rect = state.split_block_rect;
            if self.hide_widget_tiles != state.hide_widget_tiles {
                self.hide_widget_tiles = state.hide_widget_tiles;
                // Real visibility, not just a draw-skip: the widget snapshot
                // (what tests and tools read) reports the visible flag and
                // the last drawn area, both of which survive a mere skip.
                for tile in self.tiles.values() {
                    tile.set_visible(cx, !self.hide_widget_tiles);
                }
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
        let mut options = HitOptions::new()
            .with_capture_overload(true)
            .with_sweep_area(self.area);
        if matches!(self.gesture, Gesture::DraggingItem) {
            // An item in flight legitimately travels outside the pager's rect: down
            // onto the dock to be dropped there, or up over the edit bar. Sweep
            // hit-testing would otherwise read "finger left my rect" as a sweep-out
            // and fire FingerUp at the boundary, dropping the item at the crossing
            // point (and making a drag begun in the dock end before it moved at
            // all). Widen the hit rect for the duration so the drag keeps its
            // finger until it's genuinely released.
            options = options.with_margin(Inset {
                left: DRAG_HIT_SLACK,
                top: DRAG_HIT_SLACK,
                right: DRAG_HIT_SLACK,
                bottom: DRAG_HIT_SLACK,
            });
        }
        let hit = event.hits_with_options(cx, self.area, options);
        let Some(state) = scope.data.get_mut::<AppState>() else {
            return;
        };
        self.grid = state.layout.grid();

        match hit {
            Hit::FingerDown(fe) => {
                // Presses on the floating create bar belong to the bar —
                // without this the co-capturing pager would ALSO treat them as
                // grid taps and could open an app hidden underneath it.
                if self.create_rect.size.x > 0.0 && self.create_rect.contains(fe.abs) {
                    return;
                }
                // Same for a split-pick's docked pane: it draws over the grid,
                // and taps inside it must not open the icons it covers.
                if self.split_block_rect.size.x > 0.0 && self.split_block_rect.contains(fe.abs) {
                    return;
                }
                // A fresh press: re-arm the widget finger-takeover latch.
                self.widget_takeover_done = false;
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
                // A running app tile is an app, not an icon: only its title bar
                // is a handle. A press on the body belongs to the app, so the
                // pager takes no gesture at all and the event forwards through.
                if let Some(ItemKey::App(instance)) = &item {
                    let is_live = self
                        .app_tiles
                        .contains_key(instance)
                        && self.expanded_app != Some(*instance);
                    if is_live
                        && !self
                            .app_bar_rects
                            .get(instance)
                            .is_some_and(|bar| bar.contains(fe.abs))
                    {
                        self.gesture = Gesture::Idle;
                        return;
                    }
                }

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
                            // Ask the app to confirm before removing, iOS-style.
                            let name = state
                                .registry
                                .get(placed.app_id())
                                .map(|m| m.name.clone())
                                .unwrap_or_else(|| placed.app_id().clone());
                            let label = match &placed.kind {
                                PlacedKind::App { .. } => name,
                                PlacedKind::Widget { .. } => format!("{name} widget"),
                            };
                            cx.widget_action(
                                self.uid,
                                HomePagerAction::RequestRemove { item: placed.key(), label },
                            );
                            self.gesture = Gesture::Consumed;
                            return;
                        }
                        // Resize handle in the bottom-right corner of widget
                        // tiles — resize MODE only. In jiggle mode the corner
                        // is where you grab a widget to drag it, and a hidden
                        // resize hotspot there stole those drags.
                        if let PlacedKind::Widget { instance, cols, rows, .. } = &placed.kind {
                            let corner = cell.pos + cell.size;
                            if self.resize_hint == Some(*instance)
                                && (fe.abs - corner).length() < RESIZE_HIT_SIZE
                            {
                                self.gesture = Gesture::ResizingTile {
                                    instance: *instance,
                                    start_span: (*cols, *rows),
                                    start_abs: fe.abs,
                                };
                                // Snapshot the page so the resize reflow is
                                // recomputed from a pristine layout each frame.
                                self.resize_pristine =
                                    Some((page, state.layout.pages[page].items.clone()));
                                self.set_sweep_lock(cx, true);
                                return;
                            }
                        }
                        self.gesture = Gesture::Lifted {
                            item: placed.key(),
                            start: fe.abs,
                        };
                        // Also arm the long-press timer so holding still (rather than
                        // moving into a drag) pops the item's context menu even in
                        // edit/jiggle mode — the only way to reach it on touch.
                        self.long_press_timer = cx.start_timeout(LONG_PRESS_SECS);
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
                                        let widget = match &item {
                                            ItemKey::Widget(i) => Some(*i),
                                            _ => None,
                                        };
                                        if let Some(action) =
                                            self.menu_action_for(&state.layout, item)
                                        {
                                            self.resize_hint = widget;
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
                            // Moving into a drag: cancel the pending long-press menu.
                            cx.stop_timer(self.long_press_timer);
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
                                // Dragging an item to rearrange it does NOT enter
                                // jiggle/edit mode (no wobble, no top edit bar); it
                                // just moves the item. Edit mode is entered only via
                                // long-pressing empty space or the "Edit Home Screen"
                                // button.
                                self.drag = Some(DragState {
                                    from: (p, placed.col, placed.row),
                                    item: placed,
                                    grab_offset: fe.abs - cell.pos,
                                    pos: cell.pos,
                                    target: None,
                                    edge: None,
                                    is_new: false,
                                    from_dock: None,
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
                            } else if matches!(item, Some(ItemKey::App(_))) {
                                // Find the icon's rect for the zoom-out animation and
                                // its app id (the key is only the placement instance).
                                if let Some((page, idx)) = self.item_at(&state.layout, fe.abs) {
                                    let placed = &state.layout.pages[page].items[idx];
                                    let app_id = placed.app_id().clone();
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
                self.resize_pristine = None;
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

        // The widget to draw the Android resize indicator around: the one whose
        // context menu is open (edit-mode resizing keeps its own blue corner grip, so
        // the two don't clash). Its on-screen rect is captured in the item loop below,
        // then the indicator is drawn (in the overlay) around it.
        let active_resize = self.resize_hint;
        let mut resize_rect: Option<Rect> = None;

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
        // Rebuilt every frame from what actually gets drawn.
        self.icon_hits.clear();
        for (page_idx, page) in pages.iter().enumerate() {
            let p = page_idx as f64;
            if (p - self.page_pos).abs() >= 1.0 {
                continue;
            }
            for (idx, item) in page.items.iter().enumerate() {
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
                let local_target = target_rect.pos - geom.page_origin(p, self.page_pos);
                let anim = self
                    .anim_pos
                    .entry(key.clone())
                    .or_insert(local_target);
                let diff = local_target - *anim;
                if diff.length() > 0.5 {
                    *anim += diff * 0.15;
                    any_anim = true;
                } else {
                    *anim = local_target;
                }
                let mut draw_pos = geom.page_origin(p, self.page_pos) + *anim;
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
                    PlacedKind::App { id, instance, .. } => {
                        let state = scope.data.get::<AppState>().unwrap();
                        if span == (1, 1) {
                            self.ensure_icon(cx, state, *instance, id)
                        } else if self.expanded_app == Some(*instance) {
                            // Checked BEFORE hide_widget_tiles: the stand-in is
                            // a plain card (no glass overlay to float over a
                            // pane), and it is the one thing that SHOULD show
                            // in these cells while the app is off elsewhere.
                            self.ensure_app_away(cx, state, *instance, id)
                        } else if state.hide_widget_tiles {
                            // Same rule as widget tiles: a live tile's glass
                            // composites above the whole main pass, so it must
                            // not draw while a mini-app pane is up.
                            continue;
                        } else {
                            let tile = self.ensure_app_tile(cx, state, *instance, id);
                            // The app's usable box inside the host chrome, so
                            // its script reflows for the cell it was given.
                            let content = dvec2(
                                target_rect.size.x - gap * 2.0 - APP_TILE_PAD_X,
                                target_rect.size.y - gap * 2.0 - APP_TILE_PAD_Y,
                            );
                            self.notify_app_tile_size(cx, *instance, content);
                            tile
                        }
                    }
                    PlacedKind::Widget { instance, app_id, .. } => {
                        let state = scope.data.get::<AppState>().unwrap();
                        // While any mini-app pane exists, tiles don't draw: a
                        // glass tile composites its whole subtree ABOVE the
                        // main pass, so it would float in front of the pane
                        // (split-pick keeps home visible, so this is the only
                        // way the pane stays on top). Icons stay drawn — they
                        // render inline and are what pick mode is for.
                        if state.hide_widget_tiles {
                            continue;
                        }
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
                if let Some(child) = &child {
                    child.draw_walk_all(cx, scope, child_walk);
                    // An app icon's tappable area is the TILE plus its LABEL and
                    // nothing else — captured from what was actually drawn
                    // rather than derived from the cell, because the cell is far
                    // larger than the icon and every attempt to compute the
                    // offset by hand was wrong in one direction or the other.
                    // A live app tile's bar is its grab handle; remember where
                    // it landed so presses on it can be told from presses on
                    // the app body underneath.
                    if let PlacedKind::App { instance, .. } = &item.kind {
                        if span != (1, 1) && self.expanded_app != Some(*instance) {
                            let bar = child.widget(cx, ids!(tile_bar)).area().rect(cx);
                            if bar.size.y > 1.0 {
                                self.app_bar_rects.insert(*instance, bar);
                            }
                            // The bar's buttons are matched against RAW pointer
                            // positions (see handle_app_tile_chrome), so their
                            // rects have to be remembered from the draw.
                            let ex = child.widget(cx, ids!(tile_expand)).area().rect(cx);
                            if ex.size.x > 1.0 {
                                self.app_btn_rects.insert(*instance, ex);
                            }
                        } else if span != (1, 1) {
                            // The stand-in: the whole card is the way back.
                            let r = child.area().rect(cx);
                            if r.size.x > 1.0 {
                                self.app_away_rects.insert(*instance, r);
                            }
                        }
                    }
                    if matches!(item.kind, PlacedKind::App { cols: 1, rows: 1, .. }) {
                        // One rect, straight from the `Fit` group — it already
                        // spans the tile, the spacing and however many lines
                        // the label took.
                        let group = child.widget(cx, ids!(icon_group)).area().rect(cx);
                        if group.size.y > 1.0 {
                            self.icon_hits.push((page_idx, idx, group));
                        }
                    }
                }
                // Capture this widget's actual on-screen rect (its rendered area, so
                // the indicator hugs the real card even if it doesn't fill the cell)
                // if it's the resize target.
                // Either kind: long-pressing an APP icon arms the same frame,
                // and its handle is how you grow it into a running tile — so
                // an app placement has to report its rect here as well, or the
                // frame is armed but nothing ever draws it.
                let placed_instance = match &item.kind {
                    PlacedKind::Widget { instance, .. } => Some(*instance),
                    PlacedKind::App { instance, .. } => Some(*instance),
                };
                if let Some(instance) = &placed_instance {
                    if Some(*instance) == active_resize {
                        // A 1x1 icon does NOT own its whole cell visually: the
                        // AppIcon box fills the cell so `align.y` has slack to
                        // work with, while the icon+label sit in the `icon_group`
                        // Fit box inside it. Framing the cell drew a border
                        // hanging far below the label (and straight under the
                        // long-press menu), so frame the group instead — the
                        // same rect the menu anchors to.
                        let tight = if span == (1, 1) {
                            child
                                .as_ref()
                                .map(|c| c.widget(cx, ids!(icon_group)).area().rect(cx))
                                .filter(|r| r.size.x > 1.0 && r.size.y > 1.0)
                        } else {
                            None
                        };
                        let area_rect = tight
                            .or_else(|| child.as_ref().map(|c| c.area().rect(cx)))
                            .filter(|r| r.size.x > 1.0 && r.size.y > 1.0)
                            .unwrap_or(Rect {
                                pos: draw_pos + dvec2(gap, gap),
                                size: dvec2(
                                    target_rect.size.x - gap * 2.0,
                                    target_rect.size.y - gap * 2.0,
                                ),
                            });
                        // Remember it so the grab region matches the corner
                        // actually drawn (a 1x1 icon's frame is its group, not
                        // its cell — grabbing the cell corner would mean
                        // reaching for empty space below the border).
                        self.last_resize_rect = Some(area_rect);
                        resize_rect = Some(area_rect);
                    }
                }
            }
        }

        // Draw the Android-style resize indicator (outline + corner handle). While a
        // menu-resize drag is in progress the border's corner tracks the finger
        // (snapping the widget to whole cells); otherwise it hugs the widget whose
        // context menu is open. Drawn in an overlay so it sits above the glass lens.
        let indicator_rect = if let Some(mr) = &self.menu_resize {
            // Top-left is pinned to the widget's cell. The bottom-right follows the
            // finger, but its floor is the widget's live (committed, snapped) size:
            // the border tracks the finger smoothly, then snaps to each gridline as
            // the widget grows, and never cuts inside the card. The ceiling is the
            // grid edge.
            // Inset by the same WIDGET_GAP the tile itself is drawn with. The
            // raw CELL rect is what the widget sits inside, not what it
            // occupies — using it made the outline jump a gap wider and a gap
            // higher the moment the handle was grabbed, which pushed a
            // top-row widget's frame off the top of the pager.
            let gap = WIDGET_GAP;
            let topleft =
                geom.cell_rect(mr.page, self.page_pos, mr.col, mr.row, (1, 1)).pos
                    + dvec2(gap, gap);
            let span = scope
                .data
                .get::<AppState>()
                .and_then(|s| Self::widget_placement(&s.layout, mr.instance))
                .map(|(.., span)| span)
                .unwrap_or(mr.start_span);
            // Sizes lose the gap on BOTH edges, so the floor matches exactly
            // what the committed widget draws and the border never cuts into
            // the card.
            let floor_w = span.0.max(1) as f64 * geom.cell.x - gap * 2.0;
            let floor_h = span.1.max(1) as f64 * geom.cell.y - gap * 2.0;
            let max_w = self.grid.0.saturating_sub(mr.col) as f64 * geom.cell.x - gap * 2.0;
            let max_h = self.grid.1.saturating_sub(mr.row) as f64 * geom.cell.y - gap * 2.0;
            Some(Rect {
                pos: topleft,
                size: dvec2(
                    (mr.finger.x - topleft.x).max(floor_w).min(max_w),
                    (mr.finger.y - topleft.y).max(floor_h).min(max_h),
                ),
            })
        } else {
            resize_rect
        };
        if let Some(r) = indicator_rect {
            let inf = RESIZE_FRAME_INFLATE;
            // Clamp the inflated quad so a widget flush against a screen edge
            // doesn't draw its outline/handle off-screen — but allow it to reach
            // `inf` ABOVE the pager, because a top-row widget's top edge IS the
            // pager's top edge. Clamping there cut the quad while the shader
            // still measured its outline `pad` in from the quad, so the outline
            // landed inside the widget and the frame looked broken. There's room
            // above: the create bar's reserved slot sits there, and the bar now
            // composites over it.
            let bounds = Rect {
                pos: dvec2(self.last_rect.pos.x, self.last_rect.pos.y - inf),
                size: dvec2(self.last_rect.size.x, self.last_rect.size.y + inf * 2.0),
            };
            // Less overhang below than above: a cell's label sits at its
            // bottom, so a symmetric inflate pushed the frame down into the
            // next row and read as too tall.
            let bottom_inf = inf - RESIZE_FRAME_BOTTOM_TRIM;
            let frame = Self::clamp_rect(
                Rect {
                    pos: r.pos - dvec2(inf, inf),
                    size: r.size + dvec2(inf * 2.0, inf + bottom_inf),
                },
                bounds,
            );
            if self.drag_layer.is_none() {
                self.drag_layer = Some(DrawList2d::new(cx));
            }
            self.drag_layer.as_mut().unwrap().begin_overlay_reuse(cx);
            self.draw_resize.draw_abs(cx, frame);
            self.drag_layer.as_mut().unwrap().end(cx);
        }

        // Draw the dragged item last so it floats above everything, lifted with a
        // soft shadow and scaled up slightly (the classic "picked up" feel).
        //
        // Resolve what to draw using only Copy geometry plus a cheap WidgetRef (Rc)
        // clone — never a per-frame clone of the kind's owned String id. The dragged
        // widget is almost always already cached; only the first frame of a fresh
        // drawer drag-in has to create (and clone the id for) a brand-new icon.
        enum DragWidget {
            Ready(Option<WidgetRef>),
            MakeIcon(WidgetInstanceId, MiniAppId),
        }
        let dragged = self.drag.as_ref().map(|d| {
            let need = match &d.item.kind {
                PlacedKind::App { id, instance, .. } => match self.icons.get(instance) {
                    Some(icon) => DragWidget::Ready(Some(icon.clone())),
                    None => DragWidget::MakeIcon(*instance, id.clone()),
                },
                PlacedKind::Widget { instance, .. } => {
                    DragWidget::Ready(self.tiles.get(instance).cloned())
                }
            };
            (d.item.span(), d.pos, need)
        });
        if let Some((span, drag_pos, need)) = dragged {
            let base = dvec2(geom.cell.x * span.0 as f64, geom.cell.y * span.1 as f64);
            let scale = 1.08;
            let size = base * scale;
            // Grow around the item's center so it doesn't jump on lift.
            let pos = drag_pos - (size - base) * 0.5;
            let child_walk = Walk {
                abs_pos: Some(pos),
                margin: Default::default(),
                width: Size::Fixed(size.x),
                height: Size::Fixed(size.y),
                metrics: Default::default(),
            };
            // A drawer app being dragged in may have no home icon yet — create it.
            let widget = match need {
                DragWidget::Ready(w) => w,
                DragWidget::MakeIcon(instance, id) => {
                    let state = scope.data.get::<AppState>().unwrap();
                    self.ensure_icon(cx, state, instance, &id)
                }
            };
            // Draw the lift shadow + dragged item into their own overlay so a dragged
            // APP ICON (a plain view) floats ABOVE the widget tiles' glass lens
            // overlays instead of being occluded when it passes over a widget.
            if self.drag_layer.is_none() {
                self.drag_layer = Some(DrawList2d::new(cx));
            }
            self.drag_layer.as_mut().unwrap().begin_overlay_reuse(cx);
            self.draw_shadow.draw_abs(
                cx,
                Rect {
                    pos: pos + dvec2(0.0, 4.0),
                    size,
                },
            );
            if let Some(widget) = widget {
                widget.draw_walk_all(cx, scope, child_walk);
            }
            self.drag_layer.as_mut().unwrap().end(cx);
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
    /// The page index currently shown (used to know which page to delete).
    pub fn current_page_index(&self) -> usize {
        self.borrow().map_or(0, |inner| inner.current_page())
    }

    /// Deletes the page at `page` and its contents, snapping to a valid page.
    pub fn delete_page(&self, cx: &mut Cx, state: &mut AppState, page: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.delete_page(cx, state, page);
        }
    }

    /// Animates the pager to the given page, swiping smoothly through any pages in
    /// between (e.g. jumping to a freshly-added page).
    pub fn go_to_page(&self, cx: &mut Cx, layout: &LauncherLayout, page: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.settle_to(cx, layout, page as f64);
        }
    }

    /// Sets (or clears with `None`) which widget shows the resize indicator around
    /// it — set while that widget's context menu is open.
    /// Lends a live tile's RUNNING widget to the app screen: the pager keeps
    /// its own reference (so the isolate survives no matter what the app
    /// screen does with its copy) and draws a stand-in in the cells until the
    /// app comes back. Returns the widget to hand over.
    pub fn lend_app_host(
        &self,
        cx: &mut Cx,
        instance: WidgetInstanceId,
    ) -> Option<WidgetRef> {
        let mut inner = self.borrow_mut()?;
        let host = inner.app_tiles.get(&instance)?.clone();
        // Fullscreen chrome for its time away.
        host.widget(cx, ids!(tile_bar)).set_visible(cx, false);
        host.widget(cx, ids!(header)).set_visible(cx, true);
        inner.expanded_app = Some(instance);
        inner.redraw(cx);
        Some(host)
    }

    /// Takes the lent widget back into its cells: tile chrome again, and the
    /// stand-in stops drawing. Safe to call when nothing is lent.
    pub fn reclaim_app_host(&self, cx: &mut Cx) {
        let Some(mut inner) = self.borrow_mut() else { return };
        let Some(instance) = inner.expanded_app.take() else { return };
        if let Some(host) = inner.app_tiles.get(&instance).cloned() {
            // Back under the pager in the tree, mirroring where it draws again.
            cx.widget_tree_insert_child_deep(
                inner.uid,
                LiveId::from_str_num("atile", instance),
                host.clone(),
            );
            host.set_visible(cx, true);
            host.widget(cx, ids!(header)).set_visible(cx, false);
            // ...and the × inside it explicitly. It's a GlassButton, which
            // paints into its OWN overlay draw list; hiding only the parent
            // header leaves that overlay un-flushed and the button hangs over
            // the home screen forever. Hiding the button itself stops it being
            // re-begun, and the full repaint below clears the stale list.
            host.widget(cx, ids!(back_button)).set_visible(cx, false);
            host.widget(cx, ids!(tile_bar)).set_visible(cx, true);
            // Back on the home surface: silent again (see ensure_app_tile).
            if let Some(mut splash) = host.widget(cx, ids!(splash)).borrow_mut::<Splash>() {
                splash.set_host_prompts(cx, false);
            }
            cx.widget_tree_mark_dirty(inner.uid);
        }
        // Its content box changed while it was away; make the next draw
        // re-notify the script.
        inner.app_tile_sizes.remove(&instance);
        inner.redraw(cx);
    }

    /// Which placement, if any, currently has its app expanded to fullscreen.
    pub fn expanded_app(&self) -> Option<WidgetInstanceId> {
        self.borrow().and_then(|inner| inner.expanded_app)
    }

    /// Tears down the live host for a placement (used when it shrinks back to
    /// an icon, is removed, or its app is stopped).
    pub fn drop_app_host(&self, cx: &mut Cx, instance: WidgetInstanceId) {
        let Some(mut inner) = self.borrow_mut() else { return };
        inner.app_tiles.remove(&instance);
        inner.app_aways.remove(&instance);
        inner.app_tile_sizes.remove(&instance);
        inner.app_bar_rects.remove(&instance);
        if inner.expanded_app == Some(instance) {
            inner.expanded_app = None;
        }
        inner.redraw(cx);
    }

    /// Every app id with a live home tile, for teardown bookkeeping.
    pub fn live_app_tiles(&self) -> Vec<WidgetInstanceId> {
        self.borrow()
            .map(|inner| inner.app_tiles.keys().copied().collect())
            .unwrap_or_default()
    }

    pub fn set_resize_hint(&self, cx: &mut Cx, instance: Option<WidgetInstanceId>) {
        if let Some(mut inner) = self.borrow_mut() {
            if inner.resize_hint != instance {
                inner.resize_hint = instance;
                inner.redraw(cx);
            }
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

    /// Removes the item with the given key (after the remove is confirmed).
    pub fn remove_by_key(&self, cx: &mut Cx, state: &mut AppState, key: &ItemKey) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.remove_by_key(cx, state, key);
        }
    }

    /// Starts dragging a new app in from the drawer (see [`HomePager::begin_external_drag`]).
    /// Returns whether the drag actually started (false if the finger was already up).
    pub fn begin_external_drag(
        &self,
        cx: &mut Cx,
        layout: &LauncherLayout,
        app_id: MiniAppId,
        instance: WidgetInstanceId,
        abs: Vec2d,
        from_area: Area,
        from_dock: Option<usize>,
    ) -> bool {
        if let Some(mut inner) = self.borrow_mut() {
            inner.begin_external_drag(cx, layout, app_id, instance, abs, from_area, from_dock)
        } else {
            false
        }
    }

    /// The dock slot a hovering drag would drop into, if one is hovering the dock.
    pub fn dock_hover(&self) -> Option<usize> {
        self.borrow().and_then(|inner| inner.dock_hover)
    }

    /// Rebuild `app_id`'s cached grid icons from its current manifest (used
    /// after an AI refine swaps the manifest in place).
    /// See [`HomePager::update_app_caps`].
    pub fn update_app_caps(&self, cx: &mut Cx, layout: &LauncherLayout, app_id: &MiniAppId, caps: &[String]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.update_app_caps(cx, layout, app_id, caps);
        }
    }

    /// See [`HomePager::deliver_ipc`].
    pub fn deliver_ipc(
        &self,
        cx: &mut Cx,
        layout: &LauncherLayout,
        app_id: &MiniAppId,
        from: &str,
        data: &str,
        skip_heap: usize,
    ) -> usize {
        if let Some(mut inner) = self.borrow_mut() {
            inner.deliver_ipc(cx, layout, app_id, from, data, skip_heap)
        } else {
            0
        }
    }

    pub fn refresh_app_icons(&self, cx: &mut Cx, layout: &LauncherLayout, app_id: &MiniAppId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.refresh_app_icons(cx, layout, app_id);
        }
    }

    /// Drop `app_id`'s widget tiles (refine → rebuild from new source;
    /// uninstall → kill the isolate before its data dir is removed). Mark-dead
    /// only; the caller runs `gc_dead_splash_isolates` to reclaim.
    pub fn drop_app_widget_tiles(
        &self,
        cx: &mut Cx,
        layout: &LauncherLayout,
        app_id: &MiniAppId,
    ) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.drop_app_widget_tiles(cx, layout, app_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A dragged widget clears its landing zone by relocating BOTH icons and
    /// other widgets — previously anything multi-cell made the drop invalid.
    #[test]
    fn widget_reflow_relocates_icons_and_widgets() {
        let grid = (4u8, 6u8);
        let mut page = HomePage::default();
        // A 2x2 widget at (0,0) and two icons beside it.
        page.items.push(PlacedItem {
            kind: PlacedKind::Widget { instance: 1, app_id: "w".into(), cols: 2, rows: 2 },
            col: 0,
            row: 0,
        });
        page.items.push(PlacedItem {
            kind: PlacedKind::App { id: "a".into(), instance: 2, cols: 1, rows: 1 },
            col: 2,
            row: 0,
        });
        page.items.push(PlacedItem {
            kind: PlacedKind::App { id: "b".into(), instance: 3, cols: 1, rows: 1 },
            col: 3,
            row: 0,
        });

        // Drop an incoming 3x2 widget over the top-left: it overlaps the 2x2
        // widget AND one icon, so both must be re-placed.
        let dragged = ItemKey::Widget(99);
        let plan = HomePager::plan_widget_reflow(grid, &page, &dragged, 0, 0, 3, 2)
            .expect("both the widget and the icon should find new homes");
        assert_eq!(plan.len(), 2, "the 2x2 widget and the overlapped icon move");

        // Nothing may be parked back under the incoming footprint (cols 0-2,
        // rows 0-1), and the relocated widget must fit its 2x2 span on-grid.
        for (key, (c, r)) in &plan {
            let (w, h) = if matches!(key, ItemKey::Widget(_)) { (2, 2) } else { (1, 1) };
            assert!(
                *c >= 3 || *r >= 2,
                "{key:?} was parked back inside the dragged footprint at ({c},{r})"
            );
            assert!(c + w <= grid.0 && r + h <= grid.1, "{key:?} placed off-grid");
        }
    }

    /// The resize indicator's inflated quad is clamped to the pager bounds so a
    /// widget flush against a screen edge never draws its outline/handle off-screen.
    /// Regression test for the indicator bleeding past the right edge.
    #[test]
    fn clamp_keeps_indicator_within_bounds() {
        let bounds = Rect { pos: dvec2(0.0, 0.0), size: dvec2(400.0, 800.0) };
        // A frame overflowing the right and top edges (widget near the corner).
        let frame = Rect { pos: dvec2(360.0, -12.0), size: dvec2(80.0, 120.0) };
        let c = HomePager::clamp_rect(frame, bounds);
        assert!(c.pos.x >= bounds.pos.x - 0.01, "left within bounds");
        assert!(c.pos.y >= bounds.pos.y - 0.01, "top within bounds");
        assert!(
            c.pos.x + c.size.x <= bounds.pos.x + bounds.size.x + 0.01,
            "right within bounds"
        );
        assert!(
            c.pos.y + c.size.y <= bounds.pos.y + bounds.size.y + 0.01,
            "bottom within bounds"
        );
        // A frame already inside is returned unchanged.
        let inside = Rect { pos: dvec2(50.0, 60.0), size: dvec2(100.0, 100.0) };
        let same = HomePager::clamp_rect(inside, bounds);
        assert_eq!(same.pos, inside.pos);
        assert_eq!(same.size, inside.size);
    }
}
