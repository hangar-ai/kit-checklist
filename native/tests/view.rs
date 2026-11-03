//! `test/view.test.js`, case for case: what the page draws, from the view the
//! tab hands Hangar (widgets, not a DOM), both languages, every state
//! designed, and the presses that reach the behaviour. Nothing a list or an
//! answer says is more than text, here by construction: a widget holds a
//! string, and no string is a widget.

mod common;

use serde_json::json;

use common::{LIST_PATH, Setup, Sim};
use hangar_kit_sdk::ui::{ButtonKind, Element, EmptyKind, Notice, ProgressKind, Status};
use kit_checklist_tab::Sending;
use kit_checklist_tab::logic::Verdict;

fn walk<T>(sim: &Sim, mut pick: impl FnMut(&Element) -> Option<T>) -> Vec<T> {
    let mut found = Vec::new();
    sim.tab.view().root.for_each(&mut |element| {
        if let Some(item) = pick(element) {
            found.push(item);
        }
    });
    found
}

/// The titles of the headers.
fn headers(sim: &Sim) -> Vec<String> {
    walk(sim, |e| match e {
        Element::Header(h) => Some(h.title.clone()),
        _ => None,
    })
}

/// The words and glyphs of the states ("0 of 3 answered").
fn states(sim: &Sim) -> Vec<(Status, String)> {
    walk(sim, |e| match e {
        Element::State(s) => Some((s.status, s.label.clone())),
        _ => None,
    })
}

/// (label, pressed) of every choice, in the page's order.
fn filters(sim: &Sim) -> Vec<(String, bool)> {
    walk(sim, |e| match e {
        Element::Filter(f) => Some((f.label.clone(), f.pressed)),
        _ => None,
    })
}

/// (label, kind, takes a press) of every button.
fn buttons(sim: &Sim) -> Vec<(String, ButtonKind, bool)> {
    walk(sim, |e| match e {
        Element::Button(b) => Some((
            b.label.clone().unwrap_or_default(),
            b.kind,
            b.on_press.is_some(),
        )),
        _ => None,
    })
}

fn button(sim: &Sim, label: &str) -> Option<(ButtonKind, bool)> {
    buttons(sim)
        .into_iter()
        .find(|(name, ..)| name == label)
        .map(|(_, kind, enabled)| (kind, enabled))
}

/// (kind, text) of every banner.
fn banners(sim: &Sim) -> Vec<(Notice, String)> {
    walk(sim, |e| match e {
        Element::Banner(b) => Some((b.kind, b.text.clone())),
        _ => None,
    })
}

/// The plain texts of the page (not those a widget holds as its label).
fn plain(sim: &Sim) -> Vec<String> {
    walk(sim, |e| match e {
        Element::Text(t) => Some(t.text.clone()),
        _ => None,
    })
}

