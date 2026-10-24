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
from types import SimpleNamespace  # noqa: E402


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


def test_a_stale_merge_that_reverts_a_test_is_refused(tmp_path, monkeypatch):
    monkeypatch.setattr(ps, "BODY_CHECKS", "refuse")  # the switch turned back on: the description checks refuse
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


def test_an_own_commit_that_deletes_a_file_and_its_tests_is_refused_unless_the_description_names_it(tmp_path, monkeypatch):
    monkeypatch.setattr(ps, "BODY_CHECKS", "refuse")  # the switch turned back on: the description checks refuse
    repo, head = squash_of_a_stale_tree(tmp_path)
    why = ps.refusals(head, "", base="main", cwd=repo)
    assert why and "scev/expand_iv.rs" in why[0] and "whole file" in why[0] and "1 test function" in why[0], why
    assert ps.refusals(head, "Deletes: scev/expand_iv.rs, the old expander", base="main", cwd=repo) == []


def test_a_rewritten_generated_page_is_not_a_deletion(tmp_path, monkeypatch):
    """#2760, #2873 and #2925 were refused as 'deletes docs/optimizations/backend/jumps.md' (30-35 lines) for a regenerated page: three PRs sat out of the queue."""
    monkeypatch.setattr(ps, "BODY_CHECKS", "refuse")  # the switch turned back on: the description checks refuse
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
    monkeypatch.setattr(ps, "run_streaming", lambda command, cwd: Done())
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


def test_the_smoke_stage_lines_reach_the_squash_log_as_they_happen(capsys):
    """The 16:16 batch logged every stage at 16:23:48: the log was printed at the end. A STAGE line of the smoke is echoed the moment it arrives."""
    import sys

    child = [sys.executable, "-c", "import sys,time\nprint('12:00:00 STAGE step programs', flush=True)\ntime.sleep(0.3)\nprint('FAILING:')\nprint('PARITY SMOKE PASS x')"]
    done = ps.run_streaming(child, Path("."))
    shown = capsys.readouterr().out
    assert shown.strip() == "12:00:00 STAGE step programs" and done.returncode == 0 and done.stdout.splitlines()[-1] == "PARITY SMOKE PASS x"


def test_every_squash_stage_prints_a_flushed_timestamped_line_before_its_work(capsys, monkeypatch):
    import stages

    stages.stage("build")
    line = capsys.readouterr().out
    assert line.strip().endswith(" STAGE build") and line[2] == ":" and line[5] == ":"
    monkeypatch.setattr(ps, "smoke_bins", lambda env=None: "/x")
    monkeypatch.setattr(ps, "run_streaming", lambda command, cwd: type("D", (), {"returncode": 0, "stdout": "FAILING:\nPARITY SMOKE PASS x\n", "stderr": ""})())
    ps.smoke_failures()
    assert " STAGE smoke" in capsys.readouterr().out


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


def test_a_red_head_file_no_longer_excuses_an_unformatted_pr_file_but_the_fix_passes(tmp_path, monkeypatch):
    """The user's order: tools/fmt.sh applies to every PR. precheck had excused any fmt failure the head already had (92 files of parity were unformatted that way): a PR that leaves a head-red file unformatted is refused,
    the PR that formats it passes, and a new red file is named as before. Ratchets keep the old excuse."""
    line = run_precheck(tmp_path, monkeypatch, Tree({"a.rs"}, {"a.rs"}), number="1")  # head red in A, the PR leaves A red
    assert line.startswith("1: REFUSED: fmt") and "a.rs" in line, line
    assert run_precheck(tmp_path, monkeypatch, Tree({"a.rs"}, set()), number="2") == "2: PASS"  # the PR formats A
    line = run_precheck(tmp_path, monkeypatch, Tree({"a.rs"}, {"a.rs", "b.rs"}), number="3")  # the PR adds B
    assert line.startswith("3: REFUSED: fmt") and "b.rs" in line, line


def test_a_precheck_logs_the_seconds_each_check_took(tmp_path, monkeypatch):
    """A precheck pass took ~16 minutes for 16 PRs and nothing said where: the ratchets, the merge or fmt. One log line per PR now names each."""
    run_precheck(tmp_path, monkeypatch, Tree(set(), set()), number="7")
    row = json.loads((tmp_path / "precheck-times.log").read_text().splitlines()[-1])
    assert row["pr"] == "7" and {"merge", "fmt", "ratchets", "load"} <= row.keys(), row  # the load average: 150 s of ratchets at load 44 is not 57 s at load 30


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


def test_a_fast_batch_leaves_out_the_pr_that_cannot_join_and_keeps_the_rest():
    """The user's call: merge everything, skip the tests. One PR that conflicts or deletes unnamed code is left out with its reason; it does not fail or halve the batch."""
    why = {"2": "conflicts with an earlier PR of the batch", "4": "deletes 31 lines"}
    kept, left_out = ps.fast_merge(["1", "2", "3", "4", "5"], lambda number: why.get(number, ""))
    assert kept == ["1", "3", "5"] and left_out == why, (kept, left_out)


