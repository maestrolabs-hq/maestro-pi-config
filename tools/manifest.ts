import { homedir } from "node:os";
import { join } from "node:path";
import process from "node:process";

/**
 * Structural, rather than the ambient `NodeJS.ProcessEnv`: this file should
 * type-check without depending on a global namespace being in scope.
 */
export type Env = Readonly<Record<string, string | undefined>>;

/** Where a captured item lives on this machine, and how it is stored here. */
export type Entry = {
  /** Path inside this repository, relative to the repository root. */
  readonly repo: string;
  /** Absolute path on the machine. Derived, never written down. */
  readonly live: string;
  /** A directory is compared and copied by its contents. */
  readonly kind: "file" | "dir";
  /**
   * Stored with the home directory replaced by `${HOME}`, because the value
   * is machine-specific. Expanded on restore, collapsed on sync.
   */
  readonly templated?: boolean;
  /** Restored with the executable bit set. */
  readonly executable?: boolean;
};

/**
 * Pi keeps its agent directory under the user's home on every platform. An
 * explicit override wins, so a machine with a relocated agent directory can
 * still sync.
 */
export function piAgentDir(env: Env = process.env): string {
  return env.PI_AGENT_DIR ?? join(homedir(), ".pi", "agent");
}

/** The tool-agnostic MCP config directory, per the adapter's documented order. */
export function mcpConfigDir(env: Env = process.env): string {
  const xdg = env.XDG_CONFIG_HOME;
  return join(xdg ?? join(homedir(), ".config"), "mcp");
}

/** A directory on PATH that the user owns. */
export function userBinDir(env: Env = process.env): string {
  return env.PI_CONFIG_BIN_DIR ?? join(homedir(), ".local", "bin");
}

export function manifest(env: Env = process.env): readonly Entry[] {
  const pi = piAgentDir(env);
  const home = homedir();
  return [
    { repo: "config/pi/settings.json", live: join(pi, "settings.json"), kind: "file" },
    { repo: "config/pi/claude-bridge.json", live: join(pi, "claude-bridge.json"), kind: "file" },
    { repo: "config/pi/models-store.json", live: join(pi, "models-store.json"), kind: "file" },
    { repo: "config/pi/skills", live: join(pi, "skills"), kind: "dir" },
    { repo: "config/mcp/mcp.json", live: join(mcpConfigDir(env), "mcp.json"), kind: "file" },
    {
      repo: "config/tools/mempalace/config.template.json",
      live: join(home, ".mempalace", "config.json"),
      kind: "file",
      templated: true,
    },
    {
      repo: "config/tools/codegraphcontext/config.yaml",
      live: join(home, ".codegraphcontext", "config.yaml"),
      kind: "file",
    },
    {
      repo: "config/tools/codegraphcontext/env.template",
      live: join(home, ".codegraphcontext", ".env"),
      kind: "file",
      templated: true,
    },
    { repo: "config/bin", live: userBinDir(env), kind: "dir", executable: true },
  ];
}

/** Collapse this machine's home directory to a placeholder. */
export function toTemplate(text: string, home: string = homedir()): string {
  return text.split(home).join("${HOME}");
}

/** Expand the placeholder to this machine's home directory. */
export function fromTemplate(text: string, home: string = homedir()): string {
  return text.split("${HOME}").join(home);
}
