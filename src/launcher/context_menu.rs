//! The long-press context menu for apps and widgets, shown inside a Modal.
//!
//! Which entries appear depends on where the long-press happened (home screen
//! icon, home screen widget, or drawer) and on the app itself (built-ins can't
//! be uninstalled; only apps that provide a widget offer "Add widget").

use makepad_widgets::*;

use crate::mini_apps::registry::{MiniAppId, WidgetInstanceId};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    // A menu row: a flat, full-width button that highlights on hover. The whole
    // menu floats on a liquid-glass panel, so the rows stay light (iOS-style).
    // Every standard entry carries an icon, so the menu scans by shape rather
    // than by reading each line. ButtonFlatter (not the glass button) because
    // only it draws `draw_icon`.
    let MenuButton = ButtonFlatter{
        width: Fill
        height: 38
        align: Align{x: 0.0, y: 0.5}
        padding: Inset{left: 16, right: 16}
        draw_text +: {
            color: #xf2f6ff
            text_style: theme.font_regular{font_size: 13}
        }
        draw_icon +: { color: #xd7e2f5 }
        icon_walk: Walk{width: 16, height: 16, margin: Inset{right: 10}}
    }

    let MenuDivider = View{
        width: Fill
        height: 2
        margin: Inset{top: 6, bottom: 6, left: 14, right: 14}
        show_bg: true
        // Has to read as a rule over BRIGHT lensed glass, where the original
        // 10%-white hairline simply vanished. Drawn as a soft-ended line so it
        // looks deliberate rather than like a seam.
        draw_bg +: {
            pixel: fn(){
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                sdf.box(0.0, 0.5, self.rect_size.x, 1.0, 0.5)
                sdf.fill(vec4(1.0, 1.0, 1.0, 0.34))
                return sdf.result
            }
        }
    }

    // A size preset chip in the widget gallery (e.g. "2×2"). The selected one is
    // shown with a leading dot via set_text, so no per-chip styling state is needed.
    let SizeChip = ButtonFlatter{
        width: Fit
        height: 30
        padding: Inset{left: 13, right: 13}
        draw_text +: {
            color: #xd9e8ff
            text_style: theme.font_bold{font_size: 12}
        }
    }

    mod.widgets.LauncherContextMenuBase = #(LauncherContextMenu::register_widget(vm))

    // A small callout triangle bridging the menu to the icon it belongs to. One
    // points up (shown when the menu sits below the icon), one down (menu above).
    // Positioned horizontally at the icon's centre by `set_callout`.
    let CalloutUp = View{
        visible: false
        width: Fill
        height: 9
        clip_x: false, clip_y: false
        callout_tri := View{
            width: 26
            height: 9
            show_bg: true
            draw_bg +: {
                pixel: fn(){
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                    let w = self.rect_size.x
                    let h = self.rect_size.y
                    sdf.move_to(w * 0.5, 0.0)
                    sdf.line_to(w, h)
                    sdf.line_to(0.0, h)
                    sdf.close_path()
                    sdf.fill(vec4(0.58, 0.64, 0.79, 0.86))
                    return sdf.result
                }
            }
        }
    }
    let CalloutDown = View{
        visible: false
        width: Fill
        height: 9
        clip_x: false, clip_y: false
        callout_tri := View{
            width: 26
            height: 9
            show_bg: true
            draw_bg +: {
                pixel: fn(){
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                    let w = self.rect_size.x
                    let h = self.rect_size.y
                    sdf.move_to(0.0, 0.0)
                    sdf.line_to(w, 0.0)
                    sdf.line_to(w * 0.5, h)
                    sdf.close_path()
                    sdf.fill(vec4(0.58, 0.64, 0.79, 0.86))
                    return sdf.result
                }
            }
        }
    }

    mod.widgets.LauncherContextMenu = set_type_default() do mod.widgets.LauncherContextMenuBase{
        width: 268
        height: Fit
        flow: Down

        callout_up := CalloutUp{}

        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 0
            padding: Inset{top: 10, bottom: 10, left: 0, right: 0}
            // The stock panel shadow is tight (radius 13) and fairly opaque
            // (#x0007) against a corner_radius of 10 — over a bright wallpaper
            // that left a hard dark wedge just outside each corner, which read
            // as the corner not being rounded at all. A wider, softer shadow
            // with a radius that matches the corner dissolves it.
            draw_bg +: {
                corner_radius: 14.0
                shadow_color: #x0004
                shadow_radius: 20.0
                shadow_offset: vec2(0.0, 6.0)
            }

            title_row := View{
                width: Fill
                height: Fit
                flow: Right
                spacing: 11
                align: Align{y: 0.5}
                padding: Inset{left: 16, right: 16, bottom: 6}
                glyph := Label{
                    text: ""
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_regular{font_size: 22}
                    }
                }
                title := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 15}
                    }
                }
            }
            MenuDivider{}

            // Up to four app quick-action shortcuts.
            shortcut_0 := MenuButton{visible: false}
            shortcut_1 := MenuButton{visible: false}
            shortcut_2 := MenuButton{visible: false}
            shortcut_3 := MenuButton{visible: false}
            shortcut_divider := MenuDivider{visible: false}

            // ⓘ is U+24D8, a real circled i in the text font — so it takes the
            // row's colour like the label does. The emoji ℹ️ is a blue SQUARE
            // on Apple, and U+1F6C8 (🛈) is tofu here.
            info_button := MenuButton{
                text: "App Info"
                draw_icon +: { svg: crate_resource("self:resources/icons/info.svg") }
            }
            add_home_button := MenuButton{
                text: "Add to Home Screen"
                draw_icon +: { svg: crate_resource("self:resources/icons/home.svg") }
            }
            add_widget_button := MenuButton{
                text: "Add Widget to Home"
                draw_icon +: { svg: crate_resource("self:resources/icons/add.svg") }
            }
            remove_home_button := MenuButton{
                text: "Remove from Home"
                draw_icon +: { svg: crate_resource("self:resources/icons/close.svg") }
            }
            remove_widget_button := MenuButton{
                text: "Remove Widget"
                draw_icon +: { svg: crate_resource("self:resources/icons/close.svg") }
            }
            modify_button := MenuButton{
                text: "Modify App…"
                draw_icon +: { svg: crate_resource("self:resources/icons/edit.svg") }
            }
            uninstall_button := MenuButton{
                text: "Uninstall"
                draw_text +: { color: #xff8888 }
                // Icon tinted to match the label: this is the destructive row.
                draw_icon +: {
                    svg: crate_resource("self:resources/icons/trash.svg")
                    color: #xff8888
                }
            }
            uninstall_disabled_label := Label{
                visible: false
                width: Fill
                height: 38
                align: Align{x: 0.0, y: 0.5}
                padding: Inset{left: 16, right: 16}
                text: "Uninstall (pre-installed)"
                draw_text +: {
                    color: #x8fa6c8aa
                    text_style: theme.font_regular{font_size: 13}
                }
            }
        }

        callout_down := CalloutDown{}
    }

    mod.widgets.LauncherWidgetPickerBase = #(LauncherWidgetPicker::register_widget(vm))

    // The widget gallery shown from edit mode. Two stages on one panel:
    //  1. `wp_list` — one row per app that provides a home-screen widget.
    //  2. `wp_detail` — a LIVE preview of the chosen widget's Splash plus size
    //     chips; revealed once a row is tapped, hidden again on "Back".
    mod.widgets.LauncherWidgetPicker = set_type_default() do mod.widgets.LauncherWidgetPickerBase{
        width: 300
        height: Fit
        flow: Down

        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 0
            padding: Inset{top: 10, bottom: 10, left: 0, right: 0}

            wp_title := Label{
                margin: Inset{left: 16, bottom: 6}
                text: "Add Widget"
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_bold{font_size: 15}
                }
            }
            MenuDivider{}

            wp_list := View{
                width: Fill
                height: Fit
                flow: Down
                wp_0 := MenuButton{visible: false}
                wp_1 := MenuButton{visible: false}
                wp_2 := MenuButton{visible: false}
                wp_3 := MenuButton{visible: false}
                wp_4 := MenuButton{visible: false}
                wp_5 := MenuButton{visible: false}
            }

            wp_detail := View{
                visible: false
                width: Fill
                height: Fit
                flow: Down
                spacing: 10
                padding: Inset{left: 14, right: 14, top: 2}

                // Live preview: the real widget Splash, on a glass card, resized
                // to the chosen span by the Rust side (which also calls the
                // widget's on_widget_resize so content reflows exactly as on home).
                View{
                    width: Fill
                    height: Fit
                    align: Align{x: 0.5}
                    wp_preview_card := glass.Card{
                        width: 210
                        height: 176
                        flow: Overlay
                        padding: 0
                        draw_bg +: {
                            corner_radius: 16.0
                            tint_color: #x6fa6ff
                            tint_alpha: 0.14
                        }
                        View{
                            width: Fill
                            height: Fill
                            flow: Down
                            align: Align{x: 0.5, y: 0.5}
                            padding: 8
                            wp_preview := Splash{
                                width: Fill
                                height: Fit
                            }
                        }
                    }
                }

                // Size preset chips.
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 6
                    align: Align{x: 0.5}
                    wp_size_0 := SizeChip{visible: false}
                    wp_size_1 := SizeChip{visible: false}
                    wp_size_2 := SizeChip{visible: false}
                    wp_size_3 := SizeChip{visible: false}
                }

                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 8
                    margin: Inset{top: 2}
                    wp_back := glass.GlassButton{
                        text: "‹ Back"
                        height: 40
                    }
                    wp_add := glass.GlassButtonProminent{
                        text: "Add Widget"
                        width: Fill
                        height: 40
                    }
                }
            }
        }
    }

    mod.widgets.LauncherBackgroundMenuBase = #(LauncherBackgroundMenu::register_widget(vm))

    // The menu shown on right-click of empty home-screen space (desktop): the
    // whole-home-screen equivalent of the per-icon menu above.
    mod.widgets.LauncherBackgroundMenu = set_type_default() do mod.widgets.LauncherBackgroundMenuBase{
        width: 232
        height: Fit
        flow: Down

        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 0
            padding: Inset{top: 8, bottom: 8, left: 0, right: 0}

            View{
                width: Fill
                height: Fit
                flow: Down
                spacing: 1
                padding: Inset{left: 16, right: 16, bottom: 6, top: 2}
                Label{
                    text: "Home Screen"
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 14}
                    }
                }
            }
            MenuDivider{}

            bg_edit_button := MenuButton{text: "Edit Home Screen"}
            bg_search_button := MenuButton{text: "Search"}
            bg_drawer_button := MenuButton{text: "All Apps"}
            bg_store_button := MenuButton{text: "Get More Apps…"}
            bg_import_button := MenuButton{text: "Import App…"}
            bg_wallpaper_button := MenuButton{text: "Change Wallpaper"}
            MenuDivider{}
            bg_delete_page_button := MenuButton{
                text: "Delete This Page"
                draw_text +: { color: #xff8888 }
            }
        }
    }
}

