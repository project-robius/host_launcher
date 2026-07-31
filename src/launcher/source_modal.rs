//! A read-only look at a mini-app's Splash source, in a popup over App Info.
//!
//! Generated apps are the normal case here: the agent wrote the thing, and when
//! it misbehaves the first question is always "what did it actually write?".
//! Built-ins work too — the source is on disk either way.
//!
//! Uses makepad's `CodeView` (read-only, syntax-highlighted, no gutter) rather
//! than a Label, which is the same choice robrix makes for its event-source
//! modal.

use makepad_code_editor::code_view::CodeViewWidgetExt;
use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherSourceModalBase = #(LauncherSourceModal::register_widget(vm))

    mod.widgets.LauncherSourceModal = set_type_default() do mod.widgets.LauncherSourceModalBase{
        width: Fill
        height: Fill
        flow: Down
        margin: Inset{
            top: (28.0 + mod.widgets.SAFE_INSET_PAD_TOP),
            bottom: (28.0 + mod.widgets.SAFE_INSET_PAD_BOTTOM),
            left: (10.0 + mod.widgets.SAFE_INSET_PAD_LEFT),
            right: (10.0 + mod.widgets.SAFE_INSET_PAD_RIGHT),
        }

        glass.Panel{
            width: Fill
            height: Fill
            flow: Down
            padding: Inset{top: 12, bottom: 12, left: 0, right: 0}

            // Header: × on the left like every other page here, then what
            // you're looking at.
            // Subtitle sits BELOW the title row, not inside it — otherwise the
            // row centres on the two-line block and the × lands between the
            // lines rather than on the title.
            View{
                width: Fill
                height: Fit
                flow: Down
                padding: Inset{left: 10, right: 16, bottom: 8}
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 10
                    align: Align{y: 0.5}
                    sm_close := glass.GlassButton{
                        width: 36
                        height: 36
                        // Square, and Sdf2d.box doubles the radius, so 9 draws a
                        // perfect circle — a single glyph deserves a disc, not a pill.
                        draw_glass +: { corner_radius: uniform(9) }
                        // Measured, not guessed: centred on the row the ×
                        // landed 2px below the title's optical centre (the
                        // 22pt × and the ~16pt title have different ink
                        // boxes). A bottom margin in a y-centred row lifts it
                        // by half the margin.
                        margin: Inset{bottom: 4}
                        text: "×"
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 22}
                        }
                    }
                    sm_title := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #ffffff
                            text_style: theme.font_bold{font_size: 15}
                        }
                    }
                }
                // Spacer rather than a margin, so it lines up under the title
                // by construction (close width + the title row's spacing).
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    View{width: 48, height: 1}
                    sm_subtitle := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #x9dccffcc
                            text_style: theme.font_regular{font_size: 10.5}
                        }
                    }
                }
            }

            // The source itself. The editor scrolls BOTH axes on its own
            // (CodeEditor owns a ScrollBars), so there's no wrapping scroll
            // view — and x matters here: Splash lines run long and word wrap
            // is off, so a horizontal scrollbar is the difference between
            // reading the code and guessing at it.
            //
            // Size goes on `editor`, not on the CodeView: CodeView has no walk
            // of its own and forwards `walk()` to the editor, so `width:`/
            // `height:` set here would be "property not defined on type".
            sm_body := View{
                width: Fill
                height: Fill
                flow: Down
                padding: Inset{left: 14, right: 8, bottom: 8}
                sm_code := CodeView{
                    editor +: {
                        width: Fill
                        height: Fill
                        draw_text +: {
                            text_style: theme.font_code{font_size: 8.5}
                        }
                    }
                }
            }
        }
    }
}

/// What the modal is showing.
#[derive(Clone, Debug, Default)]
pub struct SourceContext {
    pub name: String,
    /// Line under the title: id, kind, and how big the source is.
    pub subtitle: String,
    pub source: String,
}

#[derive(Clone, Debug, Default)]
pub enum SourceModalAction {
    Close,
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherSourceModal {
    #[deref]
    view: View,
}

impl LauncherSourceModal {
    fn show(&mut self, cx: &mut Cx, context: SourceContext) {
        self.view.label(cx, ids!(sm_title)).set_text(cx, &context.name);
        self.view.label(cx, ids!(sm_subtitle)).set_text(cx, &context.subtitle);
        self.view.code_view(cx, ids!(sm_code)).set_text(cx, &context.source);
        self.view.redraw(cx);
    }
}

impl Widget for LauncherSourceModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        if self.view.glass_button(cx, ids!(sm_close)).clicked(actions) {
            cx.widget_action(self.widget_uid(), SourceModalAction::Close);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherSourceModalRef {
    pub fn show(&self, cx: &mut Cx, context: SourceContext) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, context);
        }
    }
}
