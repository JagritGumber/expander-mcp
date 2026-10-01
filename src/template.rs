use std::collections::HashMap;

pub fn expand(prompt: &str, arguments: &str) -> String {
    let mut result = prompt
        .replace("$ARGUMENTS", arguments)
        .replace("{{args}}", arguments);
    let tokens: Vec<&str> = arguments.split_whitespace().collect();

    for index in (1..=9).rev() {
        let value = tokens.get(index - 1).copied().unwrap_or_default();
        result = result.replace(&format!("${index}"), value);
    }

    let named: HashMap<&str, &str> = tokens
        .iter()
        .filter_map(|token| token.split_once('='))
        .filter(|(key, _)| {
            !key.is_empty()
                && key
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
        .collect();
    for (key, value) in named {
        result = result.replace(&format!("{{{{{key}}}}}"), value);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_whole_positional_and_named_arguments() {
        let prompt = "Whole={{args}} first=$1 second=$2 focus={{FOCUS}}";
        assert_eq!(
            expand(prompt, "alpha beta FOCUS=security"),
            "Whole=alpha beta FOCUS=security first=alpha second=beta focus=security"
        );
    }

    #[test]
    fn leaves_plain_prompts_unchanged() {
        assert_eq!(
            expand("Review this carefully.", ""),
            "Review this carefully."
        );
    }
}
