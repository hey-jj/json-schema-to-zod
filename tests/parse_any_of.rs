mod common;

use common::refs_v4;
use json_schema_to_zod::parse_any_of;
use serde_json::json;

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