def test_the_precheck_formats_only_the_prs_own_rust_files(tmp_path, monkeypatch):
    """fmt --check over the whole tree took ~13 s of a ~70 s precheck for a PR that touches a handful of files; the PR side checks only its changed .rs files."""
    for name, text in {"ok.rs": "fn a() {}\n", "bad.rs": "fn  b( ) {}\n", "notes.md": "x\n"}.items():
        (tmp_path / name).write_text(text)

    class Done:
        def __init__(self, out, code=0):
            self.stdout, self.returncode = out, code

    run = lambda cmd, cwd=None, input=None, **kw: Done("fn b() {}\n" if cmd[-1] == "bad.rs" else input)  # noqa: E731
    assert ps.fmt_differs(tmp_path, ["ok.rs", "bad.rs", "notes.md", "gone.rs"], run=run) == {"bad.rs"}

    seen = []

    def checks_with_only():
        def fmt(root, only=None):
            seen.append(only)
            return {}

        fmt.takes_only = True
        return [("fmt", fmt)]

    ps.precheck_items(tmp_path, checks_with_only(), only=["a.rs"])
    assert seen == [["a.rs"]], seen


def test_a_stray_untracked_page_does_not_turn_a_pr_that_adds_it_into_a_conflict(tmp_path):
    """combine_demanded.md, reassociate.md and reassociate_fneg.md sat untracked in the gate checkout after a docs regeneration; every PR adding them was refused as a 'conflict' for hours."""
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    commit(repo, "a.rs", "fn a() {}\n", "base")
    sh(repo, "checkout", "-q", "-b", "pr")
    commit(repo, "page.md", "from the pr\n", "pr adds the page")
    sh(repo, "checkout", "-q", "main")
    (repo / "page.md").write_text("left by a regeneration\n")  # untracked, the file the PR adds
    ps.clean_checkout("main", cwd=repo)
    merge = subprocess.run(["git", "merge", "--no-edit", "-q", "pr"], cwd=repo, capture_output=True, text=True)
    assert merge.returncode == 0 and (repo / "page.md").read_text() == "from the pr\n", merge.stderr
def test_fast_mode_lets_a_refactor_delete_lines_but_not_a_file(tmp_path, monkeypatch):
    """#2855, #2856, #2881, #2942, #2958, #3010 and #3028 sat out a 02:37 batch: each deleted 21-76 lines of its own file, and nothing landed for 27 minutes."""
    monkeypatch.setattr(ps, "BODY_CHECKS", "refuse")  # the switch turned back on: the description checks refuse
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    commit(repo, "a.rs", "".join(f"fn f{i}() {{}}\n" for i in range(60)), "base")
    sh(repo, "checkout", "-q", "-b", "pr")
    (repo / "a.rs").write_text("fn f0() {}\n")
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "refactor(a): shrink")
    head = sh(repo, "rev-parse", "HEAD")
    monkeypatch.delenv("LAND_FAST", raising=False)
    assert ps.refusals(head, "", base="main", cwd=repo)
    monkeypatch.setenv("LAND_FAST", "1")
    assert ps.refusals(head, "", base="main", cwd=repo) == []
    (tmp_path / "s").mkdir()
    stale, stale_head = squash_of_a_stale_tree(tmp_path / "s")
    assert ps.refusals(stale_head, "", base="main", cwd=stale)


def test_a_pr_whose_only_conflict_is_a_generated_page_is_brought_up_to_date_on_its_branch(tmp_path, monkeypatch):
    """#2878, #2872, #2941, #2943, #2949 and #2951 conflicted only in an index readme: regenerated locally, but `gh pr merge` refused them as unmergeable at 03:02, so none landed."""
    origin = tmp_path / "origin.git"
    sh(tmp_path, "init", "-q", "--bare", "-b", "main", str(origin))
    work = tmp_path / "w"
    sh(tmp_path, "clone", "-q", str(origin), str(work))
    commit(work, "index.md", "a\n", "base")
    sh(work, "push", "-q", "origin", "HEAD:main")
    sh(work, "checkout", "-q", "-b", "pr")
    head = commit(work, "index.md", "a\npr\n", "pr adds a page")
    sh(work, "push", "-q", "origin", "pr")
    sh(work, "checkout", "-q", "main")
    commit(work, "index.md", "a\nmain\n", "main adds a page")
    sh(work, "push", "-q", "origin", "main")

    def regenerate(conflicts, cwd):
        sh(cwd, "checkout", "--ours", "--", *conflicts)
        sh(cwd, "add", "-A")
        sh(cwd, "commit", "-q", "--no-edit")
        return ""

    monkeypatch.setattr(ps, "regenerate", regenerate)
    fresh = ps.update_generated_only(head, "pr", base="origin/main", cwd=work)
    assert fresh and fresh != head
    sh(work, "fetch", "-q", "origin")
    assert sh(work, "rev-parse", "origin/pr") == fresh
    sh(work, "merge-base", "--is-ancestor", "origin/main", fresh)


