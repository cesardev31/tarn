//! The prelude scope and the list of standard-library modules.
//!
//! Provisional: these move to real `core` declarations once the stdlib exists.

use crate::{Resolved, ScopeId, Symbol, SymbolKind};

pub const PRIMITIVES: &[&str] = &[
    "bool", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64", "isize", "usize", "f32", "f64", "string", "void",
    "never",
];

pub const BUILTINS: &[&str] = &["print", "panic", "channel"];

pub const PRELUDE_TYPES: &[&str] = &["Option", "Result", "Error", "Channel", "Sender"];

/// (variant, parent type)
pub const PRELUDE_VARIANTS: &[(&str, &str)] = &[("Some", "Option"), ("None", "Option"), ("Ok", "Result"), ("Err", "Result")];

/// Standard-library modules that `import` may name (spec phase 16 list).
/// Their members are not checked yet.
pub const STD_MODULES: &[&str] = &[
    "core", "string", "collections", "fs", "path", "process", "io", "time", "json", "http", "net", "tls", "testing",
    "logging", "crypto", "encoding", "compression", "cli", "os",
];

pub fn build(r: &mut Resolved) -> ScopeId {
    let scope = r.add_scope(crate::ScopeKind::Prelude, None, None);
    let mk = |name: &str, kind: SymbolKind| Symbol {
        name: name.to_string(),
        kind,
        module: None,
        def: None,
        span: None,
        scope,
        is_pub: true,
        body: None,
    };
    for p in PRIMITIVES {
        r.declare(scope, mk(p, SymbolKind::Primitive));
    }
    for b in BUILTINS {
        r.declare(scope, mk(b, SymbolKind::Builtin));
    }
    for t in PRELUDE_TYPES {
        r.declare(scope, mk(t, SymbolKind::PreludeType));
    }
    for (v, parent) in PRELUDE_VARIANTS {
        let parent = r.scope(scope).get(parent).unwrap();
        let (id, _) = r.declare(scope, mk(v, SymbolKind::Variant { parent }));
        r.members.entry(parent).or_default().push(id);
    }
    scope
}
