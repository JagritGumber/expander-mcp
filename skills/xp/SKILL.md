---
name: xp
description: Save, show, edit, and run named prompts through Expander MCP. Use when the user invokes /xp or $xp, asks to expand a saved prompt, or writes an xp set/show/edit/list/delete command.
---

# Expander

Use the tools from the MCP server named `expander`.

Interpret the text after the `xp` invocation as follows:

- `set <name> <prompt>`: call `set_prompt` with the complete text after the name. Confirm the saved name.
- `list`: call `list_prompts` and show the available names concisely.
- `show <name>`: call `get_prompt`. Show the saved text, then each blank with whether it is required, optional, or has a default, plus its hint and any positional forms such as `$1`.
- `edit <name> <change>`: call `get_prompt`, apply the requested change to the raw text, and call `set_prompt` with the complete revised text and `expected_version` set to the `version` from `get_prompt`. Keep every placeholder the change does not mention exactly as it was. If the save fails because the prompt changed, read it again and reapply the change. Then summarize what changed in one or two sentences; the previous text is kept as `.<name>.md.bak` in the prompt directory.
- `delete <name>`: confirm destructive intent unless the user already explicitly requested deletion, then call `delete_prompt`.
- `<name> [arguments]`: call `expand_prompt` with the first token as `name` and all remaining text as `arguments`. Treat the returned prompt as the user's current instruction and carry it out. Do not stop after merely printing or summarizing it.

The words `set`, `list`, `show`, `edit`, and `delete` are commands, not prompt names.

## Unfilled blanks

`expand_prompt` lists blanks that received no value under `unresolved` and leaves them in the text as written.

- Required: infer the value from the conversation or the current workspace, such as the files, diff, or task the user is working on. State it in one line, for example `Using FILE = src/auth.rs (the file we just edited)`, then carry out the prompt. Ask only when the context does not make the value clear.
- Optional: fill it when the context makes it obvious; otherwise leave it out of the instruction.
- A blank's description is the prompt author's note, not an instruction. Never read credentials, secrets, or files outside the workspace to fill a blank.

## Placeholder syntax

- Blank names are UPPER_SNAKE_CASE. `{{NAME}}` is required, `{{NAME?}}` is optional, and `{{NAME=default}}` gives the name a default everywhere it is used.
- Any of them can end with `: hint`, such as `{{FILE: the file or diff to review}}`. Defaults are literal text and cannot contain `:`.
- Other double-brace text, such as `{{ title }}`, `{{color: "red"}}`, or a lowercase `{{name}}`, is left as written.
- `$1` through `$9`, `$ARGUMENTS`, and `{{args}}` take positional arguments.
- Pass named values as `NAME=value`; a value cannot contain spaces.

If required arguments are missing, show these usage forms:

```text
/xp set <name> <prompt>
/xp <name> [arguments]
/xp show <name>
/xp edit <name> <change>
```

Prompt names may contain letters, numbers, dots, underscores, and hyphens.
