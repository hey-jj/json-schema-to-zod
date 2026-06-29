use json_schema_to_zod::parse_const;
use serde_json::json;

#[test]
fn falsy_constant() {
    assert_eq!(parse_const(&json!({ "const": false })), "z.literal(false)");
}

#[test]
fn empty_string_constant() {
    assert_eq!(parse_const(&json!({ "const": "" })), r#"z.literal("")"#);
}
