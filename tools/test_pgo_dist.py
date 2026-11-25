"""tools/pgo_dist.sh --check: the version and training-list checks of the PGO recipe, without a build."""

import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_check_mode_passes_with_the_toolchain_and_training_list():
    done = subprocess.run([str(ROOT / "tools/pgo_dist.sh"), "--check"], capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    assert done.stdout.startswith("pgo_dist: ok (LLVM ")


def test_training_list_leaves_out_what_the_gain_is_measured_on():
    names = subprocess.run(["python3", str(ROOT / "tools/pgo_train.py"), "--list"], capture_output=True, text=True, check=True).stdout.split()
    assert names and not {"bit_test.c", "memory_expansion.c", "long_compare_chain.c", "os_execute.c", "shl_long_by_one.c"} & set(names)
