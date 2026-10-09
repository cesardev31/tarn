# Phase 31 report: port-driven general-purpose foundations

Design: [ADR 0060](adr/0060-general-purpose-foundations.md).

## What was implemented

- Nominal Hash and Eq implementations for integer/bool/string keys in core.
  Primitive implementations require trusted core provenance; user modules cannot
  gain this privilege by choosing a filename.
- Ordinary owned Map/Set in collections, direct borrowed lookup, replacement,
  removal with cluster repair, slot inspection and owned drains.
- Vec.swap, explicit modular integer arithmetic, comparator heapsort and first-
  match binary search. Ordinary arithmetic/conversions remain checked.
- Explicit reference dereference places for Copy reads, borrowing and assignment,
  using existing IR projections and ownership/drop checking. Pattern projection
  semantics remain unchanged; Option-wrapped reference results were removed from
  the inherited map prototype.
- Blocking non-owning console streams with partial/complete byte I/O, text output,
  flush and ordinary errors, including broken pipes.
- Monotonic Instant and Unix UTC Timestamp in time; durations retain millisecond
  precision. ASCII string.fields follows actual proc-style parsing needs.
- A partial status_device network counter/rate collector in
  [examples/status_metrics](../examples/status_metrics/README.md). It samples
  /proc/net/dev twice and writes sorted JSON with elapsed time and a timestamp.
- Corrected HTTP/process/string documentation and added module guides, diagnostic
  descriptions and an assessment follow-up including historical text performance.

## Tests

Validation results are recorded in the final validation section.

New coverage includes:

- Forced collisions across wrapped probe clusters, resizing, replacement,
  removal, reuse after drain, owned string values and custom nominal key types.
- Negative/minimum/maximum signed keys, Unicode string keys, bool keys,
  modular overflow, primitive dynamic Hash dispatch and owned deref assignment.
- Empty sorting/search, duplicate first-match search, misses outside/between
  values, and swapping non-Copy strings including equal indices.
- Collection result escape, owner move/mutation and mutable-alias rejection;
  temporary lookup keys do not determine result lifetime.
- Editor loading of ordinary bundled modules, trusted-core primitive coherence,
  and golden E3074/E3075 diagnostics.
- Binary pipes containing UTF-8 bytes, distinct stdout/stderr, EOF, EPIPE handling,
  signal-mask/disposition preservation and preservation of a pending SIGPIPE.
- Monotonic ordering, reversed-interval rejection, same-instant zero, wall-clock
  fraction bounds, and deterministic proc parsing/rates/order/reset fixtures.

## Bugs found

1. The inherited signed Hash implementations converted negative values directly
   to u64; Tarn's checked conversions aborted. Signed hashing now encodes bits
   without negating the minimum i64 or weakening normal casts.
2. Primitive interface implementations were wired into static specialization but
   dynamic tables still required ADTs. Both use the existing resolved primitive
   implementation targets; native primitive/string dynamic Hash tests cover it.
3. Primitive comparison bodies compare references. Backend comparison must read
   their referents instead of comparing reference addresses.
4. A parser recovery fixture used `* 3` as invalid syntax. Deref makes that syntax
   valid to parse, so the fixture now uses invalid prefix `/ 3` and continues to
   test recovery of multiple independent errors. Assignment diagnostic goldens
   now include reference places.
5. A for-loop over a temporary lines Vec with continue hit existing temporary
   cleanup restrictions. The port binds its owned lines before iterating; this
   phase does not redesign temporary lifetime/loop cleanup.

6. Parenthesized dereference assignment lost its assignment context and was
   checked as a forbidden non-Copy read. Parentheses now preserve that context;
   the native owned-string replacement fixture uses `(*loan) = "new"`.

7. Three resolution snapshots included either shifted core declaration spans or
   imports of the former placeholder `collections` module. They now record the
   real module and its declarations; changes were inspected and the suite is
   rerun without snapshot regeneration.

## Decisions I would defend

- Direct lookup loans preserve the existing aggregate-reference restriction.
  Presence checks plus aborting get match the established Vec.get policy, though
  fallible callback lookup may eventually offer better ergonomics.
- Collections remain Tarn code over verified Vec ownership. No runtime owner
  registry, new destructor hook, iterator lifetime system or hash backend magic.
- Heapsort gives predictable worst-case comparisons without requiring Copy,
  extra allocations or a new total-order interface.
- Console views do not claim ownership of process-global descriptors. The native
  adapter only controls this call's SIGPIPE and reports errno.
- Clock FFI is explicit about Linux x86_64 layout. No new native scheduler,
  cryptographic implementation or platform boundary is introduced.

## Decisions I still question

- Deterministic unkeyed hashing is not collision-flood hardened. Randomized hash
  policy and custom hashers require evidence and separate design.
- Contains plus get performs two lookups when absence is normal. The current
  reference-storage restriction makes Option<&V> inappropriate; alternative safe
  ergonomics deserve real application evidence.
- Slot-based iteration exposes table structure and is less convenient than a
  future sound traversal protocol. It should not become an iteration-order promise.
- Millisecond durations suffice for this port but not fine-grained profiling.
- Flush currently flushes all libc streams to interoperate with print; a broader
  I/O design should consider per-stream buffering and ordering contracts.

## Known limitations

This is not a full status_device port: the tray, GUI, CPU/disk/process collectors
and ticket-classifier HTTPS integration are absent. Network rows sort by name,
not by activity as in the Go UI. Rates use f64 scaling with upper-bound saturation.

No ordered/deque collections, stable sort, generic slice sorting, stored-reference
iterators, generic I/O interfaces, line reader, async console, float formatter,
calendar/time-zone library or HTTP/TLS client is added. Strings still allocate
owned split/field results. Phase 29's 2.6x strings result is historical, not a new
benchmark; borrowed substring design remains outstanding.

Hash misuse can degrade correctness/performance but must not be interpreted as
permission to bypass ownership. Existing allocation exhaustion and checked
capacity overflow retain abort semantics. Flush and global console views do not
serialize application records across threads.

## Final validation

- `cargo build --workspace` and `cargo check --workspace`: successful, no build warnings; CLI and LSP artifacts rebuilt.
- Final `cargo test -p tarn_backend --test native --no-fail-fast`: **23 passed, 0 failed** (407 s).
- Final foundation backend tests: **4 passed**; collection/editor/loan tests: **2 passed**; resolver unit tests: **4 passed**; final resolution suites: **3 passed**; type goldens: **2 passed**; parser tests: **36 passed**.
- Valgrind on the final collections fixture: **0 errors, 0 bytes/blocks in use at exit**, 17 allocations and 17 frees.
- Live read-only network collector: successful JSON output, 100 ms measured interval, name-sorted interfaces and a UTC Unix timestamp.
- Initial sandbox checks could not connect to the local HTTPS registry and the live timer/executor returned EPERM. The same HTTPS tests and live metrics run succeeded outside that restriction.

The full workspace run started before the last reference-place correction. Its
native fixture captured the old frontend and rejected parenthesized owned
replacement with E3074. The rebuilt final native suite above passes that fixture;
this earlier command is not described as a clean single full-workspace pass.
The full command completed with **275 passed, 2 failed, 1 ignored**. The second
failure was the resolution snapshot group described above, updated after
inspection. Final native and resolution suite reruns replace those two failed
groups; targeted tests also include the additional signal/core-provenance checks
added during review. Both final reruns passed; no unresolved test failure remains.
This is validation across the full workspace plus final corrected-target reruns,
not a claim that the earlier single command exited successfully.
