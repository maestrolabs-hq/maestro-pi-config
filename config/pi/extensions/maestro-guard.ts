import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

/** Blocks execution in the main session; subagents retain full access. */
const BLOCKED = new Set(["edit", "write", "bash", "ctx_execute", "ctx_batch_execute", "ctx_execute_file", "mcpScript", "ast_grep_replace", "lens_diagnostic_mark"]);

export default function (pi: ExtensionAPI) {
 if (process.env.PI_SUBAGENT_CHILD === "1" || process.env.PI_MAESTRO_GUARD === "0") return;
 pi.on("tool_call", (event) => {
  if (!BLOCKED.has(event.toolName)) return;
  return {
   block: true,
   reason: `Maestro guard: '${event.toolName}' is blocked in the main session. Orchestrate instead of executing: delegate edits and shell work to a subagent; read, grep, find, ls, MCP queries, and AST tools remain allowed.`,
  };
 });
}
