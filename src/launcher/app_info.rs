//! The App Info screen: everything about one mini-app in one place, the way a
//! phone's per-app settings page works.
//!
//! It exists because the long-press menu was turning into a junk drawer. That
//! menu now keeps only the things you do *to the home screen* (open, place,
//! remove, modify); everything that is really *about the app* — what it is,
//! what it may do, how much it has stored, its version history, and the
//! destructive actions — lives here.

use makepad_widgets::*;

use crate::mini_apps::{registry::MiniAppId, versions::AppVersion};

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherAppInfoBase = #(LauncherAppInfo::register_widget(vm))

    // A "label — value" line, the bread and butter of a settings page.
    let InfoRow = View{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 10
        padding: Inset{left: 16, right: 16, top: 5, bottom: 5}
        ai_key := Label{
            width: Fill
            text: ""
            draw_text +: {
                color: #xd9e6ffcc
                text_style: theme.font_regular{font_size: 12}
            }
        }
        ai_val := Label{
            width: Fit
            text: ""
            draw_text +: {
                color: #xf2f6ff
                text_style: theme.font_bold{font_size: 12}
            }
        }
    }

    // One archived version: when, why, and a Restore button.
    let VersionRow = View{
        width: Fill
        height: 58
        flow: Right
        align: Align{y: 0.5}
        spacing: 10
        padding: Inset{left: 16, right: 12}
        View{
            width: Fill
            height: Fit
            flow: Down
            spacing: 2
            clip_x: true
            vh_when := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    text_style: theme.font_bold{font_size: 12.5}
                }
            }
            vh_note := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 10}
                }
            }
        }
        vh_restore := glass.GlassButton{
            text: "Restore"
            width: 78
            height: 28
            draw_text +: { text_style: theme.font_bold{font_size: 11} }
        }
    }

    let SectionLabel = Label{
        margin: Inset{left: 16, top: 10, bottom: 2}
        draw_text +: {
            color: #x9dccff99
            text_style: theme.font_bold{font_size: 9.5}
        }
    }

    mod.widgets.LauncherAppInfo = set_type_default() do mod.widgets.LauncherAppInfoBase{
        // Full-screen, like an open mini-app: this is a page you go INTO, not a
        // popover, and its content (version history especially) outgrows any
        // fixed card. Sides keep the app window's hairline inset; top and
        // bottom give up more so a strip of dimmed backdrop stays tappable —
        // dismissing by tapping outside should still work, and there has to be
        // an outside for that.
        width: Fill
        height: Fill
        flow: Down
        margin: Inset{
            top: (28.0 + mod.widgets.SAFE_INSET_PAD_TOP),
            bottom: (28.0 + mod.widgets.SAFE_INSET_PAD_BOTTOM),
            left: (4.0 + mod.widgets.SAFE_INSET_PAD_LEFT),
            right: (4.0 + mod.widgets.SAFE_INSET_PAD_RIGHT),
        }

        glass.Panel{
            width: Fill
            height: Fill
            flow: Down
            spacing: 0
            padding: Inset{top: 14, bottom: 12, left: 0, right: 0}

            // Header: icon, name, and what kind of app this is.
            View{
                width: Fill
                height: Fit
                flow: Right
                spacing: 12
                align: Align{y: 0.5}
                padding: Inset{left: 10, right: 16, bottom: 8}
                // Same window control as a mini-app's host header: × at the
                // top-left, U+00D7 (U+2715 isn't in IBM Plex Sans and draws a
                // .notdef box). A modal you can only dismiss by tapping the
                // dimmed backdrop isn't discoverable, and it's the one page
                // here you reach from somewhere else.
                ai_close := glass.GlassButton{
                    width: 38
                    height: 34
                    text: "×"
                    draw_text +: {
                        text_style: theme.font_bold{font_size: 22}
                    }
                }
                ai_glyph := Label{
                    text: ""
                    draw_text +: { text_style: theme.font_regular{font_size: 34} }
                }
                View{
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 2
                    ai_name := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #ffffff
                            text_style: theme.font_bold{font_size: 17}
                        }
                    }
                    ai_kind := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #x9dccffcc
                            text_style: theme.font_regular{font_size: 11}
                        }
                    }
                }
            }

            // Everything below the header scrolls — the header keeps the ×
            // on screen, which is the point of having it.
            ai_body := ScrollYView{
                width: Fill
                height: Fill
                flow: Down
                spacing: 0
                // Primary actions.
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 8
                    padding: Inset{left: 16, right: 16, bottom: 4}
                    ai_open := glass.GlassButtonProminent{
                        width: Fill
                        height: 36
                        text: "Open"
                        draw_text +: { text_style: theme.font_bold{font_size: 13} }
                    }
                    ai_modify := glass.GlassButton{
                        width: Fill
                        height: 36
                        text: "✏️  Modify"
                        draw_text +: { text_style: theme.font_bold{font_size: 13} }
                    }
                    ai_force_stop := glass.GlassButton{
                        width: Fill
                        height: 36
                        text: "Force Stop"
                        draw_text +: { text_style: theme.font_bold{font_size: 13} }
                    }
                }

                SectionLabel{text: "ABOUT"}
                ai_row_kind := InfoRow{}
                ai_row_home := InfoRow{}
                ai_row_widget := InfoRow{}
                ai_row_net := InfoRow{}

                SectionLabel{text: "STORAGE"}
                // Saved data gets its own row so it can carry a Clear button.
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{y: 0.5}
                    spacing: 10
                    padding: Inset{left: 16, right: 12, top: 3, bottom: 3}
                    Label{
                        width: Fill
                        text: "Saved data"
                        draw_text +: {
                            color: #xd9e6ffcc
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                    ai_data_size := Label{
                        width: Fit
                        text: ""
                        draw_text +: {
                            color: #xf2f6ff
                            text_style: theme.font_bold{font_size: 12}
                        }
                    }
                    ai_clear_data := glass.GlassButton{
                        text: "Clear"
                        width: 66
                        height: 28
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                }
                ai_row_code := InfoRow{}

                ai_versions_label := SectionLabel{text: "VERSION HISTORY"}
                ai_ver_0 := VersionRow{}
                ai_ver_1 := VersionRow{}
                ai_ver_2 := VersionRow{}
                ai_ver_3 := VersionRow{}
                ai_more := Label{
                    visible: false
                    width: Fill
                    margin: Inset{left: 16, top: 2}
                    text: ""
                    draw_text +: {
                        color: #x9dccff99
                        text_style: theme.font_regular{font_size: 10}
                    }
                }

                // Destructive action, visually separated at the bottom.
                View{
                    width: Fill
                    height: Fit
                    padding: Inset{left: 16, right: 16, top: 12}
                    ai_uninstall := glass.GlassButton{
                        width: Fill
                        height: 36
                        text: "Uninstall"
                        draw_text +: {
                            color: #xff8f7a
                            text_style: theme.font_bold{font_size: 13}
                        }
                    }
                }
            }
        }
    }
}

