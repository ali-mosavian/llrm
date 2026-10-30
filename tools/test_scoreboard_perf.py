"""tools/scoreboard.py counts with `perf stat`: this test needs the counters, so the pytest-perf step runs it (REQUIRES) and a hosted gate reports it missing by name."""
import importlib.util
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("scoreboard", HERE / "scoreboard.py")
scoreboard = importlib.util.module_from_spec(spec)
sys.modules["scoreboard"] = scoreboard
spec.loader.exec_module(scoreboard)

REQUIRES = ["perf"]


def test_a_compile_that_fails_gives_no_count():
    count, why = scoreboard.instructions(["python3", "-c", "import sys; sys.stderr.write('boom'); sys.exit(1)"], ok=True)
    assert count is None and "boom" in why
