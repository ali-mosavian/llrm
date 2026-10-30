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


def test_concurrent_sysroot_builds_extract_once_and_never_expose_a_half_built_root(tmp_path):
    """Gate run 4: qb_arm64's tests ran the script in parallel into one tree; dpkg-deb failed "Cannot unlink" and a program ran on a half-extracted root and segfaulted."""
    import os
    import subprocess

    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    log = tmp_path / "dpkg.log"
    fakes = {
        "apt-cache": "echo libgcc-14-dev-arm64-cross",
        "apt-get": "touch a.deb b.deb c.deb; sleep 0.2",
        "dpkg": f'echo "$2" >> {log}; sleep 0.2; mkdir -p "$3/usr/aarch64-linux-gnu/lib"; touch "$3/usr/aarch64-linux-gnu/lib/$(basename "$2").so"',
    }
    for name, body in fakes.items():
        (bin_dir / name).write_text(f"#!/bin/sh\n{body}\n")
        (bin_dir / name).chmod(0o755)
    env = {**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}", "ARM64_SYSROOT": str(tmp_path / "sysroot")}
    script = str(HERE.parent / "build-arm64-sysroot.sh")
    runs = [subprocess.Popen([script], env=env, stdout=subprocess.PIPE, text=True) for _ in range(4)]
    for run in runs:
        out, _ = run.communicate()
        root = Path(out.strip())
        assert run.returncode == 0 and (root / ".complete").is_file()
        assert len(list((root / "usr/aarch64-linux-gnu/lib").iterdir())) == 3
    assert len(log.read_text().split()) == 3, "one builder extracts each package once"
