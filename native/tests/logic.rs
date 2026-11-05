//! The pure part of the tab. The values the kit's earlier HTML tab wrote (the
//! keys, the answers' file, the summary) are held to the byte, so a person's
//! answers survive the change of tab.

mod common;

use std::collections::BTreeMap;

use serde_json::{Value, json};

use common::{LIST_PATH, keys, list, list_steps};
use kit_checklist_tab::logic::{
    self, Answer, Kind, Overall, Saved, Shape, StepState, Unreadable, Verdict,
};
use kit_checklist_tab::strings::{EN, FR, strings};

fn read(text: &str) -> Result<logic::List, Unreadable> {
    logic::parse_list(text, "a.json")
}

fn shape_of(text: &str) -> Shape {
    match read(text) {
        Err(Unreadable::Shape(shape)) => shape,
        other => panic!("expected a shape problem for {text:?}, got {other:?}"),
    }
}

#[test]
fn the_path_param_is_read_like_form_input() {
    assert_eq!(
        logic::read_path(Some("recette/login.json")),
        Ok("recette/login.json".into())
    );
    assert_eq!(
        logic::read_path(Some(" recette\\login.json ")),
        Ok("recette/login.json".into())
    );
    assert_eq!(logic::read_path(None), Err(Kind::Missing));
    assert_eq!(logic::read_path(Some("  ")), Err(Kind::Missing));
    for bad in [
        "../x.json",
        "/etc/passwd",
        "C:/x.json",
        "a/../b.json",
        ".hidden/x.json",
        "a/.x.json",
        "a//b.json",
        "a:b",
        "a\nb",
        "a/",
    ] {
        assert_eq!(logic::read_path(Some(bad)), Err(Kind::Path), "{bad}");
    }
    assert_eq!(logic::read_path(Some(&"a".repeat(1025))), Err(Kind::Path));
    assert!(logic::read_path(Some(&"a".repeat(1024))).is_ok());
}

#[test]
fn a_good_list_is_read_trimmed_and_named_after_its_file_when_it_has_no_title() {
    let parsed = logic::parse_list(&list().to_string(), LIST_PATH).unwrap();
    assert_eq!(parsed.title, "Login and logout");
    assert_eq!(parsed.steps.len(), 3);
    let bare = logic::parse_list(
        &json!({ "steps": [{ "text": "  Do it  " }] }).to_string(),
        "recette/login.json",
    )
    .unwrap();
    assert_eq!(bare.title, "login.json");
    assert_eq!(bare.steps.len(), 1);
    assert_eq!(bare.steps[0].text, "Do it");
    assert_eq!(bare.steps[0].expect, "");
    assert!(
        logic::parse_list(&format!("\u{feff}{}", list()), LIST_PATH).is_ok(),
        "a BOM is dropped"
    );
}

