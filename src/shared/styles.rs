//! Shared launcher styling: the animated glass backdrop and the app icon tile pieces.
//!
//! The launcher follows the "liquid glass" look from makepad's aichat/glass examples:
//! a dark navy base, an animated vector-silk backdrop, and translucent glass surfaces
//! floating above it. Icon tiles use a cheap flat-translucent style (no gauss lensing)
//! since dozens of them are visible at once; real refracting glass is reserved for
//! larger, fewer surfaces like the drawer panel and mini-app chrome.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*

    // The window-filling animated backdrop: flowing vector silk behind everything,
    // plus a barely-there veil to seat text without dimming the glass refraction.
    mod.widgets.LauncherBackdrop = View{
        width: Fill
        height: Fill
        flow: Overlay
        Svg{
            width: Fill
            height: Fill
            animating: true
            draw_svg +: {
                preserve_aspect: false
                svg: crate_resource("self:resources/background.svg")
            }
        }
        View{
            width: Fill
            height: Fill
            show_bg: true
            draw_bg.color: #x05070e18
        }
    }

    // The rounded translucent square behind an app icon's glyph.
    mod.widgets.LauncherIconTile = RoundedView{
        width: 56
        height: 56
        align: Align{x: 0.5, y: 0.5}
        show_bg: true
        draw_bg +: {
            color: #xffffff14
            border_color: #xffffff22
            border_size: 1.0
            border_radius: 14.0
        }
    }

    // The emoji glyph drawn inside an icon tile.
    mod.widgets.LauncherIconGlyph = Label{
        width: Fit
        height: Fit
        text: "?"
        draw_text +: {
            color: #ffffff
            text_style: theme.font_regular{font_size: 26}
        }
    }

    // The app name drawn under an icon tile.
    mod.widgets.LauncherIconName = Label{
        width: Fit
        height: Fit
        text: ""
        draw_text +: {
            color: #xf2f6ffee
            text_style: theme.font_bold{font_size: 9.5}
        }
    }
}
