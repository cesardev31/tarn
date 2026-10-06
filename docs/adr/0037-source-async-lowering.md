# ADR 0037: source async lowering onto suspended execution

Status: accepted for Phase 13, within the single-computation-tree v0 limits
below. `async fn` and `await` are syntax and compiler lowering over the Phase-12C
Pending/Ready/Waker model (ADR 0036). They do not define a second async runtime.

## Source facts

An `async` modifier stays on the syntactic FnDecl; `await` stays an expression
node. Resolution uses the ordinary item namespace and stable symbol IDs. FnSig
records the modifier and the declared output `T`. Calling the function produces
an owned, non-Copy `async computation<T>`; `await` unwraps `T`, including
generic inference. `await` also accepts the trusted Phase-12C `Operation<R>`
declaration by identity, never a look-alike struct (E3061). Ordinary closures
reset the async context (E3060). Prefix precedence makes `try await f()` mean
`try (await f())`. Async blocks, async closures, select/join, yield, executor
spawning, cancellation, timers and channels are not part of Phase 13.

## Call semantics: lazy

A call executes no body statement. It lowers to `Aggregate::AsyncFrame(f, T..)`:
the arguments are moved into a newly allocated stable heap frame, the state word
is 0, and the value is an owned `(poll code, frame)` pair. Ownership phases see an
ordinary aggregate: arguments are moved, and the computation holds exactly the
loans its arguments held. Creating or moving the computation therefore keeps its
borrowed inputs unavailable until it is destroyed (E4104), and a computation that
borrows a local cannot escape that local's function (E4201).

## Await semantics and wake forwarding

`await child` lowers in the source CFG to an ordinary loop:

```text
poll:   r = child(&waker)           // or Operation.poll_with(&mut child, waker)
        switch discriminant(r)
Pending: drop r; Suspend { resume: poll, abandon: <scope drops>; Abandon }
Ready:   value = move r.Ready.0     // child is destroyed at statement end
```

The async body has one hidden `&Waker` parameter: the waker of the current poll.
Source computations are pollers invoked directly with the parent's waker, exactly
as `Operation.poll` invokes its poller; trusted manual Operations forward wakes
with the existing `poll_with` link. No new wake mechanism exists. Leaves arm
readiness on that same Waker; one Waker owns one registration (ADR 0036), so a
completed stdlib async leaf releases its registration like a completed Operation
(the private `_disarm` preserves an already queued wake). Sequential awaits in
one computation tree therefore never hold two registrations at once.

## Source form: ownership on ordinary places

Lowering keeps async bodies as ordinary IR functions whose locals are ordinary
places. The only additions are two terminators:

- `Suspend { resume, abandon }`: the computation returns Pending here. A later
  poll continues at `resume`; destroying the pending computation runs `abandon`.
- `Abandon`: end of an abandonment path, producing no result.

Every body begins with `Suspend` (the unstarted state). Abandonment blocks are
lowered exactly like an early `return`: drops of every pending temporary and
enclosing scope, in the same order. Move/initialization checking, borrow checking
(NLL liveness across the suspension edge) and drop elaboration therefore decide
everything with no async-specific ownership model: partial moves before a
suspension stay moved; loans live across a suspension stay live; each suspension
state receives its own verified destruction plan, including the pending child.

The single borrow-checker refinement: the result of polling a source computation
carries the loans the computation captured, not a loan of the computation value.
This is sound because the async body's own provenance already rejects returning
frame storage (every frame local, including moved-in parameters, dies at the
body's `return`). Manual pollers keep the conservative rule, because ADR 0036
allows their results to borrow poller storage.

## Physical frames

After the source form is verified (including flag initialization dataflow),
`async_frame::lower` makes the state machine explicit; it makes no ownership
decision:

```text
bb0: switch state  0 -> old entry (flag setup) -> abandon ? A0 : R0
                   k -> abandon ? Ak : Rk
                done -> abandon ? return : panic("polled after completion")
Suspend k -> state = k; _0 = Pending; return
Return    -> _0 = Ready(move result); state = done; return
Abandon   -> state = done; _0 = Pending; return
```

State IDs follow block order and are deterministic; loops reuse their states.
The function returns `Progress<T>`, receives `(frame, waker, abandon)`, and lists
the locals stored in the frame: construction parameters, the state word, every
local live on entry to a resume or abandonment edge (destruction is a use), and
every address-taken local (a reference held across a suspension may point into
it). The per-poll waker and abandon flag are never stored. All drop flags are
frame-resident, so initialization persists across polls and function-entry flag
setup runs once. A structural verifier checks dense dispatch, valid state writes,
placement and construction arity; it is part of post-drop verification.

The backend performs only mechanical work: it places listed locals and flags at
fixed frame offsets, allocates and fills frames at construction, and emits a poll
adapter (`mut fn(&Waker)` ABI) and a destruction adapter (abandon = true, then
free). Frame destruction is the body's own verified abandonment path; there is no
separate async destructor runtime and Cranelift makes no async decision.

Frame addresses are stable: the frame is allocated once and never moved; moving
the computation moves only its `(code, frame)` pair. References into stored
locals therefore remain valid across suspensions without public pinning.

## Child completion

The completed child is destroyed at the end of the awaiting statement, releasing
its child-only loans; a borrowed result keeps only the loans it carries. If the
child is a manual Operation whose result borrows its poller storage, the ordinary
borrow checker rejects that early destruction instead of extending storage.

## Executor entry and blocking policy

There is no hidden global executor and no `async main` sugar in v0. An async
computation coerces one-way to the trusted manual poller type
`mut fn(&Waker) Progress<T>` (same representation), so it can be wrapped in an
`Operation` and added to the existing `Executor` or driven explicitly:

```tarn
execution := try net.Execution.new()
var app = net.Operation.new(&execution, serve(&mut listener))
result := try execution.block_on(&mut app)
```

`Execution.block_on(&mut Operation<R>)` polls, waits for readiness when not
runnable, and returns the Ready value. The caller owns the Operation, so a
borrowed result is tied to caller storage through ordinary provenance, and the
Operation and its Waker are destroyed exactly once by the caller's scope.

Networking uses distinct names: `read_async`, `write_async`, `write_all_async`,
`accept_async`, `Connecting.finish_async`, `recv_from_async`. They are ordinary
`async fn` in the trusted `net` module over the Phase-12C poll functions, built on
two private primitives: `_with_waker(body)` (call `body` with the current poll's
waker) and `_async_park()` (a bare `Suspend`). Blocking methods keep their names
and behavior. Calling a blocking operation (including DNS resolution) from async
code is allowed and blocks the whole executor; there is no hidden thread pool.
Async I/O on a blocking socket returns an error before entering the syscall.

## Recursion

Direct and mutual async recursion is accepted. A child computation is a 16-byte
`(code, frame)` pair whose frame is a separate heap allocation, so no frame type
contains itself; this is the existing owned closure-environment indirection.
Expanding generic recursion is still bounded by the monomorphization limit, and
the polling depth equals the await chain depth (as ordinary recursion's stack).

## Diagnostics

Ownership diagnostics are ordinary source diagnostics on the async body or the
call site (E4001, E4104, E4201, ...). Generated locals and states never appear in
user messages. E3062 (checkpoint gate) is retired and never reused.

## Limits and future evidence

- One computation tree per Operation; no select/join, so one leaf registration.
- `block_on` takes a caller-owned Operation; an owned-result convenience would
  need a way to state that a generic result carries no frame loans.
- The stored set over-approximates (all address-taken locals, all flags).
- Async methods in interfaces/impls, async closures/blocks and cancellation are
  not supported. Await depth is native call depth during a poll.
