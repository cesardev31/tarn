# ADR 0013 — Name resolution model

Status: accepted (2026-10-05).

## Decisions

1. **Side tables, not AST annotations.** The resolver returns `Resolved`:
   symbols (`SymbolId`), a scope tree (`ScopeId`, each with parent and
   symbols), and per-module tables keyed by `NodeId`: `defs` (node →
   introduced symbol), `uses` (node → `Res` + span), `captures`
   (closure → captured locals). The AST stays semantically neutral.
2. **One namespace per scope.** Types, functions, modules and values share a
   namespace: `fn run` and `struct run` in one module is E2002.
3. **Items are order-independent; locals are not.** Module-level names are
   visible everywhere in the module (and through `mod.name` if `pub`).
   A local is visible from the statement after its declaration; an earlier
   use is E2004 (not E2001), pointing at the later declaration.
4. **Construct bindings share the scope of the body's top level.** Parameters
   + function body, closure parameters + closure body, `for` variable + loop
   body, pattern bindings + guard + arm body. Redeclaring them at that top
   level is E2003; shadowing them in a nested block is allowed.
5. **Methods are members, not scope entries.** `fn T.m` adds `m` to the
   member table of `T`, which must be a struct or enum of the same module
   (E2011). Members are reached only through paths (`T.m`, `Enum.Variant`)
   or values (type checker). Variants and inherent methods share the member
   namespace of their type (E2002 on clash).
6. **Static paths are resolved here; value member access is not.** `a.b` is
   resolved when `a` is a module, struct, enum or std module; when `a` is a
   value, the resolver records nothing for `.b` (needs types).
7. **Standard modules are opaque** until the stdlib exists: `fs.x` resolves
   to `Res::External { module: "fs", path: ["x"] }` without checking.
8. **Imports**: name = last path segment, private to the importing module
   (not re-exported, E2006), cycles allowed (items are hoisted), unused
   imports warn (W2003).
9. **Name resolution only runs on syntactically valid programs** (driver).

## Alternatives considered

- *Resolved names stored in the AST* (`Option<SymbolId>` fields): simpler
  lookups, but the parser output becomes phase-dependent, incremental reuse of
  parsed modules gets harder, and tools must know which phase filled what.
  Rejected.
- *Separate type and value namespaces* (Rust): lets `struct P` and `fn P`
  coexist (useful for tuple-struct constructors). Tarn has no tuple structs,
  and two meanings for one name in one module hurts readers and agents.
  Rejected; revisit only if a construct needs it.
- *Hoisting locals* (JavaScript `var`): rejected; order matters for moves.
- *Bindings in a scope enclosing the body* (`for x` in its own scope, body
  nested): allows `x := ...` in the body as a shadow. Rejected: redeclaring a
  loop variable at the top of its own body is almost always a mistake, and
  this rule is the same one already chosen for function parameters.

## Consequences for later phases

- The type checker resolves: value member access, method calls on values,
  `Res::ScrutineeVariant` (ADR 0014), struct literal field names, interface
  dispatch, std members once the stdlib exists.
- Ownership can use `captures` directly to decide what each closure borrows
  or moves, and `SymbolKind::Local { mutable }` for assignment checks.
- LSP/agent tools get go-to-definition and references from `uses` (each has
  a span) without re-running the resolver logic.

## Risks

- Running resolution only on parse-clean files means a single syntax error
  hides all name errors. Fine for the CLI; the LSP will want resolution on
  recovered trees (the AST already has `Error` nodes; the resolver skips them).
- Module-level shadowing warning (W2002) may be noisy for parameters named
  like modules (`path`, `json`). Re-measure on the xlinux port.
