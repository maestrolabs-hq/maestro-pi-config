import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { inspect, restore } from "./config.ts";
import {
  type Entry,
  fromTemplate,
  mcpConfigDir,
  piAgentDir,
  toTemplate,
  userBinDir,
} from "./manifest.ts";

function scratch(): string {
  return mkdtempSync(join(tmpdir(), "maestro-pi-config-"));
}

function entryIn(root: string, live: string, extra: Partial<Entry> = {}): Entry {
  mkdirSync(join(root, "config"), { recursive: true });
  return { repo: "config/thing.json", live, kind: "file", ...extra };
}

test("paths are derived from the environment, never hardcoded", () => {
  const env = {
    PI_AGENT_DIR: "/somewhere/agent",
    XDG_CONFIG_HOME: "/somewhere/cfg",
    PI_CONFIG_BIN_DIR: "/somewhere/bin",
  };
  assert.equal(piAgentDir(env), "/somewhere/agent");
  assert.equal(mcpConfigDir(env), join("/somewhere/cfg", "mcp"));
  assert.equal(userBinDir(env), "/somewhere/bin");
});

test("a template round-trips through the home directory", () => {
  const home = "/home/someone";
  const live = `${home}/.mempalace/palace`;
  assert.equal(toTemplate(live, home), "${HOME}/.mempalace/palace");
  assert.equal(fromTemplate(toTemplate(live, home), home), live);
});

test("identical content is not reported as drift", () => {
  const root = scratch();
  const live = join(root, "live.json");
  const entry = entryIn(root, live);
  writeFileSync(join(root, entry.repo), '{"a":1}');
  writeFileSync(live, '{"a":1}');
  assert.equal(inspect(entry, root).state, "same");
});

test("a trailing newline counts as drift, because a restore would write it", () => {
  const root = scratch();
  const live = join(root, "live.json");
  const entry = entryIn(root, live);
  writeFileSync(join(root, entry.repo), '{"a":1}\n');
  writeFileSync(live, '{"a":1}');
  assert.equal(inspect(entry, root).state, "differs");
});

test("restore writes nothing unless applied", () => {
  const root = scratch();
  const live = join(root, "nested", "live.json");
  const entry = entryIn(root, live);
  writeFileSync(join(root, entry.repo), '{"a":1}');

  const planned = restore(root, false, [entry]);
  assert.ok(planned.includes(live), "the plan names the file it would write");
  assert.equal(existsSync(live), false, "dry run must not create it");

  restore(root, true, [entry]);
  assert.equal(readFileSync(live, "utf8"), '{"a":1}');
});
