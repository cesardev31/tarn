# Phase 32/33 report: text performance and a real HTTPS workflow

Design: [ADR 0061](adr/0061-offset-text-traversal.md),
[ADR 0062](adr/0062-bounded-system-https.md).

## Implemented

- Ordinary offset-based text traversal: Range, SplitCursor, next_field,
  next_line, next_split, split_ranges and split_count. Borrowed range_bytes
  checks UTF-8 scalar boundaries and retains the source's ordinary loan.
  copy_span explicitly obtains an owner. prefix_scalars supports bounded
  Unicode-scalar truncation without allocating; parse_u64_bytes parses borrowed
  ASCII decimal directly. No stored references or string-layout change.
- Builder integer append formats into stack arrays and bulk-appends, eliminating
  temporary decimal-string allocations, including signed-minimum handling.
- examples/textstats: stdin word counts, validated UTF-8, 8 MiB input bound,
  descending-frequency/byte-lexicographic output, fallible console writes.
  Field traversal allocates no parts vector; map keys remain owned and repeated
  key lookup still creates temporary strings.
- Semantic Copy bound fix: declared copy structs must use the Copy capability
  before nominal ADT implementation lookup. Regression: Vec<Range>.at. Existing
  ownership/provenance rules still reject source escape and overwrite.
- An ordinary bundled, overrideable https module with blocking GET/POST,
  RFC 3986 query-component encoding, mandatory certificate/hostname verification,
  explicit libcurl timeout, bounded owned response bytes and HTTP status results.
  Native handles/header lists stay local and are cleaned on all paths. libcurl
  loads lazily from the system; no Cargo dependencies or curl subprocess.
- Parsed JSON Value implements Encode. Nested values and exact number spelling
  round-trip; even manually created Number text is validated before output.
- examples/ticket_classifier: selected-conversation Chatwoot reads, sorted/
  filtered/limited messages, 2,000-scalar truncation, one request with all three
  TypeSafe questions, and lossless JSON output. Credentials are environment
  inputs. No live external requests or operational ticket writes were performed.

## Performance evidence

Benchmark construction/split workloads return the same 4,200,000 count. Baseline
string source comes from commit 533aec7; it is compiled as an ordinary local module
with the same final compiler. Go is 1.26.0 on Linux amd64. Native executables use
Tarn's normal optimizing backend, not Rust compiler release speed as a proxy.
The ranges variant uses offsets; original Tarn split still obtains owned parts,
whereas Go strings.Split obtains source-backed string views.

Initial five-run measurements were noisy: baseline median 1.214 s versus current
0.513 s and Go 0.252 s. A repeated series produced baseline 0.711 s, current
0.515 s, ranges 0.455 s and Go 0.247 s. Do not treat the first 2.4x improvement
as a stable result. Background validation/system load and warmup affect timings.
These establish useful allocation removal, not universal speed parity with Go.
Final five-run measurements use one warmup per executable and rotating execution
order, with the same final compiler and identical output:

| Variant | Median seconds | Min–max seconds |
|---|---:|---:|
| Phase-31 string source | 0.599 | 0.588–0.674 |
| Builder improvement; owned split | 0.516 | 0.507–0.519 |
| Builder improvement; split offsets | 0.450 | 0.448–0.475 |
| Go strings.Builder / strings.Split | 0.253 | 0.246–0.324 |

The final sample improves the original workload by about 14%; the offset variant
is about 25% faster than the Phase-31 baseline. Tarn still takes about 2.0x Go's
time for owned split and 1.8x for offset split. This is one short workload under
ordinary workstation load, not a controlled performance certification.


Reproduce current comparisons with `benchmarks/vs-go/run.sh strings strings-ranges`.
To recreate the baseline, put the HEAD/Phase-31 string module and strings.tarn
benchmark into a temporary directory and compile its main.tarn. Keep source
semantics and compiler settings identical. Measure repeated executions and check
outputs before comparing elapsed time.

