# Cross-thread capabilities and scoped tasks report (phase 11B)

## Transfer semantics

Transfer is compile-time permission to move a value across a native task boundary.
It implies neither Copy nor Share. Scalars, bool, strings, void and never qualify.
Arrays, structs and enum payloads qualify only when every substituted component
qualifies. Shared references require Share referents; mutable references require
Transfer referents. Provenance and completion must separately permit the move.
Task<R> requires Transfer for R; moving a handle preserves unique ownership.

Queries follow declaration vectors and substitute generic arguments. They reject
cycles and stop at depth 64 or 4096 nodes. Exhausted queries reject conservatively.
Unknown resources receive no optimistic permission.

## Share semantics

Share permits concurrent shared observation. Ordinary scalars, strings and ADTs
qualify structurally. Mutable references and Task<R> do not qualify. Neither byte
size nor Copy grants Share. Shared references require Share referents. Concurrent
mutation still requires ordinary exclusive loans; dynamic indexes remain
conservative. No split API or value-dependent disjointness proof was added.

## Generic capability bounds

Core declares Transfer and Share identities, consumed as semantic capabilities
rather than runtime interfaces. Existing T: Transfer and T: Share syntax and
ordinary obligations enforce bounds. Ordinary impls cannot grant capabilities.
Source-level diagnostics E3047 and E3048 distinguish missing evidence and invalid
impls. E3050 rejects dynamic objects of these semantic bounds. Numeric defaults
precede capability checking.

A Transfer parameter can contain references. Unscoped spawn of an unknown
reference-bearing parameter therefore remains rejected by existing loan checking.
Scoped generic workers can transfer such parameters while completing before their
loans end. Known callable arguments supply capture evidence for generic bounds;
erased signatures alone never establish capabilities.

## Callable capability derivation

Known callable values retain creation-site component evidence in the type checker.
Owned captures require the requested capability. Shared captures require Share;
mutable captures require Transfer and prevent Share. A Share callable must have
shared invocation. Nested known callables use their component evidence; reassignment
invalidates evidence conservatively. Evidence expansion is bounded to 4096 entries.
Named function items have no environment. Invocation and environment transfer
remain separate properties. Unknown returned callables and callable fields without
preserved capture evidence are conservatively rejected at task boundaries.

## Scoped task representation

Scoped spawn remains a direct zero-parameter closure literal inside lexical scope.
Typed tables distinguish it from unscoped spawn. Callee::TaskSpawn carries a scoped
bit and, for scoped calls, a second reference operand to dedicated
TaskScopeWitness storage. The witness never reaches the worker ABI; it expresses
scope membership in ordinary loan flow. The verifier checks its typed source.

Scope-owned Task values are the completion obligations. Discarded expressions
retain hidden owned Task temporaries through block cleanup. Handles can move
between local bindings and ordinary aggregates inside the scope. No second runtime
owner or task-specific move-checker state exists: each child still has exactly one
owned handle and the phase-11A runtime allocation. A scope witness prevents that
owner from escaping. Task destruction waits and frees the allocation as before.

Lowering emits task-containing resource destruction before other local storage
ends, including return, try, break, continue and nested cleanup. Scope completion
markers follow those executable destruction obligations. The original ordering
regression remains a gate. Abstract redundant drops become dead through existing
move checking and drop elaboration; the backend never discovers completion order.

## Loan retention and completion

Scoped spawn transfers capture and environment-storage loans into the Task holder.
Ordinary aggregate loan flow preserves them. Task destruction is a liveness use
because joining may access borrowed storage. Whole-handle moves transfer holdings;
join or destruction releases the consumed holder. Existing NLL conflict checks
therefore reject parent use, assignment and overlapping mutable workers before
completion, and permit access after explicit whole-handle join. Conditional joins
and loops use the existing forward union and backward liveness analyses.

