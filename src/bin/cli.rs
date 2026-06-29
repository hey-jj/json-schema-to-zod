//! Command line wrapper around the library.
//!
//! Reads a JSON Schema from a file, an argument, or stdin and prints Zod source
//! code. The module defaults to ESM, and only `--zodVersion 3` selects v3.

use std::collections::BTreeMap;
use std::fs;
use std::io::{IsTerminal, Read};
use std::path::Path;
use std::process::exit;

use json_schema_to_zod::{json_schema_to_zod, Module, Options, TypeExport, ZodVersion};
use serde_json::Value;

/// A CLI parameter definition. The `name` lives on the param so the list keeps
/// its source order and stays the single place a parameter is declared.
struct Param {
    name: &'static str,
    shorthand: &'static str,
    value: ParamValue,
    required: Required,
    description: &'static str,
}

/// What kind of value a parameter expects.
enum ParamValue {
    Flag,
    Str,
    Number,
    Enum(&'static [&'static str]),
}

/// Whether a parameter is required, and the message to throw if it is missing.
enum Required {
    No,
    Message(&'static str),
}

/// A parsed argument value.
enum Parsed {
    Flag(bool),
    Str(String),
    Number(i64),
}

/// The parameter list in declaration order. Parsing and help both walk this in
/// order, so a parameter is declared once and order needs no second list.
fn params() -> Vec<Param> {
    let stdin_is_tty = atty_stdin();
    vec![
        Param {
            name: "input",
            shorthand: "i",
            value: ParamValue::Str,
            required: if stdin_is_tty {
                Required::Message("input is required when no JSON or file path is piped")
            } else {
                Required::No
            },
            description: "JSON or a source file path. Required if no data is piped.",
        },
        Param {
            name: "output",
            shorthand: "o",
            value: ParamValue::Str,
            required: Required::No,
            description: "A file path to write to. If not supplied stdout will be used.",
        },
        Param {
            name: "name",
            shorthand: "n",
            value: ParamValue::Str,
            required: Required::No,
            description: "The name of the schema in the output.",
        },
        Param {
            name: "depth",
            shorthand: "d",
            value: ParamValue::Number,
            required: Required::No,
            description:
                "Maximum depth of recursion before falling back to z.any(). Defaults to 0.",
        },
        Param {
            name: "module",
            shorthand: "m",
            value: ParamValue::Enum(&["esm", "cjs", "none"]),
            required: Required::No,
            description: "Module syntax; 'esm', 'cjs' or 'none'. Defaults to 'esm'.",
        },
        Param {
            name: "type",
            shorthand: "t",
            value: ParamValue::Str,
            required: Required::No,
            description: "The name of the (optional) inferred type export.",
        },
        Param {
            name: "noImport",
            shorthand: "ni",
            value: ParamValue::Flag,
            required: Required::No,
            description: "Removes the `import { z } from 'zod';` or equivalent from the output.",
        },
        Param {
            name: "withJsdocs",
            shorthand: "wj",
            value: ParamValue::Flag,
            required: Required::No,
            description: "Generate jsdocs off of the description property.",
        },
        Param {
            name: "zodVersion",
            shorthand: "zv",
            value: ParamValue::Number,
            required: Required::No,
            description: "Target Zod version: 3 or 4. Defaults to 4.",
        },
    ]
}

fn atty_stdin() -> bool {
    // Match `process.stdin.isTTY`. When stdin is a terminal there is no piped
    // input, so `input` becomes required.
    std::io::stdin().is_terminal()
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        exit(1);
    }
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().collect();
    let defs = params();

    if argv.iter().any(|a| a == "--help" || a == "-h") {
        print_help(&defs);
        exit(0);
    }

    let parsed = parse_args(&defs, &argv)?;

    let input = match parsed.get("input") {
        Some(Parsed::Str(s)) => s.clone(),
        _ => read_pipe(),
    };

    let json_schema = parse_or_read_json(&input)?;

    let zod_version = match parsed.get("zodVersion") {
        Some(Parsed::Number(3)) => ZodVersion::V3,
        _ => ZodVersion::V4,
    };

    let module = match parsed.get("module") {
        Some(Parsed::Str(s)) => match s.as_str() {
            "cjs" => Module::Cjs,
            "none" => Module::None,
            _ => Module::Esm,
        },
        _ => Module::Esm,
    };

    let mut options = Options::default()
        .module(module)
        .no_import(matches!(parsed.get("noImport"), Some(Parsed::Flag(true))))
        .with_jsdocs(matches!(parsed.get("withJsdocs"), Some(Parsed::Flag(true))))
        .zod_version(zod_version);
    if let Some(Parsed::Str(n)) = parsed.get("name") {
        options = options.name(n.clone());
    }
    if let Some(Parsed::Number(d)) = parsed.get("depth") {
        options = options.depth(*d);
    }
    if let Some(Parsed::Str(t)) = parsed.get("type") {
        options = options.type_export(TypeExport::Named(t.clone()));
    }

