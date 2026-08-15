//! The permission manager: the per-PERMISSION view of the world.
//!
//! App Info answers "what may this app do?". This answers the other question a
//! phone's privacy settings exist for — "which apps can see my location?" —
//! and it is where the access log lives, because "who used what, and when" is
//! a question about capabilities, not about one app.
//!
//! Two stages in one page: a list of every capability with a tally, and a
//! drill-down listing the apps that declare the one you picked. Fixed slots
//! throughout (10 capabilities is the whole catalog; the app list is capped
//! and reports its own overflow rather than truncating in silence).

use makepad_widgets::*;

use crate::mini_apps::registry::MiniAppId;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherPermissionManagerBase = #(LauncherPermissionManager::register_widget(vm))
    mod.widgets.LauncherPermissionPickerBase = #(LauncherPermissionPicker::register_widget(vm))

    let SectionLabel = Label{
        margin: Inset{left: 16, top: 10, bottom: 2}
        draw_text +: {
            color: #x9dccff99
            text_style: theme.font_bold{font_size: 9.5}
        }
    }

    // One capability in the top-level list: what it is and how many apps hold it.
    let CapRow = View{
        visible: false
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 10
        padding: Inset{left: 16, right: 12, top: 5, bottom: 5}
        cap_glyph := Label{
            text: ""
            draw_text +: { text_style: theme.font_regular{font_size: 17} }
        }
        View{
            width: Fill
            height: Fit
            flow: Down
            spacing: 1
            clip_x: true
            cap_title := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    text_style: theme.font_bold{font_size: 12.5}
                }
            }
            cap_tally := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 10}
                }
            }
        }
        cap_open := glass.GlassButton{
            text: "View"
            width: 66
            height: 28
            draw_text +: { text_style: theme.font_bold{font_size: 11} }
        }
    }

    // One app inside a capability's drill-down.
    let CapAppRow = View{
        visible: false
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 10
        padding: Inset{left: 16, right: 12, top: 5, bottom: 5}
        ca_glyph := Label{
            text: ""
            draw_text +: { text_style: theme.font_regular{font_size: 17} }
        }
        View{
            width: Fill
            height: Fit
            flow: Down
            spacing: 1
            clip_x: true
            ca_name := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    text_style: theme.font_bold{font_size: 12.5}
                }
            }
            ca_used := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 10}
                }
            }
        }
        ca_dot := View{
            width: 10
            height: 10
            show_bg: true
            draw_bg +: {
                color: uniform(#x8fe3a3)
                pixel: fn() {
                    let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                    sdf.circle(self.rect_size.x * 0.5, self.rect_size.y * 0.5, self.rect_size.x * 0.5)
                    sdf.fill(self.color)
                    return sdf.result
                }
            }
        }
        ca_state := Label{
            width: 62
            text: ""
            draw_text +: {
                color: #xf2f6ff
                text_style: theme.font_bold{font_size: 10.5}
            }
        }
        ca_change := glass.GlassButton{
            text: "Change"
            width: 76
            height: 28
            draw_text +: { text_style: theme.font_bold{font_size: 11} }
        }
    }

    // One line of the access log.
    let LogRow = Label{
        visible: false
        width: Fill
        margin: Inset{left: 16, top: 1, bottom: 1}
        text: ""
        draw_text +: {
            color: #xd9e6ffcc
            text_style: theme.font_regular{font_size: 11}
        }
    }

    mod.widgets.LauncherPermissionManager = set_type_default() do mod.widgets.LauncherPermissionManagerBase{
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
            padding: Inset{top: 14, bottom: 12, left: 0, right: 0}

            View{
                width: Fill
                height: Fit
                flow: Right
                spacing: 12
                align: Align{y: 0.5}
                padding: Inset{left: 10, right: 16, bottom: 8}
                pm_close := glass.GlassButton{
                    width: 36
                    height: 36
                    draw_glass +: { corner_radius: uniform(9) }
                    margin: Inset{bottom: 4}
                    text: "×"
                    draw_text +: { text_style: theme.font_bold{font_size: 22} }
                }
                pm_title := Label{
                    width: Fill
                    text: "Permissions"
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 17}
                    }
                }
                // Only in the drill-down: back to the capability list.
                pm_back := glass.GlassButton{
                    visible: false
                    text: "Back"
                    width: 68
                    height: 30
                    draw_text +: { text_style: theme.font_bold{font_size: 11} }
                }
            }
            pm_subtitle := Label{
                width: Fill
                margin: Inset{left: 70, bottom: 8}
                text: "What your mini-apps are allowed to do"
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 11}
                }
            }

            pm_body := ScrollYView{
                width: Fill
                height: Fill
                flow: Down

                // Stage 1: every capability, with its tally.
                pm_caps := View{
                    width: Fill
                    height: Fit
                    flow: Down
                    SectionLabel{text: "HOW STRICT"}
                    View{
                        width: Fill
                        height: Fit
                        flow: Right
                        align: Align{y: 0.5}
                        spacing: 10
                        padding: Inset{left: 16, right: 12, top: 4, bottom: 4}
                        View{
                            width: Fill
                            height: Fit
                            flow: Down
                            spacing: 1
                            Label{
                                width: Fill
                                text: "Ask for everything"
                                draw_text +: {
                                    color: #xf2f6ff
                                    text_style: theme.font_bold{font_size: 12.5}
                                }
                            }
                            Label{
                                width: Fill
                                text: "Even the low-risk ones stop being allowed by default."
                                draw_text +: {
                                    color: #x9dccffcc
                                    text_style: theme.font_regular{font_size: 9.5}
                                }
                            }
                        }
                        pm_strict := glass.GlassButton{
                            text: "Off"
                            width: 66
                            height: 28
                            draw_text +: { text_style: theme.font_bold{font_size: 11} }
                        }
                    }
                    View{
                        width: Fill
                        height: Fit
                        padding: Inset{left: 16, right: 16, top: 2, bottom: 2}
                        pm_reset := glass.GlassButton{
                            width: Fill
                            height: 30
                            text: "Reset every app's permissions"
                            draw_text +: {
                                color: #xff8f7a
                                text_style: theme.font_bold{font_size: 11.5}
                            }
                        }
                    }

                    SectionLabel{text: "CAPABILITIES"}
                    pm_cap_0 := CapRow{}
                    pm_cap_1 := CapRow{}
                    pm_cap_2 := CapRow{}
                    pm_cap_3 := CapRow{}
                    pm_cap_4 := CapRow{}
                    pm_cap_5 := CapRow{}
                    pm_cap_6 := CapRow{}
                    pm_cap_7 := CapRow{}
                    pm_cap_8 := CapRow{}
                    pm_cap_9 := CapRow{}
                    pm_cap_10 := CapRow{}
                    pm_cap_11 := CapRow{}

                    SectionLabel{text: "RECENT ACTIVITY"}
                    pm_log_empty := Label{
                        width: Fill
                        margin: Inset{left: 16, top: 2, bottom: 2}
                        text: "Nothing yet — no app has used a capability."
                        draw_text +: {
                            color: #xd9e6ffcc
                            text_style: theme.font_regular{font_size: 11}
                        }
                    }
                    pm_log_0 := LogRow{}
                    pm_log_1 := LogRow{}
                    pm_log_2 := LogRow{}
                    pm_log_3 := LogRow{}
                    pm_log_4 := LogRow{}
                    pm_log_5 := LogRow{}
                    pm_log_6 := LogRow{}
                    pm_log_7 := LogRow{}
                }

                // Stage 2: the apps that declare one capability.
                pm_apps := View{
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    pm_cap_blurb := Label{
                        width: Fill
                        margin: Inset{left: 16, right: 16, top: 2, bottom: 6}
                        text: ""
                        draw_text +: {
                            color: #xc8d6f0
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                    SectionLabel{text: "APPS THAT ASK FOR THIS"}
                    pm_none := Label{
                        visible: false
                        width: Fill
                        margin: Inset{left: 16, top: 2}
                        text: "No installed app declares this."
                        draw_text +: {
                            color: #xd9e6ffcc
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                    pm_app_0 := CapAppRow{}
                    pm_app_1 := CapAppRow{}
                    pm_app_2 := CapAppRow{}
                    pm_app_3 := CapAppRow{}
                    pm_app_4 := CapAppRow{}
                    pm_app_5 := CapAppRow{}
                    pm_app_6 := CapAppRow{}
                    pm_app_7 := CapAppRow{}
                    pm_app_8 := CapAppRow{}
                    pm_app_9 := CapAppRow{}
                    pm_app_10 := CapAppRow{}
                    pm_app_11 := CapAppRow{}
                    pm_more := Label{
                        visible: false
                        width: Fill
                        margin: Inset{left: 16, top: 4}
                        text: ""
                        draw_text +: {
                            color: #x9dccff99
                            text_style: theme.font_regular{font_size: 10}
                        }
                    }
                }
            }
        }
    }

    // A short "pick one" list, used to add a capability to a user-owned app.
    mod.widgets.LauncherPermissionPicker = set_type_default() do mod.widgets.LauncherPermissionPickerBase{
        width: 300
        height: Fit
        flow: Down
        glass.Panel{
            width: Fill
            height: Fit
            flow: Down
            spacing: 6
            padding: Inset{top: 18, bottom: 14, left: 18, right: 18}
            pk_title := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #ffffff
                    text_style: theme.font_bold{font_size: 15}
                }
            }
            pk_body := Label{
                width: Fill
                margin: Inset{bottom: 6}
                text: "It can ask for this once you add it."
                draw_text +: {
                    color: #xc8d6f0
                    text_style: theme.font_regular{font_size: 11.5}
                }
            }
            pk_0 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_1 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_2 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_3 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_4 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_5 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_6 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_7 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_8 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_9 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_10 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
            pk_11 := glass.GlassButton{visible: false, width: Fill, height: 34, text: ""}
        }
    }
}

