"""A fast batch took every queued PR and left out the ones that conflicted with an earlier member: a full smoke for the others each time.
`assemble` keeps the PRs that merge one after another, and the rest wait."""

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402


def sh(repo: Path, *args: str) -> str:
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return done.stdout.strip()


def branch(repo: Path, name: str, path: str, text: str) -> str:
    sh(repo, "checkout", "-q", "-b", name, "main")
    (repo / path).write_text(text)
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", name)
    return sh(repo, "rev-parse", "HEAD")


def test_the_pr_that_conflicts_with_an_earlier_member_waits_and_the_third_joins(tmp_path):
    """Three PRs, the first two edit one line of one file: before, the batch took all three and #2 was left out after the smoke."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "shared.txt").write_text("one\ntwo\nthree\n")
    (repo / "other.txt").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {
        "1": branch(repo, "a", "shared.txt", "one\nTWO\nthree\n"),
        "2": branch(repo, "b", "shared.txt", "one\n2\nthree\n"),
        "3": branch(repo, "c", "other.txt", "y\n"),
    }
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2", "3"], heads, base="main", cwd=repo)
    assert kept == ["1", "3"]
    assert list(waits) == ["2"] and "shared.txt" in waits["2"]


def test_the_limit_counts_kept_prs_only(tmp_path):
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "f").write_text("x\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {str(n): branch(repo, f"b{n}", f"f{n}", "y\n") for n in range(1, 5)}
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(list(heads), heads, base="main", cwd=repo, limit=2)
    assert kept == ["1", "2"] and not waits


def test_a_pr_whose_build_fails_alone_is_dropped_before_assembly_and_checked_once(tmp_path):
    """The 15:34 batch spent 29.6 of 48 minutes rerunning PRs that failed alone: `alone` drops them with the reason, and a head already judged is not built again."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    good, bad = branch(repo, "good", "good.txt", "ok\n"), branch(repo, "bad", "bad.txt", "broken\n")
    built = []

    def refusals(head: str, base: str) -> list[str]:
        built.append(head)
        return ["does not build as a whole: error[E0425]"] if head == bad else []

    heads = {"1": good, "2": bad}
    kept, dropped = ps.alone(["1", "2"], heads, refusals=refusals, cache=tmp_path / "cache")
    assert kept == ["1"] and "E0425" in dropped["2"] and "2" not in kept
    ps.alone(["1", "2"], heads, refusals=refusals, cache=tmp_path / "cache")
    assert built == [good, bad], built


def test_a_pr_that_merges_but_does_not_build_alone_is_refused_with_the_compiler_text(tmp_path):
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "bad", "b.rs", "fn(\n")
    sh(repo, "checkout", "-q", "main")
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "error: expected identifier", differs=lambda *_: set(), scanners=lambda _: set())
    assert why and "does not build as a whole" in why[0] and "expected identifier" in why[0], why


def test_two_prs_adding_a_test_of_one_name_do_not_share_a_batch(tmp_path):
    """#4678 and #4680 added the same test in different places: no conflict, so both joined, and the batch failed 'defined multiple times'."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "t.rs").write_text("fn first() {}\n\n\n\n\n\n\n\n\nfn last() {}\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    sh(repo, "checkout", "-qb", "a")
    (repo / "t.rs").write_text("fn first() {}\nfn a_frame_cell_is_not_encodable() {}\n\n\n\n\n\n\n\n\nfn last() {}\n")
    sh(repo, "commit", "-qam", "a")
    heads = {"1": sh(repo, "rev-parse", "HEAD")}
    sh(repo, "checkout", "-q", "main")
    sh(repo, "checkout", "-qb", "b")
    (repo / "t.rs").write_text("fn first() {}\n\n\n\n\n\n\n\n\nfn last() {}\nfn a_frame_cell_is_not_encodable() {}\n")
    sh(repo, "commit", "-qam", "b")
    heads["2"] = sh(repo, "rev-parse", "HEAD")
    sh(repo, "checkout", "-q", "main")
    kept, waits = ps.assemble(["1", "2"], heads, base="main", cwd=repo)
    assert kept == ["1"] and "a_frame_cell_is_not_encodable" in waits["2"] and "#1" in waits["2"]


def test_the_scanners_of_an_alone_check_run_in_the_scratch_tree_not_the_runners(tmp_path, monkeypatch):
    """The dry run printed `scanners: 2 targets, 0 red (tree 4d319defc)` for every PR: integration_failures defaulted to ROOT, so a PR red in its own tree passed."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "pr", "b.txt", "b\n")
    sh(repo, "checkout", "-q", "main")
    seen = []
    smoke = ps.parity_smoke_module()
    monkeypatch.setattr(smoke, "integration_failures", lambda targets, root=None, **_: seen.append(root) or set())
    assert ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: set()) == []
    assert seen == [repo], seen


