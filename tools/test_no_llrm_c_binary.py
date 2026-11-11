"""The Watcom front end `llrm-c` is gone: no tool script names its binary."""

import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_no_tool_names_the_llrm_c_binary():
    """Dev tools kept running `llrm-c` (callconv, compile-gap, ladder, identical, ...) after it was deleted: each failed 'no such file'."""
    files = subprocess.run(["git", "grep", "-l", "-P", r"\bllrm-c\b(?![-c])", "--", "tools"], cwd=ROOT, capture_output=True, text=True).stdout.split()
    files = [one for one in files if one != "tools/test_no_llrm_c_binary.py"]
    assert not files, files
