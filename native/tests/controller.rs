//! The tab's behaviour against a pretend Hangar: reading the list, keeping
//! answers within data.write's rate, the badge, sending, and reopening to test
//! a fix. Time is `Sim::advance`; the last write before a tab goes away is the
//! tab going behind another, the one thing a native tab hears before it is
//! put away.

mod common;

use std::collections::BTreeMap;

use serde_json::{Value, json};

use common::{LIST_PATH, Setup, keys, list};
use kit_checklist_tab::logic::{self, Kind, StepState, Verdict};
use kit_checklist_tab::{SAVE_DELAY_MS, SAVE_SPACING_MS, Sending};

fn badge(state: Option<&str>, text: Option<&str>) -> Value {
    match (state, text) {
        (Some(state), Some(text)) => json!({ "state": state, "text": text }),
        _ => json!({ "state": null }),
    }
}

fn state_of(sim: &common::Sim, index: usize) -> StepState {
    let loaded = sim.kit().loaded().expect("a list is shown");
    logic::state_of(&loaded.saved, &loaded.keys[index])
}

#[test]
fn it_reads_the_list_names_the_tab_after_it_and_starts_with_a_running_badge() {
    let sim = Setup::new().open();
    assert_eq!(sim.kit().view_name(), "list");
    assert_eq!(sim.kit().loaded().unwrap().list.steps.len(), 3);
    assert_eq!(
        sim.host.calls_to("files.read"),
        vec![&json!({ "path": LIST_PATH })]
    );
    assert_eq!(
        sim.host.calls_to("tab.setTitle"),
        vec![&json!({ "title": "Login and logout" })]
    );
    assert_eq!(sim.host.badges(), vec![badge(Some("running"), Some("0/3"))]);
    assert_eq!(sim.kit().caller(), Some("Ava"));
    // The params come with the start: nothing to ask for.
    assert!(sim.host.calls_to("tab.params").is_empty());
}

#[test]
fn each_answer_moves_the_badge_running_then_success() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.answer(1, Verdict::Skip);
    assert_eq!(
        sim.host.badges().last(),
        Some(&badge(Some("running"), Some("2/3")))
    );
    sim.answer(2, Verdict::Ok);
    assert_eq!(
        sim.host.badges().last(),
        Some(&badge(Some("success"), Some("3/3")))
    );
}

#[test]
fn a_failed_step_makes_the_badge_a_failure_once_everything_is_answered_and_unanswering_goes_back() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.answer(1, Verdict::Fail);
    sim.answer(2, Verdict::Ok);
    assert_eq!(
        sim.host.badges().last(),
        Some(&badge(Some("failure"), Some("1 failed")))
    );
    sim.clear(1, Verdict::Fail);
    assert_eq!(
        sim.host.badges().last(),
        Some(&badge(Some("running"), Some("2/3")))
    );
    assert_eq!(state_of(&sim, 1), StepState::Todo);
}

#[test]
fn answers_are_written_after_a_pause_together_and_survive_a_restart() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.answer(1, Verdict::Fail);
    sim.note(1, "The dashboard is blank.");
    assert_eq!(sim.host.calls_to("data.write").len(), 0, "nothing at once");
    sim.advance(SAVE_DELAY_MS);
    assert_eq!(sim.host.calls_to("data.write").len(), 1);
    let written = sim.host.saved(LIST_PATH);
    assert_eq!(
        written["answers"][&keys()[1]]["note"],
        "The dashboard is blank."
    );

    let again = Setup::new().no_caller().data(sim.host.data.clone()).open();
    assert_eq!(state_of(&again, 0), StepState::Ok);
    let loaded = again.kit().loaded().unwrap();
    assert_eq!(
        loaded.saved.answers[&loaded.keys[1]],
        logic::Answer::failed("The dashboard is blank.")
    );
}

