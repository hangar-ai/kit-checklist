//! What is neither the behaviour nor the drawing: the params read like form
//! input, the list file checked, the answers kept, the badge and the summary
//! an agent receives. Pure: no SDK type, no clock, no call. `logic.js`, ported.
//!
//! The answers' file and the steps' keys are the HTML tab's, to the byte: a
//! person who opens the same list in Hangar 1.x and in the native Hangar finds
//! the same answers. So text is measured the way JavaScript measures it, in
//! UTF-16 code units, and trimmed the way `String.prototype.trim` trims.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;
use serde_json::Value;

pub const MAX_STEPS: usize = 200;
pub const MAX_TEXT: usize = 1000;
pub const MAX_NOTE: usize = 2000;
/// `panes.send` takes 20 000 characters.
pub const SUMMARY_LIMIT: usize = 19000;
/// `tab.setTitle`.
pub const TITLE_LIMIT: usize = 40;
/// `tab.setBadge`'s text.
pub const BADGE_TEXT_LIMIT: usize = 12;
const PATH_LIMIT: usize = 1024;
const LIST_TITLE_LIMIT: usize = 200;
const ERROR_DETAIL_LIMIT: usize = 300;

// ----------------------------------------------------------------- Text, as JavaScript has it.

/// `String.prototype.length`.
pub fn js_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// `text.slice(0, n)`: the first `n` UTF-16 code units, a character that
/// would be cut in two being left out.
pub fn js_prefix(text: &str, n: usize) -> &str {
    let mut units = 0;
    for (at, c) in text.char_indices() {
        units += c.len_utf16();
        if units > n {
            return &text[..at];
        }
    }
    text
}

/// What `\s` matches, and `trim()` takes off.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// `text.trim()`.
pub fn js_trim(text: &str) -> &str {
    text.trim_matches(is_js_space)
}

/// `text.replace(/\s+/g, " ").trim()`: one line.
fn one_line(text: &str) -> String {
    text.split(is_js_space)
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// At most `max` UTF-16 code units, the last of them a `…` when it was cut.
fn clip(text: &str, max: usize) -> String {
    if js_len(text) > max {
        format!("{}…", js_prefix(text, max - 1))
    } else {
        text.to_owned()
    }
}

/// A note, held to what the answers' file keeps of one.
pub fn clip_note(text: &str) -> String {
    js_prefix(text, MAX_NOTE).to_owned()
}

// ----------------------------------------------------------------- Why a list is not shown.

/// What a list that is not shown is, for the words that say it and whether
/// trying again can help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The tab was opened without a path.
    Missing,
    /// The path cannot be used.
    Path,
    NotFound,
    Denied,
    Json,
    Empty,
    Shape,
    /// Anything else.
    Error,
}

impl Kind {
    /// The middle of the keys of its words: `err.<name>.title`.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Missing => "missing",
            Kind::Path => "path",
            Kind::NotFound => "not_found",
            Kind::Denied => "denied",
            Kind::Json => "json",
            Kind::Empty => "empty",
            Kind::Shape => "shape",
            Kind::Error => "error",
        }
    }

    /// Reading the same thing again may give something else.
    pub fn can_retry(self) -> bool {
        !matches!(self, Kind::Missing | Kind::Path)
    }
}

/// What to say of a refused or failed call: the kind the page has words for.
pub fn classify(code: &str) -> Kind {
    match code {
        "denied" => Kind::Denied,
        "not_found" => Kind::NotFound,
        _ => Kind::Error,
    }
}

/// A call's message, as much as is shown.
pub fn detail_of(message: &str) -> String {
    js_prefix(message, ERROR_DETAIL_LIMIT).to_owned()
}

/// The `path` param, read like form input: relative, inside the workspace
/// folder, not hidden (files.read refuses those), one line.
pub fn read_path(path: Option<&str>) -> Result<String, Kind> {
    let value = path.map(js_trim).unwrap_or("");
    if value.is_empty() {
        return Err(Kind::Missing);
    }
    let path = value.replace('\\', "/");
    let bad = js_len(&path) > PATH_LIMIT
        || path.contains([':', '\r', '\n', '\0'])
        || path.starts_with('/')
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment.starts_with('.'));
    if bad { Err(Kind::Path) } else { Ok(path) }
}

