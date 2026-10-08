package main

import "fmt"

type Shape struct {
	kind int
	a, b int64
}

func area(s *Shape) int64 {
	switch s.kind {
	case 0:
		return 3 * s.a * s.a
	case 1:
		return s.a * s.b
	}
	return 0
}

func main() {
	shapes := make([]Shape, 0, 3000000)
	for i := int64(0); i < 3000000; i++ {
		switch i % 3 {
		case 0:
			shapes = append(shapes, Shape{0, i % 50, 0})
		case 1:
			shapes = append(shapes, Shape{1, i % 30, i % 40})
		default:
			shapes = append(shapes, Shape{2, 0, 0})
		}
	}
	var total int64
	for round := 0; round < 20; round++ {
		for i := range shapes {
			total += area(&shapes[i])
		}
	}
	fmt.Println(total)
}
