package main

import "fmt"

func main() {
	data := make([]int64, 0, 20000000)
	for i := int64(0); i < 20000000; i++ {
		data = append(data, i%977)
	}
	var total int64
	for round := 0; round < 5; round++ {
		for _, value := range data {
			total += value
		}
	}
	fmt.Println(total)
}
