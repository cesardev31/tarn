# ADR 0060: General-purpose foundations driven by a metrics port

Status: accepted for Phase 31 implementation (2026-10-08).

## Evidence

`status_device/internal/metrics/system.go` uses string-keyed counter maps,
sets for duplicate suppression, sorting and elapsed-time measurements. Its
network counter collector can be ported independently of its GUI. Console
I/O also makes the collector and ordinary pipeline tools usable. The whole
tray application and the HTTP-dependent ticket classifier are outside this
phase. Phase 29 reports a strings benchmark at 2.6 times Go's time; that is
historical workload-specific evidence, not a new measurement or a claim about
all text operations. Borrowed substring support remains separate design work.

## Decisions

- Declare nominal `Hash` in core, consistent with Eq: equal keys must hash
  equally. Core supplies integer, bool and string implementations. Float keys
  are excluded. Hash values and map iteration order are not stable contracts.
- Permit primitive interface implementations only in the built-in core module,
  implementing ADR 0016's stated direction; ordinary user modules retain E2024.
  This does not authorize foreign-type implementations or specialization.
- Use a noncryptographic mixed integer/FNV string hash. The map is not hardened
  against attacker-chosen collision floods. Security-sensitive input needs limits;
  randomized hash defense is future work, explicitly not claimed here.
- Implement Map/Set as ordinary owned Tarn over Vec, with open addressing,
  power-of-two capacity, load <= 3/4 and gap repair after removal.
- Do not return Option<&V> or introduce stored reference iterators. `contains`
  tests absence; `get`/`get_mut` return ordinary direct loans and abort for missing
  keys, like Vec.get. Slot inspection and consuming drains provide iteration.
  Return provenance and conflicts use existing ownership analysis.
- Add Vec.swap as a mechanical byte exchange for arbitrary owned values, plus
  explicit modular integer arithmetic required by hashing. Ordinary arithmetic
  and numeric casts remain checked. Dereferencing a reference reads only Copy
  values, or denotes a place for borrowing/assignment; raw pointers keep FFI rules.
- Provide comparator-based unstable heapsort over Vec and first-match binary
  search over slices. No implicit ordering capability or stored iterator loans.
- Add blocking non-owning console stream views implemented over libc through FFI.
  They never close standard descriptors. Errors/partial I/O remain explicit;
  complete writes retry EINTR and treat zero progress as WriteZero. Stream output
  is unbuffered at the Tarn layer. Explicit flush also flushes libc stdio to
  interoperate with print; it does not flush owned network/application buffers.
- Add monotonic Instant and UTC Unix timestamps through Linux x86_64 clock_gettime
  ABI via FFI. Clocks report errors; durations retain millisecond precision.
  No calendar/time-zone API or new runtime scheduler is introduced.

## Validation requirements

Test collisions, removal/growth, negative/minimum integer keys, custom keys,
non-Copy payload destruction, loan escape/mutation rejection, sort boundaries
and binary-search misses. Test console pipes and output errors, clock ordering
and field bounds, and a deterministic proc-style fixture for the collector.
Run the workspace suite and live read-only collection. Keep source-proc metrics
separate from GUI support and do not describe the partial port as a full port.
