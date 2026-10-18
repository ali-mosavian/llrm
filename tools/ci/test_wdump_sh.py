"""toolchain/wdump.sh prints the path of wdump alone on stdout, even when a cache miss rebuilds Open Watcom's tree and its build is noisy.

On a restore timeout the setup action's `wdump=$(sh toolchain/wdump.sh)` captured the build's log with the path, and `echo "WDUMP=$wdump" >> $GITHUB_ENV`
failed with "Invalid format": gate (python) never ran a test (#2093)."""

import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def noisy_tree(tmp_path: Path) -> Path:
    """An Open Watcom tree whose bootstrap prints to stdout and whose wmake prints and leaves a host executable as wdump.exe."""
    tree = tmp_path / "ow"
    (tree / "bld/exedump").mkdir(parents=True)
    (tree / "setvars.sh").write_text("echo 'setvars: environment ready'\n")
    (tree / "build.sh").write_text(
        "echo 'build.sh boot: compiling 4000 files'\nmkdir -p build/binbuild\n"
        f"printf '#!/bin/sh\\necho wmake noise\\ncp {shutil.which('true')} wdump.exe\\n' > build/binbuild/wmake\nchmod +x build/binbuild/wmake\n"
    )
    (tree / "build.sh").chmod(0o755)
    return tree


def test_stdout_is_one_line_naming_an_existing_file_after_a_noisy_rebuild(tmp_path):
    env = {**os.environ, "OWROOT": str(noisy_tree(tmp_path)), "XDG_CACHE_HOME": str(tmp_path / "cache")}
    done = subprocess.run(["sh", str(ROOT / "toolchain/wdump.sh")], capture_output=True, text=True, env=env)
    assert done.returncode == 0, done.stderr[-400:]
    lines = done.stdout.splitlines()
    assert len(lines) == 1 and Path(lines[0]).is_file() and lines[0].endswith("/wdump"), done.stdout
    assert "build.sh boot: compiling" in done.stderr, "the build's log goes to stderr"


def test_the_setup_step_reads_the_last_line_and_fails_with_a_message_when_it_is_no_executable():
    step = (ROOT / ".github/actions/setup/action.yml").read_text()
    assert "sh toolchain/wdump.sh | tail -n 1" in step and "which is not an executable file" in step


def captured_scripts():
    """Every toolchain script a workflow or tool captures with `$(sh toolchain/NAME.sh)` (or runs through `$(toolchain/NAME.sh)`)."""
    import re

    found = set()
    for path in [*(ROOT / ".github").rglob("*.yml"), *(ROOT / "tools").rglob("*.py"), *(ROOT / "tools").rglob("*.sh")]:
        if path.name == "test_wdump_sh.py":
            continue
        found |= set(re.findall(r"\$\((?:sh |bash )?(?:\./)?toolchain/(\w+\.sh)", path.read_text(errors="replace")))
    return sorted(found)


def test_every_captured_toolchain_script_prints_its_result_on_a_descriptor_of_its_own():
    """The class, not one script: a script whose stdout is captured by `$(...)` must send every log to stderr and print its result on fd 3, or a build on a
    cache miss pollutes the captured value (wdump.sh, #2093); gcctorture.sh only escaped by a `>&2` on one call."""
    scripts = captured_scripts()
    assert {"wdump.sh", "gcctorture.sh"} <= set(scripts), scripts
    for name in scripts:
        text = (ROOT / "toolchain" / name).read_text()
        assert "exec 3>&1 1>&2" in text and ">&3" in text, f"{name} prints its result on stdout among its logs"


def test_gcctorture_prints_one_line_even_when_git_is_noisy(tmp_path):
    stub = tmp_path / "bin"
    stub.mkdir()
    (stub / "git").write_text("#!/bin/sh\necho 'git: noise on stdout'\n")
    (stub / "git").chmod(0o755)
    env = {**os.environ, "PATH": f"{stub}:{os.environ['PATH']}", "XDG_CACHE_HOME": str(tmp_path / "cache")}
    done = subprocess.run(["sh", str(ROOT / "toolchain/gcctorture.sh")], capture_output=True, text=True, env=env)
    assert done.returncode == 0, done.stderr[-300:]
    assert len(done.stdout.splitlines()) == 1 and done.stdout.strip().endswith("gcc.c-torture/execute"), done.stdout
    assert "git: noise on stdout" in done.stderr
