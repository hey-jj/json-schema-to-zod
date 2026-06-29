//! The recursive dispatcher and metadata application.

use serde_json::Value;

use crate::parsers;
use crate::predicates as its;
use crate::types::{Refs, Seen};
use crate::util::{compact_json, truthy};

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
        return if truthy(schema) {
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
        if truthy(desc) {
            parsed.push_str(&format!(".describe({})", compact_json(desc)));
        }
    }
    parsed
}

/// Append `.default(<json>)` when `default` is present and not null/undefined.
fn add_defaults(schema: &Value, mut parsed: String) -> String {
    if let Some(default) = schema.get("default") {
        // `default !== undefined` in JS. An explicit `null` default is kept.
        parsed.push_str(&format!(".default({})", compact_json(default)));
    }
    parsed
}

/// Append `.readonly()` when `readOnly` is truthy.
fn add_annotations(schema: &Value, mut parsed: String) -> String {
    if let Some(read_only) = schema.get("readOnly") {
        if truthy(read_only) {
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

#[cfg(test)]
mod tests {
    use crate::types::Refs;

    /// Two-argument dispatch wrapper for the tests.
    fn parse_schema(schema: &serde_json::Value, refs: &Refs) -> String {
        super::parse_schema(schema, refs, false)
    }

    fn refs_v4() -> Refs {
        Refs::default_v4()
    }

    mod parse_schema_mod {
        #[allow(unused_imports)]
        use super::{parse_schema, refs_v4};
        use crate::types::Refs;
        use serde_json::json;

        #[test]
        fn usable_without_providing_refs() {
            assert_eq!(
                parse_schema(&json!({ "type": "string" }), &Refs::default_v4()),
                "z.string()"
            );
        }

        #[test]
        fn returns_a_seen_and_processed_ref() {
            let refs = refs_v4();
            let schema = json!({
                "type": "object",
                "properties": { "prop": { "type": "string" } }
            });
            let first = parse_schema(&schema, &refs);
            let second = parse_schema(&schema, &refs);
            assert!(!first.is_empty());
            assert!(!second.is_empty());
            // A second visit to the same node returns the memoized string.
            assert_eq!(first, second);
        }

        #[test]
        fn readonly_schema() {
            assert_eq!(
                parse_schema(&json!({ "type": "string", "readOnly": true }), &refs_v4()),
                "z.string().readonly()"
            );
        }

        #[test]
        fn nullable() {
            assert_eq!(
                parse_schema(&json!({ "type": "string", "nullable": true }), &refs_v4()),
                "z.string().nullable()"
            );
        }

        #[test]
        fn enum_mixed() {
            assert_eq!(
                parse_schema(&json!({ "enum": ["someValue", 57] }), &refs_v4()),
                r#"z.union([z.literal("someValue"), z.literal(57)])"#
            );
        }

        #[test]
        fn multiple_type() {
            assert_eq!(
                parse_schema(&json!({ "type": ["string", "number"] }), &refs_v4()),
                "z.union([z.string(), z.number()])"
            );
        }

        #[test]
        fn if_then_else() {
            let expected = r#"z.union([z.number(), z.boolean()]).superRefine((value,ctx) => {
  const result = z.string().safeParse(value).success
    ? z.number().safeParse(value)
    : z.boolean().safeParse(value);
  if (!result.success) {
    result.error.errors.forEach((error) => ctx.addIssue(error))
  }
})"#;
            assert_eq!(
                parse_schema(
                    &json!({
                        "if": { "type": "string" },
                        "then": { "type": "number" },
                        "else": { "type": "boolean" }
                    }),
                    &refs_v4()
                ),
                expected
            );
        }

        #[test]
        fn any_of() {
            assert_eq!(
                parse_schema(
                    &json!({ "anyOf": [{ "type": "string" }, { "type": "number" }] }),
                    &refs_v4()
                ),
                "z.union([z.string(), z.number()])"
            );
        }

        #[test]
        fn one_of_v4() {
            let expected = r#"z.any().superRefine((x, ctx) => {
    const schemas = [z.string(), z.number()];
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
  })"#;
            assert_eq!(
                parse_schema(
                    &json!({ "oneOf": [{ "type": "string" }, { "type": "number" }] }),
                    &refs_v4()
                ),
                expected
            );
        }

        // Added pin tests for behavior the spec calls out but the corpus does not
        // exercise directly.

        #[test]
        fn bare_boolean_and_null_parsers() {
            assert_eq!(
                parse_schema(&json!({ "type": "boolean" }), &refs_v4()),
                "z.boolean()"
            );
            assert_eq!(
                parse_schema(&json!({ "type": "null" }), &refs_v4()),
                "z.null()"
            );
        }

        #[test]
        fn empty_schema_falls_through_to_any() {
            // {} has no type, so no construct matches and the fallback is z.any().
            assert_eq!(parse_schema(&json!({}), &refs_v4()), "z.any()");
            // type "any" is not a recognized primitive, so it also falls through.
            assert_eq!(
                parse_schema(&json!({ "type": "any" }), &refs_v4()),
                "z.any()"
            );
        }

        #[test]
        fn boolean_schema_values() {
            assert_eq!(parse_schema(&json!(true), &refs_v4()), "z.any()");
            assert_eq!(parse_schema(&json!(false), &refs_v4()), "z.never()");
        }

        #[test]
        fn partial_conditional_without_else_falls_through() {
            // The conditional guard needs if, then, and else. Missing else means no
            // construct matches and the fallback is z.any().
            assert_eq!(
                parse_schema(
                    &json!({ "if": { "type": "string" }, "then": { "type": "number" } }),
                    &refs_v4()
                ),
                "z.any()"
            );
        }

        #[test]
        fn ignored_object_keywords_are_dropped() {
            // propertyNames, minProperties, maxProperties, unevaluatedProperties are
            // recognized fields with no emitter. They do not change the output.
            let with_extra = parse_schema(
                &json!({
                    "type": "object",
                    "properties": { "a": { "type": "string" } },
                    "required": ["a"],
                    "propertyNames": { "type": "string" },
                    "minProperties": 1,
                    "maxProperties": 2,
                    "unevaluatedProperties": false
                }),
                &refs_v4(),
            );
            assert_eq!(with_extra, r#"z.object({ "a": z.string() })"#);
        }

        #[test]
        fn unknown_string_format_is_plain_string() {
            assert_eq!(
                parse_schema(
                    &json!({ "type": "string", "format": "hostname" }),
                    &refs_v4()
                ),
                "z.string()"
            );
        }

        #[test]
        fn json_escaping_in_literals_and_keys() {
            // const with a backslash, an object key with a quote, describe with a
            // newline. Each must use JSON.stringify escaping.
            assert_eq!(
                parse_schema(&json!({ "const": "back\\slash" }), &refs_v4()),
                r#"z.literal("back\\slash")"#
            );
            assert_eq!(
                parse_schema(
                    &json!({ "type": "object", "properties": { "a\"b": { "type": "string" } }, "required": ["a\"b"] }),
                    &refs_v4()
                ),
                r#"z.object({ "a\"b": z.string() })"#
            );
            assert_eq!(
                parse_schema(
                    &json!({ "type": "string", "description": "line1\nline2" }),
                    &refs_v4()
                ),
                r#"z.string().describe("line1\nline2")"#
            );
        }

        #[test]
        fn re_encounter_without_depth_bails_to_any() {
            // A node already in progress (in `seen` with no memoized result) and
            // no depth set returns z.any() on re-encounter.
            use crate::types::Seen;

            let node = json!({ "type": "string" });
            let id = &node as *const serde_json::Value as usize;
            let refs = refs_v4();
            refs.seen.borrow_mut().insert(id, Seen { n: 0, r: None });

            assert_eq!(super::super::parse_schema(&node, &refs, false), "z.any()");
        }

        #[test]
        fn depth_allows_one_re_expansion_then_bails() {
            // With depth 1, the first re-encounter increments the counter and
            // re-expands. The second re-encounter hits n >= depth and bails.
            use crate::types::Seen;

            let node = json!({ "type": "string" });
            let id = &node as *const serde_json::Value as usize;
            let mut refs = refs_v4();
            refs.depth = Some(1);
            refs.seen.borrow_mut().insert(id, Seen { n: 0, r: None });

            // n (0) < depth (1): increment to 1 and parse normally.
            assert_eq!(
                super::super::parse_schema(&node, &refs, false),
                "z.string()"
            );
            assert_eq!(refs.seen.borrow().get(&id).unwrap().n, 1);

            // The full parse memoized a result, so clear it to force the depth
            // branch again on the next visit.
            refs.seen.borrow_mut().get_mut(&id).unwrap().r = None;

            // n (1) >= depth (1): bail to z.any().
            assert_eq!(super::super::parse_schema(&node, &refs, false), "z.any()");
        }
    }

    mod parse_nullable {
        #[allow(unused_imports)]
        use super::{parse_schema, refs_v4};
        use serde_json::json;

        #[test]
        fn nullable_does_not_add_default_twice() {
            assert_eq!(
                parse_schema(
                    &json!({ "type": "string", "nullable": true, "default": null }),
                    &refs_v4()
                ),
                r#"z.string().nullable().default(null)"#
            );
        }
    }

    mod parse_multiple_type {
        #[allow(unused_imports)]
        use super::{parse_schema, refs_v4};
        use serde_json::json;

        #[test]
        fn multitype_property_default_applied_once() {
            assert_eq!(
                parse_schema(
                    &json!({
                        "type": "object",
                        "properties": {
                            "prop": { "type": ["string", "null"], "default": null }
                        }
                    }),
                    &refs_v4()
                ),
                r#"z.object({ "prop": z.union([z.string(), z.null()]).default(null) })"#
            );
        }

        #[test]
        fn bare_multiple_type_union() {
            assert_eq!(
                parse_schema(&json!({ "type": ["string", "number"] }), &refs_v4()),
                "z.union([z.string(), z.number()])"
            );
        }
    }
}
