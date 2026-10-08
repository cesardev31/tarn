package main

import "fmt"

func main() {
	var total uint64
	for i := uint64(0); i < 300000000; i++ {
		total += (i*7 + 3) % 1000
	}
	fmt.Println(total)
}
