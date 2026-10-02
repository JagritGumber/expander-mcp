mod mcp;
mod setup;
mod store;
mod template;

use std::env;
use std::io::{self, Read};
use std::process::ExitCode;

use store::PromptStore;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("expander-mcp: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "serve".to_string());
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "serve" => mcp::serve(PromptStore::from_environment()?),
        "setup" => setup::run(&rest),
        "set" => cli_set(&rest),
        "get" | "expand" => cli_get(&rest),
        "show" => cli_show(&rest),
        "list" | "ls" => cli_list(),
        "delete" | "remove" | "rm" => cli_delete(&rest),
        "path" => {
            println!("{}", PromptStore::from_environment()?.root().display());
            Ok(())
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("expander-mcp {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        unknown => Err(format!(
            "unknown command '{unknown}'. Run 'expander-mcp help'."
        )),
    }
}

fn cli_set(args: &[String]) -> Result<(), String> {
    let Some(name) = args.first() else {
        return Err("usage: expander-mcp set <name> <prompt...> (or pipe prompt on stdin)".into());
    };

    let content = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        let mut content = String::new();
        io::stdin()
            .read_to_string(&mut content)
            .map_err(|error| format!("could not read prompt from stdin: {error}"))?;
        content
    };

    if content.trim().is_empty() {
        return Err("prompt content cannot be empty".into());
    }

    PromptStore::from_environment()?.set(name, &content)?;
    println!("Saved '{name}'.");
    Ok(())
}

fn cli_get(args: &[String]) -> Result<(), String> {
    let Some(name) = args.first() else {
        return Err("usage: expander-mcp get <name> [arguments...]".into());
    };
    let prompt = PromptStore::from_environment()?.get(name)?;
    let arguments = args.get(1..).unwrap_or_default().join(" ");
    let expansion = template::expand(&prompt, &arguments);
    print!("{}", expansion.text);
    if !expansion.text.ends_with('\n') {
        println!();
    }
    if !expansion.unresolved.is_empty() {
        let blanks: Vec<String> = expansion
            .unresolved
            .iter()
            .map(|blank| {
                let kind = if blank.required {
                    "required"
                } else {
                    "optional"
                };
                format!("{} ({kind})", blank.name)
            })
            .collect();
        eprintln!("unfilled: {} (pass NAME=value to fill)", blanks.join(", "));
    }
    Ok(())
}

fn cli_show(args: &[String]) -> Result<(), String> {
    let Some(name) = args.first() else {
        return Err("usage: expander-mcp show <name>".into());
    };
    let prompt = PromptStore::from_environment()?.get(name)?;
    print!("{prompt}");
    if !prompt.ends_with('\n') {
        println!();
    }

    let blanks = template::placeholders(&prompt);
    let positional = template::positional(&prompt);
    if blanks.is_empty() && positional.is_empty() {
        println!("\nNo blanks.");
        return Ok(());
    }
    if !blanks.is_empty() {
        println!("\nBlanks:");
        let rows: Vec<(&str, String, &str)> = blanks
            .iter()
            .map(|blank| {
                let kind = match (&blank.default, blank.required) {
                    (Some(default), _) => format!("default: {default}"),
                    (None, true) => "required".to_string(),
                    (None, false) => "optional".to_string(),
                };
                (
                    blank.name.as_str(),
                    kind,
                    blank.hint.as_deref().unwrap_or_default(),
                )
            })
            .collect();
        let name_width = rows.iter().map(|row| row.0.len()).max().unwrap_or(0);
        let kind_width = rows
            .iter()
            .map(|row| row.1.chars().count())
            .max()
            .unwrap_or(0);
        for (name, kind, hint) in rows {
            let line = format!("  {name:<name_width$}  {kind:<kind_width$}  {hint}");
            println!("{}", line.trim_end());
        }
    }
    if !positional.is_empty() {
        println!("\nPositional: {}", positional.join(", "));
    }
    Ok(())
}

fn cli_list() -> Result<(), String> {
    for prompt in PromptStore::from_environment()?.list()? {
        if prompt.description.is_empty() {
            println!("{}", prompt.name);
        } else {
            println!("{:<24} {}", prompt.name, prompt.description);
        }
    }
    Ok(())
}

fn cli_delete(args: &[String]) -> Result<(), String> {
    let Some(name) = args.first() else {
        return Err("usage: expander-mcp delete <name>".into());
    };
    PromptStore::from_environment()?.delete(name)?;
    println!("Deleted '{name}'.");
    Ok(())
}

fn print_help() {
    println!(
        "expander-mcp {version}\n\
         Fast, local-first saved prompts for MCP agents.\n\n\
         Usage:\n\
           expander-mcp setup [options]       Detect, configure, and verify agent clients\n\
           expander-mcp set <name> <prompt>   Save or replace a prompt\n\
           expander-mcp get <name> [args]     Print an expanded prompt\n\
           expander-mcp show <name>           Print a prompt's template and its blanks\n\
           expander-mcp list                  List saved prompts\n\
           expander-mcp delete <name>         Delete a prompt\n\
           expander-mcp path                  Print the prompt directory\n\
           expander-mcp serve                 Run the stdio MCP server\n\n\
         Agent shortcuts:\n\
           /xp set review-prompt Review this change for regressions.\n\
           /xp review-prompt\n\
           /xp show review-prompt\n\n\
         Setup options:\n\
           --client <name>  Target a specific harness (repeatable)\n\
           --dry-run        Print a zero-write installation plan\n\
           --json           Emit machine-readable setup results\n\
           --force          Replace conflicting managed entries",
        version = env!("CARGO_PKG_VERSION")
    );
}
