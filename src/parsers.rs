//! One parser per JSON Schema construct. Each returns a Zod source fragment.

use serde_json::{json, Map, Value};

use crate::jsdocs::add_jsdocs;
use crate::parse_schema::parse_schema;
use crate::predicates as its;
use crate::types::{PathSegment, Refs, ZodVersion};
use crate::util::{compact_json, half, json_string_literal, truthy, with_message, MessageSlot};

fn key(s: &str) -> PathSegment {
    PathSegment::Key(s.to_string())
}

fn idx(i: usize) -> PathSegment {
    PathSegment::Index(i)
}

/// `z.boolean()`. Ignores the schema body.
pub fn parse_boolean() -> String {
    "z.boolean()".to_string()
}

/// `z.null()`.
pub fn parse_null() -> String {
    "z.null()".to_string()
}

/// `z.any()`. The fallback for unrecognized schemas.
pub fn parse_default() -> String {
    "z.any()".to_string()
}

/// `z.literal(<json>)` from the `const` value.
pub fn parse_const(schema: &Value) -> String {
    let c = schema.get("const").unwrap_or(&Value::Null);
    format!("z.literal({})", compact_json(c))
}

/// Lower an `enum` to a literal, a string enum, or a union of literals.
///
/// Empty yields `z.never()`. A single value yields `z.literal`. All strings
/// yield `z.enum`. A mix yields `z.union` of literals.
pub fn parse_enum(schema: &Value) -> String {
    let values = match schema.get("enum").and_then(|v| v.as_array()) {
        Some(v) => v,
        None => return "z.never()".to_string(),
    };

    if values.is_empty() {
        return "z.never()".to_string();
    }
    if values.len() == 1 {
        return format!("z.literal({})", compact_json(&values[0]));
    }
    if values.iter().all(|x| x.is_string()) {
        // The string branch joins with a comma and no space. This matches the
        // bare array-to-string coercion the contract locks in.
        let joined = values
            .iter()
            .map(compact_json)
            .collect::<Vec<_>>()
            .join(",");
        return format!("z.enum([{joined}])");
    }
    let joined = values
        .iter()
        .map(|x| format!("z.literal({})", compact_json(x)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("z.union([{joined}])")
}

/// Build `z.string()` and append format, pattern, length, and content
/// modifiers in a fixed order.
pub fn parse_string(schema: &Value) -> String {
    let mut r = String::from("z.string()");

    r.push_str(&with_message(
        schema,
        "format",
        |value, _json| {
            let v = value.as_str()?;
            match v {
                "email" => Some(MessageSlot::NoMessage {
                    open: ".email(".into(),
                    close: ")".into(),
                }),
                "ip" => Some(MessageSlot::NoMessage {
                    open: ".ip(".into(),
                    close: ")".into(),
                }),
                "ipv4" => Some(MessageSlot::WithMessage {
                    open: ".ip({ version: \"v4\"".into(),
                    prefix: ", message: ".into(),
                    close: " })".into(),
                }),
                "ipv6" => Some(MessageSlot::WithMessage {
                    open: ".ip({ version: \"v6\"".into(),
                    prefix: ", message: ".into(),
                    close: " })".into(),
                }),
                "uri" => Some(MessageSlot::NoMessage {
                    open: ".url(".into(),
                    close: ")".into(),
                }),
                "uuid" => Some(MessageSlot::NoMessage {
                    open: ".uuid(".into(),
                    close: ")".into(),
                }),
                "date-time" => Some(MessageSlot::WithMessage {
                    open: ".datetime({ offset: true".into(),
                    prefix: ", message: ".into(),
                    close: " })".into(),
                }),
                "time" => Some(MessageSlot::NoMessage {
                    open: ".time(".into(),
                    close: ")".into(),
                }),
                "date" => Some(MessageSlot::NoMessage {
                    open: ".date(".into(),
                    close: ")".into(),
                }),
                "binary" => Some(MessageSlot::NoMessage {
                    open: ".base64(".into(),
                    close: ")".into(),
                }),
                "duration" => Some(MessageSlot::NoMessage {
                    open: ".duration(".into(),
                    close: ")".into(),
                }),
                _ => None,
            }
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "pattern",
        |_value, json| {
            Some(MessageSlot::WithMessage {
                open: format!(".regex(new RegExp({json})"),
                prefix: ", ".into(),
                close: ")".into(),
            })
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "minLength",
        |_value, json| {
            Some(MessageSlot::WithMessage {
                open: format!(".min({json}"),
                prefix: ", ".into(),
                close: ")".into(),
            })
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "maxLength",
        |_value, json| {
            Some(MessageSlot::WithMessage {
                open: format!(".max({json}"),
                prefix: ", ".into(),
                close: ")".into(),
            })
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "contentEncoding",
        |value, _json| {
            if value.as_str() == Some("base64") {
                Some(MessageSlot::NoMessage {
                    open: ".base64(".into(),
                    close: ")".into(),
                })
            } else {
                None
            }
        },
        None,
    ));

    let content_media_type = with_message(
        schema,
        "contentMediaType",
        |value, _json| {
            if value.as_str() == Some("application/json") {
                Some(MessageSlot::WithMessage { open: ".transform((str, ctx) => { try { return JSON.parse(str); } catch (err) { ctx.addIssue({ code: \"custom\", message: \"Invalid JSON\" }); }}".into(), prefix: ", ".into(), close: ")".into() })
            } else {
                None
            }
        },
        None,
    );

    if !content_media_type.is_empty() {
        r.push_str(&content_media_type);
        r.push_str(&with_message(
            schema,
            "contentSchema",
            |value, _json| {
                // JS guards with `value instanceof Object`, which is true for
                // arrays as well as plain objects. Match both.
                if value.is_object() || value.is_array() {
                    // Parsed with fresh refs, independent of the parent walk.
                    let inner = parse_schema(value, &Refs::default_v4(), false);
                    Some(MessageSlot::WithMessage {
                        open: format!(".pipe({inner}"),
                        prefix: ", ".into(),
                        close: ")".into(),
                    })
                } else {
                    None
                }
            },
            None,
        ));
    }

    r
}

/// Build `z.number()` and append int, multipleOf, and range modifiers.
pub fn parse_number(schema: &Value) -> String {
    let mut r = String::from("z.number()");

    if schema.get("type").and_then(|t| t.as_str()) == Some("integer") {
        r.push_str(&with_message(
            schema,
            "type",
            |_v, _j| {
                Some(MessageSlot::NoMessage {
                    open: ".int(".into(),
                    close: ")".into(),
                })
            },
            None,
        ));
    } else {
        r.push_str(&with_message(
            schema,
            "format",
            |value, _json| {
                if value.as_str() == Some("int64") {
                    Some(MessageSlot::NoMessage {
                        open: ".int(".into(),
                        close: ")".into(),
                    })
                } else {
                    None
                }
            },
            None,
        ));
    }

    let starts_int = r.starts_with("z.number().int(");
    r.push_str(&with_message(
        schema,
        "multipleOf",
        |value, json| {
            if value.as_f64() == Some(1.0) {
                if starts_int {
                    return None;
                }
                return Some(MessageSlot::NoMessage {
                    open: ".int(".into(),
                    close: ")".into(),
                });
            }
            Some(MessageSlot::WithMessage {
                open: format!(".multipleOf({json}"),
                prefix: ", ".into(),
                close: ")".into(),
            })
        },
        None,
    ));

    let minimum_is_number = schema
        .get("minimum")
        .map(|v| v.is_number())
        .unwrap_or(false);
    let exclusive_min_is_number = schema
        .get("exclusiveMinimum")
        .map(|v| v.is_number())
        .unwrap_or(false);
    let exclusive_min_is_true = schema.get("exclusiveMinimum") == Some(&Value::Bool(true));

    if minimum_is_number {
        if exclusive_min_is_true {
            r.push_str(&with_message(
                schema,
                "minimum",
                |_v, json| {
                    Some(MessageSlot::WithMessage {
                        open: format!(".gt({json}"),
                        prefix: ", ".into(),
                        close: ")".into(),
                    })
                },
                None,
            ));
        } else {
            r.push_str(&with_message(
                schema,
                "minimum",
                |_v, json| {
                    Some(MessageSlot::WithMessage {
                        open: format!(".gte({json}"),
                        prefix: ", ".into(),
                        close: ")".into(),
                    })
                },
                None,
            ));
        }
    } else if exclusive_min_is_number {
        r.push_str(&with_message(
            schema,
            "exclusiveMinimum",
            |_v, json| {
                Some(MessageSlot::WithMessage {
                    open: format!(".gt({json}"),
                    prefix: ", ".into(),
                    close: ")".into(),
                })
            },
            None,
        ));
    }

    let maximum_is_number = schema
        .get("maximum")
        .map(|v| v.is_number())
        .unwrap_or(false);
    let exclusive_max_is_number = schema
        .get("exclusiveMaximum")
        .map(|v| v.is_number())
        .unwrap_or(false);
    let exclusive_max_is_true = schema.get("exclusiveMaximum") == Some(&Value::Bool(true));

    if maximum_is_number {
        if exclusive_max_is_true {
            r.push_str(&with_message(
                schema,
                "maximum",
                |_v, json| {
                    Some(MessageSlot::WithMessage {
                        open: format!(".lt({json}"),
                        prefix: ", ".into(),
                        close: ")".into(),
                    })
                },
                None,
            ));
        } else {
            r.push_str(&with_message(
                schema,
                "maximum",
                |_v, json| {
                    Some(MessageSlot::WithMessage {
                        open: format!(".lte({json}"),
                        prefix: ", ".into(),
                        close: ")".into(),
                    })
                },
                None,
            ));
        }
    } else if exclusive_max_is_number {
        r.push_str(&with_message(
            schema,
            "exclusiveMaximum",
            |_v, json| {
                Some(MessageSlot::WithMessage {
                    open: format!(".lt({json}"),
                    prefix: ", ".into(),
                    close: ")".into(),
                })
            },
            None,
        ));
    }

    r
}

/// Lower an `array` to `z.tuple` or `z.array` with min, max, and unique
/// modifiers.
pub fn parse_array(schema: &Value, refs: &Refs) -> String {
    if let Some(items) = schema.get("items").and_then(|i| i.as_array()) {
        // Tuple. Items join with a comma and no space, the same bare
        // array-to-string coercion the contract locks in.
        let parts = items
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let child = refs.with_path(refs.push_path(&[key("items"), idx(i)]));
                parse_schema(v, &child, false)
            })
            .collect::<Vec<_>>()
            .join(",");
        return format!("z.tuple([{parts}])");
    }

    let mut r = match schema.get("items") {
        None | Some(Value::Null) => "z.array(z.any())".to_string(),
        Some(items) if !truthy(items) => "z.array(z.any())".to_string(),
        Some(items) => {
            let child = refs.with_path(refs.push_path(&[key("items")]));
            format!("z.array({})", parse_schema(items, &child, false))
        }
    };

    r.push_str(&with_message(
        schema,
        "minItems",
        |_v, json| {
            Some(MessageSlot::WithMessage {
                open: format!(".min({json}"),
                prefix: ", ".into(),
                close: ")".into(),
            })
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "maxItems",
        |_v, json| {
            Some(MessageSlot::WithMessage {
                open: format!(".max({json}"),
                prefix: ", ".into(),
                close: ")".into(),
            })
        },
        None,
    ));

    if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
        r.push_str(&with_message(
            schema,
            "uniqueItems",
            |_v, _json| {
                Some(MessageSlot::WithMessage {
                    open: ".refine((arr) => arr.every((item, i) => arr.indexOf(item) == i)".into(),
                    prefix: ", ".into(),
                    close: ")".into(),
                })
            },
            Some("All items must be unique!"),
        ));
    }

    r
}

/// `<inner>.nullable()` where `inner` is the schema with `nullable` removed and
/// metadata blocked. The outer frame applies metadata to the wrapper.
pub fn parse_nullable(schema: &Value, refs: &Refs) -> String {
    let inner = refs.intern(crate::util::omit(schema, "nullable"));
    format!("{}.nullable()", parse_schema(&inner, refs, true))
}

/// `z.any().refine(...)` that rejects anything matching the `not` schema.
pub fn parse_not(schema: &Value, refs: &Refs) -> String {
    let null = Value::Null;
    let not = schema.get("not").unwrap_or(&null);
    let child = refs.with_path(refs.push_path(&[key("not")]));
    format!(
        "z.any().refine((value) => !{}.safeParse(value).success, \"Invalid input: Should NOT be valid against schema\")",
        parse_schema(not, &child, false)
    )
}

/// `z.union([...])` over each entry in a multi-valued `type`. Per-branch
/// defaults are dropped so the union carries the default once.
pub fn parse_multiple_type(schema: &Value, refs: &Refs) -> String {
    // The dispatcher only routes here when `type` is an array, so this never
    // fails. An empty array maps zero branches and emits `z.union([])`.
    let types = schema
        .get("type")
        .and_then(|t| t.as_array())
        .expect("dispatcher guarantees an array type");

    let parts = types
        .iter()
        .map(|t| {
            let mut obj = schema.clone();
            if let Value::Object(map) = &mut obj {
                map.insert("type".to_string(), t.clone());
            }
            let interned = refs.intern(obj);
            let child = refs.with_path_without_defaults(refs.path.clone());
            parse_schema(&interned, &child, false)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("z.union([{parts}])")
}

/// `z.union([...])` over `anyOf`. Empty yields `z.any()`, a single member
/// unwraps.
pub fn parse_any_of(schema: &Value, refs: &Refs) -> String {
    let any_of = schema.get("anyOf").and_then(|v| v.as_array());
    let any_of = match any_of {
        Some(a) => a,
        None => return "z.any()".to_string(),
    };

    if any_of.is_empty() {
        return "z.any()".to_string();
    }
    if any_of.len() == 1 {
        let child = refs.with_path(refs.push_path(&[key("anyOf"), idx(0)]));
        return parse_schema(&any_of[0], &child, false);
    }
    let parts = any_of
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let child = refs.with_path(refs.push_path(&[key("anyOf"), idx(i)]));
            parse_schema(s, &child, false)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("z.union([{parts}])")
}

const ORIGINAL_INDEX: &str = "__originalIndex";

/// Tag each `allOf` member with its source index, mirroring the hidden Symbol.
/// Booleans become `{}` or `{ not: {} }`. If any member already carries the
/// tag, the array is returned unchanged.
fn ensure_original_index(arr: &[Value]) -> Vec<Value> {
    let mut new_arr = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        match item {
            Value::Bool(true) => {
                new_arr.push(json!({ ORIGINAL_INDEX: i }));
            }
            Value::Bool(false) => {
                new_arr.push(json!({ ORIGINAL_INDEX: i, "not": {} }));
            }
            Value::Object(map) => {
                if map.contains_key(ORIGINAL_INDEX) {
                    return arr.to_vec();
                }
                let mut next = map.clone();
                next.insert(ORIGINAL_INDEX.to_string(), Value::from(i));
                new_arr.push(Value::Object(next));
            }
            other => {
                new_arr.push(other.clone());
            }
        }
    }
    new_arr
}

/// Lower `allOf` to a right-leaning chain of `z.intersection`. Empty yields
/// `z.never()`.
pub fn parse_all_of(schema: &Value, refs: &Refs) -> String {
    let all_of = schema.get("allOf").and_then(|v| v.as_array());
    let all_of = match all_of {
        Some(a) => a,
        None => return "z.never()".to_string(),
    };

    if all_of.is_empty() {
        return "z.never()".to_string();
    }
    if all_of.len() == 1 {
        let item = &all_of[0];
        let original = item
            .get(ORIGINAL_INDEX)
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let mut segs = vec![key("allOf")];
        match original {
            Some(i) => segs.push(idx(i)),
            None => segs.push(PathSegment::Key("undefined".to_string())),
        }
        let child = refs.with_path(refs.push_path(&segs));
        // Strip the index tag before parsing so it does not leak into output.
        // An untagged item keeps its tree identity; a tagged one is rebuilt.
        if item.get(ORIGINAL_INDEX).is_some() {
            let cleaned = refs.intern(strip_original_index(item));
            return parse_schema(&cleaned, &child, false);
        }
        return parse_schema(item, &child, false);
    }

    let tagged = ensure_original_index(all_of);
    let (left, right) = half(&tagged);
    let left_schema = refs.intern(json!({ "allOf": left }));
    let right_schema = refs.intern(json!({ "allOf": right }));
    format!(
        "z.intersection({}, {})",
        parse_all_of(&left_schema, refs),
        parse_all_of(&right_schema, refs)
    )
}

/// Remove the index tag from an object so it does not appear in emitted source.
fn strip_original_index(item: &Value) -> Value {
    match item {
        Value::Object(map) if map.contains_key(ORIGINAL_INDEX) => {
            let mut next = map.clone();
            next.remove(ORIGINAL_INDEX);
            Value::Object(next)
        }
        other => other.clone(),
    }
}

/// `z.discriminatedUnion("<prop>", [...])`. Empty yields `z.any()`, a single
/// member unwraps.
pub fn parse_simple_discriminated_one_of(schema: &Value, refs: &Refs) -> String {
    let one_of = schema.get("oneOf").and_then(|v| v.as_array());
    let one_of = match one_of {
        Some(o) => o,
        None => return "z.any()".to_string(),
    };

    if one_of.is_empty() {
        return "z.any()".to_string();
    }
    if one_of.len() == 1 {
        let child = refs.with_path(refs.push_path(&[key("oneOf"), idx(0)]));
        return parse_schema(&one_of[0], &child, false);
    }

    let prop = schema
        .get("discriminator")
        .and_then(|d| d.get("propertyName"))
        .and_then(|p| p.as_str())
        .unwrap_or("");
    let parts = one_of
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let child = refs.with_path(refs.push_path(&[key("oneOf"), idx(i)]));
            parse_schema(s, &child, false)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "z.discriminatedUnion({}, [{parts}])",
        json_string_literal(prop)
    )
}

/// Lower a generic `oneOf` to a `z.any().superRefine(...)` that passes when
/// exactly one branch matches. The emitted body differs between Zod v3 and v4.
pub fn parse_one_of(schema: &Value, refs: &Refs) -> String {
    let one_of = schema.get("oneOf").and_then(|v| v.as_array());
    let one_of = match one_of {
        Some(o) => o,
        None => return "z.any()".to_string(),
    };

    if one_of.is_empty() {
        return "z.any()".to_string();
    }
    if one_of.len() == 1 {
        let child = refs.with_path(refs.push_path(&[key("oneOf"), idx(0)]));
        return parse_schema(&one_of[0], &child, false);
    }

    let is3 = refs.zod_version == ZodVersion::V3;
    let schemas = one_of
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let child = refs.with_path(refs.push_path(&[key("oneOf"), idx(i)]));
            parse_schema(s, &child, false)
        })
        .collect::<Vec<_>>()
        .join(", ");

    let error_type = if is3 { "ZodError" } else { "core.$ZodIssue" };
    let error_push = if is3 {
        "result.error"
    } else {
        "...result.error.issues"
    };
    let path = if is3 { "ctx.path" } else { "[]" };
    let union_line = if is3 {
        "unionErrors: errors"
    } else {
        "errors: [errors]"
    };
    let custom_extra = if is3 {
        ""
    } else {
        "\n        errors: [errors],"
    };

    format!(
        r#"z.any().superRefine((x, ctx) => {{
    const schemas = [{schemas}];
    const {{ errors, failed }} = schemas.reduce<{{
      errors: z.{error_type}[];
      failed: number;
    }}>(
      ({{ errors, failed }}, schema) =>
        ((result) =>
          result.error
            ? {{
                errors: [...errors, {error_push}],
                failed: failed + 1,
              }}
            : {{ errors, failed }})(
          schema.safeParse(x),
        ),
      {{ errors: [], failed: 0 }},
    );
    const passed = schemas.length - failed;
    if (passed !== 1) {{
      ctx.addIssue(errors.length ? {{
        path: {path},
        code: "invalid_union",
        {union_line},
        message: "Invalid input: Should pass single schema. Passed " + passed,
      }} : {{
        path: {path},
        code: "custom",{custom_extra}
        message: "Invalid input: Should pass single schema. Passed " + passed,
      }});
    }}
  }})"#
    )
}

