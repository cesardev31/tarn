# ADR 0055: static string literals

Status: accepted. Phase 27; [report](../phase-27-report.md).

## Context

Every evaluation of a string literal called `tarn_rt_string`, which
allocated a heap copy, and every destruction freed it. A literal inside a
loop or a request handler cost one `malloc` and one `free` per evaluation.
Phase 26 made literals easier to pass (`f("GET")`), which makes this cost
more common.

## Decision

- Each distinct literal is emitted once per program as a read-only image in
  the exact `TarnString` layout (`u64` length, then bytes), 8-byte aligned,
  in a dedicated `tarn_strings` section. Its value is the image's address.
- `tarn_rt_drop_string` returns without `free` when the pointer lies
  between the GNU linker's `__start_tarn_strings` and `__stop_tarn_strings`
  (weak, so programs without literals link). This is the only place that
  releases string storage.
- Ownership semantics are unchanged: a literal is still an owned, non-Copy
  `string` that moves, is stored and is destroyed exactly once by verified
  post-drop IR. Only storage release is skipped, and the destruction trace
  still records it. Strings are immutable, so a static image is never
  written. `clone`, concatenation and parsing still produce heap strings.
- The backend learns no ownership fact: the decision is made by address at
  run time, inside the runtime's single release function.

## Alternatives considered

- **A flag bit in the length or a new header field**: changes the layout
  that inline codegen reads (`string.len`, `is_empty`, `clone` load offset
  0) and every runtime routine; rejected for the larger blast radius.
- **Reference counting or a separate `&'static string` type**: new semantic
  machinery for a pure representation optimization; rejected.
- **Interning at run time**: still allocates and needs synchronization.

## Consequences

Measured with 10,000,000 evaluations of one literal: `malloc` calls from
10,000,001 to 1 and run time from 1.04 s to 0.94 s with the unoptimized
backend. The section-bounds check costs two comparisons per string drop.
Programs are linked by the system GNU toolchain on Linux, which provides the
bracket symbols; another linker would need equivalent bounds.
