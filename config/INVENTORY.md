# Inventory

What this machine has that a fresh one will not. The versions are a
reading taken on the date below, not a lock.

Captured 2026-08-30.

## Runtimes

| Component | Version |
| --- | --- |
| pi | 0.84.4 |
| node | v24.20.0 (LTS Krypton) |
| rust | 1.98.0 |
| uv | 0.12.7 |

## Pi packages

Declared in `config/pi/settings.json`; `pi install` restores them.

- `npm:pi-web-access`
- `npm:@juicesharp/rpiv-ask-user-question`
- `npm:@juicesharp/rpiv-todo`
- `npm:@narumitw/pi-usage`
- `npm:pi-subagents`
- `npm:pi-lens`
- `npm:@dietrichgebert/ponytail`
- `npm:context-mode`
- `npm:pi-claude-bridge`
- `npm:pi-mcp-adapter`
- `git:github.com/obra/superpowers`
- `git:github.com/mattpocock/skills@6654f6b60cd9d5be8b54c6fafe44346dabeb3b76`
- `git:github.com/robonuggets/gauntlet-loop`
- `npm:@plannotator/pi-extension`

## Python tools (uv)

- codegraphcontext v0.6.8
- graphifyy v0.9.51
- mempalace v3.8.0
- specify-cli v1.0.1

The `semantica` tool (installed after this capture) additionally requires the
spaCy models `en_core_web_md` and `en_core_web_sm` installed into its uv tool
venv; without them entity/relation extraction silently degrades to a naive
pattern stub (`UNKNOWN` entities, `related_to` relations). Install via
`uv pip install --python <semantica tool venv python> <model wheel>` —
`python -m spacy download` does not work in a uv tool environment.
`config/provision.txt` has no manifest kind for a spaCy model, so this is a
manual post-provision step for now; capturing it in provisioning is a
follow-up.

## Rust tools (cargo)

- just 1.58.0
- prek 0.5.0
- cargo-deny 0.20.2
- cargo-machete 0.9.2

## Standalone binaries

| Binary | Source |
| --- | --- |
| gh 2.98.0 | cli/cli release tarball |
| github-mcp-server | github/github-mcp-server |
| llama-server, llama-cli, llama-bench | built from source, symlinked from the llama.cpp build |
| herdr, claude, codex, hf | installed separately |

## Never captured

| Path | Why |
| --- | --- |
| `~/.pi/agent/auth.json` | credentials. Re-auth on the new machine. |
| `~/.config/gh/hosts.yml` | GitHub token. Run `gh auth login`. |
| `~/.pi/agent/trust.json` | per-machine project trust, absolute paths |
| `~/.pi/agent/{npm,git,bin}` | 616 MB of reinstallable packages |
| `~/.pi/agent/sessions` | conversation history, not configuration |
| `~/.pi/agent/mcp-*cache.json` | regenerated on demand |
| model weights (~207 GB) | fetched, not carried |
