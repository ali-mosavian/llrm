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


def test_a_pull_request_runs_the_cheap_tier_and_only_the_labelled_one_also_bench_cost_and_perf():
    """Every push of every pull request ran bench, cost and perf (a 55-minute candidate-sized run each): they run once, for the merge candidate."""
    assert "labeled]" in CI.split("pull_request:")[1].split("push:")[0]
    plan = CI.split("  plan:")[1].split("outputs:")[0]
    assert "github.event.label.name == 'merge-candidate'" in plan, "any other label would start a run"
    assert "contains(github.event.pull_request.labels.*.name, 'merge-candidate')" in CI
    assert 'g not in ("bench", "cost", "perf")' in CI and 'os.environ["CANDIDATE"] == "true"' in CI
    assert "github.event_name != 'pull_request' ||" in CI, "a push to main keeps every group: main records the bench counts"


def test_the_verdict_line_says_which_tier_ran_and_a_cheap_pass_is_not_a_candidate_pass():
    """cimerge.sh reads this line through tools/ci/candidate_ok.py: a cheap PASS must not parse as a candidate one."""
    import candidate_ok

    assert 'CANDIDATE PASS at ${head:0:12}$on"' in CI and '(cheap: no perf)"' in CI
    assert candidate_ok.CANDIDATE.search("GATE full CANDIDATE PASS at 0123456789ab on ba9876543210")
    assert not candidate_ok.CANDIDATE.search("GATE full PASS at 0123456789ab on ba9876543210 (cheap: no perf)")


def test_only_a_pull_request_into_main_starts_the_gate():
    """A parity pull request into feat/parity merges on a build and a smoke set (tools/parity_smoke.py); its CI runs were cancelled by hand."""
    assert "branches: [main]" in CI.split("pull_request:")[1].split("push:")[0]