def test_a_pr_with_a_real_conflict_is_not_pushed_to(tmp_path, monkeypatch):
    origin = tmp_path / "origin.git"
    sh(tmp_path, "init", "-q", "--bare", "-b", "main", str(origin))
    work = tmp_path / "w"
    sh(tmp_path, "clone", "-q", str(origin), str(work))
    commit(work, "code.rs", "a\n", "base")
    sh(work, "push", "-q", "origin", "HEAD:main")
    sh(work, "checkout", "-q", "-b", "pr")
    head = commit(work, "code.rs", "a\npr\n", "pr")
    sh(work, "push", "-q", "origin", "pr")
    sh(work, "checkout", "-q", "main")
    commit(work, "code.rs", "a\nmain\n", "main")
    sh(work, "push", "-q", "origin", "main")
    monkeypatch.setattr(ps, "regenerate", lambda conflicts, cwd: "conflicts in files nothing generates: " + ", ".join(conflicts))
    assert ps.update_generated_only(head, "pr", base="origin/main", cwd=work) == ""
    sh(work, "fetch", "-q", "origin")
    assert sh(work, "rev-parse", "origin/pr") == head


def test_a_generated_only_conflict_pr_is_not_refused_at_the_squash(monkeypatch):
    """The 03:02 symptom: six PRs the runner had regenerated were refused at `gh pr merge` (GitHub: not mergeable) because their branches never got the merge."""
    monkeypatch.setattr(ps, "FAST", True)
    calls = []

    def run(cmd, **_):
        calls.append(cmd)
        if cmd[:3] == ["gh", "pr", "merge"]:
            return subprocess.CompletedProcess(cmd, 0 if cmd[cmd.index("--match-head-commit") + 1] == "newsha" else 1)
        return subprocess.CompletedProcess(cmd, 0)

    info = {"headRefOid": "oldsha", "headRefName": "pr", "title": "t", "mergeable": "CONFLICTING"}
    assert ps.squash("2878", info, "", run=run, update=lambda head, branch: "newsha", settle=0)
    comment = next(c for c in calls if c[:3] == ["gh", "pr", "comment"])
    assert "never force" in comment[-1] and "newsha"[:9] in comment[-1]
    assert not ps.squash("2872", info, "", run=run, update=lambda head, branch: "", settle=0)


def named_assistant():
    return "Co-Authored-By: " + "Clau" + "de <noreply@example.com>"


def sub(tmp_path, name):
    path = tmp_path / name
    path.mkdir()
    return path


def pr_over_main(tmp_path, file_text="fn a() {}\n", message="feat: a"):
    repo = tmp_path / "r"
    repo.mkdir()
    sh(repo, "init", "-q", "-b", "main")
    commit(repo, "a.rs", "fn base() {}\n", "base")
    sh(repo, "checkout", "-q", "-b", "pr")
    head = commit(repo, "a.rs", file_text, message)
    return repo, head


def test_a_pr_that_names_the_assistant_is_refused_wherever_it_does(tmp_path):
    """The user's rule: nothing in the repository, its PRs or its history names the assistant. The runner had no check; #3080's description did."""
    repo, head = pr_over_main(tmp_path)
    assert ps.refusals(head, "", base="main", cwd=repo, title="feat: a") == []
    assert any("title" in w for w in ps.refusals(head, "", base="main", cwd=repo, title="feat: " + "Clau" + "de did it"))
    assert any("description" in w for w in ps.refusals(head, "🤖 Generated with " + "Clau" + "de Code", base="main", cwd=repo))
    repo2, head2 = pr_over_main(sub(tmp_path, "m"), message="feat: a\n\n" + named_assistant())
    assert any("commit" in w for w in ps.refusals(head2, "", base="main", cwd=repo2))
    repo3, head3 = pr_over_main(sub(tmp_path, "l"), file_text="// written by " + "Clau" + "de\nfn a() {}\n")
    assert any("added line" in w for w in ps.refusals(head3, "", base="main", cwd=repo3))


def test_the_word_clause_does_not_name_the_assistant(tmp_path):
    """A loose pattern would refuse every PR with a where clause or an if clause."""
    repo, head = pr_over_main(tmp_path, file_text="// the where clause and the if clause\nfn a() {}\n", message="docs: a clause")
    assert ps.refusals(head, "the clause", base="main", cwd=repo, title="feat: clause") == []


def deleted_page(tmp_path: Path, link: str, other_link: str = "") -> tuple[Path, str]:
    """A PR deletes docs/optimizations/passes/dse.md; docs/optimizations/readme.md links it with `link`."""
    repo = tmp_path / "r"
    (repo / "docs/optimizations/passes").mkdir(parents=True)
    sh(repo, "init", "-q", "-b", "main")
    (repo / "docs/optimizations/passes/dse.md").write_text("# dse\n")
    commit(repo, "docs/optimizations/readme.md", f"| [`dse`]({link}) |\n{other_link}", "pages")
    sh(repo, "checkout", "-q", "-b", "pr")
    (repo / "docs/optimizations/passes/dse.md").unlink()
    sh(repo, "add", "-A")
    sh(repo, "commit", "-qm", "refactor(dse): delete the old dse pass")
    return repo, sh(repo, "rev-parse", "HEAD")


