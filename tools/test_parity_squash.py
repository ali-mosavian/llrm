"""#2329 squashed a stale head and reverted decide's LVI step (a82b332be): the squash must refuse what a bare `gh pr merge` merged."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402


def sh(repo: Path, *args: str) -> str:
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True)
    assert done.returncode == 0, done.stderr
    return done.stdout.strip()


def commit(repo: Path, name: str, text: str, message: str) -> str:
    (repo / name).write_text(text)
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", message)
    return sh(repo, "rev-parse", "HEAD")


TESTED = "#[test]\nfn keeps_lvi() {}\n\nfn other() {}\n"


def stale_merge(tmp_path: Path) -> tuple[Path, str]:
    """main gets a test file; a branch cut before it merges main and its resolution drops the test (the net diff deletes it)."""
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    commit(repo, "base.rs", "fn base() {}\n", "base")
    sh(repo, "checkout", "-q", "-b", "pr")
    commit(repo, "mine.rs", "fn mine() {}\n", "the PR's own work")
    sh(repo, "checkout", "-q", "main")
    commit(repo, "tests.rs", TESTED, "another PR lands a test")
    sh(repo, "checkout", "-q", "pr")
    sh(repo, "merge", "-q", "--no-commit", "main")
    (repo / "tests.rs").write_text("fn other() {}\n")  # the resolution that reverts the test
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "merge main")
    return repo, sh(repo, "rev-parse", "HEAD")


def test_a_stale_merge_that_reverts_a_test_is_refused(tmp_path):
    repo, head = stale_merge(tmp_path)
    why = ps.refusals(head, "", base="main", cwd=repo)
    assert why and "tests.rs" in why[0] and "1 test function" in why[0], why


def test_a_reverts_line_in_the_description_allows_it(tmp_path):
    repo, head = stale_merge(tmp_path)
    assert ps.refusals(head, "Reverts: the lvi test, it moved to lvi_tests.rs", base="main", cwd=repo) == []


def test_a_head_that_lacks_current_base_is_refused(tmp_path):
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    commit(repo, "a.rs", "fn a() {}\n", "a")
    sh(repo, "checkout", "-q", "-b", "pr")
    head = commit(repo, "b.rs", "fn b() {}\n", "b")
    sh(repo, "checkout", "-q", "main")
    commit(repo, "c.rs", "fn c() {}\n", "main moved")
    why = ps.refusals(head, "", base="main", cwd=repo)
    assert why and "does not contain main" in why[0]


def test_a_clean_head_with_its_own_changes_passes(tmp_path):
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    commit(repo, "t.rs", TESTED, "base")
    sh(repo, "checkout", "-q", "-b", "pr")
    head = commit(repo, "t.rs", "#[test]\nfn keeps_lvi() {}\n\nfn other() { 1; }\n", "my change to a file i touch")
    assert ps.refusals(head, "", base="main", cwd=repo) == []


def test_removed_test_functions_are_counted_in_rust_and_python():
    diff = "-#[test]\n-fn a() {}\n+x\n-def test_b():\n-    pass\n- def helper():\n"
    assert ps.deleted_tests(diff) == 2


def test_known_red_names_a_smoke_step_and_an_issue_and_goes_in_the_body_as_one_line():
    known = ps.parse_known_red(["sc=2438"])
    assert known == {"sc": "#2438"} and ps.known_red_line(known) == "Known red: sc (#2438)" and ps.known_red_line({}) == ""
    for bad in ("sc", "nope=2438", "sc=x"):
        try:
            ps.parse_known_red([bad])
        except SystemExit:
            continue
        raise AssertionError(f"{bad!r} was accepted")


def test_the_smoke_binaries_default_to_the_release_dir_when_all_three_are_there(tmp_path):
    """backend-slopes could not run the tool without PARITY_BIN: unset, it finds the release dir, or says what to build."""
    release = tmp_path / "release"
    release.mkdir()
    env = {"LLRM_BIN": str(release)}
    try:
        ps.smoke_bins(env)
    except SystemExit as why:
        assert "lacks llrm-c, llrm-qb, llrm-nib" in str(why)
    else:
        raise AssertionError("an empty release dir was accepted")
    for name in ps.BINARIES:
        (release / name).write_text("")
    assert ps.smoke_bins(env) == str(release)
    assert ps.smoke_bins({**env, "PARITY_BIN": "/x"}) == "/x"


def squash_of_a_stale_tree(tmp_path: Path):
    """#2443: the PR's own commit carried a tree older than #2429 and deleted scev/expand_iv.rs (855 lines) and its tests while it aligned functions."""
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    (repo / "scev").mkdir()
    commit(repo, "emission.rs", "fn align() {}\n", "base")
    commit(repo, "scev/expand_iv.rs", "".join(f"fn expand_{i}() {{}}\n" for i in range(60)) + "#[test]\nfn expands() {}\n", "#2429 lands")
    sh(repo, "checkout", "-q", "-b", "pr")
    (repo / "scev/expand_iv.rs").unlink()  # the stale tree: this file is not there
    (repo / "emission.rs").write_text("fn align() { 16 }\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "feat(emission): align functions")
    return repo, sh(repo, "rev-parse", "HEAD")


def test_an_own_commit_that_deletes_a_file_and_its_tests_is_refused_unless_the_description_names_it(tmp_path):
    repo, head = squash_of_a_stale_tree(tmp_path)
    why = ps.refusals(head, "", base="main", cwd=repo)
    assert why and "scev/expand_iv.rs" in why[0] and "whole file" in why[0] and "1 test function" in why[0], why
    assert ps.refusals(head, "Deletes: scev/expand_iv.rs, the old expander", base="main", cwd=repo) == []


def test_a_rewritten_generated_page_is_not_a_deletion(tmp_path):
    """#2760, #2873 and #2925 were refused as 'deletes docs/optimizations/backend/jumps.md' (30-35 lines) for a regenerated page: three PRs sat out of the queue."""
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    page = "> Generated from the `//!` header of [`a.rs`](x); edit the header, not this file.\n" + "".join(f"line {i}\n" for i in range(60))
    hand = "".join(f"line {i}\n" for i in range(60))
    commit(repo, "gen.md", page, "page")
    commit(repo, "hand.md", hand, "hand page")
    sh(repo, "checkout", "-q", "-b", "pr")
    (repo / "gen.md").write_text(page.split("\n", 1)[0] + "\nshort\n")
    (repo / "hand.md").write_text("short\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "docs: rewrite")
    why = ps.refusals(sh(repo, "rev-parse", "HEAD"), "", base="main", cwd=repo)
    assert len(why) == 1 and why[0].startswith("hand.md"), why


def test_a_squash_that_touches_files_the_pr_never_listed_is_detected_after_landing(tmp_path):
    repo, head = squash_of_a_stale_tree(tmp_path)
    sh(repo, "checkout", "-q", "main")
    sh(repo, "merge", "-q", "--squash", "pr")
    sh(repo, "commit", "-qm", "squash")
    squash = sh(repo, "rev-parse", "HEAD")
    assert ps.squash_strays(squash, {"emission.rs"}, cwd=repo) == [("scev/expand_iv.rs", "-62/+0")]
    assert ps.squash_strays(squash, {"emission.rs", "scev/expand_iv.rs"}, cwd=repo) == []


def test_the_base_is_read_again_at_the_merge_call(tmp_path):
    """feat/parity moved while the fast set ran; a containment check from the start of the run missed it."""
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    commit(repo, "a.rs", "fn a() {}\n", "a")
    sh(repo, "checkout", "-q", "-b", "pr")
    head = commit(repo, "b.rs", "fn b() {}\n", "b")
    sh(repo, "checkout", "-q", "main")
    assert ps.stale_at_the_call(head, fetch=lambda: None, base="main", cwd=repo) == [], "a head that contains the base is fine"
    moved = lambda: commit(repo, "c.rs", "fn c() {}\n", "main moved while the checks ran")  # noqa: E731
    why = ps.stale_at_the_call(head, fetch=moved, base="main", cwd=repo)
    assert why and "moved while the checks ran" in why[0], why


def make_pair(tmp_path):
    """A git repo with a cargo workspace and two branches off one base, each compiling beside the base, not together: A makes `undef` take `&mut Context`
    (and fixes its caller), B adds a second caller with a `&Context`. origin/feat/parity is A, landed."""
    import subprocess as sp

    def run(*cmd):
        sp.run(cmd, cwd=tmp_path, check=True, capture_output=True, text=True, env={**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"})

    (tmp_path / "lib/src").mkdir(parents=True)
    (tmp_path / "app/src").mkdir(parents=True)
    (tmp_path / "Cargo.toml").write_text('[workspace]\nmembers = ["lib", "app"]\nresolver = "2"\n')
    (tmp_path / "lib/Cargo.toml").write_text('[package]\nname = "lib"\nversion = "0.0.0"\nedition = "2021"\n')
    (tmp_path / "app/Cargo.toml").write_text('[package]\nname = "app"\nversion = "0.0.0"\nedition = "2021"\n[dependencies]\nlib = { path = "../lib" }\n')
    (tmp_path / "lib/src/lib.rs").write_text("pub struct Context;\npub fn undef(_c: &Context) {}\n")
    (tmp_path / "app/src/lib.rs").write_text("pub fn fold(c: &lib::Context) { lib::undef(c); }\n")
    run("git", "init", "-q", "-b", "base")
    run("git", "add", "-A")
    run("git", "commit", "-q", "-m", "base")
    run("git", "checkout", "-q", "-b", "b")
    (tmp_path / "app/src/bin").mkdir()
    (tmp_path / "app/src/bin/b.rs").write_text("fn main() { let c = lib::Context; lib::undef(&c); }\n")
    run("git", "add", "-A")
    run("git", "commit", "-q", "-m", "b adds a caller")
    run("git", "checkout", "-q", "base")
    (tmp_path / "lib/src/lib.rs").write_text("pub struct Context;\npub fn undef(_c: &mut Context) {}\n")
    (tmp_path / "app/src/lib.rs").write_text("pub fn fold(c: &mut lib::Context) { lib::undef(c); }\n")
    run("git", "commit", "-q", "-am", "a changes the signature")
    run("git", "update-ref", "refs/remotes/origin/feat/parity", "HEAD")
    return run


def test_two_prs_that_compile_apart_and_not_together_are_refused_the_second_time(tmp_path):
    """#2544 and #2564 each compiled; landed back to back, feat/parity did not build for three hours. The PR is tested merged with the base it lands on."""
    import parity_smoke

    make_pair(tmp_path)
    head = subprocess.run(["git", "rev-parse", "b"], cwd=tmp_path, capture_output=True, text=True).stdout.strip()
    why = ps.merged_refusals(head, base="origin/feat/parity", check=parity_smoke.workspace_check, cwd=tmp_path)
    assert why and "does not build as a whole" in why[0], why
    alone = subprocess.run(["git", "rev-parse", "base~0"], cwd=tmp_path, capture_output=True, text=True).stdout.strip()
    assert ps.merged_refusals(alone, base="origin/feat/parity", check=parity_smoke.workspace_check, cwd=tmp_path) == []


def test_a_second_landing_waits_for_the_first_one_to_release_the_lock(tmp_path):
    lock = str(tmp_path / "land.lock")
    with ps.land_lock(lock):
        try:
            with ps.land_lock(lock, wait=False):
                raise AssertionError("two landings held the lock at once")
        except BlockingIOError:
            pass
    with ps.land_lock(lock, wait=False):
        pass


def test_a_failing_pr_in_four_is_isolated_and_the_other_three_land():
    """One smoke on the combined tree; when it fails the set is halved: 1+2 pass and land, 3 fails alone and is refused, 4 is tested on top and lands."""
    landed, refused, tested = [], [], []

    def passes(group):
        tested.append(list(group))
        return "3" not in group

    ps.batch(["1", "2", "3", "4"], passes, landed.append, refused.append)
    assert landed == ["1", "2", "4"] and refused == ["3"], (landed, refused)
    assert tested[0] == ["1", "2", "3", "4"] and ["1", "2"] in tested and ["3"] in tested and ["4"] in tested


def test_four_passing_prs_cost_one_test_and_land_in_order():
    landed, tested = [], []
    ps.batch(["1", "2", "3", "4"], lambda group: (tested.append(group), True)[1], landed.append, lambda pr: None)
    assert landed == ["1", "2", "3", "4"] and len(tested) == 1


def test_a_fix_for_a_red_head_is_accepted_and_a_new_failure_is_refused():
    """The gate deadlocked on a red head: the fix for it failed the same checks the head did, so it could not land. The merged tree may fail a SUBSET of the head's failures."""
    red = {"ratchet:block_list_ratchet", "qc_modules"}
    assert ps.subset_refusals(red, {"qc_modules"}, fixes=True) == []
    assert ps.subset_refusals(red, set(), fixes=True) == []
    why = ps.subset_refusals(red, {"qc_modules", "run"}, fixes=False)
    assert why and "run" in why[0] and "ratchet:block_list_ratchet" in why[0] and "qc_modules" in why[0], why
    assert ps.subset_refusals(set(), {"run"}, fixes=False)


def test_an_equal_failure_set_lands_only_without_the_fixes_claim():
    red = {"qc_modules"}
    assert ps.subset_refusals(red, set(red), fixes=False) == []
    assert ps.subset_refusals(red, set(red), fixes=True)
    assert ps.claims_fix("Fixes red: the ratchet\n") and ps.claims_fix("TL;DR\nFixes a red head\n") and not ps.claims_fix("This fixes reds nothing")


def test_the_head_failure_set_is_computed_once_per_sha_and_never_guessed(tmp_path):
    import pytest

    calls = []
    assert ps.head_failures("abc", lambda: (calls.append(1), {"x"})[1], tmp_path) == {"x"}
    assert ps.head_failures("abc", lambda: (calls.append(1), {"y"})[1], tmp_path) == {"x"} and len(calls) == 1
    with pytest.raises(SystemExit, match="no failure set is cached"):
        ps.head_failures("other", None, tmp_path)


def test_a_feat_parity_commit_absent_from_the_landing_log_is_listed(tmp_path):
    """Direct pushes bypassed the lock; the log is what every landing writes, so a commit with no line in it is a landing that did not go through the squash."""
    log = tmp_path / "landed.log"
    ps.log_landed("aaaaaaa1111", "2700", "gate-ok", log)
    ps.log_landed("bbbbbbb2222", "2701", "gate-ok", log)
    assert ps.unlogged(log, ["aaaaaaa", "ccccccc", "bbbbbbb"]) == ["ccccccc"]
    assert ps.unlogged(tmp_path / "none.log", ["ddddddd"]) == ["ddddddd"]


def test_bins_older_than_the_merged_tree_are_rebuilt_or_named_not_an_empty_refusal(tmp_path, monkeypatch):
    """The merge rewrote files after the binaries were built: the smoke exited non-zero with an empty last line and the tool printed 'the fast set fails' with no reason."""
    import llrmbin

    stale = [["llrm-c"]]
    monkeypatch.setattr(llrmbin, "stale_binaries", lambda directory, repo=None: stale[0])
    monkeypatch.setattr(ps, "smoke_bins", lambda env=None: str(tmp_path))
    built = []

    def fresh():
        built.append(1)
        stale[0] = []

    assert ps.ensure_bins(build=fresh) == "" and built == [1]
    stale[0] = ["llrm-c", "llrm-qb"]
    why = ps.ensure_bins(build=lambda: built.append(2))
    assert "still older" in why and "llrm-c, llrm-qb" in why, why
    stale[0] = []
    assert ps.ensure_bins(build=lambda: built.append(3)) == "" and 3 not in built


def test_a_smoke_that_exits_non_zero_without_a_summary_line_names_its_stderr(monkeypatch):
    class Done:
        returncode, stdout, stderr = 1, "", "stale binaries in /x: llrm-c older than the source\nrun cargo build"

    monkeypatch.setattr(ps, "smoke_bins", lambda env=None: "/x")
    monkeypatch.setattr(ps.subprocess, "run", lambda *a, **k: Done())
    failed, last = ps.smoke_failures()
    assert failed == {"smoke-did-not-finish"} and "stale binaries in /x" in last and "exited 1" in last, last


def test_a_new_failing_test_inside_a_red_crate_is_refused_and_one_the_head_fails_too_is_not():
    """Head red in llrm-qb (test_a); a PR adds test_b that fails: the check-level rule saw `unit llrm-qb` fail on both sides and landed it."""
    head = {"unit llrm-qb::test_a"}
    merged = {"unit llrm-qb::test_a", "unit llrm-qb::test_b"}
    on_head = lambda crate, tests: {t for t in tests if t == "test_a"}  # test_b passes on the head
    assert ps.subset_refusals(ps.head_expanded(head, merged, on_head), merged, False) != []
    assert ps.subset_refusals(ps.head_expanded(head, merged, lambda crate, tests: set(tests)), merged, False) == []  # the head fails test_b too


def test_a_refusal_after_a_rebuild_names_the_newest_source_the_oldest_binary_and_a_failed_build(monkeypatch):
    """#2764 was refused "binaries still older than the merged tree after a rebuild" with nothing to say whether the build failed or a source was newer."""
    import llrmbin

    monkeypatch.setattr(ps, "smoke_bins", lambda env=None: "/x")
    monkeypatch.setattr(llrmbin, "stale_binaries", lambda directory, cwd=None: ["llrm-c", "llrm-qb"])
    monkeypatch.setattr(llrmbin, "stale_found", lambda directory, repo=None: [("llrm-c", 100.0, "crates/a/src/x.rs", 160.0), ("llrm-qb", 90.0, "crates/b/src/y.rs", 130.0)])

    class Failed:
        returncode, stderr, stdout = 101, "error[E0425]: cannot find type\nerror: could not compile `llrm-nib`", ""

    why = ps.ensure_bins(build=lambda: Failed())
    assert "oldest binary llrm-qb" in why and "newest source crates/a/src/x.rs" in why and "70 s later" in why, why
    assert "the rebuild exited 101" in why and "could not compile `llrm-nib`" in why, why


GENERATOR = """import sys
from pathlib import Path
root = Path(__file__).resolve().parents[2]
pages = sorted(p.stem for p in (root / "docs/optimizations/passes").glob("page_*.md"))
want = "".join(f"- {name}\\n" for name in ["a", *pages])
index = root / "docs/optimizations/passes/readme.md"
if "--sync" in sys.argv:
    index.write_text(want)
elif "--check" in sys.argv and index.read_text() != want:
    print("stale index"); sys.exit(1)
"""


def generated_repo(tmp_path: Path) -> Path:
    repo = tmp_path / "r"
    (repo / "tools/docsbuild").mkdir(parents=True)
    (repo / "docs/optimizations/passes").mkdir(parents=True)
    sh(repo, "init", "-q", "-b", "main")
    (repo / "tools/docsbuild/build.py").write_text(GENERATOR)
    (repo / "docs/optimizations/passes/readme.md").write_text("- a\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "base")
    return repo


def test_two_prs_that_each_add_a_page_merge_by_regenerating_the_index(tmp_path):
    """Both PRs rewrote the generated index at the same line: git conflicted, a human merged generated text, and #2790, the LSR stack and indvars waited on it."""
    repo = generated_repo(tmp_path)
    sh(repo, "checkout", "-q", "-b", "pr")
    (repo / "docs/optimizations/passes/page_c.md").write_text("# c\n")
    (repo / "docs/optimizations/passes/readme.md").write_text("- a\n- page_c\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "c")
    head = sh(repo, "rev-parse", "HEAD")
    sh(repo, "checkout", "-q", "main")
    (repo / "docs/optimizations/passes/page_b.md").write_text("# b\n")
    (repo / "docs/optimizations/passes/readme.md").write_text("- a\n- page_b\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "b")
    sha, why = ps.merged_result(head, base="main", cwd=repo)
    assert why == "" and sha, why
    assert (repo / "docs/optimizations/passes/readme.md").read_text() == "- a\n- page_b\n- page_c\n"
    assert sh(repo, "status", "--porcelain") == ""


def test_a_conflict_in_a_file_nothing_generates_is_still_refused(tmp_path):
    repo = generated_repo(tmp_path)
    (repo / "x.rs").write_text("fn x() {}\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "x")
    sh(repo, "checkout", "-q", "-b", "pr")
    (repo / "x.rs").write_text("fn x() { 1 }\n")
    sh(repo, "commit", "-qam", "pr x")
    head = sh(repo, "rev-parse", "HEAD")
    sh(repo, "checkout", "-q", "main")
    (repo / "x.rs").write_text("fn x() { 2 }\n")
    sh(repo, "commit", "-qam", "main x")
    sha, why = ps.merged_result(head, base="main", cwd=repo)
    assert sha is None and "x.rs" in why


def test_eight_passing_prs_cost_one_smoke_and_nine_are_refused(monkeypatch):
    """Batches of four took 12-16 minutes each with ~38 PRs ready: eight PRs ride one smoke, and a failing eight is halved to its bad PR in three more."""
    landed, tested = [], []
    ps.batch([str(n) for n in range(1, 9)], lambda group: (tested.append(group), True)[1], landed.append, lambda pr: None)
    assert landed == [str(n) for n in range(1, 9)] and len(tested) == 1
    landed, refused, tested = [], [], []
    ps.batch([str(n) for n in range(1, 9)], lambda group: (tested.append(group), "6" not in group)[1], landed.append, refused.append)
    assert refused == ["6"] and landed == ["1", "2", "3", "4", "5", "7", "8"] and len(tested) == 7
    with pytest.raises(SystemExit, match="at most 8"):
        ps.merge_batch([str(n) for n in range(1, 10)])


class Tree:
    """A fake dev checkout: `state` is "head" until the merge, then "merged"; each check reports the files the state fails."""

    def __init__(self, head_fmt: set[str], merged_fmt: set[str], ratchets: set[str] = frozenset()):
        self.state, self.head_fmt, self.merged_fmt, self.ratchets = "head", head_fmt, merged_fmt, ratchets

    def merge(self, head, base, cwd):
        self.state = "merged"
        return "deadbeef", ""

    def checks(self):
        fmt = lambda root: {"fmt": set(self.head_fmt if self.state == "head" else self.merged_fmt)} if (self.head_fmt if self.state == "head" else self.merged_fmt) else {}
        rat = lambda root: {name: {name} for name in self.ratchets} if self.state == "merged" else {}
        return [("fmt", fmt), ("ratchets", rat)]


def run_precheck(tmp_path, monkeypatch, tree, number="2836", **extra):
    dev = tmp_path / "wt"
    dev.mkdir(exist_ok=True)
    monkeypatch.setattr(ps, "git", lambda *args, cwd=None: "")
    monkeypatch.setattr(ps, "ensure_bins", lambda *a, **k: (_ for _ in ()).throw(AssertionError("a dist build")))
    info = {"headRefOid": f"head{number}", "body": "", "baseRefName": "feat/parity"}
    return ps.precheck(number, dev=dev, checks=tree.checks(), merge=tree.merge, deletions=lambda *a: [], info=info, cache=tmp_path / f"cache{number}.json", base_sha="base", **extra)


def test_precheck_refuses_a_pr_with_a_dense_id_hash_map_without_any_dist_build(tmp_path, monkeypatch):
    """#2836 added a hash map keyed by a dense id and cost a batch 26 minutes of halving: the seeded-maps ratchet names it in seconds, with no dist build."""
    line = run_precheck(tmp_path, monkeypatch, Tree(set(), set(), {"ratchet:seeded_maps"}))
    assert line.startswith("2836: REFUSED: ratchet:seeded_maps"), line
    assert run_precheck(tmp_path, monkeypatch, Tree(set(), set()), number="2837") == "2837: PASS"


def test_a_red_head_file_refuses_neither_a_clean_pr_nor_the_fix_but_a_new_red_file_is_named(tmp_path, monkeypatch):
    """precheck refused five PRs for fmt files that feat/parity itself fails (saverestore.rs, flow.rs ...): with any red head file nothing passed, the fixes included."""
    assert run_precheck(tmp_path, monkeypatch, Tree({"a.rs"}, {"a.rs"}), number="1") == "1: PASS"  # head red in A, PR adds none
    assert run_precheck(tmp_path, monkeypatch, Tree({"a.rs"}, set()), number="2") == "2: PASS"  # the PR fixes A
    line = run_precheck(tmp_path, monkeypatch, Tree({"a.rs"}, {"a.rs", "b.rs"}), number="3")  # the PR adds B
    assert line.startswith("3: REFUSED: fmt") and "b.rs" in line and "a.rs" not in line, line


def test_a_precheck_logs_the_seconds_each_check_took(tmp_path, monkeypatch):
    """A precheck pass took ~16 minutes for 16 PRs and nothing said where: the ratchets, the merge or fmt. One log line per PR now names each."""
    run_precheck(tmp_path, monkeypatch, Tree(set(), set()), number="7")
    row = json.loads((tmp_path / "precheck-times.log").read_text().splitlines()[-1])
    assert row["pr"] == "7" and {"merge", "fmt", "ratchets"} <= row.keys(), row


def test_a_precheck_verdict_is_cached_per_pr_head_and_feat_parity_sha(tmp_path, monkeypatch):
    """The runner asks about ~40 candidates per cycle: the same (head, base) must not merge and check again, a moved feat/parity must."""
    tree, merges = Tree(set(), set()), []
    original = tree.merge
    tree.merge = lambda head, base, cwd: (merges.append(head), original(head, base, cwd))[1]
    dev = tmp_path / "wt"
    dev.mkdir()
    monkeypatch.setattr(ps, "git", lambda *args, cwd=None: "")
    info = {"headRefOid": "abc", "body": "", "baseRefName": "feat/parity"}
    args = dict(dev=dev, checks=tree.checks(), merge=tree.merge, deletions=lambda *a: [], info=info, cache=tmp_path / "cache.json")
    assert ps.precheck("5", base_sha="p1", **args) == "5: PASS" and ps.precheck("5", base_sha="p1", **args) == "5: PASS" and len(merges) == 1
    assert ps.precheck("5", base_sha="p2", **args) == "5: PASS" and len(merges) == 2
    assert ps.precheck("9", base_sha="p2", **args) == "9: PASS" and len(merges) == 2  # the verdict is for the head sha, whichever number asks