#[test]
fn typing_a_note_never_writes_faster_than_data_write_allows() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Fail);
    for second in 0..180 {
        sim.note(0, &format!("note {second}"));
        sim.advance(1000);
    }
    sim.advance(SAVE_SPACING_MS);
    let times = &sim.host.write_times;
    for pair in times.windows(2) {
        assert!(pair[1] - pair[0] >= SAVE_SPACING_MS, "writes are spaced");
    }
    for at in 10..times.len() {
        assert!(times[at] - times[at - 10] >= 60_000, "never 11 in a minute");
    }
    assert!(times.len() >= 10, "but it does keep up: {}", times.len());
    assert_eq!(
        sim.host.saved(LIST_PATH)["answers"][&keys()[0]]["note"],
        "note 179"
    );
}

#[test]
fn a_tab_going_behind_another_writes_at_once() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.visible(false);
    assert_eq!(sim.host.calls_to("data.write").len(), 1);
    sim.visible(false);
    assert_eq!(
        sim.host.calls_to("data.write").len(),
        1,
        "nothing new, nothing written"
    );
    sim.visible(true);
    assert_eq!(sim.host.calls_to("data.write").len(), 1);
    // And the pause that was waiting is gone with it.
    sim.advance(60_000);
    assert_eq!(sim.host.calls_to("data.write").len(), 1);
}

#[test]
fn a_tab_closed_within_the_pause_loses_only_what_the_pause_held() {
    // What a native tab cannot do that the page did: write on `pagehide`.
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.advance(SAVE_DELAY_MS - 1);
    assert!(
        sim.host.data.is_empty(),
        "within the pause, nothing is written"
    );
    sim.advance(1);
    assert!(!sim.host.data.is_empty(), "at its end, it is");
}

#[test]
fn data_denied_the_tab_works_says_answers_are_not_kept_and_stops_trying() {
    let mut sim = Setup::new().deny(&["data.read", "data.write"]).open();
    assert_eq!(sim.kit().view_name(), "list");
    assert!(!sim.kit().persist());
    sim.answer(0, Verdict::Ok);
    sim.advance(SAVE_DELAY_MS + SAVE_SPACING_MS);
    assert_eq!(sim.host.calls_to("data.write").len(), 0);
    assert_eq!(sim.kit().counts().unwrap().ok, 1);

    let mut write_only = Setup::new().deny(&["data.write"]).open();
    write_only.answer(0, Verdict::Ok);
    write_only.advance(SAVE_DELAY_MS);
    assert!(
        !write_only.kit().persist(),
        "a refused write turns saving off, once"
    );
    assert_eq!(write_only.host.calls_to("data.write").len(), 1);
    write_only.answer(1, Verdict::Ok);
    write_only.advance(60_000);
    assert_eq!(
        write_only.host.calls_to("data.write").len(),
        1,
        "and it stops trying"
    );
}

#[test]
fn a_rate_limited_write_is_tried_again_not_lost() {
    let mut sim = Setup::new().write_limit(1).open();
    sim.answer(0, Verdict::Ok);
    sim.advance(SAVE_DELAY_MS);
    sim.answer(1, Verdict::Ok);
    sim.advance(SAVE_SPACING_MS + 1000);
    assert!(sim.kit().persist(), "a limit is not a refusal");
    assert!(
        sim.host.saved(LIST_PATH)["answers"][&keys()[1]].is_null(),
        "not written yet"
    );
    sim.advance(70_000);
    assert_eq!(
        sim.host.saved(LIST_PATH)["answers"][&keys()[1]]["status"],
        "ok",
        "written once the minute is over"
    );
}

