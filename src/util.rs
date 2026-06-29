//! Small helpers shared by the parsers.

use std::fmt::Write as _;

use serde_json::Value;

/// Serialize a JSON value the way `JSON.stringify` does for a single value.
///
/// Strings, booleans, null, and integers reuse `serde_json`, which matches V8:
/// strings get double quotes with `\"`, `\\`, `\n`, `\t`, and control
/// characters as `\uXXXX`, forward slashes stay unescaped, and non-ASCII stays
/// raw UTF-8. Objects and arrays are compact with no spaces.
///
/// Floating point numbers use [`ecma_number_to_string`] instead, because
/// `serde_json` and V8 format them differently. `serde_json` keeps a trailing
/// `.0` on whole numbers and switches to exponent form at a lower magnitude.
/// V8 prints the shortest round-tripping decimal and only uses exponent form
/// for exponents below -6 or above 21.
pub fn compact_json(value: &Value) -> String {
    let mut out = String::new();
    write_json(&mut out, value);
    out
}

/// Serialize a string as a JSON string literal, matching `JSON.stringify` of a
/// string. The result is double-quoted and escaped.
pub fn json_string_literal(s: &str) -> String {
    compact_json(&Value::String(s.to_string()))
}

/// Append the `JSON.stringify` form of `value` to `out`.
fn write_json(out: &mut String, value: &Value) {
    match value {
        Value::Number(n) => {
            // Integers serialize the same in `serde_json` and V8. Only floats
            // need the ECMAScript formatter.
            if n.is_f64() {
                out.push_str(&ecma_number_to_string(n.as_f64().unwrap()));
            } else {
                let _ = write!(out, "{n}");
            }
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                // Object keys reuse the string serializer for escaping.
                out.push_str(&serde_json::to_string(k).expect("string serializes"));
                out.push(':');
                write_json(out, v);
            }
            out.push('}');
        }
        // Strings, booleans, and null match V8 byte for byte under serde_json.
        other => out.push_str(&serde_json::to_string(other).expect("value serializes")),
    }
}

/// Format a finite `f64` the way ECMAScript `Number.prototype.toString` does,
/// which is what `JSON.stringify` uses for numbers.
///
/// V8 prints the shortest decimal that round-trips. Whole numbers carry no
/// fraction, `-0` prints as `0`, and exponent notation only appears when the
/// decimal point sits past 21 digits or before the sixth fractional place.
///
/// Integers outside the f64 safe range (above 2^53) never reach this path,
/// since `serde_json` keeps them as exact integers.
fn ecma_number_to_string(x: f64) -> String {
    if x == 0.0 {
        // Covers both +0 and -0.
        return "0".to_string();
    }

    let neg = x < 0.0;
    let abs = x.abs();

    // Rust's `{:e}` gives the shortest round-tripping mantissa and a base-10
    // exponent, for example "1.2345e20" or "5e-7". Split it into the digit
    // string and the exponent.
    let formatted = format!("{abs:e}");
    let (mantissa, exp_part) = formatted.split_once('e').expect("`{:e}` emits an exponent");
    let exp: i32 = exp_part.parse().expect("exponent is an integer");

    // Drop the decimal point to get the significant digits, then trim trailing
    // zeros while keeping at least one digit.
    let mut digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }

    let k = digits.len() as i32;
    // The mantissa has one digit before the point, so the value is
    // mantissa * 10^exp. In ECMAScript terms the decimal point sits at n, where
    // value = digits * 10^(n - k). That gives n = exp + 1.
    let n = exp + 1;

    let mut out = String::new();
    if neg {
        out.push('-');
    }

    if k <= n && n <= 21 {
        // All digits, then n - k trailing zeros.
        out.push_str(&digits);
        for _ in 0..(n - k) {
            out.push('0');
        }
    } else if 0 < n && n <= 21 {
        // Point falls inside the digits.
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        // Leading "0." with -n zeros before the digits.
        out.push_str("0.");
        for _ in 0..(-n) {
            out.push('0');
        }
        out.push_str(&digits);
    } else {
        // Exponent form: one leading digit, optional fraction, then e±exp.
        if k == 1 {
            out.push_str(&digits);
        } else {
            out.push_str(&digits[..1]);
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let e = n - 1;
        out.push('e');
        out.push(if e >= 0 { '+' } else { '-' });
        let _ = write!(out, "{}", e.abs());
    }

    out
}

