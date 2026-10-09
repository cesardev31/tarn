// Equivalent Linux collector workload: /proc/stat, stat, statm and meminfo.
// No smaps_rollup, GUI, polling delay or PSS; percentages use aggregate CPU ticks.
package main

import (
	"fmt"
	"os"
	"sort"
	"strconv"
	"strings"
	"time"
)

type cpu struct{ total, idle uint64 }
type proc struct {
	pid, ticks, start, rss uint64
	name                   string
}
type usage struct {
	name string
	pid  uint64
	cpu  float64
	rss  uint64
}

func field(s string, i int) uint64 {
	f := strings.Fields(s)
	if i >= len(f) {
		return 0
	}
	n, _ := strconv.ParseUint(f[i], 10, 64)
	return n
}
func readCPU() (cpu, error) {
	b, e := os.ReadFile("/proc/stat")
	if e != nil {
		return cpu{}, e
	}
	line := strings.Split(string(b), "\n")[0]
	var total uint64
	for i := 1; i < 9; i++ {
		total += field(line, i)
	}
	return cpu{total, field(line, 4) + field(line, 5)}, nil
}
func sample() ([]proc, error) {
	entries, e := os.ReadDir("/proc")
	if e != nil {
		return nil, e
	}
	out := []proc{}
	for _, entry := range entries {
		pid, e := strconv.ParseUint(entry.Name(), 10, 64)
		if e != nil {
			continue
		}
		dir := "/proc/" + entry.Name()
		b, e := os.ReadFile(dir + "/stat")
		if e != nil {
			continue
		}
		s := string(b)
		a, z := strings.Index(s, "("), strings.LastIndex(s, ")")
		if a < 0 || z <= a || z+2 > len(s) {
			continue
		}
		rest := s[z+2:]
		m, e := os.ReadFile(dir + "/statm")
		if e != nil {
			continue
		}
		out = append(out, proc{pid, field(rest, 11) + field(rest, 12), field(rest, 19), field(string(m), 1), s[a+1 : z]})
	}
	return out, nil
}
func run() error {
	prevCPU, e := readCPU()
	if e != nil {
		return e
	}
	first, e := sample()
	if e != nil {
		return e
	}
	prev := map[uint64]proc{}
	for _, p := range first {
		prev[p.pid] = p
	}
	began := time.Now()
	var last []usage
	var cpuNow float64
	for round := 0; round < 20; round++ {
		cur, e := readCPU()
		if e != nil {
			return e
		}
		var delta uint64
		if cur.total >= prevCPU.total {
			delta = cur.total - prevCPU.total
		}
		cpuNow = 0
		if delta > 0 && cur.idle >= prevCPU.idle && cur.idle-prevCPU.idle <= delta {
			cpuNow = float64(delta-(cur.idle-prevCPU.idle)) / float64(delta) * 100
		}
		procs, e := sample()
		if e != nil {
			return e
		}
		last = []usage{}
		next := map[uint64]proc{}
		for _, p := range procs {
			pct := 0.0
			if b, ok := prev[p.pid]; ok && delta > 0 && p.start == b.start && p.ticks >= b.ticks {
				pct = float64(p.ticks-b.ticks) / float64(delta) * 100
			}
			last = append(last, usage{p.name, p.pid, pct, p.rss * uint64(os.Getpagesize()) / 1048576})
			next[p.pid] = p
		}
		prevCPU = cur
		prev = next
	}
	spent := time.Since(began)
	sort.Slice(last, func(i, j int) bool { return last[i].cpu > last[j].cpu })
	b, e := os.ReadFile("/proc/meminfo")
	if e != nil {
		return e
	}
	var total, avail uint64
	for _, line := range strings.Split(string(b), "\n") {
		if strings.HasPrefix(line, "MemTotal:") {
			total = field(line, 1)
		}
		if strings.HasPrefix(line, "MemAvailable:") {
			avail = field(line, 1)
		}
	}
	if avail > total {
		return fmt.Errorf("invalid memory counters")
	}
	mem := 0.0
	if total > 0 {
		mem = float64(total-avail) / float64(total) * 100
	}
	fmt.Println("cpu%", cpuNow, "mem%", mem, "processes", len(last), "ms per tick", spent.Milliseconds()/20)
	return nil
}
func main() {
	if e := run(); e != nil {
		fmt.Fprintln(os.Stderr, e)
		os.Exit(1)
	}
}
