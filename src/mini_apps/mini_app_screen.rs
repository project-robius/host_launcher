//! The fullscreen host for running mini-apps.
//!
//! Each opened app gets its own host view (glass header + a scrollable `Splash`
//! instance) which stays alive after the app is "closed", iOS-style: closing
//! just hides the host, so reopening restores the app instantly with its state
//! intact. "Force stop" (from the context menu) drops the host entirely, which
//! tears down the app's Splash VM isolate; the next open starts it fresh.
//!
//! Opening animates the host from the tapped icon's rect out to (nearly) the
//! full screen; closing runs the same animation in reverse.

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

        AppHost := RoundedView{
            width: Fill
            height: Fill
            flow: Down
            show_bg: true
            draw_bg +: {
                color: #x070c16f4
                border_color: #xffffff1e
                border_size: 1.0
                border_radius: 20.0
            }
            padding: Inset{top: 6, left: 6, right: 6, bottom: 6}

            header := View{
                width: Fill
                height: 44
                flow: Right
                spacing: 8
                align: Align{y: 0.5}
                padding: Inset{left: 4, right: 4}

                // The single window control: a close (×) button top-left, iOS-
                // sheet-style. Uses U+00D7 (in the theme font) — the fancier U+2715
                // isn't in IBM Plex Sans and renders as a .notdef box.
                back_button := glass.GlassButton{
                    width: 36
                    height: 36
                    // Square, and Sdf2d.box doubles the radius, so 9 draws a
                    // perfect circle — a single glyph deserves a disc, not a pill.
                    draw_glass +: { corner_radius: uniform(9) }
                    text: "×"
                    draw_text +: {
                        text_style: theme.font_bold{font_size: 22}
                    }
                }
                glyph := Label{
                    text: ""
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_regular{font_size: 16}
                    }
                }
                title := Label{
                    text: ""
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 14}
                    }
                }
                View{width: Fill, height: 1}
            }

            content := ScrollYView{
                width: Fill
                height: Fill
                flow: Down
                padding: Inset{left: 8, right: 8, top: 4, bottom: 8}
                splash := Splash{
                    width: Fill
                    height: Fit
                }
            }
        }
    }
}

/// Duration of the open/close zoom animation, in seconds.
const ZOOM_SECS: f64 = 0.42;

/// The screen's animation phase.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Phase {
    #[default]
    Hidden,
    /// Zooming out from the icon rect; t goes 0 -> 1.
    Opening,
    Open,
    /// Zooming back into the icon rect; t goes 1 -> 0.
    Closing,
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
    active: Option<MiniAppId>,
    #[rust]
    phase: Phase,
    /// Animation progress 0 (icon rect) .. 1 (fullscreen).
    #[rust]
    t: f64,
    /// The icon rect the current open/close animates from/to.
    #[rust]
    anchor_rect: Rect,
    #[rust]
    next_frame: NextFrame,
    #[rust]
    last_frame_time: f64,
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
    /// Whether an app is currently shown (or animating in/out).
    pub fn is_showing(&self) -> bool {
        self.phase != Phase::Hidden
    }

    /// Whether an app fully covers the screen (not mid open/close zoom). Used to
    /// hide the home screen behind it — otherwise the home's glass widgets (and
    /// dock) render their refraction overlay *over* the app. During the zoom the
    /// home stays visible so the app animates over real content.
    pub fn is_fully_open(&self) -> bool {
        self.phase == Phase::Open
    }

    /// Whether the given app has a live (running) host instance.
    pub fn is_running(&self, app_id: &MiniAppId) -> bool {
        self.hosts.contains_key(app_id)
    }

    /// Opens the given app, creating its host (and Splash isolate) if needed,
    /// then zooms it out from `from_rect`.
    pub fn open_app(&mut self, cx: &mut Cx, manifest: &MiniAppManifest, from_rect: Rect) {
        let uid = self.widget_uid();
        if !self.hosts.contains_key(&manifest.id) {
            let Some(template) = self.templates.get(&live_id!(AppHost)) else {
                error!("BUG: MiniAppScreen is missing its AppHost template");
                return;
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
            if manifest.allow_net {
                if let Some(mut splash) = host.widget(cx, ids!(splash)).borrow_mut::<Splash>() {
                    splash.set_allow_net(true);
                }
            }
            // The app's private storage jail (its `fs` root), assigned BEFORE
            // the source evals so top-level fs.read boot loads see it.
            if let Some(mut splash) = host.widget(cx, ids!(splash)).borrow_mut::<Splash>() {
                splash.set_sandbox_dir(cx, Some(crate::app_sandbox_dir(&manifest.id)));
            }
            // Evaluating the source spins up the app's own isolated Splash VM.
            host.widget(cx, ids!(splash)).set_text(cx, &manifest.source);
            self.hosts.insert(manifest.id.clone(), host);
        }

        // Only the active host is visible; backgrounded (alive-but-hidden) hosts
        // are marked not-visible so their widgets report as hidden, even though
        // their Splash VM keeps running.
        for (id, host) in &self.hosts {
            host.set_visible(cx, id == &manifest.id);
        }
        self.active = Some(manifest.id.clone());
        self.anchor_rect = from_rect;
        self.phase = Phase::Opening;
        self.t = 0.0;
        self.last_frame_time = 0.0;
        self.next_frame = cx.new_next_frame();
        cx.redraw_all();
    }

    /// Starts the zoom-back-to-icon close animation.
    pub fn close_active(&mut self, cx: &mut Cx) {
        if matches!(self.phase, Phase::Open | Phase::Opening) {
            self.phase = Phase::Closing;
            self.last_frame_time = 0.0;
            self.next_frame = cx.new_next_frame();
            self.redraw(cx);
        }
    }

    /// Tears down an app's host entirely, killing its Splash VM isolate.
    /// The next open starts the app from scratch.
    pub fn force_stop(&mut self, cx: &mut Cx, app_id: &MiniAppId) {
        if self.hosts.remove(app_id).is_some() {
            if self.active.as_ref() == Some(app_id) {
                self.active = None;
                self.phase = Phase::Hidden;
            }
            cx.widget_tree_mark_dirty(self.widget_uid());
            // Reclaim the dropped host's Splash isolate promptly, and repaint
            // fully so no stale overlay draw lists linger.
            makepad_widgets::widget_async::gc_dead_splash_isolates(cx);
            cx.redraw_all();
        }
    }
}

