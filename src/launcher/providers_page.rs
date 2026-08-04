//! "AI Providers" — add a provider key, switch between the ones you have,
//! replace a key, or forget one.
//!
//! One list rather than two: every provider shows its own state (Active /
//! Use / Add), so selecting, adding and replacing are the same gesture in the
//! same place. A separate "configured" list above an "available" list looked
//! tidier on paper and made you hunt for the row you wanted.
//!
//! The key field is masked with an eye toggle, like robrix's login — it's a
//! secret, and a settings page is exactly where someone else reads over your
//! shoulder.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherProvidersPageBase = #(LauncherProvidersPage::register_widget(vm))

    // One provider: name + what we know about its credential, then its action.
    let ProviderRow = View{
        visible: false
        width: Fill
        height: Fit
        flow: Right
        align: Align{y: 0.5}
        spacing: 10
        // left: 16 is measured, not chosen — it puts the gutter control's
        // centre exactly halfway between the panel's edge and the start of the
        // provider name.
        padding: Inset{left: 16, right: 12, top: 7, bottom: 7}
        align: Align{y: 0.5}
        // The left gutter carries the row's PRIMARY verb, and which verb that
        // is depends on whether there's a key: a set-up provider can be
        // selected (radio), one that isn't can be added (+). One column, one
        // decision, and the rows stay aligned because the holder is always
        // there whichever child is showing.
        View{
            width: 24
            height: 24
            flow: Overlay
            align: Align{x: 0.5, y: 0.5}
            // Lifted onto the TITLE's line instead of the row's centre: the
            // rows are two (sometimes three) lines tall, and a gutter control
            // floating beside the middle of that block reads as belonging to
            // nothing. Bottom margin in a y-centred row lifts by half of it —
            // the value is measured against the title's own centre below.
            margin: Inset{bottom: 24}
            pr_pick := RadioButton{
                // 20 == the mark's own `size` below. The shader draws the mark
                // at the box's left edge, so any box WIDER than the mark leaves
                // the slack on the right and the mark reads left-of-centre.
                // Matching them makes the holder's align do the centring.
                width: 20
                height: 20
                // RadioButtonFlat is built for "mark then label", so it aligns
                // its mark to the LEFT of its box and pads it. With no label
                // that left-biased the mark, and it sat visibly left of the +
                // discs that take the same slot on other rows.
                align: Align{x: 0.5, y: 0.5}
                padding: 0
                text: ""
                // The default 15pt mark is noticeably smaller than the 22px
                // "+" disc that takes its place on rows without a key, and the
                // two swap in and out of the same slot — matching the sizes
                // keeps the gutter from visibly changing weight row to row.
                draw_bg +: {
                    size: uniform(20.0)
                    mark_color: uniform(#x8fa6cc88)
                    mark_color_active: uniform(#xffffff)
                }
            }
            pr_add := glass.GlassButton{
                visible: false
                width: 24
                height: 24
                // Square, and Sdf2d.box doubles the radius, so 6 draws a
                // perfect circle.
                draw_glass +: { corner_radius: uniform(6) }
                text: "+"
                draw_text +: { text_style: theme.font_bold{font_size: 16} }
            }
        }
        View{
            width: Fill
            height: Fit
            flow: Down
            // 0, not 1: the two labels' own line boxes already carry space,
            // and the gap read as a break between two unrelated lines rather
            // than a name with its status underneath.
            spacing: 0
            pr_name := Label{
                width: Fill
                text: ""
                draw_text +: {
                    color: #xf2f6ff
                    // 1.0, not the theme's 1.5: the extra leading is what put
                    // 17px between the name and its own status line.
                    text_style: theme.font_bold{font_size: 13.5, line_spacing: 1.0}
                }
            }
            pr_detail := Label{
                width: Fill
                // Pulled up: the two labels' line boxes left 17px between the
                // name and its own status line, which read as a gap between
                // unrelated rows rather than one item's two lines.
                margin: Inset{top: -3}
                text: ""
                draw_text +: {
                    color: #x9dccffcc
                    text_style: theme.font_regular{font_size: 10}
                }
            }
        }
        // Delete, only for a key this app saved — an exported variable or an
        // `octos auth login` belongs to something else. A trash can rather
        // than an ×: × reads as "dismiss this row", and this destroys a stored
        // credential. Confirmed before it happens (see the confirm modal).
        // ButtonFlatter, not GlassButton: only the flat button carries an icon
        // (`draw_icon`/`icon_walk`) — the glass one draws text only, which is
        // why the eye toggles above are flat too.
        // A glass disc like every other button on this page, with the icon
        // drawn on top: `glass.GlassButton` renders text only, so the disc and
        // the icon have to be separate widgets. Same split as the create bar's
        // send disc — the flat button on top is the hit target.
        pr_forget_wrap := View{
            visible: false
            width: 28
            height: 28
            flow: Overlay
            align: Align{x: 0.5, y: 0.5}
            // A plain RoundedView, NOT a glass button: a glass lens composites
            // over later siblings, so an icon drawn on top of one is invisible
            // (the create bar's send disc is a RoundedView for the same
            // reason). Tinted to read as the destructive action.
            RoundedView{
                width: 28
                height: 28
                show_bg: true
                draw_bg +: {
                    color: #x8a3a3a66
                    border_radius: 14.0
                    border_size: 1.0
                    border_color: #xffb0b04d
                }
            }
            pr_forget := ButtonFlatter{
                width: 28
                height: 28
                // Centre the icon in the disc. All four of these are needed —
                // ButtonFlat's defaults are built for a text button with an
                // optional leading icon, and each one pushes the glyph off the
                // middle of a disc:
                //   margin  — `mspace_v_1`, a 3px vertical inset. Measured: the
                //             button box came out at y=157 inside a wrapper at
                //             y=154, so the icon drew 3px below the disc.
                //   spacing — the icon/label gap, still applied with an empty
                //             label, which shifts the icon left of centre.
                //   padding — the text button's left/right breathing room.
                align: Align{x: 0.5, y: 0.5}
                margin: 0
                spacing: 0
                padding: 0
                text: ""
                draw_icon +: {
                    svg: crate_resource("self:resources/icons/trash.svg")
                    color: #xffc9c9ee
                }
                icon_walk: Walk{width: 15, height: 15}
            }
        }
        // "Make this the one a fresh launch starts with." Only offered on the
        // selected row, and only when it isn't already the default — a button
        // that would change nothing shouldn't be there to press.
        pr_default := glass.GlassButton{
            visible: false
            width: Fit
            height: 28
            padding: Inset{left: 10, right: 10}
            text: "Set as default"
            draw_text +: { text_style: theme.font_bold{font_size: 11} }
        }
        pr_action := glass.GlassButton{
            width: 74
            height: 28
            text: "Add"
            draw_text +: { text_style: theme.font_bold{font_size: 11.5} }
        }
    }

    mod.widgets.LauncherProvidersPage = set_type_default() do mod.widgets.LauncherProvidersPageBase{
        width: Fill
        height: Fill
        // Overlay so the delete confirmation can sit ON the page rather than
        // replacing it — you should be able to see the row you're about to
        // destroy while deciding.
        flow: Overlay
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
            padding: Inset{top: 12, bottom: 12, left: 0, right: 0}

            // Pinned header. The subtitle is deliberately NOT in the title
            // row: with a two-line block in there the row centres on the
            // block, which puts the × between the two lines instead of on the
            // title. Everything in the title row is a single line, so their
            // centres coincide by construction.
            View{
                width: Fill
                height: Fit
                flow: Down
                padding: Inset{left: 10, right: 16, bottom: 6}
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 10
                    align: Align{y: 0.5}
                    pv_close := glass.GlassButton{
                        width: 36
                        height: 36
                        // Square, and Sdf2d.box doubles the radius, so 9 draws a
                        // perfect circle — a single glyph deserves a disc, not a pill.
                        draw_glass +: { corner_radius: uniform(9) }
                        // Measured, not guessed: centred on the row the ×
                        // landed 2px below the title's optical centre (the
                        // 22pt × and the ~16pt title have different ink
                        // boxes). A bottom margin in a y-centred row lifts it
                        // by half the margin.
                        margin: Inset{bottom: 4}
                        text: "×"
                        draw_text +: { text_style: theme.font_bold{font_size: 22} }
                    }
                    pv_title := Label{
                        width: Fill
                        text: "AI Providers"
                        draw_text +: {
                            color: #ffffff
                            text_style: theme.font_bold{font_size: 16}
                        }
                    }
                }
                // Indented under the title: close width + the row's spacing.
                // A spacer, not a margin: it lines the subtitle up under the
                // title by construction (close width + the title row's
                // spacing) instead of relying on how margins compose here.
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    View{width: 48, height: 1}
                    pv_subtitle := Label{
                        width: Fill
                        // Names both controls, because a bare ★ next to a
                        // "Use" button is a guess otherwise — and the whole
                        // point of splitting them is that they differ.
                        text: "Which service writes your apps."
                        draw_text +: {
                            color: #x9dccffcc
                            text_style: theme.font_regular{font_size: 10.5}
                        }
                    }
                }
                // HOW a run executes, which nothing else on this page reveals.
                // The rows below say which service answers and where its key
                // came from; none of that tells you whether the agent is a
                // child process, this process, or a binary the environment
                // pointed us at — and those behave differently enough that
                // "why is it doing X" is unanswerable without it.
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    margin: Inset{top: 6}
                    View{width: 48, height: 1}
                    pv_runtime := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #x9dccff99
                            text_style: theme.font_regular{font_size: 10}
                        }
                    }
                }
            }

                // What's actually stopping a generation, when something is.
                // ABOVE the provider list on purpose: with octos missing,
                // every row below is a key that cannot run, and burying that
                // under them is how the user ends up pasting a second key to
                // fix a problem no key can fix.
                pv_blocker := View{
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 8
                    margin: Inset{left: 16, right: 16, bottom: 8}
                    padding: Inset{left: 14, right: 14, top: 12, bottom: 12}
                    show_bg: true
                    draw_bg +: {
                        // A warm-tinted card rather than the page's cool glass:
                        // this is the one thing on the page that is wrong.
                        pixel: fn(){
                            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                            sdf.box(1.0, 1.0, self.rect_size.x - 2.0, self.rect_size.y - 2.0, 10.0)
                            sdf.fill_keep(vec4(0.23, 0.17, 0.10, 0.80))
                            sdf.stroke(vec4(1.0, 0.72, 0.42, 0.67), 1.0)
                            return sdf.result
                        }
                    }
                    pv_blocker_title := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #xffd9a0
                            text_style: theme.font_bold{font_size: 12.5}
                        }
                    }
                    pv_blocker_detail := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #xf0e4d4
                            text_style: theme.font_regular{font_size: 11}
                        }
                    }
                    // Monospace and small enough that the install line fits
                    // without wrapping: a URL broken across two lines reads as
                    // a typo, and this is text people retype.
                    pv_blocker_cmd := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #xffffff
                            // 7.5 rather than the 8.5 used elsewhere for code:
                            // measured, the 63-character install line needs it
                            // to fit the banner's width. The layouter breaks a
                            // long URL mid-token, so "almost fits" means a
                            // command split across two lines at no space.
                            text_style: theme.font_code{font_size: 7.5}
                        }
                    }
                    // Only shown when there IS a command to copy: a blocker
                    // with no fix-by-command would leave a button that lies.
                    pv_blocker_copy := glass.GlassButton{
                        height: 32
                        padding: Inset{left: 14, right: 14}
                        visible: false
                        text: "Copy install command"
                        draw_text +: { text_style: theme.font_bold{font_size: 11} }
                    }
                }

                // The key entry, revealed by Add / Replace on a row so the
                // field always belongs to a named provider — a bare
                // "paste a key" box is what made this guessy before.
                pv_entry := View{
                    visible: false
                    width: Fill
                    height: Fit
                    flow: Down
                    spacing: 6
                    padding: Inset{left: 16, right: 16, top: 10, bottom: 4}
                    pv_entry_title := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #xf2f6ff
                            text_style: theme.font_bold{font_size: 12.5}
                        }
                    }
                    View{
                        width: Fill
                        height: Fit
                        flow: Overlay
                        align: Align{x: 1.0, y: 0.5}
                        pv_key_input := LauncherTextInput{
                            width: Fill
                            height: 40
                            // One long token: wrapping it reads as noise.
                            flow: Right
                            padding: Inset{top: 10, bottom: 10, left: 10, right: 40}
                            empty_text: "Paste the API key"
                            is_password: true
                            autocapitalize: None
                            autocorrect: Disabled
                            content_type: Password
                        }
                        View{
                            width: 38
                            height: Fill
                            align: Align{x: 0.5, y: 0.5}
                            pv_key_show := ButtonFlatter{
                                width: Fit
                                height: Fit
                                padding: 5
                                text: ""
                                draw_icon +: {
                                    svg: crate_resource("self:resources/icons/eye_closed.svg")
                                    color: #x9dccffcc
                                }
                                icon_walk: Walk{width: 18, height: 18}
                            }
                            pv_key_hide := ButtonFlatter{
                                visible: false
                                width: Fit
                                height: Fit
                                padding: 5
                                text: ""
                                draw_icon +: {
                                    svg: crate_resource("self:resources/icons/eye_open.svg")
                                    color: #x9dccffcc
                                }
                                icon_walk: Walk{width: 18, height: 18}
                            }
                        }
                    }
                    pv_status := Label{
                        width: Fill
                        text: ""
                        draw_text +: {
                            color: #x9dccff
                            text_style: theme.font_bold{font_size: 11}
                        }
                    }
                    View{
                        width: Fill
                        height: Fit
                        flow: Right
                        spacing: 8
                        align: Align{x: 1.0}
                        pv_cancel := glass.GlassButton{
                            width: Fit
                            height: 32
                            text: "Cancel"
                            draw_text +: { text_style: theme.font_bold{font_size: 12} }
                        }
                        pv_save := glass.GlassButtonProminent{
                            width: Fit
                            height: 32
                            text: "Save"
                            draw_text +: { text_style: theme.font_bold{font_size: 12} }
                        }
                    }
                }

            // Shown when an ACP agent command is in charge: without it the
            // list below reads as "nothing is set up" next to a launcher that
            // is working perfectly.
            pv_note := Label{
                visible: false
                width: Fill
                text: ""
                padding: Inset{left: 16, right: 16, bottom: 6}
                draw_text +: {
                    color: #x9dccff
                    text_style: theme.font_regular{font_size: 10.5}
                }
            }

            pv_body := ScrollYView{
                width: Fill
                height: Fill
                flow: Down

                View{
                    width: Fill
                    height: Fit
                    flow: Down

                    pv_0 := ProviderRow{}
                    pv_1 := ProviderRow{}
                    pv_2 := ProviderRow{}
                    pv_3 := ProviderRow{}
                    pv_4 := ProviderRow{}
                    pv_5 := ProviderRow{}
                    pv_6 := ProviderRow{}
                    pv_7 := ProviderRow{}
                    pv_8 := ProviderRow{}
                    pv_9 := ProviderRow{}
                    pv_10 := ProviderRow{}

                    // Where the keys end up, so nobody has to guess.
                    pv_footer := Label{
                        width: Fill
                        text: ""
                        padding: Inset{left: 16, right: 16, top: 14}
                        draw_text +: {
                            color: #x9fb0cc
                            text_style: theme.font_regular{font_size: 9.5}
                        }
                    }
                }
            }
        }

        // Deleting a credential is destructive and can't be undone from here —
        // the key itself is gone and has to be re-pasted. It gets a question.
        pv_confirm := View{
            visible: false
            width: Fill
            height: Fill
            flow: Overlay
            align: Align{x: 0.5, y: 0.5}
            // A scrim, so the sheet reads as modal and stray taps on the list
            // behind it land here instead of on a row.
            show_bg: true
            draw_bg +: {
                pixel: fn(){
                    return vec4(0.02, 0.03, 0.06, 0.72)
                }
            }
            glass.Panel{
                width: Fill
                height: Fit
                flow: Down
                spacing: 12
                margin: Inset{left: 22, right: 22}
                padding: Inset{left: 20, right: 20, top: 18, bottom: 16}
                pv_confirm_title := Label{
                    width: Fill
                    text: "Delete this provider?"
                    draw_text +: {
                        color: #xffffff
                        text_style: theme.font_bold{font_size: 14}
                    }
                }
                pv_confirm_body := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #xd8e4ff
                        text_style: theme.font_regular{font_size: 11.5}
                    }
                }
                View{
                    width: Fill
                    height: Fit
                    flow: Right
                    spacing: 10
                    align: Align{x: 1.0, y: 0.5}
                    pv_confirm_cancel := glass.GlassButton{
                        width: Fit
                        height: 32
                        padding: Inset{left: 16, right: 16}
                        text: "Cancel"
                        draw_text +: { text_style: theme.font_bold{font_size: 11.5} }
                    }
                    pv_confirm_delete := glass.GlassButtonProminent{
                        width: Fit
                        height: 32
                        padding: Inset{left: 16, right: 16}
                        text: "Delete"
                        draw_text +: { text_style: theme.font_bold{font_size: 11.5} }
                    }
                }
            }
        }
    }
}