/// Lower `if`/`then`/`else` to a union refined by the condition.
pub fn parse_if_then_else(schema: &Value, refs: &Refs) -> String {
    let null = Value::Null;
    let if_schema = schema.get("if").unwrap_or(&null);
    let then_schema = schema.get("then").unwrap_or(&null);
    let else_schema = schema.get("else").unwrap_or(&null);

    let if_child = refs.with_path(refs.push_path(&[key("if")]));
    let then_child = refs.with_path(refs.push_path(&[key("then")]));
    let else_child = refs.with_path(refs.push_path(&[key("else")]));

    let s_if = parse_schema(if_schema, &if_child, false);
    let s_then = parse_schema(then_schema, &then_child, false);
    let s_else = parse_schema(else_schema, &else_child, false);

    format!(
        r#"z.union([{s_then}, {s_else}]).superRefine((value,ctx) => {{
  const result = {s_if}.safeParse(value).success
    ? {s_then}.safeParse(value)
    : {s_else}.safeParse(value);
  if (!result.success) {{
    result.error.issues.forEach((error) => ctx.addIssue(error))
  }}
}})"#
    )
}

/// Coerce a combinator member that bears object keywords but no `type` into an
/// object schema by injecting `type: "object"`.
fn coerce_object_member(x: &Value) -> Value {
    if let Value::Object(map) = x {
        let no_type = !map.contains_key("type");
        let has_object_keys = map.contains_key("properties")
            || map.contains_key("additionalProperties")
            || map.contains_key("patternProperties");
        if no_type && has_object_keys {
            let mut next = map.clone();
            next.insert("type".to_string(), Value::String("object".to_string()));
            return Value::Object(next);
        }
    }
    x.clone()
}