/// Where the long-press that opened the menu came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuSource {
    HomeIcon,
    HomeWidget,
    Drawer,
}

/// Everything the menu needs to know about its subject.
#[derive(Clone, Debug)]
pub struct MenuContext {
    pub app_id: MiniAppId,
    pub widget_instance: Option<WidgetInstanceId>,
    /// The placement instance of the specific home icon this menu is for (if opened
    /// from a home app icon), so "Remove from Home" removes just that icon.
    pub home_instance: Option<WidgetInstanceId>,
    pub source: MenuSource,
    /// Whether the app is currently running (shows Force Stop).
    pub running: bool,
    /// Whether the app's icon is already on the home screen.
    pub on_home: bool,
    /// Whether the app provides a home-screen widget.
    pub has_widget: bool,
    /// Built-in apps can't be uninstalled.
    pub builtin: bool,
    /// The app's quick-action shortcuts (top of the menu).
    pub shortcuts: Vec<String>,
    /// One-line detail shown when "App info" is picked.
    pub info: String,
}

/// Actions emitted when the user picks a menu entry.
#[derive(Clone, Debug, Default)]
pub enum ContextMenuAction {
    Open(MiniAppId),
    AddToHome(MiniAppId),
    AddWidget(MiniAppId),
    /// Remove the app from home. `instance` = the specific icon to remove (a home
    /// app-icon menu); `None` removes every copy (a drawer/dock menu with no
    /// specific home placement).
    RemoveFromHome { app_id: MiniAppId, instance: Option<WidgetInstanceId> },
    RemoveWidget(WidgetInstanceId),
    EnterEditMode,
    /// Open the App Info page: everything about the app, its version history,
    /// and the destructive actions.
    AppInfo(MiniAppId),
    /// Rewrite this app with AI: prefills + focuses the create bar so the user
    /// can type the change they want.
    Modify(MiniAppId),
    Uninstall(MiniAppId),
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherContextMenu {
    #[deref]
    view: View,
    #[rust]
    context: Option<MenuContext>,
}

/// The menu panel's fixed width, used to position it next to its anchor.
pub const MENU_WIDTH: f64 = 268.0;
/// Height of the callout triangle bridging the menu to its icon (see the DSL).
pub const MENU_CALLOUT_H: f64 = 9.0;
/// Width of the callout triangle.
const MENU_CALLOUT_W: f64 = 26.0;
/// Keep the callout triangle at least this far from the menu's side edges.
const MENU_CALLOUT_MARGIN: f64 = 20.0;

impl LauncherContextMenu {
    /// Points the callout triangle at the icon the menu belongs to: shows the
    /// up-pointing triangle (on top) when the menu sits below the icon, or the
    /// down-pointing one (on bottom) when it sits above. `center_x` is the icon's
    /// centre relative to the menu's left edge; the triangle is clamped to stay
    /// within the menu.
    pub fn set_callout(&mut self, cx: &mut Cx, points_up: bool, center_x: f64) {
        self.view.widget(cx, ids!(callout_up)).set_visible(cx, points_up);
        self.view.widget(cx, ids!(callout_down)).set_visible(cx, !points_up);
        let tx = center_x.clamp(MENU_CALLOUT_MARGIN, MENU_WIDTH - MENU_CALLOUT_MARGIN)
            - MENU_CALLOUT_W * 0.5;
        let tri = if points_up {
            self.view.view(cx, ids!(callout_up.callout_tri))
        } else {
            self.view.view(cx, ids!(callout_down.callout_tri))
        };
        if let Some(mut tri) = tri.borrow_mut() {
            tri.walk.margin.left = tx;
        }
        self.view.redraw(cx);
    }

