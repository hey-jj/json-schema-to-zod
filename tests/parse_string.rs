use json_schema_to_zod::parse_string;
use serde_json::json;

#[test]
fn date_time_with_message() {
    assert_eq!(
        parse_string(&json!({
            "type": "string",
            "format": "date-time",
            "errorMessage": { "format": "hello" }
        })),
        r#"z.string().datetime({ offset: true, message: "hello" })"#
    );
}

#[test]
fn email() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "email" })),
        "z.string().email()"
    );
}

#[test]
fn ip_and_ipv6() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "ip" })),
        "z.string().ip()"
    );
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "ipv6" })),
        r#"z.string().ip({ version: "v6" })"#
    );
}

#[test]
fn uri() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "uri" })),
        "z.string().url()"
    );
}

#[test]
fn uuid() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "uuid" })),
        "z.string().uuid()"
    );
}

#[test]
fn time() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "time" })),
        "z.string().time()"
    );
}

#[test]
fn date() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "date" })),
        "z.string().date()"
    );
}

#[test]
fn duration() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "duration" })),
        "z.string().duration()"
    );
}

#[test]
fn base64_variants() {
    assert_eq!(
        parse_string(&json!({ "type": "string", "contentEncoding": "base64" })),
        "z.string().base64()"
    );
    assert_eq!(
        parse_string(&json!({
            "type": "string",
            "contentEncoding": "base64",
            "errorMessage": { "contentEncoding": "x" }
        })),
        r#"z.string().base64("x")"#
    );
    assert_eq!(
        parse_string(&json!({ "type": "string", "format": "binary" })),
        "z.string().base64()"
    );
    assert_eq!(
        parse_string(&json!({
            "type": "string",
            "format": "binary",
            "errorMessage": { "format": "x" }
        })),
        r#"z.string().base64("x")"#
    );
}

#[test]
fn stringified_json() {
    let schema = json!({
        "type": "string",
        "contentMediaType": "application/json",
        "contentSchema": {
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name", "age"]
        }
    });
    assert_eq!(
        parse_string(&schema),
        r#"z.string().transform((str, ctx) => { try { return JSON.parse(str); } catch (err) { ctx.addIssue({ code: "custom", message: "Invalid JSON" }); }}).pipe(z.object({ "name": z.string(), "age": z.number().int() }))"#
    );
}

#[test]
fn stringified_json_with_messages() {
    let schema = json!({
        "type": "string",
        "contentMediaType": "application/json",
        "contentSchema": {
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name", "age"]
        },
        "errorMessage": { "contentMediaType": "x", "contentSchema": "y" }
    });
    assert_eq!(
        parse_string(&schema),
        r#"z.string().transform((str, ctx) => { try { return JSON.parse(str); } catch (err) { ctx.addIssue({ code: "custom", message: "Invalid JSON" }); }}, "x").pipe(z.object({ "name": z.string(), "age": z.number().int() }), "y")"#
    );
}

#[test]
fn combined_format_pattern_lengths_with_messages() {
    assert_eq!(
        parse_string(&json!({
            "type": "string",
            "format": "ipv4",
            "pattern": "x",
            "minLength": 1,
            "maxLength": 2,
            "errorMessage": {
                "format": "ayy",
                "pattern": "lmao",
                "minLength": "deez",
                "maxLength": "nuts"
            }
        })),
        r#"z.string().ip({ version: "v4", message: "ayy" }).regex(new RegExp("x"), "lmao").min(1, "deez").max(2, "nuts")"#
    );
}
