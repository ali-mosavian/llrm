"""`uv run --project tools python -m pytest tools/test_oracles.py`

Every program of tests/run/c compiles at -O2 with every `LLRM_CHECK_*` oracle on (the ones tools/identical.py sets): a kept result
that is not what working it out again gives panics there. #1854 made a loop's box search read a step one guard too wide, which the
table's `LLRM_CHECK_REPLAY` caught on tests/run/c/shrink_wrapped_frame.c only in the identity gate, an hour on (#1867).
"""
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import identical  # noqa: E402
import llrmbin  # noqa: E402

FIXTURES = sorted((HERE.parent / "tests/run/c").glob("*.c"))


def compiled(source: Path) -> str | None:
    env = {**{k: v for k, v in os.environ.items() if not k.startswith("LLRM_")}, **{f"LLRM_CHECK_{one}": "1" for one in identical.CHECKS}}
    with tempfile.TemporaryDirectory() as work:  # an object, not /dev/null: llrm-c encodes only for `.obj`, so the null device never reached the encoder
        out = str(Path(work) / "out.obj")
        done = subprocess.run([str(llrmbin.bin_dir() / "llrm-c"), "-m32", "-march=i486", "-O2", "-o", out, str(source)], capture_output=True, text=True, env=env)
        if done.returncode and ("wccq failed" in done.stderr or "unsupported builtin" in done.stderr):  # llrm-c's Watcom front end has no <stdint.h> and no overflow builtins; llrm-cc over clang is the C front end
            done = subprocess.run([str(llrmbin.bin_dir() / "llrm-cc"), "-O2", "-c", "-o", out, str(source)], capture_output=True, text=True, env={**env, "LLRM_CC_TARGET": "x86_32-dos"})
    return None if done.returncode == 0 else f"{source.name}: {done.stderr.strip().splitlines()[0][:200] if done.stderr.strip() else done.returncode}"


def test_a_program_llrm_c_cannot_compile_goes_through_llrm_cc():
    """float_bits.c includes <stdint.h>, which llrm-c's wccq lacks: the oracle test failed on it for days."""
    assert compiled(FIXTURES[0].parent / "float_bits.c") is None


def test_every_run_program_compiles_with_every_oracle_on():
    assert len(FIXTURES) >= 90, f"{len(FIXTURES)} programs in tests/run/c"
    with ThreadPoolExecutor(int(os.environ.get("JOBS", "6"))) as pool:
        failed = [one for one in pool.map(compiled, FIXTURES) if one]
    assert not failed, "an oracle fired: " + "; ".join(failed)
