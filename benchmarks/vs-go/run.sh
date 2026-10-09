#!/usr/bin/env bash
# Tarn vs Go: same program, same output, median wall time of 5 runs.
set -euo pipefail
cd "$(dirname "$0")"
TARN=${TARN:-../../target/release/tarn}
out=$(mktemp -d)
median() { sort -n | awk '{a[NR]=$1} END{print a[int((NR+1)/2)]}'; }
time_of() { for _ in 1 2 3 4 5; do /usr/bin/time -f "%e" "$@" >/dev/null 2>>"$out/t"; done; tail -5 "$out/t" | median; : > "$out/t"; }
printf "%-10s %8s %8s %7s\n" bench tarn go ratio
for name in ${@:-arith fib vec strings enums jsoncodec parallel tasks}; do
    "$TARN" build "$name.tarn" -o "$out/$name-tarn" >/dev/null
    go_source="$name.go"
    if [ "$name" = "strings-ranges" ]; then go_source="strings.go"; fi
    go build -o "$out/$name-go" "$go_source"
    [ "$("$out/$name-tarn")" = "$("$out/$name-go")" ] || { echo "$name: outputs differ"; exit 1; }
    t=$(time_of "$out/$name-tarn"); g=$(time_of "$out/$name-go")
    printf "%-10s %8s %8s %7s\n" "$name" "$t" "$g" "$(awk -v t="$t" -v g="$g" 'BEGIN{ if (g>0) printf "%.1fx", t/g; else print "-" }')"
done
rm -rf "$out"
