# Standard Library General-Purpose Assessment

Assessment date: 2026-10-08.

## Verdict and scope

Tarn has a substantial foundational standard library, especially for owned
resources, networking and concurrency. It is not yet a generous general-purpose
library: everyday collections, algorithms, console I/O, text and number handling,
and clocks remain more limited than its systems infrastructure.

This assessment inventories the current workspace, including uncommitted HTTP
and networking changes. It reviews source APIs, documentation, examples and the
presence of test fixtures; it does not establish that the current tests pass,
that every advertised operation works, or that performance and production
readiness have been validated. No compiler or runtime changes accompany it.

The target is practical general-purpose development on the supported Linux
x86_64 platform. Platform expansion is a separate decision. A standard library
alone cannot make a language general-purpose: language ergonomics, diagnostics,
tooling, package availability and operational reliability also matter.

## Phase 34 foundations follow-up

The original inventory is historical. The workspace now additionally provides
finite float conversion, ASCII casing/comparison, replacement and string
formatting, reverse search, bounded console line input, explicit buffering,
owned streaming CSV, blocking sleep and recoverable map lookup helpers.
A CSV-to-JSON CLI and corrected status-device collector provide executable
pressure. `tarn stdlib --json` exposes embedded public APIs and source identity
for agents; existing Option.ok_or/map and Result.map_err were overlooked by
an independent port report, so discovery matters as much as API coverage.

See [foundation report](phase-34-foundations-report.md),
[updated execution plan](phase-34-plan.md) and [agent guide](llm-guide.md).
Full agent-tooling milestones, desktop integration, richer Unicode/calendar
handling and the extended classifier remain open. No broad production-readiness
or language-wide performance claim follows from these examples.

## Phase 31 follow-up

The initial assessment below predates Phase 31. The workspace now implements
Map/Set with nominal Hash/Eq, Vec heapsort and slice binary search, blocking
console streams, monotonic Instant, Unix UTC timestamps and ASCII string fields.
See [collections](collections.md), [console](console.md), [clocks](clocks.md) and
[the partial metrics port](../examples/status_metrics/README.md). These additions
address parts of the original gaps; generic I/O/line reading, more collections,
rich text/float formatting, calendar handling and HTTPS clients remain open.

The three documentation discrepancies listed below have been corrected.
Implementation scope and validation: [Phase 31 report](phase-31-report.md).
Phase 29's workload-specific strings benchmark reported 2.6x Go's elapsed time
([source report](phase-29-report.md)); this has not been remeasured here.
Owned substring allocation and byte-loop checks remain text-performance work.
Borrowed substring design must preserve the current aggregate-reference boundary.

## Phase 32/33 follow-up

Offset field/line/split traversal now avoids copying parts; borrowed UTF-8 byte
views retain normal source provenance. Builder integer append removes temporary
strings. A bounded UTF-8 stdin word-frequency CLI supplies application evidence.
The ordinary https module now provides a bounded blocking GET/POST client via
system libcurl with full TLS verification and query-component encoding. A
selected-ticket classification CLI exercises Chatwoot and TypeSafe contracts
against local HTTPS mocks. Parsed JSON Values now support lossless encoding.

General borrowed &string values, richer text/float formatting, Unicode utilities,
streaming/async HTTPS, richer URL handling and a complete Go application port
remain open. Scope, measurements and validation:
[Phase 32/33 report](phase-32-33-report.md).

## Original coverage (before Phase 31)

The original assessment covered 13 modules; Phase 31 adds collections and console. Module count is not a measure of
completeness: depth, composability and the amount of application code required
are more useful criteria.