/// (title, sub, body widgets) of every row of the steps.
fn steps(sim: &Sim) -> Vec<(String, Option<String>, usize)> {
    walk(sim, |e| match e {
        Element::List(l) => Some(
            l.items
                .iter()
                .map(|i| (i.title.clone(), i.sub.clone(), i.body.len()))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    })
    .into_iter()
    .flatten()
    .collect()
}

fn note_fields(sim: &Sim) -> Vec<(String, String)> {
    walk(sim, |e| match e {
        Element::TextArea(a) => Some((a.id.clone(), a.value.clone())),
        _ => None,
    })
}

/// The one `empty` of a page that is a state: (kind, title, text, its buttons).
fn state_page(sim: &Sim) -> (EmptyKind, String, String, Vec<String>) {
    let mut found = walk(sim, |e| match e {
        Element::Empty(s) => Some((
            s.kind,
            s.title.clone(),
            s.text.clone().unwrap_or_default(),
            s.actions
                .iter()
                .filter_map(|a| match a {
                    Element::Button(b) => b.label.clone(),
                    _ => None,
                })
                .collect(),
        )),
        _ => None,
    });
    assert_eq!(found.len(), 1, "one state, drawn");
    found.remove(0)
}

#[test]
fn a_list_draws_its_title_a_progress_bar_each_step_with_three_choices() {
    let sim = Setup::new().open();
    assert_eq!(headers(&sim), ["Login and logout"]);
    assert_eq!(states(&sim)[0], (Status::Running, "0 of 3 answered".into()));
    let bars = walk(&sim, |e| match e {
        Element::Progress(p) => Some((p.value, p.kind)),
        _ => None,
    });
    assert_eq!(bars, [(Some(0.0), ProgressKind::Normal)]);
    let rows = steps(&sim);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].0, "1. Open /login and enter a wrong password.");
    assert_eq!(
        rows[0].1.as_deref(),
        Some("Expected: An error shows under the field.")
    );
    let choices = filters(&sim);
    assert_eq!(choices.len(), 9);
    assert!(choices.iter().all(|(_, pressed)| !pressed));
    assert_eq!(
        choices[..3]
            .iter()
            .map(|(label, _)| label.as_str())
            .collect::<Vec<_>>(),
        ["OK", "Problem", "Skip"]
    );
    // One section holds them, with their count.
    let counts = walk(&sim, |e| match e {
        Element::Section(s) => Some((s.title.clone(), s.count.clone())),
        _ => None,
    });
    assert_eq!(counts, [("Steps".to_owned(), Some("3".to_owned()))]);
}

#[test]
fn the_page_is_a_header_a_bar_a_part_that_scrolls_and_a_foot() {
    let sim = Setup::new().open();
    let Element::Page(page) = &sim.tab.view().root else {
        panic!("a page");
    };
    assert!(page.fill, "a page that does not scroll as a whole");
    assert!(matches!(page.children[1], Element::Scroll(_)));
    assert!(matches!(&page.children[2], Element::Bar(bar) if bar.foot));
}

#[test]
fn pressing_a_choice_answers_a_second_press_clears_it_problem_opens_a_note() {
    let mut sim = Setup::new().open();
    sim.press_choice(0, "OK");
    assert_eq!(sim.kit().counts().unwrap().ok, 1);
    assert_eq!(
        filters(&sim).iter().filter(|(_, pressed)| *pressed).count(),
        1
    );
    sim.press_choice(0, "OK");
    assert_eq!(
        sim.kit().counts().unwrap().ok,
        0,
        "pressing OK again clears it"
    );
    assert!(filters(&sim).iter().all(|(_, pressed)| !pressed));
    sim.press_choice(1, "Problem");
    assert_eq!(
        note_fields(&sim),
        [("note-1".to_owned(), String::new())],
        "a failed step asks what went wrong"
    );
    assert_eq!(sim.kit().counts().unwrap().fail, 1);
    sim.press_choice(1, "Problem");
    assert!(
        note_fields(&sim).is_empty(),
        "and clearing it takes the field away"
    );
}

#[test]
fn problem_puts_the_cursor_in_its_note() {
    let mut sim = Setup::new().open();
    sim.tab.take_operations();
    sim.press_choice(1, "Problem");
    let operations = sim.tab.take_operations();
    assert_eq!(operations.len(), 1);
    assert_eq!(
        serde_json::to_value(&operations[0]).unwrap(),
        json!({ "type": "focus", "id": "note-1" })
    );
    // OK and Skip have no field to go to.
    sim.press_choice(0, "OK");
    assert!(sim.tab.take_operations().is_empty());
}

#[test]
fn the_note_of_a_failed_step_is_what_the_agent_will_read() {
    let mut sim = Setup::new().open();
    sim.press_choice(1, "Problem");
    sim.note(1, "Blank page.");
    assert_eq!(
        note_fields(&sim),
        [("note-1".to_owned(), "Blank page.".to_owned())],
        "the field shows what was typed"
    );
    sim.press("Send to agent");
    assert!(sim.sent()[0].contains("Note: Blank page."));
}

