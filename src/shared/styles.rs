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
        wallpaper_veil := View{
            width: Fill
            height: Fill
            show_bg: true
            draw_bg.color: #x05070e18
        }
    }

    // A sleek dark-glass squircle behind an app icon's glyph. Its per-app tint is
    // set on `color` (see ensure_icon), but the shader deepens + desaturates it into
    // a rich, dark jewel tone (near-black at the bottom) rather than a bright flat
    // fill — a premium, modern icon plate, cohesive with the dark liquid-glass theme.
    mod.widgets.LauncherIconTile = View{
        width: 56
        height: 56
        align: Align{x: 0.5, y: 0.5}
        show_bg: true
        draw_bg +: {
            color: #x4a90d9
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let sz = self.rect_size
                // NOTE: sdf.box doubles this radius internally, so 0.105*56 ≈ 5.9
                // gives a ~12px visual corner — an iOS squircle, not a near-circle.
                let rad = sz.x * 0.105
                let tint = self.color.xyz
                // Deepen + slightly desaturate the raw app colour into a dark jewel
                // tone: rich and understated, never a kindergarten primary.
                let luma = tint.x * 0.3 + tint.y * 0.59 + tint.z * 0.11
                let muted = mix(tint, vec3(luma, luma, luma), 0.30)
                let deep = muted * 0.42
                // Vertical gradient: a touch of light at the top edge, near-black
                // at the bottom, for glassy depth.
                let top = mix(deep, vec3(1.0, 1.0, 1.0), 0.10)
                let bot = deep * 0.35
                let grad = mix(top, bot, self.pos.y)
                sdf.box(1.5, 1.5, sz.x - 3.0, sz.y - 3.0, rad)
                sdf.fill(vec4(grad.x, grad.y, grad.z, 1.0))
                // A soft glassy highlight hugging the top edge.
                let sheen = pow(clamp(1.0 - self.pos.y * 2.4, 0.0, 1.0), 2.5) * 0.14
                sdf.box(1.5, 1.5, sz.x - 3.0, sz.y - 3.0, rad)
                sdf.fill(vec4(1.0, 1.0, 1.0, sheen))
                // A fine light rim for the glass edge.
                sdf.box(1.5, 1.5, sz.x - 3.0, sz.y - 3.0, rad)
                sdf.stroke(vec4(1.0, 1.0, 1.0, 0.13), 1.0)
                return sdf.result
            }
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

    // The canonical launcher text input: rounded translucent field with a hairline
    // border and consistent left/right text padding so the placeholder/text never
    // sits flush against the edge. Used for every text field (drawer + search).
    mod.widgets.LauncherTextInput = TextInput{
        width: Fill
        height: 44
        padding: Inset{left: 14, right: 12, top: 10, bottom: 10}
        draw_bg +: {
            color: #xffffff14
            border_color: #xffffff26
            border_size: 1.0
            border_radius: 13.0
        }
        draw_text +: {
            text_style: theme.font_regular{font_size: 15}
        }
    }

    // An app's name under its icon. `width: Fill` (not Fit) is load-bearing:
    // Fit lets a long name grow past its cell and collide with the next icon's
    // label, which is what happens as soon as the window narrows. Bounded, the
    // label's default `right_wrap` flow wraps it, and two lines with an
    // ellipsis is the most a grid cell can carry before it starts eating the
    // row below.
    mod.widgets.LauncherIconName = Label{
        width: Fill
        height: Fit
        align: Align{x: 0.5}
        max_lines: 2
        text_overflow: Ellipsis
        text: ""
        draw_text +: {
            color: #xf2f6ffee
            text_style: theme.font_bold{font_size: 11}
        }
    }

    // A hosted mini-app: the chrome around a Splash isolate. ONE template
    // serves all three presentations — fullscreen, a split pane, and a live
    // tile on the home grid — because expanding a home tile must hand the
    // SAME running widget (same isolate, same state) to the app screen, not
    // start a second one. Whoever draws it picks the chrome: `header` for
    // fullscreen/split, `tile_bar` for a grid cell.
    mod.widgets.AppHost = RoundedView{
        width: Fill
        height: Fill
        flow: Down
        show_bg: true
        draw_bg +: {
            color: #x070c16f4
            border_color: #xffffff1e
            border_size: 1.0
            border_radius: 20.0
        }
        padding: Inset{top: 6, left: 6, right: 6, bottom: 6}

        header := View{
            width: Fill
            height: 44
            flow: Right
            spacing: 8
            align: Align{y: 0.5}
            padding: Inset{left: 4, right: 4}

            // The close (×) button, top-left. Drawn in SDF, NOT as a
            // glass.GlassButton: a GlassButton paints into its own overlay
            // draw list, and an overlay is only flushed when its codeflow
            // parent redraws — but MiniAppScreen::draw_walk early-returns
            // while Hidden without ever beginning its list. The result was a
            // × that hung over the home screen after every close, immune to
            // set_visible because the flag was already false. SDF has no
            // overlay, so it simply stops existing when it stops drawing.
            // Hit-tested by MiniAppScreen alongside split_button.
            back_button := View{
                width: 36
                height: 36
                show_bg: true
                draw_bg +: {
                    pixel: fn(){
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        sdf.circle(18.0, 18.0, 15.0)
                        sdf.fill(#xffffff1f)
                        sdf.move_to(12.5, 12.5)
                        sdf.line_to(23.5, 23.5)
                        sdf.stroke(#xf2f6ffee, 2.0)
                        sdf.move_to(23.5, 12.5)
                        sdf.line_to(12.5, 23.5)
                        sdf.stroke(#xf2f6ffee, 2.0)
                        return sdf.result
                    }
                }
            }
            glyph := Label{
                text: ""
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_regular{font_size: 16}
                }
            }
            title := Label{
                text: ""
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_bold{font_size: 14}
                }
            }
            View{width: Fill, height: 1}

            // Split-screen toggle, top-right: one screen outline with a
            // center divider, drawn in SDF (the theme font has no such
            // glyph). NOT two filled bars — that reads as a pause button.
            // Single app: dock to a pane and pick a partner; pick mode:
            // cancel back to fullscreen; split: fullscreen this pane's
            // app. Hit-tested by MiniAppScreen.
            split_button := View{
                width: 36
                height: 36
                show_bg: true
                draw_bg +: {
                    // 1.0 = horizontal divider (top/bottom split, what a
                    // tall window gets); 0.0 = vertical (side by side).
                    // Synced by sync_split_icons; the default matches the
                    // portrait phone shape.
                    horizontal: uniform(1.0)
                    pixel: fn(){
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        sdf.box(8.5, 10.5, 19.0, 15.0, 3.0)
                        sdf.stroke(#xd9e8ffd0, 1.6)
                        if self.horizontal > 0.5 {
                            sdf.rect(8.5, 17.2, 19.0, 1.6)
                        } else {
                            sdf.rect(17.2, 10.5, 1.6, 15.0)
                        }
                        sdf.fill(#xd9e8ffd0)
                        return sdf.result
                    }
                }
            }
        }

        // Home-cell chrome, hidden until a pager tile shows this host: the
        // app's name plus expand/shrink. It is also the GRAB BAR — the pager
        // hit-tests this rect for drags and long-presses, so the app body
        // below stays fully interactive. Its buttons are SDF-drawn: the
        // theme fonts have no diagonal-arrow glyphs (they render as boxes).
        tile_bar := View{
            visible: false
            width: Fill
            height: 26
            flow: Right
            spacing: 6
            align: Align{y: 0.5}
            padding: Inset{left: 8, right: 8}
            tile_glyph := Label{
                text: ""
                padding: 0
                margin: 0
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_regular{font_size: 11}
                }
            }
            // Fill + ellipsis, NOT Fit: a long app name used to grow the
            // label until it shoved the expand/shrink buttons off the end of
            // the bar. Bounded, it truncates and the buttons always show.
            // (This Fill also does the spacer's old job of pushing them
            // right, so there's no separate spacer view.)
            tile_title := Label{
                width: Fill
                height: Fit
                text: ""
                padding: 0
                margin: 0
                max_lines: 1
                text_overflow: Ellipsis
                draw_text +: {
                    color: #xdfe8ffdd
                    text_style: theme.font_bold{font_size: 11}
                }
            }
            // Expand to fullscreen: a diagonal arrow with heads at both ends.
            tile_expand := View{
                width: 24
                height: 24
                show_bg: true
                draw_bg +: {
                    pixel: fn(){
                        let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                        sdf.move_to(7.5, 16.5)
                        sdf.line_to(16.5, 7.5)
                        sdf.stroke(#xd9e8ffd0, 1.4)
                        sdf.move_to(12.0, 7.5)
                        sdf.line_to(16.5, 7.5)
                        sdf.line_to(16.5, 12.0)
                        sdf.stroke(#xd9e8ffd0, 1.4)
                        sdf.move_to(7.5, 12.0)
                        sdf.line_to(7.5, 16.5)
                        sdf.line_to(12.0, 16.5)
                        sdf.stroke(#xd9e8ffd0, 1.4)
                        return sdf.result
                    }
                }
            }
        }

        content := ScrollYView{
            width: Fill
            height: Fill
            flow: Down
            padding: Inset{left: 8, right: 8, top: 4, bottom: 8}
            splash := Splash{
                width: Fill
                height: Fit
            }
        }
    }

    // The "×" remove badge straddling an icon/widget's top-left corner in edit
    // mode, iOS-style. Drawn entirely in SDF (not a font glyph, which renders
    // thin and off-centre at this size): a soft drop shadow, a crisp opaque
    // light disc with a hairline ring, and a bold dark-grey X. `show_bg` with an
    // inline `pixel` shader replaces the default flat fill.
    mod.widgets.LauncherRemoveBadge = View{
        visible: false
        width: 24
        height: 24
        clip_x: false
        clip_y: false
        show_bg: true
        draw_bg +: {
            pixel: fn(){
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let c = self.rect_size * 0.5
                let r = c.x - 2.5
                // Soft ambient shadow, seated just below the disc.
                sdf.blur = 2.5
                sdf.circle(c.x, c.y + 1.0, r)
                sdf.fill(vec4(0.0, 0.0, 0.0, 0.30))
                sdf.blur = 0.0
                // Crisp opaque light disc (iOS material look).
                sdf.circle(c.x, c.y, r)
                sdf.fill(vec4(0.965, 0.965, 0.975, 1.0))
                // Hairline ring so the disc reads on light wallpaper too.
                sdf.circle(c.x, c.y, r)
                sdf.stroke(vec4(0.0, 0.0, 0.0, 0.10), 0.8)
                // The X: two crisp diagonal strokes in iOS label-grey.
                let d = r * 0.40
                sdf.move_to(c.x - d, c.y - d)
                sdf.line_to(c.x + d, c.y + d)
                sdf.move_to(c.x + d, c.y - d)
                sdf.line_to(c.x - d, c.y + d)
                sdf.stroke(vec4(0.24, 0.24, 0.26, 1.0), 1.7)
                return sdf.result
            }
        }
    }

}
