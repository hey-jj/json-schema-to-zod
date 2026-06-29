mod common;

use common::refs_v4;
use json_schema_to_zod::parse_schema;
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