#[test]
fn send_gives_the_agent_that_asked_a_plain_summary_and_marks_the_result_as_sent() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.answer(1, Verdict::Fail);
    sim.note(1, "Blank dashboard.");
    sim.answer(2, Verdict::Skip);
    sim.press("Send to agent");
    let calls = sim.host.calls_to("panes.send");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0]["pane"], "Ava");
    let text = calls[0]["text"].as_str().unwrap();
    assert!(
        text.contains(
            "FAILED:\n- Step 2: Sign in with the test account.\n  Expected: The dashboard shows.\n  Note: Blank dashboard."
        ),
        "{text}"
    );
    assert!(text.contains("SKIPPED:\n- Step 3:"));
    assert_eq!(sim.kit().sending(), &Sending::Sent("Ava".into()));
    assert_eq!(sim.host.calls_to("notify").len(), 1);
    assert_eq!(
        sim.host.calls_to("notify")[0],
        &json!({ "title": "Result sent to Ava" }),
        "no body"
    );
    let sent_at = sim.kit().loaded().unwrap().saved.sent_at;
    assert!(sent_at.is_some());
    assert_eq!(
        sim.host.saved(LIST_PATH)["sentAt"].as_i64(),
        sent_at,
        "and that is kept"
    );
}

#[test]
fn an_answer_after_sending_is_a_result_not_yet_sent() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.press("Send to agent");
    assert_eq!(sim.kit().sending(), &Sending::Sent("Ava".into()));
    sim.answer(1, Verdict::Ok);
    assert_eq!(sim.kit().sending(), &Sending::Idle);
    assert_eq!(sim.kit().loaded().unwrap().saved.sent_at, None);
}

#[test]
fn a_note_typed_after_sending_is_a_result_not_yet_sent_either() {
    let mut sim = Setup::new().open();
    sim.answer(1, Verdict::Fail);
    sim.press("Send to agent");
    assert_eq!(sim.kit().sending(), &Sending::Sent("Ava".into()));
    sim.note(1, "more");
    assert_eq!(sim.kit().sending(), &Sending::Idle);
    assert_eq!(sim.kit().loaded().unwrap().saved.sent_at, None);
}

#[test]
fn nothing_to_send_or_nobody_to_send_to_it_does_not_call_panes_send() {
    let mut empty = Setup::new().open();
    // The button is there and disabled: pressing it does nothing.
    assert!(!sends_enabled(&empty));
    assert_eq!(empty.kit().sending(), &Sending::Idle);
    empty.advance(1000);
    assert_eq!(empty.host.calls_to("panes.send").len(), 0);

    let mut person = Setup::new().no_caller().open();
    assert_eq!(person.kit().caller(), None);
    person.answer(0, Verdict::Ok);
    assert_eq!(person.host.calls_to("panes.send").len(), 0);
    assert!(
        person
            .tab
            .texts()
            .iter()
            .all(|text| text != "Send to agent"),
        "there is nothing to send to: the summary is copied"
    );
}

/// Whether the page's Send button takes a press.
fn sends_enabled(sim: &common::Sim) -> bool {
    use hangar_kit_sdk::ui::Element;
    let mut enabled = None;
    sim.tab.view().root.for_each(&mut |element| {
        if let Element::Button(button) = element
            && button.label.as_deref() == Some("Send to agent")
        {
            enabled = Some(button.on_press.is_some());
        }
    });
    enabled.expect("a Send button")
}

