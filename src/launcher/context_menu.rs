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

    let MenuButton = ButtonFlat{
        width: Fill
        height: 40
        align: Align{x: 0.0, y: 0.5}
        padding: Inset{left: 14, right: 14}
        draw_text +: {
            color: #xf2f6ff
            text_style: theme.font_regular{font_size: 13}
        }
    }

    mod.widgets.LauncherContextMenuBase = #(LauncherContextMenu::register_widget(vm))

    mod.widgets.LauncherContextMenu = set_type_default() do mod.widgets.LauncherContextMenuBase{
        width: 260
        height: Fit
        flow: Down
        show_bg: true
        draw_bg +: {
            color: #x101828f2
            border_color: #xffffff20
            border_size: 1.0
            border_radius: 16.0
        }
        padding: Inset{top: 10, bottom: 10}

        title_row := View{
            width: Fill
            height: Fit
            flow: Right
            spacing: 10
            align: Align{y: 0.5}
            padding: Inset{left: 14, right: 14, bottom: 8}
            glyph := Label{
                text: ""
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_regular{font_size: 20}
                }
            }
            title := Label{
                text: ""
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_bold{font_size: 14}
                }
            }
        }
        Hr{height: 1, margin: Inset{left: 10, right: 10, bottom: 4}}

        open_button := MenuButton{text: "Open"}
        add_home_button := MenuButton{text: "Add to Home Screen"}
        add_widget_button := MenuButton{text: "Add Widget to Home"}
        remove_home_button := MenuButton{text: "Remove from Home"}
        remove_widget_button := MenuButton{text: "Remove Widget"}
        edit_button := MenuButton{text: "Edit Home Screen"}
        force_stop_button := MenuButton{text: "Force Stop"}
        uninstall_button := MenuButton{
            text: "Uninstall"
            draw_text +: {
                color: #xff8888
            }
        }
        uninstall_disabled_label := Label{
            visible: false
            width: Fill
            height: 40
            align: Align{x: 0.0, y: 0.5}
            padding: Inset{left: 14, right: 14}
            text: "Uninstall (pre-installed)"
            draw_text +: {
                color: #x8fa6c8aa
                text_style: theme.font_regular{font_size: 13}
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
    pub source: MenuSource,
    /// Whether the app is currently running (shows Force Stop).
    pub running: bool,
    /// Whether the app's icon is already on the home screen.
    pub on_home: bool,
    /// Whether the app provides a home-screen widget.
    pub has_widget: bool,
    /// Built-in apps can't be uninstalled.
    pub builtin: bool,
}

/// Actions emitted when the user picks a menu entry.
#[derive(Clone, Debug, Default)]
pub enum ContextMenuAction {
    Open(MiniAppId),
    AddToHome(MiniAppId),
    AddWidget(MiniAppId),
    RemoveFromHome(MiniAppId),
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

impl LauncherContextMenu {
    /// Configures the menu for the given subject and shows the relevant entries.
    pub fn show(&mut self, cx: &mut Cx, glyph: &str, name: &str, context: MenuContext) {
        self.view.label(cx, ids!(glyph)).set_text(cx, glyph);
        self.view.label(cx, ids!(title)).set_text(cx, name);

        let is_widget = context.source == MenuSource::HomeWidget;
        let show = |v: &View, id: &[LiveId], visible: bool, cx: &mut Cx| {
            v.widget(cx, id).set_visible(cx, visible);
        };
        show(&self.view, ids!(open_button), !is_widget, cx);
        show(
            &self.view,
            ids!(add_home_button),
            context.source == MenuSource::Drawer && !context.on_home,
            cx,
        );
        show(
            &self.view,
            ids!(add_widget_button),
            context.has_widget && !is_widget,
            cx,
        );
        show(
            &self.view,
            ids!(remove_home_button),
            context.source == MenuSource::HomeIcon,
            cx,
        );
        show(&self.view, ids!(remove_widget_button), is_widget, cx);
        show(
            &self.view,
            ids!(edit_button),
            context.source != MenuSource::Drawer,
            cx,
        );
        show(&self.view, ids!(force_stop_button), context.running, cx);
        show(
            &self.view,
            ids!(uninstall_button),
            !context.builtin && !is_widget,
            cx,
        );
        show(
            &self.view,
            ids!(uninstall_disabled_label),
            context.builtin && !is_widget,
            cx,
        );

        self.context = Some(context);
        self.view.redraw(cx);
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
        let v = &self.view;
        let action = if v.button(cx, ids!(open_button)).clicked(actions) {
            ContextMenuAction::Open(context.app_id.clone())
        } else if v.button(cx, ids!(add_home_button)).clicked(actions) {
            ContextMenuAction::AddToHome(context.app_id.clone())
        } else if v.button(cx, ids!(add_widget_button)).clicked(actions) {
            ContextMenuAction::AddWidget(context.app_id.clone())
        } else if v.button(cx, ids!(remove_home_button)).clicked(actions) {
            ContextMenuAction::RemoveFromHome(context.app_id.clone())
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
    pub fn show(&self, cx: &mut Cx, glyph: &str, name: &str, context: MenuContext) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, glyph, name, context);
        }
    }
}