def test_alone_counts_the_verdicts_it_read_from_the_cache(tmp_path):
    """The hit rate of the cache decides whether the filter pays: the second round reads every verdict of the first."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    heads = {"1": branch(repo, "x", "x.txt", "x\n")}
    ps.HITS[0] = 0
    ps.alone(["1"], heads, refusals=lambda h, b: [], cache=tmp_path / "cache")
    assert ps.HITS[0] == 0
    ps.alone(["1"], heads, refusals=lambda h, b: [], cache=tmp_path / "cache")
    assert ps.HITS[0] == 1


def test_a_pr_whose_only_issue_is_a_fmt_difference_is_kept(tmp_path):
    """The first `alone` precheck dropped #4736 and #4744 for `fmt.sh would change selectiondag/emit.rs`; the squash formats every PR, so the s3 pair started its batch without them."""
    repo = tmp_path / "repo"
    sh(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / "a.txt").write_text("a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    head = branch(repo, "pr", "emit.rs", "fn  f( ){}\n")
    sh(repo, "checkout", "-q", "main")
    why = ps.alone_refusals(head, base="main", cwd=repo, check=lambda cwd: "", differs=lambda *_: {"emit.rs"}, scanners=lambda _: set())
    assert why == [], why


def test_a_test_the_base_fails_too_blames_no_pr_and_probes_none(tmp_path):
    """Batch 2 (20:07) merged 16 PRs on a base red in two llrm-core tests and probed every PR alone: each fails the base's test, so each looked guilty."""
    probed = []
    by_package = {"llrm-core": {"dag_function_calls::a_red_test"}}
    found = ps.unit_offenders(
        [str(n) for n in range(1, 17)],
        by_package,
        merge_alone=lambda number: probed.append(number) or True,
        failing=lambda package, tests: True,
        base_red=lambda wanted: {("llrm-core", "dag_function_calls::a_red_test")},
    )
    assert found == [] and probed == [], (found, probed)


def test_a_pr_is_probed_only_for_a_package_it_can_change_and_a_new_red_test_is_still_blamed(tmp_path):
    graph = {"llrm-core": {"llrm-mir"}, "llrm-mir": set(), "llrm-x86": {"llrm-core"}, "llrm-nib": {"llrm-mir"}}
    packages = {name: Path("/r/crates") / name for name in graph}
    files = {"1": ["crates/llrm-nib/src/a.rs"], "2": ["crates/llrm-mir/src/b.rs"], "3": ["Cargo.toml"]}
    root = Path("/r")
    affects = lambda number, package: ps.can_affect(files[number], package, packages, graph, root)  # noqa: E731
    probed = []
    found = ps.unit_offenders(
        ["1", "2", "3"],
        {"llrm-core": {"t"}},
        merge_alone=lambda number: probed.append(number) or True,
        failing=lambda package, tests: probed[-1] == "2",
        affects=affects,
    )
    assert probed == ["2", "3"], probed  # 1 changes llrm-nib, which llrm-core does not use; 3 is a file no package owns
    assert found == ["2"], found