/// Split a slice into a left and right half. The split point is `len / 2`
/// rounded down, so for odd lengths the right half is one longer.
pub fn half<T: Clone>(arr: &[T]) -> (Vec<T>, Vec<T>) {
    let mid = arr.len() / 2;
    (arr[..mid].to_vec(), arr[mid..].to_vec())
}

/// How a modifier renders around an optional error message.
///
/// `NoMessage` has an opener and a closer with no place to put a message, so
/// any message is dropped. `WithMessage` carries an opener, a message prefix,
/// and a closer, so a message lands between the opener and the closer.
pub enum MessageSlot {
    /// `open` then `close` with no message insertion point.
    NoMessage {
        /// Text before the message slot.
        open: String,
        /// Text after the message slot.
        close: String,
    },
    /// `open`, then `prefix` and the message, then `close`.
    WithMessage {
        /// Text before the message slot.
        open: String,
        /// Separator inserted before the message when one exists.
        prefix: String,
        /// Text after the message slot.
        close: String,
    },
}

/// Render a constraint modifier with an optional error message.
///
/// Reads `schema[key]`. If absent, returns an empty string. Otherwise calls
/// `get` with the raw value and its `JSON.stringify` form. If `get` returns a
/// slot, assembles the opener, the optional `errorMessage[key]` or `fallback`,
/// and the closer.
pub fn with_message<F>(schema: &Value, key: &str, get: F, fallback: Option<&str>) -> String
where
    F: FnOnce(&Value, &str) -> Option<MessageSlot>,
{
    // `schema[key] !== undefined` in JS. A present `null` counts as defined,
    // an absent key does not. serde_json maps that to `Some`/`None`.
    let Some(value) = schema.get(key) else {
        return String::new();
    };

    let json = compact_json(value);
    let Some(slot) = get(value, &json) else {
        return String::new();
    };

    let (opener, prefix, closer) = match slot {
        MessageSlot::NoMessage { open, close } => (open, String::new(), close),
        MessageSlot::WithMessage {
            open,
            prefix,
            close,
        } => (open, prefix, close),
    };

    let mut r = String::new();
    r.push_str(&opener);

    // `schema.errorMessage?.[key] ?? fallback`. A present, non-null entry wins
    // and is serialized as-is, so a number or boolean message survives. An
    // absent or null entry falls through to the fallback. With neither, the
    // message stays undefined and no prefix or text is appended.
    if let Some(value) = error_message(schema, key) {
        r.push_str(&prefix);
        r.push_str(&compact_json(value));
    } else if let Some(text) = fallback {
        r.push_str(&prefix);
        r.push_str(&json_string_literal(text));
    }

    r.push_str(&closer);
    r
}

/// Read `schema.errorMessage[key]` when present and not null.
///
/// JS uses `?? fallback`, so a null entry counts as absent. Any other value,
/// including a number or boolean, is returned and later JSON-serialized.
fn error_message<'a>(schema: &'a Value, key: &str) -> Option<&'a Value> {
    schema
        .get("errorMessage")
        .and_then(|m| m.get(key))
        .filter(|v| !v.is_null())
}