    /// Configures the menu for the given subject and shows the relevant entries.
    /// Returns the estimated panel height, for anchoring the popup near its icon.
    pub fn show(&mut self, cx: &mut Cx, glyph: &str, name: &str, context: MenuContext) -> f64 {
        self.view.label(cx, ids!(glyph)).set_text(cx, glyph);
        self.view.label(cx, ids!(title)).set_text(cx, name);

        let is_widget = context.source == MenuSource::HomeWidget;
        let show = |v: &View, id: &[LiveId], visible: bool, cx: &mut Cx| {
            v.widget(cx, id).set_visible(cx, visible);
        };

        // App shortcuts (only for non-widget subjects).
        let shortcut_ids: [&[LiveId]; 4] = [
            ids!(shortcut_0),
            ids!(shortcut_1),
            ids!(shortcut_2),
            ids!(shortcut_3),
        ];
        let mut shown_shortcuts = 0;
        for (i, id) in shortcut_ids.iter().enumerate() {
            let visible = !is_widget && i < context.shortcuts.len();
            if visible {
                self.view.button(cx, id).set_text(cx, &context.shortcuts[i]);
                shown_shortcuts += 1;
            }
            show(&self.view, id, visible, cx);
        }
        show(&self.view, ids!(shortcut_divider), shown_shortcuts > 0, cx);

        let entries = [
            !is_widget,
            context.source == MenuSource::Drawer && !context.on_home,
            context.has_widget && !is_widget,
            context.source == MenuSource::HomeIcon,
            is_widget,
            // AI modify: ANY app's script can be rewritten, built-ins included
            // (a modified built-in becomes a user override you can revert).
            !is_widget,
            !context.builtin && !is_widget,
            // Built-ins simply omit the Uninstall row (no disabled placeholder).
            false,
        ];
        show(&self.view, ids!(info_button), entries[0], cx);
        show(&self.view, ids!(add_home_button), entries[1], cx);
        show(&self.view, ids!(add_widget_button), entries[2], cx);
        show(&self.view, ids!(remove_home_button), entries[3], cx);
        show(&self.view, ids!(remove_widget_button), entries[4], cx);
        show(&self.view, ids!(modify_button), entries[5], cx);
        show(&self.view, ids!(uninstall_button), entries[6], cx);
        show(&self.view, ids!(uninstall_disabled_label), entries[7], cx);

        self.context = Some(context);
        self.view.redraw(cx);

        // Estimated panel height, used to anchor the popup above/below the icon
        // without overlapping it. Must track the DSL: the title_row header (~92
        // incl. panel padding), 38pt rows on 6pt spacing (≈44 each), and ~7pt per
        // divider. Kept a touch generous so a menu placed above the icon always
        // clears it rather than overhanging.
        let rows = shown_shortcuts + entries.iter().filter(|v| **v).count();
        let dividers = if shown_shortcuts > 0 { 2.0 } else { 1.0 };
        92.0 + dividers * 7.0 + rows as f64 * 44.0
    }
}

impl Widget for LauncherContextMenu {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);