fn emit_record(value_schema: &str, refs: &Refs) -> String {
    if refs.zod_version == ZodVersion::V3 {
        format!("z.record({value_schema})")
    } else {
        format!("z.record(z.string(), {value_schema})")
    }
}

fn emit_error_path(refs: &Refs) -> String {
    if refs.zod_version == ZodVersion::V3 {
        "path: [...ctx.path, key]".to_string()
    } else {
        "path: [key]".to_string()
    }
}

/// Lower an `object` schema. Handles properties, additionalProperties,
/// patternProperties, the strict/catchall/record base, and appended
/// `anyOf`/`oneOf`/`allOf` combinators.
pub fn parse_object(schema: &Value, refs: &Refs) -> String {
    let empty_map = Map::new();
    let props_map = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .unwrap_or(&empty_map);
    let has_properties_key = schema.get("properties").is_some();

    let mut properties: Option<String> = None;
    if has_properties_key {
        if props_map.is_empty() {
            properties = Some("z.object({})".to_string());
        } else {
            let required_array = schema.get("required").and_then(|r| r.as_array());
            let entries = props_map
                .iter()
                .map(|(prop_key, prop_schema)| {
                    let child = refs.with_path(refs.push_path(&[key("properties"), key(prop_key)]));
                    let mut result = format!(
                        "{}: {}",
                        json_string_literal(prop_key),
                        parse_schema(prop_schema, &child, false)
                    );

                    if refs.with_jsdocs && prop_schema.is_object() {
                        result = add_jsdocs(prop_schema, &result);
                    }

                    let has_default =
                        prop_schema.is_object() && prop_schema.get("default").is_some();

                    let required = match required_array {
                        Some(arr) => arr.iter().any(|r| r.as_str() == Some(prop_key.as_str())),
                        None => {
                            prop_schema.is_object()
                                && prop_schema.get("required") == Some(&Value::Bool(true))
                        }
                    };

                    let optional = !has_default && !required;
                    if optional {
                        format!("{result}.optional()")
                    } else {
                        result
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            properties = Some(format!("z.object({{ {entries} }})"));
        }
    }

    let additional_properties = schema.get("additionalProperties").map(|ap| {
        let child = refs.with_path(refs.push_path(&[key("additionalProperties")]));
        parse_schema(ap, &child, false)
    });

    let pattern_properties =
        build_pattern_properties(schema, refs, &properties, &additional_properties);

    let mut output = build_base(
        &properties,
        &pattern_properties,
        &additional_properties,
        refs,
    );

    if its::has_any_of(schema) {
        let mapped = refs.intern(map_combinator(schema, "anyOf"));
        output.push_str(&format!(".and({})", parse_any_of(&mapped, refs)));
    }
    if its::has_one_of(schema) {
        let mapped = refs.intern(map_combinator(schema, "oneOf"));
        output.push_str(&format!(".and({})", parse_one_of(&mapped, refs)));
    }
    if its::has_all_of(schema) {
        let mapped = refs.intern(map_combinator(schema, "allOf"));
        output.push_str(&format!(".and({})", parse_all_of(&mapped, refs)));
    }

    output
}

/// Build a schema copy where the named combinator array has object coercion
/// applied to each member.
fn map_combinator(schema: &Value, combinator: &str) -> Value {
    let mut obj = schema.clone();
    if let Value::Object(map) = &mut obj {
        if let Some(Value::Array(arr)) = map.get(combinator) {
            let mapped: Vec<Value> = arr.iter().map(coerce_object_member).collect();
            map.insert(combinator.to_string(), Value::Array(mapped));
        }
    }
    obj
}

/// Select the base expression from properties, patternProperties, and
/// additionalProperties by precedence.
fn build_base(
    properties: &Option<String>,
    pattern_properties: &Option<String>,
    additional_properties: &Option<String>,
    refs: &Refs,
) -> String {
    match properties {
        Some(props) => match pattern_properties {
            Some(pp) => format!("{props}{pp}"),
            None => match additional_properties {
                Some(ap) if ap == "z.never()" => format!("{props}.strict()"),
                Some(ap) => format!("{props}.catchall({ap})"),
                None => props.clone(),
            },
        },
        None => match pattern_properties {
            Some(pp) => pp.clone(),
            None => match additional_properties {
                Some(ap) => emit_record(ap, refs),
                None => emit_record("z.any()", refs),
            },
        },
    }
}

/// Render the `.catchall(...)`/record prefix plus the `.superRefine(...)` body
/// for `patternProperties`. Returns `None` when there are none.
fn build_pattern_properties(
    schema: &Value,
    refs: &Refs,
    properties: &Option<String>,
    additional_properties: &Option<String>,
) -> Option<String> {
    let pattern_map = schema
        .get("patternProperties")
        .and_then(|p| p.as_object())?;

    // Parse each pattern value once, preserving key order.
    let parsed: Vec<(String, String)> = pattern_map
        .iter()
        .map(|(pkey, value)| {
            let child = refs.with_path(refs.push_path(&[key("patternProperties"), key(pkey)]));
            (pkey.clone(), parse_schema(value, &child, false))
        })
        .collect();

    let values: Vec<String> = parsed.iter().map(|(_, v)| v.clone()).collect();
    let mut pp = String::new();

    if properties.is_some() {
        if let Some(ap) = additional_properties {
            let mut all = values.clone();
            all.push(ap.clone());
            pp.push_str(&format!(".catchall(z.union([{}]))", all.join(", ")));
        } else if values.len() > 1 {
            pp.push_str(&format!(".catchall(z.union([{}]))", values.join(", ")));
        } else {
            // Single value interpolated without join, no surrounding spacing.
            pp.push_str(&format!(".catchall({})", values.join(",")));
        }
    } else if let Some(ap) = additional_properties {
        let mut all = values.clone();
        all.push(ap.clone());
        pp.push_str(&emit_record(
            &format!("z.union([{}])", all.join(", ")),
            refs,
        ));
    } else if values.len() > 1 {
        pp.push_str(&emit_record(
            &format!("z.union([{}])", values.join(", ")),
            refs,
        ));
    } else {
        pp.push_str(&emit_record(&values.join(","), refs));
    }

    pp.push_str(".superRefine((value, ctx) => {\n");
    pp.push_str("for (const key in value) {\n");

    let has_additional = additional_properties.is_some();
    if has_additional {
        if let Some(prop_obj) = schema.get("properties").and_then(|p| p.as_object()) {
            let keys = prop_obj
                .keys()
                .map(|k| json_string_literal(k))
                .collect::<Vec<_>>()
                .join(", ");
            pp.push_str(&format!("let evaluated = [{keys}].includes(key)\n"));
        } else {
            pp.push_str("let evaluated = false\n");
        }
    }

    let error_path = emit_error_path(refs);
    for (pkey, parsed_value) in &parsed {
        pp.push_str(&format!(
            "if (key.match(new RegExp({}))) {{\n",
            json_string_literal(pkey)
        ));
        if has_additional {
            pp.push_str("evaluated = true\n");
        }
        pp.push_str(&format!(
            "const result = {parsed_value}.safeParse(value[key])\n"
        ));
        pp.push_str("if (!result.success) {\n");
        pp.push_str(&format!(
            "ctx.addIssue({{
          {error_path},
          code: 'custom',
          message: `Invalid input: Key matching regex /${{key}}/ must match schema`,
          params: {{
            issues: result.error.issues
          }}
        }})\n"
        ));
        pp.push_str("}\n");
        pp.push_str("}\n");
    }

    if has_additional {
        let ap = additional_properties.as_ref().unwrap();
        pp.push_str("if (!evaluated) {\n");
        pp.push_str(&format!("const result = {ap}.safeParse(value[key])\n"));
        pp.push_str("if (!result.success) {\n");
        pp.push_str(&format!(
            "ctx.addIssue({{
          {error_path},
          code: 'custom',
          message: `Invalid input: must match catchall schema`,
          params: {{
            issues: result.error.issues
          }}
        }})\n"
        ));
        pp.push_str("}\n");
        pp.push_str("}\n");
    }

    pp.push_str("}\n");
    pp.push_str("})");

    Some(pp)
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::predicates::is_simple_discriminated_one_of;
    use crate::types::{Refs, ZodVersion};

    fn refs_v4() -> Refs {
        Refs::default_v4()
    }

    fn refs_v3() -> Refs {
        let mut r = Refs::default_v4();
        r.zod_version = ZodVersion::V3;
        r
    }

    mod parse_const {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn falsy_constant() {
            assert_eq!(parse_const(&json!({ "const": false })), "z.literal(false)");
        }

        #[test]
        fn empty_string_constant() {
            assert_eq!(parse_const(&json!({ "const": "" })), r#"z.literal("")"#);
        }
    }

    mod parse_enum {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn empty_enum_is_never() {
            assert_eq!(parse_enum(&json!({ "enum": [] })), "z.never()");
        }

        #[test]
        fn single_item_enum_is_literal() {
            assert_eq!(
                parse_enum(&json!({ "enum": ["someValue"] })),
                r#"z.literal("someValue")"#
            );
        }

        #[test]
        fn all_string_enum_is_enum_array() {
            assert_eq!(
                parse_enum(&json!({ "enum": ["someValue", "anotherValue"] })),
                r#"z.enum(["someValue","anotherValue"])"#
            );
        }

        #[test]
        fn mixed_enum_is_union() {
            assert_eq!(
                parse_enum(&json!({ "enum": ["someValue", 57] })),
                r#"z.union([z.literal("someValue"), z.literal(57)])"#
            );
        }
    }

    mod parse_number {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn integer() {
            assert_eq!(
                parse_number(&json!({ "type": "integer" })),
                "z.number().int()"
            );
            assert_eq!(
                parse_number(&json!({ "type": "integer", "multipleOf": 1 })),
                "z.number().int()"
            );
            assert_eq!(
                parse_number(&json!({ "type": "number", "multipleOf": 1 })),
                "z.number().int()"
            );
        }

        #[test]
        fn minimum_with_exclusive_minimum_true() {
            assert_eq!(
                parse_number(&json!({ "type": "number", "exclusiveMinimum": true, "minimum": 2 })),
                "z.number().gt(2)"
            );
        }

        #[test]
        fn plain_minimum() {
            assert_eq!(
                parse_number(&json!({ "type": "number", "minimum": 2 })),
                "z.number().gte(2)"
            );
        }

        #[test]
        fn maximum_with_exclusive_maximum_true() {
            assert_eq!(
                parse_number(&json!({ "type": "number", "exclusiveMaximum": true, "maximum": 2 })),
                "z.number().lt(2)"
            );
        }

        #[test]
        fn numeric_exclusive_maximum() {
            assert_eq!(
                parse_number(&json!({ "type": "number", "exclusiveMaximum": 2 })),
                "z.number().lt(2)"
            );
        }

        #[test]
        fn error_messages_per_key() {
            assert_eq!(
                parse_number(&json!({
                    "type": "number",
                    "format": "int64",
                    "exclusiveMinimum": 0,
                    "maximum": 2,
                    "multipleOf": 2,
                    "errorMessage": {
                        "format": "ayy",
                        "multipleOf": "lmao",
                        "exclusiveMinimum": "deez",
                        "maximum": "nuts"
                    }
                })),
                r#"z.number().int("ayy").multipleOf(2, "lmao").gt(0, "deez").lte(2, "nuts")"#
            );
        }
    }

    mod parse_string {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn date_time_with_message() {
            assert_eq!(
                parse_string(&json!({
                    "type": "string",
                    "format": "date-time",
                    "errorMessage": { "format": "hello" }
                })),
                r#"z.string().datetime({ offset: true, message: "hello" })"#
            );
        }

        #[test]
        fn email() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "email" })),
                "z.string().email()"
            );
        }

        #[test]
        fn ip_and_ipv6() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "ip" })),
                "z.string().ip()"
            );
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "ipv6" })),
                r#"z.string().ip({ version: "v6" })"#
            );
        }

        #[test]
        fn uri() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "uri" })),
                "z.string().url()"
            );
        }

        #[test]
        fn uuid() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "uuid" })),
                "z.string().uuid()"
            );
        }

        #[test]
        fn time() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "time" })),
                "z.string().time()"
            );
        }

        #[test]
        fn date() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "date" })),
                "z.string().date()"
            );
        }

        #[test]
        fn duration() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "duration" })),
                "z.string().duration()"
            );
        }

        #[test]
        fn base64_variants() {
            assert_eq!(
                parse_string(&json!({ "type": "string", "contentEncoding": "base64" })),
                "z.string().base64()"
            );
            assert_eq!(
                parse_string(&json!({
                    "type": "string",
                    "contentEncoding": "base64",
                    "errorMessage": { "contentEncoding": "x" }
                })),
                r#"z.string().base64("x")"#
            );
            assert_eq!(
                parse_string(&json!({ "type": "string", "format": "binary" })),
                "z.string().base64()"
            );
            assert_eq!(
                parse_string(&json!({
                    "type": "string",
                    "format": "binary",
                    "errorMessage": { "format": "x" }
                })),
                r#"z.string().base64("x")"#
            );
        }

        #[test]
        fn stringified_json() {
            let schema = json!({
                "type": "string",
                "contentMediaType": "application/json",
                "contentSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "age": { "type": "integer" }
                    },
                    "required": ["name", "age"]
                }
            });
            assert_eq!(
                parse_string(&schema),
                r#"z.string().transform((str, ctx) => { try { return JSON.parse(str); } catch (err) { ctx.addIssue({ code: "custom", message: "Invalid JSON" }); }}).pipe(z.object({ "name": z.string(), "age": z.number().int() }))"#
            );
        }

        #[test]
        fn stringified_json_with_messages() {
            let schema = json!({
                "type": "string",
                "contentMediaType": "application/json",
                "contentSchema": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "age": { "type": "integer" }
                    },
                    "required": ["name", "age"]
                },
                "errorMessage": { "contentMediaType": "x", "contentSchema": "y" }
            });
            assert_eq!(
                parse_string(&schema),
                r#"z.string().transform((str, ctx) => { try { return JSON.parse(str); } catch (err) { ctx.addIssue({ code: "custom", message: "Invalid JSON" }); }}, "x").pipe(z.object({ "name": z.string(), "age": z.number().int() }), "y")"#
            );
        }

        #[test]
        fn combined_format_pattern_lengths_with_messages() {
            assert_eq!(
                parse_string(&json!({
                    "type": "string",
                    "format": "ipv4",
                    "pattern": "x",
                    "minLength": 1,
                    "maxLength": 2,
                    "errorMessage": {
                        "format": "ayy",
                        "pattern": "lmao",
                        "minLength": "deez",
                        "maxLength": "nuts"
                    }
                })),
                r#"z.string().ip({ version: "v4", message: "ayy" }).regex(new RegExp("x"), "lmao").min(1, "deez").max(2, "nuts")"#
            );
        }

        #[test]
        fn array_content_schema_still_pipes() {
            // `value instanceof Object` is true for arrays, so an array
            // contentSchema keeps the `.pipe(...)`. An empty array parses to
            // `z.any()` through the boolean branch.
            assert_eq!(
                parse_string(&json!({
                    "type": "string",
                    "contentMediaType": "application/json",
                    "contentSchema": []
                })),
                r#"z.string().transform((str, ctx) => { try { return JSON.parse(str); } catch (err) { ctx.addIssue({ code: "custom", message: "Invalid JSON" }); }}).pipe(z.any())"#
            );
        }
    }

    mod parse_array {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn tuple_from_items_array() {
            assert_eq!(
                parse_array(
                    &json!({ "type": "array", "items": [{ "type": "string" }, { "type": "number" }] }),
                    &refs_v4()
                ),
                "z.tuple([z.string(),z.number()])"
            );
        }

        #[test]
        fn array_from_items_object() {
            assert_eq!(
                parse_array(
                    &json!({ "type": "array", "items": { "type": "string" } }),
                    &refs_v4()
                ),
                "z.array(z.string())"
            );
        }

        #[test]
        fn max_items() {
            assert_eq!(
                parse_array(
                    &json!({ "type": "array", "maxItems": 2, "items": { "type": "string" } }),
                    &refs_v4()
                ),
                "z.array(z.string()).max(2)"
            );
        }

        #[test]
        fn unique_items() {
            assert_eq!(
                parse_array(
                    &json!({ "type": "array", "uniqueItems": true, "items": { "type": "string" } }),
                    &refs_v4()
                ),
                r#"z.array(z.string()).refine((arr) => arr.every((item, i) => arr.indexOf(item) == i), "All items must be unique!")"#
            );
        }

        #[test]
        fn min_items() {
            assert_eq!(
                parse_array(
                    &json!({ "type": "array", "items": { "type": "string" }, "minItems": 3 }),
                    &refs_v4()
                ),
                "z.array(z.string()).min(3)"
            );
        }

        #[test]
        fn tuple_ignores_min_max_unique() {
            // A tuple takes the early return, so min, max, and unique never
            // attach.
            assert_eq!(
                parse_array(
                    &json!({
                        "type": "array",
                        "items": [{ "type": "string" }],
                        "minItems": 1,
                        "maxItems": 2,
                        "uniqueItems": true
                    }),
                    &refs_v4()
                ),
                "z.tuple([z.string()])"
            );
        }
    }

    mod parse_all_of {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn empty_is_never() {
            assert_eq!(
                parse_all_of(&json!({ "allOf": [] }), &refs_v4()),
                "z.never()"
            );
        }

        #[test]
        fn boolean_true_member() {
            assert_eq!(
                parse_all_of(
                    &json!({ "allOf": [{ "type": "string" }, true] }),
                    &refs_v4()
                ),
                "z.intersection(z.string(), z.any())"
            );
        }

        #[test]
        fn boolean_false_member() {
            assert_eq!(
                parse_all_of(
                    &json!({ "allOf": [{ "type": "string" }, false] }),
                    &refs_v4()
                ),
                r#"z.intersection(z.string(), z.any().refine((value) => !z.any().safeParse(value).success, "Invalid input: Should NOT be valid against schema"))"#
            );
        }

        #[test]
        fn three_members_split_right_leaning() {
            // half([boolean, number, string]) -> left [boolean], right [number, string].
            assert_eq!(
                parse_all_of(
                    &json!({ "allOf": [{ "type": "boolean" }, { "type": "number" }, { "type": "string" }] }),
                    &refs_v4()
                ),
                "z.intersection(z.boolean(), z.intersection(z.number(), z.string()))"
            );
        }

        #[test]
        fn four_members_split_evenly() {
            // half([string, number, boolean, null]) -> [string, number] and
            // [boolean, null], so each side nests one more intersection.
            assert_eq!(
                parse_all_of(
                    &json!({ "allOf": [
                        { "type": "string" },
                        { "type": "number" },
                        { "type": "boolean" },
                        { "type": "null" }
                    ] }),
                    &refs_v4()
                ),
                "z.intersection(z.intersection(z.string(), z.number()), z.intersection(z.boolean(), z.null()))"
            );
        }
    }

    mod parse_any_of {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn union_from_two_or_more() {
            assert_eq!(
                parse_any_of(
                    &json!({ "anyOf": [{ "type": "string" }, { "type": "number" }] }),
                    &refs_v4()
                ),
                "z.union([z.string(), z.number()])"
            );
        }

        #[test]
        fn single_schema_unwraps() {
            assert_eq!(
                parse_any_of(&json!({ "anyOf": [{ "type": "string" }] }), &refs_v4()),
                "z.string()"
            );
        }

        #[test]
        fn empty_is_any() {
            assert_eq!(parse_any_of(&json!({ "anyOf": [] }), &refs_v4()), "z.any()");
        }
    }

    mod parse_one_of {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn v3_union_from_two_or_more() {
            let expected = r#"z.any().superRefine((x, ctx) => {
    const schemas = [z.string(), z.number()];
    const { errors, failed } = schemas.reduce<{
      errors: z.ZodError[];
      failed: number;
    }>(
      ({ errors, failed }, schema) =>
        ((result) =>
          result.error
            ? {
                errors: [...errors, result.error],
                failed: failed + 1,
              }
            : { errors, failed })(
          schema.safeParse(x),
        ),
      { errors: [], failed: 0 },
    );
    const passed = schemas.length - failed;
    if (passed !== 1) {
      ctx.addIssue(errors.length ? {
        path: ctx.path,
        code: "invalid_union",
        unionErrors: errors,
        message: "Invalid input: Should pass single schema. Passed " + passed,
      } : {
        path: ctx.path,
        code: "custom",
        message: "Invalid input: Should pass single schema. Passed " + passed,
      });
    }
  })"#;
            assert_eq!(
                parse_one_of(
                    &json!({ "oneOf": [{ "type": "string" }, { "type": "number" }] }),
                    &refs_v3()
                ),
                expected
            );
        }

        #[test]
        fn single_schema_unwraps() {
            assert_eq!(
                parse_one_of(&json!({ "oneOf": [{ "type": "string" }] }), &refs_v4()),
                "z.string()"
            );
        }

        #[test]
        fn empty_is_any() {
            assert_eq!(parse_one_of(&json!({ "oneOf": [] }), &refs_v4()), "z.any()");
        }
    }

    mod parse_not {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn refine_rejects_inner_schema() {
            assert_eq!(
                parse_not(&json!({ "not": { "type": "string" } }), &refs_v4()),
                r#"z.any().refine((value) => !z.string().safeParse(value).success, "Invalid input: Should NOT be valid against schema")"#
            );
        }
    }

    mod parse_object {
        #[allow(unused_imports)]
        use super::*;

        #[test]
        fn missing_properties() {
            assert_eq!(
                parse_object(&json!({ "type": "object" }), &refs_v4()),
                "z.record(z.string(), z.any())"
            );
        }

        #[test]
        fn empty_properties() {
            assert_eq!(
                parse_object(&json!({ "type": "object", "properties": {} }), &refs_v4()),
                "z.object({})"
            );
        }

        #[test]
        fn optional_and_required_properties() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["myRequiredString"],
                        "properties": {
                            "myOptionalString": { "type": "string" },
                            "myRequiredString": { "type": "string" }
                        }
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "myOptionalString": z.string().optional(), "myRequiredString": z.string() })"#
            );
        }

        #[test]
        fn additional_properties_false_with_props() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["myString"],
                        "properties": { "myString": { "type": "string" } },
                        "additionalProperties": false
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "myString": z.string() }).strict()"#
            );
        }

        #[test]
        fn additional_properties_true_with_props() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["myString"],
                        "properties": { "myString": { "type": "string" } },
                        "additionalProperties": true
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "myString": z.string() }).catchall(z.any())"#
            );
        }

        #[test]
        fn additional_properties_schema_with_props() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["myString"],
                        "properties": { "myString": { "type": "string" } },
                        "additionalProperties": { "type": "number" }
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "myString": z.string() }).catchall(z.number())"#
            );
        }

        #[test]
        fn additional_properties_false_without_props() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": false }),
                    &refs_v4()
                ),
                "z.record(z.string(), z.never())"
            );
        }

        #[test]
        fn additional_properties_true_without_props() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": true }),
                    &refs_v4()
                ),
                "z.record(z.string(), z.any())"
            );
        }

        #[test]
        fn additional_properties_schema_without_props() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": { "type": "number" } }),
                    &refs_v4()
                ),
                "z.record(z.string(), z.number())"
            );
        }

        #[test]
        fn falsy_default_in_prop() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "properties": { "s": { "type": "string", "default": "" } }
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "s": z.string().default("") })"#
            );
        }

        #[test]
        fn object_with_any_of() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" } },
                        "anyOf": [
                            { "required": ["b"], "properties": { "b": { "type": "string" } } },
                            { "required": ["c"], "properties": { "c": { "type": "string" } } }
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "a": z.string() }).and(z.union([z.object({ "b": z.string() }), z.object({ "c": z.string() })]))"#
            );
        }

        #[test]
        fn object_with_any_of_empty_member() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" } },
                        "anyOf": [
                            { "required": ["b"], "properties": { "b": { "type": "string" } } },
                            {}
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "a": z.string() }).and(z.union([z.object({ "b": z.string() }), z.any()]))"#
            );
        }

        #[test]
        fn object_with_one_of() {
            let expected = r#"z.object({ "a": z.string() }).and(z.any().superRefine((x, ctx) => {
    const schemas = [z.object({ "b": z.string() }), z.object({ "c": z.string() })];
    const { errors, failed } = schemas.reduce<{
      errors: z.core.$ZodIssue[];
      failed: number;
    }>(
      ({ errors, failed }, schema) =>
        ((result) =>
          result.error
            ? {
                errors: [...errors, ...result.error.issues],
                failed: failed + 1,
              }
            : { errors, failed })(
          schema.safeParse(x),
        ),
      { errors: [], failed: 0 },
    );
    const passed = schemas.length - failed;
    if (passed !== 1) {
      ctx.addIssue(errors.length ? {
        path: [],
        code: "invalid_union",
        errors: [errors],
        message: "Invalid input: Should pass single schema. Passed " + passed,
      } : {
        path: [],
        code: "custom",
        errors: [errors],
        message: "Invalid input: Should pass single schema. Passed " + passed,
      });
    }
  }))"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" } },
                        "oneOf": [
                            { "required": ["b"], "properties": { "b": { "type": "string" } } },
                            { "required": ["c"], "properties": { "c": { "type": "string" } } }
                        ]
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn object_with_one_of_empty_member() {
            let expected = r#"z.object({ "a": z.string() }).and(z.any().superRefine((x, ctx) => {
    const schemas = [z.object({ "b": z.string() }), z.any()];
    const { errors, failed } = schemas.reduce<{
      errors: z.core.$ZodIssue[];
      failed: number;
    }>(
      ({ errors, failed }, schema) =>
        ((result) =>
          result.error
            ? {
                errors: [...errors, ...result.error.issues],
                failed: failed + 1,
              }
            : { errors, failed })(
          schema.safeParse(x),
        ),
      { errors: [], failed: 0 },
    );
    const passed = schemas.length - failed;
    if (passed !== 1) {
      ctx.addIssue(errors.length ? {
        path: [],
        code: "invalid_union",
        errors: [errors],
        message: "Invalid input: Should pass single schema. Passed " + passed,
      } : {
        path: [],
        code: "custom",
        errors: [errors],
        message: "Invalid input: Should pass single schema. Passed " + passed,
      });
    }
  }))"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" } },
                        "oneOf": [
                            { "required": ["b"], "properties": { "b": { "type": "string" } } },
                            {}
                        ]
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn object_with_all_of() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" } },
                        "allOf": [
                            { "required": ["b"], "properties": { "b": { "type": "string" } } },
                            { "required": ["c"], "properties": { "c": { "type": "string" } } }
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "a": z.string() }).and(z.intersection(z.object({ "b": z.string() }), z.object({ "c": z.string() })))"#
            );
        }

        #[test]
        fn object_with_all_of_empty_member() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" } },
                        "allOf": [
                            { "required": ["b"], "properties": { "b": { "type": "string" } } },
                            {}
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "a": z.string() }).and(z.intersection(z.object({ "b": z.string() }), z.any()))"#
            );
        }

        #[test]
        fn functional_properties_shape() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" }, "b": { "type": "number" } }
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "a": z.string(), "b": z.number().optional() })"#
            );
        }

        #[test]
        fn properties_and_additional_properties() {
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" }, "b": { "type": "number" } },
                        "additionalProperties": { "type": "boolean" }
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "a": z.string(), "b": z.number().optional() }).catchall(z.boolean())"#
            );
        }

        #[test]
        fn properties_and_single_pattern_properties() {
            let expected = r#"z.object({ "a": z.string(), "b": z.number().optional() }).catchall(z.array(z.any())).superRefine((value, ctx) => {
for (const key in value) {
if (key.match(new RegExp("\\."))) {
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" }, "b": { "type": "number" } },
                        "patternProperties": { "\\.": { "type": "array" } }
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn properties_additional_and_pattern_properties() {
            let expected = r#"z.object({ "a": z.string(), "b": z.number().optional() }).catchall(z.union([z.array(z.any()), z.array(z.any()).min(1), z.boolean()])).superRefine((value, ctx) => {
for (const key in value) {
let evaluated = ["a", "b"].includes(key)
if (key.match(new RegExp("\\."))) {
evaluated = true
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
if (key.match(new RegExp("\\,"))) {
evaluated = true
const result = z.array(z.any()).min(1).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
if (!evaluated) {
const result = z.boolean().safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: must match catchall schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" }, "b": { "type": "number" } },
                        "additionalProperties": { "type": "boolean" },
                        "patternProperties": {
                            "\\.": { "type": "array" },
                            "\\,": { "type": "array", "minItems": 1 }
                        }
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn additional_properties_only() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": { "type": "boolean" } }),
                    &refs_v4()
                ),
                "z.record(z.string(), z.boolean())"
            );
        }

        #[test]
        fn additional_and_pattern_properties() {
            let expected = r#"z.record(z.string(), z.union([z.array(z.any()), z.array(z.any()).min(1), z.boolean()])).superRefine((value, ctx) => {
for (const key in value) {
let evaluated = false
if (key.match(new RegExp("\\."))) {
evaluated = true
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
if (key.match(new RegExp("\\,"))) {
evaluated = true
const result = z.array(z.any()).min(1).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
if (!evaluated) {
const result = z.boolean().safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: must match catchall schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "additionalProperties": { "type": "boolean" },
                        "patternProperties": {
                            "\\.": { "type": "array" },
                            "\\,": { "type": "array", "minItems": 1 }
                        }
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn single_item_pattern_properties() {
            let expected = r#"z.record(z.string(), z.array(z.any())).superRefine((value, ctx) => {
for (const key in value) {
if (key.match(new RegExp("\\."))) {
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "patternProperties": { "\\.": { "type": "array" } }
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn pattern_properties_multi() {
            let expected = r#"z.record(z.string(), z.union([z.array(z.any()), z.array(z.any()).min(1)])).superRefine((value, ctx) => {
for (const key in value) {
if (key.match(new RegExp("\\."))) {
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
if (key.match(new RegExp("\\,"))) {
const result = z.array(z.any()).min(1).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "patternProperties": {
                            "\\.": { "type": "array" },
                            "\\,": { "type": "array", "minItems": 1 }
                        }
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn pattern_properties_and_properties() {
            let expected = r#"z.object({ "a": z.string(), "b": z.number().optional() }).catchall(z.union([z.array(z.any()), z.array(z.any()).min(1)])).superRefine((value, ctx) => {
for (const key in value) {
if (key.match(new RegExp("\\."))) {
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
if (key.match(new RegExp("\\,"))) {
const result = z.array(z.any()).min(1).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "required": ["a"],
                        "properties": { "a": { "type": "string" }, "b": { "type": "number" } },
                        "patternProperties": {
                            "\\.": { "type": "array" },
                            "\\,": { "type": "array", "minItems": 1 }
                        }
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        // Zod v3 variants.

        #[test]
        fn v3_missing_properties() {
            assert_eq!(
                parse_object(&json!({ "type": "object" }), &refs_v3()),
                "z.record(z.any())"
            );
        }

        #[test]
        fn v3_additional_properties_false_without_props() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": false }),
                    &refs_v3()
                ),
                "z.record(z.never())"
            );
        }

        #[test]
        fn v3_additional_properties_true_without_props() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": true }),
                    &refs_v3()
                ),
                "z.record(z.any())"
            );
        }

        #[test]
        fn v3_additional_properties_schema_without_props() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": { "type": "number" } }),
                    &refs_v3()
                ),
                "z.record(z.number())"
            );
        }

        #[test]
        fn v3_additional_properties_functional() {
            assert_eq!(
                parse_object(
                    &json!({ "type": "object", "additionalProperties": { "type": "boolean" } }),
                    &refs_v3()
                ),
                "z.record(z.boolean())"
            );
        }

        #[test]
        fn v3_pattern_properties_uses_ctx_path() {
            let expected = r#"z.record(z.array(z.any())).superRefine((value, ctx) => {
for (const key in value) {
if (key.match(new RegExp("\\."))) {
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [...ctx.path, key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "patternProperties": { "\\.": { "type": "array" } }
                    }),
                    &refs_v3()
                ),
                expected
            );
        }

        #[test]
        fn v3_additional_and_pattern_properties() {
            let expected = r#"z.record(z.union([z.array(z.any()), z.boolean()])).superRefine((value, ctx) => {
for (const key in value) {
let evaluated = false
if (key.match(new RegExp("\\."))) {
evaluated = true
const result = z.array(z.any()).safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [...ctx.path, key],
          code: 'custom',
          message: `Invalid input: Key matching regex /${key}/ must match schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
if (!evaluated) {
const result = z.boolean().safeParse(value[key])
if (!result.success) {
ctx.addIssue({
          path: [...ctx.path, key],
          code: 'custom',
          message: `Invalid input: must match catchall schema`,
          params: {
            issues: result.error.issues
          }
        })
}
}
}
})"#;
            assert_eq!(
                parse_object(
                    &json!({
                        "type": "object",
                        "additionalProperties": { "type": "boolean" },
                        "patternProperties": { "\\.": { "type": "array" } }
                    }),
                    &refs_v3()
                ),
                expected
            );
        }
    }

    mod parse_simple_discriminated {
        #[allow(unused_imports)]
        use super::*;

        fn guard(v: Value) -> bool {
            is_simple_discriminated_one_of(&v)
        }

        #[test]
        fn emit_two_or_more_members() {
            assert_eq!(
                parse_simple_discriminated_one_of(
                    &json!({
                        "discriminator": { "propertyName": "objectType" },
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": { "objectType": { "type": "string", "enum": ["typeA"] } },
                                "required": ["objectType"]
                            },
                            {
                                "type": "object",
                                "properties": { "objectType": { "type": "string", "enum": ["typeB"] } },
                                "required": ["objectType"]
                            }
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.discriminatedUnion("objectType", [z.object({ "objectType": z.literal("typeA") }), z.object({ "objectType": z.literal("typeB") })])"#
            );
        }

        #[test]
        fn emit_single_member_unwraps() {
            assert_eq!(
                parse_simple_discriminated_one_of(
                    &json!({
                        "discriminator": { "propertyName": "objectType" },
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": { "objectType": { "type": "string", "enum": ["typeA"] } },
                                "required": ["objectType"]
                            }
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "objectType": z.literal("typeA") })"#
            );
        }

        #[test]
        fn emit_empty_is_any() {
            assert_eq!(
                parse_simple_discriminated_one_of(
                    &json!({ "oneOf": [], "discriminator": { "propertyName": "objectType" } }),
                    &refs_v4()
                ),
                "z.any()"
            );
        }

        #[test]
        fn emit_const_discriminator() {
            assert_eq!(
                parse_simple_discriminated_one_of(
                    &json!({
                        "discriminator": { "propertyName": "kind" },
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": {
                                    "kind": { "type": "string", "const": "person" },
                                    "name": { "type": "string" }
                                },
                                "required": ["kind", "name"]
                            },
                            {
                                "type": "object",
                                "properties": {
                                    "kind": { "type": "string", "const": "company" },
                                    "companyName": { "type": "string" }
                                },
                                "required": ["kind", "companyName"]
                            }
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.discriminatedUnion("kind", [z.object({ "kind": z.literal("person"), "name": z.string() }), z.object({ "kind": z.literal("company"), "companyName": z.string() })])"#
            );
        }

        #[test]
        fn property_name_is_json_escaped() {
            // A propertyName with a quote must be escaped, not interpolated raw.
            assert_eq!(
                parse_simple_discriminated_one_of(
                    &json!({
                        "discriminator": { "propertyName": "a\"b" },
                        "oneOf": [
                            {
                                "type": "object",
                                "properties": { "x": { "type": "string", "enum": ["typeA"] } },
                                "required": ["x"]
                            },
                            {
                                "type": "object",
                                "properties": { "x": { "type": "string", "enum": ["typeB"] } },
                                "required": ["x"]
                            }
                        ]
                    }),
                    &refs_v4()
                ),
                r#"z.discriminatedUnion("a\"b", [z.object({ "x": z.literal("typeA") }), z.object({ "x": z.literal("typeB") })])"#
            );
        }

        // Type guard accept cases.

        #[test]
        fn guard_accepts_const_values() {
            assert!(guard(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "const": "A" },
                            "value": { "type": "string" }
                        },
                        "required": ["type", "value"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "const": "B" },
                            "count": { "type": "number" }
                        },
                        "required": ["type", "count"]
                    }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_accepts_single_value_enum() {
            assert!(guard(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "enum": ["person"] },
                            "name": { "type": "string" }
                        },
                        "required": ["kind", "name"]
                    }
                ],
                "discriminator": { "propertyName": "kind" }
            })));
        }

        #[test]
        fn guard_accepts_discriminator_in_required() {
            assert!(guard(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "const": "A" },
                            "value": { "type": "string" }
                        },
                        "required": ["type", "value"]
                    }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        // Type guard reject cases.

        #[test]
        fn guard_rejects_numeric_discriminator() {
            assert!(!guard(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "version": { "type": "number", "const": 1 },
                            "data": { "type": "string" }
                        }
                    }
                ],
                "discriminator": { "propertyName": "version" }
            })));
        }

        #[test]
        fn guard_rejects_no_one_of() {
            assert!(!guard(
                json!({ "discriminator": { "propertyName": "type" } })
            ));
        }

        #[test]
        fn guard_rejects_no_discriminator() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
                ]
            })));
        }

        #[test]
        fn guard_rejects_empty_one_of() {
            assert!(!guard(json!({
                "oneOf": [],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_discriminator_without_property_name() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
                ],
                "discriminator": {}
            })));
        }

        #[test]
        fn guard_rejects_non_string_property_name() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
                ],
                "discriminator": { "propertyName": 123 }
            })));
        }

        #[test]
        fn guard_rejects_non_object_one_of_member() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "string" },
                    { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_member_missing_discriminator_prop() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "object", "properties": { "value": { "type": "string" } } }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_prop_without_const_or_single_enum() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "object", "properties": { "type": { "type": "string" } } }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_multi_value_enum() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "object", "properties": { "type": { "type": "string", "enum": ["A", "B"] } } }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_member_without_properties() {
            assert!(!guard(json!({
                "oneOf": [{ "type": "object" }],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_unsupported_discriminator_type() {
            assert!(!guard(json!({
                "oneOf": [
                    { "type": "object", "properties": { "type": { "type": "boolean", "const": true } } }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_null_and_undefined_one_of() {
            assert!(!guard(json!({
                "oneOf": null,
                "discriminator": { "propertyName": "type" }
            })));
            // Absent oneOf stands in for JS undefined.
            assert!(!guard(json!({
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_discriminator_not_in_required() {
            assert!(!guard(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "const": "A" },
                            "value": { "type": "string" }
                        },
                        "required": ["value"]
                    }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_no_required_array() {
            assert!(!guard(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "const": "A" },
                            "value": { "type": "string" }
                        }
                    }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }

        #[test]
        fn guard_rejects_non_array_required() {
            assert!(!guard(json!({
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "type": { "type": "string", "const": "A" },
                            "value": { "type": "string" }
                        },
                        "required": true
                    }
                ],
                "discriminator": { "propertyName": "type" }
            })));
        }
    }
}
