use serde::Serialize;
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const XP_SKILL: &str = include_str!("../skills/xp/SKILL.md");

const CODEX_PROMPT: &str = r#"---
description: Save or run a named prompt from Expander MCP
argument-hint: set <name> <prompt> | <name> [arguments] | list | delete <name>
---

Use the `xp` skill to interpret and execute this command: `$ARGUMENTS`
"#;

const LEGACY_CODEX_PROMPT: &str = r#"---
description: Save or run a named prompt from Expander MCP
argument-hint: set <name> <prompt> | <name> [arguments] | list | delete <name>
---

Interpret `$ARGUMENTS` as an Expander command. For `set <name> <prompt>`, call the Expander MCP `set_prompt` tool. For `list`, call `list_prompts`. For `delete <name>`, confirm destructive intent unless already explicit, then call `delete_prompt`. Otherwise call `expand_prompt` with the first token as the name and the remaining text as arguments, then follow the returned prompt as the user's current instruction instead of merely printing it.
"#;

const LEGACY_CODEX_SKILL: &str = r#"---
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

const LEGACY_CLAUDE_COMMAND: &str = r#"---
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

const KNOWN_CLIENTS: &[&str] = &["codex", "claude", "pi", "gemini", "opencode", "cursor"];

#[derive(Debug)]
struct SetupOptions {
    force: bool,
    dry_run: bool,
    json: bool,
    clients: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SetupResult {
    target: String,
    status: &'static str,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<Vec<String>>,
}

struct ClientSpec {
    id: String,
    binary: String,
    add: Vec<String>,
    inspect: Vec<String>,
    remove: Vec<String>,
}

pub fn run(arguments: &[String]) -> Result<(), String> {
    let options = parse_options(arguments)?;
    let executable = env::current_exe()
        .map_err(|error| format!("could not locate expander-mcp: {error}"))?
        .canonicalize()
        .map_err(|error| format!("could not resolve expander-mcp path: {error}"))?;
    let executable_string = executable.to_string_lossy().into_owned();

    let explicit = !options.clients.is_empty();
    let requested = if explicit {
        options.clients.clone()
    } else {
        KNOWN_CLIENTS
            .iter()
            .map(|client| (*client).to_string())
            .collect()
    };

    if !options.json {
        let mode = if options.dry_run {
            "Planning"
        } else {
            "Configuring"
        };
        println!("{mode} Expander MCP from {}", executable.display());
    }

    let mut results = Vec::new();
    for client in requested {
        let result = if client == "cursor" {
            configure_cursor(&executable_string, &options, explicit)?
        } else {
            let spec = client_spec(&client, &executable_string);
            configure_cli_client(spec, &options, explicit)
        };
        results.push(result);
    }
    let client_result_count = results.len();

    install_skill_adapters(&options, &mut results)?;
    emit_results(&results, options.json)?;

    let failed = results.iter().any(|result| result.status == "failed");
    let configured = results
        .iter()
        .take(client_result_count)
        .any(|result| matches!(result.status, "installed" | "unchanged" | "planned"));
    if failed {
        return Err("one or more requested targets failed; see the setup results above".into());
    }
    if explicit && !configured {
        return Err("none of the explicitly requested MCP clients could be configured".into());
    }

    if !options.json && !options.dry_run {
        println!("\nReady. Restart open agent sessions, then use:");
        println!("  Claude Code and compatible harnesses: /xp review-prompt");
        println!("  Codex: $xp review-prompt (or /prompts:xp review-prompt)");
        println!(
            "  Any other stdio MCP client: add 'expander' with '{} serve'",
            executable.display()
        );
    }
    Ok(())
}

fn parse_options(arguments: &[String]) -> Result<SetupOptions, String> {
    let mut options = SetupOptions {
        force: false,
        dry_run: false,
        json: false,
        clients: Vec::new(),
    };
    let mut index = 0;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--force" | "--replace" => options.force = true,
            "--dry-run" => options.dry_run = true,
            "--json" => options.json = true,
            "--client" | "--harness" => {
                index += 1;
                let client = arguments
                    .get(index)
                    .ok_or("--client requires a client name")?;
                if client == "all" {
                    options.clients = KNOWN_CLIENTS
                        .iter()
                        .map(|value| (*value).to_string())
                        .collect();
                } else if !options.clients.contains(client) {
                    options.clients.push(client.clone());
                }
            }
            value => return Err(format!("unknown setup option '{value}'")),
        }
        index += 1;
    }
    Ok(options)
}

