# Tarn vs Go

Each benchmark exists twice, `name.tarn` and `name.go`, with identical
output; `run.sh` checks the outputs match and prints the median wall time of
five runs.

    cargo build --release -p tarn
    ./benchmarks/vs-go/run.sh            # all
    ./benchmarks/vs-go/run.sh strings    # one

| Benchmark | Exercises |
|---|---|
| arith | checked integer arithmetic in a loop (Go does not check overflow) |
| fib | recursive calls |
| vec | push and iteration over a large `Vec<i64>` |
| strings | `string.Builder`, number formatting, `split` |
| enums | enum construction and `match` through references |
| jsoncodec | `json.encode_list` and `json.parse` of 1000 objects |

Results and decisions: [ADR 0056](../../docs/adr/0056-native-performance-baseline.md).
