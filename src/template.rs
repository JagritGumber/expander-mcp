use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Longest `{{...}}` the scanner looks across, which keeps scanning linear in the prompt size.
const MAX_BRACES: usize = 256;

/// A named blank in a saved prompt.
///
/// Blank names are UPPER_SNAKE_CASE. `{{NAME}}` is required, `{{NAME?}}` is optional,
/// `{{NAME=value}}` gives `NAME` a default everywhere it is used, and any form may end with
/// `: hint`. Defaults are literal text and cannot contain `:`. Other double-brace text, such
/// as `{{ title }}`, `{{color: "red"}}` or a lowercase `{{name}}`, is left exactly as written
/// unless a matching `name=value` argument is passed, as in earlier versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Placeholder {
    pub name: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub text: String,
    /// Blanks that received no value and have no default. They keep their original text,
    /// hint included, so the agent can fill them from context.
    pub unresolved: Vec<Placeholder>,
}

pub fn expand(prompt: &str, arguments: &str) -> Expansion {
    expand_with(prompt, arguments, &HashMap::new())
}

/// Expands `prompt` in a single pass, so substituted values are never expanded again.
/// `named` values (from MCP `prompts/get`) take precedence over `NAME=value` tokens.
pub fn expand_with(prompt: &str, arguments: &str, named: &HashMap<String, String>) -> Expansion {
    let tokens: Vec<&str> = arguments.split_whitespace().collect();
    let mut values: HashMap<&str, &str> = tokens
        .iter()
        .filter_map(|token| token.split_once('='))
        .filter(|(key, _)| is_key(key))
        .collect();
    for (key, value) in named {
        values.insert(key, value);
    }
    let blanks = Blanks::scan(prompt);

    let mut text = String::with_capacity(prompt.len());
    let mut unresolved = Vec::new();
    let mut reported = HashSet::new();
    let mut rest = prompt;
    while let Some(index) = rest.find(['$', '{']) {
        text.push_str(&rest[..index]);
        rest = &rest[index..];

        if let Some(after) = rest.strip_prefix("$ARGUMENTS") {
            text.push_str(arguments);
            rest = after;
        } else if let Some(position) = positional_index(rest) {
            text.push_str(tokens.get(position - 1).copied().unwrap_or_default());
            rest = &rest[2..];
        } else if let Some((inner, after)) = braces(rest) {
            let whole = &rest[..rest.len() - after.len()];
            if inner == "args" {
                text.push_str(arguments);
            } else if let Some(blank) = parse(inner).and_then(|parsed| blanks.get(&parsed.name)) {
                match values
                    .get(blank.name.as_str())
                    .copied()
                    .or(blank.default.as_deref())
                {
                    Some(value) => text.push_str(value),
                    None => {
                        text.push_str(whole);
                        if reported.insert(blank.name.as_str()) {
                            unresolved.push(blank.clone());
                        }
                    }
                }
            } else if let Some(value) = values.get(inner).filter(|_| is_key(inner)) {
                text.push_str(value);
            } else {
                // Not ours (for example `{{#each}}` or `{{color: "red"}}`): keep it and scan inside.
                text.push_str("{{");
                rest = &rest[2..];
                continue;
            }
            rest = after;
        } else {
            text.push_str(&rest[..1]);
            rest = &rest[1..];
        }
    }
    text.push_str(rest);

    Expansion { text, unresolved }
}

/// Every named blank in `prompt`, in first-appearance order. See [`Placeholder`].
pub fn placeholders(prompt: &str) -> Vec<Placeholder> {
    Blanks::scan(prompt).list
}

/// The positional forms `prompt` uses, such as `$1` or `$ARGUMENTS`, in first-appearance order.
/// Text inside a blank, such as a default of `$1`, is literal and not counted.
pub fn positional(prompt: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = prompt;
    while let Some(index) = rest.find(['$', '{']) {
        rest = &rest[index..];
        let (form, skip) = if rest.starts_with("$ARGUMENTS") {
            (Some("$ARGUMENTS".to_string()), "$ARGUMENTS".len())
        } else if let Some(position) = positional_index(rest) {
            (Some(format!("${position}")), 2)
        } else if let Some((inner, after)) =
            braces(rest).filter(|(inner, _)| *inner == "args" || parse(inner).is_some())
        {
            let form = (inner == "args").then(|| "{{args}}".to_string());
            (form, rest.len() - after.len())
        } else {
            (None, 1)
        };
        if let Some(form) = form.filter(|form| !found.contains(form)) {
            found.push(form);
        }
        rest = &rest[skip..];
    }
    found
}

/// Blanks merged by name: the first default and hint win, and a name is required when some
/// use of it is required and no use gives it a default.
struct Blanks {
    list: Vec<Placeholder>,
    index: HashMap<String, usize>,
}

