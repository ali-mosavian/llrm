from pathlib import Path

CI = (Path(__file__).parents[2] / ".github/workflows/ci.yml").read_text()


def test_a_push_to_main_has_a_concurrency_group_of_its_own():
    """Pushes shared the group `ci-refs/heads/main`: GitHub keeps one pending run per group, so a burst of merges cancelled three main runs unrun."""
    group = next(line for line in CI.splitlines() if line.strip().startswith("group: ci-"))
    assert "github.ref" not in group and "github.run_id" in group


def test_only_pull_requests_cancel_a_superseded_run():
    assert "cancel-in-progress: ${{ github.event_name == 'pull_request' }}" in CI


def test_only_main_and_own_pull_requests_reach_tinytim():
    """The runner is chosen in `plan` from the heartbeat and the fork flag; build and gate take it, no job names tinytim itself."""
    eligible = CI.split("id: runners")[1].split("run:")[0]
    assert "head.repo.full_name == github.repository" in eligible and "github.ref == 'refs/heads/main'" in eligible
    assert "runs-on: ${{ fromJSON(needs.plan.outputs.runner_build) }}" in CI
    assert "runs-on: ${{ fromJSON(needs.plan.outputs.runner_groups)[matrix.group] }}" in CI
    assert "tinytim" not in "\n".join(l for l in CI.splitlines() if l.strip().startswith("runs-on:"))


def test_perf_may_only_be_missing_on_a_hosted_runner():
    assert "GATE_ALLOW_MISSING: perf" in CI and "runner.environment == 'self-hosted'" in CI


def test_the_verdict_line_keeps_its_form_and_names_the_runners():
    """Parity needs `GATE <tier> PASS at <head12>` unchanged; the runners follow it, so one PR passing on tinytim and failing hosted is legible."""
    assert 'line="GATE ${{ needs.plan.outputs.tier }} PASS at ${head:0:12}"' in CI and "[runners: ${{ needs.plan.outputs.runner_summary }}]" in CI
