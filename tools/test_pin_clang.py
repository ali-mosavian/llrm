import subprocess
import sys
from pathlib import Path

import pytest

TOOL = Path(__file__).with_name("pin_clang.py")


def run(env_path):
    return subprocess.run(
        [sys.executable, str(TOOL)],
        env={"LLRM_PIN_CLANG": env_path, "PATH": "/usr/bin:/bin"},
        capture_output=True,
        text=True,
    )


@pytest.mark.skipif(not Path("/usr/bin/clang").exists(), reason="no distro clang")
def test_the_distro_clang_is_refused_as_the_pin():
    """The vsgcc scripts and a bare `clang` ledger line ran PATH clang, the
    distro 20.1.8, whose pass pipeline is not the pin's: its Ir read as the pin."""
    done = run("/usr/bin/clang")
    assert done.returncode != 0
    assert "d1106deb71cc" in done.stderr


def test_a_missing_binary_is_refused():
    assert run("/nonexistent/clang").returncode != 0


ROOT = Path(__file__).resolve().parents[1]
VSGCC = next((ROOT / "crates/target").glob("*/vsgcc"))


@pytest.mark.skipif(not Path("/usr/bin/clang").exists(), reason="no distro clang")
def test_the_vsgcc_scripts_refuse_the_distro_clang(tmp_path):
    """The scripts called PATH's `clang`, the distro's 20.1.8, from 2026-10-06 to 2026-10-23, and its numbers were read as the pin's: the clang column of the vsgcc tables,
    the rows series' min(gcc, clang), the compile-time comparisons. levels_time.command and build.sh now take the pin or stop."""
    import os

    env = {**os.environ, "LLRM_PIN_CLANG": "/usr/bin/clang"}
    probe = f"import sys; sys.path[:0] = ['{ROOT / 'tools'}', '{VSGCC}']; import levels_time; levels_time.command('clang', 'O2', __import__('pathlib').Path('x.c'))"
    done = subprocess.run([sys.executable, "-c", probe], capture_output=True, text=True, env=env)
    assert done.returncode != 0 and "not the pin" in done.stderr, done.stderr
    built = subprocess.run(["bash", str(VSGCC / "build.sh"), "sieve"], capture_output=True, text=True, env={**env, "VSGCC_WORK": str(tmp_path), "LLRM": "/bin/true"})
    assert built.returncode != 0 and "not the pin" in built.stderr, built.stderr


def test_no_script_calls_a_bare_clang():
    """The call sites that were PATH's clang: a command list `(["clang"`, `clang $F` in build.sh, a default of "clang". They take pin_clang.path() now."""
    import re

    bare = re.compile(r"""\(\["clang"|default="clang"|^\s+clang \$F|\$\(clang """)
    found = {}
    for path in [*VSGCC.rglob("*.py"), *VSGCC.rglob("*.sh"), *(ROOT / "tools").glob("*.py")]:
        if path.name.startswith("test_"):
            continue
        lines = [n for n, line in enumerate(path.read_text().splitlines(), 1) if bare.search(line)]
        if lines:
            found[str(path.relative_to(ROOT))] = lines
    assert not found, f"a bare clang (PATH's, the distro's): {found}"
