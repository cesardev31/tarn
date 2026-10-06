# ADR 0037: source async lowering onto suspended execution

Status: proposed for Phase 13; syntax/type foundation implemented, executable
frame lowering unimplemented. This is not an async completion or safety claim.

## Source facts implemented at this checkpoint

An async modifier remains on the syntactic FnDecl; Await remains an expression
node. Resolution uses the ordinary item namespace and stable symbol IDs. FnSig
separately records the source modifier and declared output T. Calling that
signature produces an owned, non-Copy `async computation<T>` semantic type.
Await unwraps its output, including generic inference. It also accepts the
resolved trusted Phase-12C Operation<R> declaration, not a struct with that name.
Ordinary closures reset the enclosing async context. Prefix precedence makes
`try await f()` mean `try (await f())`.

E3062 deliberately prevents async declarations from reaching native codegen at
this checkpoint. No body is executed synchronously as recovery. E3060 and E3061
identify invalid context and invalid operands. The current compiler has not yet
implemented async construction, polling, networking wrappers or block_on.

## Intended lazy construction and explicit execution binding

The chosen direction is lazy execution: construction transfers parameters into
owned computation storage but executes no body statements. Creating or moving a
computation must preserve its captured input loans. A normal main with an explicit
block_on entry is preferred over introducing a second main convention.

Execution cannot be hidden in a global variable. Phase 12C allocates Wakers from
an explicit Execution owner, while a source computation can be created before
block_on. The lowering therefore needs a deliberate binding step: an owned lazy
factory is consumed under the selected Execution before its first poll. Nested
computations bind to that same owner and use existing poll_with forwarding.
The private representation and verified binding functions remain to be implemented.

## Stable frame storage and stored-local analysis

Parameters must survive lazy construction. Locals needed across suspension must
be selected using CFG liveness, including destruction and borrowed-storage uses.
All locals must not simply be placed in the frame. State IDs and internal type
identities must be deterministic and outside user name lookup.

A frame containing a buffer borrowed by a pending child must allocate stable
storage before creating that child. Packing each state's live values into a fresh
closure or enum payload can move the buffer while retaining its old address.
Such a continuation transformation is unsuitable for that case. The intended
frame allocation remains stable until verified destruction; moving its owned
handle must not move its payload. No general public Pin or raw self-pointer API
is proposed.

Ownership transfer through mutable frame access also requires deliberate lowering.
Ordinary move checking correctly rejects moving an owned value behind &mut.
That restriction must not be relaxed globally to make generated polling work.
The representation must expose owned transitions to the existing checker or
preserve its proven decisions through a separately verified physical frame pass.
The final pass placement is unresolved until an executable resource/borrow proof
is available; it must precede native codegen and remain outside Cranelift.

## Initialization, partial moves and destruction

Initialization and drop state must persist across polls. Function-entry flag
initialization cannot run again on every resume. A partial move before await must
not resurrect a consumed child field when polling resumes. Child construction,
call-result success edges, overwrite, return, try, break and continue must retain
ordinary move/init and drop decisions.

Every suspension state needs normal abandonment destruction, including the child
operation and its borrowed storage in the correct order. Generated destruction
must use existing abstract drops, move-path classifications and post-drop plans;
C must not infer initialized fields or application resource destruction.
These transitions and destruction functions are not implemented yet.

## Child results and provenance

Ready transfers the child result once. Finishing the child must release only
child-only loans. A result that borrows child storage prevents immediate finish;
the lowering must reject conservatively when the existing model cannot establish
an independent caller-storage source.

Source-body provenance must distinguish caller-owned inputs from locals that
become frame storage. A returned reference into an owned local must not become
valid merely because that local was moved into a heap frame. Existing E4203 for
user reference-bearing declarations remains unchanged. Any precision added for
verified generated results needs its own regression against a result that really
borrows the child environment; removing all holder loans is not valid.

## Intended API and limits

Networking will use distinct read_async/write_async/write_all_async/accept_async/
connect_async/recv_async spellings over Phase-12C operations. Blocking method
behavior must not change based on surrounding syntax. Known blocking calls and
DNS remain blocking if permitted; they stall this executor, with no hidden pool.
A deliberate rejection policy may be adopted before the source API is finalized.

Direct and mutual recursive async construction should be rejected in v0. Generic
specialization, ordinary owned closures and caller-backed dynamic references must
reuse their existing contracts. Async closures, async blocks, executor spawning,
channels, timers, cancellation, HTTP, TLS, multithread executors, io_uring, public
pinning, user destructors and optimization remain outside Phase 13.

## Validation required before acceptance

The source parser/type tests at this checkpoint do not prove executable async.
Acceptance requires the full source/native/drop/borrow/wake/mutation corpus in the
Phase-13 request, including stable borrowed buffers, partial moves, several awaits,
loops, nested forwarding, abandonment at every state and exactly-once results.
See [the checkpoint](../source-async-checkpoint.md) for verified current scope.
