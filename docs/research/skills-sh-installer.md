# skills.sh installer patterns for a universal Expander MCP installer

Research date: 2026-10-02. Sources are limited to first-party documentation and source repositories.

## Executive finding

`skills.sh` is a useful model for discovery and target selection, but an MCP installer should improve on it in three ways:

1. Never interpret `--yes` plus “nothing detected” as permission to install everywhere. Require an explicit target or return a clear nonzero error.
2. Separate **plan** from **apply**. `skills add` has a confirmation summary but no true dry-run; Expander should offer `--dry-run` with zero writes and a machine-readable plan.
3. Use each harness's native MCP command when available, then run its native list/get/health command. Config-file mutation should be a typed, atomic fallback.

## How the skills.sh installer works

### Detection registry

The implementation keeps a registry of agent definitions. Each entry declares an ID, display name, project skills directory, global skills directory, and an asynchronous `detectInstalled()` predicate. Most predicates test known config directories; some inspect project files or installed dependencies. Environment overrides are honored for important roots, including `CODEX_HOME` and `CLAUDE_CONFIG_DIR`. Detection fans out with `Promise.all()` and returns every positive match. [Agent registry](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/agents.ts)

There is a second kind of detection: whether the CLI itself is running inside a coding agent. `@vercel/detect-agent` supplies the signal; skills.sh maps its names to installer target IDs. Cursor receives extra filtering because a weak environment signal can also appear in ordinary integrated terminals. When an agent context is detected, `skills add` turns on noninteractive mode and, absent an explicit `--agent`, targets that agent plus universal agents. [Runtime-agent detection](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/detect-agent.ts) [Add flow](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/add.ts)

Pattern to reuse: keep static harness metadata separate from runtime-context detection, and record evidence for each match. For MCP, useful evidence is executable presence/version plus an existing config root; a directory alone is weak evidence.

### Target selection

The selection order is:

- `--agent '*'`: every registered target.
- Explicit `--agent <id>...`: validate IDs and use exactly those targets.
- Otherwise detect installed agents.
- No detected agents: prompt interactively; with `--yes`, skills.sh instead selects all agents.
- One detected agent, or `--yes`: select detected agents and ensure the shared/universal targets are included.
- Multiple detected agents interactively: show a searchable multi-select; previous choices are remembered.
- `--all` expands to every skill, every agent, and `--yes`.

The CLI also exits nonzero when an interactive prompt is required but stdin is not a TTY, and tells callers to pass `--agent` plus `-y`. [Add flow](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/add.ts) [CLI reference](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/README.md)

Pattern to change: for Expander, `--yes` should only accept already-determined choices. If detection returns zero or an ambiguous set, fail with an actionable message unless `--harness` or an explicit `--all-harnesses` was supplied.

### Scope and layout

