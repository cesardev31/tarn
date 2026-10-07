# ADR 0049: integrated isolated test runner

Status: accepted for Phase 19.

## Context

Panic aborts the process. A test runner cannot recover by unwinding. The CLI
needs deterministic discovery, one build, explicit timeouts and agent output.

## Decision

`tarn test [entry]` discovers ordinary, non-generic, safe synchronous functions
named `test_*` in the entry and sorted sibling `*_test.tarn` files. Tests have
no parameters or receiver and return void or `Result<void, E>`. An invalid
signature fails discovery. Imported helper modules are not test roots.
Sibling files remain separate modules: tests can access their own private
items, while access to the application follows ordinary public visibility.
No new attributes, assertions, visibility exceptions or stdlib module are added.

The driver generates source harnesses in the owning modules, then runs the
normal frontend, ownership checks and drop elaboration. The original application
main remains available. A distinct harness entry is selected explicitly for
monomorphization and code generation; the backend never infers test ownership.
One executable selects an integer test index through argv. This is a private
harness ABI, not a public process-arguments API. Every selected test starts a
fresh child process, sequentially. The private entry disables Linux core
collection after exec so an abort is not delayed by an external core collector.
Err values are reported before an abort;
printable scalars, enum variant names/payloads and accessible scalar struct
fields are shown; nested non-scalar values retain their type boundary.
This does not introduce a general reflection API.

`--filter` selects by substring of module-qualified name. `--timeout` takes
positive integer milliseconds (default 10000). Timeout kills the isolated
process group and reaps the direct child. stdout and stderr are drained
concurrently. Every test reports status and captured output; JSON Lines emit
one test record and a summary. No matches succeeds with zero tests. Compilation
errors fail before any child runs. `--link` retains explicit native grants.

Watch polls every 250 ms and debounces one stable interval. Run builds before
replacing the child; failed builds preserve it. Replacement sends SIGTERM to
the child's isolated process group, waits up to two seconds, then SIGKILL and
reaps. SIGINT/SIGTERM request session shutdown and the same cleanup. Test watch
also observes the sibling test-file set so additions and removals trigger a run.
There is no detached watch child or alternate compiler/runtime ownership model.

## Consequences

Tests may panic without preventing later tests. Abort and timeout do not
promise in-process resource cleanup; OS process isolation is the boundary.
Output capture preserves bytes. Text decoding validates UTF-8; invalid streams
are reported as byte arrays, with null text fields in JSON. No user code executes during discovery. Clean
remains explicitly deferred: current outputs live beside source and there is
no safe CLI-owned artifact inventory or cache directory.

Generated harness references to prelude reporting, panic, Result variants and
the dispatcher integer type bind to stable prelude symbols after resolution.
Only appended source ranges receive these bindings; application names retain
ordinary resolution. This prevents shadowed application names from changing
the harness failure contract. All remaining frontend and ownership passes are
unchanged.
