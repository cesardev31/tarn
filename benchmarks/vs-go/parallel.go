package main

import (
	"fmt"
	"sync"
)

func work(start, end uint64) uint64 {
	var total uint64
	for i := start; i < end; i++ {
		total += (i*7 + 3) % 1000
	}
	return total
}

func main() {
	results := make([]uint64, 8)
	var wg sync.WaitGroup
	for w := 0; w < 8; w++ {
		wg.Add(1)
		go func(w int) {
			defer wg.Done()
			start := uint64(w) * 100000000
			results[w] = work(start, start+100000000)
		}(w)
	}
	wg.Wait()
	var total uint64
	for _, r := range results {
		total += r
	}
	fmt.Println(total)
}