fn client_spec(client: &str, executable: &str) -> ClientSpec {
    match client {
        "codex" => ClientSpec {
            id: "codex".into(),
            binary: "codex".into(),
            add: strings(&["mcp", "add", "expander", "--", executable, "serve"]),
            inspect: strings(&["mcp", "get", "expander"]),
            remove: strings(&["mcp", "remove", "expander"]),
        },
        "claude" | "claude-code" => ClientSpec {
            id: "claude".into(),
            binary: "claude".into(),
            add: strings(&[
                "mcp", "add", "--scope", "user", "expander", "--", executable, "serve",
            ]),
            inspect: strings(&["mcp", "get", "expander"]),
            remove: strings(&["mcp", "remove", "--scope", "user", "expander"]),
        },
        "pi" => ClientSpec {
            id: "pi".into(),
            binary: "pi".into(),
            add: strings(&["mcp", "add", "expander", "--", executable, "serve"]),
            inspect: strings(&["mcp", "list"]),
            remove: strings(&["mcp", "remove", "expander"]),
        },
        "gemini" | "gemini-cli" => ClientSpec {
            id: "gemini".into(),
            binary: "gemini".into(),
            add: strings(&[
                "mcp", "add", "--scope", "user", "expander", executable, "serve",
            ]),
            inspect: strings(&["mcp", "list"]),
            remove: strings(&["mcp", "remove", "--scope", "user", "expander"]),
        },
        "opencode" => ClientSpec {
            id: "opencode".into(),
            binary: "opencode".into(),
            add: strings(&[
                "mcp", "add", "expander", "--global", "--", executable, "serve",
            ]),
            inspect: strings(&["mcp", "list"]),
            remove: strings(&["mcp", "remove", "expander", "--global"]),
        },
        other => ClientSpec {
            id: other.to_string(),
            binary: other.to_string(),
            add: strings(&["mcp", "add", "expander", "--", executable, "serve"]),
            inspect: strings(&["mcp", "list"]),
            remove: strings(&["mcp", "remove", "expander"]),
        },
    }
}

fn configure_cli_client(spec: ClientSpec, options: &SetupOptions, explicit: bool) -> SetupResult {
    if !binary_in_path(&spec.binary) {
        return SetupResult {
            target: spec.id.clone(),
            status: if explicit { "failed" } else { "skipped" },
            detail: format!("{} executable was not found", spec.binary),
            command: Some(command_vector(&spec.binary, &spec.add)),
        };
    }

    if options.dry_run {
        return SetupResult {
            target: spec.id.clone(),
            status: "planned",
            detail: "would inspect the current state, then add or preserve the user-level stdio registration".into(),
            command: Some(command_vector(&spec.binary, &spec.add)),
        };
    }

    let inspection = run_command(&spec.binary, &spec.inspect);
    let exists = inspection
        .as_ref()
        .is_ok_and(|output| output.status.success() && output_text(output).contains("expander"));
    if exists && !options.force {
        return SetupResult {
            target: spec.id.clone(),
            status: "unchanged",
            detail: "registration found through the client's MCP interface".into(),
            command: None,
        };
    }

    if exists && options.force {
        if let Err(message) = successful_command(&spec.binary, &spec.remove) {
            return failed_result(&spec.id, message, &spec.binary, &spec.remove);
        }
    }
    if let Err(message) = successful_command(&spec.binary, &spec.add) {
        return failed_result(&spec.id, message, &spec.binary, &spec.add);
    }

    match run_command(&spec.binary, &spec.inspect) {
        Ok(output) if output.status.success() && output_text(&output).contains("expander") => {
            SetupResult {
                target: spec.id.clone(),
                status: "installed",
                detail: "registration verified through the client's MCP interface".into(),
                command: Some(command_vector(&spec.binary, &spec.add)),
            }
        }
        Ok(output) => failed_result(
            &spec.id,
            format!(
                "registration could not be verified: {}",
                output_text(&output).trim()
            ),
            &spec.binary,
            &spec.inspect,
        ),
        Err(message) => failed_result(&spec.id, message, &spec.binary, &spec.inspect),
    }
}

