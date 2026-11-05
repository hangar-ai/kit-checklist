//! Shared by the tests: a pretend Hangar that answers the tab's
//! calls over files and a data folder held in memory, and a clock whose timers
//! only fire when the test says so.
//! No test touches a real file.
#![allow(dead_code)]

use std::collections::BTreeMap;

use serde_json::{Value, json};

use hangar_kit_sdk::api::KitError;
use hangar_kit_sdk::testing::{self, Harness, StartExt};
use hangar_kit_sdk::ui::abi::Call;
use hangar_kit_sdk::ui::{Element, HandlerId};

use kit_checklist_tab::Checklist;
use kit_checklist_tab::logic::{self, Verdict};

pub const LIST_PATH: &str = "recette/login.json";

/// `Date.parse("2026-10-31T18:00:00Z")`, where the page's fake clock starts.
pub const T0: i64 = 1_793_469_600_000;

pub const PERMISSIONS: [&str; 5] = [
    "files.read",
    "data.read",
    "data.write",
    "panes.send",
    "notify",
];

pub fn list() -> Value {
    json!({
        "title": "Login and logout",
        "steps": [
            { "text": "Open /login and enter a wrong password.", "expect": "An error shows under the field." },
            { "text": "Sign in with the test account.", "expect": "The dashboard shows." },
            { "text": "Press Sign out, then Back.", "expect": "The login page shows." }
        ]
    })
}

pub fn list_steps() -> Vec<logic::Step> {
    logic::parse_list(&list().to_string(), LIST_PATH)
        .expect("the list is good")
        .steps
}

pub fn keys() -> Vec<String> {
    logic::step_keys(&list_steps())
}

pub fn refusal(code: &str) -> KitError {
    KitError::new(code, code)
}

/// A window.hangar that answers as docs/kits.md says: `files` is the
/// workspace (path → text), `data` the kit's data folder; `deny` lists the
/// methods answered `denied`; `fail` maps a method to the error code it
/// answers; `write_limit` makes `data.write` answer `rate_limited` past that
/// many writes in a minute of the fake clock.
pub struct Host {
    pub files: BTreeMap<String, String>,
    pub data: BTreeMap<String, String>,
    pub deny: Vec<String>,
    pub fail: BTreeMap<String, String>,
    pub write_limit: usize,
    writes: Vec<i64>,
    /// Every call made, in order: its method and params.
    pub calls: Vec<(String, Value)>,
    /// When each `data.write` was made, answered or not.
    pub write_times: Vec<i64>,
}

impl Host {
    pub fn calls_to(&self, method: &str) -> Vec<&Value> {
        self.calls
            .iter()
            .filter(|(name, _)| name == method)
            .map(|(_, params)| params)
            .collect()
    }

    /// The `tab.setBadge` params, in order.
    pub fn badges(&self) -> Vec<Value> {
        self.calls_to("tab.setBadge").into_iter().cloned().collect()
    }

    /// The answers file of the list at `path`, as written.
    pub fn saved(&self, path: &str) -> Value {
        let text = self
            .data
            .get(&logic::data_path(path))
            .expect("answers were written");
        serde_json::from_str(text).expect("and are JSON")
    }

    fn answer(&mut self, call: &Call, now: i64) -> Result<Value, KitError> {
        let method = call.method.as_str();
        self.calls.push((call.method.clone(), call.params.clone()));
        if self.deny.iter().any(|denied| denied == method) {
            return Err(KitError::new(
                KitError::DENIED,
                format!("this tab was not granted {method}"),
            ));
        }
        if let Some(code) = self.fail.get(method) {
            return Err(refusal(code));
        }
        let path = call.params["path"].as_str().unwrap_or("");
        match method {
            "tab.setTitle" | "tab.setBadge" => Ok(json!({})),
            "files.read" => match self.files.get(path) {
                Some(text) => Ok(json!(text.strip_prefix('\u{feff}').unwrap_or(text))),
                None => Err(KitError::new(
                    KitError::NOT_FOUND,
                    format!("{path} was not found in the workspace folder"),
                )),
            },
            "data.read" => match self.data.get(path) {
                Some(text) => Ok(json!({ "text": text })),
                None => Err(refusal(KitError::NOT_FOUND)),
            },
            "data.write" => {
                self.write_times.push(now);
                self.writes.retain(|at| now - at < 60_000);
                if self.writes.len() >= self.write_limit {
                    return Err(refusal(KitError::RATE_LIMITED));
                }
                self.writes.push(now);
                self.data.insert(
                    path.to_owned(),
                    call.params["text"].as_str().unwrap_or("").to_owned(),
                );
                Ok(json!({}))
            }
            "panes.send" => Ok(json!({ "queued": true, "pane": call.params["pane"] })),
            "notify" => Ok(Value::Null),
            "ui.copy" => Ok(json!({})),
            "ui.toast" => Ok(json!({ "action": false })),
            other => Err(KitError::new(KitError::UNKNOWN, format!("unknown {other}"))),
        }
    }
}

