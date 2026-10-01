# Expander MCP

Save a long prompt once. Run it in any MCP-capable agent with a tiny command.

```text
/xp set review-prompt Review the current changes for correctness, regressions,
security issues, and missing tests. Cite exact files and lines.

/xp review-prompt
```

Expander stores prompts locally as Markdown, retrieves them through MCP, and tells the agent to follow the expanded prompt as the current instruction. It is a small native binary: no daemon, database, runtime, account, cloud sync, or telemetry.

## Install

Linux and macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/JagritGumber/expander-mcp/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/JagritGumber/expander-mcp/main/install.ps1 | iex
```

The installer downloads the matching release binary and runs `expander-mcp setup`. Setup detects Codex and Claude Code, registers the MCP server, and installs their `xp` adapters. Restart any agent sessions that were already open.

You can rerun setup at any time:

```sh
expander-mcp setup
```

## Use

| Intent | Claude Code | Codex |
|---|---|---|
| Save | `/xp set review-prompt <prompt>` | `/prompts:xp set review-prompt <prompt>` |
| Run | `/xp review-prompt` | `/prompts:xp review-prompt` |
| Modern Codex skill | — | `$xp review-prompt` |
| List | `/xp list` | `/prompts:xp list` |
| Delete | `/xp delete review-prompt` | `/prompts:xp delete review-prompt` |

Codex reserves its slash-command namespace and currently exposes local custom commands as `/prompts:<name>`. Expander also installs the modern `$xp` skill. Claude Code supports the exact `/xp` spelling.

### Arguments and templates

Saved prompts can use these placeholders:

- `$ARGUMENTS` or `{{args}}` — all invocation arguments
- `$1` through `$9` — whitespace-separated positional arguments
- `{{NAME}}` — a `NAME=value` argument

```text
/xp set focused-review Review $1 with emphasis on {{FOCUS}}.
/xp focused-review src/auth.rs FOCUS=security
```

## Terminal CLI

The same library is available without an agent:

```sh
expander-mcp set review-prompt "Review this change carefully."
expander-mcp list
expander-mcp get review-prompt
expander-mcp delete review-prompt
expander-mcp path
```

Pipe multiline prompts into `set`:

```sh
expander-mcp set review-prompt < review-prompt.md
```

Prompts live in `$XDG_DATA_HOME/expander-mcp/prompts` or `~/.local/share/expander-mcp/prompts`. Set `EXPANDER_MCP_DIR` to use another directory or a synced folder.

## MCP tools

- `set_prompt(name, prompt)`
- `expand_prompt(name, arguments?)`
- `list_prompts()`
- `delete_prompt(name)`

The server also implements MCP `prompts/list` and `prompts/get`, so clients with native prompt browsing can discover saved prompts directly.

### Manual client configuration

```sh
codex mcp add expander -- expander-mcp serve
claude mcp add --scope user expander -- expander-mcp serve
```

Generic stdio configuration:

```json
{
  "mcpServers": {
    "expander": {
      "command": "expander-mcp",
      "args": ["serve"]
    }
  }
}
```

## Build

```sh
cargo build --release
cargo test
```

Requires Rust 1.85 or newer. The server uses newline-delimited JSON-RPC over stdio and has only `serde` and `serde_json` as dependencies.

## Privacy and safety

- Prompts never leave your computer through Expander.
- Expander does not execute prompt text; the connected agent decides how to act on it under that agent's normal permissions.
- Prompt names are restricted to safe filename characters.
- Deletion is a separate MCP tool and the installed adapters require explicit intent.

## License

MIT

