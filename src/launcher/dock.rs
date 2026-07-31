//! The bottom dock: a persistent glass bar of favorite app icons, shown on every
//! home page (like the iOS dock / Android favorites tray).
//!
//! It mirrors the home pager's "draw icon child widgets at absolute positions"
//! approach, but with a single fixed row and no paging, editing, or drag: the dock
//! is deliberately a calm, always-there launch strip. Tapping an icon opens the app
//! with the same zoom-from-rect animation as the grid; right-click or long-press
//! opens the app's shortcut menu.

use std::collections::HashMap;

use makepad_widgets::*;

use crate::{
    app::{AppState, MAX_DOCK_ITEMS},
    launcher::{home_pager::tile_tint_color, notif_badge::NotifBadgeWidgetRefExt},
    mini_apps::registry::MiniAppId,
};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherDockBase = #(LauncherDock::register_widget(vm))

    // The empty slot opened up under an item being dragged into the dock — the
    // dock's answer to the grid's drop-target outline, drawn in the gap the
    // icons shuffle aside to make.
    set_type_default() do #(DrawDockSlot::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn(){
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.box(2.5, 2.5, self.rect_size.x - 5.0, self.rect_size.y - 5.0, 14.0)
            sdf.fill(vec4(1.0, 1.0, 1.0, 0.08))
            sdf.stroke(vec4(1.0, 1.0, 1.0, 0.4), 1.5)
            return sdf.result
        }
    }

    mod.widgets.LauncherDock = set_type_default() do mod.widgets.LauncherDockBase{
        width: Fill
        height: 102

        // The full-width liquid-glass rounded rectangle the icons sit on: a real
        // lensing surface (same kit as the menus), refracting the backdrop.
        DockBar := glass.LensSurface{
            width: Fill
            height: Fill
            draw_bg +: {
                corner_radius: 22.0
                tint_color: #xf8fbff
                tint_alpha: 0.03
                border_alpha: 0.7
            }
        }

        // A single icon slot: a tinted glass tile with an emoji glyph, no label
        // (dock icons are label-less, iOS-style).
        DockIcon := View{
            width: Fill
            height: Fill
            flow: Overlay
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
            // The edit-mode "×" remove badge, straddling the top-left corner just
            // like a home-screen icon's. Hidden outside edit mode.
            View{
                width: Fill
                height: Fill
                align: Align{x: 0.0, y: 0.0}
                clip_x: false, clip_y: false
                badge := LauncherRemoveBadge{
                    margin: Inset{top: -8, left: -8}
                }
            }
        }
    }
}

/// Actions emitted by the dock for the app to handle.
#[derive(Clone, Debug, Default)]
pub enum DockAction {
    /// A dock icon was tapped; open the app, animating out from `from_rect`.
    OpenApp { app_id: MiniAppId, from_rect: Rect },
    /// A dock icon was long-pressed or right-clicked; show its shortcut menu.
    ShowContextMenu { app_id: MiniAppId, anchor: Rect },
    /// A lifted dock icon started moving: hand the still-down finger (captured by
    /// `area`) to the home pager so the same touch drags it out of the dock.
    DragOutApp { app_id: MiniAppId, area: Area, abs: Vec2d },
    /// The edit-mode "×" on a dock icon was tapped; drop it from the dock.
    RemoveFromDock(MiniAppId),
    #[default]
    None,
}

/// Finger movement below this (in points) still counts as a tap.
const TAP_SLOP: f64 = 8.0;
/// How long a press must be held (secs) to count as a long press.
const LONG_PRESS_SECS: f64 = 0.5;
/// The size of each dock icon tile.
const ICON_SIZE: f64 = 56.0;
/// Hit radius of the edit-mode "×" badge straddling an icon's top-left corner.
const BADGE_HIT_SIZE: f64 = 26.0;

/// The outlined empty slot shown where a dragged item would land in the dock.
#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawDockSlot {
    #[deref]
    draw_super: DrawQuad,
}