pub fn base_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

// ----------------------------------------------------------------- The list.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub text: String,
    /// What the person should see; empty when the list says nothing.
    pub expect: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    pub title: String,
    pub steps: Vec<Step>,
}

/// Why a list file is not a list, in a form that has words in both languages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    NotObject,
    NoSteps,
    TooMany { max: usize, n: usize },
    StepNotObject { n: usize },
    StepText { n: usize },
    StepExpect { n: usize },
    StepLong { n: usize, max: usize },
    TitleType,
}

impl Shape {
    /// The end of the key of its words: `shape.<code>`.
    pub fn code(&self) -> &'static str {
        match self {
            Shape::NotObject => "notObject",
            Shape::NoSteps => "noSteps",
            Shape::TooMany { .. } => "tooMany",
            Shape::StepNotObject { .. } => "stepNotObject",
            Shape::StepText { .. } => "stepText",
            Shape::StepExpect { .. } => "stepExpect",
            Shape::StepLong { .. } => "stepLong",
            Shape::TitleType => "titleType",
        }
    }

    /// What fills the `{…}` of its words.
    pub fn vars(&self) -> Vec<(&'static str, String)> {
        match self {
            Shape::TooMany { max, n } | Shape::StepLong { n, max } => {
                vec![("max", max.to_string()), ("n", n.to_string())]
            }
            Shape::StepNotObject { n } | Shape::StepText { n } | Shape::StepExpect { n } => {
                vec![("n", n.to_string())]
            }
            Shape::NotObject | Shape::NoSteps | Shape::TitleType => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    /// The file holds nothing.
    Empty,
    /// Not JSON; the parser's words.
    Json(String),
    Shape(Shape),
}

impl Unreadable {
    pub fn kind(&self) -> Kind {
        match self {
            Unreadable::Empty => Kind::Empty,
            Unreadable::Json(_) => Kind::Json,
            Unreadable::Shape(_) => Kind::Shape,
        }
    }
}

/// The list, parsed and checked.
pub fn parse_list(text: &str, path: &str) -> Result<List, Unreadable> {
    let source = text.strip_prefix('\u{feff}').unwrap_or(text);
    if js_trim(source).is_empty() {
        return Err(Unreadable::Empty);
    }
    let data: Value = serde_json::from_str(source)
        .map_err(|error| Unreadable::Json(detail_of(&error.to_string())))?;
    let shape = Unreadable::Shape;
    let Value::Object(data) = data else {
        return Err(shape(Shape::NotObject));
    };
    if data.get("title").is_some_and(|title| !title.is_string()) {
        return Err(shape(Shape::TitleType));
    }
    let raw_steps = match data.get("steps") {
        Some(Value::Array(steps)) if !steps.is_empty() => steps,
        _ => return Err(shape(Shape::NoSteps)),
    };
    if raw_steps.len() > MAX_STEPS {
        return Err(shape(Shape::TooMany {
            max: MAX_STEPS,
            n: raw_steps.len(),
        }));
    }
    let mut steps = Vec::with_capacity(raw_steps.len());
    for (index, raw) in raw_steps.iter().enumerate() {
        let n = index + 1;
        let Value::Object(raw) = raw else {
            return Err(shape(Shape::StepNotObject { n }));
        };
        let text = match raw.get("text") {
            Some(Value::String(text)) if !js_trim(text).is_empty() => text,
            _ => return Err(shape(Shape::StepText { n })),
        };
        let expect = match raw.get("expect") {
            None | Some(Value::Null) => "",
            Some(Value::String(expect)) => expect,
            Some(_) => return Err(shape(Shape::StepExpect { n })),
        };
        if js_len(text) > MAX_TEXT || js_len(expect) > MAX_TEXT {
            return Err(shape(Shape::StepLong { n, max: MAX_TEXT }));
        }
        steps.push(Step {
            text: js_trim(text).to_owned(),
            expect: js_trim(expect).to_owned(),
        });
    }
    let title = match data.get("title") {
        Some(Value::String(title)) if !js_trim(title).is_empty() => {
            js_prefix(js_trim(title), LIST_TITLE_LIMIT).to_owned()
        }
        _ => base_name(path).to_owned(),
    };
    Ok(List { title, steps })
}

// ----------------------------------------------------------------- Identity of a step, and where answers live.

/// FNV-1a over UTF-16 code units, eight hex digits: the page's `fnv`.
pub fn fnv(text: &str) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for unit in text.encode_utf16() {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// A step's identity: a hash of its text, and which of the identical texts it
/// is. Answers follow a step that the agent moved or reworded around, and only
/// a changed text loses its own.
pub fn step_keys(steps: &[Step]) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    steps
        .iter()
        .map(|step| {
            let base = fnv(&step.text);
            let count = seen.entry(base.clone()).or_insert(0);
            let key = format!("{base}-{count}");
            *count += 1;
            key
        })
        .collect()
}

/// Where the answers of the list at `path` live in the kit's data folder.
pub fn data_path(path: &str) -> String {
    let mut slug = String::new();
    let mut in_run = false;
    for c in path.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            slug.push(c);
            in_run = false;
        } else if !in_run {
            slug.push('_');
            in_run = true;
        }
    }
    let start = slug.len().saturating_sub(40);
    format!("answers/{}-{}.json", &slug[start..], fnv(path))
}

