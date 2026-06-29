use json_schema_to_zod::omit;
use serde_json::json;

#[test]
fn omit_removes_key_and_keeps_rest() {
    let input = json!({ "a": true, "b": true });
    let output = omit(&input, "b");
    assert_eq!(output.get("a"), Some(&json!(true)));
    assert_eq!(output.get("b"), None);
}