#[test]
fn the_note_has_its_label_and_its_hint_in_both_languages() {
    let mut sim = Setup::new().open();
    sim.press_choice(0, "Problem");
    let areas = walk(&sim, |e| match e {
        Element::TextArea(a) => Some((a.label.clone(), a.placeholder.clone())),
        _ => None,
    });
    assert_eq!(
        areas,
        [(
            Some("What went wrong".to_owned()),
            "Describe what you saw instead".to_owned()
        )]
    );
    let mut fr = Setup::new().fr().open();
    fr.press_choice(0, "Problème");
    let areas = walk(&fr, |e| match e {
        Element::TextArea(a) => Some((a.label.clone(), a.placeholder.clone())),
        _ => None,
    });
    assert_eq!(
        areas,
        [(
            Some("Ce qui ne va pas".to_owned()),
            "Décrivez ce que vous avez vu à la place".to_owned()
        )]
    );
}

#[test]
fn the_send_button_is_for_the_agent_that_asked_primary_once_everything_is_answered() {
    let mut sim = Setup::new().open();
    assert_eq!(
        button(&sim, "Send to agent"),
        Some((ButtonKind::Default, false))
    );
    assert!(plain(&sim).contains(&"Answer at least one step to send the result.".to_owned()));
    sim.answer(0, Verdict::Ok);
    assert_eq!(
        button(&sim, "Send to agent"),
        Some((ButtonKind::Default, true))
    );
    assert!(
        plain(&sim).contains(&"2 steps are still unanswered: the agent will be told.".to_owned())
    );
    sim.answer(1, Verdict::Ok);
    assert!(
        plain(&sim).contains(&"1 step is still unanswered: the agent will be told.".to_owned())
    );
    sim.answer(2, Verdict::Ok);
    assert_eq!(
        button(&sim, "Send to agent"),
        Some((ButtonKind::Primary, true))
    );
    assert!(
        plain(&sim)
            .iter()
            .all(|text| !text.contains("unanswered") && !text.contains("Answer at least")),
        "nothing left to say"
    );
    sim.press("Send to agent");
    assert_eq!(
        banners(&sim),
        [(
            Notice::Info,
            "Sent to Ava. It fixes what failed: open this list again to test it.".to_owned()
        )]
    );
    assert_eq!(
        button(&sim, "Send again"),
        Some((ButtonKind::Default, true))
    );
    assert!(button(&sim, "Send to agent").is_none());
}

#[test]
fn sending_shows_that_it_is_sending() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.tab.press("Send to agent"); // the answer is held back
    assert_eq!(sim.kit().sending(), &Sending::Sending("Ava".into()));
    assert_eq!(button(&sim, "Sending…"), Some((ButtonKind::Default, false)));
    sim.settle();
    assert_eq!(sim.kit().sending(), &Sending::Sent("Ava".into()));
}

#[test]
fn a_refused_send_is_an_alert_with_what_to_do() {
    let mut sim = Setup::new().fail("panes.send", "not_found").open();
    sim.answer(0, Verdict::Ok);
    sim.press("Send to agent");
    let alerts = banners(&sim);
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].0, Notice::Error);
    assert!(
        alerts[0].1.contains("no longer in this workspace"),
        "{}",
        alerts[0].1
    );
    assert_eq!(
        button(&sim, "Send to agent"),
        Some((ButtonKind::Default, true)),
        "and it can be tried again"
    );
}