Disjoint field loans remain disjoint; indexed loans remain conservative. Scope
witness escape uses E4208; ordinary overlapping accesses retain E4101/E4102/E4104
with a worker-completion note. No second concurrency borrow checker was added.

## Task result rules

Every worker result requires Transfer. Explicit join transfers its ownership;
implicit completion invokes verified result destruction before borrowed storage
ends. Worker panic still aborts the process. Scoped results containing references,
unknown generic reference containment or erased callables remain rejected with
E3049; join alone does not establish result provenance. Existing unscoped result
escape diagnostics remain intact.

## Native/dynamic capability contracts

Decls contains explicit NativeCapabilities evidence keyed by resolved type or
interface identity. Transfer and Share can be granted independently; absence
means neither. Trusted evidence overrides structural inspection for that declared
resource. Dynamic references require such interface evidence, never just method
sets or a concrete implementation. Contract behavior has independent unit tests.
No native resource API, source annotation syntax or dynamic capability inference
was invented. Existing extern/native contracts remain trusted boundaries.

## Tests

Native fixtures cover scalar/string/generic transfer, owned callable captures,
generic capability bounds, four shared readers, repeated join-shortened mutable
loans, shared owned callable invocation, disjoint mutable fields, nested workers
and completion on normal/return/try/break/continue exits. Stress uses joins rather
than sleeps. Exact traces require unused results to drop before each parent and
inner cleanup before outer cleanup. Three corrupted scoped metadata cases reject.

Golden diagnostics cover missing bounds, dynamic references, Task Share, invalid
capability impls, borrowed results, parent reads, overlapping workers and escaping
handles. Canonical memory_safety includes corresponding safe and unsafe paths.
Both mutation corpora include 11B fixtures. Validation: cargo test passes,
including all 18 backend tests and two pthread runtime tests; cargo check
--workspace and working/staged git diff --check pass without compiler warnings.
The host-dependent benchmark remains intentionally ignored. Existing callable, moves, borrows,
drops, native runtime and CLI tests remain gates.

## Bugs found

Discarded spawn temporaries originally completed at statement end, concealing
concurrent conflicts. Scoped task temporaries now survive to block completion.
Task destruction previously did not count as reference use in NLL; worker loans
could otherwise end before join. Consumed holder holdings must also be released,
or dead abstract drops would prevent explicit join from shortening a loan.
Ordinary aggregate containment must inspect declared fields to preserve task
loans through nongeneric wrappers. Closure bodies require their own scope context,
independent of the scope where the closure is created. Scoped metadata must be
merged into the final type tables before lowering.

## Decisions I would defend

Semantic capability identities; bounded declaration-ordered structural queries;
capture evidence rather than signature guesses; unique handle-owned completion
obligations with scope witness loans; explicit destruction before storage end;
ordinary NLL loan flow; unchanged pthread execution and callable ABI. Conservative
rejections preserve the established pipeline without new lifetime syntax.

## Decisions I still question

Scoped handles cannot be passed to ordinary functions or captured by other
callables; complete them locally first. This avoids hiding completion obligations
in erased environments. Whole-handle joins shorten loans precisely; projected
handles retain aggregate-level loan flow conservatively. Capture evidence is not
fully preserved through arbitrary callable-returning APIs or callable ADT fields.
Native contract source spelling and borrowed task results need future design.
Task-containing resources may be destroyed earlier within cleanup than unrelated
bindings, deliberately before borrowed storage. These choices favor correctness
and can be refined with application evidence.

## What 11C must solve

Explicit shared storage ownership, a minimal mutex owner/guard contract, ordinary
guard provenance and verified unlock destruction, and concrete sequentially
consistent atomics. Unknown native authority must remain explicit. No Mutex,
atomics, async, scheduler, pools, detach, cancellation or optimization was added.
ADR 0033 remains proposed. Synchronization requires separate approval. Trusted
native contracts and remaining bootstrap gaps still prevent claiming complete
end-to-end language memory safety.