/// What a row offers, which is entirely decided by where its key lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderState {
    /// In use right now. Its action replaces the key.
    Active,
    /// Set up but not selected. Its action switches to it for this session.
    Configured,
    /// Known to us, no credential yet. Its action adds one.
    Available,
    /// Chosen outside the app (`HOST_LAUNCHER_AGENT_CMD`). Nothing to press —
    /// it's decided by how the launcher was started.
    External,
}

/// One row of the page.
#[derive(Clone, Debug)]
pub struct ProviderEntry {
    pub id: String,
    pub label: String,
    pub detail: String,
    pub state: ProviderState,
    /// Whether this app may forget the credential (it only owns what it wrote).
    pub forgettable: bool,
    /// Whether a fresh launch would start with this one.
    pub is_default: bool,
    /// Whether the star should be offered at all — there's nothing to make the
    /// default about a provider with no credential, or about an agent command
    /// (which is decided by the environment, not by this page).
    pub can_default: bool,
}

/// Everything the page renders.
#[derive(Clone, Debug, Default)]
pub struct ProvidersContext {
    pub entries: Vec<ProviderEntry>,
    /// The provider whose key is being entered: (id, prompt line).
    pub pending: Option<(String, String)>,
    /// Error or hint under the key field.
    pub status: String,
    /// Path of the file the keys are written to.
    pub config_path: String,
    /// Explains an externally-chosen agent, when there is one.
    pub note: String,
    /// One line on HOW a run executes — child process, this process, or a
    /// command the environment chose. Computed by the app from
    /// `generate::runtime`, not derived here: it has to be the same answer
    /// `start_backend` acts on.
    pub runtime: String,
}

