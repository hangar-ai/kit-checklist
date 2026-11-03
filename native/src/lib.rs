//! Checklist, native: the tab of the kit, in Rust compiled to WebAssembly.
//!
//! A person works through a list of manual checks that an agent wrote in a JSON
//! file of the workspace. Each step is marked OK, Problem (with a note of what
//! was seen) or Skip; the answers are kept in the kit's data folder; the result
//! goes back to the agent that opened the tab, who fixes what failed.
//!
//! The tab asks for five permissions:
//!
//! - `logic.rs`: the list checked, the answers kept, the summary.
//! - this file: loading, saving within `data.write`'s rate, the title and the
//!   badge, sending.
//! - `view.rs`: the state, drawn.
//! - `strings.rs`: the words, in English and French.
//!
//! A closing tab gets no last update, so the answers are written as they come,
//! after a pause of 1.5 s, and a tab that goes behind another writes at once.
//! What is closed within the pause is lost.

use std::time::Duration;

use hangar_kit_sdk::api::{BadgeState, DataText, Done, Queued, TabParamsAnswer, Theme};
use hangar_kit_sdk::prelude::*;

pub mod logic;
pub mod strings;
mod view;

use logic::{Answer, Badge, Counts, Kind, List, Overall, Saved, Shape, Verdict};

/// `data.write` is limited to 10 a minute: answers are written after a pause...
pub const SAVE_DELAY_MS: i64 = 1500;
/// ...never closer than this...
pub const SAVE_SPACING_MS: i64 = 7000;
/// ...and tried again after this when Hangar says too many.
pub const RETRY_MS: i64 = 15000;

/// The timer key of the answers' write.
const SAVE: &str = "save";

/// A list, read and checked, with the person's answers to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub list: List,
    /// One per step: `logic::step_keys`.
    pub keys: Vec<String>,
    pub saved: Saved,
}

impl Loaded {
    pub fn counts(&self) -> Counts {
        logic::progress(&self.keys, &self.saved)
    }
}

/// Why there is no list to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub kind: Kind,
    pub path: String,
    /// What the failed call or the parser said.
    pub detail: String,
    /// For `Kind::Shape`: what is wrong with the list.
    pub shape: Option<Shape>,
}

/// A list read, its answers not read yet.
#[derive(Debug)]
struct Incoming {
    list: List,
    keys: Vec<String>,
}

#[derive(Debug)]
enum Phase {
    Loading(Option<Incoming>),
    Ready(Loaded),
    Failed(Problem),
}

/// Where the result's going to the agent stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sending {
    Idle,
    Sending(String),
    Sent(String),
    Failed {
        pane: String,
        /// `denied`, `not_found`, `unavailable`, `rate_limited` or `error`.
        code: &'static str,
        detail: String,
    },
}

pub struct Checklist {
    t: Strings,
    phase: Phase,
    path: Option<String>,
    /// The pane of the agent that asked for the tab; none when the person did.
    caller: Option<String>,
    /// False once the data folder refused: the list still works, unsaved.
    persist: bool,
    send: Sending,
    /// A later read makes the earlier answers stale.
    generation: u64,
    dirty: bool,
    /// The `save` timer waits.
    save_armed: bool,
    /// A `data.write` is out and not answered.
    writing: bool,
    last_write: Option<i64>,
    /// A read waits for the write in flight.
    after_flush: Option<TabParamsAnswer>,
    last_title: Option<String>,
    last_badge: Option<String>,
    last_params: TabParamsAnswer,
}

#[derive(Debug, Clone)]
pub enum Message {
    /// A filter of a step pressed; pressing the chosen one takes the answer back.
    Choose(usize, Verdict),
    /// The note of a failed step, typed.
    Note(usize, String),
    Send,
    Sent(u64, String, Result<Queued, KitError>),
    Copy,
    Copied(Result<Done, KitError>),
    Retry,
    /// The pause is over: write the answers.
    Save,
    Wrote(Result<Done, KitError>),
    /// The tab was asked for again.
    Params(TabParamsAnswer),
    Read(u64, Result<String, KitError>),
    Answers(u64, Result<DataText, KitError>),
    Visible(bool),
    Theme(Theme),
}

