"""A name two test dirs share breaks collection of the whole tools/ tree.

The hosted gate went red on both: tools/ladder/gen.py shadowed tools/intrinsics/gen.py (`ImportError: cannot import name 'SHAPES' from 'gen'`)
and tools/test_pr_state.py shared a basename with tools/ci/test_pr_state.py (`import file mismatch`)."""

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

TOOLS = Path(__file__).resolve().parent


def test_no_two_test_files_share_a_basename():
    seen = defaultdict(list)
    for one in TOOLS.rglob("test_*.py"):
        if ".venv" not in one.parts:
            seen[one.name].append(str(one.relative_to(TOOLS)))
    assert {name: where for name, where in seen.items() if len(where) > 1} == {}


def test_the_dirs_that_shared_a_module_name_collect_together():
    cmd = [sys.executable, "-m", "pytest", "--collect-only", "-q", "-p", "no:cacheprovider",
           "ladder/test_ladder.py", "intrinsics/test_gen.py", "test_landing_pr_state.py", "ci/test_pr_state.py"]
    run = subprocess.run(cmd, cwd=TOOLS, capture_output=True, text=True, timeout=120)
    assert run.returncode == 0, run.stdout[-1500:] + run.stderr[-500:]


def test_nothing_imports_the_ladder_generator_under_its_old_name():
    """#5961 renamed ladder/gen.py to ladder_gen.py; test_alloc_scaling.py kept an indented `import gen` the rename's grep missed, and the
    batch's tool-tests failed `module 'gen' has no attribute 'generate'`."""
    old_import = re.compile(r"^\s*(import gen|from gen import)\b", re.MULTILINE)
    stale = [str(one.relative_to(TOOLS)) for one in TOOLS.rglob("*.py") if ".venv" not in one.parts and "ladder" in one.read_text() and old_import.search(one.read_text())]
    assert stale == []