#[derive(Clone, Debug, Default)]
pub enum ProvidersAction {
    Close,
    /// Open the key field for this provider (Add, or Replace on the active one).
    EnterKey(String),
    /// Switch to an already-configured provider for the rest of this session.
    Use(String),
    /// Make this provider what a fresh launch starts with.
    MakeDefault(String),
    /// Drop a saved key. Only ever emitted by the confirmation's Delete.
    Forget(String),
    /// Commit the typed key for the pending provider.
    Save(String),
    /// Abandon key entry.
    CancelEntry,
    #[default]
    None,
}

const ROW_IDS: [&[LiveId]; 11] = [
    ids!(pv_0),
    ids!(pv_1),
    ids!(pv_2),
    ids!(pv_3),
    ids!(pv_4),
    ids!(pv_5),
    ids!(pv_6),
    ids!(pv_7),
    ids!(pv_8),
    ids!(pv_9),
    ids!(pv_10),
];

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherProvidersPage {
    #[deref]
    view: View,
    /// The provider the confirmation sheet is currently asking about.
    #[rust]
    confirming: Option<String>,
    #[rust]
    entries: Vec<ProviderEntry>,
}

impl LauncherProvidersPage {
    /// Shows or hides every row's interactive control.
    ///
    /// The confirmation sheet dims the list behind it with a scrim, but a
    /// scrim can't dim GLASS: each glass button opens its own draw list and
    /// composites after ordinary content, so the +, Edit and "Set as default"
    /// discs stayed bright on top of the sheet — and stayed clickable. Taking
    /// them out of the tree for the duration is the honest fix: the rows are
    /// still readable underneath, which is the point of a scrim, and nothing
    /// floats above the question.
    fn set_row_controls_visible(&mut self, cx: &mut Cx, on: bool) {
        // The header's × is glass too, and would float over the sheet exactly
        // like the row buttons did.
        self.view.widget(cx, ids!(pv_close)).set_visible(cx, on);
        for (i, &row) in ROW_IDS.iter().enumerate() {
            let Some(entry) = self.entries.get(i).cloned() else {
                continue;
            };
            let has_key = entry.state != ProviderState::Available;
            let configured =
                matches!(entry.state, ProviderState::Active | ProviderState::Configured);
            self.view.widget(cx, &[row, ids!(pr_pick)].concat()).set_visible(cx, on && has_key);
            self.view.widget(cx, &[row, ids!(pr_add)].concat()).set_visible(cx, on && !has_key);
            self.view
                .widget(cx, &[row, ids!(pr_action)].concat())
                .set_visible(cx, on && configured);
            self.view
                .widget(cx, &[row, ids!(pr_forget_wrap)].concat())
                .set_visible(cx, on && entry.forgettable);
            self.view.widget(cx, &[row, ids!(pr_default)].concat()).set_visible(
                cx,
                on && entry.can_default
                    && entry.state == ProviderState::Active
                    && !entry.is_default,
            );
        }
    }

