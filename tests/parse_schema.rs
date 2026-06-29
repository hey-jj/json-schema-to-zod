mod common;

use common::refs_v4;
use json_schema_to_zod::{parse_schema, Refs};
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
    assert_eq!(parse_schema(&json!({ "type": "boolean" }), &refs_v4()), "z.boolean()");
    assert_eq!(parse_schema(&json!({ "type": "null" }), &refs_v4()), "z.null()");
}

#[test]
fn empty_schema_falls_through_to_any() {
    // {} has no type, so no construct matches and the fallback is z.any().
    assert_eq!(parse_schema(&json!({}), &refs_v4()), "z.any()");
    // type "any" is not a recognized primitive, so it also falls through.
    assert_eq!(parse_schema(&json!({ "type": "any" }), &refs_v4()), "z.any()");
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
        parse_schema(&json!({ "type": "string", "format": "hostname" }), &refs_v4()),
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
        parse_schema(&json!({ "type": "string", "description": "line1\nline2" }), &refs_v4()),
        r#"z.string().describe("line1\nline2")"#
    );
}
