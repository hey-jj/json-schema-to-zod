use json_schema_to_zod::{
    json_schema_to_zod, Module, Options, PathSegment, TypeExport, ZodVersion,
};
use serde_json::json;

fn esm() -> Options {
    Options {
        module: Some(Module::Esm),
        ..Default::default()
    }
}

#[test]
fn accepts_schema_smoke() {
    assert!(
        !json_schema_to_zod(&json!({ "type": "string" }), Options::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn simple_esm() {
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), esm()).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.string()\n"
    );
}

#[test]
fn skip_import_line() {
    let opts = Options {
        module: Some(Module::Esm),
        no_import: true,
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "export default z.string()\n"
    );
}

#[test]
fn add_type_capitalized() {
    let opts = Options {
        name: Some("mySchema".into()),
        module: Some(Module::Esm),
        type_export: Some(TypeExport::Flag),
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport const mySchema = z.string()\nexport type MySchema = z.infer<typeof mySchema>\n"
    );
}

#[test]
fn add_type_custom_name() {
    let opts = Options {
        name: Some("mySchema".into()),
        module: Some(Module::Esm),
        type_export: Some(TypeExport::Named("MyType".into())),
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport const mySchema = z.string()\nexport type MyType = z.infer<typeof mySchema>\n"
    );
}

#[test]
fn throws_when_cjs_and_type() {
    let opts = Options {
        name: Some("hello".into()),
        module: Some(Module::Cjs),
        type_export: Some(TypeExport::Flag),
        ..Default::default()
    };
    assert!(json_schema_to_zod(&json!({ "type": "string" }), opts).is_err());
}

#[test]
fn throws_when_type_but_no_name() {
    let opts = Options {
        module: Some(Module::Esm),
        type_export: Some(TypeExport::Flag),
        ..Default::default()
    };
    assert!(json_schema_to_zod(&json!({ "type": "string" }), opts).is_err());
}

#[test]
fn includes_defaults() {
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "default": "foo" }), esm()).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.string().default(\"foo\")\n"
    );
}

#[test]
fn includes_falsy_default() {
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "default": "" }), esm()).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.string().default(\"\")\n"
    );
}

#[test]
fn includes_falsy_const() {
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "const": "" }), esm()).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.literal(\"\")\n"
    );
}

#[test]
fn can_exclude_defaults() {
    let opts = Options {
        module: Some(Module::Esm),
        without_defaults: true,
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "default": "foo" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.string()\n"
    );
}

#[test]
fn includes_describes() {
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "description": "foo" }), esm()).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.string().describe(\"foo\")\n"
    );
}

#[test]
fn can_exclude_describes() {
    let opts = Options {
        module: Some(Module::Esm),
        without_describes: true,
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "description": "foo" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.string()\n"
    );
}

#[test]
fn includes_jsdocs() {
    let schema = json!({
        "type": "object",
        "description": "Description for schema",
        "properties": {
            "prop": { "type": "string", "description": "Description for prop" },
            "obj": {
                "type": "object",
                "description": "Description for object that is multiline\nMore content\n\nAnd whitespace",
                "properties": {
                    "nestedProp": { "type": "string", "description": "Description for nestedProp" },
                    "nestedProp2": { "type": "string", "description": "Description for nestedProp2" }
                }
            }
        }
    });
    let opts = Options {
        module: Some(Module::Esm),
        with_jsdocs: true,
        ..Default::default()
    };
    let expected = "import { z } from \"zod\"\n\n/**Description for schema*/\nexport default z.object({ \n/**Description for prop*/\n\"prop\": z.string().describe(\"Description for prop\").optional(), \n/**\n* Description for object that is multiline\n* More content\n* \n* And whitespace\n*/\n\"obj\": z.object({ \n/**Description for nestedProp*/\n\"nestedProp\": z.string().describe(\"Description for nestedProp\").optional(), \n/**Description for nestedProp2*/\n\"nestedProp2\": z.string().describe(\"Description for nestedProp2\").optional() }).describe(\"Description for object that is multiline\\nMore content\\n\\nAnd whitespace\").optional() }).describe(\"Description for schema\")\n";
    assert_eq!(json_schema_to_zod(&schema, opts).unwrap(), expected);
}

#[test]
fn removes_optionality_when_default_present() {
    assert_eq!(
        json_schema_to_zod(
            &json!({
                "type": "object",
                "properties": { "prop": { "type": "string", "default": "def" } }
            }),
            esm()
        )
        .unwrap(),
        "import { z } from \"zod\"\n\nexport default z.object({ \"prop\": z.string().default(\"def\") })\n"
    );
}

#[test]
fn handles_falsy_default_boolean() {
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "boolean", "default": false }), esm()).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.boolean().default(false)\n"
    );
}

#[test]
fn ignores_undefined_default() {
    // serde_json has no undefined. Absent default stands in for it.
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "null" }), esm()).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.null()\n"
    );
}

#[test]
fn custom_parser_override() {
    let opts = Options {
        parser_override: Some(Box::new(|schema, refs| {
            if refs.path.len() == 2
                && refs.path[0] == PathSegment::Key("allOf".into())
                && refs.path[1] == PathSegment::Index(2)
                && schema.get("type").and_then(|t| t.as_str()) == Some("boolean")
                && schema.get("description").and_then(|d| d.as_str()) == Some("foo")
            {
                Some("myCustomZodSchema".to_string())
            } else {
                None
            }
        })),
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(
            &json!({
                "allOf": [
                    { "type": "string" },
                    { "type": "number" },
                    { "type": "boolean", "description": "foo" }
                ]
            }),
            opts
        )
        .unwrap(),
        "z.intersection(z.string(), z.intersection(z.number(), myCustomZodSchema))"
    );
}

#[test]
fn cjs_with_name() {
    let opts = Options {
        module: Some(Module::Cjs),
        name: Some("someName".into()),
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "const { z } = require(\"zod\")\n\nmodule.exports = { \"someName\": z.string() }\n"
    );
}

#[test]
fn cjs_without_name() {
    let opts = Options {
        module: Some(Module::Cjs),
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "const { z } = require(\"zod\")\n\nmodule.exports = z.string()\n"
    );
}

#[test]
fn name_only_no_module() {
    let opts = Options {
        name: Some("someName".into()),
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "const someName = z.string()"
    );
}

#[test]
fn boolean_schema_any() {
    assert_eq!(
        json_schema_to_zod(&json!(true), Options::default()).unwrap(),
        "z.any()"
    );
}

#[test]
fn v3_record_syntax() {
    let opts = Options {
        module: Some(Module::Esm),
        zod_version: ZodVersion::V3,
        ..Default::default()
    };
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "object" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.record(z.any())\n"
    );
}
