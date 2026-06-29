mod common;

use common::refs_v4;
use json_schema_to_zod::parse_not;
use serde_json::json;

#[test]
fn refine_rejects_inner_schema() {
    assert_eq!(
        parse_not(&json!({ "not": { "type": "string" } }), &refs_v4()),
        r#"z.any().refine((value) => !z.string().safeParse(value).success, "Invalid input: Should NOT be valid against schema")"#
    );
}
