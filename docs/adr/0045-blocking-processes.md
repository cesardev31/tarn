# ADR 0045: owned blocking child processes

Status: accepted

## Decision

Phase 15D adds a trusted `stdlib/process` layer above io. Public configuration,
byte accumulation, UTF-8 conversion and error categories remain Tarn. Private C
bridges posix_spawnp, spawn actions, read, waitpid and SIGKILL. No shell is implied:
program and arguments are separate UTF-8 strings and NUL is rejected.

The public launch method is `start`, because `spawn` is reserved syntax; no
parser change is needed. Command owns program/arguments/optional working-directory
text. Fluent builder methods consume the command; start/status/output borrow it and can be reused.
Environment and PATH are inherited. No global environment or parent working-
directory mutation is introduced. An explicit shell program remains possible.

Process uniquely owns child completion authority and is non-Copy, Transfer and
not Share. id borrows, kill borrows mutably and requests SIGKILL without reaping;
wait consumes, including on error. Verified post-drop waits/reaps on every normal
exit without detaching or killing implicitly. Drop can block indefinitely.
No runtime owner flag, child registry, backend ownership inference or user Drop.
Foreign reaping/SIGCHLD disposition changes are outside safe Tarn's API and may
invalidate OS completion assumptions; external native code must respect ownership.

ExitStatus explicitly distinguishes exited codes from terminating signals.
Nonzero exit and signals are successful observations, not I/O errors. Output
owns both byte vectors; strict text conversion returns InvalidData on invalid
UTF-8. Public fallible operations return io.Error, preserving original errno in
private process domain 4 without changing network/filesystem mappings.

Start/status inherit stdio. Output uses /dev/null stdin and two private owned
pipe readers in Phase 11 native tasks. Both readers start before waiting; stdout
and stderr cannot deadlock by filling one pipe while the other is read. Read
loops/byte accumulation/EINTR retry live in Tarn. On an error, ordinary Task and
pipe destruction joins readers and closes handles. Capture is unbounded and
waits for pipe EOF, which descendants may delay. No timeout, cancellation,
streaming/public pipe API, process group or async scheduler is introduced.

Spawn uses CLOEXEC pipes and GNU libc chdir/closefrom spawn actions. Child closes
all descriptors >=3 after mapping stdio, so unrelated files/sockets/runtime fds
are not inherited. Child signal mask is empty and SIGPIPE is default. No Tarn
callback or allocator executes in the fork child, safe with native task threads.
GNU libc 2.34+ is required by closefrom actions on initial Linux x86_64.

The consuming wait bridge and destructor retry EINTR mechanically, because
returning ownership between attempts would duplicate completion responsibility.
Linux close is never retried. Acquisition failures release temporary native fds
before returning. Successful raw PID/fd lanes are wrapped once, immediately, in
verified owned resources; this private Copy raw-handle window is bootstrap debt
shared with fs/net. Structural verification validates catalogs, signatures,
private field shapes and native capabilities before backend emission.

## Alternatives and limits

Drop-detach would permit zombies; implicit kill would surprise applications.
Wait-on-drop matches native Task and keeps completion structured. This can hang
for persistent children: explicit kill is required if that is intended policy.
Sequential pipe capture is incorrect for large dual-output children; runtime C
poll loops would move high-level logic out of Tarn. Two reader tasks reuse the
existing safe concurrency model, at the cost of two pthreads per capture.

Temporary-file capture avoids pipes but introduces filesystem effects, privacy
and cleanup concerns. A shell-string API creates ambiguity and injection hazards.
Custom env/stdin/redirection, try_wait, resource limits, pidfds/process groups,
async process I/O and portability remain separate future decisions. No package
hooks, package manager, sandbox, HTTP or Phase 16 is implemented here.