const CAP_IDS: [&[LiveId]; 12] = [
    ids!(pm_cap_0), ids!(pm_cap_1), ids!(pm_cap_2), ids!(pm_cap_3), ids!(pm_cap_4),
    ids!(pm_cap_5), ids!(pm_cap_6), ids!(pm_cap_7), ids!(pm_cap_8), ids!(pm_cap_9),
    ids!(pm_cap_10), ids!(pm_cap_11),
];

// Every capability needs a row, or the manager would hide one.
const _: () = assert!(CAP_IDS.len() >= crate::permissions::Permission::ALL.len());
const APP_IDS: [&[LiveId]; 12] = [
    ids!(pm_app_0), ids!(pm_app_1), ids!(pm_app_2), ids!(pm_app_3), ids!(pm_app_4),
    ids!(pm_app_5), ids!(pm_app_6), ids!(pm_app_7), ids!(pm_app_8), ids!(pm_app_9),
    ids!(pm_app_10), ids!(pm_app_11),
];
const LOG_IDS: [&[LiveId]; 8] = [
    ids!(pm_log_0), ids!(pm_log_1), ids!(pm_log_2), ids!(pm_log_3),
    ids!(pm_log_4), ids!(pm_log_5), ids!(pm_log_6), ids!(pm_log_7),
];
const PICK_IDS: [&[LiveId]; 12] = [
    ids!(pk_0), ids!(pk_1), ids!(pk_2), ids!(pk_3), ids!(pk_4),
    ids!(pk_5), ids!(pk_6), ids!(pk_7), ids!(pk_8), ids!(pk_9),
    ids!(pk_10), ids!(pk_11),
];

