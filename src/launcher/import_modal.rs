//! "Import App": install a mini-app someone else exported.
//!
//! Two routes, because sharing an app happens two ways. A file dropped in the
//! exchange folder shows up as a row here; a bundle pasted from a chat goes in
//! the paste box. (Makepad has no native file picker yet, so the folder *is*
//! the picker — and it's the same folder Export writes to.)

use std::path::PathBuf;

use makepad_widgets::*;

use crate::mini_apps::bundle::ImportEntry;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherImportModalBase = #(LauncherImportModal::register_widget(vm))

    // One file found in the exchange folder.
    let ImportRow = View{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 12
        padding: Inset{left: 16, right: 12, top: 6, bottom: 6}
        imp_icon := Label{
            text: ""
            draw_text +: { text_style: theme.font_regular{font_size: 22} }
        }
        View{
            width: Fill
            height: Fit
            flow: Down
            spacing: 1
            imp_name := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    text_style: theme.font_bold{font_size: 14}
                }
            }
            imp_sub := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 10.5}
                }
            }
        }
        imp_action := glass.GlassButton{
            text: "Install"
            width: 76
            height: 32
            draw_text +: { text_style: theme.font_bold{font_size: 12} }
        }
    }

    mod.widgets.LauncherImportModal = set_type_default() do mod.widgets.LauncherImportModalBase{
        width: 356
        height: Fit
        flow: Down

        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 0
            padding: Inset{top: 10, bottom: 12, left: 0, right: 0}

            View{
                width: Fill
                height: Fit
                flow: Down
                spacing: 1
                padding: Inset{left: 16, right: 16, bottom: 6, top: 2}
                Label{
                    text: "Import App"
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 16}
                    }
                }
                Label{
                    width: Fill
                    text: "Install an app someone exported — from the exchange folder, or pasted."
                    draw_text +: {
                        color: #x9dccffcc
                        text_style: theme.font_regular{font_size: 10.5}
                    }
                }
            }

            imp_0 := ImportRow{visible: false}
            imp_1 := ImportRow{visible: false}
            imp_2 := ImportRow{visible: false}
            imp_3 := ImportRow{visible: false}
            imp_4 := ImportRow{visible: false}
            imp_5 := ImportRow{visible: false}

            // Shown instead of the rows when the folder is empty: without it
            // the modal looks broken rather than merely empty.
            imp_empty := View{
                width: Fill
                height: Fit
                flow: Down
                spacing: 2
                padding: Inset{left: 16, right: 16, top: 6, bottom: 6}
                Label{
                    width: Fill
                    text: "No app files found."
                    draw_text +: {
                        color: #xd9e6ff
                        text_style: theme.font_bold{font_size: 12}
                    }
                }
                imp_folder_path := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #x9fb0cc
                        text_style: theme.font_regular{font_size: 10}
                    }
                }
            }

            View{
                width: Fill
                height: Fit
                flow: Down
                spacing: 6
                padding: Inset{left: 16, right: 16, top: 14}
                glass.Caption{text: "OR PASTE A BUNDLE"}
                imp_paste := LauncherTextInput{
                    is_multiline: true
                    height: Fit{min: FitBound.Abs(52), max: FitBound.Abs(110)}
                    empty_text: "Paste an exported app here"
                }
                imp_status := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #xff9d9d
                        text_style: theme.font_regular{font_size: 11}
                    }
                }
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 8
                    imp_reveal := glass.GlassButton{
                        text: "Open Folder"
                        width: Fill
                        height: 36
                        draw_text +: { text_style: theme.font_bold{font_size: 12} }
                    }
                    imp_paste_install := glass.GlassButtonProminent{
                        text: "Install Pasted"
                        width: Fill
                        height: 36
                        draw_text +: { text_style: theme.font_bold{font_size: 12} }
                    }
                }
            }
        }
    }
}

/// Emitted when the user picks something to install.
#[derive(Clone, Debug, Default)]
pub enum ImportModalAction {
    /// Install the bundle at this path (a row's Install button).
    InstallFile(PathBuf),
    /// Install this pasted text.
    InstallText(String),
    /// Reveal the exchange folder in the system file manager.
    OpenFolder,
    #[default]
    None,
}

const IMPORT_ROW_IDS: [&[LiveId]; 6] = [
    ids!(imp_0),
    ids!(imp_1),
    ids!(imp_2),
    ids!(imp_3),
    ids!(imp_4),
    ids!(imp_5),
];

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherImportModal {
    #[deref]
    view: View,
    #[rust]
    entries: Vec<ImportEntry>,
}

impl LauncherImportModal {
    fn show(&mut self, cx: &mut Cx, entries: &[ImportEntry], folder: &str) {
        self.entries = entries.to_vec();
        for (i, &row) in IMPORT_ROW_IDS.iter().enumerate() {
            match entries.get(i) {
                Some(e) => {
                    self.view
                        .label(cx, &[row, ids!(imp_icon)].concat())
                        .set_text(cx, &e.icon);
                    self.view
                        .label(cx, &[row, ids!(imp_name)].concat())
                        .set_text(cx, &e.name);
                    self.view
                        .label(cx, &[row, ids!(imp_sub)].concat())
                        .set_text(cx, &e.detail);
                    self.view.widget(cx, row).set_visible(cx, true);
                }
                None => self.view.widget(cx, row).set_visible(cx, false),
            }
        }
        // The path is only worth screen space when there's nothing to show:
        // it's the instruction for what to do about that.
        self.view
            .label(cx, ids!(imp_folder_path))
            .set_text(cx, folder);
        self.view
            .widget(cx, ids!(imp_empty))
            .set_visible(cx, entries.is_empty());
        self.view.text_input(cx, ids!(imp_paste)).set_text(cx, "");
        self.view.label(cx, ids!(imp_status)).set_text(cx, "");
        self.view.redraw(cx);
    }

    /// Why the last attempt failed, in the modal (which stays open so the
    /// paste can be fixed rather than re-pasted).
    fn set_status(&mut self, cx: &mut Cx, message: &str) {
        self.view.label(cx, ids!(imp_status)).set_text(cx, message);
        self.view.redraw(cx);
    }
}

impl Widget for LauncherImportModal {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        for (i, &row) in IMPORT_ROW_IDS.iter().enumerate() {
            if self
                .view
                .glass_button(cx, &[row, ids!(imp_action)].concat())
                .clicked(actions)
            {
                if let Some(e) = self.entries.get(i) {
                    cx.widget_action(
                        self.widget_uid(),
                        ImportModalAction::InstallFile(e.path.clone()),
                    );
                }
                return;
            }
        }
        if self
            .view
            .glass_button(cx, ids!(imp_paste_install))
            .clicked(actions)
        {
            let text = self.view.text_input(cx, ids!(imp_paste)).text();
            cx.widget_action(self.widget_uid(), ImportModalAction::InstallText(text));
        }
        if self.view.glass_button(cx, ids!(imp_reveal)).clicked(actions) {
            cx.widget_action(self.widget_uid(), ImportModalAction::OpenFolder);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherImportModalRef {
    pub fn show(&self, cx: &mut Cx, entries: &[ImportEntry], folder: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, entries, folder);
        }
    }

    pub fn set_status(&self, cx: &mut Cx, message: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_status(cx, message);
        }
    }
}