fn configure_cursor(
    executable: &str,
    options: &SetupOptions,
    explicit: bool,
) -> Result<SetupResult, String> {
    let home = home_directory()?;
    let path = home.join(".cursor/mcp.json");
    let detected = path.parent().is_some_and(Path::exists)
        || binary_in_path("cursor")
        || binary_in_path("cursor-agent");
    if !detected && !explicit {
        return Ok(SetupResult {
            target: "cursor".into(),
            status: "skipped",
            detail: "Cursor was not detected".into(),
            command: None,
        });
    }

    let mut document = if path.exists() {
        serde_json::from_str::<Value>(
            &fs::read_to_string(&path)
                .map_err(|error| format!("could not read {}: {error}", path.display()))?,
        )
        .map_err(|error| format!("{} is not valid JSON: {error}", path.display()))?
    } else {
        json!({})
    };
    let root = document
        .as_object_mut()
        .ok_or_else(|| format!("{} must contain a JSON object", path.display()))?;
    let servers = root
        .entry("mcpServers")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| format!("mcpServers in {} must be a JSON object", path.display()))?;
    let desired = json!({"command":executable,"args":["serve"]});

    if servers.get("expander") == Some(&desired) {
        return Ok(SetupResult {
            target: "cursor".into(),
            status: "unchanged",
            detail: format!("registration already matches in {}", path.display()),
            command: None,
        });
    }
    if servers.contains_key("expander") && !options.force {
        return Ok(SetupResult {
            target: "cursor".into(),
            status: "failed",
            detail: format!(
                "a different expander entry exists in {}; use --force to replace it",
                path.display()
            ),
            command: None,
        });
    }
    if options.dry_run {
        return Ok(SetupResult {
            target: "cursor".into(),
            status: "planned",
            detail: format!("would atomically merge expander into {}", path.display()),
            command: None,
        });
    }

    servers.insert("expander".into(), desired);
    write_json_atomically(&path, &document)?;
    Ok(SetupResult {
        target: "cursor".into(),
        status: "installed",
        detail: format!(
            "registered in {}; restart Cursor to connect",
            path.display()
        ),
        command: None,
    })
}

fn install_skill_adapters(
    options: &SetupOptions,
    results: &mut Vec<SetupResult>,
) -> Result<(), String> {
    let home = home_directory()?;
    let paths = [
        home.join(".agents/skills/xp/SKILL.md"),
        home.join(".claude/skills/xp/SKILL.md"),
        home.join(".codex/skills/xp/SKILL.md"),
    ];
    for path in paths {
        results.push(install_file(
            "xp-skill",
            &path,
            XP_SKILL,
            &[LEGACY_CODEX_SKILL],
            options,
        )?);
    }
    results.push(install_file(
        "codex-xp-command",
        &home.join(".codex/prompts/xp.md"),
        CODEX_PROMPT,
        &[LEGACY_CODEX_PROMPT],
        options,
    )?);

    let legacy = home.join(".claude/commands/xp.md");
    if !options.dry_run && legacy.exists() {
        let content = fs::read_to_string(&legacy)
            .map_err(|error| format!("could not inspect {}: {error}", legacy.display()))?;
        if content == LEGACY_CLAUDE_COMMAND {
            fs::remove_file(&legacy).map_err(|error| {
                format!("could not remove legacy {}: {error}", legacy.display())
            })?;
            results.push(SetupResult {
                target: "legacy-claude-command".into(),
                status: "replaced",
                detail: format!(
                    "removed managed legacy adapter {}; the xp skill replaces it",
                    legacy.display()
                ),
                command: None,
            });
        }
    }
    Ok(())
}

