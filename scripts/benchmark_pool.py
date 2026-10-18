"""Pool under load: throughput, latency and memory with 1, 2, 4 and 8 workers.

Informational, never a CI gate. Each configuration starts a fresh pool, warms
it, then submits jobs from client threads as fast as results come back.
"""

import concurrent.futures
import hashlib
import json
import os
import platform
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BINARY = Path(os.environ.get("PYNOX_BIN", ROOT / "target/release/pynox")).resolve()
sys.path.insert(0, str(ROOT / "python"))
os.environ["PYNOX_BIN"] = str(BINARY)

import pynox  # noqa: E402

WORKLOADS = {
    "tiny": "print(1)",
    "cpu_20ms": "print(sum(i * i for i in range(300000)))",
    "json_64k": "import json\nprint(len(json.dumps([{'k': i, 'v': 'x' * 20} for i in range(2000)])))",
}
JOBS = 400


def tree_rss_kib(pid):
    """Resident memory of a process and its descendants, from /proc."""
    children = {}
    for entry in Path("/proc").iterdir():
        if entry.name.isdigit():
            try:
                stat = (entry / "stat").read_text()
                parent = int(stat.rsplit(")", 1)[1].split()[1])
                children.setdefault(parent, []).append(int(entry.name))
            except (OSError, IndexError, ValueError):
                pass
    total, stack = 0, [pid]
    while stack:
        current = stack.pop()
        try:
            for line in Path(f"/proc/{current}/status").read_text().splitlines():
                if line.startswith("VmRSS:"):
                    total += int(line.split()[1])
        except OSError:
            pass
        stack.extend(children.get(current, []))
    return total


def measure(workers, code):
    with pynox.Pool(workers=workers) as pool:
        for _ in range(workers * 4):
            assert pool.run(code).ok
        idle_rss = tree_rss_kib(pool._process.pid)
        latencies = []

        def one(_):
            start = time.perf_counter_ns()
            result = pool.run(code)
            assert result.ok, result
            return (time.perf_counter_ns() - start) / 1e6

        peak = idle_rss
        start = time.perf_counter()
        with concurrent.futures.ThreadPoolExecutor(workers * 2) as executor:
            futures = [executor.submit(one, i) for i in range(JOBS)]
            for index, future in enumerate(concurrent.futures.as_completed(futures)):
                latencies.append(future.result())
                if index % 50 == 0:
                    peak = max(peak, tree_rss_kib(pool._process.pid))
        elapsed = time.perf_counter() - start
        stats = pool.stats()
    latencies.sort()
    return {
        "workers": workers,
        "jobs": JOBS,
        "throughput_per_s": round(JOBS / elapsed, 1),
        "p50_ms": round(statistics.median(latencies), 2),
        "p95_ms": round(latencies[int(len(latencies) * 0.95) - 1], 2),
        "max_ms": round(latencies[-1], 2),
        "idle_rss_mib": round(idle_rss / 1024, 1),
        "peak_rss_mib": round(peak / 1024, 1),
        "pool_counters": {k: stats[k] for k in ("jobs", "ok", "failed", "worker_errors")},
    }


def main():
    info = json.loads(subprocess.check_output([BINARY, "doctor"]))
    results = []
    for name, code in WORKLOADS.items():
        for workers in (1, 2, 4, 8):
            row = {"workload": name, **measure(workers, code)}
            results.append(row)
            print(
                f"{name:9} workers={workers}  {row['throughput_per_s']:7.1f} jobs/s  "
                f"p50 {row['p50_ms']:6.2f} ms  p95 {row['p95_ms']:6.2f} ms  "
                f"rss idle {row['idle_rss_mib']:6.1f} MiB peak {row['peak_rss_mib']:6.1f} MiB"
            )
    report = {
        "runtime": info,
        "measured_at": datetime.now(timezone.utc).isoformat(),
        "platform": platform.platform(),
        "cpus": os.cpu_count(),
        "method": (
            f"{JOBS} jobs per configuration from 2x-workers client threads through the pynox "
            "package; warmup of 4 jobs per worker; latency is request to result as seen by the "
            "client; RSS sums the pool process tree from /proc, sampled every 50 results"
        ),
        "binary_sha256": hashlib.sha256(BINARY.read_bytes()).hexdigest(),
        "results": results,
    }
    (ROOT / "dist").mkdir(exist_ok=True)
    destination = ROOT / "dist/benchmark-pool.json"
    destination.write_text(json.dumps(report, indent=2) + "\n")
    print(destination)


if __name__ == "__main__":
    main()
