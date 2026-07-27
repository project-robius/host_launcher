//! The "create an app" bar: a Google-search-bar-style glass pill at the top of
//! the home screen. Type what you want ("a pomodoro timer", "dice roller with
//! stats") and hit return — an AI agent (any ACP agent; `octos acp` by
//! default) writes a Splash mini-app, the launcher validates it against the
//! real parser, and the finished app lands on the home screen.
//!
//! The bar is declarative; all behavior lives in `App` (submit/cancel actions,
//! generation progress, the busy/idle/flash state flips), matching how the
//! edit bar is driven.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // The drop-down agent-activity panel: floats OVER the home grid (it lives
    // in an Overlay stack with the pager, so it never reflows the icons) and
    // shows what the agent is doing during a generation — phases, tool calls,
    // validation errors, and a live tail of the code being written. The ︿︿
    // button collapses it to a small ﹀﹀ chip.
    mod.widgets.LauncherActivityPanel = View{
        width: Fill
        height: Fit
        margin: Inset{left: 8, right: 8, top: 2}
        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 6
            padding: Inset{top: 10, bottom: 12, left: 16, right: 10}
            View{
                width: Fill
                height: Fit
                flow: Right
                align: Align{x: 0.0, y: 0.5}
                glass.Caption{text: "AGENT ACTIVITY" width: Fill}
                activity_collapse := ButtonFlatter{
                    width: Fit
                    height: 26
                    padding: Inset{left: 10, right: 10}
                    text: "︿︿"
                    draw_text +: {
                        color: #xd9e6ffaa
                        text_style: theme.font_bold{font_size: 11}
                    }
                }
            }
            activity_log := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xd9e6ffd8
                    text_style: theme.font_regular{font_size: 11.5}
                }
            }
            activity_stream := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #x9dccff90
                    text_style: theme.font_regular{font_size: 10}
                }
            }
        }
    }

    // The collapsed state's re-expand affordance: a small floating chip.
    mod.widgets.LauncherActivityChip = View{
        width: Fill
        height: Fit
        align: Align{x: 1.0}
        margin: Inset{right: 16, top: 2}
        activity_expand := glass.GlassButton{
            width: Fit
            height: 28
            padding: Inset{left: 12, right: 12}
            text: "﹀﹀"
            draw_text +: {
                text_style: theme.font_bold{font_size: 11}
            }
        }
    }

    mod.widgets.LauncherCreateBar = View{
        width: Fill
        height: Fit
        // Float clear of the screen sides like the dock's pill.
        margin: Inset{left: 8, right: 8, bottom: 6}

        create_pill := glass.Group{
            width: Fill
            height: 52
            flow: Right
            spacing: 10
            align: Align{x: 0.0, y: 0.5}
            padding: Inset{left: 16, right: 8}
            draw_bg +: {
                corner_radius: 26.0
                tint_color: #xf8fbff
                tint_alpha: 0.035
                border_alpha: 0.6
            }

            // A sparkle marks this as the AI entry point (emoji — the text
            // fonts have no ✦ glyph and render tofu). Tapping it opens the
            // provider setup modal.
            create_glyph := ButtonFlatter{
                width: Fit
                height: 40
                text: "✨"
                draw_text +: {
                    text_style: theme.font_regular{font_size: 15}
                }
            }

            // Idle state: the prompt input, wrapped in a View because raw
            // TextInput ignores set_visible (a Widget-trait no-op default).
            // Transparent in every state — the pill is the chrome. The empty
            // (hint-showing) state paints color_empty, not color, so the whole
            // family has to be zeroed or the input draws a pill-in-a-pill.
            create_idle := View{
                width: Fill
                height: Fit
                create_input := LauncherTextInput{
                    height: 40
                    // The pill already spaces the text off the ✨ (its own
                    // `spacing`), so drop the field's standard left inset —
                    // otherwise the two stack and the caret sits way in.
                    padding: Inset{left: 0, right: 12, top: 10, bottom: 10}
                    empty_text: "Create an app…"
                    draw_bg +: {
                        border_size: 0.0
                        color: #x00000000
                        color_hover: #x00000000
                        color_focus: #x00000000
                        color_down: #x00000000
                        color_empty: #x00000000
                        color_disabled: #x00000000
                    }
                }
            }

            // Busy state: live status + a stop button. Hidden while idle.
            create_busy := View{
                visible: false
                width: Fill
                height: Fit
                flow: Right
                spacing: 8
                align: Align{x: 0.0, y: 0.5}
                create_status := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #xd9e6ffcc
                        text_style: theme.font_regular{font_size: 14}
                    }
                }
                create_cancel := glass.GlassButton{
                    width: Fit
                    height: 36
                    text: "Stop"
                    draw_text +: {
                        text_style: theme.font_bold{font_size: 12}
                    }
                }
            }
        }
    }
}
