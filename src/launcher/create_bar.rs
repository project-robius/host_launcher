//! The "create an app" bar: a glass pill at the top of the home screen that
//! floats OVER the grid (see HomeScreen's two layers) rather than sitting in
//! it, so it can grow without shoving icons around. Describe what you want
//! ("a pomodoro timer", "dice roller with stats") and send — an AI agent (any
//! ACP agent; `octos acp` by default) writes a Splash mini-app, the launcher
//! validates it against the real parser, and the finished app lands on the
//! home screen.
//!
//! It has two faces in one box: idle, a multi-line composer that grows with
//! the prompt up to 75% of the screen; busy, the agent's console — status,
//! activity log, and the code streaming in. The prompt's space becomes the
//! console rather than a second surface opening somewhere else.
//!
//! The bar is declarative; all behavior lives in `App` (submit/cancel actions,
//! generation progress, the busy/idle/flash state flips), matching how the
//! edit bar is driven.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherCreateBar = View{
        width: Fill
        height: Fit
        // Float clear of the screen sides like the dock's pill.
        margin: Inset{left: 8, right: 8, bottom: 6}

        create_pill := glass.Group{
            width: Fill
            height: Fit
            flow: Right
            spacing: 10
            // TOP-aligned, not centred: once the prompt (or the agent's
            // output) grows tall, the ✨ should stay pinned at the top like a
            // chat composer, not drift to the middle of a big box.
            align: Align{x: 0.0, y: 0.0}
            padding: Inset{left: 16, right: 8, top: 6, bottom: 6}
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
            // Send is stacked UNDER the prompt rather than beside it — a
            // button in the same row would shorten every wrapped line, not
            // just the last one.
            create_idle := View{
                width: Fill
                height: Fit
                flow: Down
                create_input := LauncherTextInput{
                    // Grows with the prompt and caps at 75% of the screen,
                    // scrolling internally past that. Enter submits and
                    // Shift+Enter starts a new line (Cmd/Ctrl+Enter always
                    // submits), the usual composer bargain.
                    is_multiline: true
                    submit_on_enter: true
                    height: Fit{
                        min: FitBound.Abs(40),
                        max: FitBound.Rel{base: Base.Full, factor: 0.75},
                    }
                    // Transparent in every state — the pill is the chrome. The
                    // empty (hint-showing) state paints color_empty, not
                    // color, so the whole family has to be zeroed or the input
                    // draws a pill-in-a-pill.
                    //
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
                // Enter only submits when a PHYSICAL keyboard is present (a
                // soft keyboard's Enter has to be able to type a newline), so
                // a multi-line composer needs an explicit send affordance or
                // there'd be no way to submit on a phone at all. Shown once
                // there's something to send; the row is Fit, so while it's
                // hidden the bar keeps its one-line resting height.
                View{
                    width: Fill
                    height: Fit
                    align: Align{x: 1.0}
                    create_send := glass.GlassButtonProminent{
                        visible: false
                        width: Fit
                        height: 32
                        margin: Inset{right: 4, bottom: 4}
                        text: "Send"
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 12}
                        }
                    }
                }
            }

            // Busy state: the same space, now the agent's console — what it's
            // doing, and the code as it streams in. Hidden while idle.
            create_busy := View{
                visible: false
                width: Fill
                height: Fit
                flow: Down
                spacing: 6
                create_head := View{
                    width: Fill
                    height: 40
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
                    // Collapses the console back to this one status line.
                    create_collapse := ButtonFlatter{
                        width: Fit
                        height: 26
                        padding: Inset{left: 10, right: 10}
                        text: "︿︿"
                        draw_text +: {
                            color: #xd9e6ffdd
                            text_style: theme.font_bold{font_size: 12}
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
                // The reused prompt space: phases, tool calls and validation
                // errors above a live tail of the script being written.
                create_output := View{
                    width: Fill
                    height: Fit{max: FitBound.Rel{base: Base.Full, factor: 0.62}}
                    flow: Down
                    spacing: 6
                    padding: Inset{bottom: 6, right: 8}
                    clip_y: true
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
        }
    }
}
