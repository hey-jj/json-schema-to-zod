//! Option bag, reference state, and the target Zod version.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::Value;

/// Target Zod major version. The version changes `z.record` arity and the
/// `superRefine` error path and issue shape. Everything else is identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZodVersion {
    /// Zod 3 idioms: `z.record(value)` and `path: [...ctx.path, key]`.
    V3,
    /// Zod 4 idioms: `z.record(z.string(), value)` and `path: [key]`.
    V4,
}

impl Default for ZodVersion {
    fn default() -> Self {
        ZodVersion::V4
    }
}

/// How the output is wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Module {
    /// CommonJS: `const { z } = require("zod")` plus `module.exports = ...`.
    Cjs,
    /// ES module: `import { z } from "zod"` plus `export default ...`.
    Esm,
    /// No module wrapper. A bare expression or `const name = ...`.
    None,
}

/// A parser override hook. It receives the current schema node and the live
/// refs. Returning `Some(code)` short-circuits parsing and emits `code`
/// verbatim. Returning `None` falls through to normal parsing.
pub type ParserOverride = Box<dyn Fn(&Value, &Refs) -> Option<String>>;

/// Public options for [`json_schema_to_zod`](crate::json_schema_to_zod).
///
/// All fields are optional. `zod_version` defaults to [`ZodVersion::V4`].
#[derive(Default)]
pub struct Options {
    /// Name of the schema constant in the output.
    pub name: Option<String>,
    /// Module wrapping. `None` here means no wrapper (a bare expression).
    pub module: Option<Module>,
    /// Drop `.default(...)` modifiers.
    pub without_defaults: bool,
    /// Drop `.describe(...)` modifiers.
    pub without_describes: bool,
    /// Emit JSDoc blocks from `description` values.
    pub with_jsdocs: bool,
    /// Replace any node's emission with a custom string.
    pub parser_override: Option<ParserOverride>,
    /// Maximum re-expansion depth for recursive nodes before `z.any()`.
    pub depth: Option<i64>,
    /// Emit `export type <Name> = z.infer<typeof name>`. A string sets the
    /// type name. The flag capitalizes `name`. Requires `name` and ESM module.
    pub type_export: Option<TypeExport>,
    /// Drop the `import`/`require` line.
    pub no_import: bool,
    /// Target Zod version. Defaults to v4.
    pub zod_version: ZodVersion,
}

/// The `type` option: a flag that capitalizes `name`, or an explicit name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExport {
    /// Derive the type name by capitalizing the first code unit of `name`.
    Flag,
    /// Use this exact type name.
    Named(String),
}

/// Per-node bookkeeping in the `seen` map.
#[derive(Debug, Clone)]
pub struct Seen {
    /// Re-expansion counter for recursive nodes.
    pub n: i64,
    /// Memoized post-metadata emission, set once parsing completes.
    pub r: Option<String>,
}

/// One step in a [`Refs`] path. Either an object key or an array index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    /// An object property name or keyword.
    Key(String),
    /// An array index.
    Index(usize),
}

/// Live reference state threaded through every parser.
///
/// `path` is informational. It feeds the parser override and the v3
/// `superRefine` path. `seen` and `parser_override` are shared by reference so
/// cycle detection, memoization, and the override hook stay consistent across
/// the whole walk. Cloning a `Refs` shares those two and copies the scalars,
/// matching the JS spread `{ ...refs, path }`.
#[derive(Clone)]
pub struct Refs {
    /// Breadcrumb of the current node.
    pub path: Vec<PathSegment>,
    /// Identity map of visited nodes keyed by address, shared across frames.
    pub seen: Rc<RefCell<HashMap<usize, Seen>>>,
    /// Drop `.default(...)` modifiers.
    pub without_defaults: bool,
    /// Drop `.describe(...)` modifiers.
    pub without_describes: bool,
    /// Emit JSDoc blocks from `description` values.
    pub with_jsdocs: bool,
    /// Maximum re-expansion depth for recursive nodes.
    pub depth: Option<i64>,
    /// Target Zod version.
    pub zod_version: ZodVersion,
    /// Optional override hook, shared across frames.
    pub parser_override: Rc<Option<ParserOverride>>,
    /// Storage for schemas built during the walk. Interning a constructed
    /// schema here gives it a stable, never-reused address so cycle detection
    /// and memoization see distinct identities, the same way distinct JS
    /// objects do. Shared across frames.
    pub arena: Rc<RefCell<Vec<Box<Value>>>>,
}

impl Refs {
    /// Build refs from [`Options`], starting at an empty path with an empty
    /// seen map. The override hook moves out of `options`.
    pub fn from_options(options: &mut Options) -> Self {
        Refs {
            path: Vec::new(),
            seen: Rc::new(RefCell::new(HashMap::new())),
            without_defaults: options.without_defaults,
            without_describes: options.without_describes,
            with_jsdocs: options.with_jsdocs,
            depth: options.depth,
            zod_version: options.zod_version,
            parser_override: Rc::new(options.parser_override.take()),
            arena: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Default refs for a direct parser call: empty path, empty seen map, v4,
    /// no options set. Matches the upstream `{ seen: new Map(), path: [] }`
    /// default where `zodVersion` is absent and the v4 branch is taken.
    pub fn default_v4() -> Self {
        Refs {
            path: Vec::new(),
            seen: Rc::new(RefCell::new(HashMap::new())),
            without_defaults: false,
            without_describes: false,
            with_jsdocs: false,
            depth: None,
            zod_version: ZodVersion::V4,
            parser_override: Rc::new(None),
            arena: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Intern a constructed schema and return a reference with a stable address.
    ///
    /// The arena owns the box for the rest of the walk, so the address never
    /// moves and is never reused. That gives each constructed schema a distinct
    /// identity in the `seen` map, matching JS object identity.
    ///
    /// # Safety
    ///
    /// The returned reference borrows the boxed value. The box lives in the
    /// arena, which lives as long as any `Refs` clone, which outlives the walk
    /// that uses the reference. The arena only ever pushes, never removes or
    /// reorders, so the box address stays valid.
    #[allow(unsafe_code)]
    pub fn intern(&self, value: Value) -> &Value {
        let boxed = Box::new(value);
        let ptr: *const Value = &*boxed;
        self.arena.borrow_mut().push(boxed);
        // SAFETY: see the doc comment above. The pointee outlives this borrow.
        unsafe { &*ptr }
    }

    /// Clone this `Refs` with a new path. Shared state stays shared.
    pub fn with_path(&self, path: Vec<PathSegment>) -> Refs {
        let mut next = self.clone();
        next.path = path;
        next
    }

    /// Clone this `Refs` with a new path and `without_defaults` forced on.
    /// Used by the multiple-type parser so per-branch defaults are dropped.
    pub fn with_path_without_defaults(&self, path: Vec<PathSegment>) -> Refs {
        let mut next = self.clone();
        next.path = path;
        next.without_defaults = true;
        next
    }

    /// Append segments to the current path and return the extended vector.
    pub fn push_path(&self, segments: &[PathSegment]) -> Vec<PathSegment> {
        let mut p = self.path.clone();
        p.extend_from_slice(segments);
        p
    }
}