impl Blanks {
    fn scan(prompt: &str) -> Self {
        let mut blanks = Self {
            list: Vec::new(),
            index: HashMap::new(),
        };
        let mut rest = prompt;
        while let Some(index) = rest.find("{{") {
            rest = &rest[index..];
            // Step exactly as `expand_with` does, so both agree on what is a blank.
            match braces(rest) {
                Some((inner, after)) => match parse(inner) {
                    Some(blank) => {
                        blanks.merge(blank);
                        rest = after;
                    }
                    None => rest = &rest[2..],
                },
                None => rest = &rest[1..],
            }
        }
        blanks
    }

    fn merge(&mut self, blank: Placeholder) {
        match self.index.get(&blank.name) {
            Some(&position) => {
                let known = &mut self.list[position];
                known.default = known.default.take().or(blank.default);
                known.hint = known.hint.take().or(blank.hint);
                known.required = (known.required || blank.required) && known.default.is_none();
            }
            None => {
                self.index.insert(blank.name.clone(), self.list.len());
                self.list.push(blank);
            }
        }
    }

    fn get(&self, name: &str) -> Option<&Placeholder> {
        self.index.get(name).map(|&position| &self.list[position])
    }
}

fn positional_index(text: &str) -> Option<usize> {
    match text.as_bytes() {
        [b'$', digit @ b'1'..=b'9', ..] => Some(usize::from(digit - b'0')),
        _ => None,
    }
}

/// Splits `{{inner}}rest` into `inner` and `rest`. The search stops at a newline, at the next
/// `{{`, or after `MAX_BRACES` bytes, so no part of the prompt is scanned more than a bounded
/// number of times.
fn braces(text: &str) -> Option<(&str, &str)> {
    let body = text.strip_prefix("{{")?;
    let bytes = body.as_bytes();
    for index in 0..bytes.len().min(MAX_BRACES) {
        match (bytes[index], bytes.get(index + 1)) {
            (b'\n', _) | (b'{', Some(b'{')) => return None,
            (b'}', Some(b'}')) => return Some((&body[..index], &body[index + 2..])),
            _ => {}
        }
    }
    None
}

fn parse(inner: &str) -> Option<Placeholder> {
    let (spec, hint) = match inner.split_once(':') {
        Some((spec, hint)) => (spec, Some(hint.trim()).filter(|hint| !hint.is_empty())),
        None => (inner, None),
    };
    let (head, default) = match spec.split_once('=') {
        Some((head, default)) => (head, Some(default.trim().to_string())),
        None => (spec, None),
    };
    let head = head.trim_end();
    let (name, optional) = match head.strip_suffix('?') {
        Some(name) => (name, true),
        None => (head, false),
    };
    is_blank_name(name).then(|| Placeholder {
        name: name.to_string(),
        required: !optional && default.is_none(),
        default,
        hint: hint.map(str::to_string),
    })
}

/// UPPER_SNAKE_CASE, which keeps code such as JSX, Jinja, or Vue templates out of reach.
fn is_blank_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
        && characters.all(|character| {
            character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
        })
}