#[test]
fn a_refused_send_says_why_keeps_the_answers_and_can_be_tried_again() {
    for code in ["denied", "not_found", "unavailable", "rate_limited"] {
        let mut sim = Setup::new().fail("panes.send", code).open();
        sim.answer(0, Verdict::Fail);
        sim.press("Send to agent");
        match sim.kit().sending() {
            Sending::Failed {
                code: got, pane, ..
            } => {
                assert_eq!((*got, pane.as_str()), (code, "Ava"));
            }
            other => panic!("{code}: {other:?}"),
        }
        assert_eq!(sim.kit().counts().unwrap().fail, 1);
        assert_eq!(
            sim.kit().loaded().unwrap().saved.sent_at,
            None,
            "not sent, not marked"
        );
        let words = kit_checklist_tab::strings::strings("en").t(&format!("send.{code}"));
        assert_ne!(words, format!("send.{code}"));
        assert!(sends_enabled(&sim), "and it can be tried again");
    }
    let mut sim = Setup::new().fail("panes.send", "weird").open();
    sim.answer(0, Verdict::Ok);
    sim.press("Send to agent");
    match sim.kit().sending() {
        Sending::Failed { code, .. } => assert_eq!(*code, "error"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn notify_denied_does_not_undo_a_send() {
    let mut sim = Setup::new().deny(&["notify"]).open();
    sim.answer(0, Verdict::Ok);
    sim.press("Send to agent");
    assert_eq!(sim.kit().sending(), &Sending::Sent("Ava".into()));
}

#[test]
fn asking_for_the_same_list_again_keeps_the_answers_and_marks_the_failed_ones_to_redo() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.answer(1, Verdict::Fail);
    sim.note(1, "Blank dashboard.");
    sim.answer(2, Verdict::Skip);
    sim.press("Send to agent");
    let keys = sim.kit().loaded().unwrap().keys.clone();

    sim.ask_again(Some(LIST_PATH), Some("Ava"));
    assert_eq!(sim.kit().view_name(), "list");
    assert_eq!(state_of(&sim, 0), StepState::Ok, "kept");
    assert_eq!(state_of(&sim, 2), StepState::Skip, "kept");
    assert_eq!(state_of(&sim, 1), StepState::Redo);
    assert_eq!(
        sim.kit().loaded().unwrap().saved.redo[&keys[1]],
        "Blank dashboard."
    );
    assert_eq!(sim.kit().counts().unwrap().state, logic::Overall::Running);
    assert_eq!(
        sim.host.badges().last(),
        Some(&badge(Some("running"), Some("2/3")))
    );
    assert_eq!(sim.kit().sending(), &Sending::Idle);

    sim.answer(1, Verdict::Ok);
    sim.press("Send to agent");
    assert_eq!(sim.sent().len(), 2);
    assert!(sim.sent()[1].contains("Nothing failed"));
    assert_eq!(
        sim.host.badges().last(),
        Some(&badge(Some("success"), Some("3/3")))
    );
}

#[test]
fn a_failure_that_was_not_sent_stays_a_failure_when_the_tab_is_asked_for_again() {
    let mut sim = Setup::new().open();
    sim.answer(1, Verdict::Fail);
    sim.ask_again(Some(LIST_PATH), Some("Ava"));
    assert_eq!(state_of(&sim, 1), StepState::Fail);
}

#[test]
fn the_failed_to_redo_mark_survives_a_restart_too() {
    let mut first = Setup::new().open();
    first.answer(1, Verdict::Fail);
    first.note(1, "bad");
    first.press("Send to agent");
    let mut second = Setup::new().data(first.host.data.clone()).open();
    assert_eq!(state_of(&second, 1), StepState::Redo);
    second.advance(SAVE_DELAY_MS);
    let keys = second.kit().loaded().unwrap().keys.clone();
    assert_eq!(second.host.saved(LIST_PATH)["redo"][&keys[1]], "bad");
}

#[test]
fn the_agent_rewording_a_step_loses_that_steps_answer_and_no_others() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.answer(1, Verdict::Ok);
    sim.visible(false); // written at once
    let mut edited = list();
    edited["steps"][1]["text"] = json!("Sign in with the other account.");
    sim.host.files.insert(LIST_PATH.into(), edited.to_string());
    sim.ask_again(Some(LIST_PATH), None);
    assert_eq!(state_of(&sim, 0), StepState::Ok);
    assert_eq!(state_of(&sim, 1), StepState::Todo);
}

#[test]
fn two_lists_keep_their_answers_apart() {
    let mut sim = Setup::new()
        .path("a.json")
        .file("a.json", list().to_string())
        .file("b.json", list().to_string())
        .no_caller()
        .open();
    sim.answer(0, Verdict::Ok);
    sim.visible(false);
    sim.ask_again(Some("b.json"), None);
    assert_eq!(sim.kit().counts().unwrap().answered, 0);
    sim.ask_again(Some("a.json"), None);
    assert_eq!(
        sim.kit().counts().unwrap().answered,
        1,
        "and a keeps its own"
    );
}

#[test]
fn asking_for_another_list_writes_the_first_ones_answers_and_leaves_no_timer_behind() {
    // The page's `dispose`: nothing of the first list waits once another is read.
    let mut sim = Setup::new()
        .path("a.json")
        .file("a.json", list().to_string())
        .file("b.json", list().to_string())
        .open();
    sim.answer(0, Verdict::Ok);
    assert_eq!(sim.pending_timers(), 1, "the pause is waiting");
    sim.ask_again(Some("b.json"), None);
    assert_eq!(
        sim.host.calls_to("data.write").len(),
        1,
        "a's answers went first"
    );
    assert_eq!(sim.pending_timers(), 0);
    sim.advance(60_000);
    assert_eq!(sim.host.calls_to("data.write").len(), 1);
    assert_eq!(sim.kit().path(), Some("b.json"));
}

#[test]
fn a_missing_malformed_or_unreadable_list_is_said_and_nothing_else_breaks() {
    let cases = [
        (Setup::new().no_params(), Kind::Missing),
        (Setup::new().path("../x.json"), Kind::Path),
        (Setup::new().path("nope.json"), Kind::NotFound),
        (
            Setup::new().path("bad.json").file("bad.json", "{oops"),
            Kind::Json,
        ),
        (
            Setup::new().path("empty.json").file("empty.json", ""),
            Kind::Empty,
        ),
        (
            Setup::new()
                .path("shape.json")
                .file("shape.json", r#"{"steps":[{}]}"#),
            Kind::Shape,
        ),
        (
            Setup::new()
                .path("a.json")
                .file("a.json", "[]")
                .deny(&["files.read"]),
            Kind::Denied,
        ),
    ];
    for (setup, kind) in cases {
        let mut sim = setup.open();
        assert_eq!(sim.kit().view_name(), "error", "{kind:?}");
        assert_eq!(sim.kit().problem().unwrap().kind, kind);
        assert_eq!(sim.host.calls_to("data.write").len(), 0);
        assert_eq!(
            sim.host
                .badges()
                .last()
                .cloned()
                .unwrap_or_else(|| badge(None, None)),
            badge(None, None),
            "no stale badge"
        );
        // No step to answer, nothing to send.
        sim.advance(60_000);
        assert_eq!(sim.host.calls_to("panes.send").len(), 0);
        assert_eq!(sim.host.calls_to("data.write").len(), 0);
        assert!(sim.kit().counts().is_none());
        assert_eq!(sim.kit().summary(), "");
    }
}

#[test]
fn retry_reads_the_list_again_and_a_list_written_meanwhile_shows() {
    let mut sim = Setup::new().no_file(LIST_PATH).open();
    assert_eq!(sim.kit().problem().unwrap().kind, Kind::NotFound);
    sim.host.files.insert(LIST_PATH.into(), list().to_string());
    sim.press("Try again");
    assert_eq!(sim.kit().view_name(), "list");
}

#[test]
fn a_later_request_makes_an_earlier_slow_answer_stale() {
    let b = json!({ "title": "B", "steps": [{ "text": "only" }] });
    let mut sim = Setup::new()
        .path("a.json")
        .file("a.json", list().to_string())
        .file("b.json", b.to_string())
        .no_caller()
        .start(); // a.json's answer is held back
    assert_eq!(sim.kit().view_name(), "loading");
    sim.tab.event(
        "tab.params",
        json!({ "params": { "path": "b.json" }, "caller": null }),
    );
    sim.answer_call("files.read", "b.json");
    sim.settle(); // and now a's, which comes last and is not heard
    assert_eq!(sim.kit().loaded().unwrap().list.title, "B");
    assert_eq!(sim.kit().path(), Some("b.json"));
    assert_eq!(
        sim.host.calls_to("data.read").len(),
        1,
        "a's list was never gone on with"
    );
}

#[test]
fn a_reply_to_a_send_for_a_list_read_since_is_not_heard() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    // The send is out and its answer held back, as a slow agent's would be.
    sim.tab.press("Send to agent");
    sim.tab.event(
        "tab.params",
        json!({ "params": { "path": LIST_PATH }, "caller": { "pane": "Ava" } }),
    );
    sim.settle();
    assert_eq!(
        sim.kit().sending(),
        &Sending::Idle,
        "the new read starts clean"
    );
    assert_eq!(sim.kit().loaded().unwrap().saved.sent_at, None);
}

#[test]
fn a_read_waits_for_the_write_in_flight() {
    let mut sim = Setup::new().start();
    sim.settle();
    sim.answer(0, Verdict::Ok);
    sim.advance(SAVE_DELAY_MS - 1);
    // The write goes out; its answer is held back.
    sim.tab.event("tab.visibility", json!({ "visible": false }));
    sim.tab.event(
        "tab.params",
        json!({ "params": { "path": LIST_PATH }, "caller": null }),
    );
    // Only the write is out: the new read waits for it, so that it reads
    // what the write put there.
    assert_eq!(
        sim.tab.methods(),
        vec!["data.write"],
        "{:?}",
        sim.tab.methods()
    );
    sim.settle();
    assert_eq!(sim.kit().view_name(), "list");
    assert_eq!(state_of(&sim, 0), StepState::Ok, "read after the write");
}

#[test]
fn a_badge_refused_by_hangar_never_breaks_the_tab() {
    let mut sim = Setup::new()
        .fail("tab.setBadge", "denied")
        .fail("tab.setTitle", "invalid")
        .open();
    assert_eq!(sim.kit().view_name(), "list");
    sim.answer(0, Verdict::Ok);
    assert_eq!(sim.kit().counts().unwrap().ok, 1);
}

#[test]
fn the_title_and_the_badge_are_only_sent_when_they_change() {
    let mut sim = Setup::new().open();
    sim.answer(0, Verdict::Ok);
    sim.clear(0, Verdict::Ok);
    sim.answer(0, Verdict::Ok);
    sim.ask_again(Some(LIST_PATH), Some("Ava"));
    assert_eq!(sim.host.calls_to("tab.setTitle").len(), 1);
    let badges = sim.host.badges();
    for pair in badges.windows(2) {
        assert_ne!(pair[0], pair[1], "{badges:?}");
    }
}

#[test]
fn the_answers_of_the_html_tab_are_read_here() {
    // A file as the kit's earlier HTML tab kept it.
    let keys = keys();
    let html_tab_wrote = format!(
        "{{\n \"v\": 1,\n \"answers\": {{\n  \"{}\": {{\n   \"status\": \"ok\"\n  }},\n  \"{}\": {{\n   \"status\": \"fail\",\n   \"note\": \"blank\"\n  }}\n }},\n \"redo\": {{}},\n \"sentAt\": null\n}}",
        keys[0], keys[1]
    );
    let sim = Setup::new()
        .data(BTreeMap::from([(
            "answers/recette_login.json-7b4e567b.json".to_owned(),
            html_tab_wrote,
        )]))
        .open();
    assert_eq!(state_of(&sim, 0), StepState::Ok);
    assert_eq!(state_of(&sim, 1), StepState::Fail);
    assert_eq!(state_of(&sim, 2), StepState::Todo);
    assert_eq!(
        sim.host.calls_to("data.read"),
        vec![&json!({ "path": "answers/recette_login.json-7b4e567b.json" })]
    );
}
