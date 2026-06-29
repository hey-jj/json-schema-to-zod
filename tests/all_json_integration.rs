use json_schema_to_zod::{json_schema_to_zod, Options, ZodVersion};
use serde_json::Value;

/// Load all.json and add a root `type: "object"` so the walk descends into
/// every property. The raw fixture has no root type and emits `z.any()`.
fn load_wrapped() -> Value {
    let path = format!("{}/tests/fixtures/all.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("read all.json");
    let mut value: Value = serde_json::from_str(&text).expect("parse all.json");
    if let Value::Object(map) = &mut value {
        map.insert("type".to_string(), Value::String("object".to_string()));
    }
    value
}

/// Emit a bare expression with default options, the same fragment the walk
/// produces with no module wrapper.
fn emit(schema: &Value, version: ZodVersion) -> String {
    json_schema_to_zod(schema, Options::default().zod_version(version)).expect("emit succeeds")
}

#[test]
fn raw_root_has_no_type_so_emits_any() {
    let path = format!("{}/tests/fixtures/all.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("read all.json");
    let value: Value = serde_json::from_str(&text).expect("parse all.json");
    assert_eq!(emit(&value, ZodVersion::V4), "z.any()");
}

#[test]
fn all_json_v4_matches_golden() {
    let golden = include_str!("fixtures/all.v4.golden.txt");
    assert_eq!(emit(&load_wrapped(), ZodVersion::V4), golden);
}

#[test]
fn all_json_v3_matches_golden() {
    let golden = include_str!("fixtures/all.v3.golden.txt");
    assert_eq!(emit(&load_wrapped(), ZodVersion::V3), golden);
}
