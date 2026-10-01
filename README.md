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

The installer downloads a checksum-verified native binary and runs `expander-mcp setup`. Setup detects supported harnesses, registers the MCP server through their native interface, installs the portable `xp` skill, and inspects the resulting registrations. Restart agent sessions that were already open.

You can rerun setup at any time:

```sh
expander-mcp setup
```

Target a particular harness, preview every action without writing, or consume structured results:

```sh
expander-mcp setup --client pi
expander-mcp setup --dry-run
expander-mcp setup --dry-run --json
```

Built-in adapters cover Codex, Claude Code, Pi, Gemini CLI, OpenCode, and Cursor. An explicitly requested missing or unverifiable client is an error instead of a silent skip. Other CLIs that follow the common `mcp add/list/remove` shape can be targeted by executable name:

```sh
expander-mcp setup --client my-agent
```

MCP standardizes how the agent and server communicate; each harness still owns its installation command. When a harness uses a different CLI shape, use its equivalent of `mcp add expander -- expander-mcp serve` or add the generic stdio configuration shown below.

## Install the `xp` skill into other harnesses

Expander is also a standard Agent Skill, so [skills.sh](https://skills.sh) can install the interaction layer into its supported coding agents:

```sh
npx skills add JagritGumber/expander-mcp --skill xp --global
```

This interactively detects and selects installed harnesses. MCP and Agent Skills solve different layers: the skill teaches the agent what `xp` means, while the MCP registration provides the four local tools.

## Use

| Intent | Skill-aware harness | Codex |
|---|---|---|
| Save | `/xp set review-prompt <prompt>` | `$xp set review-prompt <prompt>` |
| Run | `/xp review-prompt` | `$xp review-prompt` |
| List | `/xp list` | `$xp list` |
| Delete | `/xp delete review-prompt` | `$xp delete review-prompt` |

Invocation syntax is owned by each harness, not MCP. Codex uses the modern `$xp` skill syntax and retains `/prompts:xp` as a compatibility adapter; Claude Code exposes the skill as `/xp`.

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
pi mcp add expander -- expander-mcp serve
gemini mcp add --scope user expander expander-mcp serve
opencode mcp add expander --global -- expander-mcp serve
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
