# Tarn vs Go

Each benchmark exists twice, `name.tarn` and `name.go`, with identical
output; `run.sh` checks the outputs match and prints the median wall time of
five runs.

    ./benchmarks/vs-go/run.sh            # all
    ./benchmarks/vs-go/run.sh strings    # one

The runner uses `tarn` from PATH, including its embedded standard library.
Override it with `TARN=/absolute/path/to/tarn`. It does not rebuild the compiler.
Temporary executables are removed on success or failure.

| Benchmark | Exercises |
|---|---|
| arith | checked integer arithmetic in a loop (Go does not check overflow) |
| fib | recursive calls |
| vec | push and iteration over a large `Vec<i64>` |
| builder | text construction and number formatting, without splitting |
| format-i64 | owned decimal formatting of negative integers, including i64 minimum |
| strings | `string.Builder`, number formatting, `split` |
| enums | enum construction and `match` through references |
| jsoncodec | `json.encode_list` and `json.parse` of 1000 objects |
| parallel | native tasks performing arithmetic |
| tasks | creation and joining of native tasks |

Vectors reserve the same capacity in both languages. This requires a current
release compiler; compatibility runs without reservation are separate results.

Results and decisions: [ADR 0056](../../docs/adr/0056-native-performance-baseline.md).

`./run.sh strings strings-ranges` also compares offset-based split results with
Go's existing strings.Split. Both versions build identical text and produce the
same count; the original Tarn benchmark retains owned copies, while the ranges
variant retains only offsets through `string.split_ranges`.
This distinguishes representation cost from the
Builder improvement. Neither is evidence of Unicode/grapheme processing speed.

`./benchmarks/vs-go/http/run.sh` separately measures HTTP throughput and server
CPU/RSS (100,000 requests, 64 clients; configurable with REQUESTS and CLIENTS).
Tarn uses `http.serve_parallel` with eight workers; Go uses concurrent `net/http`.
Each request opens a new connection. The worker/scheduling models still differ.