// ----------------------------------------------------------------- The answers.

/// What a person said of a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Ok,
    Fail,
    Skip,
}

impl Verdict {
    fn name(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Fail => "fail",
            Verdict::Skip => "skip",
        }
    }

    fn parse(name: &str) -> Option<Verdict> {
        match name {
            "ok" => Some(Verdict::Ok),
            "fail" => Some(Verdict::Fail),
            "skip" => Some(Verdict::Skip),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub verdict: Verdict,
    /// What went wrong, for a failed step; empty otherwise.
    pub note: String,
}

impl Answer {
    pub fn new(verdict: Verdict) -> Self {
        Answer {
            verdict,
            note: String::new(),
        }
    }

    pub fn failed(note: impl Into<String>) -> Self {
        Answer {
            verdict: Verdict::Fail,
            note: note.into(),
        }
    }
}

/// What the data folder holds of a list, made safe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    pub answers: BTreeMap<String, Answer>,
    /// Steps that failed last time, with the note, now to be tested again.
    pub redo: BTreeMap<String, String>,
    /// When the result went to the agent, in ms since the epoch.
    pub sent_at: Option<i64>,
}

/// What the data folder held, made safe: only known keys, known statuses,
/// bounded notes. Anything that does not read is nothing saved.
pub fn read_saved(text: &str, keys: &[String]) -> Saved {
    let mut saved = Saved::default();
    let source = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Ok(Value::Object(data)) = serde_json::from_str::<Value>(source) else {
        return saved;
    };
    let known: std::collections::HashSet<&str> = keys.iter().map(String::as_str).collect();
    let note = |value: Option<&Value>| match value {
        Some(Value::String(text)) => clip_note(text),
        _ => String::new(),
    };
    if let Some(Value::Object(answers)) = data.get("answers") {
        for (key, answer) in answers {
            let Some(verdict) = answer
                .get("status")
                .and_then(Value::as_str)
                .and_then(Verdict::parse)
            else {
                continue;
            };
            if !known.contains(key.as_str()) {
                continue;
            }
            let note = if verdict == Verdict::Ok {
                String::new()
            } else {
                note(answer.get("note"))
            };
            saved.answers.insert(key.clone(), Answer { verdict, note });
        }
    }
    if let Some(Value::Object(redo)) = data.get("redo") {
        for (key, value) in redo {
            if known.contains(key.as_str()) && !saved.answers.contains_key(key) {
                saved.redo.insert(key.clone(), note(Some(value)));
            }
        }
    }
    saved.sent_at = match data.get("sentAt") {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().filter(|n| n.is_finite()).map(|n| n as i64)),
        _ => None,
    };
    saved
}