/// What a test opens a tab with.
pub struct Setup {
    pub params: Vec<(String, String)>,
    pub caller: Option<String>,
    pub host: Host,
    pub locale: String,
}

impl Setup {
    /// The tab on `LIST_PATH`, asked for by the pane Ava, over the good list.
    pub fn new() -> Setup {
        let mut files = BTreeMap::new();
        files.insert(LIST_PATH.to_owned(), list().to_string());
        Setup {
            params: vec![("path".into(), LIST_PATH.into())],
            caller: Some("Ava".into()),
            host: Host {
                files,
                data: BTreeMap::new(),
                deny: Vec::new(),
                fail: BTreeMap::new(),
                write_limit: 10,
                writes: Vec::new(),
                calls: Vec::new(),
                write_times: Vec::new(),
            },
            locale: "en".into(),
        }
    }

    pub fn no_params(mut self) -> Setup {
        self.params.clear();
        self
    }

    pub fn path(mut self, path: &str) -> Setup {
        self.params = vec![("path".into(), path.into())];
        self
    }

    pub fn no_caller(mut self) -> Setup {
        self.caller = None;
        self
    }

    pub fn file(mut self, path: &str, text: impl Into<String>) -> Setup {
        self.host.files.insert(path.into(), text.into());
        self
    }

    pub fn no_file(mut self, path: &str) -> Setup {
        self.host.files.remove(path);
        self
    }

    pub fn data(mut self, data: BTreeMap<String, String>) -> Setup {
        self.host.data = data;
        self
    }

    pub fn deny(mut self, methods: &[&str]) -> Setup {
        self.host.deny = methods.iter().map(|m| (*m).into()).collect();
        self
    }

    pub fn fail(mut self, method: &str, code: &str) -> Setup {
        self.host.fail.insert(method.into(), code.into());
        self
    }

    pub fn write_limit(mut self, limit: usize) -> Setup {
        self.host.write_limit = limit;
        self
    }

    pub fn fr(mut self) -> Setup {
        self.locale = "fr".into();
        self
    }

    /// Starts the tab and answers what it asks until it waits for nothing.
    pub fn open(self) -> Sim {
        let mut sim = self.start();
        sim.settle();
        sim
    }

    /// Starts the tab and leaves its first calls waiting.
    pub fn start(self) -> Sim {
        testing::set_now(Some(T0));
        let mut start = testing::start().grant(PERMISSIONS).locale(self.locale);
        for (name, value) in self.params {
            start = start.param(name, value);
        }
        if let Some(pane) = self.caller {
            start = start.caller(pane);
        }
        Sim {
            tab: Harness::<Checklist>::new(start),
            host: self.host,
            now: T0,
            deadlines: BTreeMap::new(),
        }
    }
}

/// A started tab, its pretend Hangar and its clock.
pub struct Sim {
    pub tab: Harness<Checklist>,
    pub host: Host,
    now: i64,
    /// When each waiting `Context::after` timer comes due.
    deadlines: BTreeMap<String, i64>,
}

impl Sim {
    pub fn kit(&self) -> &Checklist {
        self.tab.kit()
    }

    pub fn now(&self) -> i64 {
        self.now
    }