def test_a_squash_that_deletes_a_page_a_doc_still_links_to_is_refused_naming_the_linker(tmp_path):
    """#fd939a155 and bbdfe8c06 deleted the dse, unroll and peel pages and left the readme's links: docs_links was red on feat/parity for hours."""
    repo, head = deleted_page(tmp_path, "passes/dse.md")
    why = ps.refusals(head, "Deletes: docs/optimizations/passes/dse.md", base="main", cwd=repo)
    assert len(why) == 1 and why[0].startswith("docs/optimizations/readme.md: still links to docs/optimizations/passes/dse.md"), why


def test_deleting_a_page_with_its_links_gone_or_to_another_file_of_that_name_passes(tmp_path):
    repo, head = deleted_page(tmp_path, "passes/deadstore.md", "[`x`](../elsewhere/dse.md) and [web](https://example.com/dse.md)\n")
    assert ps.refusals(head, "Deletes: docs/optimizations/passes/dse.md", base="main", cwd=repo) == []


def test_a_perf_pr_without_a_rows_line_is_refused():
    """#3251 (perf: GlobalsAA as a module use scan) landed with no rows pair and broke alias/farstruct.nib: a perf title needs a `Rows:` line."""
    assert ps.perf_needs_rows("perf(analysis): one use scan", "TL;DR: faster\nOwner: x")
    assert ps.perf_needs_rows("perf: faster", "")
    assert not ps.perf_needs_rows("perf(analysis): one use scan", "Rows: pair p3251, base e26a479f1924, new 46f5a90986bc: no row above base, run diff clean")
    assert not ps.perf_needs_rows("fix(analysis): a bug", "Owner: x")
    assert not ps.perf_needs_rows("", "")


def test_a_feat_pr_that_wires_a_pass_into_the_pipeline_needs_a_rows_line():
    """#3344 (feat: LoopInterchange, on at every level) had no rows requirement because its title was feat(: wiring pipeline.rs now needs a `Rows:` line."""
    wiring = ["crates/opt/llrm-transforms/src/pipeline.rs", "crates/opt/llrm-transforms/src/loopinterchange.rs"]
    assert ps.perf_needs_rows("feat(loopinterchange): the pin's pass", "Owner: x", wiring)
    assert not ps.perf_needs_rows("feat(loopinterchange): the pin's pass", "Rows: pair p, 852 rows identical", wiring)
    assert not ps.perf_needs_rows("feat(ivdescriptors): a port", "Owner: x", ["crates/opt/llrm-analysis/src/ivdescriptors.rs"])


def test_a_refactor_pr_that_changes_pass_code_needs_a_rows_line():
    """A refactor( PR with a pass-code diff and no Rows line is refused; docs/test-only refactors, fix( and feat( are not."""
    code = ["crates/opt/llrm-analysis/src/globalsaa.rs", "docs/x.md"]
    assert ps.perf_needs_rows("refactor(analysis): one scan", "Owner: x", code)
    assert not ps.perf_needs_rows("refactor(analysis): one scan", "Rows: pair p1 base a new b: clean", code)
    assert not ps.perf_needs_rows("refactor(docs): move", "Owner: x", ["docs/x.md", "crates/opt/llrm-analysis/src/globalsaa_tests.rs"])
    assert not ps.perf_needs_rows("fix(analysis): bug", "Owner: x", code)
    assert not ps.perf_needs_rows("feat(analysis): new", "Owner: x", code)
    assert ps.perf_needs_rows("perf(analysis): faster", "Owner: x", [])


def test_a_new_failing_test_inside_an_already_red_step_is_refused_and_a_fixed_one_is_noted():
    """The gate compared steps: a head red on `ratchet:x` hid a second test of x going red (llrm-qb lib went from 4 to 27 red tests with no refusal)."""
    head = {"ratchet:x::old_one", "run p [-O2]"}
    merged = {"ratchet:x::old_one", "ratchet:x::new_one", "run p [-O2]"}
    why = ps.subset_refusals(head, merged, fixes=False)
    assert why and "ratchet:x::new_one" in why[0] and "old_one" not in why[0].split(", which")[0], why
    assert ps.subset_refusals(head, set(head), fixes=False) == []
    assert ps.fixed_names(head, {"run p [-O2]"}) == ["ratchet:x::old_one"]


def test_a_head_cached_with_a_bare_step_name_covers_that_steps_tests():
    """A failure set cached before steps named their tests holds `ratchet:x`: the tests inside it are not new for that head."""
    assert ps.new_names({"ratchet:x"}, {"ratchet:x::t", "ratchet:y::t"}) == ["ratchet:y::t"]


def test_a_red_test_in_a_changed_crate_is_refused_and_one_the_head_fails_too_is_not():
    """#3578 landed with four red llrm-transforms lib tests: the fast path ran no test. With LAND_FAST_TESTS the changed crates' lib tests run on the merged tree and a failing one the head
    passes refuses the batch. Control on the real functions (a planted `assert!(false)` in llrm-transforms): refused naming it in 88 s; none on the clean head."""
    files = ["crates/opt/llrm-transforms/src/memcpyoptimizer.rs", "docs/x.md"]
    red = {"unit llrm-transforms::memcpyoptimizer::tests::four_byte_stores_are_one_memset"}
    asked = {}

    def on_merged(packages):
        asked["packages"] = set(packages)
        return set(red)

    assert ps.new_lib_failures(files, on_merged=on_merged, on_head=lambda by_package: set()) == red
    assert asked["packages"] == {"llrm-transforms", "llrm-mir"}
    assert ps.new_lib_failures(files, on_merged=on_merged, on_head=lambda by_package: {next(iter(red))}) == set()
    assert ps.new_lib_failures(["docs/x.md"], on_merged=on_merged, on_head=lambda by_package: set()) == set()