        let Some(context) = self.context.clone() else {
            return;
        };
        let Event::Actions(actions) = event else {
            return;
        };
        let uid = self.widget_uid();

        // "App Info & History": hand off to the full page (which owns the
        // storage/versions/destructive actions the menu used to carry).
        if self.view.button(cx, ids!(info_button)).clicked(actions) {
            cx.widget_action(uid, ContextMenuAction::AppInfo(context.app_id.clone()));
            return;
        }
        // Any shortcut just opens the app (display-only quick actions in this demo).
        let shortcut_ids: [&[LiveId]; 4] = [
            ids!(shortcut_0),
            ids!(shortcut_1),
            ids!(shortcut_2),
            ids!(shortcut_3),
        ];
        for id in shortcut_ids {
            if self.view.button(cx, id).clicked(actions) {
                cx.widget_action(uid, ContextMenuAction::Open(context.app_id.clone()));
                return;
            }
        }

        let v = &self.view;
        let action = if v.button(cx, ids!(add_home_button)).clicked(actions) {
            ContextMenuAction::AddToHome(context.app_id.clone())
        } else if v.button(cx, ids!(add_widget_button)).clicked(actions) {
            ContextMenuAction::AddWidget(context.app_id.clone())
        } else if v.button(cx, ids!(remove_home_button)).clicked(actions) {
            ContextMenuAction::RemoveFromHome {
                app_id: context.app_id.clone(),
                instance: context.home_instance,
            }
        } else if v.button(cx, ids!(remove_widget_button)).clicked(actions) {
            match context.widget_instance {
                Some(instance) => ContextMenuAction::RemoveWidget(instance),
                None => ContextMenuAction::None,
            }
        } else if v.button(cx, ids!(modify_button)).clicked(actions) {
            ContextMenuAction::Modify(context.app_id.clone())
        } else if v.button(cx, ids!(uninstall_button)).clicked(actions) {
            ContextMenuAction::Uninstall(context.app_id.clone())
        } else {
            ContextMenuAction::None
        };
        if !matches!(action, ContextMenuAction::None) {
            cx.widget_action(uid, action);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherContextMenuRef {
    pub fn show(&self, cx: &mut Cx, glyph: &str, name: &str, context: MenuContext) -> f64 {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, glyph, name, context)
        } else {
            0.0
        }
    }
}

