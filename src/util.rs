//! Small helpers shared by the parsers.

use serde_json::Value;

/// Serialize a JSON value the way `JSON.stringify` does for a single value.
///
/// `serde_json` compact output matches V8 for the value domain this library
/// touches: strings get double quotes with `\"`, `\\`, `\n`, `\t`, and control
/// characters as `\uXXXX`. Forward slashes are not escaped. Non-ASCII is left
/// as raw UTF-8. Numbers, booleans, and null are bare. Objects and arrays are
/// compact with no spaces.
pub fn json_stringify(value: &Value) -> String {
    serde_json::to_string(value).expect("serde_json cannot fail on an in-memory Value")
}

/// Serialize a string as a JSON string literal, matching `JSON.stringify` of a
/// string. The result is double-quoted and escaped.
pub fn json_stringify_str(s: &str) -> String {
    json_stringify(&Value::String(s.to_string()))
}

/// Split a slice into a left and right half. The split point is `len / 2`
/// rounded down, so for odd lengths the right half is one longer.
pub fn half<T: Clone>(arr: &[T]) -> (Vec<T>, Vec<T>) {
    let mid = arr.len() / 2;
    (arr[..mid].to_vec(), arr[mid..].to_vec())
}

/// A builder describing how a modifier renders around an optional message.
///
/// `Two` has an opener and a closer with no message slot, so any message is
/// dropped. `Three` carries an opener, a message prefix, and a closer.
pub enum Builder {
    /// `[opener, closer]` with no message insertion point.
    Two(String, String),
    /// `[opener, prefix, closer]` with a message slot between opener and closer.
    Three(String, String, String),
}

/// Render a constraint modifier with an optional error message.
///
/// Reads `schema[key]`. If absent, returns an empty string. Otherwise calls
/// `get` with the raw value and its `JSON.stringify` form. If `get` returns a
/// builder, assembles the opener, the optional `errorMessage[key]` or
/// `fallback`, and the closer.
pub fn with_message<F>(schema: &Value, key: &str, get: F, fallback: Option<&str>) -> String
where
    F: FnOnce(&Value, &str) -> Option<Builder>,
{
    // `schema[key] !== undefined` in JS. A present `null` counts as defined,
    // an absent key does not. serde_json maps that to `Some`/`None`.
    let Some(value) = schema.get(key) else {
        return String::new();
    };

    let json = json_stringify(value);
    let Some(builder) = get(value, &json) else {
        return String::new();
    };

    let (opener, prefix, closer) = match builder {
        Builder::Two(o, c) => (o, String::new(), c),
        Builder::Three(o, p, c) => (o, p, c),
    };

    let mut r = String::new();
    r.push_str(&opener);

    let message = error_message(schema, key).or_else(|| fallback.map(|s| s.to_string()));
    if let Some(msg) = message {
        r.push_str(&prefix);
        r.push_str(&json_stringify_str(&msg));
    }
    r.push_str(&closer);
    r
}

/// Read `schema.errorMessage[key]` as a string, if present and a string.
fn error_message(schema: &Value, key: &str) -> Option<String> {
    schema
        .get("errorMessage")
        .and_then(|m| m.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Return a shallow copy of `obj` with `key` removed. Preserves key order.
/// Non-objects are returned unchanged.
pub fn omit(obj: &Value, key: &str) -> Value {
    match obj {
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                if k != key {
                    out.insert(k.clone(), v.clone());
                }
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}
