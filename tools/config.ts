import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative } from "node:path";
import { type Entry, fromTemplate, manifest, toTemplate } from "./manifest.ts";

export type State = "same" | "differs" | "absent-live" | "absent-repo";

export type Report = {
  readonly entry: Entry;
  readonly state: State;
  /** Paths that differ, relative to the entry, for a directory. */
  readonly detail: readonly string[];
};

function filesUnder(root: string): string[] {
  if (!existsSync(root)) return [];
  const out: string[] = [];
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const full = join(dir, name);
      if (statSync(full).isDirectory()) walk(full);
      else out.push(relative(root, full));
    }
  };
  walk(root);
  return out.sort();
}

/** Repository form of a live file: templated entries collapse the home path. */
function asStored(text: string, entry: Entry): string {
  return entry.templated ? toTemplate(text) : text;
}

/** Live form of a stored file: templated entries expand the placeholder. */
function asLive(text: string, entry: Entry): string {
  return entry.templated ? fromTemplate(text) : text;
}

function read(path: string): string | null {
  return existsSync(path) ? readFileSync(path, "utf8") : null;
}

export function inspect(entry: Entry, root: string): Report {
  const repoPath = join(root, entry.repo);
  const repoHere = existsSync(repoPath);
  const liveHere = existsSync(entry.live);

  if (!liveHere) return { entry, state: "absent-live", detail: [] };
  if (!repoHere) return { entry, state: "absent-repo", detail: [] };

  if (entry.kind === "file") {
    const stored = read(repoPath);
    const live = read(entry.live);
    const same = stored !== null && live !== null && stored === asStored(live, entry);
    return { entry, state: same ? "same" : "differs", detail: same ? [] : [entry.repo] };
  }

  // A directory entry tracks only the files it already carries: the live
  // directory may hold unrelated things, and claiming those is not this
  // repository's business.
  const differing = filesUnder(repoPath).filter((rel) => {
    const stored = read(join(repoPath, rel));
    const live = read(join(entry.live, rel));
    return live === null || stored === null || stored !== asStored(live, entry);
  });
  return { entry, state: differing.length === 0 ? "same" : "differs", detail: differing };
}

export function status(root: string, entries: readonly Entry[] = manifest()): readonly Report[] {
  return entries.map((entry) => inspect(entry, root));
}

/** Machine to repository. Writes only inside the repository. */
export function sync(root: string, entries: readonly Entry[] = manifest()): readonly string[] {
  const written: string[] = [];
  for (const entry of entries) {
    if (!existsSync(entry.live)) continue;
    const repoPath = join(root, entry.repo);
    if (entry.kind === "file") {
      const live = read(entry.live);
      if (live === null) continue;
      mkdirSync(dirname(repoPath), { recursive: true });
      writeFileSync(repoPath, asStored(live, entry));
      written.push(entry.repo);
      continue;
    }
    for (const rel of filesUnder(repoPath)) {
      const live = read(join(entry.live, rel));
      if (live === null) continue;
      writeFileSync(join(repoPath, rel), asStored(live, entry));
      written.push(join(entry.repo, rel));
    }
  }
  return written;
}

/**
 * Repository to machine. Nothing is written unless `apply` is true: this
 * overwrites live configuration, so the default has to be harmless.
 */
export function restore(
  root: string,
  apply: boolean,
  entries: readonly Entry[] = manifest(),
): readonly string[] {
  const touched: string[] = [];
  const put = (target: string, text: string, executable?: boolean) => {
    touched.push(target);
    if (!apply) return;
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, text);
    if (executable) chmodSync(target, 0o755);
  };

  for (const entry of entries) {
    const repoPath = join(root, entry.repo);
    if (!existsSync(repoPath)) continue;
    if (entry.kind === "file") {
      const stored = read(repoPath);
      if (stored !== null) put(entry.live, asLive(stored, entry), entry.executable);
      continue;
    }
    for (const rel of filesUnder(repoPath)) {
      const stored = read(join(repoPath, rel));
      if (stored !== null) put(join(entry.live, rel), asLive(stored, entry), entry.executable);
    }
  }
  return touched;
}
