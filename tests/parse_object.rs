mod common;

use common::{refs_v3, refs_v4};
use json_schema_to_zod::parse_object;
use serde_json::json;

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