// The picker offers what an app has NOT declared, so it can need every slot.
const _: () = assert!(PICK_IDS.len() >= crate::permissions::Permission::ALL.len());

/// One capability row in the top-level list.
#[derive(Clone, Debug)]
pub struct CapRowInfo {
    pub id: String,
    pub glyph: String,
    pub title: String,
    /// "2 of 3 apps allowed" / "No app asks for this".
    pub tally: String,
}

/// One app row inside a capability's drill-down.
#[derive(Clone, Debug)]
pub struct CapAppInfo {
    pub app_id: MiniAppId,
    pub glyph: String,
    pub name: String,
    pub state_label: String,
    pub state_color: u32,
    /// "last used 2m ago" / "never used" / "" when it can't use it at all.
    pub used: String,
}

/// Everything the page renders, gathered by the app layer.
#[derive(Clone, Debug, Default)]
pub struct PermissionManagerContext {
    pub caps: Vec<CapRowInfo>,
    /// Recent access lines, newest first.
    pub log: Vec<String>,
    /// Whether "ask for everything" is on.
    pub strict: bool,
    /// Set while drilled into one capability.
    pub open_cap: Option<(String, String, String)>,
    /// Apps for the open capability.
    pub apps: Vec<CapAppInfo>,
}

#[derive(Clone, Debug, Default)]
pub enum PermissionManagerAction {
    Close,
    /// Drill into a capability by its permission id.
    OpenCap(String),
    /// Back to the capability list.
    Back,
    /// Flip "ask for everything".
    ToggleStrict,
    /// Forget every answer for every app.
    ResetAll,
    /// Edit one app's grant for the open capability.
    ChooseApp { app_id: MiniAppId, perm: String },
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherPermissionManager {
    #[deref]
    view: View,
    #[rust]
    context: Option<PermissionManagerContext>,
}

impl LauncherPermissionManager {
    pub fn show(&mut self, cx: &mut Cx, context: PermissionManagerContext) {
        let drilled = context.open_cap.is_some();
        self.view.widget(cx, ids!(pm_caps)).set_visible(cx, !drilled);
        self.view.widget(cx, ids!(pm_apps)).set_visible(cx, drilled);
        self.view.widget(cx, ids!(pm_back)).set_visible(cx, drilled);

        if let Some((_, title, blurb)) = &context.open_cap {
            self.view.label(cx, ids!(pm_title)).set_text(cx, title);
            self.view.label(cx, ids!(pm_subtitle)).set_text(cx, "");
            self.view.label(cx, ids!(pm_cap_blurb)).set_text(cx, blurb);
        } else {
            self.view.label(cx, ids!(pm_title)).set_text(cx, "Permissions");
            self.view
                .label(cx, ids!(pm_subtitle))
                .set_text(cx, "What your mini-apps are allowed to do");
        }

        for (i, &row) in CAP_IDS.iter().enumerate() {
            let cap = context.caps.get(i);
            self.view.widget(cx, row).set_visible(cx, cap.is_some());
            if let Some(cap) = cap {
                self.view.label(cx, &[row, ids!(cap_glyph)].concat()).set_text(cx, &cap.glyph);
                self.view.label(cx, &[row, ids!(cap_title)].concat()).set_text(cx, &cap.title);
                self.view.label(cx, &[row, ids!(cap_tally)].concat()).set_text(cx, &cap.tally);
            }
        }

        self.view
            .glass_button(cx, ids!(pm_strict))
            .set_text(cx, if context.strict { "On" } else { "Off" });
        self.view
            .widget(cx, ids!(pm_log_empty))
            .set_visible(cx, context.log.is_empty() && !drilled);
        for (i, &row) in LOG_IDS.iter().enumerate() {
            let line = context.log.get(i);
            self.view.widget(cx, row).set_visible(cx, line.is_some());
            if let Some(line) = line {
                self.view.label(cx, row).set_text(cx, line);
            }
        }

        self.view
            .widget(cx, ids!(pm_none))
            .set_visible(cx, drilled && context.apps.is_empty());
        for (i, &row) in APP_IDS.iter().enumerate() {
            let app = context.apps.get(i);
            self.view.widget(cx, row).set_visible(cx, app.is_some());
            if let Some(app) = app {
                self.view.label(cx, &[row, ids!(ca_glyph)].concat()).set_text(cx, &app.glyph);
                self.view.label(cx, &[row, ids!(ca_name)].concat()).set_text(cx, &app.name);
                self.view.label(cx, &[row, ids!(ca_used)].concat()).set_text(cx, &app.used);
                self.view
                    .label(cx, &[row, ids!(ca_state)].concat())
                    .set_text(cx, &app.state_label);
                let color = state_dot_color(app.state_color);
                let mut dot = self.view.widget(cx, &[row, ids!(ca_dot)].concat());
                script_apply_eval!(cx, dot, {
                    draw_bg +: { color: #(color) }
                });
            }
        }
        // Overflow is reported, never silent.
        let hidden = context.apps.len().saturating_sub(APP_IDS.len());
        let more = self.view.label(cx, ids!(pm_more));
        more.set_visible(cx, hidden > 0);
        if hidden > 0 {
            more.set_text(cx, &format!("+{hidden} more not shown"));
        }

        self.context = Some(context);
        self.view.redraw(cx);
    }
}

impl Widget for LauncherPermissionManager {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else { return };
        let Some(context) = self.context.clone() else { return };

