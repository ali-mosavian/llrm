"""check-ivusers.py reports a compile that panics, and sets the flag the compiler reads."""
import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("check_ivusers", HERE / "check-ivusers.py")
check_ivusers = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check_ivusers)


def fake(tmp_path, body):
    script = tmp_path / "llrm-c"
    script.write_text(f"#!/bin/sh\n{body}\n")
    script.chmod(0o755)
    return str(script)


def test_a_compile_that_panics_is_reported_not_counted_as_clean(tmp_path):
    """A runner that read only stdout would have printed `0 failed` for 284 stale carries."""
    key, why = check_ivusers.compile_one(("f -O2", [fake(tmp_path, "echo 'IvUsers stale' >&2; exit 101")]))
    assert key == "f -O2" and "stale" in why


def test_the_compiler_runs_with_the_check_on(tmp_path):
    key, why = check_ivusers.compile_one(("f -O2", [fake(tmp_path, '[ "$LLRM_CHECK_IVUSERS" = 1 ] || exit 1')]))
    assert why == ""
