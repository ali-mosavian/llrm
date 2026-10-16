"""Which benchmarks a CI shard of the bench step measures.

    python tools/bench/shards.py of I N      the names of shard I (1-based) of N, one per line
    python tools/bench/shards.py measure     rewrite shards.toml: each benchmark's CPU seconds, measured (a few minutes)

shards.toml is each benchmark's cost, checked in so a split never depends on the host that makes it. The split is greedy, heaviest
first onto the lightest shard: the same tree always gives the same shards. Re-measure when benchmarks are added or change a lot.
"""

from __future__ import annotations

import os
import sys
import tempfile
import tomllib
import subprocess
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import benchlist  # noqa: E402

HERE = Path(__file__).resolve().parent
COSTS = HERE / "shards.toml"


def names(root: Path | None = None) -> list[str]:
    bench = (root or HERE.parents[1]) / "bench"
    return [str(one.relative_to(bench)) for one in benchlist.directories(bench)]


def costs(path: Path = COSTS) -> dict[str, float]:
    return tomllib.loads(path.read_text()) if path.is_file() else {}


def split(benchmarks: list[str], count: int, cost: dict[str, float]) -> list[list[str]]:
    """`benchmarks` in `count` shards of about equal cost. One with no cost listed (a new benchmark) counts as the dearest, so it
    cannot make a shard heavier than the table says."""
    unknown = max(cost.values(), default=1.0)
    shards: list[list[str]] = [[] for _ in range(count)]
    load = [0.0] * count
    for name in sorted(benchmarks, key=lambda n: (-cost.get(n, unknown), n)):
        at = min(range(count), key=lambda i: (load[i], i))
        shards[at].append(name)
        load[at] += cost.get(name, unknown)
    return [sorted(one) for one in shards]


def of(index: int, count: int, benchmarks: list[str] | None = None, cost: dict[str, float] | None = None) -> list[str]:
    """Shard `index` (1-based) of `count`."""
    if not 1 <= index <= count:
        raise SystemExit(f"shard {index} of {count}")
    return split(benchmarks if benchmarks is not None else names(), count, cost if cost is not None else costs())[index - 1]


def cpu_seconds(name: str, work: Path) -> float:
    """CPU seconds (user+system, children included) one `bench.py NAME --no-judge` run takes: what a runner pays, whatever else the host is doing."""
    command = ["uv", "run", "-q", "--project", str(HERE.parent), "python", str(HERE / "bench.py"), name, "--no-judge", "--work", str(work / name.replace("/", "_"))]
    child = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    _, status, usage = os.wait4(child.pid, 0)
    if status:  # a run that failed early costs nothing and would be a table of zeros
        raise SystemExit(f"shards: {name} did not measure: {child.stderr.read()[-300:]}")
    return usage.ru_utime + usage.ru_stime


def table(seconds: dict[str, float]) -> str:
    """shards.toml's text."""
    return "".join(f'"{name}" = {seconds[name]:.1f}\n' for name in sorted(seconds))


def measure(work: Path, jobs: int = 8) -> str:
    """Each benchmark's cost, measured: `jobs` at a time (CPU time does not stretch with the load)."""
    found = names()
    with ThreadPoolExecutor(jobs) as pool:
        return table(dict(zip(found, pool.map(lambda name: cpu_seconds(name, work), found))))


if __name__ == "__main__":
    if sys.argv[1:2] == ["of"]:
        print(*of(int(sys.argv[2]), int(sys.argv[3])), sep="\n")
    elif sys.argv[1:2] == ["measure"]:
        with tempfile.TemporaryDirectory(prefix="shards-", dir=os.environ.get("CARGO_TARGET_DIR")) as tmp:
            COSTS.write_text(measure(Path(tmp)))
    else:
        raise SystemExit(__doc__)
