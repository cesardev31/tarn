# Source async implementation checkpoint (Phase 13)

Phase 12C is approved. Phase 13 is in progress, not complete.

## Implemented

- Reserved async and await tokens, a syntactic async function modifier and Await
  expression nodes; the parser does not erase suspension syntax.
- Prefix precedence, normal resolution and traversal of captured operands.
- Separate function declared output and call-result computation types, recursive
  generic inference/substitution and source-level display of computation types.
- Await context checks, ordinary closure context isolation and nominal trusted
  Operation identity checks.
- Automatic loading of execution declarations for source async functions.
- An explicit E3062 gate preventing accidental synchronous native lowering.

## Verified scope

Three new parser tests cover modifiers/generics, precedence/newlines and rejected
async blocks/closures/nested declarations. Three driver tests check async call
versus output types, generic inference, ordinary closure context, trusted manual
operations and rejection of a name-only Operation imitation. Parser and type
crate tests and workspace checking pass.

The initial baseline's executable/integration tests passed. Its doctest stage was
invalidated by overlapping rebuilds; a separate workspace doctest run passed.
Final workspace validation is still in progress at this checkpoint.

## Next implementation boundary

[ADR 0037](adr/0037-source-async-lowering.md) is proposed. Stable frame storage,
across-suspension liveness, persistent initialization/drop state, verified resume
functions, child completion/provenance, recursive rejection, explicit block_on,
async networking wrappers and native frame/resource traces remain unimplemented.
No async body currently executes. The existing executor, Waker, readiness runtime
and backend have not been redesigned. No future-phase APIs or dependencies were
added. This file must not be presented as a Phase-13 completion report.
