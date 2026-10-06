// Go reference for evidence/wc/wc.tarn.
package main

import (
	"fmt"
	"strings"
)

type Counts struct{ lines, words, bytes uint64 }

func isSpace(b byte) bool { return b == ' ' || b == '\n' || b == '\t' }

func count(text []byte, totals *Counts) {
	inWord := false
	for _, b := range text {
		if b == '\n' {
			totals.lines++
		}
		if isSpace(b) {
			inWord = false
		} else if !inWord {
			inWord = true
			totals.words++
		}
	}
	totals.bytes += uint64(len(text))
}

func fib(n int64) int64 {
	if n < 2 {
		return n
	}
	return fib(n-1) + fib(n-2)
}

func main() {
	text := []byte(strings.Repeat("the quick brown fox jumps over the lazy dog\nlorem ipsum dolor sit amet\n", 64))[:4096]
	var totals Counts
	for round := 0; round < 20000; round++ {
		count(text, &totals)
	}
	fmt.Println(totals.lines)
	fmt.Println(totals.words)
	fmt.Println(totals.bytes)
	fmt.Println(fib(32))
}
