"use strict";
// The manifest, checked by hand against docs/native/kits-native.md in Hangar: Hangar's validator
// is Rust and cannot be reached from a Node test. A value's rule is written in what Rust and
// JavaScript read alike, as Hangar's window reads it. The tab itself is tested in native/tests/.
const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const KIT = path.join(__dirname, "..");
const manifest = JSON.parse(fs.readFileSync(path.join(KIT, "hangar-kit.json"), "utf8"));
const exists = (relative) => fs.existsSync(path.join(KIT, relative));
const whole = (pattern) => new RegExp(`^(?:${pattern})$`, "u");
const PERMISSIONS = ["workspace.read", "panes.read", "panes.send", "files.read", "files.write", "data.read", "data.write", "preview.show", "notify"];

test("the manifest's own fields", () => {
  assert.equal(manifest.schema, 2, "a native tab needs schema 2");
  assert.equal(manifest.id, "checklist");
  assert.match(manifest.id, /^[a-z0-9-]{2,40}$/);
  assert.equal(manifest.name, "Checklist");
  assert.match(manifest.version, /^\d+\.\d+\.\d+$/);
  assert.ok(manifest.description.trim());
  assert.equal(manifest.hangar, ">=2.0.0");
  assert.equal(manifest.homepage, "https://github.com/hangar-ai/kit-checklist");
  assert.ok(exists(manifest.icon) && manifest.icon.endsWith(".svg"));
  assert.ok(manifest.gallery.permissionsSummary.trim());
  assert.ok(manifest.gallery.tags.length && manifest.gallery.tags.every((tag) => typeof tag === "string" && tag.trim()));
});

test("the plugin is named like the kit, and holds a skill and nothing that runs by itself", () => {
  assert.equal(manifest.plugin, undefined);
  const plugin = JSON.parse(fs.readFileSync(path.join(KIT, ".claude-plugin", "plugin.json"), "utf8"));
  assert.equal(plugin.name, manifest.id);
  assert.equal(plugin.version, manifest.version);
  for (const part of ["hooks", ".mcp.json", ".lsp.json", "monitors", "bin", "settings.json"]) assert.ok(!exists(part), part);
  for (const key of ["hooks", "mcpServers", "lspServers", "monitors", "settings", "binaries", "dependencies"]) assert.equal(plugin[key], undefined, key);
  assert.equal(manifest.commands, undefined, "no command: the kit runs nothing");
  assert.equal(manifest.setup, undefined);
});

test("the tab: five permissions and nothing more, one tab per list", () => {
  assert.equal(manifest.tabs.length, 1);
  const tab = manifest.tabs[0];
  assert.match(tab.id, /^[a-z0-9-]{1,40}$/);
  assert.equal(tab.title, "Checklist");
  assert.ok(exists(tab.entry) && tab.entry.endsWith(".wasm") && exists(tab.icon));
  assert.notEqual(tab.singleton, true, "one tab per path");
  assert.deepEqual([...tab.permissions].sort(), ["data.read", "data.write", "files.read", "notify", "panes.send"]);
  assert.ok(tab.permissions.every((permission) => PERMISSIONS.includes(permission)));
  assert.deepEqual(Object.keys(tab.params), ["path"]);
  const param = tab.params.path;
  assert.ok(param.description.trim());
  const rule = whole(param.pattern);
  assert.ok(rule.test("recette/login.json") && rule.test("a b/c.json"));
  assert.ok(!rule.test("") && !rule.test("C:/x.json") && !rule.test("a\nb"));
  assert.doesNotMatch(param.pattern, /\(\?[<!=]/, "Rust's regex has no look-around");
});

test("the tab is native: its entry is a WebAssembly module of the kit, built from native/", () => {
  const tab = manifest.tabs[0];
  assert.equal(tab.entry, "tabs/checklist.wasm");
  assert.equal(tab.native, undefined, "no `native` field: the entry is the module");
  assert.ok(exists(tab.entry), "the module is built and committed");
  assert.ok(!path.isAbsolute(tab.entry) && !tab.entry.includes("\\") && !tab.entry.includes(":"));
  assert.ok(tab.entry.split("/").every((part) => part && !part.startsWith(".")), "inside the kit, not hidden");
  const bytes = fs.readFileSync(path.join(KIT, tab.entry));
  assert.deepEqual([...bytes.subarray(0, 8)], [0, 0x61, 0x73, 0x6d, 1, 0, 0, 0], "WebAssembly, version 1");
  assert.ok(bytes.length < 64 * 1024 * 1024, "64 MB at most");
  assert.ok(exists("scripts/build-native.sh") && exists("native/Cargo.toml") && exists("native/Cargo.lock"));
  assert.ok(!exists("tabs/checklist"), "no page beside the module");
});

test("the skill is there, named like the kit, and asks for concrete steps", () => {
  const skill = fs.readFileSync(path.join(KIT, "skills", "checklist", "SKILL.md"), "utf8");
  assert.match(skill, /^---\nname: checklist\ndescription: .+\n---\n/);
  assert.match(skill, /kit_tab_open/);
  assert.match(skill, /"kit": "checklist"/);
  assert.match(skill, /\*\*Stop\.\*\*/);
  assert.match(skill, /check it works/);
});

test("nothing personal, nothing attributed to an assistant", () => {
  const files = [];
  const skipped = new Set([".git", "node_modules", "target"]); // not the kit's own text
  const walk = (dir) =>
    fs.readdirSync(dir, { withFileTypes: true }).forEach((entry) => {
      if (entry.isDirectory()) return skipped.has(entry.name) ? undefined : walk(path.join(dir, entry.name));
      files.push(path.join(dir, entry.name));
    });
  walk(KIT);
  for (const file of files.filter((file) => path.basename(file) !== "manifest.test.js" && !file.endsWith(".wasm"))) {
    const text = fs.readFileSync(file, "utf8");
    assert.doesNotMatch(text, /C:\\Users|\/Users\/|@[a-z0-9-]+\.(fr|com)\b|Co-Authored-By/i, path.relative(KIT, file));
  }
});
