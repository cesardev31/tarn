#!/usr/bin/env bash
# Parallel HTTP: Tarn http.serve_parallel (8 workers) vs Go net/http, one
# connection per request, 64 clients. Reports server CPU and peak memory.
set -euo pipefail
cd "$(dirname "$0")"
TARN=${TARN:-tarn}
out=$(mktemp -d)
server=""
cleanup() {
    if [ -n "$server" ]; then
        pkill -TERM -P "$server" 2>/dev/null || true
        wait "$server" 2>/dev/null || true
    fi
    rm -rf "$out"
}
trap cleanup EXIT
"$TARN" build server.tarn -o "$out/tarn-server" >/dev/null
go build -o "$out/go-server" server.go
go build -o "$out/load" load.go
for which in tarn go; do
    port=$([ $which = tarn ] && echo 18091 || echo 18092)
    /usr/bin/time -f "%U+%S cpu-s  %M KB peak" -o "$out/$which.usage" "$out/$which-server" & server=$!
    sleep 0.5
    "$out/load" -url "http://127.0.0.1:$port/" -c "${CLIENTS:-64}" -n "${REQUESTS:-100000}" > "$out/$which.load"
    pkill -TERM -P "$server" 2>/dev/null || true
    wait "$server" 2>/dev/null || true
    server=""
    printf "%-5s %s  server: %s\n" "$which" "$(cat "$out/$which.load")" "$(tail -1 "$out/$which.usage")"
done
rm -rf "$out"
