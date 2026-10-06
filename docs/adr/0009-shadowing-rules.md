# ADR 0009 — Shadowing: forbidden in the same scope, allowed from outer scopes

Status: accepted (2026-10-05). Amends ADR 0006.

## Rules

1. **Same scope: error E2003.** Declaring a name that is already declared in
   the *same* block (including a parameter redeclared at the top level of the
   function body, which is the parameters' scope) is rejected:

   ```tarn
   x := 1
   x := 2          // error[E2003]: `x` is already declared in this scope
   ```

2. **Outer scope: allowed.** A nested block (`if`, `for`, `match` arm, `{ }`,
   closure body) may declare a name that exists in an enclosing scope. The
   inner binding hides the outer one until the inner block ends.

3. **Warning W2001 when it may cause confusion.** Draft-0 heuristic, chosen to
   be quiet for the common idioms and loud for the error-prone ones. The
   resolver warns when an inner binding shadows a local or parameter of the
   *same function* and either:
   - the outer binding is **used again after** the inner block ends (the
     reader may think the inner assignment changed it), or
   - the outer binding is **mutable** (`var`) — writes to the inner one look
     like writes to the outer one.

   Shadowing a module-level item or an import with a local is warning W2002
   (`print := ...` hiding the builtin is almost always a mistake).

4. **Amended by ADR 0013 (2026-10-05):** the names a construct introduces —
   parameters, closure parameters, a `for` variable, pattern bindings — live
   in the *same* scope as the top level of that construct's body. Redeclaring
   one of them at that top level is E2003; shadowing it inside a nested block
   follows rules 2–3. (Draft text said loop and match bindings were ordinary
   inner-scope declarations, which would have allowed `for i in r { i := 0 }`.)

5. **Noise measurement (2026-10-05):** on the current corpus (28 examples,
   parser and resolver pass suites) W2001 fires only in the 3 places written
   to trigger it, W2002 in 2. The corpus is small and written by the
   language authors; re-measure on the `xlinux doctor` port before making the
   heuristic final.

Warnings never block compilation; `tarn check --deny-warnings` (future) can
turn them into errors in CI.

## Why the change

The blanket ban forced renames in idiomatic code (`for line in lines` inside a
function with a `line` parameter, narrowing `value` inside a `match` arm).
Same-scope redeclaration keeps being an error because there it is almost
always a bug, and it is the case that makes "find the declaration" ambiguous
inside one block.
