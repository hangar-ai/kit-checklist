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
readable at the side panel's narrowest.

### A native tab

The tab is native (Hangar 2.0 or later): Rust compiled to WebAssembly, in `native/`, built into
`tabs/checklist.wasm`, which the manifest names as the tab's `entry`. No browser engine runs for
it: Hangar draws its widgets itself. It keeps its answers in the kit's data folder, one small
file per list, and asks for five permissions. A person who kept answers with the kit's earlier
HTML tab finds them again: the file and the steps' keys are the same to the byte.

What a native tab cannot do:

- **A last write when the tab is closed.** A closing tab gets no last update. Answers are written
  as they come, after the pause below, and when the tab goes behind another; the answers of the
  last seconds before a tab is closed are lost (1.5 s of them, up to 7 s just after a write).
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

You need Hangar 2.0.0 or later, which runs native tabs.

1. In Hangar, Settings › Kits, add the GitHub link:
   `https://github.com/hangar-ai/kit-checklist`
   (or link a local copy of this folder).
2. Enable it on a workspace, and read what you accept: the five permissions above.
3. Restart the workspace's Claude Code panes so they load the skill.

## How it is built

```
checklist/
├── hangar-kit.json            the kit: schema 2, id, hangar >=2.0.0, the tab (`entry` is the module), its `path` param, five permissions
├── icon.svg
├── .claude-plugin/plugin.json the Claude Code plugin (name = the kit's id), only a skill
├── skills/checklist/SKILL.md
├── tabs/checklist.wasm        the tab, built from native/ and committed: Hangar runs it, it never builds
├── native/                    the tab, a Rust crate on hangar-kit-sdk (cdylib + rlib)
│   ├── Cargo.toml, Cargo.lock, rust-toolchain.toml
│   ├── src/lib.rs             the tab: state, messages, loading, saving, sending
│   ├── src/logic.rs           pure, no SDK type: the list, the answers, the summary
│   ├── src/view.rs            the state as widgets
│   ├── src/strings.rs         the words, en and fr
│   └── tests/                 cargo test, through the SDK's harness
├── scripts/build-native.sh    native/ -> tabs/checklist.wasm (wasm32-unknown-unknown, release); --check
├── test/manifest.test.js      node --test: the manifest, the plugin and the skill
└── package.json, run-tests.js let `node --test kits/checklist/` find the tests on any Node
```

Choices worth knowing:

- **A step is known by its text.** Answers follow a step the agent moved, and only a
  step whose text changed loses its answer.
- **The answers' file is named after the list's path**, with a hash, so two lists never
  share one.
- **Writes are spread out.** `data.write` is limited to 10 a minute, so answers and notes
  are written after a short pause (1.5 s after the first change, not pushed back by the next
  ones) and never closer than 7 seconds, and once more when the tab is hidden. Hangar's « too many » is waited out and tried again after 15 s.
- **The module is committed.** Hangar runs the `.wasm` a kit ships and never builds one, so a
  kit installed by cloning works as it is. It is built with a pinned compiler and no path of
  the machine in it, so that the same source gives the same bytes anywhere.

## Tests

```
cargo test --manifest-path native/Cargo.toml --locked  # the native tab
node --test kits/checklist/                            # the manifest, the plugin, the skill
scripts/build-native.sh --check                        # the committed module is native/'s build
```

The native tests run on the host through the SDK's `testing::Harness`: a pretend Hangar answers the
tab's calls over files and a data folder held in memory. They also hold what the earlier HTML tab
wrote to the byte: the steps' keys, the answers' file, the summary. The Node test needs no install
step.

To change the native tab, edit `native/`, then build the module and commit it with the source:

```
rustup target add wasm32-unknown-unknown   # once (rustup reads native/rust-toolchain.toml)
scripts/build-native.sh
```

To look at it, in a Hangar repository checkout: `scripts/native-dev.sh`, then Settings › Kits ›
link this folder, open the tab, and reload it from its bar after each build. Without Hangar,
`cargo run -p hangar-kit-host --example preview -- tabs/checklist.wasm`.

## This repository

CI calls [kit-check](https://github.com/hangar-ai/.github/blob/main/.github/workflows/kit-check.yml),
then tests the native tab, rebuilds `tabs/checklist.wasm` and fails if the committed module is not
what `native/` builds. A release is a tag, `v` and the manifest's version, and it reaches Hangar's
gallery once [the catalog](https://github.com/hangar-ai/catalog) is updated and reviewed.
