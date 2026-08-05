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
//! It also makes following the tail honest. "Am I at the bottom?" was
//! previously a latch: any scroll turned auto-tail off and nothing turned it
//! back on, so scrolling down to the end left the console frozen while the run
//! carried on beneath it. `PortalList::is_at_end` answers the question
//! directly, which is what a terminal actually does.

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
    /// Set when new lines arrive while the view was already at the bottom, so
    /// the scroll happens on the next draw (when the list knows its new
    /// extent) rather than against the previous one.
    #[rust]
    tail_pending: bool,
}

impl Widget for LauncherAgentConsole {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                // An empty list still needs one slot, or the list has no
                // extent to scroll and reports nonsense for `is_at_end`.
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
    /// Tails only when the view was ALREADY at the bottom — the terminal
    /// bargain. Scroll up to read and the run stops chasing you; scroll back
    /// down and it picks the tail up again, because being at the end is a
    /// question asked of the list rather than a flag we latch.
    pub fn set_lines(&mut self, cx: &mut Cx, lines: Vec<ConsoleLine>) {
        if self.lines.len() == lines.len() && self.lines.iter().zip(&lines).all(|(a, b)| a.text == b.text) {
            return;
        }
        // NOT `lines.is_empty() || ...`: treating a first fill as "at the end"
        // and scrolling there lands past the content on a list that hasn't
        // drawn yet, and the console comes up blank. A run whose lines all
        // arrive at once therefore opens at the top, which is where you want
        // to start reading anyway; a live run appends and tails normally.
        let was_at_end = self.at_end(cx);
        let grew = lines.len() > self.lines.len();
        self.lines = lines;
        if grew && was_at_end {
            self.tail_pending = true;
        }
        self.redraw(cx);
    }

    pub fn clear(&mut self, cx: &mut Cx) {
        self.lines.clear();
        self.tail_pending = false;
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

    fn at_end(&mut self, cx: &mut Cx) -> bool {
        // An empty or not-yet-drawn list counts as "at the end" so the first
        // lines of a run tail without the user having to ask.
        self.view.portal_list(cx, ids!(console_list)).is_at_end()
    }

    /// Jumps to the newest line. Also re-arms tailing, since being at the end
    /// is exactly what `set_lines` checks.
    pub fn scroll_to_end(&mut self, cx: &mut Cx) {
        self.view.portal_list(cx, ids!(console_list)).scroll_to_end(cx);
        self.tail_pending = false;
        self.redraw(cx);
    }

    /// Consumes a pending tail request. Called after the list has drawn, when
    /// it knows its own extent.
    pub fn flush_tail(&mut self, cx: &mut Cx) {
        if std::mem::take(&mut self.tail_pending) {
            self.view.portal_list(cx, ids!(console_list)).scroll_to_end(cx);
        }
    }
}
