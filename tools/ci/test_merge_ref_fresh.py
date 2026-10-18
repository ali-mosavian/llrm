"""#2340 was labelled merge-candidate and no run came: the pull_request workflow is the merge ref's, which predated the new `on:`."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import merge_ref_fresh as m  # noqa: E402

MAIN = Path(__file__).parents[2].joinpath(".github/workflows/ci.yml").read_text()
OLD = MAIN.replace("    types: [labeled]\n", "    types: [opened, synchronize, reopened, ready_for_review]\n").replace("    branches: [main]\n    types", "    types")


def test_main_triggers_a_run_on_the_label_and_on_nothing_else_of_a_pull_request():
    block = m.on_block(MAIN)
    assert "types: [labeled]" in block and "branches: [main]" in block and "opened" not in block


def test_a_merge_ref_with_the_old_on_block_is_stale_and_says_why():
    """The merge ref made before #2358 had `types: [opened, synchronize, ...]`: a label event matched none, so the loop's label started nothing."""
    ok, why = m.check(OLD, MAIN)
    assert not ok and "label starts no run" in why


def test_a_merge_ref_with_the_old_jobs_but_the_same_triggers_is_stale_too():
    ok, why = m.check(MAIN.replace("timeout-minutes: 10", "timeout-minutes: 11", 1), MAIN)
    assert not ok and "jobs are old" in why


def test_a_merge_ref_carrying_mains_workflow_is_fresh():
    assert m.check(MAIN, MAIN)[0]
