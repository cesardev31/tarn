# Phase 14B report: timers and timeouts

Design: [ADR 0039](adr/0039-timers-and-timeouts.md).

## What was implemented

- `net.Duration` (Copy, milliseconds) and `net.Timer`, a one-shot
  `timerfd` on `CLOCK_MONOTONIC`. It is owned like a socket and closed exactly
  once by its verified destruction.
- `poll_timer` uses the canonical readiness protocol. `Timer.wait_async` and
  `sleep` are ordinary Tarn async functions.
- `AsyncTask.join_timeout(duration)` returns `Ok(Some(R))` or `Ok(None)`.
  Completion is checked before the timer, so a tie goes to the result. A timed-out
  task is abandoned through its frame destruction.
- Runtime: `tarn_rt_net_timer_new` and `tarn_rt_net_timer_read`. A
  process-unique timer identity table replaces `SO_COOKIE` for readiness
  registration.

## Tests

`compiler/backend/tests/async_timers.rs` checks the following:

- sleep ordering;
- the operation winning a race;
- a timeout winning, with the losing task abandoned;
- a tie, where the result wins;
- 200 concurrent timers;
- a timer dropped without firing;
- a zero-duration timer;
- every fd is balanced;
- every owned string is destroyed exactly once.

The full workspace suite passes. The only golden change is the new
`AsyncTask.join_timeout` provenance line.

## Bugs found

`timerfd` has no `SO_COOKIE`, so registration failed with `ENOTSOCK`. All
timerfds also share one anonymous inode. The identity table solves both
problems.

## Decisions I would defend

- Timers are readiness sources on the existing Poll: no timer thread and no
  second wake mechanism.
- Completion is checked first, so each race is decided once in one poll.
- A timeout means abandonment, not a cancellation API.

## Decisions I still question

- Timer identity lives in a global mutex-protected table. Reviewed: keep it.
  Move it into Poll only if contention, cleanup or global-state complexity
  shows up in real programs.
- Timers live in `net` alongside the executor. Reviewed: this is organizational
  debt, not architectural debt, and does not block later phases.

## Known limitations

- One-shot timers only.
- No intervals, instants or deadlines, wall clock, or public select/race.
