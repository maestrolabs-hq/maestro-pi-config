import { spawn, spawnSync } from "node:child_process";
import { accessSync, appendFileSync, constants, mkdirSync, readdirSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { env } from "node:process";
import { basename, delimiter, dirname, join } from "node:path";

interface ExtensionAPI {
  on(name: string, handler: (event: { reason: string }) => void): void;
}

/**
 * MemPalace relation derivation — the packaged CLI, nothing else.
 *
 * MemPalace derives relations inside its miners: ingest, THEN derive. Running
 * `mine <EMPTY_DIR> --mode projects --wing W` performs the derivation half for
 * one wing — hallways, entity tunnels, topic tunnels — and files nothing,
 * because there is nothing in the directory to file.
 *
 * That is the whole extension. It shells out to the `mempalace` console
 * command exactly as mempalace-autosave.ts shells out to `mempalace hook run`,
 * and imports no MemPalace internals, so there is no version guard to keep and
 * nothing to revalidate after `uv tool upgrade`.
 *
 * WHAT WAS REMOVED, AND WHY
 * -------------------------
 * A custom Python pass used to run first, back-filling the `entities` metadata
 * that `add_drawer` does not write. Measured head to head on identical copies:
 * the CLI alone produced 2014 hallways and 61 tunnels; the CLI plus the custom
 * pass produced 2015 hallways and the same 61 tunnels. One hallway, for 415
 * lines and three imported internals. Its one-time backfill of 339 drawers was
 * worth doing and is already banked in stored Chroma metadata — deleting the
 * script does not undo it.
 *
 * The documented path for relationships an agent knows about is explicit:
 * `mempalace_kg_add` for typed facts and `mempalace_create_tunnel` for a
 * deliberate cross-wing link. Neither needs code here.
 *
 * WHY session_start AND NOT session_shutdown
 * ------------------------------------------
 * `mine_palace_lock` is non-blocking, and the MCP server takes it lazily on
 * its first mutating tool call then holds it for its whole process lifetime.
 * At shutdown this races a server that still owns the palace and usually
 * loses. At startup the incoming server has not written yet, so the lease is
 * free. Starting up also makes the outcome observable: a shutdown-time child
 * is orphaned as Pi exits, so its exit code can never be read.
 *
 * NO CRASH JOURNAL IS NEEDED. The canonical repository wing is derived on
 * every session and each step is idempotent. An interruption is repaired by
 * the next session with no state carried across.
 *
 * Deliberately OUT of the memory write path. Relations are derived state and
 * can be rebuilt at any time; a memory cannot.
 *
 * Subagent children (PI_SUBAGENT_CHILD=1) skip entirely — one derivation per
 * real session is enough. NOTE: this also means a test launched from inside a
 * subagent skips, which looks exactly like a broken extension.
 *
 * Escape hatch: launch with PI_MEMPALACE_DERIVE=0 to disable for one session.
 */

const MEMPALACE_EXE = process.platform === "win32" ? "mempalace.exe" : "mempalace";

const PALACE = join(homedir(), ".mempalace", "palace");
const STATE_DIR = join(homedir(), ".mempalace", "hook_state");
const LOG_FILE = join(STATE_DIR, "derive.log");

// `mine --mode projects` INGESTS whatever it finds. Pointed at a non-empty
// directory it would file those files as drawers, so this directory is
// re-verified empty immediately before every single invocation.
const EMPTY_DIR = join(tmpdir(), "maestro-mempalace-derive-emptydir");

/** Use one stable wing for every derivation in a repository. */
export function canonicalWing(cwd: string | undefined): string {
  const start = cwd?.trim() || process.cwd();
  try {
    const result = spawnSync(
      "git",
      ["-C", start, "rev-parse", "--show-toplevel"],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    );
    const root = typeof result.stdout === "string" ? result.stdout.trim() : "";
    if (root) return basename(root.replace(/[\\/]+$/, ""));
  } catch {
    /* fall through to the session directory */
  }
  return basename(start.replace(/[\\/]+$/, "")) || "sessions";
}

function executableOnPath() {
  const path = env.PATH?.split(delimiter) ?? [];
  const names = process.platform === "win32" ? ["mempalace.exe", "mempalace"] : ["mempalace", "mempalace.exe"];
  return path.some((directory) => names.some((name) => {
    try { accessSync(join(directory, name), constants.X_OK); return true; } catch { return false; }
  }));
}

/** Append one line to the shared log. Never throws — logging must not break startup. */
function note(msg: string): void {
  try {
    mkdirSync(dirname(LOG_FILE), { recursive: true });
    const stamp = new Date()
      .toLocaleString("sv", { timeZoneName: "short" })
      .slice(0, 19);
    appendFileSync(LOG_FILE, `[${stamp}] EXT   ${msg}\n`, "utf-8");
  } catch {
    /* ignore */
  }
}

interface RunResult {
  code: number | null;
  stdout: string;
  stderr: string;
}

interface DerivationSummary {
  complete: boolean;
  failedWings: string[];
}

interface DeriveOptions {
  runCommand?: (args: string[]) => Promise<RunResult>;
  sleep?: (delayMs: number) => Promise<void>;
  verifyEmpty?: () => boolean;
  executableAvailable?: () => boolean;
  resolveCanonicalWing?: () => string;
  log?: (message: string) => void;
}

const LOCK_RETRY_DELAYS_MS = [500, 1000, 2000, 4000];
const MAX_ATTEMPTS = LOCK_RETRY_DELAYS_MS.length + 1;
const LOCK_DIAGNOSTIC =
  /^mempalace: palace .+ is held by (?:PID \d+(?: \([^\r\n]+\))?|another writer \(identity not recorded\)); wait for it to finish or stop the holder before retrying$/;

function isLockContention(result: RunResult): boolean {
  return [result.stdout, result.stderr].some((output) =>
    LOCK_DIAGNOSTIC.test(output.trim()),
  );
}

/** Run a command to completion, capturing output. Never rejects. */
function run(args: string[]): Promise<RunResult> {
  return new Promise((resolve) => {
    let stdout = "";
    let stderr = "";
    let settled = false;
    const finish = (r: RunResult) => {
      if (settled) return;
      settled = true;
      resolve(r);
    };
    try {
      const child = spawn(MEMPALACE_EXE, args, {
        stdio: ["ignore", "pipe", "pipe"],
        windowsHide: true,
      });
      child.stdout?.on("data", (d: unknown) => (stdout += String(d)));
      child.stderr?.on("data", (d: unknown) => (stderr += String(d)));
      child.on("error", (err: Error) =>
        finish({ code: null, stdout, stderr: `${stderr}\n${err.message}` }),
      );
      child.on("close", (code: number | null) =>
        finish({ code, stdout, stderr }),
      );
    } catch (err) {
      finish({
        code: null,
        stdout: "",
        stderr: err instanceof Error ? err.message : String(err),
      });
    }
  });
}

/**
 * Ensure the sacrificial directory exists and contains nothing.
 *
 * Returns false rather than deleting anything: if something unexpected is in
 * there, the safe move is to refuse to mine, not to clear it and continue.
 */
function verifiedEmpty(): boolean {
  try {
    mkdirSync(EMPTY_DIR, { recursive: true });
    const entries = readdirSync(EMPTY_DIR);
    if (entries.length > 0) {
      note(
        `ABORT: ${EMPTY_DIR} is not empty (${entries.length} entr(y|ies): ` +
          `${entries.slice(0, 5).join(", ")}). Refusing to mine — ` +
          `--mode projects would file them as drawers.`,
      );
      return false;
    }
    return true;
  } catch (err) {
    note(
      `ABORT: cannot verify ${EMPTY_DIR} is empty: ` +
        `${err instanceof Error ? err.message : String(err)}`,
    );
    return false;
  }
}

function firstNumber(stdout: string, re: RegExp): number | null {
  const m = re.exec(stdout);
  return m ? Number(m[1]) : null;
}

async function deriveAllWings(
  reason: string,
  options: DeriveOptions = {},
): Promise<DerivationSummary> {
  const runCommand = options.runCommand ?? run;
  const wait =
    options.sleep ??
    ((delayMs: number) =>
      new Promise<void>((resolve) => setTimeout(resolve, delayMs)));
  const verifyEmpty = options.verifyEmpty ?? verifiedEmpty;
  const executableAvailable =
    options.executableAvailable ?? executableOnPath;
  const log = options.log ?? note;
  const incomplete = (failedWings: string[] = []): DerivationSummary => {
    log(
      `derivation incomplete: ${failedWings.join(", ") || "prerequisite unavailable"}`,
    );
    return { complete: false, failedWings };
  };

  if (!executableAvailable()) {
    // Report a missing prerequisite instead of failing mute: a silently absent
    // step is indistinguishable from one that ran and found nothing.
    log(`skipped: mempalace executable not found on PATH (${MEMPALACE_EXE})`);
    return incomplete();
  }

  log(`launched (reason=${reason})`);
  const started = Date.now();

  const wing = options.resolveCanonicalWing?.() ?? canonicalWing(process.cwd());
  if (!wing) {
    log("ABORT: could not determine the canonical repository wing");
    return incomplete();
  }
  log(`deriving canonical wing: ${wing}`);

  let hallways = 0;
  let tunnels = 0;
  const failedWings: string[] = [];

  let r: RunResult = { code: null, stdout: "", stderr: "" };
  let attempt = 0;
  for (; attempt < MAX_ATTEMPTS; attempt++) {
      // Re-verified before EVERY invocation, not once up front: this is the
      // guard against anything dropping a file in between.
      if (!verifyEmpty()) {
        return incomplete([...failedWings, wing]);
      }

      r = await runCommand([
        "--palace",
        PALACE,
        "mine",
        EMPTY_DIR,
        "--mode",
        "projects",
        "--wing",
        wing,
      ]);
      if (r.code === 0) break;

      const locked = isLockContention(r);
      if (!locked || attempt === MAX_ATTEMPTS - 1) {
        if (locked) {
          log(
            `  ${wing}: lock retries exhausted after ${MAX_ATTEMPTS} attempts`,
          );
        }
        break;
      }

      const delayMs = LOCK_RETRY_DELAYS_MS[attempt];
      log(
        `  ${wing}: lock contention; retry ${attempt + 1}/${LOCK_RETRY_DELAYS_MS.length}` +
          ` (attempt ${attempt + 2}/${MAX_ATTEMPTS}), delay=${delayMs}ms`,
      );
    await wait(delayMs);
  }

  if (r.code !== 0) {
    failedWings.push(wing);
    log(`  ${wing}: FAILED exit=${r.code}`);
  } else {

    // The load-bearing assertion. This whole approach rests on `mine` running
    // its derivation steps while filing nothing; if a future release changes
    // that, this is where it surfaces instead of silently ingesting.
    //
    // `Files:` is checked as well as `Files processed:`, and it is the more
    // sensitive of the two: mining a directory holding one decoy file was
    // measured printing `Files: 1` alongside `Files processed: 0`, because
    // that file happened to be skipped. Only `Files: 0` actually proves the
    // directory the command walked was empty.
    const found = firstNumber(r.stdout, /^\s*Files:\s*(\d+)/m);
    const processed = firstNumber(r.stdout, /Files processed:\s*(\d+)/);
    const filed = firstNumber(r.stdout, /Drawers filed:\s*(\d+)/);
    if (found !== 0 || processed !== 0 || filed !== 0) {
      failedWings.push(wing);
      log(
        `  ${wing}: ABORT — expected Files/processed/filed all 0, got ` +
          `${found}/${processed}/${filed}. The empty-directory technique no ` +
          `longer holds; stopping before anything else is ingested.`,
      );
      return incomplete(failedWings);
    }

    const h = firstNumber(r.stdout, /Hallways:\s*\+(\d+)/) ?? 0;
    const t = firstNumber(r.stdout, /Entity tunnels:\s*\+(\d+)/) ?? 0;
    const tt = firstNumber(r.stdout, /Topic tunnels:\s*\+(\d+)/) ?? 0;
    hallways += h;
    tunnels += t + tt;
    log(
      `  ${wing}: hallways +${h}, entity tunnels +${t}, topic tunnels +${tt}`,
    );
  }

  const summary = { complete: failedWings.length === 0, failedWings };
  log(
    `derivation done: hallways +${hallways}, tunnels +${tunnels}` +
      (failedWings.length ? `, ${failedWings.length} wing(s) failed` : ""),
  );
  log(`finished in ${((Date.now() - started) / 1000).toFixed(1)}s`);
  if (!summary.complete)
    log(`derivation incomplete: ${failedWings.join(", ")}`);
  return summary;
}

export const __testing = { canonicalWing, deriveAllWings };

export default function (
  pi: ExtensionAPI,
  derive: (reason: string) => Promise<DerivationSummary> = deriveAllWings,
) {
  if (env.PI_SUBAGENT_CHILD === "1") return;
  if (env.PI_MEMPALACE_DERIVE === "0") return;

  pi.on("session_start", (event: { reason: string }) => {
    // Deliberately NOT awaited: the full sequence takes ~12s and session
    // start must not wait for it. The extension stays loaded for the session,
    // so the chain runs to completion in the background and every outcome
    // lands in the durable log.
    void derive(event.reason).catch((err) => {
      note(
        `derivation threw: ${err instanceof Error ? err.message : String(err)}`,
      );
    });
  });
}