Project scope is the default; `-g/--global` selects user scope. Interactive runs may ask for scope when it was not specified. The registry provides exact per-agent paths, while “universal” agents share `.agents/skills`. Codex uses project `.agents/skills` and global `$CODEX_HOME/skills`; Claude Code uses project `.claude/skills` and global `$CLAUDE_CONFIG_DIR/skills`; Pi uses project/global `.agents/skills`. [Supported-agent table](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/README.md#supported-agents) [Agent registry](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/agents.ts)

For Expander, scope must be modeled per harness because the meanings differ:

- user/global: private machine-wide registration;
- project: repository-local registration, often trust-gated and potentially commit-worthy;
- local/private-project (Claude Code): project-specific but stored outside the shared `.mcp.json`.

### Install mechanics and idempotency

In the default symlink mode, skills.sh copies a skill into a canonical `.agents/skills/<name>` directory, then links agent-specific locations to it. Universal agents use the canonical directory directly. Copy mode writes independent copies. A failed symlink falls back to a copy. A single unique target directory automatically uses copy mode. Existing identical symlinks are accepted, but ordinary existing destinations are removed and recreated; the preflight reports them as overwrites. [Installer](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/installer.ts)

This is repeatable but not a no-op: rerunning replaces the destination. Global and project lock files store source/hash information for updates. For an MCP installer, prefer semantic idempotency:

- normalize the desired server definition;
- inspect the existing named entry;
- if equivalent, report `unchanged` and do not rewrite;
- if different, show a redacted diff and require `--replace` (or confirmation);
- write config fallbacks atomically and preserve unrelated keys/comments where the format permits;
- never place secret values in command history or committed project config; store environment-variable names.

### Verification, dry-run, and automation

skills.sh shows an installation summary, checks whether targets will be overwritten, prints per-target results, supports `skills list`, and has `add --json` output. JSON mode requires `--yes`; prompt cancellation on non-TTY is an error. There is no documented `--dry-run`: `--list` lists source skills without installing, while the pre-install summary is still part of an interactive mutation flow. [CLI reference](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/README.md) [Add flow](https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/add.ts)

Expander should define these automation contracts:

- `--dry-run --json`: detect, resolve scope, inspect current state, and emit exact redacted commands/config diffs; never write or connect.
- `--yes`: suppress confirmation only; never invent missing target/scope decisions.
- stable statuses: `installed`, `unchanged`, `replaced`, `skipped`, `failed`, `unverified`.
- nonzero exit if any requested target fails or cannot be verified, with optional `--allow-unverified`.
- verification has two layers: registration (`list/get`) and connection/tool discovery where the harness supports it.

## Native MCP interfaces

### Codex

Official example for remote HTTP:

```sh
codex mcp add expander --url https://example.com/mcp
codex mcp list
codex mcp get expander
```

Official docs show `codex mcp add <name> --url <url>`, `codex mcp list`, and the user config form:

```toml
[mcp_servers.expander]
url = "https://example.com/mcp"
```

Codex shares configuration between its CLI and IDE. User defaults live at `~/.codex/config.toml`; trusted projects may override with `.codex/config.toml`. The official CLI source tests also exercise `codex mcp get <name>`, `--json` for list/get, stdio add syntax (`codex mcp add name -- command args...`), and `codex mcp remove <name>`. [OpenAI Docs MCP setup](https://developers.openai.com/learn/docs-mcp) [Codex config basics](https://developers.openai.com/codex/config-basic) [Codex CLI MCP tests](https://github.com/openai/codex/blob/main/codex-rs/cli/tests/mcp_list.rs)

Implementation pattern: use the native command for user scope. For project scope, write the trusted-project `.codex/config.toml` layer only if the installed Codex CLI has no scope flag; preserve existing TOML. Verify with `codex mcp get expander --json` and `codex mcp list --json`. Treat registration as distinct from a successful runtime connection.

### Claude Code

Remote HTTP:

```sh
claude mcp add --transport http --scope user expander https://example.com/mcp
claude mcp get expander
claude mcp list
```

Local stdio:

```sh
claude mcp add --scope user expander -- command arg1 arg2
```

Management commands are `claude mcp list`, `claude mcp get <name>`, and `claude mcp remove <name>`. Scopes are `local` (default: private to the current project), `project` (shared in repository-root `.mcp.json`), and `user` (all projects). A project file has a top-level `mcpServers` object; Claude asks for approval before using project servers. Same-name precedence is local, then project, then user. `${VAR}` and `${VAR:-default}` expansion is supported in command, args, env, URL, and headers. `claude mcp add-json` is useful when the installer already has a structured definition. `/mcp` verifies interactive connection/authentication. [Claude Code MCP documentation](https://code.claude.com/docs/en/mcp) [Anthropic CLI reference](https://docs.anthropic.com/en/docs/claude-code/cli-usage)

Implementation pattern: prefer `add-json` to avoid shell-token ambiguity, pass `--scope` explicitly, run `get` and `list`, and instruct the user to use `/mcp` only when OAuth or interactive connection confirmation is needed.

### Pi

Pi now has first-party built-in MCP support. Stdio and remote examples are:

```sh
pi mcp add expander -- command arg1 arg2
pi mcp add expander --url https://example.com/mcp --bearer-token-env-var EXPANDER_TOKEN
pi mcp list
```

These are user-level by default. `--local`/`-l` writes project configuration instead. Pi reads `~/.pi/agent/mcp.json` and `.pi/mcp.json`; both use a top-level `mcpServers` object. Project config is trust-gated, and a project entry overrides a user entry with the same name. Shell commands include add, remove, list, login, and logout. `pi mcp list` actually connects to every enabled server, prints tools/errors, and exits 1 for invalid entries or enabled servers that fail to connect, making it the strongest native verifier in this set. Pi supports stdio and streamable HTTP, explicitly rejects legacy SSE, and `/mcp` plus `/reload` handle session inspection/reload. [Pi MCP documentation](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/mcp.md) [Pi CLI documentation](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/cli.md) [Pi extension MCP API](https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md#mcp-servers)

Implementation pattern: the requested UX can be exactly `pi mcp add ...`; Expander should detect Pi's version/help first because older releases did not have the same built-in interface. Verify with `pi mcp list` and propagate its exit status.

### Cursor (clear config convention; no native CLI assumed)

Cursor reads `mcp.json`; the documented user location on macOS/Linux is `~/.cursor/mcp.json` with top-level `mcpServers`. A typical remote entry is:

```json
{
  "mcpServers": {
    "expander": { "url": "https://example.com/mcp" }
  }
}
```

The official setup tells users to restart Cursor after mutation. [OpenAI Docs MCP setup for Cursor](https://developers.openai.com/learn/docs-mcp)

Implementation pattern: use an atomic JSON merge, preserve unrelated servers, detect same-name conflicts, and report verification as `unverified` unless Cursor exposes a usable native status interface in the installed version.

## Proposed universal flow

```text
detect candidates -> resolve explicit target/scope -> inspect existing entry
 -> plan/redacted diff -> confirm (unless --yes) -> native add or atomic merge
 -> native get/list -> optional connection/tool check -> structured result
```

Recommended command surface:

```sh
expander mcp add --harness pi --scope user --url https://example.com/mcp
expander mcp add --harness codex --scope project -- command args...
expander mcp add --all-detected --scope user --yes --json
expander mcp add --all-detected --scope user --dry-run --json
expander mcp status [--harness ...] [--json]
expander mcp remove [--harness ...] [--scope ...]
```

Safety/behavior requirements:

- Detection returns evidence and confidence; it never mutates.
- Explicit harness choice wins over detection.
- Ambiguity is an error in noninteractive mode.
- Scope is always explicit in the plan, even when a harness has a default.
- Name collisions are never silently overwritten.
- Commands are spawned as argv arrays, never concatenated shell strings.
- Secrets are represented as environment-variable references and redacted from output.
- Each target is an independent transaction with before/after snapshots; partial success is reported precisely.
- Config fallbacks validate syntax before atomic rename and keep a recoverable backup.
- Verification records the command, exit status, and whether it proves registration only or a live connection.

## Exact primary-source URLs

- https://www.skills.sh/docs
- https://www.skills.sh/docs/cli
- https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/README.md
- https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/agents.ts
- https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/detect-agent.ts
- https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/add.ts
- https://github.com/vercel-labs/skills/blob/3694740352eeef5cdd689af694c485f1ff62eec3/src/installer.ts
- https://developers.openai.com/learn/docs-mcp
- https://developers.openai.com/codex/config-basic
- https://github.com/openai/codex/blob/main/codex-rs/cli/tests/mcp_list.rs
- https://code.claude.com/docs/en/mcp
- https://docs.anthropic.com/en/docs/claude-code/cli-usage
- https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/mcp.md
- https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/cli.md
- https://github.com/badlogic/pi-mono/blob/main/packages/coding-agent/docs/extensions.md
