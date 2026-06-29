mod common;

use common::{refs_v3, refs_v4};
use json_schema_to_zod::parse_one_of;
use serde_json::json;

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
