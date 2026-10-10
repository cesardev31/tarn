package main

import (
	"fmt"
	"strconv"
)

func main() {
	values := [8]int64{-1, -9, -10, -99, -100, -123456789, -1000000000000000000, -9223372036854775808}
	total := 0
	for round := 0; round < 1000000; round++ {
		for _, value := range values {
			text := strconv.FormatInt(value, 10)
			total += len(text)
		}
	}
	fmt.Println(total)
}
