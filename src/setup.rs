use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CLAUDE_COMMAND: &str = r#"---
description: Save or run a named prompt from Expander MCP
argument-hint: set <name> <prompt> | <name> [arguments] | list | delete <name>
---

Interpret `$ARGUMENTS` as an Expander command:

- `set <name> <prompt>`: call the Expander MCP `set_prompt` tool using all text after the name as the prompt. Confirm what was saved.
- `list`: call `list_prompts` and show the available names concisely.
- `delete <name>`: ask for confirmation unless the user's command already explicitly confirms deletion, then call `delete_prompt`.
- Otherwise, treat the first token as a prompt name and the remaining text as optional arguments. Call `expand_prompt`, then follow the returned prompt as the user's current instruction. Do not merely print or summarize it.

If the arguments are missing or malformed, show: `/xp set <name> <prompt>` or `/xp <name> [arguments]`.
"#;

const CODEX_PROMPT: &str = r#"---
description: Save or run a named prompt from Expander MCP
argument-hint: set <name> <prompt> | <name> [arguments] | list | delete <name>
---

Interpret `$ARGUMENTS` as an Expander command. For `set <name> <prompt>`, call the Expander MCP `set_prompt` tool. For `list`, call `list_prompts`. For `delete <name>`, confirm destructive intent unless already explicit, then call `delete_prompt`. Otherwise call `expand_prompt` with the first token as the name and the remaining text as arguments, then follow the returned prompt as the user's current instruction instead of merely printing it.
"#;

const CODEX_SKILL: &str = r#"---
name: xp
description: Save or run named prompts through Expander MCP. Use when the user invokes $xp, asks to expand a saved prompt, or writes an xp set/list/delete command.
---

# Expander

Interpret the text after `$xp` as follows:

- `set <name> <prompt>`: call `set_prompt` with the complete remaining prompt text.
- `list`: call `list_prompts`.
- `delete <name>`: confirm destructive intent unless the user already explicitly requested deletion, then call `delete_prompt`.
- `<name> [arguments]`: call `expand_prompt`, then execute the returned prompt as the user's current instruction. Do not stop after echoing it.

Keep prompt names to letters, numbers, dots, underscores, and hyphens.
"#;

pub fn run(force: bool) -> Result<(), String> {
    let executable = env::current_exe()
        .map_err(|error| format!("could not locate expander-mcp: {error}"))?
        .canonicalize()
        .map_err(|error| format!("could not resolve expander-mcp path: {error}"))?;

    println!("Configuring Expander MCP from {}", executable.display());
    configure_client("codex", &executable, force)?;
    configure_client("claude", &executable, force)?;

    let home = home_directory()?;
    install_adapter(&home.join(".claude/commands/xp.md"), CLAUDE_COMMAND, force)?;
    install_adapter(&home.join(".codex/prompts/xp.md"), CODEX_PROMPT, force)?;
    install_adapter(&home.join(".codex/skills/xp/SKILL.md"), CODEX_SKILL, force)?;

    println!("\nReady. Restart open agent sessions, then use:");
    println!("  Claude Code: /xp set review-prompt <prompt>");
    println!("  Claude Code: /xp review-prompt");
    println!("  Codex:       /prompts:xp review-prompt  (or $xp review-prompt)");
    Ok(())
}

fn configure_client(client: &str, executable: &Path, force: bool) -> Result<(), String> {
    if Command::new(client).arg("--version").output().is_err() {
        println!("  - {client}: not found; skipped");
        return Ok(());
    }

    let exists = Command::new(client)
        .args(["mcp", "get", "expander"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if exists && !force {
        println!("  - {client}: MCP already configured; kept existing entry");
        return Ok(());
    }
    if exists && force {
        let status = Command::new(client)
            .args(["mcp", "remove", "expander"])
            .status()
            .map_err(|error| format!("could not update {client} MCP config: {error}"))?;
        if !status.success() {
            return Err(format!(
                "{client} could not remove its existing Expander MCP entry"
            ));
        }
    }

    let mut command = Command::new(client);
    command.args(["mcp", "add"]);
    if client == "claude" {
        command.args(["--scope", "user"]);
    }
    let status = command
        .arg("expander")
        .arg("--")
        .arg(executable)
        .arg("serve")
        .status()
        .map_err(|error| format!("could not configure {client}: {error}"))?;
    if !status.success() {
        return Err(format!("{client} rejected the MCP configuration"));
    }
    println!("  - {client}: MCP configured");
    Ok(())
}

fn install_adapter(path: &Path, content: &str, force: bool) -> Result<(), String> {
    if path.exists() && !force {
        println!("  - {}: already exists; kept", path.display());
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid adapter path: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    fs::write(path, content)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    println!("  - {}: installed", path.display());
    Ok(())
}

fn home_directory() -> Result<PathBuf, String> {
    env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| "could not determine the home directory".to_string())
}