/// The file, as the page writes it: `{ v: 1, answers, redo, sentAt }`, one
/// space of indent.
pub fn write_saved(saved: &Saved) -> String {
    #[derive(Serialize)]
    struct AnswerFile<'a> {
        status: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<&'a str>,
    }
    #[derive(Serialize)]
    struct File<'a> {
        v: u8,
        answers: BTreeMap<&'a str, AnswerFile<'a>>,
        redo: &'a BTreeMap<String, String>,
        #[serde(rename = "sentAt")]
        sent_at: Option<i64>,
    }
    let file = File {
        v: 1,
        answers: saved
            .answers
            .iter()
            .map(|(key, answer)| {
                (
                    key.as_str(),
                    AnswerFile {
                        status: answer.verdict.name(),
                        note: (answer.verdict == Verdict::Fail).then_some(answer.note.as_str()),
                    },
                )
            })
            .collect(),
        redo: &saved.redo,
        sent_at: saved.sent_at,
    };
    let mut out = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut serializer = serde_json::Serializer::with_formatter(&mut out, formatter);
    if file.serialize(&mut serializer).is_err() {
        return String::new();
    }
    String::from_utf8(out).unwrap_or_default()
}

/// After the answers went to the agent, reopening the list is testing its
/// fix: what failed goes back to « to redo », its note kept to remind. True
/// when it changed anything.
pub fn mark_failed_for_redo(saved: &mut Saved) -> bool {
    if saved.sent_at.is_none() {
        return false;
    }
    let failed: Vec<String> = saved
        .answers
        .iter()
        .filter(|(_, answer)| answer.verdict == Verdict::Fail)
        .map(|(key, _)| key.clone())
        .collect();
    for key in failed {
        if let Some(answer) = saved.answers.remove(&key) {
            saved.redo.insert(key, answer.note);
        }
    }
    saved.sent_at = None;
    true
}

/// One step's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Ok,
    Fail,
    Skip,
    /// Failed last time and not answered since.
    Redo,
    Todo,
}

pub fn state_of(saved: &Saved, key: &str) -> StepState {
    match saved.answers.get(key) {
        Some(answer) => match answer.verdict {
            Verdict::Ok => StepState::Ok,
            Verdict::Fail => StepState::Fail,
            Verdict::Skip => StepState::Skip,
        },
        None if saved.redo.contains_key(key) => StepState::Redo,
        None => StepState::Todo,
    }
}

// ----------------------------------------------------------------- Progress.

/// How the list stands as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overall {
    Running,
    Failure,
    Success,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub total: usize,
    pub ok: usize,
    pub fail: usize,
    pub skip: usize,
    pub redo: usize,
    pub todo: usize,
    pub answered: usize,
    /// Redo and todo.
    pub pending: usize,
    pub done: bool,
    pub state: Overall,
}

pub fn progress(keys: &[String], saved: &Saved) -> Counts {
    let mut counts = Counts {
        total: keys.len(),
        ok: 0,
        fail: 0,
        skip: 0,
        redo: 0,
        todo: 0,
        answered: 0,
        pending: 0,
        done: false,
        state: Overall::Running,
    };
    for key in keys {
        match state_of(saved, key) {
            StepState::Ok => counts.ok += 1,
            StepState::Fail => counts.fail += 1,
            StepState::Skip => counts.skip += 1,
            StepState::Redo => counts.redo += 1,
            StepState::Todo => counts.todo += 1,
        }
    }
    counts.answered = counts.ok + counts.fail + counts.skip;
    counts.pending = counts.redo + counts.todo;
    counts.done = counts.pending == 0;
    counts.state = if !counts.done {
        Overall::Running
    } else if counts.fail > 0 {
        Overall::Failure
    } else {
        Overall::Success
    };
    counts
}

/// The tab's badge: running with a count, then success or failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Badge {
    pub state: Overall,
    /// 12 characters at most.
    pub text: String,
}

/// `failed` words what a failure is called, given how many.
pub fn badge_for(counts: &Counts, failed: impl Fn(usize) -> String) -> Badge {
    match counts.state {
        Overall::Failure => Badge {
            state: Overall::Failure,
            text: js_prefix(&failed(counts.fail), BADGE_TEXT_LIMIT).to_owned(),
        },
        state => Badge {
            state,
            text: format!("{}/{}", counts.answered, counts.total),
        },
    }
}