def test_a_passing_batch_prints_what_the_lib_layer_ran(capsys):
    """A gate silent on a pass cannot be told from one that never ran: the layer prints crates, tests passed, a hash of the test names and the seconds, on a pass too."""
    smoke = ps.parity_smoke_module()

    def on_merged(packages):
        smoke.LAST_LIB.update(crates=len(packages), passed=940, names="abcd1234", seconds=11.6)
        return set()

    assert ps.new_lib_failures(["crates/opt/llrm-transforms/src/gvn.rs"], on_merged=on_merged, on_head=lambda by_package: set()) == set()
    assert "lib tests: 2 crates, 940 tests passed (abcd1234), 12s" in capsys.readouterr().out
    assert ps.new_lib_failures(["docs/x.md"], on_merged=on_merged, on_head=lambda by_package: set()) == set()
    assert "lib tests: 0 crates, 0 tests passed (no crate changed)" in capsys.readouterr().out


def test_a_change_to_any_crate_source_also_runs_the_scan_crates_lib_tests():
    """extload.rs, booleans.rs and selectiondag/ landed three red scans (`no_file_gains_*` in llrm-mir) because only the changed crates' lib tests ran."""
    asked = []

    def on_merged(packages):
        asked.append(set(packages))
        return set()

    ps.new_lib_failures(["crates/opt/llrm-transforms/src/extload.rs"], on_merged=on_merged, on_head=lambda by_package: set())
    ps.new_lib_failures(["docs/x.md"], on_merged=on_merged, on_head=lambda by_package: set())
    assert asked == [{"llrm-transforms", "llrm-mir"}]


def test_a_unit_the_head_passes_names_the_offender_and_the_rest_are_tested_once():
    """19:12, 19:22 and 19:37: batches failed `no_file_gains_*` and were halved and halved again. With the offender named by the failing unit alone, 8 PRs cost 2 tests, not 7."""
    landed, refused, tested = [], [], []

    def passes(group):
        tested.append(list(group))
        return "6" not in group

    ps.batch([str(n) for n in range(1, 9)], passes, landed.append, refused.append, lambda group: ["6"])
    assert refused == ["6"] and landed == ["1", "2", "3", "4", "5", "7", "8"]
    assert tested == [[str(n) for n in range(1, 9)], ["1", "2", "3", "4", "5", "7", "8"]]


def test_an_offender_hook_that_names_nobody_or_everybody_falls_back_to_halving():
    landed, refused = [], []
    ps.batch(["1", "2", "3", "4"], lambda group: "3" not in group, landed.append, refused.append, lambda group: [])
    assert landed == ["1", "2", "4"] and refused == ["3"]
    landed, refused = [], []
    ps.batch(["1", "2", "3", "4"], lambda group: "3" not in group, landed.append, refused.append, lambda group: list(group))
    assert landed == ["1", "2", "4"] and refused == ["3"]


def test_two_offenders_are_both_dropped_and_the_others_land():
    landed, refused = [], []
    ps.batch(["1", "2", "3", "4"], lambda group: not {"2", "3"} & set(group), landed.append, refused.append, lambda group: ["2", "3"])
    assert refused == ["2", "3"] and landed == ["1", "4"]


def test_a_refusal_line_names_the_prs_own_reason_not_the_batchs_first_failure():
    """22:26: the queue printed the log's first REFUSED line (#3841's `mut`) for eight PRs; seven of them had been refused for failing units."""
    named = {"3841": "alone on the head it does not build: how_many.rs:1527: variable does not need to be mutable"}
    build = ["how_many.rs:1527:17: error: variable does not need to be mutable"]
    assert "does not build" in ps.refusal_line("3841", named, build, set())
    assert "how_many" not in ps.refusal_line("3843", named, [], {"unit llrm-mir::tests::no_file_gains_a_slice_read_of_a_use_list"})
    assert "no_file_gains" in ps.refusal_line("3843", named, [], {"unit llrm-mir::tests::no_file_gains_a_slice_read_of_a_use_list"})
    assert ps.refusal_line("3846", {}, [], set()).startswith("#3846: REFUSED: ")


