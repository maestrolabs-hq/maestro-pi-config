---
name: provider-fanout
description: "Trigger the same operation across all 7 code-intelligence and memory providers (CGC, CodeGraph, Codebase-Memory, Graphify, Semantica, MemPalace, Docling) in parallel, and return their answers side by side, labeled by provider. Use when the user asks to query all providers, fanout a question, compare graph opinions, index everything, refresh all graphs, or check provider status — including phrasing like \"/fanout\", \"ask every graph\", \"reindex all providers\", or \"provider status\"."
---

# Provider fanout

One request in, every provider's equivalent call out — in parallel, never
merged. This skill exists because the estate runs seven independent graph and
memory providers side by side (`docs/providers/`) and none of their
identities, indexes, scores, or results are ever fused. A fanout juxtaposes
seven labeled answers to the same question; it does not synthesize one answer
from them. Reading and comparing the rows is the caller's job.

Three verbs are covered: **query** (ask a question), **index/refresh**
(rebuild or update a provider's graph for the current repository), and
**status** (health/coverage of each provider's index). Every provider appears
in every fanout's result array, even when a verb does not apply to it — a
provider that cannot answer a verb reports why (`ok: null`, a note) rather
than being silently dropped. Triggering together, honestly, is the point.

## The equivalence mapping

Verified against the live MCP gateway tool list and cross-checked against
`docs/providers/*.md` in `maestro-core` for exact argument names. Tool names
below are the literal `tools.call` first argument.

| Provider | index/refresh | query | status |
| --- | --- | --- | --- |
| CGC | **CLI-only, requires the MCP server stopped first** — `cgc_add_code_to_graph({ repo_path })` cannot index a repository that is not already in the graph on this deployment: it fails opaquely ("Tool execution error") and creates no job (`cgc_list_jobs` stays empty, `cgc_list_indexed_repositories` unchanged). It only returns cleanly (`"already indexed"`) as a no-op on a repo already present — that is not indexing, it is a status check in disguise. The only working indexer is the CLI: `cgc --db kuzudb --path <workspace>/.maestro/state/providers/cgc/kuzudb index <repo-path> --summarize`. `--db`/`--path` are **global options before the subcommand**, not `index` flags, and are not read from `CGC_RUNTIME_DB_*` env vars by the CLI (those only affect the MCP server). KuzuDB is single-writer: the CLI fails with a lock error while `cgc mcp start` is connected, so the server must be stopped first, the CLI run per repo, then the server left to respawn lazily on the next MCP call | `cgc_find_code({ query })` (+ `cgc_analyze_code_relationships`, `cgc_execute_cypher_query` for structured questions) | `cgc_list_indexed_repositories()`, `cgc_get_repository_stats({ path })` |
| CodeGraph | **CLI only** — MCP exposes no indexing tool: `codegraph init --yes` (first run) / `codegraph sync` (incremental); a background watcher then keeps it current | `codegraph_codegraph_explore({ query })` — the live tool name has a doubled `codegraph_` prefix; do not shorten it | **CLI only** — `codegraph status --json`; the MCP `codegraph_status` tool exists but is unlisted by default and unreachable without `CODEGRAPH_MCP_TOOLS` set |
| Codebase-Memory | `codebase-memory_index_repository({ repo_path })` — the param is `repo_path`, **not** `path` | `codebase-memory_search_graph({ query })` (+ `query_graph` for Cypher-like, `get_architecture()` for a zero-arg overview) | `codebase-memory_index_status({ project })`, `codebase-memory_list_projects()` |
| Graphify | **CLI only** — the MCP surface is read-only (no build/extract tool exists): `graphify update <path>` (add `--force` after refactors that delete code) | `graphify_query_graph({ query })` | `graphify_graph_stats()` |
| Semantica | **CLI is broken in 0.6.7, use the native companion script** — `semantica ingest <path> --type repo` forwards `batch_size` (and `output`/`recursive` when given) into git clone options, which the RepoIngestor allowlist rejects; `--type file --recursive` also fails (duplicate `recursive` kwarg). The verified method drives `semantica.context.ContextGraph` directly against `SEMANTICA_KG_PATH` — see the companion script under Template B. The `semantica-mcp` server only loads `SEMANTICA_KG_PATH` at startup, so restart it before a query sees newly ingested nodes | `semantica_query_graph({ query, mode: "search" })` (+ `semantica_query_decisions`) | `semantica_get_graph_summary()` |
| MemPalace | `mempalace_mempalace_mine({ source, wing })` — the param is `source`, **not** `path`; `wing` is optional and defaults to the source directory name | `mempalace_mempalace_search({ query })` (+ `mempalace_mempalace_kg_query({ entity })` for KG-only lookups) | `mempalace_mempalace_status()`, `mempalace_mempalace_kg_stats()` |
| Docling | `docling_convert_directory_files_into_docling_document({ source })` — the param is `source`, **not** `directory`; converts, does not index in the graph sense | `docling_search_for_text_in_document_anchors({ document_key, query })` — **scoped to one already-cached document; cannot answer open code questions** | `docling_is_document_in_local_cache({ document_key })` — **per-document only; there is no global "is anything cached" call** |

Identities, indexes, scores, and results are never merged across providers —
this is a repeated, explicit rule across every page in `docs/providers/`. The
fanout templates below juxtapose labeled rows; nothing here fuses graphs.

## Template A — query-all

Fans a single question out to the five providers that can answer an open
question (CGC, Codebase-Memory, Graphify, Semantica, MemPalace). CodeGraph is
included too since `codegraph_codegraph_explore` answers questions natively.
Docling is included as an honest `n/a` row — it cannot answer code questions,
only search within one already-converted document.

```javascript
async function queryAll(question) {
  const calls = [
    { provider: "cgc", fn: () => tools.call("cgc_find_code", { query: question }) },
    { provider: "codegraph", fn: () => tools.call("codegraph_codegraph_explore", { query: question }) },
    { provider: "codebase-memory", fn: () => tools.call("codebase-memory_search_graph", { query: question }) },
    { provider: "graphify", fn: () => tools.call("graphify_query_graph", { query: question }) },
    { provider: "semantica", fn: () => tools.call("semantica_query_graph", { query: question, mode: "search" }) },
    { provider: "mempalace", fn: () => tools.call("mempalace_mempalace_search", { query: question }) },
  ];

  const results = await Promise.all(
    calls.map(async ({ provider, fn }) => {
      try {
        const res = await fn();
        return res && res.ok !== false
          ? { provider, ok: true, data: res.data ?? res }
          : { provider, ok: false, error: res?.error ?? "unknown error" };
      } catch (err) {
        return { provider, ok: false, error: String(err) };
      }
    })
  );

  results.push({
    provider: "docling",
    ok: null,
    note: "n/a: docling answers questions only within one already-cached document (search_for_text_in_document_anchors), not open code questions",
  });

  for (const r of results) {
    if (r.ok === true) emit(`${r.provider}: ok`);
    else if (r.ok === false) emit(`${r.provider}: FAILED — ${r.error}`);
    else emit(`${r.provider}: ${r.note}`);
  }

  return results;
}

return await queryAll(QUESTION);
```

## Template B — index-all

When the user says **"index"**, **"index all"**, or **"reindex"** for a repo, run
BOTH halves below in the same turn without asking further questions — that is
the entire point of this template. Neither half alone covers all seven
providers; the mcpScript half covers the three providers with a real,
verified MCP indexing call (Codebase-Memory, MemPalace, Docling), and the
shell half covers the three that require the native CLI or companion script
(CodeGraph, Graphify, Semantica). CGC is neither — it needs its own
server-stop step and must never run in the same parallel block as the other
three CLI providers (see the note after this template).

```javascript
async function indexAll(repoPath) {
  const calls = [
    { provider: "codebase-memory", fn: () => tools.call("codebase-memory_index_repository", { repo_path: repoPath }) },
    { provider: "mempalace", fn: () => tools.call("mempalace_mempalace_mine", { source: repoPath }) },
    { provider: "docling", fn: () => tools.call("docling_convert_directory_files_into_docling_document", { source: repoPath }) },
  ];

  const results = await Promise.all(
    calls.map(async ({ provider, fn }) => {
      try {
        const res = await fn();
        return res && res.ok !== false
          ? { provider, ok: true, data: res.data ?? res }
          : { provider, ok: false, error: res?.error ?? "unknown error" };
      } catch (err) {
        return { provider, ok: false, error: String(err) };
      }
    })
  );

  results.push(
    { provider: "cgc", ok: null, note: "cli-only, needs the MCP server stopped first: stop cgc mcp start, run cgc --db kuzudb --path <cgc-db-path> index <repo-path> --summarize per repo, then let Pi respawn the server on its next MCP call — this is a separate step, never run alongside the shell block below" },
    { provider: "codegraph", ok: null, note: "cli-only: codegraph init --yes (first run) or codegraph sync (incremental) — run the companion shell block" },
    { provider: "graphify", ok: null, note: "cli-only: graphify update <path> — run the companion shell block" },
    { provider: "semantica", ok: null, note: "cli-only: semantica ingest --type repo is broken in 0.6.7 — run the companion shell block's native ContextGraph script instead" }
  );

  for (const r of results) {
    if (r.ok === true) emit(`${r.provider}: indexed`);
    else if (r.ok === false) emit(`${r.provider}: FAILED — ${r.error}`);
    else emit(`${r.provider}: ${r.note}`);
  }

  return results;
}

return await indexAll(REPO_PATH);
```

Companion shell block — run alongside the mcpScript above, in the same turn,
against the same `REPO_PATH`. Substitute `REPO_PATH`, `REPO_ID` (a short
identifier for the repo, e.g. its directory name), and `SEMANTICA_KG_PATH`
(`<workspace>/.maestro/state/providers/semantica/global-graph.json`) before
running. CodeGraph and Graphify run in parallel; Semantica's script runs
after them since it uses its own Python interpreter and does not need to race
the others:

```bash
( codegraph sync || codegraph init --yes ) & \
( graphify update "$REPO_PATH" ) & \
wait

cat > /tmp/semantica-ingest-"$REPO_ID".py <<EOF
import shutil
from pathlib import Path
from semantica.context import ContextGraph

REPO = Path("$REPO_PATH")
KG = Path("$SEMANTICA_KG_PATH")
RID = "$REPO_ID"
LANG = {".md": "markdown", ".toml": "toml", ".json": "json", ".yaml": "yaml", ".yml": "yaml"}

shutil.copy2(KG, str(KG) + ".bak")
g = ContextGraph()
g.load_from_file(KG)
n = e = 0
if g.add_node(RID, "repository", RID, source="repository", repository=RID):
    n += 1
for p in sorted(REPO.rglob("*")):
    if not p.is_file():
        continue
    rel = p.relative_to(REPO)
    if rel.parts[0] in {".git", "graphify-out"}:
        continue
    if p.suffix.lower() not in LANG and p.suffix.lower() != ".txt":
        continue
    try:
        text = p.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        continue
    # Node ids are prefixed with RID to avoid colliding with maestro-core's bare-filename ids.
    nid = f"{RID}/{rel.as_posix()}"
    if g.add_node(nid, "file", text, language=LANG.get(p.suffix.lower(), "text"),
                  extension=p.suffix.lower(), relative_path=rel.as_posix(), source=RID):
        n += 1
    if g.add_edge(RID, nid, "contains"):
        e += 1
g.save_to_file(KG)
print(f"nodes added: {n}, edges added: {e}")
EOF
~/.local/share/uv/tools/semantica/bin/python /tmp/semantica-ingest-"$REPO_ID".py
```

Restart the `semantica-mcp` server after the script succeeds — it only reads
`SEMANTICA_KG_PATH` at startup, so `semantica_query_graph` will not see the
new nodes until it is reloaded.

CGC indexing is a separate step, run before or after the two blocks above,
never inside either one: stop the running `cgc mcp start` server (it holds
the KuzuDB single-writer lock the whole time it is connected), then run
`cgc --db kuzudb --path <cgc-db-path> index <repo-path> --summarize` for each
repo, then let Pi respawn the server lazily on its next MCP call. Do not call
`cgc_add_code_to_graph` expecting it to index a new repo — on this deployment
it only no-ops cleanly on repos already indexed and fails opaquely, with no
job created, on anything new.

## Template C — status-all

All seven providers report in one pass: five MCP status calls plus a
CodeGraph CLI companion, with Docling's row noting that there is no
global cache status — only a per-document check exists.

```javascript
async function statusAll() {
  const calls = [
    { provider: "cgc", fn: () => tools.call("cgc_list_indexed_repositories", {}) },
    { provider: "codebase-memory", fn: () => tools.call("codebase-memory_list_projects", {}) },
    { provider: "graphify", fn: () => tools.call("graphify_graph_stats", {}) },
    { provider: "semantica", fn: () => tools.call("semantica_get_graph_summary", {}) },
    { provider: "mempalace", fn: () => tools.call("mempalace_mempalace_status", {}) },
  ];

  const results = await Promise.all(
    calls.map(async ({ provider, fn }) => {
      try {
        const res = await fn();
        return res && res.ok !== false
          ? { provider, ok: true, data: res.data ?? res }
          : { provider, ok: false, error: res?.error ?? "unknown error" };
      } catch (err) {
        return { provider, ok: false, error: String(err) };
      }
    })
  );

  results.push(
    { provider: "codegraph", ok: null, note: "cli-only: run the companion shell block (codegraph status --json)" },
    { provider: "docling", ok: null, note: "n/a: is_document_in_local_cache is per-document only; there is no global cache-status call" }
  );

  for (const r of results) {
    if (r.ok === true) emit(`${r.provider}: ok`);
    else if (r.ok === false) emit(`${r.provider}: FAILED — ${r.error}`);
    else emit(`${r.provider}: ${r.note}`);
  }

  return results;
}

return await statusAll();
```

Companion shell block:

```bash
codegraph status --json
```

## Template D — validate (open every provider page in one browser window)

After an index-all (Template B), eyeball the result. Only two providers publish
a browsable page; the other five are validated non-visually by Template C
(status-all), so a complete validation is **Template C + Template D together**.

| Provider | Visual page | How |
| --- | --- | --- |
| Graphify | one `graph.html` per repo | static file `<repo>/graphify-out/graph.html`, opened via `file://` |
| Semantica | one workspace-global explorer | `semantica explorer start` -> `http://127.0.0.1:5173` |
| Codebase-Memory | best-effort only | `ui_port` (default 9749); reachable only while a client session exposes it, often down |
| CGC / CodeGraph / MemPalace / Docling | no page | validate via Template C status calls |

Multiple URLs passed to `msedge.exe` open as tabs in ONE window. Edge has no CLI
to create a *named* tab group, so grouping them (right-click -> "Add tabs to new
group") stays a manual step — one window is the practical grouping.

```bash
WORKSPACE="$HOME/workspace/MaestroLabs"   # adjust to your workspace root
EDGE="$(ls '/mnt/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe' \
           '/mnt/c/Program Files/Microsoft/Edge/Application/msedge.exe' 2>/dev/null | head -1)"

urls=()
for html in "$WORKSPACE"/*/graphify-out/graph.html; do
  [ -f "$html" ] && urls+=("$(wslpath -w "$html")")   # Edge needs the \\wsl.localhost\... form
done

# Optional: add the Semantica explorer (workspace-global; start it first if stopped)
semantica explorer status | grep -qi running || semantica explorer start   # -> http://127.0.0.1:5173
urls+=("http://127.0.0.1:5173")

"$EDGE" "${urls[@]}"    # one Edge window, one tab per page
```

`wslpath -w` translates each WSL path to the `\\wsl.localhost\<distro>\...` UNC
form Edge requires and derives the distro itself, so nothing here names a fixed
machine. Off WSL, replace the `"$EDGE" ...` launch with `xdg-open` (Linux) or
`open` (macOS) per URL — those open one tab each rather than a single window.

## Usage

1. Pick the verb: query (Template A), index/refresh (Template B), or status
   (Template C).
2. For query, substitute the question into `QUESTION`. For index, substitute
   the repository's absolute path into `REPO_PATH` and run both the mcpScript
   and its companion shell block in the same turn — one half without the
   other leaves providers silently unindexed.
3. Before first use in a session, the seven MCP servers must be connected —
   `mcp({ connect: "cgc" })` and the same for `codegraph`, `codebase-memory`,
   `graphify`, `semantica`, `mempalace`, `docling`. A lazy/not-yet-connected
   server fails its Promise.all branch cleanly (caught, reported as
   `ok: false`) rather than blocking the other six.
4. Read the returned array as seven independent opinions. Do not average,
   merge, or pick a "winning" provider — that decision belongs to the caller,
   never to this skill.
5. To validate an index visually, run Template D: it opens every Graphify
   `graph.html` (and, optionally, the Semantica explorer) as tabs in one Edge
   window. Pair it with Template C (status-all) to cover the five providers
   that have no page.

## Notes

- **Never merges provider identities.** Every `docs/providers/*.md` page
  states this as an estate rule; this skill's entire job is to juxtapose
  labeled rows, not fuse them into one graph.
- **CLI-only exceptions are structural, not a workaround.** CodeGraph's MCP
  server exposes no indexing tool at all (its watcher indexes automatically
  once `codegraph init` has run once). Graphify's MCP server is read-only by
  design — building or updating its graph is a CLI-only operation. Semantica's
  wired `semantica-mcp` server is also read/query-only; repository ingestion
  is a native CLI operation, but the CLI itself is broken in 0.6.7 (see
  Template B), so the verified route is the native `ContextGraph` companion
  script, not `semantica ingest`. CGC's MCP `add_code_to_graph` tool cannot
  index a new repo on this deployment at all — it fails opaquely and creates
  no job, and only returns cleanly as a no-op on a repo already indexed. Its
  only working indexer is the CLI, which in turn requires the MCP server to
  be stopped first because KuzuDB is single-writer and the server holds the
  lock for as long as it is connected. All facts come from live verification
  against this deployment, not from a limitation of this skill.
- **Docling has real `n/a` semantics, not missing coverage.** Docling holds
  no repository index at all; its cache and status are always scoped to one
  converted document, never global.
- One provider failing, timing out, or being disconnected must never sink
  the whole fanout — every template's `Promise.all` wraps each provider call
  in its own `try/catch`, so a single bad row never blocks the other six.