/// One selectable widget in the gallery.
#[derive(Clone)]
pub struct WidgetPickerEntry {
    pub app_id: MiniAppId,
    /// Pre-formatted "glyph  Name" row label.
    pub label: String,
    /// The widget's Splash source, for the live preview.
    pub source: String,
    pub min_span: (u8, u8),
    pub default_span: (u8, u8),
}

/// Emitted when the user confirms adding a widget at a chosen size.
#[derive(Clone, Debug, Default)]
pub enum WidgetPickerAction {
    Add { app_id: MiniAppId, span: (u8, u8) },
    #[default]
    None,
}

const WP_ROW_IDS: [&[LiveId]; 6] =
    [ids!(wp_0), ids!(wp_1), ids!(wp_2), ids!(wp_3), ids!(wp_4), ids!(wp_5)];
const WP_SIZE_IDS: [&[LiveId]; 4] =
    [ids!(wp_size_0), ids!(wp_size_1), ids!(wp_size_2), ids!(wp_size_3)];
/// Size presets offered in the gallery, filtered per widget to those that are
/// at least its `min_span` and fit the grid (plus the widget's own default).
const SIZE_PRESETS: [(u8, u8); 4] = [(2, 1), (2, 2), (4, 2), (4, 4)];

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherWidgetPicker {
    #[deref]
    view: View,
    #[rust]
    entries: Vec<WidgetPickerEntry>,
    /// The current grid (cols, rows), which caps the offered sizes.
    #[rust]
    grid: (u8, u8),
    /// Index of the entry whose preview/sizes are shown, if any.
    #[rust]
    selected: Option<usize>,
    /// Valid size presets for the selected widget.
    #[rust]
    sizes: Vec<(u8, u8)>,
    /// Chosen span (always one of `sizes`).
    #[rust]
    span: (u8, u8),
}

