---
name: xp
description: Save and run named prompts through Expander MCP. Use when the user invokes /xp or $xp, asks to expand a saved prompt, or writes an xp set/list/delete command.
---

# Expander

Use the tools from the MCP server named `expander`.

Interpret the text after the `xp` invocation as follows:

- `set <name> <prompt>`: call `set_prompt` with the complete text after the name. Confirm the saved name.
- `list`: call `list_prompts` and show the available names concisely.
- `delete <name>`: confirm destructive intent unless the user already explicitly requested deletion, then call `delete_prompt`.
- `<name> [arguments]`: call `expand_prompt` with the first token as `name` and all remaining text as `arguments`. Treat the returned prompt as the user's current instruction and carry it out. Do not stop after merely printing or summarizing it.

If required arguments are missing, show these usage forms:

```text
/xp set <name> <prompt>
/xp <name> [arguments]
```

Prompt names may contain letters, numbers, dots, underscores, and hyphens.

