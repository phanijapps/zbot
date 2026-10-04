use serde_json::{Map, Value};

pub struct ProjectedArguments {
    pub arguments: Value,
    pub redacted: bool,
}

/// One projection for every hook tool payload. Invalid input never falls back to raw data.
pub fn project_arguments(arguments: &Value, secrets: &[String]) -> ProjectedArguments {
    let mut redacted = false;
    let mut remaining = super::MAX_EVENT_BYTES;
    let mut values: Vec<&str> = secrets
        .iter()
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .collect();
    values.sort_unstable_by_key(|secret| std::cmp::Reverse(secret.len()));
    let projected = arguments
        .is_object()
        .then(|| visit(arguments, &values, &mut redacted, &mut remaining, 0))
        .flatten();
    match projected {
        Some(arguments) => ProjectedArguments {
            arguments,
            redacted,
        },
        None => ProjectedArguments {
            arguments: Value::Object(Map::new()),
            redacted: true,
        },
    }
}

fn sensitive(key: &str) -> bool {
    let normalized: String = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    matches!(
        normalized.as_str(),
        "header" | "headers" | "env" | "environment"
    ) || [
        "token",
        "password",
        "secret",
        "credential",
        "authorization",
        "cookie",
        "apikey",
        "accesskey",
        "privatekey",
    ]
    .iter()
    .any(|part| normalized.contains(part))
}

fn visit(
    value: &Value,
    secrets: &[&str],
    redacted: &mut bool,
    remaining: &mut usize,
    depth: usize,
) -> Option<Value> {
    if depth > 32 {
        return None;
    }
    *remaining = remaining.checked_sub(1)?;
    Some(match value {
        Value::Object(map) => {
            let mut output = Map::new();
            for (key, value) in map {
                *remaining = remaining.checked_sub(key.len())?;
                if sensitive(key) {
                    *redacted = true;
                    continue;
                }
                output.insert(
                    key.clone(),
                    visit(value, secrets, redacted, remaining, depth + 1)?,
                );
            }
            Value::Object(output)
        }
        Value::Array(array) => Value::Array(
            array
                .iter()
                .map(|value| visit(value, secrets, redacted, remaining, depth + 1))
                .collect::<Option<Vec<_>>>()?,
        ),
        Value::String(string) => {
            *remaining = remaining.checked_sub(string.len())?;
            let mut output = string.clone();
            for secret in secrets {
                if output.contains(secret) {
                    output = output.replace(secret, "[redacted]");
                    *redacted = true;
                    if output.len() > super::MAX_EVENT_BYTES {
                        return None;
                    }
                }
            }
            Value::String(output)
        }
        other => other.clone(),
    })
}

#[cfg(test)]
mod tests;
