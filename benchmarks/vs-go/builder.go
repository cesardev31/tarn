package main

import (
	"fmt"
	"strconv"
	"strings"
)

func main() {
	total := 0
	for round := 0; round < 200000; round++ {
		var builder strings.Builder
		for i := uint64(0); i < 20; i++ {
			builder.WriteString("item-")
			builder.WriteString(strconv.FormatUint(i, 10))
			builder.WriteString(",")
		}
		total += len(builder.String())
	}
	fmt.Println(total)
}