impl Widget for MiniAppScreen {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        // Step the zoom animation.
        if let Some(ne) = self.next_frame.is_event(event) {
            if matches!(self.phase, Phase::Opening | Phase::Closing) {
                let dt = if self.last_frame_time == 0.0 {
                    1.0 / 60.0
                } else {
                    (ne.time - self.last_frame_time).clamp(0.0, 0.1)
                };
                self.last_frame_time = ne.time;
                let step = dt / ZOOM_SECS;
                match self.phase {
                    Phase::Opening => {
                        self.t += step;
                        if self.t >= 1.0 {
                            self.t = 1.0;
                            self.phase = Phase::Open;
                        } else {
                            self.next_frame = cx.new_next_frame();
                        }
                    }
                    Phase::Closing => {
                        self.t -= step;
                        if self.t <= 0.0 {
                            self.t = 0.0;
                            self.phase = Phase::Hidden;
                            // Now truly hidden: mark the host not-visible so its
                            // widgets report hidden while its VM stays alive.
                            if let Some(host) = self.active.as_ref().and_then(|id| self.hosts.get(id))
                            {
                                host.set_visible(cx, false);
                            }
                            cx.widget_action(self.widget_uid(), MiniAppScreenAction::FullyClosed);
                        } else {
                            self.next_frame = cx.new_next_frame();
                        }
                    }
                    _ => (),
                }
                // The zoom reveals/covers launcher content behind it.
                cx.redraw_all();
            }
        }

        if self.phase == Phase::Hidden {
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

        // Forward events to the active app's host only; backgrounded apps stay
        // frozen (except for network responses, delivered to all).
        if let Event::NetworkResponses(_) = event {
            let hosts: Vec<WidgetRef> = self.hosts.values().cloned().collect();
            for host in hosts {
                host.handle_event(cx, event, scope);
            }
        } else if let Some(host) = self.active.as_ref().and_then(|id| self.hosts.get(id)) {
            let host = host.clone();
            host.handle_event(cx, event, scope);
        }

        // The close button in the active host's header.
        if let Event::Actions(actions) = event {
            if let Some(host) = self.active.as_ref().and_then(|id| self.hosts.get(id)) {
                let host = host.clone();
                if host.glass_button(cx, ids!(back_button)).clicked(actions) {
                    self.close_active(cx);
                }
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.phase == Phase::Hidden {
            return DrawStep::done();
        }
        cx.begin_turtle(walk, Layout::flow_overlay());
        let rect = cx.turtle().rect();

        // The fully-open rect leaves a small margin so the backdrop peeks through,
        // keeping the glass-card look from the reference screenshots.
        let full = Rect {
            pos: rect.pos + dvec2(4.0, 4.0),
            size: rect.size - dvec2(8.0, 8.0),
        };
        let eased = 1.0 - (1.0 - self.t).powi(3);
        let current = Rect {
            pos: self.anchor_rect.pos + (full.pos - self.anchor_rect.pos) * eased,
            size: self.anchor_rect.size + (full.size - self.anchor_rect.size) * eased,
        };

        if let Some(host) = self.active.as_ref().and_then(|id| self.hosts.get(id)) {
            let host = host.clone();
            let host_walk = Walk {
                abs_pos: Some(current.pos),
                margin: Default::default(),
                width: Size::Fixed(current.size.x),
                height: Size::Fixed(current.size.y),
                metrics: Default::default(),
            };
            let mut scope = Scope::empty();
            host.draw_walk_all(cx, &mut scope, host_walk);
        }

        cx.end_turtle();
        DrawStep::done()
    }
}

impl MiniAppScreenRef {
    pub fn is_showing(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_showing())
    }

    pub fn is_fully_open(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.is_fully_open())
    }

    pub fn is_running(&self, app_id: &MiniAppId) -> bool {
        self.borrow().is_some_and(|inner| inner.is_running(app_id))
    }

    pub fn open_app(&self, cx: &mut Cx, manifest: &MiniAppManifest, from_rect: Rect) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.open_app(cx, manifest, from_rect);
        }
    }

    pub fn close_active(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.close_active(cx);
        }
    }

    pub fn force_stop(&self, cx: &mut Cx, app_id: &MiniAppId) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.force_stop(cx, app_id);
        }
    }
}