#[test]
fn a_malformed_list_says_what_is_wrong() {
    assert_eq!(read(""), Err(Unreadable::Empty));
    assert_eq!(read("   \n"), Err(Unreadable::Empty));
    match read("{not json") {
        Err(Unreadable::Json(detail)) => assert!(!detail.is_empty()),
        other => panic!("{other:?}"),
    }
    assert_eq!(shape_of("[1]"), Shape::NotObject);
    assert_eq!(shape_of("null"), Shape::NotObject);
    assert_eq!(shape_of(r#"{"steps":[]}"#), Shape::NoSteps);
    assert_eq!(shape_of(r#"{"steps":"x"}"#), Shape::NoSteps);
    assert_eq!(
        shape_of(r#"{"title":3,"steps":[{"text":"a"}]}"#),
        Shape::TitleType
    );
    assert_eq!(
        shape_of(r#"{"title":null,"steps":[{"text":"a"}]}"#),
        Shape::TitleType,
        "a title that is not a text, null too"
    );
    assert_eq!(
        shape_of(r#"{"steps":["a"]}"#),
        Shape::StepNotObject { n: 1 }
    );
    assert_eq!(
        shape_of(r#"{"steps":[{"text":"a"},{"expect":"b"}]}"#),
        Shape::StepText { n: 2 }
    );
    assert_eq!(
        shape_of(r#"{"steps":[{"text":"  "}]}"#),
        Shape::StepText { n: 1 }
    );
    assert_eq!(
        shape_of(r#"{"steps":[{"text":"a","expect":4}]}"#),
        Shape::StepExpect { n: 1 }
    );
    assert_eq!(
        shape_of(&json!({ "steps": [{ "text": "x".repeat(1001) }] }).to_string()),
        Shape::StepLong { n: 1, max: 1000 }
    );
    let many = json!({ "steps": (0..201).map(|_| json!({ "text": "a" })).collect::<Vec<_>>() });
    let tool_many = shape_of(&many.to_string());
    assert_eq!(tool_many, Shape::TooMany { max: 200, n: 201 });
    assert_eq!(
        tool_many.vars(),
        vec![("max", "200".to_owned()), ("n", "201".to_owned())]
    );
    for code in [
        "notObject",
        "noSteps",
        "tooMany",
        "stepNotObject",
        "stepText",
        "stepExpect",
        "stepLong",
        "titleType",
    ] {
        for (name, table) in [("en", EN), ("fr", FR)] {
            assert!(
                table.iter().any(|(key, _)| *key == format!("shape.{code}")),
                "{name} {code}"
            );
        }
    }
    // What the page's JSON.parse would call a list, serde_json does: a number
    // of any size, a string with any escape.
    assert!(read(r#"{"steps":[{"text":"\u00e9\ud83d\ude00","expect":null}]}"#).is_ok());
}

#[test]
fn a_steps_key_follows_its_text_not_its_place() {
    let step = |text: &str| logic::Step {
        text: text.into(),
        expect: String::new(),
    };
    let keys = logic::step_keys(&[step("a"), step("b"), step("a")]);
    assert_eq!(
        keys.iter().collect::<std::collections::HashSet<_>>().len(),
        3,
        "two identical texts are two steps"
    );
    assert_eq!(
        logic::step_keys(&[step("b"), step("a"), step("a")])[0],
        keys[1]
    );
    let with_expect = logic::Step {
        text: "a".into(),
        expect: "other".into(),
    };
    assert_eq!(
        logic::step_keys(&[with_expect])[0],
        keys[0],
        "expect is not part of it"
    );
}

#[test]
fn the_keys_and_paths_are_the_html_tabs_to_the_byte() {
    // Computed by the kit's earlier HTML tab: answers written there are read here.
    assert_eq!(logic::fnv("a"), "e40c292c");
    assert_eq!(logic::fnv(""), "811c9dc5");
    assert_eq!(logic::fnv("é😀 ok"), "a3f6608d", "over UTF-16 code units");
    assert_eq!(keys(), ["e94a8a8d-0", "fbdf1541-0", "5427f3b5-0"]);
    let step = |text: &str| logic::Step {
        text: text.into(),
        expect: String::new(),
    };
    assert_eq!(
        logic::step_keys(&[step("a"), step("b"), step("a")]),
        ["e40c292c-0", "e70c2de5-0", "e40c292c-1"]
    );
    assert_eq!(
        logic::data_path(LIST_PATH),
        "answers/recette_login.json-7b4e567b.json"
    );
    assert_eq!(
        logic::data_path("é/x y.json"),
        "answers/_x_y.json-1f7a143c.json"
    );
    assert_eq!(
        logic::data_path(&("é/".repeat(500) + "x.json")),
        "answers/_x.json-b10de8a9.json"
    );
}

#[test]
fn the_data_path_is_derived_from_the_lists_path_and_is_safe() {
    let a = logic::data_path("recette/login.json");
    assert!(a.starts_with("answers/") && a.ends_with(".json"));
    assert!(
        a["answers/".len()..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    );
    assert_ne!(a, logic::data_path("recette/login_json"));
    assert_ne!(
        logic::data_path("a/b.json"),
        logic::data_path("a_b.json"),
        "a different path never shares answers"
    );
    assert_eq!(a, logic::data_path("recette/login.json"));
    assert!(logic::data_path(&("é/".repeat(500) + "x.json")).len() < 100);
}

#[test]
fn saved_answers_are_read_safely() {
    let keys = keys();
    let text = json!({
        "v": 1,
        "answers": {
            keys[0].clone(): { "status": "ok" },
            keys[1].clone(): { "status": "fail", "note": "boom" },
            "nope": { "status": "ok" },
            keys[2].clone(): { "status": "weird" }
        },
        "redo": { keys[0].clone(): "ignored: answered", keys[2].clone(): "was bad" },
        "sentAt": 12
    })
    .to_string();
    let saved = logic::read_saved(&text, &keys);
    assert_eq!(
        saved.answers,
        BTreeMap::from([
            (keys[0].clone(), Answer::new(Verdict::Ok)),
            (keys[1].clone(), Answer::failed("boom")),
        ])
    );
    assert_eq!(
        saved.redo,
        BTreeMap::from([(keys[2].clone(), "was bad".to_owned())])
    );
    assert_eq!(saved.sent_at, Some(12));
    for junk in ["", "not json", "null", "[]", r#"{"answers":3}"#] {
        assert_eq!(logic::read_saved(junk, &keys), Saved::default(), "{junk}");
    }
    let written: Value = serde_json::from_str(&logic::write_saved(&saved)).unwrap();
    assert_eq!(written["v"], 1);
    // A note is bounded, in UTF-16 units like the page's.
    let long =
        json!({ "answers": { keys[1].clone(): { "status": "fail", "note": "n".repeat(3000) } } });
    let long = logic::read_saved(&long.to_string(), &keys);
    assert_eq!(long.answers[&keys[1]].note.len(), 2000);
}

#[test]
fn the_answers_file_is_written_as_the_page_writes_it() {
    let keys = keys();
    let saved = Saved {
        answers: BTreeMap::from([
            (keys[0].clone(), Answer::new(Verdict::Ok)),
            (keys[1].clone(), Answer::failed("The page stays blank.")),
            (keys[2].clone(), Answer::new(Verdict::Skip)),
        ]),
        redo: BTreeMap::new(),
        sent_at: Some(1_761_933_600_000),
    };
    let text = logic::write_saved(&saved);
    // JSON.stringify(…, null, 1): one space of indent, v, answers, redo, sentAt.
    assert!(
        text.starts_with("{\n \"v\": 1,\n \"answers\": {\n  \""),
        "{text}"
    );
    assert!(text.contains("\n \"redo\": {},\n \"sentAt\": 1761933600000\n}"));
    let written: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(
        written,
        json!({
            "v": 1,
            "answers": {
                "e94a8a8d-0": { "status": "ok" },
                "fbdf1541-0": { "status": "fail", "note": "The page stays blank." },
                "5427f3b5-0": { "status": "skip" }
            },
            "redo": {},
            "sentAt": 1_761_933_600_000_i64
        })
    );
    // And what it writes, it reads.
    assert_eq!(logic::read_saved(&text, &keys), saved);
    assert!(logic::write_saved(&Saved::default()).contains("\"sentAt\": null"));
}

#[test]
fn after_the_result_went_out_the_failed_steps_come_back_to_redo_with_their_note() {
    let keys = keys();
    let mut saved = Saved {
        answers: BTreeMap::from([
            (keys[0].clone(), Answer::new(Verdict::Ok)),
            (keys[1].clone(), Answer::failed("blank page")),
            (keys[2].clone(), Answer::new(Verdict::Skip)),
        ]),
        redo: BTreeMap::new(),
        sent_at: Some(5),
    };
    assert!(logic::mark_failed_for_redo(&mut saved));
    assert_eq!(
        saved,
        Saved {
            answers: BTreeMap::from([
                (keys[0].clone(), Answer::new(Verdict::Ok)),
                (keys[2].clone(), Answer::new(Verdict::Skip)),
            ]),
            redo: BTreeMap::from([(keys[1].clone(), "blank page".to_owned())]),
            sent_at: None,
        }
    );
    assert!(
        !logic::mark_failed_for_redo(&mut saved),
        "nothing to do once done"
    );
    let mut unsent = Saved {
        answers: BTreeMap::from([(keys[1].clone(), Answer::failed("x"))]),
        ..Saved::default()
    };
    assert!(
        !logic::mark_failed_for_redo(&mut unsent),
        "a failure not sent stays a failure"
    );
    assert_eq!(logic::state_of(&unsent, &keys[1]), StepState::Fail);
}

#[test]
fn progress_badge_and_title() {
    let keys = keys();
    let failed = |n: usize| format!("{n} failed");
    let mut saved = Saved::default();
    let badge = logic::badge_for(&logic::progress(&keys, &saved), failed);
    assert_eq!(
        (badge.state, badge.text.as_str()),
        (Overall::Running, "0/3")
    );
    saved
        .answers
        .insert(keys[0].clone(), Answer::new(Verdict::Ok));
    saved.redo.insert(keys[1].clone(), "x".into());
    let mid = logic::progress(&keys, &saved);
    assert_eq!(
        (mid.answered, mid.redo, mid.todo, mid.state),
        (1, 1, 1, Overall::Running)
    );
    saved
        .answers
        .insert(keys[1].clone(), Answer::new(Verdict::Skip));
    saved.redo.remove(&keys[1]);
    saved.answers.insert(keys[2].clone(), Answer::failed(""));
    let done = logic::progress(&keys, &saved);
    assert_eq!(done.state, Overall::Failure);
    let badge = logic::badge_for(&done, failed);
    assert_eq!(
        (badge.state, badge.text.as_str()),
        (Overall::Failure, "1 failed")
    );
    saved
        .answers
        .insert(keys[2].clone(), Answer::new(Verdict::Ok));
    let badge = logic::badge_for(&logic::progress(&keys, &saved), failed);
    assert_eq!(
        (badge.state, badge.text.as_str()),
        (Overall::Success, "3/3")
    );
    let t = strings("fr");
    let french = logic::badge_for(&done, |n| t.tv("badgeFailure", &[("n", &n.to_string())]));
    assert!(french.text.chars().count() <= 12, "{}", french.text);
    let list = |title: &str| logic::List {
        title: title.into(),
        steps: Vec::new(),
    };
    assert!(logic::tab_title(&list(&"x".repeat(100))).chars().count() <= 40);
    assert_eq!(logic::tab_title(&list("a \n\t b")), "a b", "one line");
    assert_eq!(
        logic::tab_title(&list(&"é".repeat(100))).chars().count(),
        40
    );
}

fn scenario_saved() -> Saved {
    let keys = keys();
    Saved {
        answers: BTreeMap::from([
            (keys[0].clone(), Answer::new(Verdict::Ok)),
            (keys[1].clone(), Answer::failed("The page stays blank.")),
            (keys[2].clone(), Answer::new(Verdict::Skip)),
        ]),
        redo: BTreeMap::new(),
        sent_at: None,
    }
}

fn the_list() -> logic::List {
    logic::List {
        title: "Login and logout".into(),
        steps: list_steps(),
    }
}

#[test]
fn the_summary_is_plain_says_what_failed_with_the_notes_then_what_was_skipped() {
    let text = logic::build_summary(&the_list(), &keys(), &scenario_saved(), LIST_PATH);
    // Exactly what the kit's earlier HTML tab built for the same answers.
    assert_eq!(
        text,
        "Checklist results: \"Login and logout\" (recette/login.json).\n\
         3 steps: 1 OK, 1 failed, 1 skipped.\n\
         \n\
         FAILED:\n\
         - Step 2: Sign in with the test account.\n  Expected: The dashboard shows.\n  Note: The page stays blank.\n\
         \n\
         SKIPPED:\n\
         - Step 3: Press Sign out, then Back.\n\
         \n\
         Passed: steps 1.\n\
         \n\
         Fix what failed, then ask for the same list again with kit_tab_open (path \"recette/login.json\"): the failed steps come back to redo and you get a new summary."
    );
    assert!(text.find("FAILED") < text.find("SKIPPED"));
    assert!(!text.contains("NOT ANSWERED"));
    assert!(
        !text.contains(['*', '#', '`']),
        "no markup: it is read as it is"
    );
}

#[test]
fn the_summary_names_unanswered_steps_steps_to_redo_and_a_missing_note() {
    let keys = keys();
    let saved = Saved {
        answers: BTreeMap::from([(keys[0].clone(), Answer::failed(""))]),
        redo: BTreeMap::from([(keys[1].clone(), "blank page".to_owned())]),
        sent_at: None,
    };
    let text = logic::build_summary(&the_list(), &keys, &saved, LIST_PATH);
    assert_eq!(
        text,
        "Checklist results: \"Login and logout\" (recette/login.json).\n\
         3 steps: 0 OK, 1 failed, 0 skipped, 2 not answered.\n\
         \n\
         FAILED:\n\
         - Step 1: Open /login and enter a wrong password.\n  Expected: An error shows under the field.\n  Note: (none)\n\
         \n\
         NOT ANSWERED:\n\
         - Step 3: Press Sign out, then Back.\n\
         \n\
         NOT ANSWERED (failed before, to redo):\n\
         - Step 2: Sign in with the test account.\n  Failed before: blank page\n\
         \n\
         Fix what failed, then ask for the same list again with kit_tab_open (path \"recette/login.json\"): the failed steps come back to redo and you get a new summary."
    );
    let clean = Saved {
        answers: keys
            .iter()
            .map(|key| (key.clone(), Answer::new(Verdict::Ok)))
            .collect(),
        ..Saved::default()
    };
    assert!(
        logic::build_summary(&the_list(), &keys, &clean, LIST_PATH).ends_with("Nothing failed.")
    );
    let partial = Saved {
        answers: BTreeMap::from([(keys[0].clone(), Answer::new(Verdict::Ok))]),
        ..Saved::default()
    };
    assert!(
        logic::build_summary(&the_list(), &keys, &partial, LIST_PATH)
            .ends_with("Nothing failed in what was answered.")
    );
}

#[test]
fn a_summary_stays_under_what_panes_send_takes_whatever_the_list_holds() {
    let steps: Vec<logic::Step> = (0..200)
        .map(|i| logic::Step {
            text: format!("step {i} {}", "x".repeat(990)),
            expect: "y".repeat(990),
        })
        .collect();
    let keys = logic::step_keys(&steps);
    let saved = Saved {
        answers: keys
            .iter()
            .map(|key| (key.clone(), Answer::failed("n".repeat(2000))))
            .collect(),
        ..Saved::default()
    };
    let list = logic::List {
        title: "t".into(),
        steps,
    };
    let text = logic::build_summary(&list, &keys, &saved, "a.json");
    assert!(text.encode_utf16().count() <= 20000);
    assert!(text.chars().count() <= 20000);
}

#[test]
fn both_languages_hold_the_same_keys_with_their_accents() {
    let mut en: Vec<&str> = EN.iter().map(|(key, _)| *key).collect();
    let mut fr: Vec<&str> = FR.iter().map(|(key, _)| *key).collect();
    en.sort_unstable();
    fr.sort_unstable();
    assert_eq!(fr, en);
    assert_eq!(en.len(), 64, "the page's 64 strings");
    let fr = strings("fr");
    assert_eq!(fr.t("send"), "Envoyer à l'agent");
    assert_eq!(fr.t("problem"), "Problème");
    assert_eq!(fr.tn("stateFailure", 1, &[]), "1 étape en échec");
    assert_eq!(fr.tn("stateFailure", 3, &[]), "3 étapes en échec");
    assert_eq!(strings("en").tn("stateFailure", 3, &[]), "3 steps failed");
    // A language Hangar has that the tab has not is English.
    assert_eq!(strings("de").t("send"), "Send to agent");
    assert_eq!(strings("fr-FR").t("send"), "Envoyer à l'agent");
}

#[test]
fn javascripts_trim_and_length_are_kept() {
    // U+FEFF is white space to JavaScript, U+0085 is not; text is measured in UTF-16 units.
    assert_eq!(logic::js_trim("\u{feff} a \u{a0}"), "a");
    assert_eq!(logic::js_trim("\u{85}a"), "\u{85}a");
    assert_eq!(logic::js_len("é😀"), 3);
    assert_eq!(logic::js_prefix("a😀b", 2), "a", "a pair is not cut in two");
    assert_eq!(logic::js_prefix("a😀b", 3), "a😀");
}