    fn show(&mut self, cx: &mut Cx, context: ProvidersContext) {
        if context.entries.len() > ROW_IDS.len() {
            error!(
                "{} providers but only {} row slots; the extras are hidden",
                context.entries.len(),
                ROW_IDS.len()
            );
        }
        self.entries = context.entries.clone();
        for (i, &row) in ROW_IDS.iter().enumerate() {
            let Some(entry) = context.entries.get(i) else {
                self.view.widget(cx, row).set_visible(cx, false);
                continue;
            };
            self.view
                .label(cx, &[row, ids!(pr_name)].concat())
                .set_text(cx, &entry.label);
            self.view
                .label(cx, &[row, ids!(pr_detail)].concat())
                .set_text(cx, &entry.detail);
            self.view
                .glass_button(cx, &[row, ids!(pr_action)].concat())
                // Selection moved to the radio and adding moved to the gutter's
                // +, so this button is only ever "edit the key I already have".
                .set_text(cx, "Edit");
            self.view.widget(cx, &[row, ids!(pr_action)].concat()).set_visible(
                cx,
                matches!(entry.state, ProviderState::Active | ProviderState::Configured),
            );
            // The gutter holds exactly one control: a radio once there's a key
            // to select, a + while there isn't.
            let has_key = entry.state != ProviderState::Available;
            // External (an agent command) IS what's running, so its radio must
            // read as selected — it was showing unticked, which left NO row
            // ticked at all while an override was in charge. It is also not a
            // choice this page can change, so the click is ignored below.
            let in_use = matches!(entry.state, ProviderState::Active | ProviderState::External);
            self.view
                .radio_button(cx, &[row, ids!(pr_pick)].concat())
                .set_active(cx, in_use, Animate::No);
            self.view.widget(cx, &[row, ids!(pr_pick)].concat()).set_visible(cx, has_key);
            self.view.widget(cx, &[row, ids!(pr_add)].concat()).set_visible(cx, !has_key);
            // "Set as default" belongs to the selected row only, and only when
            // it would actually change something.
            self.view.widget(cx, &[row, ids!(pr_default)].concat()).set_visible(
                cx,
                entry.can_default && entry.state == ProviderState::Active && !entry.is_default,
            );
            self.view
                .widget(cx, &[row, ids!(pr_forget_wrap)].concat())
                .set_visible(cx, entry.forgettable);
            self.view.widget(cx, row).set_visible(cx, true);
        }

        // Lead with what's actually broken — or, when nothing is broken but
        // the setup isn't what it appears to be, say THAT. A working bridge
        // used to leave this page completely silent, which is why "why does
        // Kimi make a Claude noise" was unanswerable from inside the app.
        let banner = crate::generate::blocker()
            .map(|b| (b.title(), b.detail(), b.command()))
            .or_else(|| {
                crate::generate::advisory().map(|(t, d, cmd)| (t, d, Some(cmd)))
            });
        self.view.widget(cx, ids!(pv_blocker)).set_visible(cx, banner.is_some());
        if let Some((title, detail, command)) = &banner {
            self.view.label(cx, ids!(pv_blocker_title)).set_text(cx, title);
            self.view.label(cx, ids!(pv_blocker_detail)).set_text(cx, detail);
            // The command line and the Copy button are one unit: both appear
            // only when there is a command, so Copy is never a dead button.
            self.view
                .label(cx, ids!(pv_blocker_cmd))
                .set_text(cx, command.unwrap_or_default());
            self.view.widget(cx, ids!(pv_blocker_cmd)).set_visible(cx, command.is_some());
            self.view.widget(cx, ids!(pv_blocker_copy)).set_visible(cx, command.is_some());
        }

        // Worked out by `generate::runtime`, which shares its command-building
        // with `start_backend` — so this line cannot claim one thing while a
        // run does another.
        self.view.label(cx, ids!(pv_runtime)).set_text(cx, &context.runtime);

        let entering = context.pending.is_some();
        self.view.widget(cx, ids!(pv_entry)).set_visible(cx, entering);
        if let Some((_, prompt)) = &context.pending {
            self.view.label(cx, ids!(pv_entry_title)).set_text(cx, prompt);
        }
        self.view.label(cx, ids!(pv_status)).set_text(cx, &context.status);
        self.view.label(cx, ids!(pv_note)).set_text(cx, &context.note);
        self.view
            .widget(cx, ids!(pv_note))
            .set_visible(cx, !context.note.is_empty());
        self.view.label(cx, ids!(pv_footer)).set_text(
            cx,
            &format!(
                "Keys are written to octos's own config ({}), owner-only. This app keeps no copy.",
                context.config_path
            ),
        );
        self.view.redraw(cx);
    }

