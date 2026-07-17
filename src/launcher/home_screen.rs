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
        flow: Down
        padding: Inset{
            top: (12.0 + mod.widgets.SAFE_INSET_PAD_TOP),
            bottom: (6.0 + mod.widgets.SAFE_INSET_PAD_BOTTOM),
            left: (8.0 + mod.widgets.SAFE_INSET_PAD_LEFT),
            right: (8.0 + mod.widgets.SAFE_INSET_PAD_RIGHT),
        }

        // Edit-mode management bar (shown only while jiggling): page/widget/
        // wallpaper actions plus grid-size steppers, iOS/Android style.
        edit_bar := View{
            visible: false
            width: Fill
            height: Fit
            flow: Down
            spacing: 6
            padding: Inset{bottom: 8}
            View{
                width: Fill
                height: Fit
                flow: Right
                spacing: 6
                align: Align{x: 0.5}
                done_button := glass.GlassButton{
                    text: "Done"
                    height: 30
                    padding: Inset{left: 12, right: 12}
                    draw_text +: { text_style: theme.font_bold{font_size: 11} }
                }
                add_widget_button := glass.GlassButton{
                    text: "＋ Widget"
                    height: 30
                    padding: Inset{left: 12, right: 12}
                    draw_text +: { text_style: theme.font_bold{font_size: 11} }
                }
                wallpaper_button := glass.GlassButton{
                    text: "Wallpaper"
                    height: 30
                    padding: Inset{left: 12, right: 12}
                    draw_text +: { text_style: theme.font_bold{font_size: 11} }
                }
                add_page_button := glass.GlassButton{
                    text: "＋ Page"
                    height: 30
                    padding: Inset{left: 12, right: 12}
                    draw_text +: { text_style: theme.font_bold{font_size: 11} }
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

        home_pager := HomePager{
            width: Fill
            height: Fill
        }

        page_indicator := PageIndicator{}

        // A persistent favorites bar, shown on every page.
        dock := LauncherDock{}

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
}