impl LauncherWidgetPicker {
    /// (Re)opens the gallery on the list stage with one row per widget entry.
    pub fn show(&mut self, cx: &mut Cx, entries: &[WidgetPickerEntry], grid: (u8, u8)) {
        self.entries = entries.to_vec();
        self.grid = (grid.0.max(1), grid.1.max(1));
        for (i, id) in WP_ROW_IDS.iter().enumerate() {
            let visible = i < entries.len();
            if visible {
                self.view.button(cx, id).set_text(cx, &entries[i].label);
            }
            self.view.widget(cx, id).set_visible(cx, visible);
        }
        self.back(cx);
    }

    /// The size presets valid for a widget with the given minimum span, always
    /// including its default (clamped into the grid), capped to the slot count.
    fn sizes_for(&self, min: (u8, u8), default: (u8, u8)) -> Vec<(u8, u8)> {
        let grid = self.grid;
        let fits = |(c, r): (u8, u8)| c >= min.0 && r >= min.1 && c <= grid.0 && r <= grid.1;
        let mut v: Vec<(u8, u8)> = SIZE_PRESETS.iter().copied().filter(|&s| fits(s)).collect();
        let def = (default.0.clamp(min.0, grid.0), default.1.clamp(min.1, grid.1));
        if !v.contains(&def) {
            v.insert(0, def);
        }
        v.truncate(WP_SIZE_IDS.len());
        if v.is_empty() {
            v.push((min.0.min(grid.0).max(1), min.1.min(grid.1).max(1)));
        }
        v
    }

