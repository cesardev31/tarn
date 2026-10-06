# ADR 0032: callable invocation and closure ownership

Status: accepted for phase 10; supersedes one-shot callable locals
in ADRs 0028 and 0030. No concurrency, optimizer or public ABI stabilization.

## Invocation modes

A callable signature carries a semantic invocation mode: Shared, Mutable or Once.
Shared invocation temporarily borrows the callable. Mutable invocation exclusively
borrows it. Once invocation moves it. Ownership of a callable value remains
non-Copy, independently of how it is invoked. Moving/passing a callable transfers
its environment; repeated invocation does not imply duplicating the environment.
Named direct functions are shared reusable items and keep ordinary direct calls.

Ordinary callable type syntax `fn(args) result` denotes shared invocation.
Mutable and consuming callback contracts need explicit type spellings (`mut fn`
and `once fn`); common inferred locals do not require annotations. A mutable call
can exclusively borrow its stored callable without requiring replacement of the
binding; it must still reject calls through a shared reference and conflicting
loans. These implicit call borrows use the existing loan/NLL machinery.

## Capture syntax and requirements

`fn(...) { ... }` retains inferred shared/mutable borrow captures. `move fn(...)`
transfers captured values into a unique owned environment. A transferred reference
still carries its loan: move syntax does not turn borrowed data into owned data.
No explicit capture list or escaping-borrow exemption is introduced. Alternatives
such as a new closure literal delimiter or mandatory capture lists add syntax and
burden without helping current callback code.

Invocation mode is decided by typed capture use: shared reads need Shared,
mutation needs Mutable, and moving a non-Copy capture needs Once. A closure that
consumes captures and later reinitializes them may conservatively remain Once;
proving that reusable across all normal exits is a future semantic refinement,
not permission to read holes. Ordinary borrowing closures cannot move resources
out of borrowed storage; explicit move syntax supplies ownership when needed.
Captures of Copy values still transfer their values in move closures.

## Storage and destruction

Borrowed nonescaping environments remain stack allocated. Owned environments use
unique heap storage initially, including nonescaping owned closures: this avoids a
second escape analysis or relocating environments on return. Thus not every
closure allocates; borrowed closures and function items keep the lightweight path.
Stack allocation of proven nonescaping owned environments is explicitly deferred.
Capture-free closures use function items with a null environment, including
capture-free move closures. For a borrowed environment the typed IR carries an
explicit shared loan of a compiler-local storage witness. This loan flows through
ordinary aggregate/call/result provenance and prevents the environment itself from
escaping its stack frame, even if all captured references point into valid caller
memory. It is not a second escape/borrow analysis. Without this storage loan, a
borrowed closure returned from a reusable owned closure could retain valid capture
loans while its environment pointer still dangled.

Environment fields use the one canonical layout layer and declaration/capture
order. There is no reference count, GC, scheduler or managed-object subsystem.

The callable remains a private code/environment pair. Environment metadata must
provide executable destruction for type-erased owned values, and the post-drop
boundary must describe the owned captures to destroy. A null function-item
environment and a borrowed environment own no captures. Dropping an owned callable
runs its explicit environment plan then frees the allocation exactly once.

The environment begins with an 8-byte private destruction-thunk pointer; borrowed
environments use zero. Captures follow at their canonical alignments. Lowering
generates a dedicated ordinary destruction function with owned capture parameters,
abstract drops in capture order, and a void return. It passes through 6A, 6B and
post-drop elaboration. `FnKind::Closure` records the environment types, ownership,
consuming access and destruction-function ID; post-drop verification checks their
agreement. The destructor thunk transfers fields into that verified function and
frees the allocation after normal return. It receives only the environment pointer,
so no new user destructor API or independently inferred backend drop plan exists.

Private runtime helpers are `tarn_rt_env_alloc(size)`, `tarn_rt_env_free(pointer)`
and `tarn_rt_env_drop(pointer)` (dispatch the stored compiler-generated thunk).
Allocation failure aborts. Environments are limited to 64 KiB, like native values.
Generic specialization substitutes environment types and reserves destruction
functions using the same FIFO instance mechanism as other reachable functions.

Reusable owned bodies access captures through shared/mutable references to their
fields. These fields remain initialized on normal exits because moving a resource
capture selects Once. Once bodies receive ownership of all owned captures through
ordinary capture parameters; existing move/init/drop elaboration destroys or
transfers their remaining resources. The consumed allocation can then be freed
without dropping its transferred fields again. Panic aborts, without cleanup.

## Escape and provenance

An owned capture appears as an ordinary Move into environment construction;
borrow captures create ordinary loans. Escaping values are checked by the existing
result-provenance flow. An owned environment with no dangling captured loans can
escape. A moved reference to local storage still cannot escape. Borrowed local
closures retain E4205. Results borrowing reusable environment storage keep the
callable alive through the call loan; a result cannot outlive its owning callable.
No closure-specific second borrow checker is introduced.

## Backend boundary and tests

Capture ownership, invocation mode and environment destruction must be explicit
before backend execution. Backend specialization substitutes concrete capture
types; codegen handles layout, storage and already-decided operations only.
Minimal environment allocation/free helpers extend the abort-only runtime.
The backend must never query loans or move results to decide whether a capture
is owned, reusable, consumed or destroyed.

Required regression coverage includes repeated shared/mutable calls; reusable and
consuming owned captures; second Once use rejection; generic owned captures;
returned/nested closures; passed callbacks; moved references with valid/invalid
lifetimes; partial consumption and resource traces; stored values dropped without
being called; conditional/overwrite destruction; native source mutations and
corrupted environment metadata. Existing phase 6A–9 safety gates remain required.

Tarn is high-level by default and low-level when needed. Low-level control is a
capability, not a tax imposed on ordinary application code. Callback syntax should
express application work; environment layout/allocator machinery stays internal.

## Remaining tradeoffs

Callable-mode annotations are invariant: a shared callable does not implicitly
coerce to a mutable/consuming callable type. Generic Copy bounds are honored;
unbounded generic capture moves are consuming even for a later Copy instance.
Captures remain whole bindings, not a new field-capture inference system. A body
that moves and reinitializes a capture remains consuming conservatively. These
choices preserve one ownership pipeline and can be refined with application
examples. Heap allocation of nonescaping owned environments, small environments
in the callable pair, thunk reuse and allocator reuse remain deferred optimizations.