| Module | Current coverage | Important boundaries |
|---|---|---|
| `core` | Option/Result helpers, Vec, Eq/Copy/Transfer/Share capabilities, native task handles, Mutex/guards, concrete atomics, string primitives, abs and floating-point sqrt | Vec is the only public general-purpose collection; much of the foundation uses declared compiler intrinsics |
| `string` | UTF-8 validation, owned slices, find/contains/prefix/suffix checks, ASCII trim, split/lines, integer parsing/formatting, strip_prefix, Builder | Byte offsets; owned split results; limited Unicode and numeric conversion |
| `io` | Shared error vocabulary, native error information, Waker and Progress | No general public reader/writer ecosystem or console stream API |
| `fs` | Regular-file open/create/append, read/write, complete reads/writes, seek, sync, close, metadata, directory listing and basic mutation | Blocking regular files; metadata exposes only length and file/directory classification |
| `path` | Owned paths, join, lexical normalization, components, parent, filename/stem/extension, component-aware prefix operations | POSIX UTF-8; lexical operations do not resolve filesystem symlinks |
| `process` | Command arguments, child environment overrides and working directory, start/status/output, child ID, consuming wait, kill, validated output text | Blocking; captured output is unbounded; no public stdin pipes or timeouts; destruction waits/reaps |
| `os` | Arguments, environment text/bytes, fallback values, immediate exit, executable-access checks, disk space | Small Linux-specific surface; args uses procfs; not a comprehensive OS abstraction |
| `net` | TCP/UDP, address types and resolution, socket ownership, nonblocking mode, readiness registration, async operations, TCP buffered readers/writers | No TLS; DNS resolution blocks; buffering is specialized to TCP rather than generic I/O |
| `time` | Millisecond durations, one-shot readiness timers, async sleep | No public wall-clock/date/calendar/time-zone or instant-measurement API |
| `runtime` | Explicit execution, pollable operations, cooperative tasks, executor, block_on, task join and timeouts | Defined executor and ownership boundaries; not a general multithread async scheduler |
| `http` | Server-side bounded HTTP/1.1, owned requests/responses, headers/trailers, framing validation, chunked request parsing, deadlines, response helpers, sequential serving and parallel workers | No HTTP client/TLS; no application body streaming, HTTP/2/3, WebSocket or complete routing/middleware framework |
| `json` | Strict parsing, owned value trees, preserved number text, member access, explicit Encode interface and writer | No automatic typed decoding; access can clone trees; integer accessors do not convert fractional/exponent numbers; writer balance is the caller's responsibility |
| `ffi` | Pointer conversion/access primitives, byte-copy support and owned C strings | Unsafe interoperability is an escape hatch, not a substitute for ergonomic safe APIs |

The HTTP CRUD example demonstrates composition across HTTP, JSON, strings and
filesystem persistence. Native fixtures exist for filesystem, paths, processes,
environment handling, JSON, HTTP, atomics and async execution. Their existence
is evidence of intended coverage, not a fresh execution result.

Sources: [stdlib](../stdlib/), [HTTP CRUD example](../examples/http_crud.tarn),
[native fixtures](../tests/native/pass/).

## Strengths to preserve

- Resource APIs have deliberate ownership and completion behavior. Files,
  sockets, processes and task handles are not interchangeable integer handles.
- Shared I/O errors make composition possible without hiding ordinary failures.
- Networking includes blocking, readiness and async layers instead of exposing
  only raw system calls.
- HTTP handles bounded input, framing and deadlines rather than only a happy-path
  parser. This is meaningful depth within its documented protocol subset.
- JSON, strings, paths and application HTTP policy are substantially expressed in
  Tarn, helping expose real language ergonomics.
- Explicit executors and ordinary ownership keep scheduling and resource lifetime
  decisions visible.

These strengths should survive convenience improvements. New APIs must not hide
borrows, detach owned work, introduce implicit shell execution or move semantic
ownership decisions into the backend.

## Original gaps and priorities (before Phase 31)

### Collections and algorithms: highest priority

Missing public facilities include maps, sets, queues/deques, ordered collections,
and reusable sorting/searching operations. Applications otherwise repeatedly
implement linear scans and collection management.

Start with a coherent equality/hash contract, a map, a set and slice sorting and
searching. Decide how keys, mutation and borrowed access fit the existing
ownership model before choosing representation. Do not implement structural
user-type equality or stored references as an incidental shortcut.

Acceptance evidence should include a word-frequency CLI, indexed CRUD storage,
custom key types, collision tests, owned non-Copy values, mutation/borrowing
regressions and realistic performance measurements. Algorithmic behavior and
ordering guarantees must be documented.

### Console and composable I/O: highest priority

Public stdin/stdout/stderr streams, line input, explicit flush, fallible output,
and reusable buffering are missing. Existing printing does not provide a general
stream abstraction; fs.File is intentionally restricted to regular files.

