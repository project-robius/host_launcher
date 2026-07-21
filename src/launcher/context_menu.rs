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
    let MenuButton = ButtonFlatter{
        width: Fill
        height: 38
        align: Align{x: 0.0, y: 0.5}
        padding: Inset{left: 16, right: 16}
        draw_text +: {
            color: #xf2f6ff
            text_style: theme.font_regular{font_size: 13}
        }
    }

    let MenuDivider = View{
        width: Fill
        height: 1
        margin: Inset{top: 3, bottom: 3, left: 10, right: 10}
        show_bg: true
        draw_bg +: { color: #xffffff1a }
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
                View{
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 1
                    title := Label{
                        text: ""
                        draw_text +: {
                            color: #ffffff
                            text_style: theme.font_bold{font_size: 15}
                        }
                    }
                    // Empty by default (no dev-jargon subtitle); only populated on
                    // demand when "App info" is picked.
                    subtitle := Label{
                        text: ""
                        draw_text +: {
                            color: #x9dccffcc
                            text_style: theme.font_regular{font_size: 10.5}
                        }
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

            open_button := MenuButton{text: "Open"}
            info_button := MenuButton{text: "App info"}
            add_home_button := MenuButton{text: "Add to Home Screen"}
            add_widget_button := MenuButton{text: "Add Widget to Home"}
            remove_home_button := MenuButton{text: "Remove from Home"}
            remove_widget_button := MenuButton{text: "Remove Widget"}
            edit_button := MenuButton{text: "Edit Home Screen"}
            force_stop_button := MenuButton{text: "Force Stop"}
            uninstall_button := MenuButton{
                text: "Uninstall"
                draw_text +: { color: #xff8888 }
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

    // The "add a widget" chooser shown from edit mode: one row per app that
    // provides a home-screen widget.
    mod.widgets.LauncherWidgetPicker = set_type_default() do mod.widgets.LauncherWidgetPickerBase{
        width: 268
        height: Fit
        flow: Down

        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 0
            padding: Inset{top: 10, bottom: 10, left: 0, right: 0}

            Label{
                margin: Inset{left: 16, bottom: 6}
                text: "Add Widget"
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_bold{font_size: 15}
                }
            }
            MenuDivider{}
            wp_0 := MenuButton{visible: false}
            wp_1 := MenuButton{visible: false}
            wp_2 := MenuButton{visible: false}
            wp_3 := MenuButton{visible: false}
            wp_4 := MenuButton{visible: false}
            wp_5 := MenuButton{visible: false}
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
    ForceStop(MiniAppId),
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
        self.view.label(cx, ids!(subtitle)).set_text(cx, "");

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
            !is_widget,
            context.source == MenuSource::Drawer && !context.on_home,
            context.has_widget && !is_widget,
            context.source == MenuSource::HomeIcon,
            is_widget,
            context.source != MenuSource::Drawer,
            context.running,
            !context.builtin && !is_widget,
            // Built-ins simply omit the Uninstall row (no disabled placeholder).
            false,
        ];
        show(&self.view, ids!(open_button), entries[0], cx);
        show(&self.view, ids!(info_button), entries[1], cx);
        show(&self.view, ids!(add_home_button), entries[2], cx);
        show(&self.view, ids!(add_widget_button), entries[3], cx);
        show(&self.view, ids!(remove_home_button), entries[4], cx);
        show(&self.view, ids!(remove_widget_button), entries[5], cx);
        show(&self.view, ids!(edit_button), entries[6], cx);
        show(&self.view, ids!(force_stop_button), entries[7], cx);
        show(&self.view, ids!(uninstall_button), entries[8], cx);
        show(&self.view, ids!(uninstall_disabled_label), entries[9], cx);

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

        // "App info": reveal the isolation details inline without closing the menu.
        if self.view.button(cx, ids!(info_button)).clicked(actions) {
            self.view.label(cx, ids!(subtitle)).set_text(cx, &context.info);
            self.view.redraw(cx);
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
        let action = if v.button(cx, ids!(open_button)).clicked(actions) {
            ContextMenuAction::Open(context.app_id.clone())
        } else if v.button(cx, ids!(add_home_button)).clicked(actions) {
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
        } else if v.button(cx, ids!(edit_button)).clicked(actions) {
            ContextMenuAction::EnterEditMode
        } else if v.button(cx, ids!(force_stop_button)).clicked(actions) {
            ContextMenuAction::ForceStop(context.app_id.clone())
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

/// Emitted when the user picks an app in the widget chooser.
#[derive(Clone, Debug, Default)]
pub enum WidgetPickerAction {
    Add(MiniAppId),
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherWidgetPicker {
    #[deref]
    view: View,
    /// The app id behind each visible row.
    #[rust]
    entries: Vec<MiniAppId>,
}

impl LauncherWidgetPicker {
    /// Populates one row per (app id, "glyph  name") entry, up to six.
    pub fn show(&mut self, cx: &mut Cx, entries: &[(MiniAppId, String)]) {
        let row_ids: [&[LiveId]; 6] = [
            ids!(wp_0),
            ids!(wp_1),
            ids!(wp_2),
            ids!(wp_3),
            ids!(wp_4),
            ids!(wp_5),
        ];
        self.entries = entries.iter().map(|(id, _)| id.clone()).collect();
        for (i, id) in row_ids.iter().enumerate() {
            let visible = i < entries.len();
            if visible {
                self.view.button(cx, id).set_text(cx, &entries[i].1);
            }
            self.view.widget(cx, id).set_visible(cx, visible);
        }
        self.view.redraw(cx);
    }
}

impl Widget for LauncherWidgetPicker {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        let row_ids: [&[LiveId]; 6] = [
            ids!(wp_0),
            ids!(wp_1),
            ids!(wp_2),
            ids!(wp_3),
            ids!(wp_4),
            ids!(wp_5),
        ];
        for (i, id) in row_ids.iter().enumerate() {
            if self.view.button(cx, id).clicked(actions) {
                if let Some(app_id) = self.entries.get(i) {
                    cx.widget_action(self.widget_uid(), WidgetPickerAction::Add(app_id.clone()));
                }
                return;
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherWidgetPickerRef {
    pub fn show(&self, cx: &mut Cx, entries: &[(MiniAppId, String)]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, entries);
        }
    }
}

/// Actions emitted by the empty-space (right-click) home-screen menu.
#[derive(Clone, Debug, Default)]
pub enum BackgroundMenuAction {
    EnterEditMode,
    OpenSearch,
    OpenDrawer,
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
