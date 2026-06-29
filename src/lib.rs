//! Generate Zod schema source code from JSON Schema.
//!
//! This crate is a pure text transform. It takes a JSON Schema value and emits
//! a string of TypeScript or JavaScript that builds an equivalent Zod schema.
//! It does not validate data. The output is source text that references a `z`
//! import you supply at runtime.
//!
//! The entry point is [`json_schema_to_zod`]. It walks the schema tree and
//! dispatches each node to a parser that returns a Zod source fragment. The
//! fragments concatenate into a `z.method(...)` chain.
//!
//! # Example
//!
//! ```
//! use json_schema_to_zod::{json_schema_to_zod, Module, Options};
//! use serde_json::json;
//!
//! let mut opts = Options::default();
//! opts.module = Some(Module::Esm);
//! let code = json_schema_to_zod(&json!({ "type": "string" }), opts).unwrap();
//! assert_eq!(code, "import { z } from \"zod\"\n\nexport default z.string()\n");
//! ```
//!
//! # Zod versions
//!
//! Output targets Zod v4 by default. Set [`Options::zod_version`] to
//! [`ZodVersion::V3`] for v3 idioms. The version only changes `z.record` arity
//! and the `superRefine` error path and issue shape.

#![deny(unsafe_code)]
#![warn(missing_docs)]

mod jsdocs;
mod parse_schema;
mod parsers;
mod predicates;
mod types;
mod util;

use serde_json::Value;

pub use jsdocs::{add_jsdocs, expand_jsdocs};
pub use parsers::{
    parse_all_of, parse_any_of, parse_array, parse_boolean, parse_const, parse_default, parse_enum,
    parse_if_then_else, parse_multiple_type, parse_not, parse_nullable, parse_number, parse_object,
    parse_one_of, parse_simple_discriminated_one_of, parse_string,
};
pub use predicates::is_simple_discriminated_one_of;
pub use types::{Module, Options, ParserOverride, PathSegment, Refs, Seen, TypeExport, ZodVersion};
pub use util::{half, json_stringify, json_stringify_str, omit, with_message, Builder};

/// Parse a schema node with the given refs.
///
/// Most callers want [`json_schema_to_zod`]. Use this to drive the dispatcher
/// directly or to inspect a fragment without module wrapping. Build refs with
/// [`Refs::default_v4`] for a fresh v4 walk.
pub fn parse_schema(schema: &Value, refs: &Refs) -> String {
    parse_schema::parse_schema(schema, refs, false)
}

/// Generate Zod source code from a JSON Schema value.
///
/// Returns the emitted code. Returns an error when the `type` export is
/// requested without both a `name` and an ESM module, which matches the only
/// validation the transform performs.
///
/// The output shape depends on [`Options::module`]:
/// - [`Module::Esm`] wraps with `import { z } from "zod"` and an
///   `export default` or `export const <name>`.
/// - [`Module::Cjs`] wraps with `const { z } = require("zod")` and
///   `module.exports`.
/// - [`Module::None`] or absent yields a bare expression, or `const <name> =`
///   when a name is set.
pub fn json_schema_to_zod(schema: &Value, mut options: Options) -> Result<String, String> {
    let module = options.module;
    let name = options.name.take();
    let type_export = options.type_export.take();
    let no_import = options.no_import;
    let with_jsdocs = options.with_jsdocs;

    if type_export.is_some() && (name.is_none() || module != Some(Module::Esm)) {
        return Err("Option `type` requires `name` to be set and `module` to be `esm`".to_string());
    }

    let refs = Refs::from_options(&mut options);
    let mut result = parse_schema::parse_schema(schema, &refs, false);

    let jsdocs = if with_jsdocs {
        schema
            .get("description")
            .and_then(|d| d.as_str())
            .filter(|d| !d.is_empty())
            .map(expand_jsdocs)
            .unwrap_or_default()
    } else {
        String::new()
    };

    match module {
        Some(Module::Cjs) => {
            let body = match &name {
                Some(n) => format!("{{ {}: {} }}", util::json_stringify_str(n), result),
                None => result.clone(),
            };
            result = format!("{jsdocs}module.exports = {body}\n");
            if !no_import {
                result = format!("{jsdocs}const {{ z }} = require(\"zod\")\n\n{result}");
            }
        }
        Some(Module::Esm) => {
            let head = match &name {
                Some(n) => format!("const {n} ="),
                None => "default".to_string(),
            };
            result = format!("{jsdocs}export {head} {result}\n");
            if !no_import {
                result = format!("import {{ z }} from \"zod\"\n\n{result}");
            }
        }
        _ => {
            if let Some(n) = &name {
                result = format!("{jsdocs}const {n} = {result}");
            }
        }
    }

    if let (Some(te), Some(n)) = (&type_export, &name) {
        let type_name = match te {
            TypeExport::Named(s) => s.clone(),
            TypeExport::Flag => capitalize_first(n),
        };
        result.push_str(&format!("export type {type_name} = z.infer<typeof {n}>\n"));
    }

    Ok(result)
}

/// Capitalize the first UTF-16 code unit of `name`, leaving the rest intact.
///
/// JS evaluates `name[0].toUpperCase() + name.substring(1)` on UTF-16 code
/// units. A lone surrogate upper-cases to itself. For ASCII and other BMP
/// names this upper-cases the first character and keeps the remainder.
fn capitalize_first(name: &str) -> String {
    let units: Vec<u16> = name.encode_utf16().collect();
    if units.is_empty() {
        return String::new();
    }

    let first_unit = units[0];
    let head = match char::from_u32(first_unit as u32) {
        Some(c) => c.to_uppercase().collect::<String>(),
        None => String::from_utf16_lossy(&[first_unit]),
    };

    let rest = String::from_utf16_lossy(&units[1..]);
    format!("{head}{rest}")
}
