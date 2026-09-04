import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

/** Injects one short skill-routing reminder after session start and compaction. */
const POLICY = `<EXTREMELY_IMPORTANT>
Skill routing: before a non-trivial request, decide whether an installed skill applies. If there is a modest chance, read its SKILL.md before acting. For trivial requests, answer directly. Load only the one or two relevant skills; never load the whole catalogue.
</EXTREMELY_IMPORTANT>`;

export default function (pi: ExtensionAPI) {
 if (process.env.PI_SUBAGENT_CHILD === "1" || process.env.PI_SKILL_ROUTER === "0") return;
 let armed = false;
 pi.on("session_start", () => { armed = true; });
 pi.on("session_compact", () => { armed = true; });
 pi.on("context", (event) => {
  if (!armed) return;
  armed = false;
  return { messages: [...event.messages, { role: "user" as const, content: POLICY, timestamp: Date.now() }] };
 });
}
