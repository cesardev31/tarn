# Release rerun and bottleneck analysis — 2026-10-09

Subsequent implementation: [string split optimization](string-optimization-2026-10-09.md)
reduced the ranges benchmark time by 26% in an alternating nine-run comparison.
The tables below preserve the pre-optimization release baseline.
The next [Builder optimization](builder-optimization-2026-10-09.md) reduced
isolated small-integer text construction time by another 10.6%.
The subsequent [signed formatting optimization](signed-format-optimization-2026-10-09.md)
reduced negative formatting time by 22.5% and halved its conversion allocations.

`cargo build --release -p tarn --offline` succeeded (Cargo reused up-to-date
release artifacts). Installed with `install -Dm755 target/release/tarn
/home/cesar/.local/bin/tarn`. Installed and build hashes match:
`9c99f5a8ab228f86f8b9bc093c2d862c5a34ae538563185e8125daa8f724717a`.
The previous installed hash was
`a5555e89196da696718a8bc61d10a357d4eae7e423f2986f4abd9d6957c885ff`.
Both report version 0.0.1; the version alone did not identify the stale binary.
Go is go1.26.0 linux/amd64. Compiler optimization is Cranelift speed and C runtime
-O2. No compiler/runtime implementation was changed for this investigation.

## Current benchmark configurations

Median of five executions in seconds; every Tarn/Go output matches. The full
compatibility suite was run first, then vec/enums/string-ranges were restored to
the supported current APIs and rerun. This table uses the restored results.

| Case | Release Tarn | Go | Tarn / Go |
|---|---:|---:|---:|
| arith | 0.65 | 0.63 | 1.0x |
| fib | 0.08 | 0.07 | 1.1x |
| vec, both preallocated | 0.48 | 0.25 | 1.9x |
| strings | 0.52 | 0.26 | 2.0x |
| strings-ranges, stdlib API | 0.47 | 0.22 | 2.1x |
| enums, both preallocated | 0.26 | 0.19 | 1.4x |
| jsoncodec | 0.26 | 0.29 | 0.9x |
| parallel | 0.59 | 0.44 | 1.3x |
| tasks | 0.08 | 0.02 | 4.0x |

Commands: `./benchmarks/vs-go/run.sh`, followed by
`./benchmarks/vs-go/run.sh vec enums strings-ranges` after restoring those APIs.
No benchmark runner was edited while it was executing.

## Same-source comparison with the outdated installed compiler

This first release run retained the earlier compatibility sources. It isolates
the compiler/runtime replacement rather than changing reservation or HTTP policy.

| Case | Old installed Tarn s | Release Tarn s |
|---|---:|---:|
| arith | 2.85 | 0.65 |
| fib | 0.09 | 0.08 |
| vec, no reservation | 0.41 | 0.26 |
| strings | 3.04 | 0.52 |
| strings-ranges, local ASCII helper | 2.24 | 0.33 |
| enums, no reservation | 0.29 | 0.24 |
| jsoncodec | 1.10 | 0.26 |
| parallel | 2.21 | 0.59 |
| tasks | 0.68 | 0.08 |

The older run is not a controlled historical regression measurement. Its missing
APIs and different binary hash confirm that it did not represent this checkout's
current compiler. Restoring vector reservation especially benefits Go: the large
unreserved-vector win is not evidence of superior Tarn loop code generation.

## Bottleneck evidence

Callgrind instruction profiles used release-compiled copies with fewer iterations:
strings 2,000 rather than 200,000 rounds, vec 200,000 rather than 20 million elements.
Strace used 125 rather than 1,250 task batches (1,000 tasks). Profiling changes
execution behavior; these percentages are not wall-clock benchmark percentages.
Raw profiles remain under `/tmp/tarn-release-profile/`.

