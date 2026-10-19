from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import postland  # noqa: E402


def test_a_failure_the_previous_head_or_the_xfail_list_has_is_not_new():
    """A loop that named every standing failure as the batch's would open an issue per head."""
    listed = {"unit llrm-qb::a": {}}
    assert postland.new_failures({"run x", "run y", "unit llrm-qb::a"}, {"run x"}, listed) == {"run y"}


def test_the_culprit_is_the_first_commit_that_fails_and_none_when_the_head_does_not():
    commits = list("abcdefgh")
    assert postland.culprit(commits, lambda sha: sha >= "e") == "e"
    assert postland.culprit(commits, lambda sha: sha >= "a") == "a"
    assert postland.culprit(commits, lambda sha: False) is None


def test_the_culprit_search_runs_log_n_probes():
    seen = []
    postland.culprit(list(map(str, range(16))), lambda sha: seen.append(sha) or int(sha) >= 11)
    assert len(seen) <= 5


def test_a_squash_subject_names_its_pr_and_a_body_its_owner():
    assert postland.pr_of("fix(gate): the sigterm test waits (#1755)") == 1755
    assert postland.pr_of("merge: origin/feat/parity") is None
    assert postland.owner_of("TL;DR: x\n\nOwner: bench-instruments-2206\n") == "bench-instruments-2206"
    assert postland.owner_of("no owner here") == "unknown"


def test_a_failure_has_a_command_that_runs_it_alone():
    assert postland.repro("run c/idioms.c [-O2]") == "python3 tools/dosbatch/run_tests.py idioms"
    assert postland.repro("unit llrm-qb::mod::test_x") == "cargo test --release -p llrm-qb --lib mod::test_x"


def test_the_report_is_a_table_of_failure_culprit_owner_repro():
    rows = [{"failure": "run scroll", "commit": "abc1234567", "pr": 2890, "owner": "simplify-combine-wiring-2128", "repro": "python3 tools/dosbatch/run_tests.py scroll"}]
    text = postland.report("f" * 40, "e" * 40, rows)
    assert "| run scroll | #2890 | simplify-combine-wiring-2128 | `python3 tools/dosbatch/run_tests.py scroll` |" in text
    assert "No new failure" in postland.report("f" * 40, "e" * 40, [])


def test_failing_tests_are_attributed_to_the_crate_whose_binary_ran_them():
    """Per-crate runs lost the root package's features: 19 llrm-c tests failed with 'built without the toolchain feature' and were reported as the batch's."""
    out = (
        "     Running unittests src/lib.rs (target/release/deps/llrm_c-0123abcd)\n\nfailures:\n    compile::tests::test_a\n\ntest result: FAILED. 1 failed\n"
        "     Running unittests src/lib.rs (target/release/deps/llrm_mir-4567ef01)\n\ntest result: ok. 3 passed\n"
        "     Running unittests src/lib.rs (target/release/deps/llrm_qb-89abcdef)\n\nfailures:\n    t::test_b\n    test_c\n\ntest result: FAILED. 2 failed\n"
    )
    assert postland.by_binary(out) == {"llrm-c": ["compile::tests::test_a"], "llrm-qb": ["t::test_b", "test_c"]}


def test_a_failed_run_that_names_no_test_is_a_failure_not_a_pass():
    """`cargo test -q` printed no `Running` lines, the parse found nothing, and 59 untested landings were reported 'No new failure'."""
    assert postland.unit_result("error: could not compile `llrm-c`\n", 101) == {"unit workspace::failed-without-a-test-name"}
    assert postland.unit_result("test result: ok. 3 passed\n", 0) == set()


def test_stderr_and_stdout_keep_their_order(tmp_path):
    """Cargo's `Running` (stderr) must stay before its crate's results (stdout); read apart, 14 failing tests had no crate."""
    script = "echo 'Running unittests (deps/llrm_c-0a1b)' >&2; echo 'failures:'; echo '    t::x'"
    output, code = postland.merged(["sh", "-c", script], tmp_path, {})
    assert code == 0 and output.index("Running") < output.index("failures:")


def test_a_tree_built_after_a_newer_one_is_still_newer_than_the_shared_targets_artifacts(tmp_path):
    """A shared target called the older commit's sources fresh and linked the newer commit's crate: 10 compile errors at the head, and a culprit (#2757) that was not."""
    import os
    import subprocess

    def git(*args):
        subprocess.run(["git", "-C", str(tmp_path), "-c", "user.name=t", "-c", "user.email=t@t", *args], capture_output=True, check=True)

    git("init", "-q")
    (tmp_path / "a.rs").write_text("x")
    git("add", "a.rs")
    git("commit", "-q", "-m", "a")
    artifact = tmp_path / "artifact"
    artifact.write_text("built later")
    os.utime(tmp_path / "a.rs", (1, 1))
    postland.fresh(tmp_path)
    assert (tmp_path / "a.rs").stat().st_mtime >= artifact.stat().st_mtime
