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

        home_pager := HomePager{
            width: Fill
            height: Fill
        }

        page_indicator := PageIndicator{}

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
