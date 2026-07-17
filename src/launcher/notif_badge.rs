//! A notification-count badge shown on the top-right corner of an app icon,
//! iOS-style: a red oval with the count in white, capped at "99+".
//!
//! Modeled on Robrix's UnreadBadge: a fixed-size view whose inner oval shrinks
//! (via `border_size`) when the count is short, and whose red core fades out
//! through a lighter warm color so it stays crisp on the colorful backdrop.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.NotifBadge = #(NotifBadge::register_widget(vm)) {
        visible: false
        width: 27, height: 18,
        align: Align{x: 0.5, y: 0.5}
        flow: Overlay,
        // Let the badge's fade-out glow render beyond its bounding rect.
        clip_x: false,
        clip_y: false,

        rounded_view := View {
            width: Fill,
            height: Fill,
            show_bg: true,
            clip_x: false,
            clip_y: false,

            draw_bg +: {
                badge_color: instance(#xFF1133)
                border_radius: instance(4.0)
                // A larger border size results in a smaller oval.
                border_size: instance(2.0)
                // Fade the red core through a lighter warm color to reduce
                // aliasing against the busy backdrop.
                fade_color: instance(#xFFC8B0)
                fade_radius: uniform(5.0)

                vertex: fn() {
                    let m = self.fade_radius
                    return self.clip_and_transform_vertex(
                        self.rect_pos - vec2(m),
                        self.rect_size + vec2(m * 2.0)
                    )
                }

                pixel: fn() {
                    let m = self.fade_radius
                    let rs3 = self.rect_size + vec2(m * 2.0)
                    let sdf = Sdf2d.viewport(self.pos * rs3)
                    let bw = self.rect_size.x - (self.border_size * 2.0)
                    let bh = self.rect_size.y - 2.0
                    let bx = m + self.border_size
                    let by = m + 1.0
                    let rad = max(1.0, self.border_radius)
                    sdf.box(bx, by, bw, bh, rad)
                    let dist = sdf.shape
                    let half_bh = bh * 0.5
                    let aa = clamp(0.5 - dist, 0.0, 1.0)
                    let band_start = -half_bh * 0.45
                    let t = clamp((dist - band_start) / (m - band_start), 0.0, 1.0)
                    let s = t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
                    let warm = mix(self.badge_color.rgb, self.fade_color.rgb, s)
                    sdf.clear(vec4(warm, 1.0 - s))
                    return sdf.result;
                }
            }
        }
        label_count := Label {
            padding: 0,
            width: Fit,
            height: Fit,
            flow: Right, // do not wrap
            text: "",
            draw_text +: {
                color: #ffffff,
                text_style: theme.font_bold{font_size: 8.0},
            }
        }
    }
}

#[derive(Script, ScriptHook, Widget)]
pub struct NotifBadge {
    #[source]
    source: ScriptObjectRef,
    #[deref]
    view: View,
    #[live]
    count: u64,
}

impl Widget for NotifBadge {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        if self.count == 0 {
            self.visible = false;
            return self.view.draw_walk(cx, scope, walk);
        }
        // The oval needs to be wider for longer text: a bigger border size
        // shrinks it around a short count, and counts above 99 show "99+".
        let (border_size, plus_sign) = if self.count > 99 {
            (0.0, "+")
        } else if self.count > 9 {
            (2.0, "")
        } else {
            (5.0, "")
        };
        self.label(cx, ids!(label_count))
            .set_text(cx, &format!("{}{plus_sign}", self.count.min(99)));
        let mut rounded_view = self.view(cx, ids!(rounded_view));
        script_apply_eval!(cx, rounded_view, {
            draw_bg +: { border_size: #(border_size) }
        });
        self.visible = true;
        self.view.draw_walk(cx, scope, walk)
    }
}

impl NotifBadgeRef {
    /// Sets the count shown by the badge (0 hides it).
    pub fn set_count(&self, count: u64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.count = count;
            inner.visible = count > 0;
        }
    }
}
