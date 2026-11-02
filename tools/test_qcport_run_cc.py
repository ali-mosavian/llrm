import subprocess
import sys
from pathlib import Path

TOOL = Path(__file__).resolve().parent / "qcport-run-cc.py"
ENV = {"PATH": "/usr/bin:/bin", "QCPORT": "/x", "QCPORT_INC": "/x", "QCPORT_BORLAND": "/x", "LLRM_BORLAND_INC": "/x"}


def run(*args, **env):
    return subprocess.run([sys.executable, str(TOOL), *args], capture_output=True, text=True, env={**ENV, **env})


def test_a_missing_fork_clang_is_named_not_a_traceback_or_a_compile_failure():
    """w6 saw the gate fail deep in a compile with LLRM_CLANG_M16 unset."""
    done = run()
    assert done.returncode != 0
    assert "LLRM_CLANG_M16 is not set" in done.stderr and "Traceback" not in done.stderr


def test_row_without_a_value_is_one_clear_line():
    done = run("--row", LLRM_CLANG_M16="/x")
    assert done.returncode != 0 and "--row wants" in done.stderr and "Traceback" not in done.stderr


def test_the_compiler_is_the_one_in_llrm_bin_not_the_checkouts_target_dir(tmp_path):
    """The full gate at 9a0cd57af1 (CARGO_TARGET_DIR outside the checkout) failed qcport and oracle-qcport on <checkout>/target/release/llrm-cc."""
    import importlib.util

    spec = importlib.util.spec_from_file_location("qcport_run_cc", TOOL)
    tool = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(tool)
    assert tool.compiler(tmp_path, {"LLRM_BIN": "/gate/bins"}) == Path("/gate/bins/llrm-cc")
    assert tool.compiler(tmp_path, {}) == tmp_path / "target/release/llrm-cc"
