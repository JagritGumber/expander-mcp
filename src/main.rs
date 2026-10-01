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
        "setup" => setup::run(rest.iter().any(|arg| arg == "--force")),
        "set" => cli_set(&rest),
        "get" | "expand" => cli_get(&rest),
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
    let expanded = template::expand(&prompt, &arguments);
    print!("{expanded}");
    if !expanded.ends_with('\n') {
        println!();
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
           expander-mcp setup [--force]       Configure detected agent clients\n\
           expander-mcp set <name> <prompt>   Save or replace a prompt\n\
           expander-mcp get <name> [args]     Print an expanded prompt\n\
           expander-mcp list                  List saved prompts\n\
           expander-mcp delete <name>         Delete a prompt\n\
           expander-mcp path                  Print the prompt directory\n\
           expander-mcp serve                 Run the stdio MCP server\n\n\
         Agent shortcuts:\n\
           /xp set review-prompt Review this change for regressions.\n\
           /xp review-prompt",
        version = env!("CARGO_PKG_VERSION")
    );
}
