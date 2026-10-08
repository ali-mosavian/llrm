"""The llrm-c that ships is the llrm-c the gate tests."""
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_a_testing_feature_is_on_by_default_so_the_test_build_and_the_shipped_build_are_one():
    """`cargo test --workspace` turned `testing` on for llrm-core, llrm-omf and llrm-analysis (a dev-dependency asks), `cargo build
    --bins` did not: two llrm-c binaries (md5 847b9c8d against 334a1058 on one tree), the second the one every test and every measurement
    used, whose compile cost differs from the first by up to 6% in a step. The output was the same (393 objects)."""
    meta = json.loads(subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline"], cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    off = [one["name"] for one in meta["packages"] if "testing" in one["features"] and "testing" not in one["features"].get("default", [])]
    assert not off, f"features `testing` of {off} are off by default: another binary is built for the tests"
