use json_schema_to_zod::parse_number;
use serde_json::json;

#[test]
fn integer() {
    assert_eq!(parse_number(&json!({ "type": "integer" })), "z.number().int()");
    assert_eq!(
        parse_number(&json!({ "type": "integer", "multipleOf": 1 })),
        "z.number().int()"
    );
    assert_eq!(
        parse_number(&json!({ "type": "number", "multipleOf": 1 })),
        "z.number().int()"
    );
}

#[test]
fn minimum_with_exclusive_minimum_true() {
    assert_eq!(
        parse_number(&json!({ "type": "number", "exclusiveMinimum": true, "minimum": 2 })),
        "z.number().gt(2)"
    );
}

#[test]
fn plain_minimum() {
    assert_eq!(
        parse_number(&json!({ "type": "number", "minimum": 2 })),
        "z.number().gte(2)"
    );
}

#[test]
fn maximum_with_exclusive_maximum_true() {
    assert_eq!(
        parse_number(&json!({ "type": "number", "exclusiveMaximum": true, "maximum": 2 })),
        "z.number().lt(2)"
    );
}

#[test]
fn numeric_exclusive_maximum() {
    assert_eq!(
        parse_number(&json!({ "type": "number", "exclusiveMaximum": 2 })),
        "z.number().lt(2)"
    );
}

#[test]
fn error_messages_per_key() {
    assert_eq!(
        parse_number(&json!({
            "type": "number",
            "format": "int64",
            "exclusiveMinimum": 0,
            "maximum": 2,
            "multipleOf": 2,
            "errorMessage": {
                "format": "ayy",
                "multipleOf": "lmao",
                "exclusiveMinimum": "deez",
                "maximum": "nuts"
            }
        })),
        r#"z.number().int("ayy").multipleOf(2, "lmao").gt(0, "deez").lte(2, "nuts")"#
    );
}
