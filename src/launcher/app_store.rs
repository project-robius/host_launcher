//! The in-launcher "App Store": a modal listing installable mini-apps (the
//! `installable_catalog`) plus any the user has already installed, each with a
//! Get / Remove action. Installing copies the manifest into the persisted user
//! apps; removing routes through the same uninstall path as the context menu.

use makepad_widgets::*;

use crate::mini_apps::registry::MiniAppId;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherAppStoreBase = #(LauncherAppStore::register_widget(vm))

    // One catalog row: emoji icon, name + subtitle, and a Get/Remove button.
    let StoreRow = View{
        width: Fill
        // Fit with a 56 floor: the permission line is a third line of text on
        // the rows that have one, and a fixed 56 would have it spill into the
        // row below.
        height: Fit{min: FitBound.Abs(56)}
        flow: Right
        align: Align{y: 0.5}
        spacing: 12
        padding: Inset{left: 16, right: 12, top: 6, bottom: 6}
        row_icon := Label{
            text: ""
            draw_text +: { text_style: theme.font_regular{font_size: 24} }
        }
        View{
            width: Fill
            height: Fit
            flow: Down
            spacing: 1
            row_name := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    // 1.0, not the theme's 1.5: the extra leading is what put
                    // a visible gap between an entry's name and its own
                    // subtitle, so the two read as unrelated lines rather than
                    // one item. Same fix as the Providers rows.
                    text_style: theme.font_bold{font_size: 14, line_spacing: 1.0}
                }
            }
            row_sub := Label{
                width: Fill
                // Pulled up against the name for the same reason: the two line
                // boxes still leave more air than the pair wants.
                margin: Inset{top: -2}
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 10.5}
                }
            }
            // What the app DECLARES, before anyone taps Get. Amber rather than
            // the subtitle's blue so it reads as a caution, not another label.
            row_perms := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf0c674cc
                    text_style: theme.font_regular{font_size: 9.5}
                }
            }
        }
        row_action := glass.GlassButton{
            text: "Get"
            width: 86
            height: 32
            draw_text +: { text_style: theme.font_bold{font_size: 12} }
        }
    }

    mod.widgets.LauncherAppStore = set_type_default() do mod.widgets.LauncherAppStoreBase{
        width: 320
        height: Fit
        flow: Down

        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 0
            padding: Inset{top: 10, bottom: 10, left: 0, right: 0}

            View{
                width: Fill
                height: Fit
                flow: Down
                spacing: 1
                padding: Inset{left: 16, right: 16, bottom: 6, top: 2}
                Label{
                    text: "App Store"
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 16}
                    }
                }
                Label{
                    width: Fill
                    text: "Each mini-app is its own isolated Splash VM."
                    draw_text +: {
                        color: #x9dccffcc
                        text_style: theme.font_regular{font_size: 10.5}
                    }
                }
                // "Wants" on a row is a declaration, and a declaration is not
                // a grant — the rows have no space to say that themselves.
                Label{
                    width: Fill
                    text: "“Wants” is what an app may ask for, not what it has."
                    draw_text +: {
                        color: #x9fb0cc
                        text_style: theme.font_regular{font_size: 9.5}
                    }
                }
            }
            store_0 := StoreRow{visible: false}
            store_1 := StoreRow{visible: false}
            store_2 := StoreRow{visible: false}
            store_3 := StoreRow{visible: false}
            store_4 := StoreRow{visible: false}
            store_5 := StoreRow{visible: false}
        }
    }
}

/// One row in the App Store.
#[derive(Clone)]
pub struct StoreEntry {
    pub app_id: MiniAppId,
    pub icon: String,
    pub name: String,
    /// Short line under the name (e.g. "Includes a widget").
    pub subtitle: String,
    /// One line naming what the app declares ("Wants: Network · Location"),
    /// pre-rendered by the app layer. Declarations, not grants: installing
    /// doesn't hand any of them over.
    pub perms: String,
    pub installed: bool,
}

/// Emitted when the user taps a row's Get / Remove button.
#[derive(Clone, Debug, Default)]
pub enum AppStoreAction {
    Install(MiniAppId),
    Uninstall(MiniAppId),
    #[default]
    None,
}

const STORE_ROW_IDS: [&[LiveId]; 6] = [
    ids!(store_0),
    ids!(store_1),
    ids!(store_2),
    ids!(store_3),
    ids!(store_4),
    ids!(store_5),
];

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherAppStore {
    #[deref]
    view: View,
    #[rust]
    entries: Vec<StoreEntry>,
}

impl LauncherAppStore {
    pub fn show(&mut self, cx: &mut Cx, entries: &[StoreEntry]) {
        if entries.len() > STORE_ROW_IDS.len() {
            // The store has a fixed row count; surface (rather than silently drop)
            // an overflow so a grown catalog is noticed and paginated/scrolled.
            error!(
                "App Store has {} entries but only {} row slots; the extras are hidden",
                entries.len(),
                STORE_ROW_IDS.len()
            );
        }
        self.entries = entries.to_vec();
        for (i, &row) in STORE_ROW_IDS.iter().enumerate() {
            match entries.get(i) {
                Some(e) => {
                    // Address nested children by [row, child] path.
                    self.view
                        .label(cx, &[row, ids!(row_icon)].concat())
                        .set_text(cx, &e.icon);
                    self.view
                        .label(cx, &[row, ids!(row_name)].concat())
                        .set_text(cx, &e.name);
                    self.view
                        .label(cx, &[row, ids!(row_sub)].concat())
                        .set_text(cx, &e.subtitle);
                    self.view
                        .label(cx, &[row, ids!(row_perms)].concat())
                        .set_text(cx, &e.perms);
                    self.view
                        .glass_button(cx, &[row, ids!(row_action)].concat())
                        .set_text(cx, if e.installed { "Remove" } else { "Get" });
                    self.view.widget(cx, row).set_visible(cx, true);
                }
                None => self.view.widget(cx, row).set_visible(cx, false),
            }
        }
        self.view.redraw(cx);
    }
}

impl Widget for LauncherAppStore {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        for (i, &row) in STORE_ROW_IDS.iter().enumerate() {
            if self
                .view
                .glass_button(cx, &[row, ids!(row_action)].concat())
                .clicked(actions)
            {
                if let Some(e) = self.entries.get(i) {
                    let action = if e.installed {
                        AppStoreAction::Uninstall(e.app_id.clone())
                    } else {
                        AppStoreAction::Install(e.app_id.clone())
                    };
                    cx.widget_action(self.widget_uid(), action);
                }
                return;
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherAppStoreRef {
    pub fn show(&self, cx: &mut Cx, entries: &[StoreEntry]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, entries);
        }
    }
}