## Validation

- Full workspace build rebuilt CLI and LSP; native C also compiled cleanly with
  -Wall -Wextra. No language-server restart was performed.
- Focused frontend tests cover Copy bounds, normal owner provenance and existing
  collection/coherence behavior. Native text assertions cover UTF-8 boundaries,
  empty/trailing splits, CRLF/bare CR, scalar prefixes and integer extremes.
- Backend integration exercises empty/non-ASCII/invalid/oversized stdin input,
  missing libcurl, local trusted/untrusted TLS, hostname mismatch, GET/POST,
  redirects disabled, HTTP errors, response bounds, timeout and truncated bodies.
- The local mock workflow checks Chatwoot authentication, private/system/blank
  filtering, chronological messages, last-20 and 2,000-scalar limits, no-customer
  rejection, TypeSafe request shape and lossless score output. No external model
  prediction or real-service compatibility was claimed.
- Valgrind on the UTF-8 CLI: 20 allocations/20 frees, zero bytes/blocks at exit,
  zero errors. HTTPS diagnostic fixture under Valgrind: zero errors, zero
  definite/indirect/possible losses, 50,312 bytes still reachable and 4 bytes
  suppressed by system suppressions. It deliberately retains process-lifetime
  libcurl/global state. The diagnostic copy used 15-second successful-request
  deadlines to accommodate instrumentation; the intentional timeout stayed
  100 ms. The first instrumented run exceeded its normal 2-second GET deadline,
  so it was not mistaken for an ownership failure.
- The sandbox denied local sockets; the same TLS tests passed with local socket
  access enabled. This was a tooling permission issue, not an HTTP failure.
- Resolver snapshots include string source bodies, so the import snapshot is
  refreshed for new declarations/line numbers; reruns do not use blessing.

The broad `cargo test --workspace --no-fail-fast` completed: 278 passed,
3 failed, 1 ignored. It started before prefix_scalars was added;
three tests used an older embedded string source while reading newer fixture/
application files, and failed with E2005 for that missing member. No other broad
failure occurred. This was not a single clean full-workspace run.

Final rebuilt-target replacements:

- `cargo test -p tarn_backend --test native native_scalar_and_aggregate_suite`:
  1 passed (22 filtered); the other 22 native tests passed in the broad run.
- `cargo test -p tarn_backend --test text_https`: 3 passed, including the added
  unavailable-dependency test and the final bounded-ticket fixture.
- Final frontend resolve/text-range/type suites: 6 passed; final collection/editor
  suite: 2 passed. Editor checks include string, json and https ordinary sources.
- Console/foundations: 4 passed. Workspace build and git diff --check are clean.

All observed broad failures have passing replacements using final sources.
Logs: /tmp/tarn-phase32-33-workspace.log, /tmp/tarn-phase32-33-native-final.log,
/tmp/tarn-phase32-33-integration-final.log and
/tmp/tarn-phase32-33-frontend-final.log.

## Limits and remaining work

Text views are borrowed UTF-8 bytes, not a new general &string substring type.
Owned split/fields still copy. Byte bounds checks remain. General formatting,
float parsing/formatting, replacement/case utilities and richer Unicode remain
open. Scalar truncation can separate graphemes/combining marks.

HTTPS is blocking, HTTPS-only, one optional application header, buffered response,
no redirects/proxies/cookies/retries/compression decoding. No response headers,
streaming, reusable connections, async client or complete URL parser. DNS timeout
semantics depend on system libcurl's resolver backend. Dependency weight and
OS-maintained security updates are explicit in ADR 0062.

The Go application port is deliberately a selected-ticket console workflow. It
omits its web UI/auth, pagination, polling/worker orchestration, cache, label/
priority writes, auto-apply and 429/529 Retry-After/backoff. Scores remain exact
JSON instead of becoming automated priority decisions. Equal timestamp ordering
is unstable. Model quality requires real labeled data, separate from transport
and deterministic workflow validation.
