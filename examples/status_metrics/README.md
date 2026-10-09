# status_device network metrics port

Run from the Tarn repository:

```sh
cargo build -p tarn
target/debug/tarn run examples/status_metrics/main.tarn
```

This partial port takes two read-only /proc/net/dev snapshots 100 ms apart and
prints JSON counters/rates with a UTC timestamp. It exercises Map, comparator
sorting, ASCII fields, clocks, console output and the existing async timer.
The source reference is status_device/internal/metrics/system.go in the sibling
status_device project. That project is not modified.

Scope and differences:

- Only network counters/rates are ported; no tray, GUI, CPU, disk or process UI.
- Headers, malformed records, empty interface names and loopback are skipped.
- Counter resets and new interfaces yield zero rates.
- Results sort by name rather than by aggregate activity as in the Go UI.
- Millisecond intervals replace float seconds; zero intervals use 1 ms.
- Rates use f64 scaling and saturate at u64's upper boundary.
- JSON output includes raw counter values and per-second rates. No float formatter
  is needed by this subset.
- Reading procfs and timers can be restricted in a sandbox. This does not imply
  application failure on the supported native platform.

The deterministic backend fixture covers parsing, ordering, counter reset and
new-interface handling independently of live traffic. This is evidence for the
stdlib additions, not a complete status_device port or performance comparison.
