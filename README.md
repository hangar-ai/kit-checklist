# Checklist

A Hangar kit for the end of an agent's work. Agents keep finishing with « please
test this » in a chat nobody re-reads. With this kit the agent lists what only a
person can check, the person works through the list in a tab, and each result goes
back to the agent, who fixes what failed.

## What it does

- **The Checklist tab.** It opens on a JSON file the agent wrote in the workspace, so
  a long list is no problem for the tab's params:

  ```json
  {
    "title": "Login and logout",
    "steps": [
      { "text": "Open /login, enter a wrong password.", "expect": "An error shows under the field." }
    ]
  }
  ```

  `text` is required, `expect` is what the person should see. Up to 200 steps. One
  tab per list.
- **Each step** is marked **OK**, **Problem** (with a note of what was seen) or
  **Skip**. The tab shows the progress, and its badge goes from running to success or
  failure. Answers are kept in the kit's own data folder, so they survive a restart.
- **« Send to agent »** sends the agent that opened the tab a plain summary: what
  failed, with the notes, then what was skipped or left unanswered. When the person
  opened the tab themselves, there is no agent to answer: the summary can be copied.
- **Testing a fix.** Once the result is sent, opening the same path again (the agent
  does it with `kit_tab_open`) keeps the answers and puts the failed steps back as
  « to redo », with the note of what went wrong.
- **A list that is missing or malformed** is named for what it is, with what to do,
  and nothing else changes. If saving is not allowed, the list still works and says
  its answers will not be kept.
- **A skill** telling agents when to use it: at the end of a work that needs a human
  check, write the list, open it with `kit_tab_open`, and stop until the answer
  arrives; write concrete steps with what to expect, never « check it works ».

English and French, from Hangar's language; light and dark with Hangar's own theme;
readable at the side panel's narrowest. It uses Hangar UI and loads nothing else.

### Two tabs of one behaviour

The tab exists twice, and the manifest names both:

- **The page** (`tabs/checklist/`, HTML) is what Hangar 1.x opens.
- **The native tab** (`native/`, Rust compiled to WebAssembly, `tabs/checklist.wasm`) is what
  the native Hangar opens. No browser engine runs for it: Hangar draws its widgets itself.
  It reads the same lists, keeps its answers in the same files (an answer given in one is
  found by the other), sends the same summary and asks for the same five permissions. It
  calls the same methods, bar `tab.params` (the start carries the params) and the clipboard
  and toast that the page had in its own document (`ui.copy`, `ui.toast`; they need no
  permission). When the module cannot be loaded, the native Hangar opens the page and says
  so in the tab's bar.

What the native tab cannot do that the page did:

- **A last write when the tab is closed.** A closing tab gets no last update (the page wrote
  on `pagehide`). Answers are written as they come, after the pause below, and when the tab
  goes behind another; the answers of the last seconds before a tab is closed are lost (1.5 s
  of them, up to 7 s just after a write).
- **Select text with the pointer** (a step, an expectation): Hangar's text cannot be selected.
  The summary has its copy button for a list nobody asked for.
- **Accessibility roles** (`role="alert"`, `aria-*`): Hangar's widgets have no accessibility
  tree yet; glyphs and icons carry a tooltip instead.

## What it can touch

It runs nothing: no command, no hook, no script. The tab asks for five permissions,
shown to you before you enable the kit:

| Permission   | What it is for                                                                     |
| ------------ | ---------------------------------------------------------------------------------- |
| `files.read` | Read the one list file named in the tab's `path`, in the workspace folder. Hidden files and folders are refused by Hangar. It cannot change a project file. |
| `data.read`  | Read your answers back, from the kit's own data folder.                            |
| `data.write` | Write them there: one small file per list, under `answers/`.                       |
| `panes.send` | Send the summary to the agent that opened the tab, and to no one else.             |
| `notify`     | Tell you once that the result went out.                                            |

The summary holds the steps' text and your notes, and goes to that agent only.

## Install

You need Hangar 1.9.0 or later. Any Hangar opens the tab: the native one runs
`tabs/checklist.wasm`, an older one the page.

1. In Hangar, Settings › Kits, add the GitHub link:
   `https://github.com/hangar-ai/kit-checklist`
   (or link a local copy of this folder).
2. Enable it on a workspace, and read what you accept: the five permissions above.
3. Restart the workspace's Claude Code panes so they load the skill.

