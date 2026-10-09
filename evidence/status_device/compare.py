#!/usr/bin/env python3
"""Alternate equivalent collectors; report workload and whole-process timing."""
import argparse
import json
import platform
import statistics
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("tarn_binary", type=Path)
parser.add_argument("go_binary", type=Path)
parser.add_argument("--runs", type=int, default=7)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
if args.runs < 3:
    parser.error("at least three measured runs are required")
binaries = {"tarn": args.tarn_binary.resolve(), "go": args.go_binary.resolve()}
for binary in binaries.values():
    subprocess.run([str(binary)], check=True, capture_output=True, timeout=60)
samples = {name: [] for name in binaries}
for iteration in range(args.runs):
    order = list(binaries) if iteration % 2 == 0 else list(reversed(binaries))
    for name in order:
        began = time.perf_counter()
        result = subprocess.run([str(binaries[name])], check=True, capture_output=True, text=True, timeout=60)
        samples[name].append({"seconds": time.perf_counter() - began, "stdout": result.stdout})
report = {
    "schema_version": 1,
    "platform": platform.system() + " " + platform.machine(),
    "workload": "20 /proc rounds; stat, statm and meminfo; no smaps_rollup or sleep",
    "method": "one warmup each; alternating order; whole process wall time",
    "runs": args.runs,
    "results": {name: {"median_seconds": statistics.median(item["seconds"] for item in values), "samples": values} for name, values in samples.items()},
}
args.output.write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps({name: value["median_seconds"] for name, value in report["results"].items()}))
