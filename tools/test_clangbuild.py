"""toolchain/clangbuild.sh names each clang tree for the sha it is built from."""

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "toolchain/clangbuild.sh"


def where(kind: str, pin: Path | None = None, *flags: str) -> subprocess.CompletedProcess:
    env = {**os.environ, "XDG_CACHE_HOME": "/cache"}
    if pin:
        env["TOOLCHAIN_PIN"] = str(pin)
    return subprocess.run(["sh", str(SCRIPT), "--print", kind, *flags], capture_output=True, text=True, env=env)


def test_the_trees_are_named_for_the_pins_shas():
    """The m16 and pin trees were `~/scratch/llvm-build*` by hand: nothing said which sha a tree was, and a stale one passed silently."""
    lines = (ROOT / "toolchain/clang/pin").read_text().splitlines()
    assert where("pin").stdout.strip() == f"/cache/llrm/clang-{lines[0][:12]}"
    assert where("m16").stdout.strip() == f"/cache/llrm/clang-m16-{lines[1].split()[0][:12]}"


def test_a_bumped_pin_names_a_new_tree(tmp_path):
    pin = tmp_path / "pin"
    pin.write_text("0123456789abcdef0123456789abcdef01234567\n89abcdef0123456789abcdef0123456789abcdef # m16\n")
    assert where("pin", pin).stdout.strip() == "/cache/llrm/clang-0123456789ab"
    assert where("m16", pin).stdout.strip() == "/cache/llrm/clang-m16-89abcdef0123"


def test_a_line_that_is_not_a_sha_is_refused(tmp_path):
    pin = tmp_path / "pin"
    pin.write_text("not a sha\n")
    done = where("pin", pin)
    assert done.returncode == 2 and "not a commit sha" in done.stderr


def test_the_open_watcom_library_is_named_for_the_commit_it_is_built_from():
    """The library tree is `ow-clib-<ow-commit>`: a bumped commit builds a new one, and the cache key (setup action) holds the script."""
    commit = (ROOT / "toolchain/owshim/ow-commit").read_text().strip()
    env = {**os.environ, "XDG_CACHE_HOME": "/cache"}
    done = subprocess.run(["sh", str(ROOT / "toolchain/owclib.sh"), "--print"], capture_output=True, text=True, env=env)
    assert done.stdout.strip() == f"/cache/llrm/ow-clib-{commit}"


def test_the_setup_action_keys_the_forks_cache_on_the_scripts_that_build_the_library():
    action = (ROOT / ".github/actions/setup/action.yml").read_text()
    for script in ("toolchain/owclib.sh", "tools/build-ow-clib.sh", "tools/ow-clib-make.py"):
        assert action.count(script) >= 2, script