    /// Reveals the preview + size stage for entry `i`.
    fn open_detail(&mut self, cx: &mut Cx, i: usize) {
        let Some(entry) = self.entries.get(i).cloned() else {
            return;
        };
        self.selected = Some(i);
        self.sizes = self.sizes_for(entry.min_span, entry.default_span);
        self.span = self
            .sizes
            .iter()
            .copied()
            .find(|&s| s == entry.default_span)
            .unwrap_or(self.sizes[0]);
        self.view.label(cx, ids!(wp_title)).set_text(cx, &entry.label);
        self.view.widget(cx, ids!(wp_list)).set_visible(cx, false);
        self.view.widget(cx, ids!(wp_detail)).set_visible(cx, true);
        // Load the live preview and reflect the chosen size.
        if let Some(mut sp) = self.view.widget(cx, ids!(wp_preview)).borrow_mut::<Splash>() {
            sp.set_debug_name(&format!("{} widget preview", entry.app_id));
        }
        self.view.widget(cx, ids!(wp_preview)).set_text(cx, &entry.source);
        self.refresh_sizes(cx);
        self.resize_preview(cx);
        self.view.redraw(cx);
    }

    /// Returns to the widget list.
    fn back(&mut self, cx: &mut Cx) {
        self.selected = None;
        self.view.label(cx, ids!(wp_title)).set_text(cx, "Add Widget");
        self.view.widget(cx, ids!(wp_detail)).set_visible(cx, false);
        self.view.widget(cx, ids!(wp_list)).set_visible(cx, true);
        // Stop the preview isolate so it isn't left ticking behind the list.
        self.view.widget(cx, ids!(wp_preview)).set_text(cx, "");
        self.view.redraw(cx);
    }

    fn set_span(&mut self, cx: &mut Cx, span: (u8, u8)) {
        if self.span == span {
            return;
        }
        self.span = span;
        self.refresh_sizes(cx);
        self.resize_preview(cx);
        self.view.redraw(cx);
    }

    /// Re-labels the size chips (the selected one gets a leading dot) and the Add
    /// button (which names the chosen size).
    fn refresh_sizes(&mut self, cx: &mut Cx) {
        for (i, id) in WP_SIZE_IDS.iter().enumerate() {
            match self.sizes.get(i) {
                Some(&(c, r)) => {
                    let sel = (c, r) == self.span;
                    let label = if sel {
                        format!("● {c}×{r}")
                    } else {
                        format!("{c}×{r}")
                    };
                    self.view.button(cx, id).set_text(cx, &label);
                    self.view.widget(cx, id).set_visible(cx, true);
                }
                None => self.view.widget(cx, id).set_visible(cx, false),
            }
        }
        let (c, r) = self.span;
        self.view
            .glass_button(cx, ids!(wp_add))
            .set_text(cx, &format!("Add {c}×{r} Widget"));
    }

    /// Tells the preview widget its chosen span so its Splash reflows exactly as
    /// it would on the home screen (rows reveal extra content, etc.). The card is
    /// fixed-size; the pixel size passed approximates the on-screen content box.
    fn resize_preview(&mut self, cx: &mut Cx) {
        let (cols, rows) = self.span;
        // Scale the reported content size with the chosen span (not the fixed
        // card), so the widget picks the same width/row tier it will on the real
        // home grid — a wider span reports a wider box, a taller span a taller one.
        let w = cols as f64 * 97.0;
        let h = rows as f64 * 80.0;
        if let Some(mut sp) = self.view.widget(cx, ids!(wp_preview)).borrow_mut::<Splash>() {
            sp.call_script_fn(
                cx,
                live_id!(on_widget_resize),
                &[
                    (cols as f64).into(),
                    (rows as f64).into(),
                    w.into(),
                    h.into(),
                ],
            );
        }
    }
}

