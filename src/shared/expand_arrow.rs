//! An animated expand/collapse triangle: points right when collapsed, rotates
//! down when open. Drawn as an SDF rather than a glyph, which matters here —
//! the text fonts have no arrowheads (˄/˅ render as tofu), and the CJK compat
//! chevrons that *do* render (︿﹀) are wide, mismatched and can't animate.
//!
//! Ported from robrix (`src/shared/expand_arrow.rs`), same project family.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.ExpandArrowBase = #(ExpandArrow::register_widget(vm))

    mod.widgets.ExpandArrow = set_type_default() do mod.widgets.ExpandArrowBase {
        width: 16, height: 16,

        draw_bg +: {
            opened: instance(0.0)
            color: instance(#xd9e6ffcc)
            border_radius: uniform(2.0)

            pixel: fn() {
                let corner_round = self.border_radius
                let sz = self.rect_size.x * 0.3 - corner_round * 0.5
                let c = vec2(self.rect_size.x * 0.5, self.rect_size.y * 0.5)
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.clear(vec4(0.0))

                // Triangle pointing up; rotation maps opened to:
                //   0.0 -> 90deg (right-pointing, collapsed)
                //   1.0 -> 180deg (down-pointing, expanded)
                sdf.rotate(self.opened * 0.5 * PI + 0.5 * PI, c.x, c.y)
                sdf.move_to(c.x - sz, c.y + sz)
                sdf.line_to(c.x, c.y - sz)
                sdf.line_to(c.x + sz, c.y + sz)
                sdf.close_path()

                // Fill, then stroke slightly wider to round the sharp corners
                // geometrically (no blur).
                sdf.fill_keep(self.color)
                return sdf.stroke(self.color, corner_round)
            }
        }

        animator: Animator{
            expand: {
                default: @collapsed
                collapsed: AnimatorState{
                    from: {all: Forward {duration: 0.15}}
                    ease: ExpDecay {d1: 0.96, d2: 0.97}
                    redraw: true
                    apply: { draw_bg: {opened: 0.0} }
                }
                expanded: AnimatorState{
                    from: {all: Forward {duration: 0.15}}
                    ease: ExpDecay {d1: 0.98, d2: 0.95}
                    redraw: true
                    apply: { draw_bg: {opened: 1.0} }
                }
            }
        }
    }
}

/// Animated expand/collapse triangle arrow.
#[derive(Script, ScriptHook, Widget, Animator)]
pub struct ExpandArrow {
    #[uid]
    uid: WidgetUid,
    #[source]
    source: ScriptObjectRef,
    #[apply_default]
    animator: Animator,
    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[walk]
    walk: Walk,
    /// The desired open state, applied to `draw_bg.opened` while drawing (so
    /// it survives a redraw that lands between event and animation).
    #[rust]
    opened_value: f32,
}

impl ExpandArrow {
    /// Animates open/closed. Event handlers only — not during a draw.
    pub fn set_is_open(&mut self, cx: &mut Cx, is_open: bool, animate: Animate) {
        self.opened_value = if is_open { 1.0 } else { 0.0 };
        self.animator_toggle(cx, is_open, animate, ids!(expand.expanded), ids!(expand.collapsed))
    }
}

impl Widget for ExpandArrow {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.animator_handle_event(cx, event).must_redraw() {
            self.draw_bg.redraw(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        if !self.animator.is_track_animating(id!(expand)) {
            self.draw_bg.set_dyn_instance(cx, id!(opened), &[self.opened_value]);
        }
        self.draw_bg.draw_walk(cx, walk);
        DrawStep::done()
    }
}
