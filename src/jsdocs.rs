//! JSDoc block rendering from schema `description` values.

use serde_json::Value;

/// Wrap a description string in a JSDoc block.
///
/// A single line renders as `/**<desc>*/\n`. Multiple lines each get a `* `
/// prefix and wrap as `/**\n* line1\n* line2\n*/\n`. Splitting uses `\n` only,
/// so a `\r` stays attached to its line.
pub fn expand_jsdocs(jsdocs: &str) -> String {
    let lines: Vec<&str> = jsdocs.split('\n').collect();
    let result = if lines.len() == 1 {
        lines[0].to_string()
    } else {
        let body = lines
            .iter()
            .map(|x| format!("* {x}"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n{body}\n")
    };
    format!("/**{result}*/\n")
}

/// Prepend a JSDoc block built from `schema.description` to `parsed`, with a
/// leading newline. Returns `parsed` unchanged when there is no description.
pub fn add_jsdocs(schema: &Value, parsed: &str) -> String {
    let description = schema.get("description").and_then(|v| v.as_str());
    match description {
        Some(d) if !d.is_empty() => format!("\n{}{}", expand_jsdocs(d), parsed),
        _ => parsed.to_string(),
    }
}
