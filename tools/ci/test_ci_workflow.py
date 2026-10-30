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
    assert "tinytim" not in "\n".join(l for l in CI.splitlines() if l.strip().startswith("runs-on:") and l.strip() != "runs-on: [self-hosted, tinytim]"), "the picker chooses; setup-check is the one job that asks for tinytim"


def test_perf_may_only_be_missing_on_a_hosted_runner():
    assert "GATE_ALLOW_MISSING: perf" in CI and "runner.environment == 'self-hosted'" in CI


def test_the_verdict_line_keeps_its_form_and_names_the_runners():
    """Parity needs `GATE <tier> PASS at <head12>` (a pull request's adds ` on <parent12>`); the runners follow it, so one PR passing on tinytim and failing hosted is legible."""
    assert 'line="GATE ${{ needs.plan.outputs.tier }} PASS at ${head:0:12}$on"' in CI and "[runners: ${{ needs.plan.outputs.runner_summary }}]" in CI


def test_nothing_runs_on_a_push_and_a_pull_request_runs_only_when_labelled_merge_candidate():
    """The gate runs in CI, only to merge into main: a run per push of every pull request queued behind the merge loop on a host at load 97."""
    pr = CI.split("pull_request:")[1].split("push:")[0]
    assert "types: [labeled]" in pr and "opened" not in pr and "synchronize" not in pr
    plan = CI.split("  plan:")[1].split("outputs:")[0]
    assert "github.event.label.name == 'merge-candidate'" in plan, "any other label would start a run"
    assert "github.event_name != 'pull_request' ||" in plan, "a push to main keeps the full gate: main records the bench counts"
    assert "branches: [main]" in CI.split("push:")[1].split("workflow_dispatch")[0]


def test_the_verdict_line_of_a_pull_request_is_the_candidate_pass_and_no_cheap_form_exists():
    """cimerge.sh reads this line through tools/ci/candidate_ok.py."""
    import candidate_ok

    assert 'CANDIDATE PASS at ${head:0:12}$on"' in CI and "cheap" not in CI.lower().replace("cheaper", "")
    assert candidate_ok.CANDIDATE.search("GATE full CANDIDATE PASS at 0123456789ab on ba9876543210")
    assert not candidate_ok.CANDIDATE.search("GATE full PASS at 0123456789ab on ba9876543210")


def test_only_a_pull_request_into_main_starts_the_gate():
    """A parity pull request into feat/parity merges on a build and a smoke set (tools/parity_smoke.py); its CI runs were cancelled by hand."""
    assert "branches: [main]" in CI.split("pull_request:")[1].split("push:")[0]


def test_the_gate_job_outlasts_its_slowest_group_measured():
    """Runs 3 and 4: `gate (bench-base)` was cancelled at the job's 60 min with its base leg unfinished (10 min of build, still measuring main's
    rows at minute 47; a cold leg is 28 min on a fast box and the runner is ~1.7x slower). The timeout must be 1.5x the 60 min it was seen to
    hit, so a cold base leg that needs more than 60 still finishes."""
    import re

    gate = CI.split("\n  gate:\n")[1]
    timeout = int(re.search(r"timeout-minutes: (\d+)", gate)[1])
    assert timeout >= 90, timeout
