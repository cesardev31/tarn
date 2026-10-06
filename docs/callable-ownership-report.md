# Callable and closure ownership completion report (phase 10)

## What was implemented

Callable types now carry shared, mutable or consuming invocation. `fn`, `mut fn`
and `once fn` spell those contracts; inferred locals require no annotations.
Shared/mutable calls lower to ordinary loans; consuming calls move the callable.
`move fn` infers and transfers captures without capture lists. Safe owned closures
can be returned, nested, passed as callbacks and stored in structs. Borrowed
closures remain on the stack. Capture-free closures use function items.

Owned environments contain canonical-layout fields and a private destruction
thunk pointer. The frontend generates ordinary destruction functions; their
abstract drops pass through move checking, borrow checking and post-drop
elaboration. Consuming bodies receive owned capture parameters, so existing
partial-move/drop rules decide which resources remain. Thunks release storage
only after those verified bodies return. The runtime adds allocation, free and
compiler-thunk dispatch; there is no GC, scheduler or user destructor hook.

Generic specialization substitutes environment types and reserves destruction
functions through its existing FIFO work list. The post-drop verifier checks
capture representation, storage witnesses and destruction-function metadata.
Backend code receives no moves, loans or provenance. ADR 0032 records the model.
The application ergonomics principle is included in AGENTS.md.

## Tests

Native fixtures cover repeated shared/mutable calls, named function items,
shared/mutable/consuming callback parameters, returned and nested move closures,
strings and clones, owned callable captures, generic owned ADTs, arrays, enums, partial field consumption,
conditional initialization, overwrite, capture reinitialization, never-invoked
closures, mixed references/owned values and zero-sized captures. Exact destruction
traces assert capture release and order. Five metadata corruptions are rejected.

Frontend pass/fail fixtures cover second consuming invocation (E4001), use after
ownership capture (E4001), escaped local references and nested stack environments
(E4205), shared/projected references to mutable callables and references to
consuming callables (E3044), replacing an environment kept alive by a returned
borrow (E4102), and overlapping mutable calls whose results remain live (E4101).
Both mutation corpora include closure fixtures; the native corpus verifies emitted
code. Existing 6A/6B/6C, memory_safety, parser/resolution/types, native/CLI and
resource-token interpreter tests remain regression gates.

Validation commands: `cargo test`, `cargo check --workspace` and
`git diff --check`. The host-dependent benchmark remains intentionally ignored.

## Bugs found

A borrowed closure returned from an owned reusable closure could hold valid
capture loans but still point to its creating function's stack environment. The
IR now includes an ordinary shared loan of compiler-local environment storage;
standard loan/result flow rejects the escape. No second borrow checker was added.

Unresolved integer literals were initially mistaken for non-Copy resources during
mode inference; numeric inference kinds now preserve Copy access. Copy fields of
owned aggregates and enum matches without moving payload bindings must also keep
shared invocation. Nested ownership captures account for capture transfer rather
than mutations private to the nested environment. Mutable-reference arguments
must require exclusive environment access even when the call reborrows them.
Zero-sized owned captures still need reference ABI lanes in reusable bodies.
Callable struct fields now resolve as value calls rather than missing methods;
shared access is checked across nested projections.

## Decisions I would defend

Separate invocation and ownership; ordinary loans for calls and environment
storage; inferred captures and explicit `move fn`; canonical layout; generated
post-drop destruction functions; whole owned capture parameters for consuming
bodies; minimal private heap runtime; conservative escaping-reference rejection.
These choices preserve the established safety pipeline and keep application
callbacks concise.

## Decisions I still question

All capturing owned environments allocate, even if they do not escape. Mode
annotations are invariant rather than capability-subtyped. Capture inference
retains whole bindings. A move followed by reinitialization conservatively remains
consuming. Application examples should guide future refinements; allocation,
thunk and capture-layout optimizations remain deferred.

## Known limitations

Borrowed capturing closures cannot return their stack environments, even if their
captured data belongs to the caller; use an owned environment containing those
references. Unbounded generic capture moves remain consuming even for a later Copy
specialization. Indirect callable results that may hold references conservatively retain call
loans; more precise callable result contracts remain future work. Environments
are limited to 64 KiB. No concurrency/spawn, async,
GC, custom destructors, unwinding, optimizer, LLVM or package manager was added.
Opaque/unmodeled APIs and trusted native contracts still prevent a complete
end-to-end language memory-safety claim.
