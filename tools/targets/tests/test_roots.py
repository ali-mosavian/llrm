"""Moving a tool one directory down broke its way to the repository root: the arm64 tools looked for `crates/` under `tools/`."""
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent


def test_python_tools_find_the_repository_root():
    """Read from the source: arm64_fuzz runs main() on import."""
    for name in ("arm64_compare.py", "arm64_fuzz.py"):
        found = re.search(r"^ROOT = Path\(__file__\)\.resolve\(\)\.parents\[(\d)\]$", (HERE.parent / name).read_text(), re.M)
        assert found, name
        assert (HERE.parent.parents[int(found.group(1)) - 1] / "Cargo.toml").is_file(), name


def test_shell_tools_find_the_repository_root():
    for script in sorted(HERE.parent.glob("*.sh")):
        found = re.search(r'root=\$\(cd "\$\(dirname "\$0"\)((?:/\.\.)+)"', script.read_text())
        if found:
            assert (script.parent / found.group(1).lstrip("/") / "Cargo.toml").is_file(), script.name


def test_sysroot_script_takes_the_newest_libgcc_the_runner_has():
    """The script named libgcc-15-dev-arm64-cross; gate 3's runner had no such package and every qb_arm64 test failed in apt-get."""
    import subprocess

    names = "libgcc-s1-arm64-cross\nlibgcc-9-dev-arm64-cross\nlibgcc-14-dev-arm64-cross\nlibgcc-13-dev-arm64-cross\n"
    out = subprocess.run([str(HERE.parent / "build-arm64-sysroot.sh"), "--newest-libgcc"], input=names, capture_output=True, text=True, check=True)
    assert out.stdout.strip() == "libgcc-14-dev-arm64-cross"


def test_the_arm64_qb_tests_and_sysroot_script_run_no_c_compiler_driver():
    """tests/qb_arm64.rs ran clang as the driver and read gcc's directory; the rule is llrm-cc (arm64-glibc row) and the sysroot's files by path."""
    root = HERE.parents[2]
    test = (root / "tests/qb_arm64.rs").read_text()
    assert 'Command::new("clang")' not in test and 'Command::new("gcc")' not in test
    assert "gcc-cross/aarch64-linux-gnu/15" not in test
    script = (HERE.parent / "build-arm64-sysroot.sh").read_text()
    assert not re.search(r"^\s*(clang|gcc)\b", script, re.M)