#[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
pub struct LauncherDock {
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
    /// Instantiated icon widgets, one per dock favorite.
    #[rust]
    icons: HashMap<MiniAppId, WidgetRef>,
    /// On-screen rect of each dock icon this frame, for hit-testing.
    #[rust]
    rects: Vec<(MiniAppId, Rect)>,

    /// The instantiated liquid-glass bar drawn behind the icons.
    #[rust]
    bar: Option<WidgetRef>,
    /// Overlay draw-list the icons render into, so they (and their overhanging
    /// notification badges) composite ABOVE the glass bar's lens overlay instead of
    /// being clipped by its top rim. Without this the bar's self-overlaying lens
    /// draws on top of the badges.
    #[rust]
    icon_layer: Option<DrawList2d>,

    /// The app currently being pressed, and where the press started.
    #[rust]
    pressed: Option<(MiniAppId, Vec2d)>,
    #[rust]
    long_press_timer: Timer,
    /// A lifted icon (long-pressed, or pressed in edit mode) and where the press
    /// started: moving it past the tap slop hands the drag to the home pager.
    #[rust]
    lifted: Option<(MiniAppId, Vec2d)>,
    /// Free-running clock (secs) driving the edit-mode jiggle, mirroring the pager.
    #[rust]
    jiggle_time: f64,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    last_frame_time: f64,
    /// Whether the edit-mode "×" badges are currently applied to the icons.
    #[rust]
    edit_visuals_applied: bool,
    /// The outline drawn in the slot a hovering drag would drop into.
    #[live]
    draw_slot: DrawDockSlot,
}

impl ScriptHook for LauncherDock {
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
        vm.cx_mut().widget_tree_mark_dirty(self.uid);
    }
}

impl WidgetNode for LauncherDock {
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
        if let Some(bar) = &self.bar {
            visit(live_id!(dock_bar), bar.clone());
        }
        for (id, child) in self.icons.iter() {
            visit(LiveId::from_str(id), child.clone());
        }
    }

    fn redraw(&mut self, cx: &mut Cx) {
        self.area.redraw(cx);
    }
}

