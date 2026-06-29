use json_schema_to_zod::parse_enum;
use serde_json::json;

#[test]
fn empty_enum_is_never() {
    assert_eq!(parse_enum(&json!({ "enum": [] })), "z.never()");
}

#[test]
fn single_item_enum_is_literal() {
    assert_eq!(
        parse_enum(&json!({ "enum": ["someValue"] })),
        r#"z.literal("someValue")"#
    );
}

#[test]
fn all_string_enum_is_enum_array() {
    assert_eq!(
        parse_enum(&json!({ "enum": ["someValue", "anotherValue"] })),
        r#"z.enum(["someValue","anotherValue"])"#
    );
}

#[test]
fn mixed_enum_is_union() {
    assert_eq!(
        parse_enum(&json!({ "enum": ["someValue", 57] })),
        r#"z.union([z.literal("someValue"), z.literal(57)])"#
    );
}
