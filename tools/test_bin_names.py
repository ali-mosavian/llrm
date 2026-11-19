import json
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_no_two_bin_targets_of_the_workspace_share_a_name():
    """Two `llrm-cc` bins (the root package's and the llrm-cc crate's) wrote one file: cargo warned of an output filename collision, and a parallel `--bin llrm-cc --bin llrm-llc` build failed removing target/release/llrm-cc."""
    meta = json.loads(subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    names = Counter(target["name"] for package in meta["packages"] for target in package["targets"] if "bin" in target["kind"])
    assert not [name for name, count in names.items() if count > 1], names
