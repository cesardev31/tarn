package main

import (
	"net/http"
	"sync"
)

func main() {
	var mu sync.Mutex
	var hits uint64
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		mu.Lock()
		hits++
		mu.Unlock()
		w.Header().Set("Content-Type", "text/plain; charset=utf-8")
		w.Write([]byte("ok"))
	})
	http.ListenAndServe("127.0.0.1:18092", nil)
}