impl LauncherDock {
    /// Ensures a child icon widget exists for the given app, tinted and glyphed.
    fn ensure_icon(&mut self, cx: &mut Cx, state: &AppState, app_id: &MiniAppId) -> Option<WidgetRef> {
        if let Some(icon) = self.icons.get(app_id) {
            return Some(icon.clone());
        }
        let template = self.templates.get(&live_id!(DockIcon))?;
        let template_value: ScriptValue = template.as_object().into();
        let icon = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        let manifest = state.registry.get(app_id)?;
        icon.label(cx, ids!(glyph)).set_text(cx, &manifest.icon);
        let tint = tile_tint_color(manifest.tint);
        let mut tile = icon.widget(cx, ids!(tile));
        script_apply_eval!(cx, tile, {
            draw_bg +: { color: #(tint) }
        });
        cx.widget_tree_insert_child_deep(self.uid, LiveId::from_str(app_id), icon.clone());
        icon.notif_badge(cx, ids!(notif))
            .set_count(state.notifications.get(app_id).copied().unwrap_or(0));
        // An icon created while already in edit mode needs its "×" straight away.
        icon.widget(cx, ids!(badge))
            .set_visible(cx, self.edit_visuals_applied);
        self.icons.insert(app_id.clone(), icon.clone());
        Some(icon)
    }

    /// Ensures the glass bar child exists (instantiated from its template).
    fn ensure_bar(&mut self, cx: &mut Cx) -> Option<WidgetRef> {
        if let Some(bar) = &self.bar {
            return Some(bar.clone());
        }
        let template = self.templates.get(&live_id!(DockBar))?;
        let template_value: ScriptValue = template.as_object().into();
        let bar = cx.with_vm(|vm| WidgetRef::script_from_value(vm, template_value));
        cx.widget_tree_insert_child_deep(self.uid, live_id!(dock_bar), bar.clone());
        self.bar = Some(bar.clone());
        Some(bar)
    }

    /// Drops `app_id`'s cached icon so the next draw rebuilds it from the
    /// (possibly changed) manifest — see the pager's `refresh_app_icons`.
    pub fn refresh_icon(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        if self.icons.remove(app_id).is_some() {
            cx.widget_tree_mark_dirty(self.uid);
            self.redraw(cx);
        }
    }

    /// Drops child widgets for apps no longer in the dock.
    fn prune_children(&mut self, cx: &mut Cx, dock: &[MiniAppId]) {
        let before = self.icons.len();
        self.icons.retain(|id, _| dock.contains(id));
        if before == self.icons.len() {
            return;
        }
        cx.widget_tree_mark_dirty(self.uid);
        // Marking dirty drops THIS widget's children from the tree, but the
        // surviving icons are still in `self.icons` — and `ensure_icon`
        // early-returns a cached ref without inserting, so nothing would ever
        // put them back. They stayed alive as refs while vanishing from the
        // tree, which is why removing one favourite made the REST of the dock
        // unaddressable (no icons, no hit-testing) until a full rebuild.
        if let Some(bar) = self.bar.clone() {
            cx.widget_tree_insert_child_deep(self.uid, live_id!(dock_bar), bar);
        }
        for (id, icon) in self.icons.clone() {
            cx.widget_tree_insert_child_deep(self.uid, LiveId::from_str(&id), icon);
        }
    }

    /// The absolute on-screen rect of a dock icon, rebasing its draw-relative rect
    /// onto the resolved dock area.
    fn icon_abs_rect(&self, cx: &mut Cx, rel: Rect) -> Rect {
        Rect {
            pos: self.area.rect(cx).pos + rel.pos,
            size: rel.size,
        }
    }

    /// Hit-tests the edit-mode "×" badge, which straddles each icon's top-left
    /// corner (same generous radius the home grid uses).
    fn badge_at(&self, cx: &mut Cx, abs: Vec2d) -> Option<MiniAppId> {
        let base = self.area.rect(cx).pos;
        self.rects
            .iter()
            .find(|(_, r)| (abs - (base + r.pos)).length() < BADGE_HIT_SIZE)
            .map(|(id, _)| id.clone())
    }

    /// The per-icon jiggle offset in edit mode: a tiny two-axis wobble with a
    /// per-app phase so no two icons move in sync (mirrors the home grid's).
    fn jiggle_offset(app_id: &MiniAppId, t: f64) -> Vec2d {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        app_id.hash(&mut h);
        let phase = (h.finish() % 1000) as f64 / 1000.0 * std::f64::consts::TAU;
        let w = 13.0;
        dvec2(
            0.8 * (w * t + phase).sin(),
            0.8 * (w * 1.11 * t + phase + 1.7).sin(),
        )
    }

    /// Shows/hides the edit-mode "×" badges to match edit mode, and kicks off the
    /// jiggle animation when entering it.
    fn sync_edit_visuals(&mut self, cx: &mut Cx, edit_mode: bool) {
        if self.edit_visuals_applied == edit_mode {
            return;
        }
        self.edit_visuals_applied = edit_mode;
        let icons: Vec<WidgetRef> = self.icons.values().cloned().collect();
        for icon in icons {
            icon.widget(cx, ids!(badge)).set_visible(cx, edit_mode);
        }
        if edit_mode {
            self.last_frame_time = 0.0;
            self.next_frame = cx.new_next_frame();
        }
        self.redraw(cx);
    }

    /// The absolute rect of the given dock icon (falls back to a unit rect).
    fn icon_anchor(&self, cx: &mut Cx, app_id: &MiniAppId) -> Rect {
        self.rects
            .iter()
            .find(|(id, _)| id == app_id)
            .map(|(_, r)| self.icon_abs_rect(cx, *r))
            .unwrap_or(Rect {
                pos: self.area.rect(cx).pos,
                size: dvec2(1.0, 1.0),
            })
    }

    /// The app icon under the given absolute point, if any.
    fn icon_at(&self, cx: &mut Cx, abs: Vec2d) -> Option<MiniAppId> {
        let base = self.area.rect(cx).pos;
        self.rects
            .iter()
            .find(|(_, r)| {
                Rect { pos: base + r.pos, size: r.size }.contains(abs)
            })
            .map(|(id, _)| id.clone())
    }
}

impl Widget for LauncherDock {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Forward to child widgets (glass bar needs events for its lens updates).
        let children: Vec<WidgetRef> = self
            .bar
            .iter()
            .cloned()
            .chain(self.icons.values().cloned())
            .collect();
        for w in children {
            w.handle_event(cx, event, scope);
        }

