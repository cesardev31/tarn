package main

import (
	"fmt"
	"strconv"
	"strings"
)

func main() {
	total := 0
	for round := 0; round < 200000; round++ {
		var b strings.Builder
		for i := uint64(0); i < 20; i++ {
			b.WriteString("item-")
			b.WriteString(strconv.FormatUint(i, 10))
			b.WriteString(",")
		}
		text := b.String()
		total += len(strings.Split(text, ","))
	}
	fmt.Println(total)
}
