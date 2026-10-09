from pathlib import Path

CI = (Path(__file__).parents[2] / ".github/workflows/ci.yml").read_text()


def test_a_push_to_main_has_a_concurrency_group_of_its_own():
    """Pushes shared the group `ci-refs/heads/main`: GitHub keeps one pending run per group, so a burst of merges cancelled three main runs unrun."""
    group = next(line for line in CI.splitlines() if line.strip().startswith("group: ci-"))
    assert "github.ref" not in group and "github.run_id" in group


def test_only_pull_requests_cancel_a_superseded_run():
    assert "cancel-in-progress: ${{ github.event_name == 'pull_request' }}" in CI
