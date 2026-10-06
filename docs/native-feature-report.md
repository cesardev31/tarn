# Native feature completion report

Historical phase-8 report. Borrowed dynamic dispatch and declaration contracts
are now documented in [the phase-9 report](dynamic-interface-report.md).

## What was implemented

Reachable generic specialization, concrete generic structs/enums including nested
owned resources, borrowed slices/fat references, borrowed nonescaping closures,
conservative semantic rejection of unknown stdlib APIs, shifts and checked
float-to-integer conversions. Dynamic `any I` dispatch was not started: the scope
stops at semantic stdlib closure, as explicitly permitted by the phase request.

## Backend/runtime changes

A backend-local specialization pass creates a verified concrete post-drop program.
Canonical layouts now include slice and callable pairs; codegen supports indirect
thunk calls and environments. No runtime helper, dependency, ownership query or
optimization pass was added. Native runtime ABI remains private and abort-only.

## New ABI/layout decisions

See ADR 0028: slice `(data, length)` and callable `(code, environment)` are each
16 bytes, aligned to 8, passed using the existing private aggregate ABI. Thunks
receive an environment pointer after the hidden result pointer if present. Capture
environments live on the creating stack frame. ADR 0029 defines checked shift
counts, signed arithmetic right shift, bit-pattern left shift and truncating,
range-checked float conversions.

## Tests added

Real executables exercise generic values/ADTs, arrays and nested string resources;
shared/mutable slices, unsizing, nested-reference iteration and bounds faults;
shared/mutable/multiple/nested captures and borrowed-return calls; numeric limits,
NaN/infinity and invalid shifts. Explicit specialization tests check reuse,
reachability, mutual recursion and deterministic naming. Generic destruction
traces match the existing resource-token interpreter. Memory-safety cases verify
E3040 rejection and preservation of declared result loans; golden diagnostics
cover unknown external APIs/types. Existing parser, resolution, types, moves,
borrows, drops, memory_safety, native/CLI and mutation suites passed `cargo test`.
The additional final numeric edge cases also pass the native suite. Baseline
collection is a separate ignored test; measurements and commands are in
`benchmarks/native/README.md`.

## Bugs found

Nested-reference slice iteration was rejected by type checking and would have
been lowered with only one Deref. It now peels all references in both stages.
Opaque stdlib calls previously had empty result-loan inflow and unknown values
were assumed not to hold references. Unknown calls/types/constructors now reject;
recovery IR uses conservative inflow. Placeholder Error and channel operations
also reject missing contracts. Several illustrative bootstrap stdlib examples
therefore deliberately stopped type-checking; name-resolution tests permit only
the documented E3040 cases.

## Decisions I would defend

FIFO reachable monomorphization with reservation before recursive traversal;
one layout authority; pair representations and small thunk ABI; stack environments
for already nonescaping captures; conservative semantic rejection instead of
invented provenance; explicit numeric boundaries before native instructions.

## Decisions I still question

The 256-instance and type-depth/node bounds need real program evidence. Existing
callable consumption is one-shot; reusable calls need a deliberate semantic change.
All reachable functions currently get thunks, and aggregate copies/drop expansions
are straightforward rather than optimized. External API metadata, reusable runtime
builds and dynamic-object destruction need future design and measurement.

## Remaining frontend/backend feature gaps

Virtual dispatch/any, ownership-capturing and escaping closures, generic dynamic
or unsized by-value substitutions, owned slices, unmodeled external stdlib APIs,
extern ABI implementation, concurrency/spawn, custom destructors, unwinding and
other platforms remain unsupported. No public aggregate ABI is promised. Explicit
core declarations are supported; the native C/intrinsic boundary remains trusted.
This work does not justify claiming complete end-to-end language memory safety.
No optimization work follows this report.
