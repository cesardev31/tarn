# Phase 34 foundations: real-program and agent ergonomics

## What changed

The independent status-device report prompted fixes for counter resets, PID
reuse and hardcoded page size, plus a comparable Go collector. Parsing numeric
fields now uses range cursors instead of repeated owned splitting. New rfind,
blocking sleep and recoverable map helpers cover concrete missing APIs while
preserving v0 ownership and syntax.

The CSV-to-JSON program validates bounded input, owned CSV fields, locale-neutral
finite floats, ASCII normalization and explicit buffered output. Invalid input
never produces a partial success report. Formatting/replacement are ordinary
Tarn functions; no implicit conversions or stored references were introduced.

`tarn stdlib [module] --json` provides public signatures, doc comments and
source lines from the actual embedded declarations and real parser, plus a
SHA-256 identity for freshness checks. The agent guide points at executable
references. This is a source API catalog, not a semantic-provenance query.

## Tests

The pre-change complete cargo test baseline passed. Focused integration tests
cover collector resets and PID identity; string/numeric/map/CSV semantics;
quoted multiline CSV and UTF-8 across buffer boundaries; rejected malformed
input with empty stdout; line bounds, CRLF, EOF and discarded unflushed output.
CLI tests parse every embedded module and check catalog schema fields and
private-member exclusion. A C harness confirms numeric-locale restoration.
Strict C compilation and workspace checking produce no warnings. Valgrind on
successful/error CSV paths reports no definite or indirect losses.

The broad cargo test run passed CLI, backend and driver suites through the
resolver, where three old resolution goldens failed. Inspection showed only
added stdlib references, with zero changed/removed existing resolutions.
Only those three files were updated. The resolver rerun, remaining driver
suites and all other workspace suites passed. The added Copy-bound/provenance
checks passed, and the final numeric fixture passed after adding NaN rejection
and CSV bounds checks. No unresolved test failure remains. The LSP stdio smoke
suite passed. Debug/release CLI and debug LSP binaries were rebuilt; debug and
release catalogs have identical source hashes.

## Measurement

`evidence/status_device/comparison.json` records seven alternated runs, with
one warmup each, after compiler tests finished. Whole-process medians for twenty
rounds were 0.4032 seconds in Tarn and 0.4044 seconds in Go. That is effectively
similar in this sample, not a general performance claim. Live process counts and
system load vary; the workload excludes PSS/smaps_rollup and desktop integration.
An earlier, concurrently loaded run is retained separately as exploratory.
The reproducible compare.py harness records every sample and its output.

## Bugs and discrepancies found

The report overlooked existing Option.ok_or/map and Result.map_err, existing
spelling suggestions and the implemented tarn profile command. Those are
primarily discovery/documentation problems. It correctly found no reverse
search or synchronous sleep and the abort-on-missing get contract.
Conditional bounds on method owners are deliberately unsupported; get_copy is
therefore a free generic function. A different shape is not invented merely
for convenience, and existing Map.get semantics remain compatible.

## Decisions defended

Ordinary API improvements precede syntax changes. A catalog uses parser facts;
semantic provenance is not fabricated from source strings. Native helpers do
only libc numeric conversion; decimal grammar and errors remain Tarn policy.
Reader errors poison state rather than resume in an ambiguous record. Buffer
flush is explicit and partial sends are tracked for safe flush retries.

## Decisions still questioned

Copy map queries and borrowed fallbacks cover many callers, but an ergonomic
owned-value lookup for non-Copy values still needs evidence. Float display is
round-trip oriented and can be verbose. Offset cursors are allocation-free but
need further fresh-agent evaluation before claiming ergonomic improvement.

## Known limitations

Full 34A-34J tooling, independent fresh-agent evaluation, desktop integration
and the extended HTTPS classifier are not delivered by this foundation. The
updated phase-34-plan separates them from completed work. No live Chatwoot or
TypeSafe requests, production writes, commits or pushes were performed.