impl Widget for LauncherWidgetPicker {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        if self.selected.is_none() {
            // List stage: tapping a row opens its preview/size detail.
            for (i, id) in WP_ROW_IDS.iter().enumerate() {
                if i < self.entries.len() && self.view.button(cx, id).clicked(actions) {
                    self.open_detail(cx, i);
                    return;
                }
            }
            return;
        }
        // Detail stage.
        for (i, id) in WP_SIZE_IDS.iter().enumerate() {
            if self.view.button(cx, id).clicked(actions) {
                if let Some(&span) = self.sizes.get(i) {
                    self.set_span(cx, span);
                }
                return;
            }
        }
        if self.view.glass_button(cx, ids!(wp_back)).clicked(actions) {
            self.back(cx);
            return;
        }
        if self.view.glass_button(cx, ids!(wp_add)).clicked(actions) {
            if let Some(entry) = self.selected.and_then(|i| self.entries.get(i)) {
                cx.widget_action(
                    self.widget_uid(),
                    WidgetPickerAction::Add { app_id: entry.app_id.clone(), span: self.span },
                );
            }
            // Reset to the list and tear down the preview isolate so its widgets
            // don't linger in the (closing) modal.
            self.back(cx);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherWidgetPickerRef {
    pub fn show(&self, cx: &mut Cx, entries: &[WidgetPickerEntry], grid: (u8, u8)) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, entries, grid);
        }
    }

    /// Returns the gallery to its list stage and tears down the live-preview
    /// Splash isolate. Call on every path that closes the picker modal so the
    /// preview VM doesn't keep ticking behind a closed modal.
    pub fn reset(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.back(cx);
        }
    }

    /// Whether the gallery is on its preview/size detail stage (a widget selected,
    /// so the live-preview isolate is running). Lets the app tear it down when the
    /// modal closes without an in-panel Back/Add.
    pub fn is_showing_detail(&self) -> bool {
        self.borrow().is_some_and(|inner| inner.selected.is_some())
    }

    /// Debug/screenshot helper: jump straight to the preview + size detail for
    /// `app_id` (as if its list row had been tapped).
    pub fn debug_open(&self, cx: &mut Cx, app_id: &MiniAppId) {
        if let Some(mut inner) = self.borrow_mut() {
            if let Some(i) = inner.entries.iter().position(|e| &e.app_id == app_id) {
                inner.open_detail(cx, i);
            }
        }
    }
}

/// Actions emitted by the empty-space (right-click) home-screen menu.
#[derive(Clone, Debug, Default)]
pub enum BackgroundMenuAction {
    EnterEditMode,
    OpenSearch,
    OpenDrawer,
    OpenAppStore,
    ImportApp,
    CycleWallpaper,
    DeletePage,
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherBackgroundMenu {
    #[deref]
    view: View,
}

impl Widget for LauncherBackgroundMenu {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        let v = &self.view;
        let action = if v.button(cx, ids!(bg_edit_button)).clicked(actions) {
            BackgroundMenuAction::EnterEditMode
        } else if v.button(cx, ids!(bg_search_button)).clicked(actions) {
            BackgroundMenuAction::OpenSearch
        } else if v.button(cx, ids!(bg_drawer_button)).clicked(actions) {
            BackgroundMenuAction::OpenDrawer
        } else if v.button(cx, ids!(bg_store_button)).clicked(actions) {
            BackgroundMenuAction::OpenAppStore
        } else if v.button(cx, ids!(bg_import_button)).clicked(actions) {
            BackgroundMenuAction::ImportApp
        } else if v.button(cx, ids!(bg_wallpaper_button)).clicked(actions) {
            BackgroundMenuAction::CycleWallpaper
        } else if v.button(cx, ids!(bg_delete_page_button)).clicked(actions) {
            BackgroundMenuAction::DeletePage
        } else {
            BackgroundMenuAction::None
        };
        if !matches!(action, BackgroundMenuAction::None) {
            cx.widget_action(self.widget_uid(), action);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}