Define reader/writer contracts only after exercising console, files, sockets and
in-memory buffers. Preserve partial counts, EOF, byte/text separation, ordinary
loans and explicit flushing. Async contracts deserve separate design rather
than assuming a blocking interface can transparently become async.

Acceptance evidence should include interactive input, Unix pipelines, redirected
output, broken pipes, invalid UTF-8, partial operations and bounded line reading.

### Text, formatting and numerical utilities: high priority

Missing areas include replacement, case conversion, character classification,
float parsing/formatting, general formatting, and a broader mathematical surface
such as rounding, trigonometry, powers and logarithms. Exact scope should follow
real workloads rather than copying another language's API catalog.

Distinguish bytes, Unicode scalar values and grapheme clusters. ASCII behavior
must be named or documented explicitly. Formatting should work with application
types without relying on reflection or violating nominal interface rules.

Acceptance evidence should include multilingual text, floating-point round trips,
special values, boundary cases and efficient text accumulation. Richer borrowed
iteration must respect the current prohibition on stored aggregate references.

### Clocks and dates: high priority

Timers cannot replace a public monotonic instant API for elapsed-time measurement
or a wall-clock timestamp API for logs and persisted records. Calendar parsing,
formatting and time-zone support are additional gaps.

Prioritize monotonic measurement and UTC timestamps first. Keep duration arithmetic,
clock adjustments, overflow and precision explicit. Calendar/time-zone support
can follow with a deliberate data-source and update policy.

Acceptance evidence should include elapsed-time measurements, timestamp round
trips, invalid dates and documented behavior under wall-clock changes.

### Internet client capabilities: high priority for application development

There is no standard HTTP client, TLS, or URL/query utility layer. Consequently,
a basic external HTTPS API integration requires substantial additional work.
Server support also lacks streaming application bodies, multipart and WebSocket.

Prioritize URL/query handling and a bounded HTTP client with explicit timeouts,
redirect policy and response limits. TLS needs a dedicated dependency and security
review: implementing cryptographic protocols internally merely to avoid a
library would be a poor tradeoff. HTTP/2/3 and WebSocket should follow demonstrated
needs rather than block a useful initial client.

Acceptance evidence should include an HTTPS JSON client, certificate and hostname
verification failures, redirects, truncated bodies, limits and timeout cleanup.

### Everyday data utilities: medium to high priority

No public standard APIs were found for random generation, secure random bytes,
Base64, hashing, cryptography, regular expressions, compression or structured
logging. These affect identifiers, protocols, integrity checks and observability.

Separate deterministic application hashing from cryptographic hashing and
noncryptographic randomness from secure randomness. A public security-sensitive
API needs vetted implementation and explicit failure behavior. Regex, compression
and structured logging may reasonably be packages if package installation and
maintenance are dependable.

### Filesystem, processes and OS depth: medium priority

Filesystem gaps include richer metadata, permissions, symlink operations,
canonicalization, recursive directory creation/walking, copy helpers and temporary
files/directories. Process gaps include configurable stdio, writable stdin,
streaming bounded capture, nonblocking status checks and timeouts.

Do not interpret this list as authorization for recursive destructive operations,
detached processes or implicit cancellation. Every new operation needs deliberate
ownership and error semantics. Async filesystem/process APIs are separate design
work; blocking calls must remain visibly blocking in async applications.

### Structured data ergonomics: medium priority

JSON supports useful applications, but explicit field encoding, manual decoding
and cloned access increase application work. Numbers can be parsed losslessly as
text, while convenient numeric conversion remains limited.

Improve typed decoding and error paths using normal language mechanisms before
introducing derive or reflection. Consider read-only traversal that avoids large
clones only within a sound ownership design. Benchmark actual documents before
reworking representation. CSV and configuration formats can initially live in
packages.

### Concurrency composition: workload-dependent priority

Native threads, mutexes, atomics and cooperative tasks provide a solid base.
Channels, bounded work queues, condition variables and ergonomic task supervision
are missing. Async cancellation, scoped tasks and multithread scheduling require
separate design phases and must not be inferred from a desire for completeness.

