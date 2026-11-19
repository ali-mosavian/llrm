"""The source-scanning ratchets checkland runs (`-p llrm-mir --lib -- no_file_ no_source_`, `-p llrm-m68k --test hygiene`) must exist.

A libtest filter that matches nothing passes: renaming a ratchet turned the gate into zero tests."""
import os
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[1]
PREFIXES = ("no_file_", "no_source_")


def listed(*cargo: str) -> list[str]:
    done = subprocess.run(["cargo", "test", "-q", *cargo, "--", "--list"], cwd=ROOT, capture_output=True, text=True, env={**os.environ, "CARGO_TARGET_DIR": os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))})
    assert done.returncode == 0, done.stderr[-500:]
    return [line.split(": ")[0].rsplit("::", 1)[-1] for line in done.stdout.splitlines() if line.endswith(": test")]


def test_the_ratchet_filters_match_at_least_three_tests():
    if not os.environ.get("LLRM_RATCHET_LIST"):
        pytest.skip("builds two test binaries: LLRM_RATCHET_LIST=1")
    names = [n for n in listed("-p", "llrm-mir", "--lib") if n.startswith(PREFIXES)] + [n for n in listed("-p", "llrm-m68k", "--test", "hygiene")]
    assert len(names) >= 3, f"checkland's ratchet set shrank to {names}"