def test_a_head_with_a_conflict_marker_in_an_added_line_is_left_out_by_name(tmp_path):
    """sdag-a pushed markers to open branches twice in a day."""
    def run(*args):
        return subprocess.run(["git", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    run("init", "-q", "-b", "base")
    run("config", "user.email", "t@t")
    run("config", "user.name", "t")
    (tmp_path / "a.rs").write_text("fn a() {}\n")
    run("add", "-A")
    run("commit", "-q", "-m", "base")
    run("checkout", "-q", "-b", "pr")
    (tmp_path / "a.rs").write_text("fn a() {}\n" + "<" * 7 + " HEAD\nfn b() {}\n=======\nfn c() {}\n" + ">" * 7 + " origin\n")
    (tmp_path / "ok.rs").write_text("let x = 1 << 7; // " + "<" * 8 + "\n")
    run("add", "-A")
    run("commit", "-q", "-m", "pr")
    why = ps.conflict_markers("base", "pr", tmp_path)
    assert why == ["a.rs: an added line is a merge conflict marker; resolve the merge and push again"]
    assert any("conflict marker" in line for line in ps.refusals("pr", "Owner: x", "base", tmp_path, "fix: x"))


def test_a_missing_deletes_line_is_a_warning_by_default_and_a_refusal_when_the_switch_is_on(tmp_path, monkeypatch):
    """The user's call (20:2x): description checks warn until the backend parts land; BODY_CHECKS=refuse turns them back into refusals."""
    repo, head = stale_merge(tmp_path)
    monkeypatch.setattr(ps, "BODY_CHECKS", "warn")
    assert ps.refusals(head, "", base="main", cwd=repo) == []
    warned = ps.warnings(head, "", base="main", cwd=repo)
    assert warned and "tests.rs" in warned[0]
    monkeypatch.setattr(ps, "BODY_CHECKS", "refuse")
    assert ps.warnings(head, "", base="main", cwd=repo) == []
    assert ps.refusals(head, "", base="main", cwd=repo)


def test_a_line_naming_the_assistant_stays_a_refusal_under_the_warning_switch(tmp_path, monkeypatch):
    monkeypatch.setattr(ps, "BODY_CHECKS", "warn")

    def run(*args):
        return subprocess.run(["git", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    run("init", "-q", "-b", "base")
    run("config", "user.email", "t@t")
    run("config", "user.name", "t")
    (tmp_path / "a.txt").write_text("one\n")
    run("add", "-A")
    run("commit", "-q", "-m", "base")
    run("checkout", "-q", "-b", "pr")
    (tmp_path / "a.txt").write_text("one\nwritten by " + "Clau" + "de\n")
    run("add", "-A")
    run("commit", "-q", "-m", "pr")
    assert any("names the assistant" in line for line in ps.refusals("pr", "", base="base", cwd=tmp_path, title="docs: x"))


def fmt_repo(tmp_path, stub):
    """A repo with a stub tools/fmt.sh, a bare origin, a `base` branch and a `pr` branch that adds a.rs with trailing blanks."""
    origin = tmp_path / "origin.git"
    repo = tmp_path / "repo"

    def run(*args, cwd=repo):
        return subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()

    run("init", "-q", "--bare", "-b", "base", str(origin), cwd=tmp_path)
    run("init", "-q", "-b", "base", str(repo), cwd=tmp_path)
    run("config", "user.email", "t@t")
    run("config", "user.name", "t")
    (repo / "tools").mkdir()
    (repo / "tools" / "fmt.sh").write_text(stub)
    (repo / "tools" / "fmt.sh").chmod(0o755)
    run("add", "-A")
    run("commit", "-q", "-m", "base")
    run("remote", "add", "origin", str(origin))
    run("push", "-q", "origin", "base")
    run("checkout", "-q", "-b", "pr")
    return repo, origin, run


STRIP = "#!/bin/sh\nif [ \"$1\" = --stdin ]; then sed 's/[ \\t]*$//'; fi\n"


def test_an_unformatted_pr_is_squashed_from_a_formatted_commit_pushed_to_its_branch(tmp_path):
    """The user's order: tools/fmt.sh applies to every PR; 92 files of parity were unformatted because the runner excused a red head."""
    repo, origin, run = fmt_repo(tmp_path, STRIP)
    (repo / "a.rs").write_text("fn a() {}   \nfn b() {}\t\n")
    run("add", "-A")
    run("commit", "-q", "-m", "pr")
    head = run("rev-parse", "HEAD")
    sha, why = ps.format_head(head, "pr", base="base", cwd=repo)
    assert why == "" and sha != head
    assert run("show", f"{sha}:a.rs") == "fn a() {}\nfn b() {}"
    assert subprocess.run(["git", "rev-parse", "refs/heads/pr"], cwd=origin, capture_output=True, text=True).stdout.strip() == sha


def test_an_already_formatted_pr_keeps_its_head_and_pushes_nothing(tmp_path):
    repo, origin, run = fmt_repo(tmp_path, STRIP)
    (repo / "a.rs").write_text("fn a() {}\n")
    run("add", "-A")
    run("commit", "-q", "-m", "pr")
    head = run("rev-parse", "HEAD")
    assert ps.format_head(head, "pr", base="base", cwd=repo) == (head, "")
    assert subprocess.run(["git", "rev-parse", "--verify", "refs/heads/pr"], cwd=origin, capture_output=True).returncode != 0


def test_a_formatter_that_crashes_refuses_the_landing(tmp_path):
    """fmt.sh exit 2 or more is a missing toolchain or a crash: a gate that tolerates 1 must not tolerate the rest."""
    repo, origin, run = fmt_repo(tmp_path, "#!/bin/sh\nexit 2\n")
    (repo / "a.rs").write_text("fn a() {}\n")
    run("add", "-A")
    run("commit", "-q", "-m", "pr")
    sha, why = ps.format_head(run("rev-parse", "HEAD"), "pr", base="base", cwd=repo)
    assert sha == "" and "exit 2" in why and "a.rs" in why


def test_a_batch_that_does_not_build_drops_the_pr_that_breaks_the_build_alone_and_lands_the_rest():
    """The 21:02 batch failed to build (#3763 added ISD variants #3806's code did not cover) and every PR in it was refused."""
    broken = {"3763"}
    merged = []

    def merge_alone(number):
        merged.append(number)
        return number != "3807"  # cannot join: skipped, not blamed

    check = lambda: "dagisel_adapter.rs:33: non-exhaustive patterns" if merged[-1] in broken else ""  # noqa: E731
    group = ["3763", "3812", "3736", "3807", "3793"]
    assert ps.build_offenders(group, merge_alone, check) == ["3763"]
    landed, refused, tested = [], [], []

    def passes(prs):
        tested.append(list(prs))
        return not broken & set(prs)

    ps.batch(group, passes, landed.append, refused.append, lambda prs: ps.build_offenders(prs, merge_alone, check))
    assert refused == ["3763"] and landed == ["3812", "3736", "3807", "3793"]
    assert tested == [group, ["3812", "3736", "3807", "3793"]]


def test_two_halves_that_build_apart_and_not_together_name_nobody_and_are_halved():
    group = ["1", "2", "3", "4"]
    assert ps.build_offenders(group, lambda number: True, lambda: "") == []

def test_a_change_to_llrm_hir_also_runs_llrm_cs_toolchain_lib_tests():
    """#2936 changed the lowering and broke llrm-c's call-boundary test, which only the toolchain feature builds and no layer ran."""
    asked = []

    def on_merged(packages):
        asked.append(set(packages))
        return set()

    ps.new_lib_failures(["crates/ir/llrm-hir/src/mir.rs"], on_merged=on_merged, on_head=lambda by_package: set())
    ps.new_lib_failures(["crates/opt/llrm-transforms/src/gvn.rs"], on_merged=on_merged, on_head=lambda by_package: set())
    assert "llrm-c" in asked[0] and "llrm-hir" in asked[0]
    assert "llrm-c" not in asked[1]


def test_the_tool_modules_are_loaded_before_a_landing_can_change_them(tmp_path, monkeypatch):
    """#3817 changed parity_smoke while the batch that held the old parity_squash was running: half old, half new tools refused the 15 known llrm-c reds as new."""
    (tmp_path / "fake_tool.py").write_text("VERSION = 1\n")
    monkeypatch.syspath_prepend(str(tmp_path))
    sys.modules.pop("fake_tool", None)
    ps.preload_tools(("fake_tool",))
    (tmp_path / "fake_tool.py").write_text("VERSION = 2\n")  # a landing merged into the checkout
    import importlib

    assert importlib.import_module("fake_tool").VERSION == 1
    assert {"parity_smoke", "xfail", "buildslot"} <= set(ps.TOOL_MODULES)


def test_the_scanner_layer_refuses_a_red_scanner_the_head_passes_and_runs_the_registry(capsys):
    """#3899 landed with tests/selectiondag_registry.rs red: the fast layer ran lib tests only. It now runs the root scanners and a test file the batch changes."""
    asked = []
    red = {"integration selectiondag_registry::a_number_with_no_row_is_reported"}

    def merged(targets):
        asked.append(list(targets))
        return set(red)

    files = ["crates/backend/llrm-core/src/backend/selectiondag/dag_function.rs", "tests/new_scan.rs"]
    assert ps.new_scanner_failures(files, on_merged=merged, on_head=lambda targets: set()) == red
    assert "selectiondag_registry" in asked[0] and "new_scan" in asked[0] and "turbo" not in asked[0]
    assert ps.new_scanner_failures(files, on_merged=merged, on_head=lambda targets: set(red)) == set()
    assert "scanners: " in capsys.readouterr().out


def test_a_pr_added_to_waits_during_the_batch_is_not_pushed_and_is_held(tmp_path, monkeypatch):
    """#4057 landed through a hold the coordinator added while its batch ran: waits.txt was read once, to build the queue. The squash reads it again."""
    waits = tmp_path / "waits.txt"
    waits.write_text("# owner waits\n2697 # parked\n")
    monkeypatch.setenv("WAITS", str(waits))
    calls = []

    def run(command, **kwargs):
        calls.append(command)
        return SimpleNamespace(returncode=0, stdout="", stderr="")

    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b"}
    assert ps.squash("4057", info, "body", run=run) is True and calls, "not held: squashed"
    calls.clear()
    waits.write_text("# owner waits\n2697 # parked\n4057 # held mid-batch\n")
    assert ps.squash("4057", info, "body", run=run) is None
    assert not calls, "nothing was pushed or merged"
    assert ps.waiting("4057") and not ps.waiting("405") and not ps.waiting("2698")


def test_the_docs_scanner_runs_at_landing_without_the_lib_tests_switch():
    """#4039 landed an unlinked docs page red (tests/docs_links): the scanner layer ran only with LAND_FAST_TESTS=1, which the runner does not set."""
    asked = []

    def on_merged(targets):
        asked.append(list(targets))
        return {"integration docs_links::test_every_doc_is_reachable_from_the_root_readme"}

    assert ps.landing_scanner_failures(["docs/working/x.md"], False, on_merged=on_merged, on_head=lambda t: set()) == {"integration docs_links::test_every_doc_is_reachable_from_the_root_readme"}
    assert asked == [["docs_links", "no_local_paths"]]
    asked.clear()
    ps.landing_scanner_failures(["docs/working/x.md"], True, on_merged=lambda t: asked.append(list(t)) or set(), on_head=lambda t: set())
    assert "docs_links" in asked[0] and len(asked[0]) > 1


def test_the_scanner_line_names_the_tree_it_scanned():
    """#4039 landed after `scanners: 13 targets, 0 red`; replaying its PRs on the head is red, and the log did not say which tree the scanners saw."""
    assert ps.scanner_tests_line(["docs_links"], set(), "abc123def") == "scanners: 1 targets, 0 red (tree abc123def)"
    assert ps.scanner_tests_line(["docs_links"], {"x"}) == "scanners: 1 targets, 1 red"


def test_a_pr_is_rescanned_when_the_batch_dropped_one_after_its_scan():
    """The batch's scanners ran on a tree with #4042 in it; #4042 failed at the squash ("Head branch was modified") and the others were pushed on a tree no scanner had seen."""
    asked = []
    red = lambda number, info: asked.append(number) or {"integration docs_links::test_every_doc_is_reachable_from_the_root_readme"}  # noqa: E731
    assert ps.rescanned_refusal("4039", {}, [], rescan=red) == "" and not asked, "nothing dropped: the batch's scan stands"
    why = ps.rescanned_refusal("4039", {}, ["4042"], rescan=red)
    assert asked == ["4039"] and "dropped #4042" in why and "docs_links" in why
    assert ps.rescanned_refusal("4039", {}, ["4042"], rescan=lambda n, i: set()) == ""
    import inspect

    assert "rescanned_refusal(number" in inspect.getsource(ps.merge_batch), "land() calls it before pushing"


def test_a_squash_github_refuses_prints_the_reason(capsys):
    """`4108: REFUSED: #4108 ` named no reason: the log held GitHub's message on a line the summary does not carry, and the only REFUSED text left was the batch's summary."""
    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b"}
    refuse = lambda command, **kwargs: SimpleNamespace(returncode=1, stdout="", stderr="GraphQL: Head branch was modified. Review and try the merge again. (mergePullRequest)\n")  # noqa: E731
    assert ps.squash("4108", info, "body", run=refuse) is False
    out = capsys.readouterr().out
    assert "#4108: REFUSED: the squash was refused by GitHub: GraphQL: Head branch was modified" in out and "(mergePullRequest)" in out


def test_a_doc_naming_a_path_of_one_machine_is_refused_at_landing_without_the_lib_tests_switch():
    """tests/no_local_paths went red on parity from a doc naming ~/scratch (#4086) and nothing at landing ran it."""
    def on_merged(targets):
        return {"integration no_local_paths::test_no_tracked_markdown_names_a_path_of_one_machine"} if "no_local_paths" in targets else set()

    failed = ps.landing_scanner_failures(["docs/working/machine-cleanup-compile-plan.md"], False, on_merged=on_merged, on_head=lambda t: set())
    assert failed == {"integration no_local_paths::test_no_tracked_markdown_names_a_path_of_one_machine"}
    smoke = ps.parity_smoke_module()
    assert set(ps.ALWAYS_SCANNERS) <= set(smoke.SCANNERS) and not set(ps.ALWAYS_SCANNERS) & set(smoke.NIGHTLY_ONLY)


def test_a_pr_refused_for_its_own_head_is_held_until_its_head_changes(tmp_path):
    """#4142 was refused six times, #4145 and #4146 four, each with the same head, each retry a batch of 1-14 minutes (21:47-22:22, 2026-10-23)."""
    state = tmp_path / "held.state"
    reasons = {"4142": "alone on the head it fails unit llrm-core::x", "4145": "alone on the head it does not build: error", "4150": "conflicts with an earlier PR of the batch or with feat/parity", "4151": "the batch dropped #4142 after its scan"}
    heads = {"4142": "aaa", "4145": "bbb", "4150": "ccc", "4151": "ddd"}
    lines = ps.hold_own_refusals(["4142", "4145", "4150", "4151"], reasons, heads, state)
    assert lines == ["#4142: held until head changes: aaa", "#4145: held until head changes: bbb"]
    assert ps.held_at("4142", "aaa", state), "the same head twice: held the second time"
    assert not ps.held_at("4142", "zzz", state), "a new head: included"
    assert not ps.held_at("4150", "ccc", state) and not ps.held_at("4151", "ddd", state), "a conflict and a dropped batch are not the PR's own"
    ps.hold_own_refusals(["4142"], reasons, {"4142": "zzz"}, state)
    assert ps.held_at("4142", "zzz", state) and not ps.held_at("4142", "aaa", state), "one line per PR, the latest head"