        let edit_mode = scope.data.get::<AppState>().is_some_and(|s| s.edit_mode);
        // Reveal/hide the "×" badges (and start the wobble) as edit mode toggles.
        self.sync_edit_visuals(cx, edit_mode);

        // A press while the create panel is expanded is a dismissal of that
        // panel, not a request to open a favourite — the grid already refuses
        // these (see `home_input_enabled`), and the dock was the remaining way
        // to open an app by accident while folding the composer away.
        //
        // This gates ONE outcome, not the whole widget. Returning early here
        // instead — which is what this did — cut the dock off from events it
        // has every right to: `home_input_enabled` counts an open context menu
        // as an overlay, so the moment a long press opened a favourite's own
        // menu the dock stopped hearing the finger that was still down, and
        // the menu could never be slid out of into a drag. Long-pressing after
        // a finished run didn't work either, since a console left up also
        // reads as "expanded".
        let can_open_app = scope
            .data
            .get::<AppState>()
            .is_some_and(|s| s.home_input_enabled);

        // Keep the edit-mode jiggle ticking while the dock is wobbling.
        if let Some(ne) = self.next_frame.is_event(event) {
            if edit_mode {
                let dt = if self.last_frame_time == 0.0 {
                    1.0 / 60.0
                } else {
                    (ne.time - self.last_frame_time).clamp(0.0, 0.1)
                };
                self.last_frame_time = ne.time;
                self.jiggle_time += dt;
                self.next_frame = cx.new_next_frame();
                self.redraw(cx);
            } else {
                self.last_frame_time = 0.0;
            }
        }

        if self.long_press_timer.is_event(event).is_some() {
            if let Some((app_id, start)) = self.pressed.take() {
                // Stay lifted so sliding the finger slips out of the menu straight
                // into a drag, exactly like a home-screen icon.
                self.lifted = Some((app_id.clone(), start));
                let anchor = self.icon_anchor(cx, &app_id);
                cx.widget_action(self.uid, DockAction::ShowContextMenu { app_id, anchor });
            }
        }

