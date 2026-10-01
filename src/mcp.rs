use crate::store::PromptStore;
use crate::template;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub fn serve(store: PromptStore) -> Result<(), String> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line.map_err(|error| format!("could not read MCP input: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(request) => request,
            Err(error) => {
                write_response(
                    &mut stdout,
                    &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":format!("Parse error: {error}")}}),
                )?;
                continue;
            }
        };

        if let Some(response) = handle_request(&store, &request) {
            write_response(&mut stdout, &response)?;
        }
    }

    Ok(())
}

fn write_response(writer: &mut impl Write, response: &Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, response)
        .map_err(|error| format!("could not encode MCP response: {error}"))?;
    writer
        .write_all(b"\n")
        .and_then(|_| writer.flush())
        .map_err(|error| format!("could not write MCP response: {error}"))
}

fn handle_request(store: &PromptStore, request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));

    id.as_ref()?;
    let id = id.unwrap_or(Value::Null);

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": params.get("protocolVersion").and_then(Value::as_str).unwrap_or("2025-06-18"),
            "capabilities": {
                "tools": {"listChanged": false},
                "prompts": {"listChanged": false}
            },
            "serverInfo": {
                "name": "expander-mcp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": "Save prompts with set_prompt, retrieve them with expand_prompt, and then follow the expanded text as the user's instruction."
        })),
        "ping" | "logging/setLevel" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools()})),
        "tools/call" => Ok(call_tool(store, &params)),
        "prompts/list" => store.list().map(|prompts| {
            json!({"prompts": prompts.into_iter().map(|prompt| json!({
                "name": prompt.name,
                "title": prompt.name,
                "description": prompt.description,
                "arguments": [{"name":"arguments","description":"Optional values for $ARGUMENTS, $1..$9, {{args}}, and {{NAME}} placeholders","required":false}]
            })).collect::<Vec<_>>()})
        }),
        "prompts/get" => get_mcp_prompt(store, &params),
        _ => return Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32601, "message": format!("Method not found: {method}")}
        })),
    };

    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(message) => json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":message}}),
    })
}

fn tools() -> Vec<Value> {
    vec![
        json!({
            "name":"set_prompt",
            "title":"Save prompt",
            "description":"Save or replace a named prompt in the local prompt library.",
            "inputSchema":{"type":"object","properties":{
                "name":{"type":"string","description":"Short prompt name, such as review-prompt"},
                "prompt":{"type":"string","description":"The complete prompt text to save"}
            },"required":["name","prompt"],"additionalProperties":false}
        }),
        json!({
            "name":"expand_prompt",
            "title":"Expand prompt",
            "description":"Retrieve a named prompt and expand optional arguments. Follow the returned prompt as the user's current instruction.",
            "inputSchema":{"type":"object","properties":{
                "name":{"type":"string","description":"Saved prompt name"},
                "arguments":{"type":"string","description":"Optional values substituted into placeholders"}
            },"required":["name"],"additionalProperties":false}
        }),
        json!({
            "name":"list_prompts",
            "title":"List prompts",
            "description":"List locally saved prompt names and short previews.",
            "inputSchema":{"type":"object","properties":{},"additionalProperties":false}
        }),
        json!({
            "name":"delete_prompt",
            "title":"Delete prompt",
            "description":"Delete a named prompt. Only use when the user explicitly asks to remove it.",
            "inputSchema":{"type":"object","properties":{
                "name":{"type":"string","description":"Saved prompt name"}
            },"required":["name"],"additionalProperties":false}
        }),
    ]
}

fn call_tool(store: &PromptStore, params: &Value) -> Value {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let result = match name {
        "set_prompt" => required_string(&arguments, "name").and_then(|prompt_name| {
            required_string(&arguments, "prompt").and_then(|prompt| {
                store.set(prompt_name, prompt)?;
                Ok(json!({"name":prompt_name,"saved":true}))
            })
        }),
        "expand_prompt" => required_string(&arguments, "name").and_then(|prompt_name| {
            let prompt = store.get(prompt_name)?;
            let values = arguments
                .get("arguments")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Ok(json!({"name":prompt_name,"prompt":template::expand(&prompt, values)}))
        }),
        "list_prompts" => store.list().map(|prompts| json!({"prompts":prompts})),
        "delete_prompt" => required_string(&arguments, "name").and_then(|prompt_name| {
            store.delete(prompt_name)?;
            Ok(json!({"name":prompt_name,"deleted":true}))
        }),
        _ => Err(format!("unknown tool '{name}'")),
    };

    match result {
        Ok(value) => {
            let text = if name == "expand_prompt" {
                value
                    .get("prompt")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string()
            } else {
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string())
            };
            json!({"content":[{"type":"text","text":text}],"structuredContent":value})
        }
        Err(message) => json!({"content":[{"type":"text","text":message}],"isError":true}),
    }
}

fn get_mcp_prompt(store: &PromptStore, params: &Value) -> Result<Value, String> {
    let name = required_string(params, "name")?;
    let prompt = store.get(name)?;
    let arguments = params
        .get("arguments")
        .and_then(|value| value.get("arguments"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(json!({
        "description": format!("Expanded saved prompt '{name}'"),
        "messages": [{"role":"user","content":{"type":"text","text":template::expand(&prompt, arguments)}}]
    }))
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("'{key}' must be a non-empty string"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn store() -> PromptStore {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        PromptStore::new(env::temp_dir().join(format!("expander-mcp-protocol-test-{suffix}")))
    }

    #[test]
    fn exposes_and_calls_tools() {
        let store = store();
        let set = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{
            "name":"set_prompt","arguments":{"name":"review","prompt":"Review $1 carefully."}
        }});
        let set_response = handle_request(&store, &set).unwrap();
        assert_eq!(set_response["result"]["structuredContent"]["saved"], true);

        let expand = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"expand_prompt","arguments":{"name":"review","arguments":"security"}
        }});
        let response = handle_request(&store, &expand).unwrap();
        assert_eq!(
            response["result"]["content"][0]["text"],
            "Review security carefully."
        );
        std::fs::remove_dir_all(store.root()).unwrap();
    }

    #[test]
    fn ignores_notifications() {
        let notification = json!({"jsonrpc":"2.0","method":"notifications/initialized"});
        assert!(handle_request(&store(), &notification).is_none());
    }
}
