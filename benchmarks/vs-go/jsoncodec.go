package main

import (
	"encoding/json"
	"fmt"
)

type Item struct {
	ID   uint64 `json:"id"`
	Name string `json:"name"`
}

func main() {
	var items []Item
	for i := uint64(0); i < 1000; i++ {
		items = append(items, Item{i, "producto número " + "x"})
	}
	total := 0
	for round := 0; round < 200; round++ {
		text, _ := json.Marshal(items)
		var value []Item
		if json.Unmarshal(text, &value) == nil {
			total += len(value) + len(text)
		}
	}
	fmt.Println(total)
}