/// JS truthiness for a JSON value.
///
/// `null`, `false`, `0`, `NaN`, and `""` are falsy. Every object and array is
/// truthy, even when empty.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn num(literal: &str) -> Value {
        serde_json::from_str(literal).expect("valid JSON number")
    }

    #[test]
    fn whole_number_floats_drop_the_fraction() {
        // A float written with a trailing zero serializes as an integer, the
        // way `JSON.stringify` does.
        for (input, want) in [
            ("2.0", "2"),
            ("1.0", "1"),
            ("0.0", "0"),
            ("5.0", "5"),
            ("-0.0", "0"),
            ("-2.0", "-2"),
            ("100.0", "100"),
        ] {
            assert_eq!(compact_json(&num(input)), want, "input {input}");
        }
    }

    #[test]
    fn fractional_floats_keep_the_shortest_form() {
        for (input, want) in [
            ("0.1", "0.1"),
            ("1.5", "1.5"),
            ("3.14", "3.14"),
            ("1234.5678", "1234.5678"),
            ("-2.5", "-2.5"),
        ] {
            assert_eq!(compact_json(&num(input)), want, "input {input}");
        }
    }

    #[test]
    fn exponent_threshold_matches_v8() {
        // V8 prints positional digits up to 21 places and below the sixth
        // fractional place, exponent form past that.
        for (input, want) in [
            ("1e20", "100000000000000000000"),
            ("1e21", "1e+21"),
            ("1e22", "1e+22"),
            ("1e-6", "0.000001"),
            ("1e-7", "1e-7"),
            // A 21-digit positional value exercises the long-mantissa path.
            ("1.5e20", "150000000000000000000"),
            ("1.23e-10", "1.23e-10"),
            ("-1e21", "-1e+21"),
        ] {
            assert_eq!(compact_json(&num(input)), want, "input {input}");
        }
    }

    #[test]
    fn integers_stay_exact() {
        for input in ["57", "0", "-7", "1000000", "9007199254740992"] {
            assert_eq!(compact_json(&num(input)), input, "input {input}");
        }
    }

    #[test]
    fn nested_floats_are_formatted_inside_arrays_and_objects() {
        assert_eq!(compact_json(&json!([1.0, 2.0])), "[1,2]");
        assert_eq!(compact_json(&json!({ "a": 2.0 })), r#"{"a":2}"#);
    }

    #[test]
    fn error_message_keeps_non_string_values() {
        // A numeric `errorMessage` entry survives and serializes as a number.
        let schema = json!({ "minLength": 1, "errorMessage": { "minLength": 5 } });
        let out = with_message(
            &schema,
            "minLength",
            |_v, json| {
                Some(MessageSlot::WithMessage {
                    open: format!(".min({json}"),
                    prefix: ", ".into(),
                    close: ")".into(),
                })
            },
            None,
        );
        assert_eq!(out, ".min(1, 5)");
    }

    #[test]
    fn null_error_message_falls_through_to_fallback() {
        // `?? fallback` treats a null entry as absent.
        let schema = json!({ "minLength": 1, "errorMessage": { "minLength": null } });
        let out = with_message(
            &schema,
            "minLength",
            |_v, json| {
                Some(MessageSlot::WithMessage {
                    open: format!(".min({json}"),
                    prefix: ", ".into(),
                    close: ")".into(),
                })
            },
            Some("too short"),
        );
        assert_eq!(out, r#".min(1, "too short")"#);
    }

    #[test]
    fn half_splits_odd_length_with_longer_right() {
        let (left, right) = half(&["A", "B", "C", "D", "E"]);
        assert_eq!(left, vec!["A", "B"]);
        assert_eq!(right, vec!["C", "D", "E"]);
    }

    #[test]
    fn half_splits_even_length_evenly() {
        let (left, right) = half(&["A", "B", "C", "D"]);
        assert_eq!(left, vec!["A", "B"]);
        assert_eq!(right, vec!["C", "D"]);
    }

    #[test]
    fn omit_removes_key_and_keeps_rest() {
        let input = json!({ "a": true, "b": true });
        let output = omit(&input, "b");
        assert_eq!(output.get("a"), Some(&json!(true)));
        assert_eq!(output.get("b"), None);
    }
}