    /// Answers the calls the tab waits on, and those its answers bring, then
    /// notes when the timers it asked for come due.
    pub fn settle(&mut self) {
        while let Some(call) = self.tab.calls().first().cloned() {
            let answer = self.host.answer(&call, self.now);
            self.tab.reply(call.id, answer);
        }
        let waiting: Vec<(String, u32)> = self
            .tab
            .subscriptions()
            .timers
            .iter()
            .filter(|timer| timer.id.starts_with("~after:"))
            .map(|timer| (timer.id.clone(), timer.every_ms))
            .collect();
        self.deadlines
            .retain(|id, _| waiting.iter().any(|(waiting, _)| waiting == id));
        for (id, every_ms) in waiting {
            self.deadlines
                .entry(id)
                .or_insert(self.now + i64::from(every_ms));
        }
    }

    /// Lets `ms` of the fake clock go by, firing the timers that come due, in
    /// order, and answering the calls they bring.
    pub fn advance(&mut self, ms: i64) {
        let end = self.now + ms;
        loop {
            self.settle();
            let due = self
                .deadlines
                .iter()
                .filter(|(_, at)| **at <= end)
                .min_by_key(|(_, at)| **at)
                .map(|(id, at)| (id.clone(), *at));
            let Some((id, at)) = due else { break };
            self.deadlines.remove(&id);
            self.set_now(self.now.max(at));
            self.tab.timer(&id);
        }
        self.set_now(end);
        self.settle();
    }

    fn set_now(&mut self, now: i64) {
        self.now = now;
        testing::set_now(Some(now));
    }

    /// How many timers wait for the clock.
    pub fn pending_timers(&self) -> usize {
        self.deadlines.len()
    }

    /// The handler of the `nth` filter labelled `label`, in the page now.
    fn filter(&self, label: &str, nth: usize) -> HandlerId {
        let mut found = Vec::new();
        self.tab.view().root.for_each(&mut |element| {
            if let Element::Filter(filter) = element
                && filter.label == label
                && let Some(handler) = filter.on_press
            {
                found.push(handler);
            }
        });
        *found
            .get(nth)
            .unwrap_or_else(|| panic!("no filter {label:?} number {nth}"))
    }

    /// Presses the choice of step `index` (from 0), as a person does.
    pub fn press_choice(&mut self, index: usize, label: &str) {
        let handler = self.filter(label, index);
        self.tab.ui(handler, None);
        self.settle();
    }

    /// Answers step `index`: pressing its OK, Problem or Skip.
    pub fn answer(&mut self, index: usize, verdict: Verdict) {
        let label = match verdict {
            Verdict::Ok => "OK",
            Verdict::Fail => "Problem",
            Verdict::Skip => "Skip",
        };
        self.press_choice(index, label);
    }

    /// Takes the answer of step `index` back to « to do »: pressing what is pressed.
    pub fn clear(&mut self, index: usize, verdict: Verdict) {
        self.answer(index, verdict);
    }

    /// Types a note in the field of step `index`.
    pub fn note(&mut self, index: usize, text: &str) {
        self.tab.type_into(&format!("note-{index}"), text);
        self.settle();
    }

    /// Presses a button of the page by its label.
    pub fn press(&mut self, label: &str) {
        self.tab.press(label);
        self.settle();
    }

    /// The tab is asked for again, as an agent does with `kit_tab_open`.
    pub fn ask_again(&mut self, path: Option<&str>, caller: Option<&str>) {
        let mut params = serde_json::Map::new();
        if let Some(path) = path {
            params.insert("path".into(), json!(path));
        }
        self.tab.event(
            "tab.params",
            json!({ "params": params, "caller": caller.map(|pane| json!({ "pane": pane })) }),
        );
        self.settle();
    }

    /// Answers the waiting call to `method` about `path`, and no other: for a
    /// test that holds some answers back.
    pub fn answer_call(&mut self, method: &str, path: &str) {
        let call = self
            .tab
            .calls()
            .iter()
            .find(|call| call.method == method && call.params["path"] == path)
            .cloned()
            .unwrap_or_else(|| panic!("no call to {method} about {path} waits"));
        let answer = self.host.answer(&call, self.now);
        self.tab.reply(call.id, answer);
    }

    /// The tab goes in front of the others, or behind one.
    pub fn visible(&mut self, visible: bool) {
        self.tab
            .event("tab.visibility", json!({ "visible": visible }));
        self.settle();
    }

    /// The `panes.send` texts, in order.
    pub fn sent(&self) -> Vec<String> {
        self.host
            .calls_to("panes.send")
            .iter()
            .map(|params| params["text"].as_str().unwrap_or("").to_owned())
            .collect()
    }
}