impl Kit for Checklist {
    type Message = Message;

    fn init(start: &Start, cx: &mut Context<Message>) -> Self {
        let params = TabParamsAnswer {
            params: start.tab.params.clone(),
            caller: start.tab.caller.clone(),
        };
        let mut tab = Checklist {
            t: strings::strings(&start.theme.locale),
            phase: Phase::Loading(None),
            path: None,
            caller: None,
            persist: true,
            send: Sending::Idle,
            generation: 0,
            dirty: false,
            save_armed: false,
            writing: false,
            last_write: None,
            after_flush: None,
            last_title: None,
            last_badge: None,
            last_params: params.clone(),
        };
        tab.load(params, cx);
        tab
    }

    fn update(&mut self, message: Message, cx: &mut Context<Message>) {
        match message {
            Message::Choose(index, verdict) => self.choose(index, verdict, cx),
            Message::Note(index, note) => self.set_note(index, &note, cx),
            Message::Send => self.send(cx),
            Message::Sent(generation, pane, answer) => self.sent(generation, pane, answer, cx),
            Message::Copy => self.copy(cx),
            Message::Copied(Ok(_)) => cx.toast(Notice::Success, self.t.t("copied")),
            Message::Copied(Err(error)) => cx.toast(Notice::Error, error.message),
            Message::Retry => {
                let params = self.last_params.clone();
                self.load(params, cx);
            }
            Message::Save => {
                self.save_armed = false;
                self.flush(cx);
            }
            Message::Wrote(answer) => self.wrote(answer, cx),
            Message::Params(params) => self.load(params, cx),
            Message::Read(generation, answer) => self.read(generation, answer, cx),
            Message::Answers(generation, answer) => self.answers(generation, answer, cx),
            // Behind another tab, the answers are put away: the page did it
            // when it was hidden.
            Message::Visible(false) => self.flush(cx),
            Message::Visible(true) => {}
            Message::Theme(theme) => self.t.set_locale(theme.locale),
        }
    }

    fn view(&self) -> Element<Message> {
        self.draw()
    }

    fn subscriptions(&self) -> Subscriptions<Message> {
        Subscriptions::none()
            .tab_params(Message::Params)
            .visibility(Message::Visible)
            .theme(Message::Theme)
    }
}

impl Checklist {
    // ------------------------------------------------------------- What a test, or the view, reads.

