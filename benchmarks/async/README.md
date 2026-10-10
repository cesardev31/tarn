# Cooperative executor scan benchmark

`tarn run benchmarks/async/executor-scan.tarn` creates 1,024 dormant Operations
and one self-waking Operation, then executes 20,000 nonblocking fair turns. It
prints 1025 and destroys every pending operation on normal return.

For execution timing, build once and time the executable separately:

```
tarn build benchmarks/async/executor-scan.tarn -o /tmp/tarn-executor-scan
/usr/bin/time /tmp/tarn-executor-scan
```

The workload isolates scanning and wake bookkeeping when few operations are
runnable. It does not measure HTTP throughput, socket readiness lookup or useful
application work. A completely idle executor sleeps in normal applications;
this intentionally self-waking operation keeps turns running for measurement.
Vary the dormant count (1024) to inspect scaling. No new executor API is required.
