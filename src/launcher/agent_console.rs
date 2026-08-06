//! The agent console: everything a generation produced, as a virtualized list.
//!
//! This was two `Label`s in a scroll view — one for the activity trail, one for
//! the whole transcript. That keeps every byte, which is the point, but a
//! `Label` lays out ALL of its text on every change, so a run producing tens of
//! KB spent the frame budget re-laying it out, and scrolling it was worse:
//! there is no partial layout to fall back on, so the cost is paid per frame
//! rather than per change.
//!
//! A `PortalList` pays for what is on screen. The run is kept as lines, and
//! only the visible ones become widgets — a 5,000-line run costs the same to
//! scroll as a 20-line one.
//!
//! Following the tail is the list's own `auto_tail` (set in the DSL below),
//! not anything here: it keeps the newest line in view while you are at the
//! bottom, stops when you scroll up, and re-arms when you scroll back down.
//! That replaced a latch — any scroll turned following off and nothing turned
//! it back on — and then replaced a hand-rolled fix for the latch, which is
//! the version worth remembering: see the DSL comment for the three ordering
//! bugs it cost.

use makepad_widgets::*;

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    mod.widgets.LauncherAgentConsoleBase = #(LauncherAgentConsole::register_widget(vm))

    mod.widgets.LauncherAgentConsole = mod.widgets.LauncherAgentConsoleBase{
        width: Fill
        // Driven from App (see `sync_console_size`): one line, then the full
        // cap once there is more than one line to show. A list can't be Fit —
        // it takes whatever height it is offered — so the height is written
        // from outside.
        height: 22
        flow: Down

        console_list := PortalList{
            width: Fill
            height: Fill
            // The list tails ITSELF. `auto_tail` keeps the last item in view
            // while you're at the bottom, stops the moment you scroll up, and
            // re-arms when you scroll back down — `tail_range = at_end &&
            // auto_tail`, evaluated inside the draw cycle where the extent is
            // actually known.
            //
            // This replaced a hand-rolled version of the same idea, which is
            // worth remembering: every ordering bug it hit came from asking
            // the list questions it couldn't answer yet. `is_at_end` before a
            // draw describes the PREVIOUS extent, `scroll_to_end` before the
            // first draw lands past the content and blanks the console, and
            // `set_first_id_and_scroll` from inside draw aborts the process.
            auto_tail: true
            // Both templates are one full-width line. The trail and the
            // agent's own output differ only in weight: the trail is what the
            // launcher did, the stream is what the agent said, and the run
            // reads as one column either way.
            Trail := View{
                width: Fill
                height: Fit
                padding: Inset{right: 8, bottom: 2}
                line := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #xd9e6ffd8
                        text_style: theme.font_regular{font_size: 11.5}
                    }
                }
            }
            Stream := View{
                width: Fill
                height: Fit
                padding: Inset{right: 8, bottom: 2}
                line := Label{
                    width: Fill
                    text: ""
                    draw_text +: {
                        color: #x9dccff90
                        text_style: theme.font_regular{font_size: 10}
                    }
                }
            }
        }
    }
}

/// Which half of the run a line came from — only the styling differs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConsoleLineKind {
    /// The launcher's own trail: phases, tool calls, validation errors.
    Trail,
    /// The agent's output: its reasoning and the code it wrote.
    Stream,
}

#[derive(Clone, Debug)]
pub struct ConsoleLine {
    pub kind: ConsoleLineKind,
    pub text: String,
}

#[derive(Script, ScriptHook, Widget)]
pub struct LauncherAgentConsole {
    #[deref]
    view: View,
    /// The whole run, oldest first. Never trimmed — this IS the record, and
    /// virtualization is what makes keeping it cheap.
    #[rust]
    lines: Vec<ConsoleLine>,
}

impl Widget for LauncherAgentConsole {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                // An empty list still needs one slot: a zero-length range
                // gives the list no extent, and `auto_tail` has nothing to
                // track.
                list.set_item_range(cx, 0, self.lines.len().max(1));
                while let Some(row_id) = list.next_visible_item(cx) {
                    let Some(line) = self.lines.get(row_id) else {
                        continue;
                    };
                    let template = match line.kind {
                        ConsoleLineKind::Trail => id!(Trail),
                        ConsoleLineKind::Stream => id!(Stream),
                    };
                    let row = list.item(cx, row_id, template);
                    // A recycled item is redrawn every frame; only write the
                    // text when it actually differs, or every frame pays for a
                    // re-layout of every visible line.
                    let label = row.label(cx, ids!(line));
                    if label.text() != line.text {
                        label.set_text(cx, &line.text);
                    }
                    row.draw_all(cx, scope);
                }
            }
        }
        DrawStep::done()
    }
}

impl LauncherAgentConsole {
    /// Replaces the run's contents. Cheap to call repeatedly: the list only
    /// materializes what is on screen, so the cost is the diff of the visible
    /// window, not of the run.
    ///
    /// Tailing is the list's own business (`auto_tail` in the DSL): it follows
    /// while you're at the bottom and leaves you alone when you aren't.
    pub fn set_lines(&mut self, cx: &mut Cx, lines: Vec<ConsoleLine>) {
        if self.lines.len() == lines.len() && self.lines.iter().zip(&lines).all(|(a, b)| a.text == b.text) {
            return;
        }
        self.lines = lines;
        self.redraw(cx);
    }

    pub fn clear(&mut self, cx: &mut Cx) {
        self.lines.clear();
        self.view.portal_list(cx, ids!(console_list)).set_first_id(0);
        self.redraw(cx);
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    /// Writes the box's height. A list can't size itself to its content — it
    /// takes what it is offered — so the console's height is App's decision
    /// (see `sync_console_size`).
    pub fn set_height(&mut self, cx: &mut Cx, height: f64) {
        self.view.walk.height = Size::Fixed(height);
        self.redraw(cx);
    }

    /// Jumps to the newest line. Also re-arms tailing, since being at the end
    /// is exactly what `set_lines` checks.
    pub fn scroll_to_end(&mut self, cx: &mut Cx) {
        self.view.portal_list(cx, ids!(console_list)).scroll_to_end(cx);
        self.redraw(cx);
    }

}