Prioritize bounded producer/consumer workloads and explicit failure/completion
handling. Avoid adding a second task model or weakening current loans to make
composition easier.

## Standard library versus ecosystem

| Placement | Suggested scope |
|---|---|
| Standard-library foundations | Collections, algorithms, console/I/O contracts, text/numeric essentials, clocks, filesystem/process basics, random bytes and basic encodings |
| Explicit placement decision | HTTP client, TLS integration, advanced Unicode, regex, compression, structured logging and calendar/time-zone facilities |
| Initially external packages | PostgreSQL/SQLite/Redis drivers, ORMs, web frameworks, authentication integrations, cloud SDKs, GUI and domain libraries |

The absence of a database driver or ORM does not by itself make the standard
library poor. The absence of a map, sorting or straightforward console input
forces basic application infrastructure into every project.

External packages are a credible answer only when resolution, integrity checking,
documentation and integration are reliable. Current pure-source package support
must not be represented as an implemented public registry, provenance system or
build sandbox. See [packages](packages.md) and
[dependency security](dependency-security.md).

## Language and tooling requirements beyond stdlib

A general-purpose Tarn must also demonstrate:

- Concise application flows without redundant ownership ceremony. Use real
  programs to identify friction while preserving concrete safety guarantees.
- Consistent diagnostics and machine-readable compiler facts, backed by the same
  frontend used in the editor.
- Predictable builds, package verification and actionable dependency conflicts.
- Useful testing, formatting, documentation and performance investigation tools.
  This report does not independently audit completeness of those tools.
- Correct resource destruction on success, error, early return and suspended
  abandonment, with regression tests for compiler and library bugs.
- Performance evidence for allocation, text building, collection operations,
  parsing, I/O and concurrency; API presence is not performance proof.
- Maintained API documentation and migration guidance. Public signatures and
  behavioral limits must remain discoverable without reading compiler code.

Adding modules cannot compensate for unsound ownership, compiler instability,
poor diagnostics or an unreliable package workflow.

## Recommended sequence

| Order | Work | Concrete outcome |
|---|---|---|
| 1 | Collections/algorithms and console I/O | Ordinary CLIs and in-memory application state need little custom infrastructure |
| 2 | Text/float formatting, clocks and basic encodings | Logs, records, numeric data and common protocols become straightforward |
| 3 | URL handling and HTTPS client integration | Applications can consume external APIs with bounded resources |
| 4 | Filesystem/process depth and JSON decoding | Automation and data applications become more ergonomic |
| 5 | Workload-driven concurrency and package libraries | Larger services compose safely without expanding the runtime speculatively |

This is a prioritization proposal, not an approved implementation phase or a
schedule. Each step should begin with representative Tarn programs, identify the
smallest necessary contracts, and then add semantic and runtime regression tests.

## Readiness criteria

Before calling Tarn a generous general-purpose language, demonstrate these
workloads with documented APIs and little bespoke infrastructure:

1. A CLI that reads stdin, parses options, groups/counts data, sorts results and
   reports fallible output.
2. A file automation tool that walks directories, manages temporary output and
   runs subprocesses with bounded capture and completion control.
3. An HTTPS JSON client with URL encoding, timeouts and useful decoding errors.
4. A concurrent HTTP service with shared indexed state, bounded admission,
   timestamps and observable failures.
5. A numeric/text data utility handling floats and multilingual input predictably.
6. A third-party package integration with reproducible locked dependency bytes
   and integrity failures that refuse execution.

Successful demonstrations should include negative cases and destruction paths,
not just successful output. They should separate standard-library capability,
package capability and compiler restrictions.

## Original documentation discrepancies (now corrected)

Resolved on 2026-10-08: the documents below now describe these APIs.

- `http.serve_parallel` and `http.Handler` provide parallel native workers, each
  with a separate listener and executor; this does not imply a multithread async
  scheduler. HTTP documentation still describes thread pools as deferred.
- `process.Command.env` allows child environment overrides, while the process
  document still lists custom environments as absent.
- `string.Builder` exists, while the strings document still lists mutable
  builders as deferred.

Resolve those descriptions separately and distinguish landed support from local
work in progress. Documentation gaps should not be mistaken for missing APIs,
and local source should not be mistaken for released, validated support.