    /// Masks or reveals the key, swapping which eye is offered.
    fn set_revealed(&mut self, cx: &mut Cx, revealed: bool) {
        self.view
            .text_input(cx, ids!(pv_key_input))
            .set_is_password(cx, !revealed);
        self.view.widget(cx, ids!(pv_key_show)).set_visible(cx, !revealed);
        self.view.widget(cx, ids!(pv_key_hide)).set_visible(cx, revealed);
    }
}

impl Widget for LauncherProvidersPage {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
        let Event::Actions(actions) = event else {
            return;
        };
        let uid = self.widget_uid();

        // The confirmation owns the page while it's up: answering it is the
        // only thing that can happen, so it's checked before anything else.
        if self.confirming.is_some() {
            if self.view.glass_button(cx, ids!(pv_confirm_cancel)).clicked(actions) {
                self.confirming = None;
                self.view.widget(cx, ids!(pv_confirm)).set_visible(cx, false);
                self.set_row_controls_visible(cx, true);
                self.view.redraw(cx);
                return;
            }
            if self.view.glass_button(cx, ids!(pv_confirm_delete)).clicked(actions) {
                let id = self.confirming.take().unwrap_or_default();
                self.view.widget(cx, ids!(pv_confirm)).set_visible(cx, false);
                self.set_row_controls_visible(cx, true);
                self.view.redraw(cx);
                cx.widget_action(uid, ProvidersAction::Forget(id));
                return;
            }
            return;
        }
        if self.view.glass_button(cx, ids!(pv_close)).clicked(actions) {
            cx.widget_action(uid, ProvidersAction::Close);
            return;
        }
        if self.view.glass_button(cx, ids!(pv_blocker_copy)).clicked(actions) {
            // Straight to the clipboard: this is a command to paste into a
            // terminal, and retyping a git URL from a phone-sized panel is
            // exactly the friction that makes people give up here. Copies
            // whatever the banner is showing, so the two can't disagree.
            let Some(command) = crate::generate::blocker()
                .and_then(|b| b.command())
                .or_else(|| crate::generate::advisory().map(|(_, _, cmd)| cmd))
            else {
                return;
            };
            cx.copy_to_clipboard(command);
            self.view
                .label(cx, ids!(pv_blocker_title))
                .set_text(cx, "Copied — paste it in a terminal");
            self.view.redraw(cx);
            return;
        }
        for (i, &row) in ROW_IDS.iter().enumerate() {
            let Some(entry) = self.entries.get(i).cloned() else {
                continue;
            };
            // Ask, don't act: this destroys a stored credential.
            if self
                .view
                .button(cx, &[row, ids!(pr_forget)].concat())
                .clicked(actions)
            {
                self.confirming = Some(entry.id.clone());
                self.view.label(cx, ids!(pv_confirm_body)).set_text(
                    cx,
                    &format!(
                        "The saved key for {} will be removed from octos's config. \
                         You'll have to paste it again to use this provider.",
                        entry.label
                    ),
                );
                self.view.widget(cx, ids!(pv_confirm)).set_visible(cx, true);
                self.set_row_controls_visible(cx, false);
                self.view.redraw(cx);
                return;
            }
            if self
                .view
                .glass_button(cx, &[row, ids!(pr_default)].concat())
                .clicked(actions)
            {
                cx.widget_action(uid, ProvidersAction::MakeDefault(entry.id));
                return;
            }
            if self.view.radio_button(cx, &[row, ids!(pr_pick)].concat()).clicked(actions) {
                // An agent command is decided by how the launcher was started;
                // selecting it would store the pseudo-id "agent-command" as a
                // real provider and report "Using agent-command for now".
                if entry.state != ProviderState::External {
                    cx.widget_action(uid, ProvidersAction::Use(entry.id));
                }
                return;
            }
            if self.view.glass_button(cx, &[row, ids!(pr_add)].concat()).clicked(actions) {
                cx.widget_action(uid, ProvidersAction::EnterKey(entry.id));
                return;
            }
            if self
                .view
                .glass_button(cx, &[row, ids!(pr_action)].concat())
                .clicked(actions)
            {
                // Every state that has this button means the same thing now:
                // open the key field for this provider.
                let action = match entry.state {
                    ProviderState::External => ProvidersAction::None,
                    _ => ProvidersAction::EnterKey(entry.id),
                };
                cx.widget_action(uid, action);
                return;
            }
        }

