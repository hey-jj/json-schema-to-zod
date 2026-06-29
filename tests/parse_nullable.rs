mod common;

use common::refs_v4;
use json_schema_to_zod::parse_schema;
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