#[test]
fn opened_by_the_person_there_is_no_agent_to_send_to_the_summary_is_copied_instead() {
    let mut sim = Setup::new().no_caller().open();
    assert!(button(&sim, "Send to agent").is_none());
    assert!(
        plain(&sim).contains(
            &"No agent asked for this list. Copy the summary to give it to one.".to_owned()
        )
    );
    assert_eq!(
        button(&sim, "Copy the summary"),
        Some((ButtonKind::Primary, false))
    );
    sim.answer(0, Verdict::Fail);
    assert_eq!(
        button(&sim, "Copy the summary"),
        Some((ButtonKind::Primary, true))
    );
    sim.press("Copy the summary");
    let copies = sim.host.calls_to("ui.copy");
    assert_eq!(copies.len(), 1);
    assert!(copies[0]["text"].as_str().unwrap().contains("FAILED"));
    let toasts = sim.host.calls_to("ui.toast");
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0]["message"], "Copied");
    assert_eq!(toasts[0]["kind"], "success");
}

#[test]
fn a_refused_copy_says_so_in_a_toast() {
    let mut sim = Setup::new().no_caller().fail("ui.copy", "denied").open();
    sim.answer(0, Verdict::Ok);
    sim.press("Copy the summary");
    let toasts = sim.host.calls_to("ui.toast");
    assert_eq!(toasts.len(), 1);
    assert_eq!(toasts[0]["kind"], "error");
}

#[test]
fn a_step_to_redo_shows_what_failed_last_time() {
    let mut sim = Setup::new().open();
    sim.answer(1, Verdict::Fail);
    sim.note(1, "Blank page.");
    sim.press("Send to agent");
    sim.ask_again(Some(LIST_PATH), Some("Ava"));
    let waiting: Vec<_> = states(&sim)
        .into_iter()
        .filter(|(status, _)| *status == Status::Waiting)
        .collect();
    assert_eq!(waiting, [(Status::Waiting, "To redo".to_owned())]);
    assert!(plain(&sim).contains(&"It failed last time: Blank page.".to_owned()));

    // Without a note, it says to test it again.
    let mut bare = Setup::new().open();
    bare.answer(1, Verdict::Fail);
    bare.press("Send to agent");
    bare.ask_again(Some(LIST_PATH), Some("Ava"));
    assert!(plain(&bare).contains(&"It failed last time. Test it again.".to_owned()));
}

#[test]
fn the_header_says_a_failure_a_clean_list_and_a_list_with_nothing_failed_but_skips() {
    let mut sim = Setup::new().open();
    for i in 0..3 {
        sim.answer(i, if i == 1 { Verdict::Fail } else { Verdict::Ok });
    }
    assert_eq!(states(&sim)[0], (Status::Failure, "1 step failed".into()));
    assert_eq!(
        walk(&sim, |e| match e {
            Element::Progress(p) => Some(p.kind),
            _ => None,
        }),
        [ProgressKind::Failure]
    );
    sim.answer(1, Verdict::Ok); // changes its mind: Problem -> OK
    assert_eq!(
        states(&sim)[0],
        (Status::Success, "Everything passed".into())
    );
    assert_eq!(
        walk(&sim, |e| match e {
            Element::Progress(p) => Some((p.value, p.kind)),
            _ => None,
        }),
        [(Some(1.0), ProgressKind::Success)]
    );
    sim.answer(2, Verdict::Skip);
    assert_eq!(states(&sim)[0], (Status::Success, "Nothing failed".into()));
}

#[test]
fn a_loading_list_says_so() {
    let sim = Setup::new().start();
    let (kind, title, ..) = state_page(&sim);
    assert_eq!(
        (kind, title.as_str()),
        (EmptyKind::Loading, "Reading the list…")
    );
}

