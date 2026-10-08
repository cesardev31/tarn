package main

import (
	"fmt"
	"sync"
)

func small(n uint64) uint64 {
	var total uint64
	for i := uint64(0); i < 1000; i++ {
		total += (n + i) % 7
	}
	return total
}

func main() {
	var total uint64
	for batch := uint64(0); batch < 1250; batch++ {
		results := make([]uint64, 8)
		var wg sync.WaitGroup
		for k := uint64(0); k < 8; k++ {
			wg.Add(1)
			go func(k uint64) {
				defer wg.Done()
				results[k] = small(batch*8 + k)
			}(k)
		}
		wg.Wait()
		for _, r := range results {
			total += r
		}
	}
	fmt.Println(total)
}
