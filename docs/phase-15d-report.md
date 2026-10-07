# Phase 15D report: owned blocking child processes

Design: [ADR 0045](adr/0045-blocking-processes.md).
Public API: [process](process.md).

## Implemented

Command owns program, arguments and optional child working directory. Fluent
new/arg/args/current_dir builders consume their inputs; start/status/output borrow
reusable configuration. Process supplies diagnostic id, mutable kill and consuming
wait. ExitStatus distinguishes exit codes from signals and provides success/code/
signal queries. Output owns stdout/stderr bytes with strict, owned text conversion.
The Swift-version example uses actual declarations and explicitly interprets a
nonzero exit as an application failure. Milestone 5 fs/path/process is implemented
within the documented Linux v0 limits; an xlinux port remains separate work.

## Ownership and capabilities

Process uniquely owns completion, is non-Copy, Transfer and not Share. Wait
consumes it, including on error. Move, task capture/result and post-drop transfer
or discharge the same obligation. Destruction waits/reaps; kill requests SIGKILL
without releasing completion. Private capture pipes also have Transfer, not Share,
and exactly-once close. No detach, implicit kill, runtime owner bit, native child
registry, backend ownership inference or user-defined Drop was added.

Configuration and output use ordinary owned ADTs/Vec/string capabilities. Output
views borrow their owner; text conversion copies only after validating UTF-8.
Nonzero exit/signal is an ExitStatus, not an I/O error. Acquisition/read/wait errors
return io.Error with useful categories and original codes in private domain 4.
Existing network/filesystem categories are unchanged.

## Runtime and task integration

Private C uses posix_spawnp and GNU libc chdir/closefrom actions, CLOEXEC pipes,
read, waitpid and kill. No Tarn code executes in a post-fork child. Arguments
are passed separately, never implicitly interpreted by a shell. Environment and
PATH are inherited; working-directory changes are child-only. Child unrelated
fds >=3 are closed, its signal mask is empty and SIGPIPE has default disposition.

Two Phase 11 native tasks drain stdout/stderr concurrently. Tarn owns buffers,
read retries, error propagation, joins and UTF-8 conversion. Capture stdin is
/dev/null; start/status inherit stdio. The consuming wait bridge retries EINTR
without manufacturing another owner; Linux close is never retried. Raw acquisition
lanes are immediately wrapped into owners, retaining the documented fs/net
bootstrap boundary. Process has a separate verified declaration catalog, including
consuming signatures that bind the identity of both child and pipe resources.

## Tests

`cargo test --workspace --locked -j4`: 204 passed, no warnings, one existing
benchmark ignored. The final trace/test refinements also pass the focused native
process suite (eight tests); the application example passes `tarn check`.
Ten new Rust tests cover
real native children, literal space/metacharacter/Unicode arguments, PATH lookup,
inherited environment, child cwd, reusable commands, inherited versus null stdin,
normal/nonzero/signal/SIGKILL completion and binary/strict UTF-8 capture.

Each large child writes 131,073 bytes to both stdout and stderr. Sixteen concurrent
workers capture a deterministic total of 2,097,168 stdout bytes. Trace assertions
cover normal exit, return, break, continue, conditional init, local move, overwrite,
self-assignment, try failure and task transfer/results. A separate wait-attempt
trace detects duplicate completion calls even when the second waitpid would
return ECHILD; mutated missing/duplicate wait traces are rejected. Private C runs 64
spawn/drop/failure cycles with /proc/self/fd balance and waitpid/ECHILD checks.
It verifies unrelated non-CLOEXEC fds do not escape and closed parent stdio does
not collide with capture pipes. Fault injection covers pipe acquisition/fd
relocation failure, EINTR and partial reads, read failure, permission failure,
wait EINTR and close EINTR. Test C/runtime compile with -Wall -Wextra -Werror.

Metadata corruption rejects Copy, Share, public resource storage, missing
intrinsics and substituting the same-shaped fs.File for a pipe owner. Canonical
memory safety adds eleven move/borrow/capability/builder/output-provenance cases.
A process fixture joins native goldens and both mutation corpora. Local source
cannot replace trusted process contracts; official editor-source analysis works.
Former opaque-process tests now use the still-unimplemented http/tls placeholders.

## Bugs found

The initial acquisition draft retained stale fd lanes if relocation failed with
closed stdio, risking double close in its error cleanup. Pipe acquisition now
publishes its lanes only after both descriptors succeed; deterministic relocation
failure and real fd-count tests cover it.

A layout-only pipe catalog could accept an unrelated fd-shaped resource identity.
A private consuming close signature now binds the catalog to its actual Pipe type;
a same-shaped File substitution is rejected. No capability or ownership safety
was weakened. The existing parser reserves spawn; public start avoids expanding
syntax for one module.

## Decisions I would defend

Owned completion with wait-on-drop avoids unstructured zombies and matches Task.
Separate argv entries, child-only cwd and closefrom actions make application
execution predictable without modifying parent globals. Reusing native tasks
for dual capture keeps high-level control in Tarn and prevents pipe deadlock.
Bytes-first output and explicit exit observations avoid lossy/error conflation.
Separate error/resource catalogs preserve existing layers and verified semantics.

## Decisions I still question

Wait-on-drop can block indefinitely; persistent-child applications need a future
explicit policy without silently introducing detach. Two reader pthreads per
capture are a correctness baseline, not a performance design. Unbounded capture,
descendant-held writers, custom stdin/environment, process groups/pidfds and async
child I/O need application evidence and separate decisions. Raw acquisition lanes
and errno retry in consuming wait remain bootstrap ABI debt. GNU libc 2.34+
actions constrain initial portability, consistent with Linux x86_64 scope.

## Known limitations

No try_wait, process timeout, cancellation, public pipes, input writer, custom env,
process groups, shell parser, platform expansion or async process integration.
Foreign native reaping/SIGCHLD changes must respect the owned-child contract.
Reader-task creation and buffer allocation retain Tarn's existing abort-on-
resource-failure policy. There is no claim of subprocess sandboxing. No package hooks, package manager,
xlinux port or Phase 16 HTTP was implemented. Stop after 15D.
