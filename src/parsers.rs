//! One parser per JSON Schema construct. Each returns a Zod source fragment.

use serde_json::{json, Map, Value};

use crate::jsdocs::add_jsdocs;
use crate::parse_schema::parse_schema;
use crate::predicates as its;
use crate::types::{PathSegment, Refs, ZodVersion};
use crate::util::{half, json_stringify, json_stringify_str, with_message, Builder};

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
    format!("z.literal({})", json_stringify(c))
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
        return format!("z.literal({})", json_stringify(&values[0]));
    }
    if values.iter().all(|x| x.is_string()) {
        // The string branch joins map output with a comma and no space, since
        // upstream interpolates an array into a template literal without join.
        let joined = values
            .iter()
            .map(json_stringify)
            .collect::<Vec<_>>()
            .join(",");
        return format!("z.enum([{joined}])");
    }
    let joined = values
        .iter()
        .map(|x| format!("z.literal({})", json_stringify(x)))
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
                "email" => Some(Builder::Two(".email(".into(), ")".into())),
                "ip" => Some(Builder::Two(".ip(".into(), ")".into())),
                "ipv4" => Some(Builder::Three(
                    ".ip({ version: \"v4\"".into(),
                    ", message: ".into(),
                    " })".into(),
                )),
                "ipv6" => Some(Builder::Three(
                    ".ip({ version: \"v6\"".into(),
                    ", message: ".into(),
                    " })".into(),
                )),
                "uri" => Some(Builder::Two(".url(".into(), ")".into())),
                "uuid" => Some(Builder::Two(".uuid(".into(), ")".into())),
                "date-time" => Some(Builder::Three(
                    ".datetime({ offset: true".into(),
                    ", message: ".into(),
                    " })".into(),
                )),
                "time" => Some(Builder::Two(".time(".into(), ")".into())),
                "date" => Some(Builder::Two(".date(".into(), ")".into())),
                "binary" => Some(Builder::Two(".base64(".into(), ")".into())),
                "duration" => Some(Builder::Two(".duration(".into(), ")".into())),
                _ => None,
            }
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "pattern",
        |_value, json| {
            Some(Builder::Three(
                format!(".regex(new RegExp({json})"),
                ", ".into(),
                ")".into(),
            ))
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "minLength",
        |_value, json| {
            Some(Builder::Three(
                format!(".min({json}"),
                ", ".into(),
                ")".into(),
            ))
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "maxLength",
        |_value, json| {
            Some(Builder::Three(
                format!(".max({json}"),
                ", ".into(),
                ")".into(),
            ))
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "contentEncoding",
        |value, _json| {
            if value.as_str() == Some("base64") {
                Some(Builder::Two(".base64(".into(), ")".into()))
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
                Some(Builder::Three(
                    ".transform((str, ctx) => { try { return JSON.parse(str); } catch (err) { ctx.addIssue({ code: \"custom\", message: \"Invalid JSON\" }); }}".into(),
                    ", ".into(),
                    ")".into(),
                ))
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
                if value.is_object() {
                    // Parsed with fresh refs, independent of the parent walk.
                    let inner = parse_schema(value, &Refs::default_v4(), false);
                    Some(Builder::Three(
                        format!(".pipe({inner}"),
                        ", ".into(),
                        ")".into(),
                    ))
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
            |_v, _j| Some(Builder::Two(".int(".into(), ")".into())),
            None,
        ));
    } else {
        r.push_str(&with_message(
            schema,
            "format",
            |value, _json| {
                if value.as_str() == Some("int64") {
                    Some(Builder::Two(".int(".into(), ")".into()))
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
                return Some(Builder::Two(".int(".into(), ")".into()));
            }
            Some(Builder::Three(
                format!(".multipleOf({json}"),
                ", ".into(),
                ")".into(),
            ))
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
                    Some(Builder::Three(
                        format!(".gt({json}"),
                        ", ".into(),
                        ")".into(),
                    ))
                },
                None,
            ));
        } else {
            r.push_str(&with_message(
                schema,
                "minimum",
                |_v, json| {
                    Some(Builder::Three(
                        format!(".gte({json}"),
                        ", ".into(),
                        ")".into(),
                    ))
                },
                None,
            ));
        }
    } else if exclusive_min_is_number {
        r.push_str(&with_message(
            schema,
            "exclusiveMinimum",
            |_v, json| {
                Some(Builder::Three(
                    format!(".gt({json}"),
                    ", ".into(),
                    ")".into(),
                ))
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
                    Some(Builder::Three(
                        format!(".lt({json}"),
                        ", ".into(),
                        ")".into(),
                    ))
                },
                None,
            ));
        } else {
            r.push_str(&with_message(
                schema,
                "maximum",
                |_v, json| {
                    Some(Builder::Three(
                        format!(".lte({json}"),
                        ", ".into(),
                        ")".into(),
                    ))
                },
                None,
            ));
        }
    } else if exclusive_max_is_number {
        r.push_str(&with_message(
            schema,
            "exclusiveMaximum",
            |_v, json| {
                Some(Builder::Three(
                    format!(".lt({json}"),
                    ", ".into(),
                    ")".into(),
                ))
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
        // Tuple. Items join with a comma and no space, matching the upstream
        // array interpolation without join.
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
            Some(Builder::Three(
                format!(".min({json}"),
                ", ".into(),
                ")".into(),
            ))
        },
        None,
    ));

    r.push_str(&with_message(
        schema,
        "maxItems",
        |_v, json| {
            Some(Builder::Three(
                format!(".max({json}"),
                ", ".into(),
                ")".into(),
            ))
        },
        None,
    ));

    if schema.get("uniqueItems") == Some(&Value::Bool(true)) {
        r.push_str(&with_message(
            schema,
            "uniqueItems",
            |_v, _json| {
                Some(Builder::Three(
                    ".refine((arr) => arr.every((item, i) => arr.indexOf(item) == i)".into(),
                    ", ".into(),
                    ")".into(),
                ))
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
    format!("{}.nullable()", parse_schema(inner, refs, true))
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
    let types = schema.get("type").and_then(|t| t.as_array());
    let types = match types {
        Some(t) => t,
        None => return "z.union([])".to_string(),
    };

    let parts = types
        .iter()
        .map(|t| {
            let mut obj = schema.clone();
            if let Value::Object(map) = &mut obj {
                map.insert("type".to_string(), t.clone());
            }
            let interned = refs.intern(obj);
            let child = refs.with_path_without_defaults(refs.path.clone());
            parse_schema(interned, &child, false)
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
            return parse_schema(cleaned, &child, false);
        }
        return parse_schema(item, &child, false);
    }

    let tagged = ensure_original_index(all_of);
    let (left, right) = half(&tagged);
    let left_schema = refs.intern(json!({ "allOf": left }));
    let right_schema = refs.intern(json!({ "allOf": right }));
    format!(
        "z.intersection({}, {})",
        parse_all_of(left_schema, refs),
        parse_all_of(right_schema, refs)
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
    format!("z.discriminatedUnion(\"{prop}\", [{parts}])")
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
    result.error.errors.forEach((error) => ctx.addIssue(error))
  }}
}})"#
    )
}

/// Coerce a combinator member that bears object keywords but no `type` into an
/// object schema, matching the upstream injection of `type: "object"`.
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
                        json_stringify_str(prop_key),
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
        output.push_str(&format!(".and({})", parse_any_of(mapped, refs)));
    }
    if its::has_one_of(schema) {
        let mapped = refs.intern(map_combinator(schema, "oneOf"));
        output.push_str(&format!(".and({})", parse_one_of(mapped, refs)));
    }
    if its::has_all_of(schema) {
        let mapped = refs.intern(map_combinator(schema, "allOf"));
        output.push_str(&format!(".and({})", parse_all_of(mapped, refs)));
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
/// additionalProperties following the upstream precedence.
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
                .map(|k| json_stringify_str(k))
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
            json_stringify_str(pkey)
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

/// JS truthiness for a JSON value.
fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
