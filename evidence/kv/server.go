// Concurrent counter server (Go reference for evidence/kv/server.tarn).
package main

import (
	"fmt"
	"net"
	"sync"
	"sync/atomic"
)

func serve(conn net.Conn, total *atomic.Uint64) {
	defer conn.Close()
	buffer := make([]byte, 64)
	for {
		count, err := conn.Read(buffer)
		if count == 0 || err != nil {
			return
		}
		now := total.Add(uint64(count))
		conn.Write([]byte{byte(now % 256)})
	}
}

func main() {
	listener, err := net.Listen("tcp", "127.0.0.1:7878")
	if err != nil {
		panic(err)
	}
	var total atomic.Uint64
	var workers sync.WaitGroup
	for i := 0; i < 4; i++ {
		conn, err := listener.Accept()
		if err != nil {
			panic(err)
		}
		workers.Add(1)
		go func() { defer workers.Done(); serve(conn, &total) }()
	}
	workers.Wait()
	fmt.Println(total.Load())
}
