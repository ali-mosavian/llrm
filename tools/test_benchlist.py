"""Which directories are benchmarks (tools/benchlist.py): those holding a bench.toml."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import benchlist  # noqa: E402


def tree(tmp_path: Path) -> Path:
    for name in ("fib", "sieve", "parity/branch"):
        (tmp_path / "bench" / name).mkdir(parents=True)
        (tmp_path / "bench" / name / "bench.toml").write_text("repetitions = 1\n")
        (tmp_path / "bench" / name / f"{Path(name).name}.c").write_text("int x;\n")
    (tmp_path / "bench/waivers").mkdir()  # not a program: #1820's waivers were listed as a benchmark called `waivers`
    (tmp_path / "bench/waivers/w.toml").write_text("reason = 'r'\n")
    (tmp_path / "bench/bench_io.h").write_text("")
    return tmp_path


def test_a_directory_with_no_bench_toml_is_not_a_benchmark_and_parity_is_asked_for(tmp_path):
    root = tree(tmp_path)
    names = lambda dirs: [str(one.relative_to(root / "bench")) for one in dirs]  # noqa: E731
    assert names(benchlist.directories(root / "bench")) == ["fib", "sieve", "parity/branch"]
    assert names(benchlist.directories(root / "bench", parity=False)) == ["fib", "sieve"]
    assert [one.name for one in benchlist.sources(".c", root / "bench", parity=False)] == ["fib.c", "sieve.c"]


def test_the_tools_that_list_the_programs_ask_it_and_survive_a_directory_that_is_not_one(tmp_path, monkeypatch):
    """vsgcc's programs.sources() read bench/waivers/waivers.c: FileNotFoundError."""
    root = tree(tmp_path)
    vsgcc = next((benchlist.ROOT / "crates/target").glob("*/vsgcc"))
    sys.path.insert(0, str(vsgcc))
    import programs

    monkeypatch.setattr(programs, "REPO", root)
    assert "waivers" not in programs.sources() and {"fib", "sieve"} <= set(programs.sources())