/// A name for the tab strip: one line, 40 characters at most.
pub fn tab_title(list: &List) -> String {
    let mut line = String::new();
    let mut in_run = false;
    for c in list.title.chars() {
        if is_js_space(c) {
            if !in_run {
                line.push(' ');
            }
            in_run = true;
        } else {
            line.push(c);
            in_run = false;
        }
    }
    line.chars().take(TITLE_LIMIT).collect()
}

// ----------------------------------------------------------------- What the agent receives.

/// What the agent receives: plain text, in English whatever the person reads,
/// since an agent handles it as it does any message. Failed steps with their
/// notes first, then skipped and unanswered ones, and what to do next.
pub fn build_summary(list: &List, keys: &[String], saved: &Saved, path: &str) -> String {
    let counts = progress(keys, saved);
    let mut lines = vec![format!(
        "Checklist results: \"{}\" ({path}).",
        one_line(&list.title)
    )];
    let mut parts = vec![
        format!("{} OK", counts.ok),
        format!("{} failed", counts.fail),
        format!("{} skipped", counts.skip),
    ];
    if counts.pending > 0 {
        parts.push(format!("{} not answered", counts.pending));
    }
    lines.push(format!("{} steps: {}.", list.steps.len(), parts.join(", ")));

    let mut section = |title: &str, wanted: StepState, with_expect: bool| {
        let mut rows = Vec::new();
        for (index, step) in list.steps.iter().enumerate() {
            let key = &keys[index];
            if state_of(saved, key) != wanted {
                continue;
            }
            let mut row = vec![format!(
                "- Step {}: {}",
                index + 1,
                clip(&one_line(&step.text), 300)
            )];
            if !step.expect.is_empty() && with_expect {
                row.push(format!(
                    "  Expected: {}",
                    clip(&one_line(&step.expect), 300)
                ));
            }
            let note = match wanted {
                StepState::Fail => saved.answers.get(key).map(|a| a.note.as_str()),
                StepState::Redo => saved.redo.get(key).map(String::as_str),
                _ => None,
            }
            .unwrap_or("");
            if wanted == StepState::Fail {
                row.push(format!(
                    "  Note: {}",
                    if note.is_empty() {
                        "(none)".to_owned()
                    } else {
                        clip(js_trim(note), 1000)
                    }
                ));
            } else if !note.is_empty() {
                row.push(format!("  Failed before: {}", clip(js_trim(note), 300)));
            }
            rows.push(row.join("\n"));
        }
        if !rows.is_empty() {
            lines.push(String::new());
            lines.push(format!("{title}:"));
            lines.extend(rows);
        }
    };
    section("FAILED", StepState::Fail, true);
    section("SKIPPED", StepState::Skip, false);
    section("NOT ANSWERED", StepState::Todo, false);
    section(
        "NOT ANSWERED (failed before, to redo)",
        StepState::Redo,
        false,
    );

    let passed: Vec<String> = keys
        .iter()
        .enumerate()
        .filter(|(_, key)| state_of(saved, key) == StepState::Ok)
        .map(|(index, _)| (index + 1).to_string())
        .collect();
    if !passed.is_empty() {
        lines.push(String::new());
        lines.push(format!("Passed: steps {}.", passed.join(", ")));
    }

    lines.push(String::new());
    lines.push(if counts.fail > 0 {
        format!(
            "Fix what failed, then ask for the same list again with kit_tab_open (path \"{path}\"): the failed steps come back to redo and you get a new summary."
        )
    } else if counts.pending > 0 {
        "Nothing failed in what was answered.".to_owned()
    } else {
        "Nothing failed.".to_owned()
    });
    clip(&lines.join("\n"), SUMMARY_LIMIT)
}

/// The refusals of `panes.send` the page has words for; anything else is `error`.
pub fn send_error_code(code: &str) -> &'static str {
    match code {
        "denied" => "denied",
        "not_found" => "not_found",
        "unavailable" => "unavailable",
        "rate_limited" => "rate_limited",
        _ => "error",
    }
}