/// Version rows the page shows at once; the rest stay on disk.
const AI_VER_IDS: [&[LiveId]; 4] = [
    ids!(ai_ver_0),
    ids!(ai_ver_1),
    ids!(ai_ver_2),
    ids!(ai_ver_3),
];

/// Everything the page needs to render, gathered by the app layer.
#[derive(Clone, Debug, Default)]
pub struct AppInfoContext {
    pub app_id: MiniAppId,
    pub name: String,
    pub icon: String,
    pub builtin: bool,
    /// True when a user modification overrode a built-in.
    pub overridden: bool,
    pub running: bool,
    pub allow_net: bool,
    pub has_widget: bool,
    /// Home-screen icon and widget placement counts, and whether it's docked.
    pub home_icons: usize,
    pub home_widgets: usize,
    pub in_dock: bool,
    /// Bytes in the app's private storage jail, and its source size.
    pub data_bytes: u64,
    pub code_bytes: u64,
    pub versions: Vec<AppVersion>,
    pub utc_offset_secs: i64,
}

/// What the user picked on the page.
#[derive(Clone, Debug, Default)]
pub enum AppInfoAction {
    /// Dismiss the page (its × button).
    Close,
    Open(MiniAppId),
    Modify(MiniAppId),
    ForceStop(MiniAppId),
    ClearData(MiniAppId),
    Uninstall(MiniAppId),
    Restore { app_id: MiniAppId, stamp: String },
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherAppInfo {
    #[deref]
    view: View,
    #[rust]
    context: Option<AppInfoContext>,
}

impl LauncherAppInfo {
    /// Renders the page for one app.
    pub fn show(&mut self, cx: &mut Cx, context: AppInfoContext) {
        self.view.label(cx, ids!(ai_glyph)).set_text(cx, &context.icon);
        self.view.label(cx, ids!(ai_name)).set_text(cx, &context.name);
        self.view
            .label(cx, ids!(ai_kind))
            .set_text(cx, &format!("{} · {}", context.app_id, kind_label(&context)));

        // Force Stop only means something while the app is actually running.
        self.view
            .widget(cx, ids!(ai_force_stop))
            .set_visible(cx, context.running);
        // A built-in can't be uninstalled — its override is reverted through
        // version history instead.
        self.view
            .widget(cx, ids!(ai_uninstall))
            .set_visible(cx, !context.builtin);

        let row = |view: &View, id: &[LiveId], key: &str, value: &str, cx: &mut Cx| {
            view.label(cx, &[id, ids!(ai_key)].concat()).set_text(cx, key);
            view.label(cx, &[id, ids!(ai_val)].concat()).set_text(cx, value);
        };
        row(&self.view, ids!(ai_row_kind), "Type", kind_label(&context), cx);
        row(
            &self.view,
            ids!(ai_row_home),
            "On home screen",
            &placement_label(context.home_icons, context.home_widgets, context.in_dock),
            cx,
        );
        row(
            &self.view,
            ids!(ai_row_widget),
            "Provides a widget",
            if context.has_widget { "Yes" } else { "No" },
            cx,
        );
        row(
            &self.view,
            ids!(ai_row_net),
            "Network access",
            if context.allow_net { "Allowed" } else { "Sandboxed (no network)" },
            cx,
        );
        self.view
            .label(cx, ids!(ai_data_size))
            .set_text(cx, &format_bytes(context.data_bytes));
        self.view
            .widget(cx, ids!(ai_clear_data))
            .set_visible(cx, context.data_bytes > 0);
        row(
            &self.view,
            ids!(ai_row_code),
            "App code",
            &format_bytes(context.code_bytes),
            cx,
        );

        // Version history, newest first; the section hides when there is none.
        let has_versions = !context.versions.is_empty();
        self.view
            .widget(cx, ids!(ai_versions_label))
            .set_visible(cx, has_versions);
        for (i, &row_id) in AI_VER_IDS.iter().enumerate() {
            let version = context.versions.get(i);
            self.view
                .widget(cx, row_id)
                .set_visible(cx, version.is_some());
            if let Some(version) = version {
                self.view
                    .label(cx, &[row_id, ids!(vh_when)].concat())
                    .set_text(
                        cx,
                        &crate::mini_apps::versions::label_for(
                            version.at_unix,
                            context.utc_offset_secs,
                        ),
                    );
                let note = if version.note.is_empty() {
                    version.name.clone()
                } else {
                    version.note.clone()
                };
                self.view
                    .label(cx, &[row_id, ids!(vh_note)].concat())
                    .set_text(cx, &ellipsize(&note, 34));
            }
        }
        let hidden = context.versions.len().saturating_sub(AI_VER_IDS.len());
        let more = self.view.label(cx, ids!(ai_more));
        more.set_visible(cx, hidden > 0);
        if hidden > 0 {
            more.set_text(cx, &format!("+{hidden} older on disk"));
        }

        self.context = Some(context);
        self.view.redraw(cx);
    }
}

fn kind_label(context: &AppInfoContext) -> &'static str {
    match (context.builtin, context.overridden) {
        (true, true) => "Built-in app (modified)",
        (true, false) => "Built-in app",
        (false, _) => "Installed app",
    }
}

