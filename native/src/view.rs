//! The state, drawn: `view.js` again. Nothing the list or an answer says is
//! more than text; every string comes from `strings.rs`.

use hangar_kit_sdk::Handler;
use hangar_kit_sdk::prelude::*;
use hangar_kit_sdk::ui::ListItem;

use crate::logic::{self, Counts, Overall, Saved, Step, StepState, Verdict};
use crate::{Checklist, Loaded, Message, Phase, Problem, Sending};

/// A failed step's note: a few lines, which is what a note is.
const NOTE_HEIGHT: f32 = 80.0;

impl Checklist {
    pub(crate) fn draw(&self) -> Element<Message> {
        match &self.phase {
            Phase::Loading(_) => page![loading(self.t.t("loading"))].into(),
            Phase::Failed(problem) => self.draw_problem(problem),
            Phase::Ready(loaded) => self.draw_list(loaded),
        }
    }

    /// A list that is missing or malformed, named for what it is, with what to
    /// do. Only what trying again can mend has a button.
    fn draw_problem(&self, problem: &Problem) -> Element<Message> {
        let t = &self.t;
        let name = problem.kind.name();
        let detail = match &problem.shape {
            Some(shape) => {
                let vars = shape.vars();
                let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (*k, v.as_str())).collect();
                t.tv(&format!("shape.{}", shape.code()), &vars)
            }
            None => problem.detail.clone(),
        };
        let title = t.t(&format!("err.{name}.title"));
        let text = t.tv(
            &format!("err.{name}.text"),
            &[("path", problem.path.as_str()), ("detail", detail.as_str())],
        );
        let mut shown = match problem.kind {
            logic::Kind::Missing => empty(title).icon(Icon::Info),
            logic::Kind::Denied => denied(title),
            _ => failed(title),
        }
        .text(text);
        if problem.kind.can_retry() {
            shown = shown.action(
                button(t.t("retry"))
                    .small()
                    .primary()
                    .on_press(Message::Retry),
            );
        }
        page![shown].into()
    }

    fn draw_list(&self, loaded: &Loaded) -> Element<Message> {
        let t = &self.t;
        let counts = loaded.counts();

        let (status, state) = match counts.state {
            Overall::Running => (
                Status::Running,
                t.tv(
                    "stateRunning",
                    &[
                        ("done", &counts.answered.to_string()),
                        ("total", &counts.total.to_string()),
                    ],
                ),
            ),
            Overall::Failure => (
                Status::Failure,
                t.tn("stateFailure", counts.fail as i64, &[]),
            ),
            Overall::Success => (
                Status::Success,
                t.t(if counts.skip > 0 {
                    "stateSuccessSkipped"
                } else {
                    "stateSuccess"
                }),
            ),
        };
        let title = if loaded.list.title.is_empty() {
            t.t("untitled")
        } else {
            loaded.list.title.clone()
        };
        let head = header(title).state(state_label(status, state.clone()));

        let bar_kind = match counts.state {
            Overall::Running => ProgressKind::Normal,
            Overall::Failure => ProgressKind::Failure,
            Overall::Success => ProgressKind::Success,
        };
        let progress = progress(Some(counts.answered as f32 / counts.total.max(1) as f32))
            .kind(bar_kind)
            .label(state);

        let mut notes: Vec<Element<Message>> = Vec::new();
        if !self.persist {
            notes.push(banner(Notice::Warning, t.t("persistOff")).into());
        }
        match &self.send {
            Sending::Sent(pane) => {
                notes.push(banner(Notice::Info, t.tv("sentTo", &[("pane", pane.as_str())])).into())
            }
            Sending::Failed { code, detail, .. } => notes.push(
                banner(
                    Notice::Error,
                    t.tv(&format!("send.{code}"), &[("detail", detail.as_str())]),
                )
                .into(),
            ),
            Sending::Idle | Sending::Sending(_) => {}
        }

        let rows = loaded
            .list
            .steps
            .iter()
            .enumerate()
            .map(|(index, step)| self.step(index, step, &loaded.keys[index], &loaded.saved));
        let steps =
            section(t.t("steps"), [list(rows).group().into()]).count(counts.total.to_string());
        notes.push(steps.into());

        page![
            col![head, progress].spacing(6.0).padding([0.0, 12.0]),
            scroll(column(notes).spacing(8.0).padding(12.0)),
            self.foot(&counts),
        ]
        .fill()
        .into()
    }

    /// One step: its text and what to expect, the three choices, and for a
    /// failed one the field for what was seen.
    fn step(
        &self,
        index: usize,
        step: &Step,
        key: &str,
        saved: &Saved,
    ) -> ListItem<Handler<Message>> {
        let t = &self.t;
        let state = logic::state_of(saved, key);
        let (status, name) = match state {
            StepState::Ok => (Status::Success, "ok"),
            StepState::Fail => (Status::Failure, "problem"),
            StepState::Skip => (Status::Neutral, "skip"),
            StepState::Redo => (Status::Waiting, "redo"),
            StepState::Todo => (Status::Neutral, "todo"),
        };
        let mut row = item(format!("{}. {}", index + 1, step.text))
            .lead(glyph(status, t.t(name)))
            .wrap();
        if !step.expect.is_empty() {
            row = row.sub(t.tv("expectedLine", &[("text", step.expect.as_str())]));
        }
        if state == StepState::Redo {
            let was = saved.redo.get(key).map(String::as_str).unwrap_or("");
            row = row.body(
                col![
                    state_label(Status::Waiting, t.t("redo")),
                    text(if was.is_empty() {
                        t.t("redoWasBare")
                    } else {
                        t.tv("redoWas", &[("note", was)])
                    })
                    .small()
                    .dim(),
                ]
                .spacing(2.0),
            );
        }
        let choice = |label: &str, verdict: Verdict, pressed: bool| {
            filter(label, pressed).on_press(Message::Choose(index, verdict))
        };
        row = row.body(
            row![
                choice(&t.t("ok"), Verdict::Ok, state == StepState::Ok),
                choice(&t.t("problem"), Verdict::Fail, state == StepState::Fail),
                choice(&t.t("skip"), Verdict::Skip, state == StepState::Skip),
            ]
            .spacing(4.0)
            .wrap(),
        );
        if let Some(answer) = saved
            .answers
            .get(key)
            .filter(|answer| answer.verdict == Verdict::Fail)
        {
            row = row.body(
                text_area(format!("note-{index}"), answer.note.as_str())
                    .label(t.t("noteLabel"))
                    .placeholder(t.t("notePlaceholder"))
                    .height(NOTE_HEIGHT)
                    .on_input(move |note| Message::Note(index, note)),
            );
        }
        row
    }

    /// What to do with the result: send it to the agent that asked, or, when a
    /// person opened the tab, copy it.
    fn foot(&self, counts: &Counts) -> Element<Message> {
        let t = &self.t;
        let sending = matches!(self.send, Sending::Sending(_));
        let sent = matches!(self.send, Sending::Sent(_));
        let answered = counts.answered > 0;
        let (hint, action): (String, Element<Message>) = if self.caller.is_none() {
            (
                t.t("noCaller"),
                button(t.t("copy"))
                    .small()
                    .primary()
                    .on_press_maybe(answered.then_some(Message::Copy))
                    .into(),
            )
        } else {
            let hint = if !answered {
                t.t("needAnswer")
            } else if counts.pending > 0 {
                t.tn("pending", counts.pending as i64, &[])
            } else {
                String::new()
            };
            let label = if sending {
                t.t("sending")
            } else if sent {
                t.t("sendAgain")
            } else {
                t.t("send")
            };
            let mut send = button(label).small();
            if counts.done && !sent {
                send = send.primary();
            }
            (
                hint,
                send.on_press_maybe((answered && !sending).then_some(Message::Send))
                    .into(),
            )
        };
        // The hint on its own line, whatever the panel's width, the action
        // at the end of the next.
        let mut lines: Vec<Element<Message>> = Vec::new();
        if !hint.is_empty() {
            lines.push(text(hint).small().dim().into());
        }
        lines.push(row![fill(), action].into());
        bar([column(lines).spacing(4.0).into()]).foot().into()
    }
}