## How it is built

```
checklist/
├── hangar-kit.json            the kit: id, hangar >=1.9.0, the tab (`entry` the page, `native` the module), its `path` param, five permissions
├── icon.svg
├── .claude-plugin/plugin.json the Claude Code plugin (name = the kit's id), only a skill
├── skills/checklist/SKILL.md
├── tabs/checklist/            the page, for Hangar 1.x
│   ├── index.html             Hangar UI (/_hangar/kit.css, kit.js, kit-ui.js), then the scripts below
│   ├── logic.js               pure: the list checked, answers read and kept, badge, summary; messages in en and fr
│   ├── controller.js          reads the list, saves within data.write's rate, sets badge and title, sends
│   ├── view.js                draws a state with createElement + textContent, never markup
│   ├── app.js                 wires them in the page, in Hangar's language
│   └── style.css              layout of a step only: every colour is Hangar UI's
├── tabs/checklist.wasm        the native tab, built from native/ and committed: Hangar runs it, it never builds
├── native/                    the native tab, a Rust crate on hangar-kit-sdk (cdylib + rlib)
│   ├── Cargo.toml, Cargo.lock, rust-toolchain.toml
│   ├── src/lib.rs             the tab: state, messages, loading, saving, sending (controller.js)
│   ├── src/logic.rs           pure, no SDK type: the list, the answers, the summary (logic.js)
│   ├── src/view.rs            the state as widgets (view.js)
│   ├── src/strings.rs         the words, en and fr (logic.js's STRINGS)
│   └── tests/                 cargo test, through the SDK's harness: the cases of test/, ported
├── scripts/build-native.sh    native/ -> tabs/checklist.wasm (wasm32-unknown-unknown, release)
├── test/                      node --test, no dependency, against a fake window.hangar
└── package.json, run-tests.js let `node --test kits/checklist/` find the tests on any Node
```

Choices worth knowing:

- **A step is known by its text.** Answers follow a step the agent moved, and only a
  step whose text changed loses its answer.
- **The answers' file is named after the list's path**, with a hash, so two lists never
  share one.
- **Writes are spread out.** `data.write` is limited to 10 a minute, so answers and notes
  are written after a short pause (1.5 s after the first change, not pushed back by the next
  ones) and never closer than 7 seconds, and once more when the tab is hidden (the page also
  when it is closed). Hangar's « too many » is waited out and tried again after 15 s.
- **The module is committed.** Hangar runs the `.wasm` a kit ships and never builds one, so a
  kit installed by cloning works as it is. It is built with a pinned compiler and no path of
  the machine in it, so that the same source gives the same bytes anywhere.

## Tests

```
node --test kits/checklist/                          # the page, the manifest, the plugin
cargo test --manifest-path native/Cargo.toml --locked  # the native tab
```

The native tests are the page's, case for case (`native/tests/logic.rs`, `controller.rs`,
`view.rs`, `manifest.rs`), run on the host through the SDK's `testing::Harness`: a pretend
Hangar answers the tab's calls over files and a data folder held in memory. They also hold
what the page and the tab share to the byte: the steps' keys, the answers' file, the summary.

To change the native tab, edit `native/`, then build the module and commit it with the source:

```
rustup target add wasm32-unknown-unknown   # once (rustup reads native/rust-toolchain.toml)
scripts/build-native.sh
```

To look at it, in a Hangar repository checkout: `scripts/native-dev.sh`, then Settings › Kits ›
link this folder, open the tab, and reload it from its bar after each build. Without Hangar,
`cargo run -p hangar-kit-host --example preview -- tabs/checklist.wasm`.

## This repository

`node --test` runs the tests, with Node alone. Hangar's own files are not in this repository. CI takes Hangar UI and the docs from the Hangar tag the
manifest's `hangar` names, and `node test/hangar-ui.js --get` does the same here. Without them, these tests
skip what needs those files, and say so:

- the permissions and methods used are in the docs, when the docs are around

CI calls [kit-check](https://github.com/hangar-ai/.github/blob/main/.github/workflows/kit-check.yml),
then tests the native tab, rebuilds `tabs/checklist.wasm` and fails if the committed module is not
what `native/` builds. A release is a tag, `v` and the manifest's version, and it reaches Hangar's
gallery once [the catalog](https://github.com/hangar-ai/catalog) is updated and reviewed.
