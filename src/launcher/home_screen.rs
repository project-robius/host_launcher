//! The home screen composition: the paged app grid with the page-indicator
//! dots and a drawer-handle chevron below it.
//!
//! Safe-area insets: the backdrop intentionally bleeds under the system bars,
//! so the home content pads itself out of the unsafe areas on all four sides.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.HomeScreen = View{
        width: Fill
        height: Fill
        // Two layers: the home column, and the create bar floating above it so
        // expanding the prompt (or the agent console) covers the grid instead
        // of squeezing it.
        flow: Overlay

        home_body := View{
            width: Fill
            height: Fill
            flow: Down
            // Left/right carry only the platform safe-area inset (0 on desktop), so the
            // grid runs edge-to-edge; widgets/icons can reach the screen sides. The dock
            // and edit bar re-inset themselves so they still float clear of the edges.
            padding: Inset{
                top: (12.0 + mod.widgets.SAFE_INSET_PAD_TOP),
                bottom: (6.0 + mod.widgets.SAFE_INSET_PAD_BOTTOM),
                left: (mod.widgets.SAFE_INSET_PAD_LEFT),
                right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
            }

            // Edit-mode management bar (shown only while jiggling): page/widget/
            // wallpaper actions plus grid-size steppers, iOS/Android style.
            edit_bar := View{
                visible: false
                width: Fill
                height: Fit
                // Clip vertically so the buttons are revealed cleanly as the bar
                // grows/shrinks during the enter/leave-edit-mode slide animation.
                clip_y: true
                flow: Down
                spacing: 6
                padding: Inset{bottom: 8}
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 6
                    align: Align{x: 0.5}
                    add_widget_button := glass.GlassButton{
                        text: "＋ Widget"
                        height: 30
                        padding: Inset{left: 11, right: 11}
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                    add_app_button := glass.GlassButton{
                        text: "＋ App"
                        height: 30
                        padding: Inset{left: 11, right: 11}
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                    wallpaper_button := glass.GlassButton{
                        text: "Wallpaper"
                        height: 30
                        padding: Inset{left: 11, right: 11}
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                    add_page_button := glass.GlassButton{
                        text: "＋ Page"
                        height: 30
                        padding: Inset{left: 12, right: 12}
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                    delete_page_button := glass.GlassButton{
                        text: "－ Page"
                        height: 30
                        padding: Inset{left: 12, right: 12}
                        draw_text +: {
                            color: #xff8a8a
                            text_style: theme.font_bold{font_size: 11}
                        }
                    }
                }
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 6
                    align: Align{x: 0.5, y: 0.5}
                    Label{
                        text: "Columns"
                        draw_text +: {
                            color: #xd9e8ffcc
                            text_style: theme.font_bold{font_size: 11}
                        }
                    }
                    col_minus := glass.GlassButton{
                        text: "−"
                        height: 26
                        padding: Inset{left: 10, right: 10}
                        draw_text +: { text_style: theme.font_bold{font_size: 12} }
                    }
                    cols_label := Label{
                        text: "4"
                        draw_text +: {
                            color: #ffffff
                            text_style: theme.font_bold{font_size: 13}
                        }
                    }
                    col_plus := glass.GlassButton{
                        text: "＋"
                        height: 26
                        padding: Inset{left: 10, right: 10}
                        draw_text +: { text_style: theme.font_bold{font_size: 12} }
                    }
                    View{width: 14, height: 1}
                    Label{
                        text: "Rows"
                        draw_text +: {
                            color: #xd9e8ffcc
                            text_style: theme.font_bold{font_size: 11}
                        }
                    }
                    row_minus := glass.GlassButton{
                        text: "−"
                        height: 26
                        padding: Inset{left: 10, right: 10}
                        draw_text +: { text_style: theme.font_bold{font_size: 12} }
                    }
                    rows_label := Label{
                        text: "6"
                        draw_text +: {
                            color: #ffffff
                            text_style: theme.font_bold{font_size: 13}
                        }
                    }
                    row_plus := glass.GlassButton{
                        text: "＋"
                        height: 26
                        padding: Inset{left: 10, right: 10}
                        draw_text +: { text_style: theme.font_bold{font_size: 12} }
                    }
                }
            }

            // Space the floating create bar occupies at rest, so the grid starts
            // below it. Hidden with the bar in edit mode.
            create_slot := View{
                width: Fill
                height: 58
            }

            home_pager := HomePager{
                width: Fill
                height: Fill
            }

            page_indicator := PageIndicator{}

            // A persistent favorites bar, shown on every page. Re-insets itself now
            // that the home screen no longer pads its sides, so the glass pill floats
            // clear of the screen edges.
            dock := LauncherDock{
                margin: Inset{left: 8, right: 8}
            }

            // A subtle chevron affordance: swipe up (or click it) to open the drawer.
            drawer_handle := ButtonFlatter{
                width: Fill
                height: 22
                align: Align{x: 0.5, y: 0.5}
                text: "︿"
                draw_text +: {
                    color: #xffffff55
                    text_style: theme.font_bold{font_size: 12}
                }
            }
        }

        // Layer 2: the AI "create an app" bar / agent console. It lives here
        // rather than in the column above so growing it (a multi-line prompt,
        // or the agent's streamed output) draws OVER the icons and widgets
        // instead of pushing them down. `create_slot` above reserves its
        // resting height, and both are hidden together in edit mode, so the
        // two layers stay aligned without any extra positioning.
        create_layer := View{
            width: Fill
            height: Fill
            flow: Down
            padding: Inset{
                top: (12.0 + mod.widgets.SAFE_INSET_PAD_TOP),
                left: (mod.widgets.SAFE_INSET_PAD_LEFT),
                right: (mod.widgets.SAFE_INSET_PAD_RIGHT),
            }
            create_bar := LauncherCreateBar{}
        }
    }
}