        let mut picked = PermissionManagerAction::None;
        if self.view.glass_button(cx, ids!(pm_close)).clicked(actions) {
            picked = PermissionManagerAction::Close;
        } else if self.view.glass_button(cx, ids!(pm_back)).clicked(actions) {
            picked = PermissionManagerAction::Back;
        } else if self.view.glass_button(cx, ids!(pm_strict)).clicked(actions) {
            picked = PermissionManagerAction::ToggleStrict;
        } else if self.view.glass_button(cx, ids!(pm_reset)).clicked(actions) {
            picked = PermissionManagerAction::ResetAll;
        } else {
            for (i, &row) in CAP_IDS.iter().enumerate() {
                if self
                    .view
                    .glass_button(cx, &[row, ids!(cap_open)].concat())
                    .clicked(actions)
                {
                    if let Some(cap) = context.caps.get(i) {
                        picked = PermissionManagerAction::OpenCap(cap.id.clone());
                    }
                }
            }
            for (i, &row) in APP_IDS.iter().enumerate() {
                if self
                    .view
                    .glass_button(cx, &[row, ids!(ca_change)].concat())
                    .clicked(actions)
                {
                    if let (Some(app), Some((perm, _, _))) =
                        (context.apps.get(i), context.open_cap.as_ref())
                    {
                        picked = PermissionManagerAction::ChooseApp {
                            app_id: app.app_id.clone(),
                            perm: perm.clone(),
                        };
                    }
                }
            }
        }
        if !matches!(picked, PermissionManagerAction::None) {
            cx.widget_action(self.widget_uid(), picked);
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherPermissionManagerRef {
    pub fn show(&self, cx: &mut Cx, context: PermissionManagerContext) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, context);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum PermissionPickerAction {
    /// Index into the options the picker was shown with.
    Pick(usize),
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherPermissionPicker {
    #[deref]
    view: View,
    #[rust]
    count: usize,
}

impl LauncherPermissionPicker {
    pub fn show(&mut self, cx: &mut Cx, app_name: &str, rows: &[String]) {
        self.view
            .label(cx, ids!(pk_title))
            .set_text(cx, &format!("Add a capability to {app_name}"));
        for (i, &id) in PICK_IDS.iter().enumerate() {
            let row = rows.get(i);
            let button = self.view.glass_button(cx, id);
            button.set_visible(cx, row.is_some());
            if let Some(row) = row {
                button.set_text(cx, row);
            }
        }
        self.count = rows.len().min(PICK_IDS.len());
        self.view.redraw(cx);
    }
}

impl Widget for LauncherPermissionPicker {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else { return };
        for i in 0..self.count {
            if self.view.glass_button(cx, PICK_IDS[i]).clicked(actions) {
                cx.widget_action(self.widget_uid(), PermissionPickerAction::Pick(i));
            }
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherPermissionPickerRef {
    pub fn show(&self, cx: &mut Cx, app_name: &str, rows: &[String]) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, app_name, rows);
        }
    }
}

/// 0xRRGGBB grant-state colour, as the shader wants it.
fn state_dot_color(rgb: u32) -> Vec4 {
    Vec4 {
        x: ((rgb >> 16) & 0xff) as f32 / 255.0,
        y: ((rgb >> 8) & 0xff) as f32 / 255.0,
        z: (rgb & 0xff) as f32 / 255.0,
        w: 1.0,
    }
}
