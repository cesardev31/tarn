# Status-device collector evidence

This ports the Linux collector workload, not the desktop app. Tarn and Go read
`/proc/stat`, numeric process directories, each process `stat`/`statm`, and
`/proc/meminfo`. Neither reads `smaps_rollup`, PSS, D-Bus or GUI state.

CPU percentages use aggregate machine ticks; a fully busy single logical CPU
on an N-CPU machine contributes approximately 100/N percent. These are not the
single-core percentages displayed by some process monitors. Counter resets,
idle deltas larger than total deltas and reused PID/starttime pairs contribute
zero. RSS uses the runtime page size. Reads racing process exit are skipped.

Twenty rounds run without a polling delay to measure collection cost. This is
not a production monitoring cadence. Use time.sleep_blocking between samples
in a monitoring application; do not include that sleep in collection timing.
Starttime distinguishes identity, but /proc is still a live, non-atomic source.
Malformed integer fields currently default to zero, matching the equivalent Go
workload; stronger field validation is separate from benchmarking.

Build:

```sh
tarn build collector.tarn -o collector-tarn
go build -o collector-go collector.go
```

`comparison-exploratory.json` records five runs per implementation on the real
host: roughly 399 processes. Another compiler test suite was running, so these
numbers are exploratory and must not establish a general language speed claim.
The isolated backend test `collector_handles_resets_pid_reuse_and_parentheses`
checks deterministic data rather than relying on host process activity.

For measurements without concurrent compiler/test activity:

```sh
python3 compare.py ./collector-tarn ./collector-go --runs 7 --output comparison.json
```

The harness warms up each executable and alternates measured ordering.
Whole-process timing includes startup and final reporting. Inspect process
counts in each sample: a live /proc workload cannot promise identical snapshots.

The final `comparison.json` contains seven measured runs in alternating order,
with one warmup each, after compiler tests finished. Median whole-process times
for 20 rounds were 0.4032 seconds (Tarn) and 0.4044 seconds (Go): similar for
this workload and sample, without a language-wide performance conclusion.
