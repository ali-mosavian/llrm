"""The tests that need the pin's llc, opt or llvm-tblgen run in the gate, on a pin tools tree its own job builds.

Gate run 2 went red on llrm-dagisel-gen's `checked_in_tables_match_a_fresh_regeneration`: no runner held the pin's source or an llvm-tblgen
(the toolchain job installs clang alone), regen.sh stopped at `LLVM_SRC is not at the pin`, and the unit group failed on every PR.
Skipping the test (DAGISEL_SKIP_PIN) would have left stale tables unchecked."""

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CI = (ROOT / ".github/workflows/ci.yml").read_text()
SETUP = (ROOT / ".github/actions/setup/action.yml").read_text()


def test_the_gate_does_not_skip_the_stale_check():
    assert "DAGISEL_SKIP_PIN" not in CI and "DAGISEL_SKIP_PIN" not in SETUP


def test_a_pintools_job_builds_the_tree_in_this_run_and_the_gate_waits_for_it():
    job = CI.split("  pintools:\n")[1].split("\n  gate:\n")[0]
    assert "./.github/actions/pintools-tree" in job
    assert "needs.pintools.result == 'success'" in CI.split("\n  gate:\n")[1].split("    runs-on")[0]


def test_the_clang_tree_jobs_keep_their_cache_keys():
    """The clang trees are an hour's build: the new tree rides in its own job and action."""
    assert "pintools" not in (ROOT / ".github/actions/clang-tree/action.yml").read_text()
    assert "pintools" not in CI.split("  toolchain:\n")[1].split("  # The pin's llc")[0]


def test_only_the_unit_group_restores_the_tree_and_a_missing_one_is_red():
    gate = CI.split("\n  gate:\n")[1].split("\n  # The verdict")[0]
    assert "pintools: ${{ (matrix.group == 'unit' || matrix.group == 'integration') && 'required' || '' }}" in gate


def test_a_failed_pintools_job_is_a_failed_verdict():
    verdict = CI.split("needs: [plan, static, toolchain, pintools,")[1]
    assert 'needs.pintools.result }}" != failure' in verdict and 'needs.pintools.result }}" != cancelled' in verdict


TOOLS = ("llc", "opt", "llvm-tblgen")


def run_step(tmp_path: Path, tools: tuple[str, ...]) -> subprocess.CompletedProcess:
    script = SETUP.split("name: The pin's tools tree (built by the pintools job)")[1].split("run: |\n")[1].split("    - name: The clang trees")[0]
    (tmp_path / "build/bin").mkdir(parents=True)
    for tool in tools:
        (tmp_path / "build/bin" / tool).write_text("#!/bin/sh\n")
        (tmp_path / "build/bin" / tool).chmod(0o755)
    script = script.replace("${{ steps.pintools.outputs.dir }}", str(tmp_path))
    (tmp_path / "env").write_text("")
    return subprocess.run(["bash", "-c", script], cwd=ROOT, capture_output=True, text=True, env={**os.environ, "GITHUB_ENV": str(tmp_path / "env")})


def test_a_missing_tree_fails_the_unit_group(tmp_path):
    done = run_step(tmp_path, tools=())
    assert done.returncode == 1 and "::error::no pin tools tree" in done.stdout


def test_a_tree_without_llc_is_red(tmp_path):
    """Gate run 2: ~/scratch/llvm-build/bin/llc did not exist on the runner, and tests/common/pin.rs, cgp_global_register_address and differential.rs failed on it."""
    done = run_step(tmp_path, tools=("opt", "llvm-tblgen"))
    assert done.returncode == 1 and "llc is missing" in done.stdout


def test_a_present_tree_exports_llvm_src_and_build(tmp_path):
    done = run_step(tmp_path, tools=TOOLS)
    env = (tmp_path / "env").read_text()
    assert done.returncode == 0, done.stdout + done.stderr
    assert f"LLVM_SRC={tmp_path}/src" in env and f"LLVM_BUILD={tmp_path}/build" in env and f"LLVM_BIN={tmp_path}/build/bin" in env


def test_the_prune_step_keeps_the_tree():
    """The prune step deletes ~/.cache/llrm trees whose last 12 characters no pin file names: pintools-<sha12> ends in the clang pin's."""
    assert "toolchain/clang/pin || { echo \"pruned" in SETUP
    sha12 = (ROOT / "toolchain/clang/pin").read_text().split()[0][:12]
    out = subprocess.run(["sh", "toolchain/pintoolsbuild.sh", "--print"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    assert out.endswith(f"pintools-{sha12}")