fn placement_label(icons: usize, widgets: usize, in_dock: bool) -> String {
    let mut parts = Vec::new();
    if icons > 0 {
        parts.push(format!("{icons} icon{}", plural(icons)));
    }
    if widgets > 0 {
        parts.push(format!("{widgets} widget{}", plural(widgets)));
    }
    if in_dock {
        parts.push("in dock".to_string());
    }
    if parts.is_empty() {
        return "Not placed".to_string();
    }
    parts.join(", ")
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

/// Human byte sizes, the way a settings page shows them.
fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "None".to_string();
    }
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    format!("{:.1} MB", kb / 1024.0)
}

/// Trims to `max` chars with an ellipsis, on a char boundary.
fn ellipsize(text: &str, max: usize) -> String {
    let text = text.replace('\n', " ");
    if text.chars().count() <= max {
        return text;
    }
    let cut: String = text.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

impl Widget for LauncherAppInfo {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        let Some(context) = self.context.clone() else {
            return;
        };
        let id = context.app_id.clone();

        let action = if self.view.glass_button(cx, ids!(ai_close)).clicked(actions) {
            AppInfoAction::Close
        } else if self.view.glass_button(cx, ids!(ai_open)).clicked(actions) {
            AppInfoAction::Open(id)
        } else if self.view.glass_button(cx, ids!(ai_modify)).clicked(actions) {
            AppInfoAction::Modify(id)
        } else if self.view.glass_button(cx, ids!(ai_force_stop)).clicked(actions) {
            AppInfoAction::ForceStop(id)
        } else if self.view.glass_button(cx, ids!(ai_clear_data)).clicked(actions) {
            AppInfoAction::ClearData(id)
        } else if self.view.glass_button(cx, ids!(ai_uninstall)).clicked(actions) {
            AppInfoAction::Uninstall(id)
        } else {
            // Version restores.
            let mut picked = AppInfoAction::None;
            for (i, &row_id) in AI_VER_IDS.iter().enumerate() {
                if self
                    .view
                    .glass_button(cx, &[row_id, ids!(vh_restore)].concat())
                    .clicked(actions)
                {
                    if let Some(version) = context.versions.get(i) {
                        picked = AppInfoAction::Restore {
                            app_id: context.app_id.clone(),
                            stamp: version.stamp.clone(),
                        };
                    }
                }
            }
            picked
        };
        if !matches!(action, AppInfoAction::None) {
            cx.widget_action(self.widget_uid(), action);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherAppInfoRef {
    /// See [`LauncherAppInfo::show`].
    pub fn show(&self, cx: &mut Cx, context: AppInfoContext) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, context);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_sizes_read_like_a_settings_page() {
        assert_eq!(format_bytes(0), "None");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MB");
    }

    #[test]
    fn placement_label_counts_icons_and_widgets() {
        assert_eq!(placement_label(0, 0, false), "Not placed");
        assert_eq!(placement_label(1, 0, false), "1 icon");
        assert_eq!(placement_label(2, 0, false), "2 icons");
        assert_eq!(placement_label(0, 1, false), "1 widget");
        assert_eq!(placement_label(1, 2, false), "1 icon, 2 widgets");
        // The dock is a placement too — an app only in the dock isn't "unplaced".
        assert_eq!(placement_label(0, 0, true), "in dock");
        assert_eq!(placement_label(1, 0, true), "1 icon, in dock");
    }
}
