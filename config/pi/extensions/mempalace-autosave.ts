import { spawn, spawnSync } from "node:child_process";
import { basename } from "node:path";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

/** Bridges Pi lifecycle events to MemPalace hooks without blocking Pi. */
const MEMPALACE = process.platform === "win32" ? "mempalace.exe" : "mempalace";
const STDIN_FLUSH_TIMEOUT_MS = 2000;

/** Use one stable wing for every session in a repository. */
export function canonicalWing(cwd: string | undefined): string {
 const start = cwd?.trim() || process.cwd();
 try {
  const result = spawnSync("git", ["-C", start, "rev-parse", "--show-toplevel"], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] });
  const root = typeof result.stdout === "string" ? result.stdout.trim() : "";
  if (root) return basename(root.replace(/[\\/]+$/, ""));
 } catch { /* fall through to the session directory */ }
 return basename(start.replace(/[\\/]+$/, "")) || "sessions";
}

export function hookPayload(sessionId: string | undefined, transcriptPath: string, cwd: string | undefined) {
 return { session_id: sessionId ?? "unknown", transcript_path: transcriptPath, stop_hook_active: false, wing: canonicalWing(cwd) };
}

function runHook(hook: "stop" | "session-end" | "precompact", sessionId: string | undefined, transcriptPath: string | undefined, cwd: string | undefined): Promise<void> {
 return new Promise((resolve) => {
  let settled = false;
  const timer = setTimeout(done, STDIN_FLUSH_TIMEOUT_MS);
  function done() { if (settled) return; settled = true; clearTimeout(timer); resolve(); }
  if (!transcriptPath) return done();
  try {
   const child = spawn(MEMPALACE, ["hook", "run", "--hook", hook, "--harness", "claude-code"], { stdio: ["pipe", "ignore", "ignore"], detached: true, windowsHide: true });
   child.on("error", done);
   child.stdin?.on("error", done);
   child.stdin?.on("close", done);
   if (!child.stdin) return done();
   // MemPalace 3.8.0's hook CLI does not accept a wing argument. Include the
   // canonical identity in the envelope for hook implementations that support
   // it; the current provider ignores this field (see the report).
   child.stdin.end(JSON.stringify(hookPayload(sessionId, transcriptPath, cwd)));
   child.unref();
  } catch { done(); }
 });
}

export default function (pi: ExtensionAPI) {
 if (process.env.PI_SUBAGENT_CHILD === "1" || process.env.PI_MEMPALACE_AUTOSAVE === "0") return;
 pi.on("agent_settled", async (_event, ctx) => await runHook("stop", ctx.sessionManager.getSessionId(), ctx.sessionManager.getSessionFile(), ctx.cwd));
 pi.on("session_before_compact", async (_event, ctx) => await runHook("precompact", ctx.sessionManager.getSessionId(), ctx.sessionManager.getSessionFile(), ctx.cwd));
 pi.on("session_shutdown", async (_event, ctx) => await runHook("session-end", ctx.sessionManager.getSessionId(), ctx.sessionManager.getSessionFile(), ctx.cwd));
}

export const __testing = { canonicalWing, hookPayload };
