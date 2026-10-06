# Evidence report: real programs after Phase 13

Purpose: measure Tarn against its stated goals (application code as concise as
Go, native performance) with real programs before adding more infrastructure.
Programs and Go references live in [`evidence/`](../evidence). Measurements:
Linux x86_64, Tarn debug compiler (Cranelift `opt_level=none`), Go default build.

## Programs

| Program | What it exercises | Tarn lines | Go lines |
|---|---|---|---|
| `kv/server` | concurrent TCP connections, shared atomic, tasks | 25 | 37 |
| `wc/wc` | byte scanning loop, struct mutation, recursion | 37 | 39 |
| `json/json` | recursive descent, `Result`/`try`, slices, enums | 115 | 180 |

(Non-blank, non-comment lines.) All three produce identical output in both languages.

## Performance

| Benchmark | Tarn | Go | Ratio |
|---|---|---|---|
| `wc` (20k × 4 KiB scan + `fib(32)`) | 0.61 s | 0.28 s | 2.2× |
| `json` (200k parses) | 1.41 s | 0.09 s | 15.7× |
| peak RSS | 2.1–2.6 MB | 2.1–2.2 MB | ≈ |
| binary size | 80–350 KB | 2.4 MB | 7–30× smaller |

### After the backend copy fix

Aggregate copies now use 8/4/2/1-byte chunks (still loading every chunk before
storing, so overlapping assignments stay safe), and struct/enum/array values are
built directly in their destination when no aggregate operand can share its
storage. No semantic change.

| Benchmark | Before | After | Go |
|---|---|---|---|
| `json` | 1.41 s | 0.51 s (5.7×) | 0.09 s |
| `wc` | 0.61 s | 0.60 s | 0.28 s |
| `Result<u8, E>` microbenchmark | 1.25 s | 0.79 s | — |

Cranelift `opt_level=speed` was measured and gave no gain, so it was not
enabled. The remaining gap is optimizer work, not copies: Go inlines the small
parser helpers, while Tarn calls them and checks every arithmetic overflow and
index. Inlining small functions is the next measurable lever.

### Original diagnosis

Root cause of the `json` gap, measured with a microbenchmark (50M calls):
returning `u8` takes 0.48 s, returning `Result<u8, E>` takes 1.25 s. Generated
code copies aggregates **byte by byte**, and the same 12-byte value is copied
three times (temporary → local → return slot). Fix candidates, all mechanical
backend work: word-sized copies, and eliding temporary-to-destination copies.

## Ergonomics findings (ordered by impact)

1. **No standard library for ordinary programs.** `stdlib/fs`, `path` and
   `process` are empty (`examples/23_filesystem.tarn` uses APIs that do not
   exist); there is no stdin, no growable vector, no map, and no byte/char access
   to `string`. A CLI tool, a real JSON library over strings, or a key-value store
   cannot be written today. Every program above works on fixed byte arrays.
2. **Unbounded concurrency is impossible.** A discarded `spawn` joins at the end
   of its statement, so an accept loop serves clients one at a time (verified:
   a second client blocks while the first stays connected). Concurrency needs a
   fixed `[N]Option<Task<_>>`, so the number of connections must be known in
   advance. Async has no spawn either. An HTTP server is blocked on this.
3. **Array ceremony.** Fixed arrays are capped at 4096 elements and literals must
   list every element (a 64-byte zero buffer is 64 written zeros). There is no
   repeat initializer.
4. **No byte/char literals.** Parsers write `u8(34)` for `"` and `u8(123)` for
   `{`; the JSON parser is harder to read than its Go counterpart for this
   reason alone.
5. **Task errors in stored handles are dropped silently.** Handles joined by
   destruction discard their `Result`; Go's version at least shows the choice.
6. **Sharing with tasks.** `move fn` capturing `&total` inside a loop moves the
   atomic itself; the working form is `shared := &total` plus `scope { ... }`.
   The diagnostics (E4001 "moved in a previous iteration", E4206) led there
   directly, but Go needs neither step.

## What worked well

- `try` makes error propagation shorter than Go in every program (json: 115 vs
  180 lines, almost all of the difference is `if err != nil`).
- Recursive descent, mutual recursion, enums with payloads, `&mut` parser state
  and slices compiled on the first attempt, with no lifetime annotations.
- Diagnostics were actionable in every failure met (including a useful shadowing
  warning). Memory use matches Go; binaries are an order of magnitude smaller.

## Recommended next steps (by evidence)

1. Done: backend copy fix (15.7× → 5.7× on `json`). Next lever: inlining of
   small functions, measured before adopting.
2. A minimal collections and I/O phase: growable `Vec<T>`, byte access to
   `string`, byte literals, stdin/stdout and basic `fs` (read/write a file).
3. Unbounded task ownership: a way to own N task handles (needs `Vec`) and/or
   executor-level spawning of async computations, before any HTTP work.
4. Repeat-initialized arrays (`[N]u8` filled with a value).

Each item came from a concrete failure above; none is copied from another
language because it exists there.
