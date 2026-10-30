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

# Programs llrm-cc over clang cannot build (Watcom headers and types); with llrm-c absent they go unchecked. Debt `oracles-llrm-cc-unbuilt`.
LLRM_CC_UNBUILT = frozenset(('bench_io.c', 'div_constants_long.c', 'div_constants_short.c', 'div_constants_ulong.c', 'far_return.c', 'fixed_point.c', 'frame_address_stack_args.c', 'huge_runs.c', 'inline_emit.c', 'inlined_calls_keep_stores_through_far_alias.c', 'logical_value.c', 'memcpy_libcall.c', 'os_console.c', 'os_console_in.c', 'os_execute.c', 'os_exit.c', 'os_files.c', 'os_find.c', 'os_more.c', 'os_names.c', 'os_screen.c', 'wc16_mixed.c', 'wc_mixed.c', 'wrapv_overflow_check.c'))


LOCATORS = ("LLRM_PIN_CLANG", "LLRM_CLANG_M16", "LLRM_OW")  # where llrm-cc finds its clang and libc: CI sets them, not ~/scratch


def oracle_env(base: dict[str, str]) -> dict[str, str]:
    """`base` without the LLRM_* switches, with the toolchain locators kept and every oracle on."""
    return {**{k: v for k, v in base.items() if not k.startswith("LLRM_") or k in LOCATORS}, **{f"LLRM_CHECK_{one}": "1" for one in identical.CHECKS}}


def compiled(source: Path) -> str | None:
    env = oracle_env(dict(os.environ))
    with tempfile.TemporaryDirectory() as work:  # an object, not /dev/null: llrm-c encodes only for `.obj`, so the null device never reached the encoder
        out = str(Path(work) / "out.obj")
        legacy = llrmbin.bin_dir() / "llrm-c"  # opt-in (feature c-legacy): absent in a default build
        done = subprocess.run([str(legacy), "-m32", "-march=i486", "-O2", "-o", out, str(source)], capture_output=True, text=True, env=env) if legacy.exists() else None
        if done is None or (done.returncode and ("wccq failed" in done.stderr or "unsupported builtin" in done.stderr)):  # llrm-c's Watcom front end has no <stdint.h> and no overflow builtins; llrm-cc over clang is the C front end
            done = subprocess.run([str(llrmbin.bin_dir() / "llrm-cc"), "-O2", "-c", "-o", out, str(source)], capture_output=True, text=True, env={**env, "LLRM_CC_TARGET": "x86_32-dos"})
    return None if done.returncode == 0 else f"{source.name}: {done.stderr.strip().splitlines()[0][:200] if done.stderr.strip() else done.returncode}"


def not_known_unbuilt(failed: list[str]) -> list[str]:
    """The failures that are not a program of LLRM_CC_UNBUILT. Whether llrm-c is built does not matter: a c-legacy llrm-c is clang under
    the Watcom flags and fails these the same way (`unknown type name '_Sat'` refused the runner's batches, whose target has one)."""
    return [one for one in failed if one.split(":")[0] not in LLRM_CC_UNBUILT]


def test_a_program_llrm_c_cannot_compile_goes_through_llrm_cc():
    """float_bits.c includes <stdint.h>, which llrm-c's wccq lacks: the oracle test failed on it for days."""
    assert compiled(FIXTURES[0].parent / "float_bits.c") is None


def test_every_run_program_compiles_with_every_oracle_on():
    assert len(FIXTURES) >= 90, f"{len(FIXTURES)} programs in tests/run/c"
    with ThreadPoolExecutor(int(os.environ.get("JOBS", "6"))) as pool:
        failed = [one for one in pool.map(compiled, FIXTURES) if one]
    failed = not_known_unbuilt(failed)
    assert not failed, "an oracle fired: " + "; ".join(failed)


def test_a_known_unbuilt_program_is_not_an_oracle_failure_with_llrm_c_built(tmp_path, monkeypatch):
    """The filter ran only without llrm-c: the runner's target has one, and fixed_point.c's `_Sat` refused #6040 (14:3x)."""
    (tmp_path / "llrm-c").write_text("")
    monkeypatch.setattr(llrmbin, "bin_dir", lambda: tmp_path)
    shown = ["fixed_point.c: tests/run/c/fixed_point.c:33:1: error: unknown type name '_Sat'", "other.c: an oracle fired"]
    assert not_known_unbuilt(shown) == [shown[1]]


def test_the_oracle_environment_keeps_the_clang_llrm_cc_is_told_of():
    """The filter dropped LLRM_PIN_CLANG and LLRM_CLANG_M16 with the other LLRM_*: llrm-cc fell back to ~/scratch/llvm-build, which only a
    developer's machine has, and every program failed 'No such file' on CI (gate 3, 15:10) while it passed here."""
    env = oracle_env({"LLRM_PIN_CLANG": "/pin/clang", "LLRM_CLANG_M16": "/m16/clang", "LLRM_DEBUG": "checks", "PATH": "/bin"})
    assert env["LLRM_PIN_CLANG"] == "/pin/clang" and env["LLRM_CLANG_M16"] == "/m16/clang" and env["PATH"] == "/bin"
    assert "LLRM_DEBUG" not in env and env["LLRM_CHECK_REPLAY"] == "1"