#[test]
fn every_refusal_is_an_empty_state_that_says_what_is_wrong_and_what_to_do() {
    let retry = vec!["Try again".to_owned()];
    let cases = [
        (
            Setup::new().no_params(),
            EmptyKind::Empty,
            "No list to show",
            false,
        ),
        (
            Setup::new().path("../x"),
            EmptyKind::Error,
            "This path cannot be used",
            false,
        ),
        (
            Setup::new().path("nope.json"),
            EmptyKind::Error,
            "List not found",
            true,
        ),
        (
            Setup::new().path("a.json").file("a.json", "{x"),
            EmptyKind::Error,
            "The list is not valid JSON",
            true,
        ),
        (
            Setup::new()
                .path("a.json")
                .file("a.json", r#"{"steps":[{"text":"ok"},{}]}"#),
            EmptyKind::Error,
            "The list is not in the expected shape",
            true,
        ),
        (
            Setup::new()
                .path("a.json")
                .file("a.json", "[]")
                .deny(&["files.read"]),
            EmptyKind::Denied,
            "This tab may not read files",
            true,
        ),
        (
            Setup::new().path("a.json").file("a.json", "  "),
            EmptyKind::Error,
            "The list is empty",
            true,
        ),
    ];
    for (setup, kind, title, can_retry) in cases {
        let sim = setup.open();
        let (got_kind, got_title, text, actions) = state_page(&sim);
        assert_eq!((got_kind, got_title.as_str()), (kind, title));
        assert!(!text.is_empty(), "{title} says what to do");
        assert_eq!(
            actions,
            if can_retry { retry.clone() } else { Vec::new() },
            "{title}"
        );
        if can_retry {
            assert_eq!(button(&sim, "Try again"), Some((ButtonKind::Primary, true)));
        }
    }
    let shape = Setup::new()
        .path("a.json")
        .file("a.json", r#"{"steps":[{"text":"ok"},{}]}"#)
        .open();
    assert!(state_page(&shape).2.contains("Step 2 has no text."));
    let missing = Setup::new().path("nope.json").open();
    assert!(
        state_page(&missing)
            .2
            .starts_with("nope.json does not exist in the workspace folder."),
        "{}",
        state_page(&missing).2
    );
}

#[test]
fn saving_denied_is_a_warning_above_the_list_and_the_list_still_works() {
    let mut sim = Setup::new().deny(&["data.read", "data.write"]).open();
    assert_eq!(
        banners(&sim),
        [(
            Notice::Warning,
            "Answers cannot be saved here: closing this tab loses them.".to_owned()
        )]
    );
    sim.answer(0, Verdict::Ok);
    assert_eq!(sim.kit().counts().unwrap().ok, 1);
    // A list whose answers are kept says nothing of the kind.
    assert!(banners(&Setup::new().open()).is_empty());
}

#[test]
fn french_with_its_accents_and_the_longer_words() {
    let sim = Setup::new().fr().open();
    assert_eq!(states(&sim)[0].1, "0 sur 3 traitées");
    let labels: Vec<String> = filters(&sim).into_iter().map(|(label, _)| label).collect();
    assert!(labels.contains(&"Problème".to_owned()) && labels.contains(&"Passer".to_owned()));
    assert_eq!(
        steps(&sim)[0].1.as_deref(),
        // Hangar's typography: a narrow no-break space before the colon.
        Some("Attendu\u{202f}: An error shows under the field.")
    );
    assert!(
        plain(&sim).contains(&"Répondez à au moins une étape pour envoyer le résultat.".to_owned())
    );
    assert!(button(&sim, "Envoyer à l'agent").is_some());
    let missing = Setup::new().fr().path("nope.json").open();
    assert_eq!(state_page(&missing).1, "Liste introuvable");
}

#[test]
fn the_language_follows_hangars_without_a_restart() {
    let mut sim = Setup::new().open();
    assert!(button(&sim, "Send to agent").is_some());
    sim.tab
        .event("theme", json!({ "scheme": "dark", "locale": "fr" }));
    assert!(button(&sim, "Envoyer à l'agent").is_some());
    assert_eq!(states(&sim)[0].1, "0 sur 3 traitées");
    // What the agent receives stays English.
    sim.answer(0, Verdict::Ok);
    sim.press("Envoyer à l'agent");
    assert!(sim.sent()[0].starts_with("Checklist results:"));
}

#[test]
fn nothing_from_the_list_or_an_answer_is_ever_more_than_text() {
    let hostile = json!({
        "title": "<img src=x onerror=alert(1)>",
        "steps": [{ "text": "<b>bold</b>", "expect": "<script>x</script>" }]
    });
    let mut sim = Setup::new().file(LIST_PATH, hostile.to_string()).open();
    assert_eq!(headers(&sim), ["<img src=x onerror=alert(1)>"]);
    let rows = steps(&sim);
    assert_eq!(rows[0].0, "1. <b>bold</b>");
    assert_eq!(rows[0].1.as_deref(), Some("Expected: <script>x</script>"));
    sim.answer(0, Verdict::Fail);
    sim.note(0, "<img src=y>");
    assert_eq!(
        note_fields(&sim),
        [("note-0".to_owned(), "<img src=y>".to_owned())]
    );
    // The page is the same widgets as for a good list: the one step's row.
    let kinds = walk(&sim, |e| match e {
        Element::Text(_) | Element::Spans(_) | Element::Link(_) | Element::Image(_) => Some(()),
        _ => None,
    });
    assert!(
        kinds.len() <= 1,
        "no text becomes a link or an image: {}",
        kinds.len()
    );
}

#[test]
fn a_press_on_a_page_drawn_before_the_last_one_still_lands() {
    // Hangar may draw a new view between a click and its delivery.
    let mut sim = Setup::new().open();
    let old = sim.tab.view().seq;
    let handler = walk(&sim, |e| match e {
        Element::Filter(f) if f.label == "OK" => f.on_press,
        _ => None,
    })[0];
    sim.answer(1, Verdict::Skip); // a new view
    assert_ne!(sim.tab.view().seq, old);
    sim.tab.ui_from(old, handler, None);
    sim.settle();
    assert_eq!(sim.kit().counts().unwrap().ok, 1);
    assert_eq!(sim.kit().counts().unwrap().skip, 1);
}

#[test]
fn the_largest_list_is_a_view_hangar_takes_and_a_summary_panes_send_takes() {
    use kit_checklist_tab::logic::{self, Answer, Saved};
    // 200 steps of 1 000 characters and as many to expect, every one failed
    // with a note of 2 000: the most a list and its answers can be.
    let steps: Vec<serde_json::Value> = (0..200)
        .map(|i| json!({ "text": format!("{i:03} {}", "x".repeat(996)), "expect": "y".repeat(1000) }))
        .collect();
    let text = json!({ "title": "Everything", "steps": steps }).to_string();
    let parsed = logic::parse_list(&text, "big.json").unwrap();
    let keys = logic::step_keys(&parsed.steps);
    let saved = Saved {
        answers: keys
            .iter()
            .map(|key| (key.clone(), Answer::failed("n".repeat(2000))))
            .collect(),
        ..Saved::default()
    };
    let data = std::collections::BTreeMap::from([(
        logic::data_path("big.json"),
        logic::write_saved(&saved),
    )]);
    let mut sim = Setup::new()
        .path("big.json")
        .file("big.json", text)
        .data(data)
        .open();
    assert_eq!(sim.kit().counts().unwrap().fail, 200);
    let size = serde_json::to_string(sim.tab.view()).unwrap().len();
    assert!(
        size < 4 * 1024 * 1024,
        "a view is 4 MB at most: {size} bytes"
    );
    // And every keystroke in a note draws it again, as quickly.
    let started = std::time::Instant::now();
    sim.note(199, "typing");
    assert!(
        started.elapsed().as_millis() < 250,
        "an update and its view take 250 ms at most, even in a debug build: {:?}",
        started.elapsed()
    );
    sim.press("Send to agent");
    let sent = &sim.sent()[0];
    assert!(sent.chars().count() <= 20_000, "{}", sent.chars().count());
    println!("largest view: {size} bytes");
}
