mod common;

use common::refs_v4;
use json_schema_to_zod::parse_array;
use serde_json::json;

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
