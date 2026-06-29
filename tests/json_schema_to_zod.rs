use json_schema_to_zod::{
    json_schema_to_zod, Module, Options, PathSegment, TypeExport, ZodVersion,
};
use serde_json::json;

fn esm() -> Options {
    Options::default().module(Module::Esm)
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
    let opts = Options::default().module(Module::Esm).no_import(true);
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "export default z.string()\n"
    );
}

#[test]
fn add_type_capitalized() {
    let opts = Options::default()
        .name("mySchema")
        .module(Module::Esm)
        .type_export(TypeExport::Flag);
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport const mySchema = z.string()\nexport type MySchema = z.infer<typeof mySchema>\n"
    );
}

#[test]
fn add_type_custom_name() {
    let opts = Options::default()
        .name("mySchema")
        .module(Module::Esm)
        .type_export(TypeExport::Named("MyType".into()));
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport const mySchema = z.string()\nexport type MyType = z.infer<typeof mySchema>\n"
    );
}

#[test]
fn throws_when_cjs_and_type() {
    let opts = Options::default()
        .name("hello")
        .module(Module::Cjs)
        .type_export(TypeExport::Flag);
    assert!(json_schema_to_zod(&json!({ "type": "string" }), opts).is_err());
}

#[test]
fn throws_when_type_but_no_name() {
    let opts = Options::default()
        .module(Module::Esm)
        .type_export(TypeExport::Flag);
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
    let opts = Options::default()
        .module(Module::Esm)
        .without_defaults(true);
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
    let opts = Options::default()
        .module(Module::Esm)
        .without_describes(true);
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
    let opts = Options::default().module(Module::Esm).with_jsdocs(true);
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
    let opts = Options::default().parser_override(Box::new(|schema, refs| {
        let path = refs.path();
        if path.len() == 2
            && path[0] == PathSegment::Key("allOf".into())
            && path[1] == PathSegment::Index(2)
            && schema.get("type").and_then(|t| t.as_str()) == Some("boolean")
            && schema.get("description").and_then(|d| d.as_str()) == Some("foo")
        {
            Some("myCustomZodSchema".to_string())
        } else {
            None
        }
    }));
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
    let opts = Options::default().module(Module::Cjs).name("someName");
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "const { z } = require(\"zod\")\n\nmodule.exports = { \"someName\": z.string() }\n"
    );
}

#[test]
fn cjs_without_name() {
    let opts = Options::default().module(Module::Cjs);
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap(),
        "const { z } = require(\"zod\")\n\nmodule.exports = z.string()\n"
    );
}

#[test]
fn name_only_no_module() {
    let opts = Options::default().name("someName");
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
    let opts = Options::default()
        .module(Module::Esm)
        .zod_version(ZodVersion::V3);
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "object" }), opts).unwrap(),
        "import { z } from \"zod\"\n\nexport default z.record(z.any())\n"
    );
}

#[test]
fn cjs_with_jsdocs_prepends_block_twice() {
    // With cjs, the jsdoc block prefixes both the require line and the
    // module.exports line.
    let opts = Options::default().module(Module::Cjs).with_jsdocs(true);
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "description": "Hello" }), opts).unwrap(),
        "/**Hello*/\nconst { z } = require(\"zod\")\n\n/**Hello*/\nmodule.exports = z.string().describe(\"Hello\")\n"
    );
}

#[test]
fn esm_with_jsdocs_prepends_block_once() {
    // With esm, the import line carries no jsdoc, so the block appears once.
    let opts = Options::default().module(Module::Esm).with_jsdocs(true);
    assert_eq!(
        json_schema_to_zod(&json!({ "type": "string", "description": "Hello" }), opts).unwrap(),
        "import { z } from \"zod\"\n\n/**Hello*/\nexport default z.string().describe(\"Hello\")\n"
    );
}