/// Keys accepted in `key=value` arguments, as in earlier versions.
fn is_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(name: &str, required: bool, default: Option<&str>, hint: Option<&str>) -> Placeholder {
        Placeholder {
            name: name.into(),
            required,
            default: default.map(Into::into),
            hint: hint.map(Into::into),
        }
    }

    #[test]
    fn expands_whole_positional_and_named_arguments() {
        let prompt = "Whole={{args}} first=$1 second=$2 focus={{FOCUS}}";
        let expansion = expand(prompt, "alpha beta FOCUS=security");
        assert_eq!(
            expansion.text,
            "Whole=alpha beta FOCUS=security first=alpha second=beta focus=security"
        );
        assert!(expansion.unresolved.is_empty());
    }

    #[test]
    fn leaves_plain_prompts_unchanged() {
        assert_eq!(
            expand("Review this carefully.", "").text,
            "Review this carefully."
        );
    }

    #[test]
    fn reads_required_optional_default_and_hint_markers() {
        let prompt = "Review {{FILE: the file or diff to review}} for {{FOCUS=security}}. {{NOTES?}} {{LENS = speed : what to optimise}}";
        assert_eq!(
            placeholders(prompt),
            vec![
                blank("FILE", true, None, Some("the file or diff to review")),
                blank("FOCUS", false, Some("security"), None),
                blank("NOTES", false, None, None),
                blank("LENS", false, Some("speed"), Some("what to optimise")),
            ]
        );
    }

    #[test]
    fn reports_unfilled_blanks_and_keeps_their_text() {
        let expansion = expand(
            "Review {{FILE: what to review}} for {{FOCUS=security}}. {{NOTES?}}",
            "",
        );
        assert_eq!(
            expansion.text,
            "Review {{FILE: what to review}} for security. {{NOTES?}}"
        );
        assert_eq!(
            expansion.unresolved,
            vec![
                blank("FILE", true, None, Some("what to review")),
                blank("NOTES", false, None, None),
            ]
        );
    }

    #[test]
    fn supplied_values_override_defaults() {
        let expansion = expand("Focus on {{FOCUS=security}}.", "FOCUS=speed");
        assert_eq!(expansion.text, "Focus on speed.");
    }

    #[test]
    fn named_values_take_precedence_over_tokens() {
        let named = HashMap::from([("FILE".to_string(), "src/auth.rs and tests".to_string())]);
        let expansion = expand_with("Review {{FILE}}.", "FILE=other.rs", &named);
        assert_eq!(expansion.text, "Review src/auth.rs and tests.");
    }

    #[test]
    fn a_blank_used_twice_is_reported_once() {
        let expansion = expand("{{FILE?}} then {{FILE: the file}}", "");
        assert_eq!(
            expansion.unresolved,
            vec![blank("FILE", true, None, Some("the file"))]
        );
    }

    #[test]
    fn substituted_values_are_not_expanded_again() {
        let expansion = expand("Say {{WORD}} and $2.", "WORD=$1 tail");
        assert_eq!(expansion.text, "Say $1 and tail.");
    }

    #[test]
    fn keeps_braces_that_are_not_blanks() {
        let prompt = "{{#each items}} {{ title }} {{ args }} {{ FOCUS=x }} {{a-b}} {{X\n}} $1";
        assert_eq!(
            expand(prompt, "one").text,
            "{{#each items}} {{ title }} {{ args }} {{ FOCUS=x }} {{a-b}} {{X\n}} one"
        );
        assert!(placeholders(prompt).is_empty());
    }

    #[test]
    fn lists_positional_forms_once_each() {
        assert_eq!(
            positional("$2 then $1, $2 again, $ARGUMENTS and {{args}} but not {{NAME}} or $0"),
            vec!["$2", "$1", "$ARGUMENTS", "{{args}}"]
        );
    }

    #[test]
    fn code_and_template_text_in_old_prompts_is_untouched() {
        // Outputs recorded from 0.2.0 for the same prompts and arguments.
        for (prompt, arguments, expected) in [
            (
                r#"<div style={{color: "red"}}>"#,
                "",
                r#"<div style={{color: "red"}}>"#,
            ),
            (
                "Hello {{customer_name}}, order {{order_id}}",
                "",
                "Hello {{customer_name}}, order {{order_id}}",
            ),
            ("Hello {{customer_name}}", "customer_name=Ana", "Hello Ana"),
            (
                "{{a=b}} {{width:100}} {{ title }}",
                "",
                "{{a=b}} {{width:100}} {{ title }}",
            ),
            (
                "{{#each items}}{{this}}{{/each}}",
                "",
                "{{#each items}}{{this}}{{/each}}",
            ),
        ] {
            let expansion = expand(prompt, arguments);
            assert_eq!(expansion.text, expected, "prompt: {prompt}");
            assert!(expansion.unresolved.is_empty(), "prompt: {prompt}");
            assert!(placeholders(prompt).is_empty(), "prompt: {prompt}");
        }
    }

    #[test]
    fn a_default_applies_everywhere_the_name_is_used() {
        let expansion = expand("Focus on {{FOCUS=security}}. Again: {{FOCUS}}.", "");
        assert_eq!(expansion.text, "Focus on security. Again: security.");
        assert!(expansion.unresolved.is_empty());
        assert_eq!(
            placeholders("{{FOCUS}} {{FOCUS=security}}"),
            vec![blank("FOCUS", false, Some("security"), None)]
        );
    }

    #[test]
    fn defaults_are_literal_and_not_positional_forms() {
        assert!(positional("Review {{FILE=$1}} now.").is_empty());
        assert_eq!(
            expand("Review {{FILE=$1}} now.", "a.rs").text,
            "Review $1 now."
        );
    }

    #[test]
    fn scanning_stays_linear_on_pathological_prompts() {
        let started = std::time::Instant::now();
        for prompt in [
            "{{".repeat(200_000),
            "{{x\n".repeat(100_000),
            (0..40_000)
                .map(|i| format!("{{{{N{i}}}}} "))
                .collect::<String>(),
        ] {
            let expansion = expand(&prompt, "");
            assert!(expansion.text.len() >= prompt.len() / 2);
            placeholders(&prompt);
        }
        assert!(
            started.elapsed().as_secs() < 3,
            "took {:?}",
            started.elapsed()
        );
    }
}
