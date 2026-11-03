//! The manifest of a native tab: the module its `entry` names, and the calls
//! the tab makes held to the permissions it declares.

mod common;

use std::path::{Path, PathBuf};

use serde_json::Value;

use common::{LIST_PATH, Setup};
use hangar_kit_sdk::api::{METHODS, Needs};
use kit_checklist_tab::logic::Verdict;

fn kit() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn manifest() -> Value {
    let text = std::fs::read_to_string(kit().join("hangar-kit.json")).expect("the manifest");
    serde_json::from_str(&text).expect("is JSON")
}

fn tab() -> Value {
    manifest()["tabs"][0].clone()
}

#[test]
fn the_manifest_is_schema_2_and_the_entry_is_the_module() {
    let manifest = manifest();
    assert_eq!(manifest["schema"], 2, "a native tab needs schema 2");
    assert_eq!(manifest["hangar"], ">=2.0.0");
    assert_eq!(manifest["id"], "checklist");
    let tab = tab();
    assert_eq!(tab["id"], "checklist");
    assert_eq!(tab["entry"], "tabs/checklist.wasm");
    assert!(tab.get("native").is_none(), "no `native` field in schema 2");
    assert!(
        !kit().join("tabs/checklist").exists(),
        "no page beside the module"
    );
}

