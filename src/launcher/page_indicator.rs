//! The row of dots under the home pager showing which page is visible.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets_internal.*
    use mod.widgets.*

    set_type_default() do #(DrawPageDot::script_shader(vm)){
        ..mod.draw.DrawQuad
        pixel: fn(){
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            sdf.circle(self.rect_size.x * 0.5, self.rect_size.y * 0.5, self.rect_size.x * 0.3)
            sdf.fill(vec4(1.0, 1.0, 1.0, 0.22 + 0.66 * self.active))
            return sdf.result
        }
    }

    mod.widgets.PageIndicatorBase = #(PageIndicator::register_widget(vm))

    mod.widgets.PageIndicator = set_type_default() do mod.widgets.PageIndicatorBase{
        width: Fill
        height: 18
    }
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawPageDot {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    active: f32,
}

#[derive(Script, WidgetRef, WidgetSet, WidgetRegister)]
pub struct PageIndicator {
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
    #[live]
    draw_dot: DrawPageDot,
    /// Continuous page position, so the active dot cross-fades during swipes.
    #[rust]
    position: f64,
    #[rust(1usize)]
    count: usize,
}

impl ScriptHook for PageIndicator {}

impl WidgetNode for PageIndicator {
    fn widget_uid(&self) -> WidgetUid {
        self.uid
    }

    fn walk(&mut self, _cx: &mut Cx) -> Walk {
        self.walk
    }

    fn area(&self) -> Area {
        self.area
    }

    fn children(&self, _visit: &mut dyn FnMut(LiveId, WidgetRef)) {}

    fn redraw(&mut self, cx: &mut Cx) {
        self.area.redraw(cx);
    }
}

impl Widget for PageIndicator {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, Layout::default());
        let rect = cx.turtle().rect();

        let dot = 7.0;
        let spacing = 14.0;
        let total = self.count as f64 * spacing;
        let start_x = rect.pos.x + (rect.size.x - total) * 0.5;
        let y = rect.pos.y + (rect.size.y - dot) * 0.5;
        for i in 0 .. self.count {
            let active = (1.0 - (self.position - i as f64).abs()).clamp(0.0, 1.0);
            self.draw_dot.active = active as f32;
            self.draw_dot.draw_abs(
                cx,
                Rect {
                    pos: dvec2(start_x + i as f64 * spacing + (spacing - dot) * 0.5, y),
                    size: dvec2(dot, dot),
                },
            );
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl PageIndicatorRef {
    pub fn set_state(&self, cx: &mut Cx, position: f64, count: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            if (inner.position - position).abs() > 0.001 || inner.count != count {
                inner.position = position;
                inner.count = count.max(1);
                inner.redraw(cx);
            }
        }
    }
}