1. **Strings: scanning, copying and ownership costs.** Callgrind recorded
   39,929,759 instructions. `split` accounted for 21.85% directly and 43.88%
   including callees; `tarn_rt_vec_extend` accounted for 9.90% directly and 21.30%
   inclusive. Direct malloc/free/realloc-family entries shown in the profile total
   about 23%; memcpy entries about 7.6%. UTF-8 validation accounts for 4.55%
   directly. Inclusive values overlap and must not be summed. Tarn split returns
   owned strings, while Go strings.Split shares the original string storage.
   The standard-library ranges implementation remains around 2x here: its cursor
   validates boundaries and scans repeatedly through next_split. The specialized
   ASCII helper measured 0.33 s versus the general stdlib API's 0.47 s in separate
   runs. This suggests cursor/general-purpose overhead also matters; it is not a
   controlled isolation of an individual check.
2. **Small tasks: synchronization overhead.** Strace observed only 9 clone3
   calls for 1,000 tasks: native threads are being reused. It observed 7,223 futex
   calls; futex accounts for 99.53% of accumulated syscall time under tracing.
   This is syscall time across threads, including waiting, not 99.53% of program
   wall time. Runtime inspection confirms the global cache mutex/condition and
   per-task mutex/condition, plus two task/result allocations. The remaining
   latency is consistent with synchronization and bookkeeping for tiny work,
   rather than creating a new OS thread for every task.
3. **Preallocated vectors: generated loop code.** Callgrind places 98.71% of
   14,790,512 instructions in the compiled main function; allocator/runtime
   functions are not dominant. The IR still has checked accumulator addition,
   and push lowering checks capacity and updates the vector header. Precise
   attribution among arithmetic checks, memory access and generated code quality
   requires an additional isolated experiment; the profile alone cannot decide.
4. **Parallel arithmetic: unproven parameter ranges.** Current IR proves the
   main arithmetic benchmark's multiply and increment safe (`mul_proven`,
   `add_proven`). The parameterized parallel work retains checked multiply and
   accumulator addition. This is a concrete difference in generated semantics
   consistent with the 1.3x gap; its exact timing contribution is not isolated.

## HTTP, restored parallel server

`./benchmarks/vs-go/http/run.sh`: 100,000 fresh connections, 64 clients, eight
Tarn workers, concurrent Go net/http. All requests succeeded. Local socket access
was granted outside the sandbox. Worker/scheduler configurations differ.

| Server | Requests/s | p50 ms | p99 ms | Failed | User + system CPU s | Peak RSS KiB |
|---|---:|---:|---:|---:|---:|---:|
| Tarn | 8520 | 6.394 | 31.049 | 0 | 10.34 + 8.67 | 2264 |
| Go | 11510 | 4.852 | 17.377 | 0 | 6.07 + 8.40 | 15964 |

This run does not isolate the remaining HTTP bottleneck; parser, allocation and
worker contention are candidates requiring a server-specific profile.

## Native fixtures

All four fixtures passed five output-checked executions using
`python3 benchmarks/native/run.py`. This table is the rerun after HTTP completed;
an earlier native pass overlapped HTTP and was excluded.

| Fixture | Complete build ms | ELF bytes | Runtime median ms |
|---|---:|---:|---:|
| arithmetic | 2316.820 | 73120 | 4.647 |
| calls | 2046.738 | 73216 | 3.725 |
| slices | 1315.958 | 73208 | 2.652 |
| dynamic | 1146.898 | 73344 | 4.884 |

The desktop was not CPU-isolated. Tiny process-startup benchmarks, 0.01-second
shell timing precision and scheduling noise limit numerical comparisons.
Frontend checks, format checks, shell syntax and diff whitespace checks passed.
A full workspace test suite was not run for this build/install/profiling task.

## Next optimization priorities

Profile-guided string scanning/Builder allocation improvements and small-task
synchronization reduction are the strongest evidence-backed targets. Preserve
owned substring semantics and safe native-task completion. For vectors, isolate
generated loop/check costs before selecting an optimization. The installation
mismatch was the main explanation for the alarming earlier measurements.