        let hit = event.hits_with_options(cx, self.area, HitOptions::new());
        match hit {
            Hit::FingerDown(fe) => {
                // Right-click opens the shortcut menu directly.
                if fe.device.mouse_button().is_some_and(|b| b.is_secondary()) {
                    if let Some(app_id) = self.icon_at(cx, fe.abs) {
                        let anchor = self.icon_anchor(cx, &app_id);
                        cx.widget_action(self.uid, DockAction::ShowContextMenu { app_id, anchor });
                    }
                    return;
                }
                if !fe.device.is_primary_hit() {
                    return;
                }
                if edit_mode {
                    // In jiggle mode the "×" takes precedence, and an icon lifts
                    // immediately so it can be dragged without a long press.
                    if let Some(app_id) = self.badge_at(cx, fe.abs) {
                        cx.widget_action(self.uid, DockAction::RemoveFromDock(app_id));
                        return;
                    }
                    if let Some(app_id) = self.icon_at(cx, fe.abs) {
                        self.lifted = Some((app_id, fe.abs));
                    }
                    return;
                }
                if let Some(app_id) = self.icon_at(cx, fe.abs) {
                    self.pressed = Some((app_id, fe.abs));
                    self.long_press_timer = cx.start_timeout(LONG_PRESS_SECS);
                }
            }
            Hit::FingerMove(fe) => {
                // A lifted icon that moves hands its still-down finger to the home
                // pager, which drags it out of the dock and onto the grid (or back
                // into the dock at a new slot).
                if let Some((app_id, start)) = self.lifted.clone() {
                    if (fe.abs - start).length() > TAP_SLOP {
                        cx.stop_timer(self.long_press_timer);
                        self.lifted = None;
                        self.pressed = None;
                        cx.widget_action(
                            self.uid,
                            DockAction::DragOutApp {
                                app_id,
                                // The dock's own area holds the capture (it hit-tests
                                // itself, not the per-icon views).
                                area: self.area,
                                abs: fe.abs,
                            },
                        );
                    }
                    return;
                }
                // Moving off the pressed icon cancels the pending long press.
                if let Some((_, start)) = &self.pressed {
                    if (fe.abs - *start).length() > TAP_SLOP {
                        cx.stop_timer(self.long_press_timer);
                        self.pressed = None;
                    }
                }
            }
            Hit::FingerUp(fe) => {
                cx.stop_timer(self.long_press_timer);
                self.lifted = None;
                if let Some((app_id, _)) = self.pressed.take() {
                    if can_open_app && self.icon_at(cx, fe.abs).as_ref() == Some(&app_id) {
                        let rel = self
                            .rects
                            .iter()
                            .find(|(id, _)| id == &app_id)
                            .map(|(_, r)| *r);
                        let from_rect = match rel {
                            Some(r) => self.icon_abs_rect(cx, r),
                            None => Rect { pos: fe.abs, size: dvec2(ICON_SIZE, ICON_SIZE) },
                        };
                        cx.widget_action(self.uid, DockAction::OpenApp { app_id, from_rect });
                    }
                }
            }
            _ => (),
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::flow_overlay());
        // The turtle rect here is the provisional draw origin (a Fill sibling above
        // the dock defers the real position). Children drawn with abs_pos relative
        // to it still land correctly, but stored hit-rects must be kept relative to
        // this origin and rebased onto `self.area` (the resolved rect) at hit time.
        let rect = cx.turtle().rect();

        let (dock, edit_mode, hover): (Vec<MiniAppId>, bool, Option<usize>) =
            match scope.data.get::<AppState>() {
                Some(state) => {
                    let dock: Vec<MiniAppId> = state
                        .layout
                        .dock
                        .iter()
                        .filter(|id| state.registry.contains(id))
                        .cloned()
                        .collect();
                    // Only promise a slot the drop can actually honour: a full dock
                    // sends the item to the grid instead, so opening a gap there
                    // would be a lie.
                    let hover = state
                        .dock_drop
                        .filter(|_| dock.len() < MAX_DOCK_ITEMS)
                        .map(|i| i.min(dock.len()));
                    (dock, state.edit_mode, hover)
                }
                None => {
                    cx.end_turtle_with_area(&mut self.area);
                    return DrawStep::done();
                }
            };
        self.prune_children(cx, &dock);

