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

    // One resource: what it is, what running out does, the amount in force,
    // and a button to change it. Same shape as PermRow on purpose — from the
    // user's side "what may this app do" and "how much may it use" are two
    // columns of one table.
    let ResRow = View{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 10
        padding: Inset{left: 16, right: 12, top: 4, bottom: 4}
        rr_glyph := Label{
            text: ""
            draw_text +: { text_style: theme.font_regular{font_size: 17} }
        }
        View{
            width: Fill
            height: Fit
            flow: Down
            spacing: 1
            clip_x: true
            rr_title := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    text_style: theme.font_bold{font_size: 12}
                }
            }
            rr_blurb := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 9.5}
                }
            }
        }
        rr_value := Label{
            width: 104
            text: ""
            draw_text +: {
                color: #xf2f6ff
                text_style: theme.font_bold{font_size: 10.5}
            }
        }
        rr_set := glass.GlassButton{
            text: "Change"
            width: 76
            height: 28
            draw_text +: { text_style: theme.font_bold{font_size: 11} }
        }
    }

    // One declared permission: what it is, what it means, and a button that
    // flips the grant. Deny-by-default lives here: the button IS the consent.
    let PermRow = View{
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 10
        padding: Inset{left: 16, right: 12, top: 4, bottom: 4}
        pr_glyph := Label{
            text: ""
            draw_text +: { text_style: theme.font_regular{font_size: 17} }
        }
        View{
            width: Fill
            height: Fit
            flow: Down
            spacing: 1
            clip_x: true
            pr_title := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    text_style: theme.font_bold{font_size: 12}
                }
            }
            pr_blurb := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 9.5}
                }
            }
        }
        // A colored dot carries the state at a glance (a word alone made
        // Allowed and Blocked look identical); the button is the ACTION.
        pr_dot := View{
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
        pr_state := Label{
            width: 62
            text: ""
            draw_text +: {
                color: #xf2f6ff
                text_style: theme.font_bold{font_size: 10.5}
            }
        }
        pr_toggle := glass.GlassButton{
            text: "Change"
            width: 76
            height: 28
            draw_text +: { text_style: theme.font_bold{font_size: 11} }
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
                    width: 36
                    height: 36
                    // Square, and Sdf2d.box doubles the radius, so 9 draws a
                    // perfect circle — a single glyph deserves a disc, not a pill.
                    draw_glass +: { corner_radius: uniform(9) }
                    // Measured: centred on the row the × sat 2px below the
                    // title's optical centre (22pt × vs ~16pt title have
                    // different ink boxes). A bottom margin in a y-centred
                    // row lifts it by half the margin.
                    margin: Inset{bottom: 4}
                    text: "×"
                    draw_text +: {
                        text_style: theme.font_bold{font_size: 22}
                    }
                }
                ai_glyph := Label{
                    text: ""
                    draw_text +: { text_style: theme.font_regular{font_size: 34} }
                }
                ai_name := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #ffffff
                        text_style: theme.font_bold{font_size: 17}
                    }
                }
            }
            // Under the title rather than beside it, so the × can centre on
            // the name. Indent clears the × and the app glyph.
            ai_kind := Label{
                width: Fill
                margin: Inset{left: 70, bottom: 8}
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 11}
                }
            }

            // Everything below the header scrolls — the header keeps the ×
            // on screen, which is the point of having it.
            ai_body := ScrollYView{
                width: Fill
                height: Fill
                flow: Down
                spacing: 0
                // Why this app is dead, when the launcher stopped it for
                // abusing the host bridge. Sits above the actions because it
                // explains why Open does nothing.
                ai_restricted := View{
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 8
                    margin: Inset{left: 16, right: 16, bottom: 8}
                    padding: Inset{left: 12, right: 12, top: 10, bottom: 10}
                    show_bg: true
                    draw_bg +: {
                        color: uniform(#xff8a8a24)
                        pixel: fn() {
                            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                            sdf.box(0.0, 0.0, self.rect_size.x, self.rect_size.y, 8.0)
                            sdf.fill(self.color)
                            return sdf.result
                        }
                    }
                    ai_restricted_text := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #xffc2c2
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                    ai_restricted_allow := glass.GlassButton{
                        width: Fill
                        height: 32
                        text: "Let it run again"
                        draw_text +: { text_style: theme.font_bold{font_size: 12} }
                    }
                }

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

                // Per-permission grant control (docs/PERMISSIONS.md). Fixed
                // slots, like the version rows: apps declare a handful at most.
                SectionLabel{text: "PERMISSIONS"}
                ai_perm_none := Label{
                    width: Fill
                    margin: Inset{left: 16, top: 2, bottom: 2}
                    text: "None — fully sandboxed"
                    draw_text +: {
                        color: #xd9e6ffcc
                        text_style: theme.font_regular{font_size: 12}
                    }
                }
                ai_perm_hint := Label{
                    visible: false
                    width: Fill
                    margin: Inset{left: 16, top: 0, bottom: 2}
                    text: "This app is running; turning Network on or off restarts it."
                    draw_text +: {
                        color: #x9dccff99
                        text_style: theme.font_regular{font_size: 9.5}
                    }
                }
                // Only for apps the user owns: a generated app declares
                // nothing, so without this it could never gain a capability.
                ai_perm_add_row := View{
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{y: 0.5}
                    padding: Inset{left: 16, right: 12, top: 4, bottom: 4}
                    Label{
                        width: Fill
                        text: "Add a capability"
                        draw_text +: {
                            color: #xd9e6ffcc
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                    ai_perm_add := glass.GlassButton{
                        text: "＋ Add"
                        width: 76
                        height: 28
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                }
                ai_perm_0 := PermRow{}
                ai_perm_1 := PermRow{}
                ai_perm_2 := PermRow{}
                ai_perm_3 := PermRow{}
                ai_perm_4 := PermRow{}
                ai_perm_5 := PermRow{}
                ai_perm_6 := PermRow{}
                ai_perm_7 := PermRow{}
                ai_perm_8 := PermRow{}
                ai_perm_9 := PermRow{}
                ai_perm_10 := PermRow{}
                ai_perm_11 := PermRow{}

                // How much of the machine it may use (src/resources.rs).
                SectionLabel{text: "RESOURCES"}
                ai_res_note := Label{
                    width: Fill
                    margin: Inset{left: 16, top: 2, bottom: 2}
                    text: ""
                    draw_text +: {
                        color: #x9dccff99
                        text_style: theme.font_regular{font_size: 9.5}
                    }
                }
                ai_res_0 := ResRow{}
                ai_res_1 := ResRow{}
                ai_res_2 := ResRow{}
                ai_res_3 := ResRow{}
                ai_res_4 := ResRow{}
                ai_res_5 := ResRow{}
                ai_res_6 := ResRow{}
                ai_res_reset := glass.GlassButton{
                    visible: false
                    width: Fill
                    height: 30
                    margin: Inset{left: 16, right: 16, top: 4, bottom: 2}
                    text: "Put every amount back to normal"
                    draw_text +: { text_style: theme.font_bold{font_size: 11} }
                }

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
                // App code gets its own row rather than an InfoRow, so the
                // size can sit beside a button that opens the source.
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{y: 0.5}
                    spacing: 10
                    padding: Inset{left: 16, right: 12, top: 3, bottom: 3}
                    Label{
                        width: Fill
                        text: "App code"
                        draw_text +: {
                            color: #xd9e6ffcc
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                    ai_code_size := Label{
                        width: Fit
                        text: ""
                        draw_text +: {
                            color: #xf2f6ff
                            text_style: theme.font_bold{font_size: 12}
                        }
                    }
                    ai_view_source := glass.GlassButton{
                        text: "View"
                        width: 66
                        height: 28
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                }
                // Export writes a shareable bundle AND copies it to the
                // clipboard; the hint line reports which file it wrote.
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    align: Align{y: 0.5}
                    spacing: 10
                    padding: Inset{left: 16, right: 12, top: 3, bottom: 3}
                    Label{
                        width: Fill
                        text: "Share this app"
                        draw_text +: {
                            color: #xd9e6ffcc
                            text_style: theme.font_regular{font_size: 12}
                        }
                    }
                    ai_export_hint := Label{
                        width: Fit
                        text: ""
                        draw_text +: {
                            color: #x9dccff
                            text_style: theme.font_bold{font_size: 11}
                        }
                    }
                    ai_export := glass.GlassButton{
                        text: "Export"
                        width: 66
                        height: 28
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                }

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

/// Permission rows, sized to the whole catalog: an import can declare every
/// permission there is, and a row that can't render is a grant that can't be
/// revoked.
/// Fixed slots for the RESOURCES rows, like AI_PERM_IDS. One per resource.
const AI_RES_IDS: [&[LiveId]; 7] = [
    ids!(ai_res_0),
    ids!(ai_res_1),
    ids!(ai_res_2),
    ids!(ai_res_3),
    ids!(ai_res_4),
    ids!(ai_res_5),
    // One spare slot: the set of resources has changed twice already, and a
    // row that has no id to render into fails at runtime rather than here.
    ids!(ai_res_6),
];
const _: () = assert!(AI_RES_IDS.len() >= crate::resources::Resource::ALL.len());

const AI_PERM_IDS: [&[LiveId]; 12] = [
    ids!(ai_perm_0),
    ids!(ai_perm_1),
    ids!(ai_perm_2),
    ids!(ai_perm_3),
    ids!(ai_perm_4),
    ids!(ai_perm_5),
    ids!(ai_perm_6),
    ids!(ai_perm_7),
    ids!(ai_perm_8),
    ids!(ai_perm_9),
    ids!(ai_perm_10),
    ids!(ai_perm_11),
];

// Every declarable permission must have a row to land in.
const _: () = assert!(AI_PERM_IDS.len() >= crate::permissions::Permission::ALL.len());

/// One declared permission, pre-rendered by the app layer.
#[derive(Clone, Debug)]
pub struct PermRowInfo {
    /// The permission's wire id (e.g. "location"), echoed back on toggle.
    pub id: String,
    pub glyph: String,
    pub title: String,
    pub blurb: String,
    /// Short current-state word shown beside the dot.
    pub state_label: String,
    pub granted: bool,
    /// Dot colour as 0xRRGGBB: green allowed, amber asks, red blocked.
    pub state_color: u32,
}

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
    /// Declared permissions with their current states, declaration order.
    pub perms: Vec<PermRowInfo>,
    /// Whether the user may add/remove declarations (their own apps only).
    pub can_edit_perms: bool,
    /// Whether ANY isolate of this app is live — fullscreen host or home
    /// tile. `running` only covers the fullscreen host, and a widget-only app
    /// still gets torn down by a network change.
    pub any_isolate_live: bool,
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
    /// How much of the machine this app may use, in force right now, one row
    /// per resource (src/resources.rs).
    pub resources: Vec<ResRowInfo>,
    /// Set when the launcher stopped this app for abusing the host bridge:
    /// the user-facing reason, and how many of its requests were refused
    /// during that run.
    pub restricted: Option<RestrictedInfo>,
}

/// One RESOURCES row: the amount in force and whether the user set it.
#[derive(Clone, Debug)]
pub struct ResRowInfo {
    /// `Resource::id()`, so an action can name it without this module
    /// depending on the enum's ordering.
    pub id: String,
    pub glyph: String,
    pub title: String,
    pub blurb: String,
    /// The amount, already rendered in the resource's own units.
    pub value: String,
    /// True when this is the user's number rather than the shipped default.
    pub custom: bool,
}

/// What the App Info banner says about a stopped app.
#[derive(Clone, Debug)]
pub struct RestrictedInfo {
    pub reason: String,
    pub when: String,
    pub refusals: u64,
}

/// What the user picked on the page.
#[derive(Clone, Debug, Default)]
pub enum AppInfoAction {
    /// Dismiss the page (its × button).
    Close,
    /// Show the app's Splash source in a popup.
    ViewSource(MiniAppId),
    /// Write a shareable bundle and copy it to the clipboard.
    Export(MiniAppId),
    Open(MiniAppId),
    Modify(MiniAppId),
    ForceStop(MiniAppId),
    ClearData(MiniAppId),
    Uninstall(MiniAppId),
    Restore { app_id: MiniAppId, stamp: String },
    /// Open the three-state choice sheet for one permission.
    ChoosePermission { app_id: MiniAppId, perm: String },
    /// Open the picker that adds a capability to a user-owned app.
    AddPermission(MiniAppId),
    /// Lift a restriction the launcher imposed for bridge abuse.
    Unrestrict(MiniAppId),
    /// Open the amount picker for one resource.
    ChooseResource { app_id: MiniAppId, resource: String },
    /// Put every resource amount for this app back to the default.
    ResetResources(MiniAppId),
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
        // The stopped-for-abuse banner, and the one control that lifts it.
        self.view
            .widget(cx, ids!(ai_restricted))
            .set_visible(cx, context.restricted.is_some());
        if let Some(r) = &context.restricted {
            let refused = match r.refusals {
                0 => String::new(),
                1 => " 1 of its requests was refused first.".to_string(),
                n => format!(" {n} of its requests were refused first."),
            };
            self.view.label(cx, ids!(ai_restricted_text)).set_text(
                cx,
                &format!(
                    "Stopped {} because it {}.{refused} Its permissions stay off \
                     until you let it run again.",
                    r.when, r.reason
                ),
            );
        }
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

        // Resource rows: always all of them — every app uses every resource,
        // and a row missing because it is "at the default" would hide the one
        // number the user is looking for.
        let custom = context.resources.iter().filter(|r| r.custom).count();
        self.view.label(cx, ids!(ai_res_note)).set_text(
            cx,
            &match custom {
                0 => "Standard amounts for this kind of surface.".to_string(),
                1 => "1 amount set by you.".to_string(),
                n => format!("{n} amounts set by you."),
            },
        );
        self.view
            .widget(cx, ids!(ai_res_reset))
            .set_visible(cx, custom > 0);
        for (i, &row_id) in AI_RES_IDS.iter().enumerate() {
            let res = context.resources.get(i);
            self.view
                .widget(cx, row_id)
                .set_visible(cx, res.is_some());
            let Some(res) = res else { continue };
            self.view
                .label(cx, &[row_id, ids!(rr_glyph)].concat())
                .set_text(cx, &res.glyph);
            self.view
                .label(cx, &[row_id, ids!(rr_title)].concat())
                .set_text(cx, &res.title);
            self.view
                .label(cx, &[row_id, ids!(rr_blurb)].concat())
                .set_text(cx, &res.blurb);
            self.view
                .label(cx, &[row_id, ids!(rr_value)].concat())
                .set_text(cx, &res.value);
            // The user's own number reads differently from a default, or
            // there is no way to see at a glance what you have changed.
            let color = state_dot_color(if res.custom { 0xFFD28A } else { 0xF2F6FF });
            let mut value = self.view.widget(cx, &[row_id, ids!(rr_value)].concat());
            script_apply_eval!(cx, value, {
                draw_text +: { color: #(color) }
            });
        }

        // Permission rows: one per declaration, empty-state label otherwise.
        self.view
            .widget(cx, ids!(ai_perm_none))
            .set_visible(cx, context.perms.is_empty());
        // Honest about consequences: shown when something of this app is
        // actually live (widget tiles included, which `running` misses).
        self.view
            .widget(cx, ids!(ai_perm_hint))
            .set_visible(cx, !context.perms.is_empty() && context.any_isolate_live);
        self.view
            .widget(cx, ids!(ai_perm_add_row))
            .set_visible(cx, context.can_edit_perms);
        for (i, &row_id) in AI_PERM_IDS.iter().enumerate() {
            let perm = context.perms.get(i);
            self.view.widget(cx, row_id).set_visible(cx, perm.is_some());
            if let Some(perm) = perm {
                self.view
                    .label(cx, &[row_id, ids!(pr_glyph)].concat())
                    .set_text(cx, &perm.glyph);
                self.view
                    .label(cx, &[row_id, ids!(pr_title)].concat())
                    .set_text(cx, &perm.title);
                self.view
                    .label(cx, &[row_id, ids!(pr_blurb)].concat())
                    .set_text(cx, &perm.blurb);
                self.view
                    .label(cx, &[row_id, ids!(pr_state)].concat())
                    .set_text(cx, &perm.state_label);
                // Colour the dot for at-a-glance state (words alone read the
                // same); the button stays a plain "Change" action.
                let color = state_dot_color(perm.state_color);
                let mut dot = self.view.widget(cx, &[row_id, ids!(pr_dot)].concat());
                script_apply_eval!(cx, dot, {
                    draw_bg +: { color: #(color) }
                });
            }
        }
        self.view
            .label(cx, ids!(ai_data_size))
            .set_text(cx, &format_bytes(context.data_bytes));
        self.view
            .widget(cx, ids!(ai_clear_data))
            .set_visible(cx, context.data_bytes > 0);
        self.view
            .label(cx, ids!(ai_export_hint))
            .set_text(cx, "");
        self.view
            .label(cx, ids!(ai_code_size))
            .set_text(cx, &format_bytes(context.code_bytes));

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
        } else if self
            .view
            .glass_button(cx, ids!(ai_restricted_allow))
            .clicked(actions)
        {
            AppInfoAction::Unrestrict(id)
        } else if self.view.glass_button(cx, ids!(ai_res_reset)).clicked(actions) {
            AppInfoAction::ResetResources(id)
        } else if let Some(resource) = AI_RES_IDS.iter().enumerate().find_map(|(i, &row_id)| {
            self.view
                .glass_button(cx, &[row_id, ids!(rr_set)].concat())
                .clicked(actions)
                .then(|| {
                    self.context
                        .as_ref()
                        .and_then(|c| c.resources.get(i))
                        .map(|r| r.id.clone())
                })
                .flatten()
        }) {
            AppInfoAction::ChooseResource { app_id: id, resource }
        } else if self.view.glass_button(cx, ids!(ai_view_source)).clicked(actions) {
            AppInfoAction::ViewSource(id)
        } else if self.view.glass_button(cx, ids!(ai_export)).clicked(actions) {
            AppInfoAction::Export(id)
        } else if self.view.glass_button(cx, ids!(ai_clear_data)).clicked(actions) {
            AppInfoAction::ClearData(id)
        } else if self.view.glass_button(cx, ids!(ai_perm_add)).clicked(actions) {
            AppInfoAction::AddPermission(id)
        } else if self.view.glass_button(cx, ids!(ai_uninstall)).clicked(actions) {
            AppInfoAction::Uninstall(id)
        } else {
            // Version restores and permission toggles, by row index.
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
            for (i, &row_id) in AI_PERM_IDS.iter().enumerate() {
                if self
                    .view
                    .glass_button(cx, &[row_id, ids!(pr_toggle)].concat())
                    .clicked(actions)
                {
                    if let Some(perm) = context.perms.get(i) {
                        picked = AppInfoAction::ChoosePermission {
                            app_id: context.app_id.clone(),
                            perm: perm.id.clone(),
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

    /// The app the page is currently rendering, so refreshes can verify they
    /// aren't about to swap in a different app's context.
    pub fn shown_app_id(&self) -> Option<MiniAppId> {
        self.borrow()
            .and_then(|inner| inner.context.as_ref().map(|c| c.app_id.clone()))
    }

    /// Result of the last Export, beside the button. Cleared by the next
    /// `show`, so it never outlives the page it belongs to.
    pub fn set_export_hint(&self, cx: &mut Cx, hint: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.label(cx, ids!(ai_export_hint)).set_text(cx, hint);
            inner.view.redraw(cx);
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

/// 0xRRGGBB grant-state colour, as the shader wants it.
fn state_dot_color(rgb: u32) -> Vec4 {
    Vec4 {
        x: ((rgb >> 16) & 0xff) as f32 / 255.0,
        y: ((rgb >> 8) & 0xff) as f32 / 255.0,
        z: (rgb & 0xff) as f32 / 255.0,
        w: 1.0,
    }
}