    /// `loading`, `list` or `error`.
    pub fn view_name(&self) -> &'static str {
        match self.phase {
            Phase::Loading(_) => "loading",
            Phase::Ready(_) => "list",
            Phase::Failed(_) => "error",
        }
    }

    pub fn loaded(&self) -> Option<&Loaded> {
        match &self.phase {
            Phase::Ready(loaded) => Some(loaded),
            _ => None,
        }
    }

    pub fn problem(&self) -> Option<&Problem> {
        match &self.phase {
            Phase::Failed(problem) => Some(problem),
            _ => None,
        }
    }

    pub fn counts(&self) -> Option<Counts> {
        self.loaded().map(Loaded::counts)
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub fn caller(&self) -> Option<&str> {
        self.caller.as_deref()
    }

    /// Whether the answers are being kept.
    pub fn persist(&self) -> bool {
        self.persist
    }

    pub fn sending(&self) -> &Sending {
        &self.send
    }

    /// What the agent would receive now; nothing outside a list.
    pub fn summary(&self) -> String {
        match &self.phase {
            Phase::Ready(loaded) => logic::build_summary(
                &loaded.list,
                &loaded.keys,
                &loaded.saved,
                self.path.as_deref().unwrap_or(""),
            ),
            _ => String::new(),
        }
    }

    // ------------------------------------------------------------- Loading.

    /// Reads the list `params` name, and again whenever an agent asks for the
    /// tab once more. The answers of the list on show are written first.
    fn load(&mut self, params: TabParamsAnswer, cx: &mut Context<Message>) {
        self.generation += 1;
        self.last_params = params.clone();
        if !self.writing && self.dirty && self.persist && self.loaded().is_some() {
            self.flush(cx);
        }
        if self.writing {
            self.after_flush = Some(params);
        } else {
            self.begin(params, cx);
        }
    }

    fn begin(&mut self, params: TabParamsAnswer, cx: &mut Context<Message>) {
        self.disarm(cx);
        self.caller = params.caller.map(|caller| caller.pane);
        self.send = Sending::Idle;
        self.persist = true;
        self.dirty = false;
        self.phase = Phase::Loading(None);
        match logic::read_path(params.params.get("path").map(String::as_str)) {
            Err(kind) => self.fail(
                Problem {
                    kind,
                    path: String::new(),
                    detail: String::new(),
                    shape: None,
                },
                cx,
            ),
            Ok(path) => {
                self.path = Some(path.clone());
                let generation = self.generation;
                cx.call(api::FilesRead { path }, move |answer| {
                    Message::Read(generation, answer)
                });
            }
        }
    }

    fn fail(&mut self, problem: Problem, cx: &mut Context<Message>) {
        self.phase = Phase::Failed(problem);
        self.set_badge(None, cx);
    }

    fn read(
        &mut self,
        generation: u64,
        answer: Result<String, KitError>,
        cx: &mut Context<Message>,
    ) {
        if generation != self.generation {
            return;
        }
        let path = self.path.clone().unwrap_or_default();
        let text = match answer {
            Ok(text) => text,
            Err(error) => {
                let problem = Problem {
                    kind: logic::classify(&error.code),
                    path,
                    detail: logic::detail_of(&error.message),
                    shape: None,
                };
                return self.fail(problem, cx);
            }
        };
        match logic::parse_list(&text, &path) {
            Err(unreadable) => {
                let problem = Problem {
                    kind: unreadable.kind(),
                    path,
                    detail: match &unreadable {
                        logic::Unreadable::Json(detail) => detail.clone(),
                        _ => String::new(),
                    },
                    shape: match unreadable {
                        logic::Unreadable::Shape(shape) => Some(shape),
                        _ => None,
                    },
                };
                self.fail(problem, cx);
            }
            Ok(list) => {
                let keys = logic::step_keys(&list.steps);
                let answers_path = logic::data_path(&path);
                self.phase = Phase::Loading(Some(Incoming { list, keys }));
                cx.call(api::DataRead { path: answers_path }, move |answer| {
                    Message::Answers(generation, answer)
                });
            }
        }
    }

    fn answers(
        &mut self,
        generation: u64,
        answer: Result<DataText, KitError>,
        cx: &mut Context<Message>,
    ) {
        if generation != self.generation {
            return;
        }
        let Phase::Loading(incoming) = &mut self.phase else {
            return;
        };
        let Some(Incoming { list, keys }) = incoming.take() else {
            return;
        };
        let mut saved = match answer {
            Ok(read) => logic::read_saved(&read.text, &keys),
            Err(error) => {
                // Denied, or a folder that fails: work on, unsaved.
                if !error.is(KitError::NOT_FOUND) {
                    self.persist = false;
                }
                Saved::default()
            }
        };
        let redo = logic::mark_failed_for_redo(&mut saved);
        let title = match logic::tab_title(&list) {
            title if title.is_empty() => self.t.t("tab"),
            title => title,
        };
        self.phase = Phase::Ready(Loaded { list, keys, saved });
        if redo {
            self.schedule(cx);
        }
        self.set_title(title, cx);
        self.sync_badge(cx);
    }

    // ------------------------------------------------------------- Title and badge.

    // Both are the tab's own business and never worth failing for.

    fn set_title(&mut self, title: String, cx: &mut Context<Message>) {
        if self.last_title.as_deref() == Some(title.as_str()) {
            return;
        }
        self.last_title = Some(title.clone());
        cx.call_quietly(api::TabSetTitle { title });
    }

    fn set_badge(&mut self, badge: Option<Badge>, cx: &mut Context<Message>) {
        let key = match &badge {
            Some(badge) => format!("{:?}|{}", badge.state, badge.text),
            None => "none".to_owned(),
        };
        if self.last_badge.as_deref() == Some(key.as_str()) {
            return;
        }
        self.last_badge = Some(key);
        cx.call_quietly(match badge {
            Some(badge) => api::TabSetBadge {
                state: Some(match badge.state {
                    Overall::Running => BadgeState::Running,
                    Overall::Failure => BadgeState::Failure,
                    Overall::Success => BadgeState::Success,
                }),
                text: Some(badge.text),
            },
            None => api::TabSetBadge {
                state: None,
                text: None,
            },
        });
    }

    fn sync_badge(&mut self, cx: &mut Context<Message>) {
        let badge = self.loaded().map(|loaded| {
            let t = &self.t;
            logic::badge_for(&loaded.counts(), |n| {
                t.tv("badgeFailure", &[("n", &n.to_string())])
            })
        });
        self.set_badge(badge, cx);
    }

    // ------------------------------------------------------------- The answers, kept.

    /// Writes within `data.write`'s rate: a pause after the first change, and
    /// never closer to the last write than the spacing. The timer is not pushed
    /// back by a change that comes while it waits, so that someone who types
    /// without stopping is still kept.
    fn schedule(&mut self, cx: &mut Context<Message>) {
        if !self.persist || self.loaded().is_none() {
            return;
        }
        self.dirty = true;
        if self.save_armed {
            return;
        }
        let wait = match self.last_write {
            Some(at) => SAVE_DELAY_MS.max(at + SAVE_SPACING_MS - cx.now()),
            None => SAVE_DELAY_MS,
        };
        self.arm(wait, cx);
    }

    fn arm(&mut self, ms: i64, cx: &mut Context<Message>) {
        self.save_armed = true;
        cx.after(SAVE, Duration::from_millis(ms.max(0) as u64), Message::Save);
    }

    fn disarm(&mut self, cx: &mut Context<Message>) {
        if self.save_armed {
            self.save_armed = false;
            cx.cancel(SAVE);
        }
    }

    /// Writes what is not written yet. A refused write turns saving off; a
    /// limit is waited out.
    fn flush(&mut self, cx: &mut Context<Message>) {
        self.disarm(cx);
        // One write at a time: the answer to the one out looks again.
        if self.writing || !self.dirty || !self.persist {
            return;
        }
        let (Some(path), Phase::Ready(loaded)) = (&self.path, &self.phase) else {
            return;
        };
        let call = api::DataWrite {
            path: logic::data_path(path),
            text: logic::write_saved(&loaded.saved),
        };
        self.dirty = false;
        self.writing = true;
        self.last_write = Some(cx.now());
        cx.call(call, Message::Wrote);
    }

    fn wrote(&mut self, answer: Result<Done, KitError>, cx: &mut Context<Message>) {
        self.writing = false;
        match answer {
            Ok(_) => {}
            Err(error) if error.is(KitError::RATE_LIMITED) => {
                self.dirty = true;
                if !self.save_armed {
                    self.arm(RETRY_MS, cx);
                }
            }
            Err(_) => {
                self.dirty = false;
                self.persist = false;
            }
        }
        if let Some(params) = self.after_flush.take() {
            return self.begin(params, cx);
        }
        // Changes that came while the write was out.
        if self.dirty && self.persist && !self.save_armed && self.loaded().is_some() {
            self.schedule(cx);
        }
    }

    // ------------------------------------------------------------- Answering.

    /// An answer after the result went out is a result not yet sent.
    fn changed(&mut self, cx: &mut Context<Message>) {
        if matches!(self.send, Sending::Sent(_) | Sending::Failed { .. }) {
            self.send = Sending::Idle;
        }
        if let Phase::Ready(loaded) = &mut self.phase {
            loaded.saved.sent_at = None;
        }
        self.schedule(cx);
        self.sync_badge(cx);
    }

    fn choose(&mut self, index: usize, verdict: Verdict, cx: &mut Context<Message>) {
        let Phase::Ready(loaded) = &mut self.phase else {
            return;
        };
        let Some(key) = loaded.keys.get(index).cloned() else {
            return;
        };
        let previous = loaded.saved.answers.get(&key);
        if previous.is_some_and(|answer| answer.verdict == verdict) {
            // Back to « to do »: the answer, and the reminder of an earlier
            // failure, are gone.
            loaded.saved.answers.remove(&key);
        } else {
            let answer = match (verdict, previous) {
                (Verdict::Fail, Some(previous)) if previous.verdict == Verdict::Fail => {
                    previous.clone()
                }
                (Verdict::Fail, _) => Answer::failed(""),
                (verdict, _) => Answer::new(verdict),
            };
            loaded.saved.answers.insert(key.clone(), answer);
            if verdict == Verdict::Fail {
                cx.focus(format!("note-{index}"));
            }
        }
        loaded.saved.redo.remove(&key);
        self.changed(cx);
    }

    /// The note of a failed step, typed: kept, with nothing else redrawn.
    fn set_note(&mut self, index: usize, note: &str, cx: &mut Context<Message>) {
        let Phase::Ready(loaded) = &mut self.phase else {
            return;
        };
        let Some(key) = loaded.keys.get(index) else {
            return;
        };
        let Some(answer) = loaded.saved.answers.get_mut(key) else {
            return;
        };
        if answer.verdict != Verdict::Fail {
            return;
        }
        answer.note = logic::clip_note(note);
        loaded.saved.sent_at = None;
        if matches!(self.send, Sending::Sent(_)) {
            self.send = Sending::Idle;
        }
        self.schedule(cx);
    }

    // ------------------------------------------------------------- Sending.

    fn send(&mut self, cx: &mut Context<Message>) {
        let Some(pane) = self.caller.clone() else {
            return;
        };
        if matches!(self.send, Sending::Sending(_)) {
            return;
        }
        match self.counts() {
            Some(counts) if counts.answered > 0 => {}
            _ => return,
        }
        let text = self.summary();
        self.send = Sending::Sending(pane.clone());
        let generation = self.generation;
        cx.call(
            api::PanesSend {
                pane: pane.clone(),
                text,
            },
            move |answer| Message::Sent(generation, pane, answer),
        );
    }

    fn sent(
        &mut self,
        generation: u64,
        pane: String,
        answer: Result<Queued, KitError>,
        cx: &mut Context<Message>,
    ) {
        // The list was read again meanwhile: this answer is to another.
        if generation != self.generation {
            return;
        }
        match answer {
            Err(error) => {
                self.send = Sending::Failed {
                    pane,
                    code: logic::send_error_code(&error.code),
                    detail: logic::detail_of(&error.message),
                };
            }
            Ok(_) => {
                if let Phase::Ready(loaded) = &mut self.phase {
                    loaded.saved.sent_at = Some(cx.now());
                }
                self.dirty = true;
                // Not granted, or over the rate limit: the page says what it can.
                cx.call_quietly(api::Notify {
                    title: self.t.tv("notifySent", &[("pane", &pane)]),
                    body: None,
                });
                self.send = Sending::Sent(pane);
                self.flush(cx);
            }
        }
    }

    /// Copies the summary, for a list nobody asked for.
    fn copy(&mut self, cx: &mut Context<Message>) {
        match self.counts() {
            Some(counts) if counts.answered > 0 => {}
            _ => return,
        }
        let text = self.summary();
        cx.call(api::CopyText { text }, Message::Copied);
    }
}

export_kit!(Checklist);
