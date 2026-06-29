//! The recursive dispatcher and metadata application.

use serde_json::Value;

use crate::parsers;
use crate::predicates as its;
use crate::types::{Refs, Seen};
use crate::util::json_stringify;

/// Parse a schema node into a Zod source fragment.
///
/// A boolean schema yields `z.any()` for `true` and `z.never()` for `false`.
/// An object schema runs through the override hook, the cycle and depth guard,
/// the parser dispatch, and then describe, default, and readonly metadata
/// unless `block_meta` is set.
///
/// `block_meta` is used by the nullable parser so metadata lands on the outer
/// `.nullable()` wrapper instead of the inner schema.
pub fn parse_schema(schema: &Value, refs: &Refs, block_meta: bool) -> String {
    if !schema.is_object() {
        // A JSON boolean schema. Arrays and null never appear at this position
        // for a valid JsonSchema, so anything non-object is treated as the
        // boolean branch: truthy maps to any, falsy to never.
        return if truthy_schema(schema) {
            "z.any()".to_string()
        } else {
            "z.never()".to_string()
        };
    }

    if let Some(over) = refs.parser_override.as_ref() {
        if let Some(custom) = over(schema, refs) {
            return custom;
        }
    }

    let id = schema as *const Value as usize;

    {
        let mut seen_map = refs.seen.borrow_mut();
        if let Some(entry) = seen_map.get_mut(&id) {
            if let Some(r) = &entry.r {
                return r.clone();
            }
            match refs.depth {
                None => return "z.any()".to_string(),
                Some(depth) if entry.n >= depth => return "z.any()".to_string(),
                Some(_) => entry.n += 1,
            }
        } else {
            seen_map.insert(id, Seen { n: 0, r: None });
        }
    }

    let mut parsed = select_parser(schema, refs);

    if !block_meta {
        if !refs.without_describes {
            parsed = add_describes(schema, parsed);
        }
        if !refs.without_defaults {
            parsed = add_defaults(schema, parsed);
        }
        parsed = add_annotations(schema, parsed);
    }

    if let Some(entry) = refs.seen.borrow_mut().get_mut(&id) {
        entry.r = Some(parsed.clone());
    }

    parsed
}

/// Append `.describe(<json>)` when `description` is a non-empty truthy value.
fn add_describes(schema: &Value, mut parsed: String) -> String {
    if let Some(desc) = schema.get("description") {
        if truthy_value(desc) {
            parsed.push_str(&format!(".describe({})", json_stringify(desc)));
        }
    }
    parsed
}

/// Append `.default(<json>)` when `default` is present and not null/undefined.
fn add_defaults(schema: &Value, mut parsed: String) -> String {
    if let Some(default) = schema.get("default") {
        // `default !== undefined` in JS. An explicit `null` default is kept.
        parsed.push_str(&format!(".default({})", json_stringify(default)));
    }
    parsed
}

/// Append `.readonly()` when `readOnly` is truthy.
fn add_annotations(schema: &Value, mut parsed: String) -> String {
    if let Some(read_only) = schema.get("readOnly") {
        if truthy_value(read_only) {
            parsed.push_str(".readonly()");
        }
    }
    parsed
}

/// Dispatch a schema object to the matching parser. First match wins.
fn select_parser(schema: &Value, refs: &Refs) -> String {
    if its::is_nullable(schema) {
        parsers::parse_nullable(schema, refs)
    } else if its::is_object(schema) {
        parsers::parse_object(schema, refs)
    } else if its::is_array(schema) {
        parsers::parse_array(schema, refs)
    } else if its::has_any_of(schema) {
        parsers::parse_any_of(schema, refs)
    } else if its::has_all_of(schema) {
        parsers::parse_all_of(schema, refs)
    } else if its::is_simple_discriminated_one_of(schema) {
        parsers::parse_simple_discriminated_one_of(schema, refs)
    } else if its::has_one_of(schema) {
        parsers::parse_one_of(schema, refs)
    } else if its::has_not(schema) {
        parsers::parse_not(schema, refs)
    } else if its::has_enum(schema) {
        parsers::parse_enum(schema)
    } else if its::has_const(schema) {
        parsers::parse_const(schema)
    } else if its::is_multiple_type(schema) {
        parsers::parse_multiple_type(schema, refs)
    } else if its::is_primitive(schema, "string") {
        parsers::parse_string(schema)
    } else if its::is_primitive(schema, "number") || its::is_primitive(schema, "integer") {
        parsers::parse_number(schema)
    } else if its::is_primitive(schema, "boolean") {
        parsers::parse_boolean()
    } else if its::is_primitive(schema, "null") {
        parsers::parse_null()
    } else if its::is_conditional(schema) {
        parsers::parse_if_then_else(schema, refs)
    } else {
        parsers::parse_default()
    }
}

/// JS truthiness for a boolean schema. `true` is truthy, `false` is falsy.
/// Numbers and strings follow JS rules. This only ever sees a boolean here.
fn truthy_schema(schema: &Value) -> bool {
    truthy_value(schema)
}

/// JS truthiness for a JSON value.
fn truthy_value(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