    let zod_schema = json_schema_to_zod(&json_schema, options).map_err(|e| e.to_string())?;

    if let Some(Parsed::Str(out)) = parsed.get("output") {
        if let Some(dir) = Path::new(out).parent() {
            if !dir.as_os_str().is_empty() {
                fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
        }
        fs::write(out, &zod_schema).map_err(|e| e.to_string())?;
    } else {
        println!("{zod_schema}");
    }

    Ok(())
}

fn parse_args(defs: &[Param], argv: &[String]) -> Result<BTreeMap<&'static str, Parsed>, String> {
    let mut result = BTreeMap::new();

    for param in defs {
        let name = param.name;
        let long = format!("--{name}");
        let short = format!("-{}", param.shorthand);
        let index = argv.iter().position(|a| *a == long || *a == short);

        let Some(index) = index else {
            match &param.required {
                Required::Message(msg) => return Err(msg.to_string()),
                Required::No => {
                    result.insert(name, Parsed::Flag(false));
                }
            }
            continue;
        };

        match &param.value {
            ParamValue::Flag => {
                result.insert(name, Parsed::Flag(true));
            }
            ParamValue::Str => {
                let val = argv
                    .get(index + 1)
                    .ok_or_else(|| format!("Expected a value for argument {name}"))?;
                result.insert(name, Parsed::Str(val.clone()));
            }
            ParamValue::Number => {
                let val = argv
                    .get(index + 1)
                    .ok_or_else(|| format!("Expected a value for argument {name}"))?;
                let parsed: f64 = parse_js_number(val)
                    .ok_or_else(|| format!("Value of argument {name} must be a valid number"))?;
                result.insert(name, Parsed::Number(parsed as i64));
            }
            ParamValue::Enum(allowed) => {
                let val = argv
                    .get(index + 1)
                    .ok_or_else(|| format!("Expected a value for argument {name}"))?;
                if !allowed.contains(&val.as_str()) {
                    return Err(format!(
                        "Value of argument {name} must be one of {}",
                        allowed.join(",")
                    ));
                }
                result.insert(name, Parsed::Str(val.clone()));
            }
        }
    }

    Ok(result)
}

/// Parse a value the way JS `Number()` does, then reject the `NaN` cases the
/// CLI treats as an error.
///
/// `Number("")` is 0. `Infinity`, `+Infinity`, and `-Infinity` parse to the
/// infinities. `0x`, `0o`, and `0b` prefixes parse the rest as an unsigned
/// integer in that base. Everything else goes through the decimal parser.
/// `nan`, `inf`, and any other non-numeric text return `None`, which the caller
/// reports as `Value of argument ... must be a valid number`.
fn parse_js_number(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }

    match trimmed {
        "Infinity" | "+Infinity" => return Some(f64::INFINITY),
        "-Infinity" => return Some(f64::NEG_INFINITY),
        _ => {}
    }

    if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        return u128::from_str_radix(rest, 16).ok().map(|n| n as f64);
    }
    if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        return u128::from_str_radix(rest, 8).ok().map(|n| n as f64);
    }
    if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        return u128::from_str_radix(rest, 2).ok().map(|n| n as f64);
    }

    // Rust's `f64` parser accepts `nan`, `inf`, and `infinity`, which JS
    // `Number()` rejects. The Infinity forms are handled above, so reject any
    // remaining input carrying a non-exponent letter.
    if trimmed
        .bytes()
        .any(|b| b.is_ascii_alphabetic() && b != b'e' && b != b'E')
    {
        return None;
    }

    let parsed = trimmed.parse::<f64>().ok()?;
    if parsed.is_nan() {
        None
    } else {
        Some(parsed)
    }
}

fn parse_or_read_json(json_or_path: &str) -> Result<Value, String> {
    let trimmed = json_or_path.trim();
    let source = if trimmed.len() < 255 && Path::new(trimmed).is_file() {
        fs::read_to_string(trimmed).map_err(|e| e.to_string())?
    } else {
        trimmed.to_string()
    };
    serde_json::from_str(&source).map_err(|_| "Unexpected end of JSON input".to_string())
}

fn read_pipe() -> String {
    let mut buf = String::new();
    let _ = std::io::stdin().read_to_string(&mut buf);
    buf
}

fn print_help(defs: &[Param]) {
    let longest = defs
        .iter()
        .map(|p| p.name.len())
        .chain(std::iter::once(5))
        .max()
        .unwrap_or(5);

    let header = format!("Name {}Short Description", " ".repeat(longest - 2));
    println!("{header}");

    for param in defs {
        let name = param.name;
        let short = format!(" -{}", param.shorthand);
        let description = format!("    {}", param.description);
        println!(
            "--{name}{}{short}{description}",
            " ".repeat(longest - name.len())
        );
    }
    // The synthetic help row.
    println!(
        "--help{} -h    Display this message :)",
        " ".repeat(longest - "help".len())
    );
}
