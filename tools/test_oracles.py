"""`uv run --project tools python -m pytest tools/test_oracles.py`

Every program of tests/run/c compiles at -O2 with every `LLRM_CHECK_*` oracle on (the ones tools/identical.py sets): a kept result
that is not what working it out again gives panics there. #1854 made a loop's box search read a step one guard too wide, which the
table's `LLRM_CHECK_REPLAY` caught on tests/run/c/shrink_wrapped_frame.c only in the identity gate, an hour on (#1867).
"""
import os
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import identical  # noqa: E402
import llrmbin  # noqa: E402

FIXTURES = sorted((HERE.parent / "tests/run/c").glob("*.c"))


def compiled(source: Path) -> str | None:
    env = {**{k: v for k, v in os.environ.items() if not k.startswith("LLRM_")}, **{f"LLRM_CHECK_{one}": "1" for one in identical.CHECKS}}
    done = subprocess.run([str(llrmbin.bin_dir() / "llrm-c"), "-m32", "-march=i486", "-O2", "-o", os.devnull, str(source)], capture_output=True, text=True, env=env)
    return None if done.returncode == 0 else f"{source.name}: {done.stderr.strip().splitlines()[0][:200] if done.stderr.strip() else done.returncode}"


def test_every_run_program_compiles_with_every_oracle_on():
    assert len(FIXTURES) >= 90, f"{len(FIXTURES)} programs in tests/run/c"
    with ThreadPoolExecutor(int(os.environ.get("JOBS", "6"))) as pool:
        failed = [one for one in pool.map(compiled, FIXTURES) if one]
    assert not failed, "an oracle fired: " + "; ".join(failed)
