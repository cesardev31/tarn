# ADR 0039: monotonic timers and timeouts (Phase 14B)

Status: accepted for Phase 14B. Timers reuse the Phase-12B readiness Poll and
the Phase-12C/13 Waker model; there is no timer thread and no sleep.

## Duration and Timer

`net.Duration` is a Copy millisecond span (`milliseconds`, `seconds`,
`as_millis`); `seconds` overflow aborts. `net.Timer` owns a one-shot Linux
`timerfd` on `CLOCK_MONOTONIC`, nonblocking and close-on-exec. It is an
fd-owning resource like the sockets: movable (Transfer, not Share) and closed
exactly once by its owner's verified destruction. A zero duration fires at once.

`poll_timer` follows the canonical readiness protocol: read, arm the current
Waker after WouldBlock, retry, return Pending only after another WouldBlock.
`Timer.wait_async()` and `sleep(duration)` are ordinary async functions in Tarn.

Readiness registration protects against descriptor reuse with SO_COOKIE, which
timers lack, and all timerfds share one anonymous inode. Each live timer
therefore gets a process-unique identity (high bit set, never a socket cookie),
recorded at creation and removed before close, under a mutex because native
threads may own timers.

## Timeouts

`await task.join_timeout(duration)` returns `Ok(Some(R))` on completion and
`Ok(None)` on timeout. The operation is an `AsyncTask` with its own Waker, so the
timer's registration on the joiner's Waker never conflicts with the operation's
I/O registration.

Race rule: every poll checks completion before the timer. When both are ready
in the same poll, the result wins. The decision happens once in one poll on one
thread, so a result is never produced twice and both outcomes never occur. On
timeout the handle is dropped: the pending task is abandoned through its
verified frame destruction (no cancellation API, no detaching). A completed
result releases the timer's registration before returning.

## Limits

One-shot timers only; no intervals, deadlines/instants, wall clock or public
select/race. Timer storage stays in `net` with the executor (module split is
recorded debt).
