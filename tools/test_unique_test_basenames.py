import subprocess
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def test_no_two_test_files_share_a_basename():
    """tools has no __init__.py, so pytest names a module by its basename: tools/intrinsics/test_gen.py beside
    tools/difftest/test_gen.py failed collection of the whole gate pytest step ("import file mismatch")."""
    files = subprocess.run(["git", "ls-files", "*.py"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    by_name = defaultdict(list)
    for one in files:
        name = Path(one).name
        if name.startswith("test_") or name.endswith("_test.py"):
            by_name[name].append(one)
    assert {name: paths for name, paths in by_name.items() if len(paths) > 1} == {}
