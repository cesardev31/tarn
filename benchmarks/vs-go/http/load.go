// Closed-loop load: `clients` goroutines send `total` GET requests, one new
// connection per request (keep-alive off for both servers).
package main

import (
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"sort"
	"sync"
	"sync/atomic"
	"time"
)

func main() {
	url := flag.String("url", "http://127.0.0.1:18091/", "target")
	clients := flag.Int("c", 64, "concurrent clients")
	total := flag.Int("n", 100000, "requests")
	flag.Parse()
	client := &http.Client{Transport: &http.Transport{DisableKeepAlives: true, MaxIdleConnsPerHost: -1}, Timeout: 10 * time.Second}
	var next, failed int64
	latencies := make([][]time.Duration, *clients)
	var wg sync.WaitGroup
	start := time.Now()
	for c := 0; c < *clients; c++ {
		wg.Add(1)
		go func(c int) {
			defer wg.Done()
			for atomic.AddInt64(&next, 1) <= int64(*total) {
				t := time.Now()
				resp, err := client.Get(*url)
				if err != nil || resp.StatusCode != 200 {
					atomic.AddInt64(&failed, 1)
					if resp != nil {
						resp.Body.Close()
					}
					continue
				}
				io.Copy(io.Discard, resp.Body)
				resp.Body.Close()
				latencies[c] = append(latencies[c], time.Since(t))
			}
		}(c)
	}
	wg.Wait()
	elapsed := time.Since(start)
	var all []time.Duration
	for _, l := range latencies {
		all = append(all, l...)
	}
	if len(all) == 0 {
		fmt.Printf("0 req/s  failed %d (no successful requests)\n", failed)
		os.Exit(1)
	}
	sort.Slice(all, func(i, j int) bool { return all[i] < all[j] })
	p := func(q float64) time.Duration { return all[int(float64(len(all)-1)*q)] }
	fmt.Printf("%.0f req/s  p50 %v  p99 %v  failed %d\n", float64(len(all))/elapsed.Seconds(), p(0.5), p(0.99), failed)
}
