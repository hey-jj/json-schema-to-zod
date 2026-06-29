//! Type guards that decide which parser handles a schema node.
//!
//! These mirror the `its` object. Each function takes a `serde_json` object
//! value. The dispatcher in [`crate::parse_schema`] checks them in a fixed
//! order where the first match wins.

use serde_json::Value;

/// True when `type === "object"`.
pub fn is_object(x: &Value) -> bool {
    x.get("type").and_then(|t| t.as_str()) == Some("object")
}

/// True when `type === "array"`.
pub fn is_array(x: &Value) -> bool {
    x.get("type").and_then(|t| t.as_str()) == Some("array")
}

/// True when `anyOf` is present.
pub fn has_any_of(x: &Value) -> bool {
    x.get("anyOf").is_some()
}

/// True when `allOf` is present.
pub fn has_all_of(x: &Value) -> bool {
    x.get("allOf").is_some()
}

/// True when `enum` is present.
pub fn has_enum(x: &Value) -> bool {
    x.get("enum").is_some()
}

/// True when `oneOf` is present.
pub fn has_one_of(x: &Value) -> bool {
    x.get("oneOf").is_some()
}

/// True when `not` is present.
pub fn has_not(x: &Value) -> bool {
    x.get("not").is_some()
}

/// True when `const` is present.
pub fn has_const(x: &Value) -> bool {
    x.get("const").is_some()
}

/// True when `nullable === true` exactly.
pub fn is_nullable(x: &Value) -> bool {
    x.get("nullable") == Some(&Value::Bool(true))
}

/// True when `type` is an array.
pub fn is_multiple_type(x: &Value) -> bool {
    x.get("type").map(|t| t.is_array()).unwrap_or(false)
}

/// True when `type === p`.
pub fn is_primitive(x: &Value, p: &str) -> bool {
    x.get("type").and_then(|t| t.as_str()) == Some(p)
}

/// True when `if`, `then`, and `else` are all present and truthy.
pub fn is_conditional(x: &Value) -> bool {
    let has = |k: &str| x.get(k).is_some();
    has("if")
        && truthy(x.get("if"))
        && has("then")
        && has("else")
        && truthy(x.get("then"))
        && truthy(x.get("else"))
}

/// Recognize a discriminated union the library can lower to
/// `z.discriminatedUnion`.
///
/// Requires a non-empty `oneOf` array, a `discriminator` object with a string
/// `propertyName`, and every `oneOf` member to be an object schema whose
/// discriminator property is a required string with a constant value (a `const`
/// or a single-element `enum`).
pub fn is_simple_discriminated_one_of(x: &Value) -> bool {
    let one_of = match x.get("oneOf") {
        Some(Value::Array(arr)) if !arr.is_empty() => arr,
        _ => return false,
    };

    let discriminator = match x.get("discriminator") {
        Some(Value::Object(_)) => x.get("discriminator").unwrap(),
        _ => return false,
    };

    let prop_name = match discriminator.get("propertyName").and_then(|v| v.as_str()) {
        Some(name) => name,
        None => return false,
    };

    one_of.iter().all(|schema| {
        if !schema.is_object() || schema.get("type").and_then(|t| t.as_str()) != Some("object") {
            return false;
        }

        let properties = match schema.get("properties") {
            Some(Value::Object(p)) => p,
            _ => return false,
        };

        let property = match properties.get(prop_name) {
            Some(p) if p.is_object() => p,
            _ => return false,
        };

        if property.get("type").and_then(|t| t.as_str()) != Some("string") {
            return false;
        }

        let has_const = property.get("const").is_some();
        let single_enum = matches!(
            property.get("enum"),
            Some(Value::Array(e)) if e.len() == 1
        );
        if !has_const && !single_enum {
            return false;
        }

        matches!(
            schema.get("required"),
            Some(Value::Array(req)) if req.iter().any(|r| r.as_str() == Some(prop_name))
        )
    })
}

/// JS truthiness for an optional JSON value as used by the conditional guard.
/// `undefined`, `null`, `false`, `0`, `""`, and `NaN` are falsy. Empty objects
/// and arrays are truthy.
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None => false,
        Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
    }
}
