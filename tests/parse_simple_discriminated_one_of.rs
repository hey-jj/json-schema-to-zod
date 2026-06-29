mod common;

use common::refs_v4;
use json_schema_to_zod::{is_simple_discriminated_one_of, parse_simple_discriminated_one_of};
use serde_json::{json, Value};

fn guard(v: Value) -> bool {
    is_simple_discriminated_one_of(&v)
}

#[test]
fn emit_two_or_more_members() {
    assert_eq!(
        parse_simple_discriminated_one_of(
            &json!({
                "discriminator": { "propertyName": "objectType" },
                "oneOf": [
                    {
                        "type": "object",
                        "properties": { "objectType": { "type": "string", "enum": ["typeA"] } },
                        "required": ["objectType"]
                    },
                    {
                        "type": "object",
                        "properties": { "objectType": { "type": "string", "enum": ["typeB"] } },
                        "required": ["objectType"]
                    }
                ]
            }),
            &refs_v4()
        ),
        r#"z.discriminatedUnion("objectType", [z.object({ "objectType": z.literal("typeA") }), z.object({ "objectType": z.literal("typeB") })])"#
    );
}

#[test]
fn emit_single_member_unwraps() {
    assert_eq!(
        parse_simple_discriminated_one_of(
            &json!({
                "discriminator": { "propertyName": "objectType" },
                "oneOf": [
                    {
                        "type": "object",
                        "properties": { "objectType": { "type": "string", "enum": ["typeA"] } },
                        "required": ["objectType"]
                    }
                ]
            }),
            &refs_v4()
        ),
        r#"z.object({ "objectType": z.literal("typeA") })"#
    );
}

#[test]
fn emit_empty_is_any() {
    assert_eq!(
        parse_simple_discriminated_one_of(
            &json!({ "oneOf": [], "discriminator": { "propertyName": "objectType" } }),
            &refs_v4()
        ),
        "z.any()"
    );
}

#[test]
fn emit_const_discriminator() {
    assert_eq!(
        parse_simple_discriminated_one_of(
            &json!({
                "discriminator": { "propertyName": "kind" },
                "oneOf": [
                    {
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "const": "person" },
                            "name": { "type": "string" }
                        },
                        "required": ["kind", "name"]
                    },
                    {
                        "type": "object",
                        "properties": {
                            "kind": { "type": "string", "const": "company" },
                            "companyName": { "type": "string" }
                        },
                        "required": ["kind", "companyName"]
                    }
                ]
            }),
            &refs_v4()
        ),
        r#"z.discriminatedUnion("kind", [z.object({ "kind": z.literal("person"), "name": z.string() }), z.object({ "kind": z.literal("company"), "companyName": z.string() })])"#
    );
}

// Type guard accept cases.

#[test]
fn guard_accepts_const_values() {
    assert!(guard(json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": "A" },
                    "value": { "type": "string" }
                },
                "required": ["type", "value"]
            },
            {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": "B" },
                    "count": { "type": "number" }
                },
                "required": ["type", "count"]
            }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_accepts_single_value_enum() {
    assert!(guard(json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["person"] },
                    "name": { "type": "string" }
                },
                "required": ["kind", "name"]
            }
        ],
        "discriminator": { "propertyName": "kind" }
    })));
}

#[test]
fn guard_accepts_discriminator_in_required() {
    assert!(guard(json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": "A" },
                    "value": { "type": "string" }
                },
                "required": ["type", "value"]
            }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

// Type guard reject cases.

#[test]
fn guard_rejects_numeric_discriminator() {
    assert!(!guard(json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "version": { "type": "number", "const": 1 },
                    "data": { "type": "string" }
                }
            }
        ],
        "discriminator": { "propertyName": "version" }
    })));
}

#[test]
fn guard_rejects_no_one_of() {
    assert!(!guard(
        json!({ "discriminator": { "propertyName": "type" } })
    ));
}

#[test]
fn guard_rejects_no_discriminator() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
        ]
    })));
}

#[test]
fn guard_rejects_empty_one_of() {
    assert!(!guard(json!({
        "oneOf": [],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_discriminator_without_property_name() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
        ],
        "discriminator": {}
    })));
}

#[test]
fn guard_rejects_non_string_property_name() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
        ],
        "discriminator": { "propertyName": 123 }
    })));
}

#[test]
fn guard_rejects_non_object_one_of_member() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "string" },
            { "type": "object", "properties": { "type": { "type": "string", "const": "A" } } }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_member_missing_discriminator_prop() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "object", "properties": { "value": { "type": "string" } } }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_prop_without_const_or_single_enum() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "object", "properties": { "type": { "type": "string" } } }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_multi_value_enum() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "object", "properties": { "type": { "type": "string", "enum": ["A", "B"] } } }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_member_without_properties() {
    assert!(!guard(json!({
        "oneOf": [{ "type": "object" }],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_unsupported_discriminator_type() {
    assert!(!guard(json!({
        "oneOf": [
            { "type": "object", "properties": { "type": { "type": "boolean", "const": true } } }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_null_and_undefined_one_of() {
    assert!(!guard(json!({
        "oneOf": null,
        "discriminator": { "propertyName": "type" }
    })));
    // Absent oneOf stands in for JS undefined.
    assert!(!guard(json!({
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_discriminator_not_in_required() {
    assert!(!guard(json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": "A" },
                    "value": { "type": "string" }
                },
                "required": ["value"]
            }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_no_required_array() {
    assert!(!guard(json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": "A" },
                    "value": { "type": "string" }
                }
            }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}

#[test]
fn guard_rejects_non_array_required() {
    assert!(!guard(json!({
        "oneOf": [
            {
                "type": "object",
                "properties": {
                    "type": { "type": "string", "const": "A" },
                    "value": { "type": "string" }
                },
                "required": true
            }
        ],
        "discriminator": { "propertyName": "type" }
    })));
}
