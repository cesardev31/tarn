# ADR 0041: trusted stdlib module layers (Phase 14R)

Status: accepted. This is a mechanical refactor. Behavior, semantics, native
ABI and ownership rules are unchanged. Only module paths move.

## Problem

By Phase 14C, `stdlib/net/net.tarn` held sockets, readiness, the executor,
async tasks, async-lowering primitives, timers and buffered I/O. The compiler
also hardcoded `"core" | "net"` in the driver, resolver, type environment,
backend, post-drop verifier and borrow checker. Every new runtime concept was
therefore networking by accident: for example, the borrow checker named
`net._task_take`.

## Layers

| Module | Contents | Imports |
|---|---|---|
| `io` | Error, ErrorKind, Waker, Progress, plus the private native result `_Raw` and the readiness/wake/async-lowering primitives | — |
| `time` | Duration, Timer, `poll_timer`, `sleep` | io |
| `net` | Sockets, addresses, `resolve`, Poll/Token/Event/Interest, `poll_*` functions, `*_async` socket methods, BufferedReader/BufferedWriter | io |
| `runtime` | Execution, Operation, Executor, AsyncTask (`join`, `join_timeout`), `spawn_async`, `block_on`, manual `*_operation` constructors | io, time, net |

The graph has no cycles. The language allows import cycles; the stdlib does
not use them.

Placement follows from three rules:

- **Methods live in the module that declares their type.** `fn a.Foo.bar()`
  does not parse. So `join_timeout` sits with `AsyncTask` in `runtime`, which
  imports `time`.
- **`async` is a keyword**, so the execution module is `runtime`. It was first
  named `task`, but 37 existing variables were called `task` (`task :=
  owner.spawn_async(...)`, `Some(task) => task.join()`). A local shadows a
  module, which produced W2002 and then resolution errors. No variable is
  named `runtime`. The native `Task<R>` stays in `core`.
- **Error must be in the lowest layer**, because net, time and runtime all return
  it, so it is `io.Error`.

`_Raw.address` became `[24]u8` so that `io` does not depend on `net`. It has the
same layout as `net.SocketAddr`; `net` wraps it, and the ABI verifier checks
both types.

Placement decisions not taken here:

- **Poll** stays public in `net` for the Phase-12B manual readiness API. Its
  registration methods take socket types, and changing its visibility would be
  an API decision, not a refactor.
- **Buffered I/O** stays in `net` while it is TcpStream-specific. It moves to
  `io` once an async read/write interface exists.
- `core` is unchanged.

## Trusted module model

The driver has one table of embedded trusted modules: core, io, time, runtime,
net. Only these modules:

- may declare `extern "intrinsic"`;
- carry the trusted contracts;
- can never be shadowed by an entry file or a local file.

`Decls` finds each declaration in its own module. Native intrinsic names are
`<layer>._<operation>`, and the backend derives the operation from that name
through `stdlib_intrinsic_operation`. Each operation must exist exactly once.

## Private access is directed downward

A trusted module may use private items and fields only of the layers it builds
on: time → io, net → io, runtime → io, time, net. It never sees layers above or
beside it; for example, net cannot see time. User modules never see private
stdlib items. A first version used a linear rank, which let net see time's
privates; the unit test `private_stdlib_access_flows_only_down_the_layers`
caught it.

This is not "friend everything", because privilege only flows down the
dependency graph. `runtime` needs it for `net`'s `Poll.handle`, `stream.fd` and
`_require_nonblocking`. A future `internal` visibility keyword can replace
the layer table.

## User-visible change

Programs import the modules they name and use the new paths, for example:

- `io.Error`;
- `runtime.Execution`, `runtime.Operation`;
- `time.Timer`;
- `runtime.read_operation`.

Async programs implicitly load `runtime` (and through it io/time/net), as
they previously loaded `net`.

## Phase 15B extension

[ADR 0043](0043-blocking-filesystem.md) adds trusted `fs` → `io`. Its text
helpers import the ordinary `string` module. No sideways privilege is granted
to net/time/runtime; filesystem intrinsics/resources have a separate catalog.

## Phase 18B extension

`ffi` is a trusted layer with no dependencies (`ffi` builds on nothing and
nothing builds on it yet). Its public intrinsics are raw pointer operations;
readers are `unsafe fn` over libc. See [ADR 0047](0047-native-c-ffi.md).