        self.rects.clear();
        if !dock.is_empty() || hover.is_some() {
            // The hovered slot takes a place in the row, so the icons shuffle aside
            // to open a real gap rather than the outline overlapping one of them.
            let n = (dock.len() + usize::from(hover.is_some())) as f64;
            // The glass pill spans the full dock width; icons spread across it
            // with equal gaps.
            let bar_w = rect.size.x;
            // The pill carries symmetric top/bottom padding around the icon row so
            // the icons sit centred with equal breathing room, and the top-right
            // notification badge (which overhangs the icon by a few px) still clears
            // the pill's top lensing rim without shoving the whole row off-centre.
            let bar_h = ICON_SIZE + 32.0;
            let bar_x = rect.pos.x + (rect.size.x - bar_w) * 0.5;
            let bar_y = rect.pos.y + (rect.size.y - bar_h) * 0.5;
            if let Some(bar) = self.ensure_bar(cx) {
                bar.draw_walk_all(
                    cx,
                    scope,
                    Walk {
                        abs_pos: Some(dvec2(bar_x, bar_y)),
                        margin: Default::default(),
                        width: Size::Fixed(bar_w),
                        height: Size::Fixed(bar_h),
                        metrics: Default::default(),
                    },
                );
            }

            // Keep the outermost icons clear of the pill's lensing rim, where the
            // refraction would visibly warp them.
            let edge_inset = 30.0_f64.min(bar_w * 0.08);
            let avail = bar_w - 2.0 * edge_inset;
            let gap = if n > 1.0 {
                (avail - n * ICON_SIZE) / (n - 1.0)
            } else {
                0.0
            };
            // Centre the icon row vertically in the pill.
            let icon_y = bar_y + (bar_h - ICON_SIZE) * 0.5;
            // Draw the icons into their own overlay so they (and the notification
            // badges overhanging their tops) composite ABOVE the glass bar's lens
            // overlay — otherwise the bar's self-overlaying lens rim clips the badge.
            if self.icon_layer.is_none() {
                self.icon_layer = Some(DrawList2d::new(cx));
            }
            self.icon_layer.as_mut().unwrap().begin_overlay_reuse(cx);
            let slot_x = |slot: usize| bar_x + edge_inset + slot as f64 * (ICON_SIZE + gap);
            // Outline the slot being hovered, under the icons.
            if let Some(h) = hover {
                self.draw_slot.draw_abs(
                    cx,
                    Rect {
                        pos: dvec2(slot_x(h), icon_y),
                        size: dvec2(ICON_SIZE, ICON_SIZE),
                    },
                );
            }
            for (i, app_id) in dock.iter().enumerate() {
                // Everything at or past the hovered slot steps one place right to
                // leave it empty.
                let slot = match hover {
                    Some(h) if i >= h => i + 1,
                    _ => i,
                };
                let icon_x = slot_x(slot);
                let icon_rect = Rect {
                    pos: dvec2(icon_x, icon_y),
                    size: dvec2(ICON_SIZE, ICON_SIZE),
                };
                // Store the icon rect relative to the draw origin; it's rebased onto
                // the resolved `self.area` when hit-testing (see `icon_at`).
                self.rects.push((
                    app_id.clone(),
                    Rect {
                        pos: icon_rect.pos - rect.pos,
                        size: icon_rect.size,
                    },
                ));
                let icon = {
                    let state = scope.data.get::<AppState>().unwrap();
                    self.ensure_icon(cx, state, app_id)
                };
                if let Some(icon) = icon {
                    // Wobble in edit mode. The stored hit-rect above stays un-jiggled
                    // (the offset is sub-pixel-ish, so taps still land naturally).
                    let draw_pos = if edit_mode {
                        icon_rect.pos + Self::jiggle_offset(app_id, self.jiggle_time)
                    } else {
                        icon_rect.pos
                    };
                    // The DockIcon view doesn't clip, so the badge overhangs freely;
                    // being in this overlay it now draws over the bar's lens rim.
                    icon.draw_walk_all(
                        cx,
                        scope,
                        Walk {
                            abs_pos: Some(draw_pos),
                            margin: Default::default(),
                            width: Size::Fixed(ICON_SIZE),
                            height: Size::Fixed(ICON_SIZE),
                            metrics: Default::default(),
                        },
                    );
                }
            }
            self.icon_layer.as_mut().unwrap().end(cx);
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}
