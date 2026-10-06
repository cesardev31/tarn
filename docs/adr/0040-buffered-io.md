# ADR 0040: buffered async I/O (Phase 14C)

Status: accepted for Phase 14C. Buffered I/O is policy written in Tarn over
the verified `read_async`/`write_async` primitives. It is not a new native I/O
subsystem, and it adds no runtime code.

## Storage

`BufferedReader` and `BufferedWriter` own a `Vec<u8>` plus `start`/`end`
offsets.

Two compiler/runtime additions were needed: `Vec<T>.as_slice` and
`Vec<T>.as_mut_slice`. These are mechanical intrinsics that produce a
slice of the vector's elements. The slice is an ordinary loan of the vector, so
pushing while it is live is E4101 (`E4101_vec_slice_then_push`).

`ErrorKind` gains `UnexpectedEof` and `LimitExceeded`.

## The stream is a parameter

Each call takes `stream &mut TcpStream`, for example
`reader.read_until(&mut stream, delimiter, max)`.

v0 structs cannot hold references, so the alternatives were:

- **A wrapper that owns the stream.** One stream could not then have a reader
  and a writer at once (an echo needs both).
- **Splitting the stream with `dup`.** This needs a new intrinsic and two owners
  of one connection.

The parameter form needs neither. It leaves the stream usable directly between
calls. It also keeps the type usable for other sources once Tarn has an async
read interface.

The cost is that a caller could pass a different stream mid-buffer. That is a
logic error, not a memory-safety error.

## Reader

| Operation | Behavior |
|---|---|
| `read(stream, target)` | Copies from the buffer, or does at most one read when the buffer is empty. A target at least as large as the buffer bypasses it. `Ok(0)` is end of stream. |
| `read_exact(stream, target)` | Returns `UnexpectedEof` if the stream ends first. Bytes already copied into the target stay consumed. |
| `read_until(stream, delimiter, max)` | Returns an owned `Vec<u8>` that includes the delimiter, at most `max` bytes long. |

`read_until` is the delimiter-oriented API, so it always takes a limit. A longer
line returns `LimitExceeded` and its bytes stay buffered. At end of stream it
returns the remaining bytes without a delimiter, and an empty vector means the
stream had ended. Both policies are documented on the declarations.

Growth: before a read, an empty buffer resets to the front, and unread bytes are
moved to the front when there is room. The capacity doubles only when the buffer
is full of unread bytes. The `max` check bounds growth for `read_until`.

## Writer

A syscall happens only in three cases:

- the buffer is full;
- a write is at least as large as the buffer and nothing is pending;
- `flush` is called explicitly.

`write` accepts at least one byte. `write_all` loops over `write`. `flush`
continues partial writes from the exact offset, and after an error exactly the
unwritten bytes remain pending.

**Destruction discards pending bytes without I/O.** Flushing is always explicit:
`Drop` cannot suspend, and blocking inside it would stall the executor.
`pending()` lets callers check the state.

## Alternatives rejected

- **Native buffered I/O in the runtime.** It would duplicate the readiness and
  error logic.
- **Flushing on drop.** It is impossible without async drop, and it would hide
  I/O.
- **Unbounded `read_until`.** It is a denial-of-service hazard ahead of HTTP.

## Limits

- `TcpStream` only. A generic version waits for an async read/write interface.
- No `read_line` string decoding, `BufReader`-style peek or `consume`, or
  vectored writes.
- Byte copies are simple loops (no `memcpy` intrinsic). Optimize later if
  benchmarks show they matter.
