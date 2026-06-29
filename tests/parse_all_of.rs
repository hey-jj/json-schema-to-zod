mod common;

use common::refs_v4;
use json_schema_to_zod::parse_all_of;
use serde_json::json;

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
