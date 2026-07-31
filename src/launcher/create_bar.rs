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

    // One agent setting: a caption and a segmented control. GlassSegmented is
    // the app's own control for a small ordered choice — a lensing glass pill
    // that slides to the selection — where a stock dropdown looked like a
    // system widget dropped onto the glass.
    //
    // Its labels come from the DSL (makepad exposes no setter), so THESE MUST
    // MATCH generate::prefs — `segment_counts_match_the_dsl` pins the counts.
    let AgentOption = View{
        visible: false
        width: Fill
        height: Fit
        flow: Right
        spacing: 10
        align: Align{y: 0.5}
        // Fixed caption column so every control starts on the same line and
        // the segmented bars all share one width.
        ao_label := Label{
            width: 58
            text: ""
            draw_text +: {
                color: #x9dccffaa
                text_style: theme.font_regular{font_size: 10}
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
            height: Fit
            // A column: the composer line, then the agent options beneath it.
            // The options are a sibling of the whole line rather than a child
            // of the input's column, so they span the pill's full width
            // instead of starting after the ✨.
            flow: Down
            padding: Inset{left: 16, right: 12, top: 6, bottom: 8}

            create_head_row := View{
            width: Fill
            height: Fit
            flow: Right
            spacing: 10
            // TOP-aligned, not centred: once the prompt (or the agent's
            // output) grows tall, the ✨ should stay pinned at the top like a
            // chat composer, not drift to the middle of a big box.
            align: Align{x: 0.0, y: 0.0}
            draw_bg +: {
                corner_radius: 26.0
                tint_color: #xf8fbff
                tint_alpha: 0.035
                border_alpha: 0.6
            }

            // A sparkle marks this as the AI entry point (emoji — the text
            // fonts have no ✦ glyph and render tofu). Tapping it opens the
            // provider setup modal. Idle only: once the agent is working, the
            // console's show/hide chevron takes over this slot, so the
            // controls stay in one place instead of crowding the status row.
            create_glyph := ButtonFlatter{
                width: Fit
                height: 40
                text: "✨"
                draw_text +: {
                    text_style: theme.font_regular{font_size: 15}
                }
            }

            // Busy only, in the sparkle's slot: hides/shows the output. The
            // arrow rotates between right (hidden) and down (showing) — an SDF
            // triangle rather than a glyph, since the fonts have no arrowheads
            // and a text chevron can't animate. Overlay because Button draws
            // its own text/icon and takes no children: the button is the hit
            // target, the arrow is what you see.
            create_toggle_wrap := View{
                visible: false
                width: 26
                height: 40
                flow: Overlay
                align: Align{x: 0.5, y: 0.5}
                create_toggle := ButtonFlatter{
                    width: 26
                    height: 40
                    text: ""
                }
                create_arrow := ExpandArrow{
                    width: 15
                    height: 15
                    // Measured, not guessed. The row is top-aligned (so the
                    // chevron stays pinned when the console grows), so the
                    // 40-tall slot sits at the top of the row's 50 floor and
                    // centres on 70 — while the status text, which the label
                    // centres in its own Fill box, lands on 73.5. A bare
                    // centre therefore reads 3.5px high; a 7px top margin in
                    // this Overlay shifts the arrow by half that, onto the
                    // text's line.
                    margin: Inset{top: 7}
                    draw_bg.color: #xd9e6ffcc
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
                // The composer line: prompt and Send side by side. TOP-aligned,
                // so Send stays on the first line as the prompt grows rather
                // than riding down with the text — same anchor as the ✨ on the
                // other side of the pill.
                View{
                width: Fill
                height: Fit
                flow: Right
                spacing: 8
                align: Align{x: 0.0, y: 0.0}
                // ONE widget in this slot, always the real field. Collapsing
                // clamps its row count (see `max_lines` below) rather than
                // swapping in a display-only face, so the draft, the caret,
                // the selection and key focus are never disturbed — an earlier
                // two-widget version broke focus on every re-expand, because
                // hiding a focused TextInput leaves Cx pointing at it.
                create_prompt := View{
                    width: Fill
                    height: Fit
                    flow: Overlay
                create_input_wrap := View{
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
                    padding: Inset{left: 0, right: 4, top: 10, bottom: 10}
                    empty_text: "Create an app…"
                    // Collapsing is done ON THIS FIELD: `max_lines` clamps the
                    // laid-out ROW COUNT, and the layouter counts rows made by
                    // hard newlines exactly like wrapped ones (it splits on
                    // '\n' and checks the row cap inside that loop), so a
                    // pasted multi-line draft folds to one line too — unlike
                    // `is_multiline: false`, which only turns off soft wrap.
                    // App flips max_lines 1 <-> 0; the overflow mode is set
                    // here because an enum name can't be written from an eval.
                    // These live on DrawText, not on TextInput.
                    draw_text +: {
                        text_overflow: Ellipsis
                        max_lines: 1
                    }
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
                }
                // Enter only submits when a PHYSICAL keyboard is present (a
                // soft keyboard's Enter has to be able to type a newline), so
                // a multi-line composer needs an explicit send affordance or
                // there'd be no way to submit on a phone at all. Shown once
                // there's something to send.
                //
                // BESIDE the field, not under it: a row of its own doubled the
                // bar's height the moment you typed a character. Costs a fixed
                // strip of width instead, which is what every composer does.
                // Overlay because Button draws its own text/icon and takes no
                // children: the button is the hit target, the plane is what you
                // see. Every paper-plane glyph in these fonts is tofu, so the
                // icon is drawn (shared::send_arrow).
                // Top-aligned by the row, then nudged down so the disc's centre
            // sits on the first LINE of text rather than on the field's top
            // edge: the input pads 10 above its text, and half a ~19px line is
            // ~9.5, so the disc's 30px box starts 10 + 9.5 - 15 below.
            create_send_wrap := View{
                    // The composer is 50 tall with one line in it and pads 10
                    // top and bottom, so its line box is 30 — but a glyph's
                    // optical centre sits a little above that box's centre
                    // (the box carries descender space the capitals don't use).
                    // 11 lands the disc on the text; the line-box centre (13)
                    // read 2px low.
                    margin: Inset{top: 11}
                    visible: false
                    width: 30
                    height: 30
                    flow: Overlay
                    align: Align{x: 0.5, y: 0.5}
                    // A plain RoundedView disc rather than a glass button: a
                    // glass lens renders OVER later siblings, so the icon on top
                    // of one is invisible. ButtonFlatter is the hit target.
                    create_send_disc := RoundedView{
                        width: 30
                        height: 30
                        show_bg: true
                        draw_bg +: {
                            color: #x2f6fe8ff
                            border_radius: 15.0
                        }
                    }
                    // robrix's send icon (resources/send.svg) — a stroked paper
                    // plane. Every paper-plane CHARACTER in these fonts is tofu.
                    Icon{
                        icon_walk: Walk{width: 15, height: Fit}
                        draw_icon +: {
                            svg: crate_resource("self:resources/send.svg")
                            color: #xffffffff
                        }
                    }
                    create_send := ButtonFlatter{
                        width: 30
                        height: 30
                        text: ""
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
                    // Fit with a floor of 50 — the same height the composer's
                    // one-line prompt occupies (create_input's 40 min plus its
                    // 10/10 padding, measured). The bar must not change size
                    // when it swaps between the two faces: collapsed is
                    // collapsed, whichever one is showing.
                    height: Fit{min: FitBound.Abs(50)}
                    flow: Right
                    spacing: 8
                    align: Align{x: 0.0, y: 0.5}
                    // Only while a run is actually in flight — a finished
                    // console must not still look busy.
                    // Makepad's own spinner: a plain View whose shader reads
                    // `self.draw_pass.time`, which the platform detects and
                    // turns into a per-frame repaint — so it animates with no
                    // Rust, no timer and nothing to stop when it's hidden.
                    create_spinner := LoadingSpinner{
                        visible: false
                        width: 15
                        height: 15
                        margin: Inset{right: 2, bottom: 2}
                        draw_bg +: {
                            color: uniform(#xd9e6ffcc)
                            stroke_width: uniform(1.8)
                            rotation_speed: uniform(0.9)
                        }
                    }
                    create_status := Label{
                        width: Fill
                        text: ""
                        // On the LABEL, not on its draw_text: Label copies its
                        // own max_lines/text_overflow onto draw_text every draw
                        // (label.rs), so setting them inside `draw_text +:` is
                        // silently overwritten on the next frame.
                        //
                        // Exactly one line, ellipsised — the same clamp the
                        // prompt gets when it collapses, so the bar's resting
                        // height is the same whichever face is up. A long API
                        // error wrapped to four lines here and the "collapsed"
                        // bar came out taller than the expanded composer. The
                        // full text is in the log below.
                        max_lines: 1
                        text_overflow: Ellipsis
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
                    // Takes Stop's place after a failure: the request is still
                    // known, so re-running it (with the effort knob nudged up
                    // where there's room) beats retyping it.
                    create_retry := glass.GlassButtonProminent{
                        visible: false
                        width: Fit
                        height: 36
                        text: "Retry"
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 12}
                        }
                    }
                    // Success's counterpart to Retry: go straight into the app
                    // that was just made, without hunting for its new icon.
                    create_open := glass.GlassButtonProminent{
                        visible: false
                        width: Fit
                        height: 36
                        text: "Open"
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 12}
                        }
                    }
                }
                // The reused prompt space: the full run history — phases, tool
                // calls and validation errors — above a live tail of the
                // script being written.
                //
                // ScrollYView, so a long run stays readable: wheel and drag
                // both scroll (`drag_scrolling`). It grows with the log to the
                // cap and then scrolls; App also pins a floor under it so a
                // shrinking stream tail can never shrink the box.
                create_output := ScrollYView{
                    width: Fill
                    // Driven from App: CONSOLE_START_HEIGHT, then the content's
                    // height up to CONSOLE_MAX_FRACTION. It can't be Fit — a
                    // scrolling view takes whatever height it's offered, so Fit
                    // would open the console at the full cap however little it
                    // has to say.
                    height: 22
                    flow: Down
                    // NO clip_y: the scroll view clips already, and clipping
                    // here truncated console_body's measured rect to the
                    // viewport — so the panel could never grow past its own
                    // height and the scroll range came out near zero.
                    // The content, free to overrun the viewport (that's what
                    // there is to scroll). App measures THIS to decide the
                    // viewport's height — measuring the labels instead would
                    // just read back the clip, and the box could never grow.
                    console_body := View{
                        width: Fill
                        height: Fit
                        flow: Down
                        spacing: 6
                        padding: Inset{bottom: 6, right: 8}
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
                // Dismisses a finished run's console. A press outside the bar
                // does the same, but that's a thing you have to *know* — a
                // finished panel should say how to close it.
                create_footer := View{
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{x: 1.0, y: 0.5}
                    padding: Inset{top: 2, right: 2}
                    // "New prompt", not "Done": pressing outside already
                    // collapses and keeps everything, so this button's whole
                    // job is the destructive one — throw away the run's output
                    // and empty the field. Naming the RESULT ("you'll get a
                    // fresh prompt") rather than the act ("clear") says what
                    // you get, and "Done" read as "close this", which is what
                    // the collapse does.
                    create_done := glass.GlassButton{
                        width: Fit
                        height: 32
                        // The pill's own 8px bottom padding put the button
                        // almost against the glass edge; 6 more lifts it clear
                        // so it reads as sitting IN the panel, not on its rim.
                        margin: Inset{bottom: 6}
                        text: "New prompt"
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 12}
                        }
                    }
                }
            }
            }

            // Agent options — model / effort / thinking, revealed once the
            // prompt is focused or has text. WHICH controls appear is up to
            // the active backend (see generate::prefs): Claude Code takes
            // all three as env vars it reads itself, octos takes only a
            // model, a foreign ACP agent takes none. Hidden by default so
            // the resting bar stays a single line.
            create_options := View{
                visible: false
                width: Fill
                height: Fit
                flow: Down
                spacing: 4
                margin: Inset{top: 6}
                // One row per knob. Labels are literal because
                // GlassSegmented reads them from the DSL — keep them in
                // step with generate::prefs.
                opt_0 := AgentOption{
                    ao_seg_0 := glass.GlassSegmented{
                        width: Fill
                        height: 28
                        labels: ["Default", "Opus 5", "Sonnet 5", "Haiku 4.5"]
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 10}
                        }
                    }
                }
                // Two effort ladders, because GlassSegmented's labels are
                // fixed in the DSL and whether the runtime knows `xhigh`
                // isn't known until we've probed it (prefs::claude_efforts).
                // App shows exactly one.
                opt_1 := AgentOption{
                    ao_seg_1 := glass.GlassSegmented{
                        width: Fill
                        height: 28
                        labels: ["Default", "Low", "Medium", "High", "Max"]
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 10}
                        }
                    }
                    ao_seg_1x := glass.GlassSegmented{
                        visible: false
                        width: Fill
                        height: 28
                        labels: ["Default", "Low", "Medium", "High", "X-High", "Max"]
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 9.5}
                        }
                    }
                    // Kimi's Coding Plan ladder: no medium rung (octos clamps
                    // Medium up to "high" for k3, so offering it would be two
                    // settings that do the same thing).
                    ao_seg_1k := glass.GlassSegmented{
                        visible: false
                        width: Fill
                        height: 28
                        labels: ["Default", "Low", "High", "Max"]
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 10}
                        }
                    }
                }
                opt_2 := AgentOption{
                    ao_seg_2 := glass.GlassSegmented{
                        width: Fill
                        height: 28
                        labels: ["Default", "On", "Off"]
                        draw_text +: {
                            text_style: theme.font_bold{font_size: 10}
                        }
                    }
                }
                // Which agent these actually reach — the controls are
                // meaningless without it, and it's the only place the
                // launcher says which backend is live. Its own line: three
                // dropdowns already fill a phone-width pill, so sharing
                // their row just clipped it.
                // Which agent these reach, and the way in to change it. The
                // backend was named here already; making the name itself the
                // button is what turns "you have to know about the ✨" into
                // something you can find.
                View{
                    width: Fill
                    // Its own breathing room: at Fit the row read as squashed
                    // against the controls above it.
                    height: 38
                    flow: Right
                    spacing: 8
                    align: Align{x: 1.0, y: 0.5}
                    margin: Inset{top: 2}
                    create_backend := Label{
                        width: Fit
                        text: ""
                        draw_text +: {
                            color: #x9dccff88
                            text_style: theme.font_regular{font_size: 9.5}
                        }
                    }
                    // Text only, and glass: glass.GlassButton has no icon slot
                    // (it isn't a Button), and an icon layered on glass hides
                    // under the lens — the same trap the send button works
                    // around. The pill matters more here than the icon.
                    create_providers := glass.GlassButton{
                        width: Fit
                        height: 28
                        text: "Edit Providers"
                        draw_text +: { text_style: theme.font_bold{font_size: 10} }
                    }
                }
            }
        }
    }
}
