//! Option bag, reference state, and the target Zod version.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::Value;

/// Target Zod major version. The version changes `z.record` arity and the
/// `superRefine` error path and issue shape. Everything else is identical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZodVersion {
    /// Zod 3 idioms: `z.record(value)` and `path: [...ctx.path, key]`.
    V3,
    /// Zod 4 idioms: `z.record(z.string(), value)` and `path: [key]`.
    #[default]
    V4,
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
/// All fields are optional. `zod_version` defaults to [`ZodVersion::V4`]. Start
/// from [`Options::default`] and chain the setters, or assign fields directly.
///
/// ```
/// use json_schema_to_zod::{Module, Options};
///
/// let opts = Options::default().module(Module::Esm).name("MySchema");
/// ```
///
/// The struct is `#[non_exhaustive]`, so adding an option later is not a
/// breaking change. Construct it through [`Options::default`] rather than a
/// struct literal.
#[derive(Default)]
#[non_exhaustive]
pub struct Options {
    /// Name of the schema constant in the output.
    pub name: Option<String>,
    /// Module wrapping. An absent value and [`Module::None`] both yield a bare
    /// expression with no `import`/`require` and no `export`.
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

impl std::fmt::Debug for Options {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Options")
            .field("name", &self.name)
            .field("module", &self.module)
            .field("without_defaults", &self.without_defaults)
            .field("without_describes", &self.without_describes)
            .field("with_jsdocs", &self.with_jsdocs)
            .field(
                "parser_override",
                &self.parser_override.as_ref().map(|_| "<fn>"),
            )
            .field("depth", &self.depth)
            .field("type_export", &self.type_export)
            .field("no_import", &self.no_import)
            .field("zod_version", &self.zod_version)
            .finish()
    }
}

impl Options {
    /// Set the schema constant name in the output.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the module wrapping.
    pub fn module(mut self, module: Module) -> Self {
        self.module = Some(module);
        self
    }

    /// Drop `.default(...)` modifiers.
    pub fn without_defaults(mut self, value: bool) -> Self {
        self.without_defaults = value;
        self
    }

    /// Drop `.describe(...)` modifiers.
    pub fn without_describes(mut self, value: bool) -> Self {
        self.without_describes = value;
        self
    }

    /// Emit JSDoc blocks from `description` values.
    pub fn with_jsdocs(mut self, value: bool) -> Self {
        self.with_jsdocs = value;
        self
    }

    /// Install a parser override hook.
    pub fn parser_override(mut self, hook: ParserOverride) -> Self {
        self.parser_override = Some(hook);
        self
    }

    /// Set the maximum re-expansion depth for recursive nodes.
    pub fn depth(mut self, depth: i64) -> Self {
        self.depth = Some(depth);
        self
    }

    /// Set the inferred type export.
    pub fn type_export(mut self, export: TypeExport) -> Self {
        self.type_export = Some(export);
        self
    }

    /// Drop the `import`/`require` line.
    pub fn no_import(mut self, value: bool) -> Self {
        self.no_import = value;
        self
    }

    /// Set the target Zod version.
    pub fn zod_version(mut self, version: ZodVersion) -> Self {
        self.zod_version = version;
        self
    }
}

/// An error from [`json_schema_to_zod`](crate::json_schema_to_zod).
///
/// The transform has one failure mode, so this is a single-variant enum. It
/// exists as a type rather than a string so callers can match on it and so it
/// implements [`std::error::Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A `type` export was requested without both a `name` and an ESM module.
    TypeExportRequiresNameAndEsm,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TypeExportRequiresNameAndEsm => {
                f.write_str("Option `type` requires `name` to be set and `module` to be `esm`")
            }
        }
    }
}

impl std::error::Error for Error {}

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
/// A parser override receives this by shared reference. Read the current
/// breadcrumb with [`Refs::path`]. The fields stay crate-private so outside
/// code cannot corrupt cycle detection or memoization.
///
/// `path` is informational. It feeds the parser override and the v3
/// `superRefine` path. `seen` and `parser_override` are shared by reference so
/// cycle detection, memoization, and the override hook stay consistent across
/// the whole walk. Cloning a `Refs` shares those two and copies the scalars,
/// matching the JS spread `{ ...refs, path }`.
#[derive(Clone)]
pub struct Refs {
    /// Breadcrumb of the current node.
    pub(crate) path: Vec<PathSegment>,
    /// Identity map of visited nodes keyed by address, shared across frames.
    pub(crate) seen: Rc<RefCell<HashMap<usize, Seen>>>,
    /// Drop `.default(...)` modifiers.
    pub(crate) without_defaults: bool,
    /// Drop `.describe(...)` modifiers.
    pub(crate) without_describes: bool,
    /// Emit JSDoc blocks from `description` values.
    pub(crate) with_jsdocs: bool,
    /// Maximum re-expansion depth for recursive nodes.
    pub(crate) depth: Option<i64>,
    /// Target Zod version.
    pub(crate) zod_version: ZodVersion,
    /// Optional override hook, shared across frames.
    pub(crate) parser_override: Rc<Option<ParserOverride>>,
    /// Storage for schemas built during the walk. Interning a constructed
    /// schema keeps its `Rc` alive here for the whole walk, so its address
    /// stays stable and is never reused. That gives each constructed schema a
    /// distinct identity in the `seen` map, the same way distinct JS objects
    /// do. Shared across frames.
    pub(crate) arena: Rc<RefCell<Vec<Rc<Value>>>>,
}

impl Refs {
    /// The breadcrumb of the node currently being parsed.
    ///
    /// A parser override reads this to branch on where the node sits in the
    /// schema tree.
    pub fn path(&self) -> &[PathSegment] {
        &self.path
    }

    /// Build refs from [`Options`], starting at an empty path with an empty
    /// seen map. The override hook moves out of `options`.
    pub(crate) fn from_options(options: &mut Options) -> Self {
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
    /// no options set. An absent version selects the v4 branch.
    pub(crate) fn default_v4() -> Self {
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

    /// Intern a constructed schema and return an owned handle to it.
    ///
    /// The arena keeps a clone of the returned `Rc` for the rest of the walk,
    /// so the value stays put and its address is never reused. Pass `&*handle`
    /// to the dispatcher to give the node a distinct identity in the `seen`
    /// map, matching JS object identity.
    pub(crate) fn intern(&self, value: Value) -> Rc<Value> {
        let node = Rc::new(value);
        self.arena.borrow_mut().push(Rc::clone(&node));
        node
    }

    /// Clone this `Refs` with a new path. Shared state stays shared.
    pub(crate) fn with_path(&self, path: Vec<PathSegment>) -> Refs {
        let mut next = self.clone();
        next.path = path;
        next
    }

    /// Clone this `Refs` with a new path and `without_defaults` forced on.
    /// Used by the multiple-type parser so per-branch defaults are dropped.
    pub(crate) fn with_path_without_defaults(&self, path: Vec<PathSegment>) -> Refs {
        let mut next = self.clone();
        next.path = path;
        next.without_defaults = true;
        next
    }

    /// Append segments to the current path and return the extended vector.
    pub(crate) fn push_path(&self, segments: &[PathSegment]) -> Vec<PathSegment> {
        let mut p = self.path.clone();
        p.extend_from_slice(segments);
        p
    }
}