        // The eye, then focus back to the field so typing continues.
        for (button, revealed) in [(ids!(pv_key_show), true), (ids!(pv_key_hide), false)] {
            if self.view.button(cx, button).clicked(actions) {
                self.set_revealed(cx, revealed);
                self.view.text_input(cx, ids!(pv_key_input)).set_key_focus(cx);
            }
        }
        if self.view.glass_button(cx, ids!(pv_cancel)).clicked(actions) {
            cx.widget_action(uid, ProvidersAction::CancelEntry);
        }
        let key = self.view.text_input(cx, ids!(pv_key_input));
        if self.view.glass_button(cx, ids!(pv_save)).clicked(actions)
            || key.returned(actions).is_some()
        {
            cx.widget_action(uid, ProvidersAction::Save(key.text()));
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        self.view.draw_walk(cx, scope, walk)
    }
}

impl LauncherProvidersPageRef {
    pub fn show(&self, cx: &mut Cx, context: ProvidersContext) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.show(cx, context);
        }
    }

    /// Clears the field and re-masks it — on open, and after a key is saved.
    pub fn reset_key_field(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.view.text_input(cx, ids!(pv_key_input)).set_text(cx, "");
            inner.set_revealed(cx, false);
        }
    }

    pub fn focus_key_field(&self, cx: &mut Cx) {
        if let Some(inner) = self.borrow() {
            inner.view.text_input(cx, ids!(pv_key_input)).set_key_focus(cx);
        }
    }
}
