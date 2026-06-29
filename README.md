# json-schema-to-zod

Turn a JSON Schema value into Zod schema source code.

This is a pure text transform. It reads a JSON Schema and emits a string of
TypeScript or JavaScript that builds an equivalent Zod schema. It does not
validate data. The output references a `z` import you supply at runtime.

## Installation

```toml
[dependencies]
json-schema-to-zod = "0.1"
```

## Usage

```rust
use json_schema_to_zod::{json_schema_to_zod, Module, Options};
use serde_json::json;

let mut opts = Options::default();
opts.module = Some(Module::Esm);

let code = json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap();
assert_eq!(code, "import { z } from \"zod\"\n\nexport default z.string()\n");
```

Pass a bare expression by leaving `module` unset:

```rust
use json_schema_to_zod::{json_schema_to_zod, Options};
use serde_json::json;

let code = json_schema_to_zod(
    &json!({ "type": "object", "properties": { "id": { "type": "integer" } } }),
    Options::default(),
)
.unwrap();
assert_eq!(code, "z.object({ \"id\": z.number().int().optional() })");
```

## Options

`Options` controls the output:

- `name` sets the schema constant name.
- `module` wraps the output as `Esm`, `Cjs`, or `None`.
- `no_import` drops the import line.
- `type_export` adds `export type ... = z.infer<...>`. It needs a name and ESM.
- `with_jsdocs` renders JSDoc blocks from `description` values.
- `without_defaults` and `without_describes` drop those modifiers.
- `depth` bounds re-expansion of recursive nodes before falling back to
  `z.any()`.
- `parser_override` replaces any node's output with your own string.
- `zod_version` targets Zod v3 or v4. The default is v4.

## Supported keywords

The transform maps `type` (string or array), `properties`, `required` (array
or per-property boolean), `additionalProperties`, `patternProperties`, `items`
(schema or tuple), length and item bounds, string `format` and `pattern`,
number bounds and `multipleOf`, `const`, `enum`, `anyOf`, `allOf`, `oneOf`,
`not`, `if`/`then`/`else`, `description`, `default`, and `readOnly`. It also
handles the OpenAPI `nullable` flag and `discriminator.propertyName` for
discriminated unions.

JSON Schema and Zod do not overlap fully. Unmapped keywords are dropped.

## Zod versions

Output targets Zod v4 by default. Set `zod_version` to `ZodVersion::V3` for v3
idioms. The version changes only `z.record` arity and the `superRefine` error
path and issue shape.

## CLI

The crate ships a binary. It reads a schema from a file, an argument, or stdin
and prints the generated code.

```sh
json-schema-to-zod -i schema.json
echo '{"type":"string"}' | json-schema-to-zod -m cjs
```

Run `json-schema-to-zod -h` for the full flag list.

## License

Licensed under the [MIT license](LICENSE).