#[test]
fn the_native_module_exists_is_a_wasm_file_and_is_inside_the_kit() {
    let native = tab()["entry"].as_str().expect("a name").to_owned();
    assert!(native.ends_with(".wasm"));
    assert!(!native.starts_with('/') && !native.contains('\\') && !native.contains(':'));
    assert!(
        native
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.')),
        "inside the kit, not hidden, no `..`: {native}"
    );
    let file = kit().join(&native);
    let bytes = std::fs::read(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    assert_eq!(&bytes[..4], b"\0asm", "WebAssembly");
    assert!(bytes.len() < 64 * 1024 * 1024, "64 MB at most");
    assert!(
        bytes.len() < 2 * 1024 * 1024,
        "about 500 KB, not a debug build: {} bytes",
        bytes.len()
    );
}

#[test]
fn the_tab_keeps_its_five_permissions_and_its_path_param() {
    let tab = tab();
    let mut permissions: Vec<&str> = tab["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();
    permissions.sort_unstable();
    assert_eq!(
        permissions,
        [
            "data.read",
            "data.write",
            "files.read",
            "notify",
            "panes.send"
        ]
    );
    assert_ne!(tab["singleton"], true, "one tab per path");
    let params = tab["params"].as_object().unwrap();
    assert_eq!(params.keys().collect::<Vec<_>>(), ["path"]);
}

#[test]
fn the_plugin_and_the_manifest_agree_on_the_version() {
    let plugin: Value = serde_json::from_str(
        &std::fs::read_to_string(kit().join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(plugin["name"], manifest()["id"]);
    assert_eq!(plugin["version"], manifest()["version"]);
    assert_eq!(
        manifest()["version"],
        env!("CARGO_PKG_VERSION"),
        "the crate is the kit's version"
    );
}

#[test]
fn every_call_the_tab_makes_is_one_its_permissions_allow() {
    let tab = tab();
    let granted: Vec<&str> = tab["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap())
        .collect();

    // Everything the tab does, once: an agent's list and a person's.
    let mut agent = Setup::new().open();
    agent.answer(0, Verdict::Ok);
    agent.answer(1, Verdict::Fail);
    agent.note(1, "blank");
    agent.advance(2000);
    agent.press("Send to agent");
    agent.ask_again(Some(LIST_PATH), Some("Ava"));
    let mut person = Setup::new().no_caller().open();
    person.answer(0, Verdict::Ok);
    person.press("Copy the summary");
    let mut broken = Setup::new().path("nope.json").open();
    broken.press("Try again");

    let mut seen = std::collections::BTreeSet::new();
    for sim in [&agent, &person, &broken] {
        seen.extend(sim.host.calls.iter().map(|(method, _)| method.clone()));
    }
    for method in &seen {
        let needs = METHODS
            .iter()
            .find(|(name, _)| name == method)
            .unwrap_or_else(|| panic!("{method} is not a method"))
            .1;
        match needs {
            Needs::Nothing => {}
            Needs::Permission(permission) => {
                assert!(granted.contains(&permission), "{method} needs {permission}")
            }
            other => panic!("{method} needs {other:?}, which the manifest does not declare"),
        }
    }
    // And all five are used: none is asked for without a reason.
    for permission in &granted {
        assert!(
            seen.iter().any(|method| {
                METHODS.iter().any(|(name, needs)| {
                    name == method && matches!(needs, Needs::Permission(p) if p == permission)
                })
            }),
            "{permission} is declared and never used; calls: {seen:?}"
        );
    }
    // The calls the tab makes of Hangar, and the two of `hangar.ui` that need
    // no permission.
    let page = [
        "files.read",
        "data.read",
        "data.write",
        "panes.send",
        "notify",
        "tab.setTitle",
        "tab.setBadge",
    ];
    for method in page {
        assert!(seen.contains(method), "{method}");
    }
    let extra: Vec<_> = seen
        .iter()
        .filter(|method| !page.contains(&method.as_str()))
        .collect();
    assert_eq!(extra, ["ui.copy", "ui.toast"], "copyButton's two");
}

// ---------------------------------------------------------------- The module itself.

/// A WebAssembly module's imports and exports, read from its sections: enough
/// to hold the committed file to the ABI without running it.
struct Interface {
    imports: Vec<(String, String)>,
    exports: Vec<String>,
}

fn leb(bytes: &[u8], at: &mut usize) -> usize {
    let (mut value, mut shift) = (0usize, 0);
    loop {
        let byte = bytes[*at];
        *at += 1;
        value |= usize::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return value;
        }
        shift += 7;
    }
}

fn name(bytes: &[u8], at: &mut usize) -> String {
    let len = leb(bytes, at);
    let text = String::from_utf8(bytes[*at..*at + len].to_vec()).expect("a UTF-8 name");
    *at += len;
    text
}

fn limits(bytes: &[u8], at: &mut usize) {
    let flags = leb(bytes, at);
    leb(bytes, at);
    if flags & 1 != 0 {
        leb(bytes, at);
    }
}

fn interface(bytes: &[u8]) -> Interface {
    let mut found = Interface {
        imports: Vec::new(),
        exports: Vec::new(),
    };
    let mut at = 8;
    while at < bytes.len() {
        let id = bytes[at];
        at += 1;
        let size = leb(bytes, &mut at);
        let end = at + size;
        let mut cursor = at;
        match id {
            2 => {
                for _ in 0..leb(bytes, &mut cursor) {
                    let module = name(bytes, &mut cursor);
                    let field = name(bytes, &mut cursor);
                    match bytes[cursor] {
                        0 => {
                            cursor += 1;
                            leb(bytes, &mut cursor);
                        }
                        1 => {
                            cursor += 2;
                            limits(bytes, &mut cursor);
                        }
                        2 => {
                            cursor += 1;
                            limits(bytes, &mut cursor);
                        }
                        3 => cursor += 3,
                        other => panic!("an import of kind {other}"),
                    }
                    found.imports.push((module, field));
                }
            }
            7 => {
                for _ in 0..leb(bytes, &mut cursor) {
                    found.exports.push(name(bytes, &mut cursor));
                    cursor += 1;
                    leb(bytes, &mut cursor);
                }
            }
            _ => {}
        }
        at = end;
    }
    found
}

#[test]
fn the_module_speaks_the_abi_and_imports_nothing_but_hangars_two() {
    let bytes = std::fs::read(kit().join("tabs/checklist.wasm")).unwrap();
    let found = interface(&bytes);
    let mut imports = found.imports.clone();
    imports.sort();
    assert_eq!(
        imports,
        [
            ("hangar".to_owned(), "log".to_owned()),
            ("hangar".to_owned(), "now".to_owned()),
        ],
        "no WASI, no JavaScript glue: built for wasm32-unknown-unknown"
    );
    for export in [
        "memory",
        "hangar_abi_version",
        "hangar_alloc",
        "hangar_init",
        "hangar_update",
        "hangar_view",
    ] {
        assert!(found.exports.iter().any(|name| name == export), "{export}");
    }
}
