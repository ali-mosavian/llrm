"""The clang trees reach the gate jobs of a pull request into main, or the job fails loudly.

Run 40780286803 (feat/parity into main) found neither tree: toolchain.yml saved them on refs/heads/feat/parity, which that pull request cannot
read. LLRM_PIN_CLANG stayed unset, a warning only, and every llrm-cc row fell back to ~/scratch/llvm-build/bin/clang (rc 1, no such file)."""

import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CI = (ROOT / ".github/workflows/ci.yml").read_text()
SETUP = (ROOT / ".github/actions/setup/action.yml").read_text()


def test_the_gate_waits_for_a_toolchain_job_that_builds_both_trees_in_this_run():
    job = CI.split("  toolchain:\n")[1].split("\n  gate:\n")[0]
    assert "kind: [pin, m16]" in job and "./.github/actions/clang-tree" in job and "timeout-minutes: 120" in job
    assert "needs: [plan, build, toolchain, tblgen]" in CI and "needs.toolchain.result == 'success'" in CI


def test_a_failed_toolchain_job_is_a_failed_verdict():
    verdict = CI.split("needs: [plan, static, toolchain,")[1]
    assert 'needs.toolchain.result }}" != failure' in verdict


def test_the_gate_jobs_require_the_trees_and_the_other_jobs_only_warn():
    gate = CI.split("\n  gate:\n")[1].split("\n  # The verdict")[0]
    assert 'clang: "required"' in gate
    assert CI.count('clang: "required"') == 1


def run_step(tmp_path: Path, required: str, tree: bool) -> subprocess.CompletedProcess:
    """The setup action's 'clang trees' step with the pin and m16 directories substituted; tree=False leaves them empty."""
    script = SETUP.split("name: The clang trees (built by the toolchain workflow)")[1].split("run: |\n")[1].split("    - name: Drop")[0]
    pin, m16 = tmp_path / "pin", tmp_path / "m16"
    for one in (pin, m16):
        (one / "bin").mkdir(parents=True)
        if tree:
            (one / "bin/clang").write_text("#!/bin/sh\necho clang version 99 d1106deb71cc\n")
            (one / "bin/clang").chmod(0o755)
    script = script.replace("${{ steps.clang.outputs.pin }}", str(pin)).replace("${{ steps.clang.outputs.m16 }}", str(m16)).replace("${{ inputs.clang }}", required)
    env = {**os.environ, "GITHUB_ENV": str(tmp_path / "env")}
    (tmp_path / "env").write_text("")
    return subprocess.run(["bash", "-c", script], cwd=ROOT, capture_output=True, text=True, env=env)


def test_a_missing_tree_fails_a_job_that_requires_it(tmp_path):
    done = run_step(tmp_path, "required", tree=False)
    assert done.returncode == 1 and "::error::no pinned clang tree" in done.stdout


def test_a_missing_tree_only_warns_elsewhere(tmp_path):
    done = run_step(tmp_path, "", tree=False)
    assert done.returncode == 0 and "::warning::no pinned clang tree" in done.stdout


def test_a_present_tree_sets_both_variables(tmp_path):
    done = run_step(tmp_path, "required", tree=True)
    env = (tmp_path / "env").read_text()
    assert done.returncode == 0, done.stdout + done.stderr
    assert "LLRM_PIN_CLANG=" in env and "LLRM_CLANG_M16=" in env


def test_the_prune_step_keeps_the_clang_trees():
    """The step after the export deleted every ~/.cache/llrm tree no scripts named: the clang trees' pin file was not among them."""
    assert "toolchain/clang/pin || { echo \"pruned" in SETUP


def test_a_built_tree_is_saved_before_the_job_ends_so_a_cancelled_run_keeps_it():
    """ci.yml cancels a superseded pull request run; the cache action's post step does not run in a cancelled job, so a ~1 h build was lost."""
    action = (ROOT / ".github/actions/clang-tree/action.yml").read_text()
    assert "actions/cache/save@v4" in action and "actions/cache/restore@v4" in action and "actions/cache@v4\n      with:\n        path: ${{ steps.tree" not in action
    assert action.index("clangbuild.sh ${{ inputs.kind }}") < action.index("actions/cache/save@v4")