fn install_file(
    target: &str,
    path: &Path,
    content: &str,
    managed_previous: &[&str],
    options: &SetupOptions,
) -> Result<SetupResult, String> {
    let existed = path.exists();
    if existed {
        let existing = fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        if existing == content {
            return Ok(SetupResult {
                target: format!("{target}:{}", path.display()),
                status: "unchanged",
                detail: "content already matches".into(),
                command: None,
            });
        }
        if !options.force && !managed_previous.contains(&existing.as_str()) {
            return Ok(SetupResult {
                target: format!("{target}:{}", path.display()),
                status: "failed",
                detail: "different content exists; use --force to replace it".into(),
                command: None,
            });
        }
    }
    if options.dry_run {
        return Ok(SetupResult {
            target: format!("{target}:{}", path.display()),
            status: "planned",
            detail: "would install the portable xp skill adapter".into(),
            command: None,
        });
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid adapter path: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    fs::write(path, content)
        .map_err(|error| format!("could not write {}: {error}", path.display()))?;
    Ok(SetupResult {
        target: format!("{target}:{}", path.display()),
        status: if existed { "replaced" } else { "installed" },
        detail: "portable xp skill adapter installed".into(),
        command: None,
    })
}

fn write_json_atomically(path: &Path, document: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("invalid config path: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let data = serde_json::to_string_pretty(document)
        .map_err(|error| format!("could not encode Cursor configuration: {error}"))?;
    fs::write(&temporary, format!("{data}\n"))
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    if path.exists() {
        fs::copy(path, path.with_extension("json.bak"))
            .map_err(|error| format!("could not back up {}: {error}", path.display()))?;
    }
    if let Err(first_error) = fs::rename(&temporary, path) {
        if path.exists() {
            fs::remove_file(path)
                .map_err(|error| format!("could not replace {}: {error}", path.display()))?;
            fs::rename(&temporary, path).map_err(|error| {
                format!(
                    "could not install {} after rename failed ({first_error}): {error}",
                    path.display()
                )
            })?;
        } else {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "could not install {}: {first_error}",
                path.display()
            ));
        }
    }
    Ok(())
}

fn emit_results(results: &[SetupResult], json_output: bool) -> Result<(), String> {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(results)
                .map_err(|error| format!("could not encode setup results: {error}"))?
        );
        return Ok(());
    }
    for result in results {
        let icon = match result.status {
            "installed" | "unchanged" => "✓",
            "planned" => "→",
            "replaced" => "↻",
            "skipped" => "-",
            _ => "✗",
        };
        println!(
            "  {icon} {:<28} {:<9} {}",
            result.target, result.status, result.detail
        );
        if let Some(command) = &result.command {
            println!("      {}", command.join(" "));
        }
    }
    Ok(())
}

fn failed_result(id: &str, detail: String, binary: &str, arguments: &[String]) -> SetupResult {
    SetupResult {
        target: id.to_string(),
        status: "failed",
        detail,
        command: Some(command_vector(binary, arguments)),
    }
}

fn successful_command(binary: &str, arguments: &[String]) -> Result<(), String> {
    let output = run_command(binary, arguments)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("command failed: {}", output_text(&output).trim()))
    }
}

fn run_command(binary: &str, arguments: &[String]) -> Result<Output, String> {
    Command::new(binary)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run {binary}: {error}"))
}

fn binary_in_path(binary: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&path).any(|directory| {
        let candidate = directory.join(binary);
        candidate.is_file() || (cfg!(windows) && directory.join(format!("{binary}.exe")).is_file())
    })
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn command_vector(binary: &str, arguments: &[String]) -> Vec<String> {
    std::iter::once(binary.to_string())
        .chain(arguments.iter().cloned())
        .collect()
}

fn home_directory() -> Result<PathBuf, String> {
    env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| "could not determine the home directory".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_repeatable_and_generic_client_targets() {
        let arguments = strings(&[
            "--client",
            "pi",
            "--harness",
            "my-agent",
            "--dry-run",
            "--json",
        ]);
        let options = parse_options(&arguments).unwrap();

        assert_eq!(options.clients, ["pi", "my-agent"]);
        assert!(options.dry_run);
        assert!(options.json);
        assert!(!options.force);
    }

    #[test]
    fn pi_uses_native_stdio_registration_shape() {
        let spec = client_spec("pi", "/opt/expander-mcp");

        assert_eq!(
            spec.add,
            strings(&["mcp", "add", "expander", "--", "/opt/expander-mcp", "serve"])
        );
        assert_eq!(spec.inspect, strings(&["mcp", "list"]));
    }
}
