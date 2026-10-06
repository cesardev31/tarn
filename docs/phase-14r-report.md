# Phase 14R report: trusted stdlib module layers

Design: [ADR 0041](adr/0041-stdlib-module-layers.md). This is a mechanical
refactor: behavior, semantics, native ABI and ownership rules are unchanged.

## What was implemented

- `stdlib/net/net.tarn` was split into four modules, with no import cycles:

  | Module | Contents | Builds on |
  |---|---|---|
  | `io` | Error, ErrorKind, Waker, Progress, plus the private wake/readiness and async-lowering primitives | — |
  | `time` | Duration, Timer, `sleep` | io |
  | `net` | Sockets, Poll, `*_async`, buffered I/O | io |
  | `runtime` | Execution, Operation, Executor, AsyncTask, `block_on`, `*_operation` | io, time, net |

- **Compiler.**
  - One table of embedded trusted modules replaces `"core" | "net"`, which
    appeared in six crates.
  - `Decls` looks each declaration up in its own module.
  - Intrinsics are named `<layer>._<operation>`, resolved through
    `stdlib_intrinsic_operation`.
  - The borrow checker, post-drop verifier, lowering and backend no longer name
    `net`.
- **Directed private access.** A trusted module may use the private items of the
  layers it builds on, and only those.
- **User-facing paths.** `io.Error`, `runtime.Execution`, `runtime.Operation`,
  `time.Timer`, `runtime.read_operation`, and so on. Async programs implicitly
  load `runtime`.
- **ABI.** `_Raw.address` is now `[24]u8`, so `io` does not depend on `net`. The
  layout is identical, and the verifier checks both types.

## Tests

The full workspace suite passes. The only changes to existing tests are:

- `import` lines and module paths in sources;
- golden line numbers and paths, after blessing.

Not one diagnostic code or message changed (verified on the diff).

New tests:

- the fixture `E2006_stdlib_internal_is_private`;
- the unit test `private_stdlib_access_flows_only_down_the_layers`.

## Bugs found

- **The first module name, `task`, collided with 37 variables named `task`.**
  A local variable shadows a module (W2002), and then `task.Operation` resolved
  against the variable. The module was renamed to `runtime`.
- **The first visibility rule used a linear layer order.** It let `net` see
  `time`'s private items even though `net` does not build on `time`. The new
  unit test caught it, and the rule now follows the dependency graph.
- **The migration script added `import "runtime"` to a program** because it
  mistook the local variable in `task.join()` for the module. The resolver's
  unused-import warning caught it.

## Decisions I would defend

- **`io` as the lowest layer that owns `Error`.** Every layer returns it, and
  `io.Error` reads naturally in application signatures.
- **Downward-only privileged access** instead of making trusted modules friends
  of each other.
- **Keeping APIs and semantics identical.** The regression suite is the oracle.

## Decisions I still question

- The `help: mark it pub in io` text on E2006 for stdlib internals is
  technically correct, but it is not useful to users. A stdlib-specific note
  would be better.
- `runtime` builds on `net` only because Execution owns a `net.Poll`. If Poll
  moves down into `io` (an API decision), `runtime` would no longer depend on
  networking.

## Known limitations

- Poll stays public in `net`, and buffered I/O stays in `net`; both are
  documented in the ADR.
- A local variable named `io`, `time`, `net` or `runtime` still shadows the
  module.
- Old ADRs and phase reports keep their historical `net.*` paths on purpose.
