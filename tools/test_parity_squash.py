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
        assert "lacks llrm-cc, llrm-llc, llrm-qb, llrm-nib" in str(why)
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
root = Path(__file__).resolve().parents[3]
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
    (repo / "tools/docs/docsbuild").mkdir(parents=True)
    (repo / "docs/optimizations/passes").mkdir(parents=True)
    sh(repo, "init", "-q", "-b", "main")
    (repo / "tools/docs/docsbuild/build.py").write_text(GENERATOR)
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
    info = {"headRefOid": f"head{number}", "body": f"problems: 0 @ head{number}", "baseRefName": "feat/parity"}
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
    info = {"headRefOid": "abc", "body": "problems: 0 @ abc", "baseRefName": "feat/parity"}
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


def test_a_listed_test_that_passes_is_a_warning_and_a_report_line_not_a_refusal(tmp_path, monkeypatch, capsys):
    """25 refusals and ~106 batch-minutes in 24 h (2026-10-27) were `xpass` names: an xfail row whose test now passes refused the batch; it warns, and the owner removes the row."""
    monkeypatch.setenv("LAND_TMP", str(tmp_path))
    xpass = {"xpass unit llrm-nib::test_debug::fixed"}
    on_head = lambda by_package: pytest.fail("the head is asked about a pass")  # noqa: E731
    assert ps.new_lib_failures(["crates/frontends/nib/src/lib.rs"], on_merged=lambda packages: set(xpass), on_head=on_head) == set()
    assert "WARNING xpass unit llrm-nib::test_debug::fixed" in capsys.readouterr().out
    ps.new_lib_failures(["crates/frontends/nib/src/lib.rs"], on_merged=lambda packages: set(xpass), on_head=on_head)
    assert (tmp_path / "xpass-report.txt").read_text() == "xpass unit llrm-nib::test_debug::fixed\n", "named once"
    scanner = {"xpass integration masked::t", "integration docs_in_sync::t"}
    assert ps.new_scanner_failures(["a.rs"], lambda targets: set(scanner), lambda targets: {"integration docs_in_sync::t"}, only=("x",), seen=tmp_path / "seen.json") == set()


def _closure(path):
    smoke = ps.parity_smoke_module()
    packages, graph = smoke.workspace_graph()
    return smoke.dependants(smoke.touched_packages([path], packages), graph)


def test_a_red_test_in_a_crate_that_depends_on_the_changed_one_is_refused():
    """llrm-c's lib tests were red on parity while PRs merged: each PR changed a crate llrm-c depends on, and the fast tier ran the changed crates only."""
    red = {"unit llrm-c::compile::tests::a_call_boundary_listing"}
    on_merged = lambda packages: set(red) if "llrm-c" in packages else set()  # noqa: E731
    assert ps.new_lib_failures(["crates/codegen/llrm-codegen/src/backend/peephole.rs"], on_merged=on_merged, on_head=lambda by_package: set()) == red


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
    assert asked["packages"] == _closure("crates/opt/llrm-transforms/src/memcpyoptimizer.rs") | {"llrm-mir"}
    assert ps.new_lib_failures(files, on_merged=on_merged, on_head=lambda by_package: {next(iter(red))}) == set()
    assert ps.new_lib_failures(["docs/x.md"], on_merged=on_merged, on_head=lambda by_package: set()) == set()


def test_a_passing_batch_prints_what_the_lib_layer_ran(capsys):
    """A gate silent on a pass cannot be told from one that never ran: the layer prints crates, tests passed, a hash of the test names and the seconds, on a pass too."""
    smoke = ps.parity_smoke_module()

    def on_merged(packages):
        smoke.LAST_LIB.update(crates=len(packages), passed=940, names="abcd1234", seconds=11.6)
        return set()

    assert ps.new_lib_failures(["crates/opt/llrm-transforms/src/gvn.rs"], on_merged=on_merged, on_head=lambda by_package: set()) == set()
    crates = len(_closure("crates/opt/llrm-transforms/src/gvn.rs") | {"llrm-mir"})
    assert f"lib tests: {crates} crates, 940 tests passed (abcd1234), 12s" in capsys.readouterr().out
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
    assert asked == [_closure("crates/opt/llrm-transforms/src/extload.rs") | {"llrm-mir"}]


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
    assert "llrm-c" in asked[1]


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


def test_the_scanner_layer_refuses_a_red_scanner_the_head_passes_and_runs_the_registry(capsys, tmp_path, monkeypatch):
    """#3899 landed with tests/selectiondag_registry.rs red: the fast layer ran lib tests only. It now runs the root scanners and a test file the batch changes."""
    asked = []
    red = {"integration selectiondag_registry::a_number_with_no_row_is_reported"}

    def merged(targets):
        asked.append(list(targets))
        return set(red)

    # a test file the PR lacks is not a target (#4965): the new scanner must exist in the tree under test
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "new_scan.rs").write_text("")
    monkeypatch.setattr(ps, "ROOT", tmp_path)
    files = ["crates/codegen/llrm-codegen/src/backend/selectiondag/dag_function.rs", "tests/new_scan.rs"]
    assert ps.new_scanner_failures(files, on_merged=merged, on_head=lambda targets: set()) == red
    assert "selectiondag_registry" in asked[0] and "new_scan" in asked[0] and "turbo" not in asked[0]
    assert ps.new_scanner_failures(files, on_merged=merged, on_head=lambda targets: set(red), seen=tmp_path / "seen.json") == set()
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
    assert asked == [sorted(ps.ALWAYS_SCANNERS)] and {"docs_in_sync", "docs_links", "no_local_paths"} <= set(asked[0])
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

    assert "rescan_check(number" in inspect.getsource(ps.merge_batch), "land() calls it before pushing"


def test_a_squash_github_refuses_prints_the_reason(capsys):
    """`4108: REFUSED: #4108 ` named no reason: the log held GitHub's message on a line the summary does not carry, and the only REFUSED text left was the batch's summary."""
    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b"}
    refuse = lambda command, **kwargs: SimpleNamespace(returncode=1, stdout="", stderr="GraphQL: Pull Request is not mergeable (mergePullRequest)\n")  # noqa: E731
    assert ps.squash("4108", info, "body", run=refuse) is False
    out = capsys.readouterr().out
    assert "#4108: REFUSED: the squash was refused by GitHub: GraphQL: Pull Request is not mergeable" in out and "(mergePullRequest)" in out


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
    reasons = {"4142": "alone on the head it fails unit llrm-codegen::x", "4145": "alone on the head it does not build: error", "4150": "conflicts with an earlier PR of the batch or with feat/parity", "4151": "the batch dropped #4142 after its scan"}
    heads = {"4142": "aaa", "4145": "bbb", "4150": "ccc", "4151": "ddd"}
    lines = ps.hold_own_refusals(["4142", "4145", "4150", "4151"], reasons, heads, state)
    assert lines == ["#4142: held until head changes: aaa", "#4145: held until head changes: bbb"]
    assert ps.held_at("4142", "aaa", state), "the same head twice: held the second time"
    assert not ps.held_at("4142", "zzz", state), "a new head: included"
    assert not ps.held_at("4150", "ccc", state) and not ps.held_at("4151", "ddd", state), "a conflict and a dropped batch are not the PR's own"
    ps.hold_own_refusals(["4142"], reasons, {"4142": "zzz"}, state)
    assert ps.held_at("4142", "zzz", state) and not ps.held_at("4142", "aaa", state), "one line per PR, the latest head"


def _gh(outcomes):
    """A gh whose successive `pr merge` calls end as `outcomes` (a stderr, or "" for success)."""
    calls = []

    def run(command, **kwargs):
        calls.append(command)
        said = outcomes[len([c for c in calls if c[:3] == ["gh", "pr", "merge"]]) - 1] if command[:3] == ["gh", "pr", "merge"] else ""
        return SimpleNamespace(returncode=1 if said else 0, stdout="", stderr=said)

    return run, calls


OUT_OF_DATE = "GraphQL: Head branch is out of date. Review and try the merge again. (mergePullRequest)"


def test_a_head_that_is_only_out_of_date_gets_parity_merged_and_the_squash_retried_once(capsys):
    """#3477 was refused twice with `Head branch is out of date`: another landing had moved feat/parity, the owner had done nothing."""
    run, calls = _gh([OUT_OF_DATE, ""])
    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b"}
    assert ps.squash("3477", info, "body", run=run, settle=0, merge=lambda head, branch: "c" * 40) is True
    merges = [c for c in calls if c[:3] == ["gh", "pr", "merge"]]
    assert len(merges) == 2 and merges[1][merges[1].index("--match-head-commit") + 1] == "c" * 40
    assert "#3477: head out of date: merged parity ccccccccc, retried" in capsys.readouterr().out


def test_out_of_date_with_a_conflict_is_the_owners_and_a_second_out_of_date_is_refused(capsys):
    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b"}
    run, calls = _gh([OUT_OF_DATE, ""])
    assert ps.squash("1", info, "body", run=run, settle=0, merge=lambda head, branch: "") is False
    assert len([c for c in calls if c[:3] == ["gh", "pr", "merge"]]) == 1, "a conflict: no retry"
    assert "does not merge into it cleanly" in capsys.readouterr().out
    run, calls = _gh([OUT_OF_DATE, OUT_OF_DATE, ""])
    assert ps.squash("2", info, "body", run=run, settle=0, merge=lambda head, branch: "d" * 40) is False
    assert len([c for c in calls if c[:3] == ["gh", "pr", "merge"]]) == 2, "one merge and one retry, no more"


def test_a_race_with_another_landing_is_not_held_as_the_prs_own():
    assert not ps.own_refusal("the squash was refused by GitHub: GraphQL: Head branch is out of date. Review and try the merge again.")
    assert not ps.own_refusal("head out of date and feat/parity does not merge into it cleanly: the owner merges fresh feat/parity")


def test_every_stage_of_the_fast_pass_stamps_the_batch_log():
    """The 11:13 batch (58 min) logged the lib tests' and the behavioural stage's seconds and nothing else: 37 min of builds, checks and waits could not be placed. Each stage of
    `passes_fast` now prints a STAGE line before its work, in the order they run."""
    import inspect

    source = inspect.getsource(ps.merge_batch)
    names = ["fast-merge", "workspace-check", "lib-tests", "scanners", "link", "behavioural", "stale"]
    at = [source.find(f'stage("{name}")') for name in names]
    assert all(i >= 0 for i in at), dict(zip(names, at))
    assert at == sorted(at), "stamped out of the order they run"


def test_beside_runs_both_and_joins_the_background_even_when_the_foreground_raises():
    """The overlap lets the release stages run while the lib tests do; a raising foreground must not leave the background touching the checkout the next pass reuses."""
    import threading

    started, finished = threading.Event(), []

    def background():
        started.set()
        finished.append(True)
        return "back"

    def foreground():
        assert started.wait(5), "the background never ran beside the foreground"
        return "front"

    assert ps.beside(background, foreground) == ("front", "back")

    finished.clear()

    def boom():
        raise RuntimeError("lib tests died")

    try:
        ps.beside(lambda: finished.append(True) or "back", boom)
        raise AssertionError("the foreground error was swallowed")
    except RuntimeError:
        pass
    assert finished == [True], "the background was not finished when beside returned"


def test_the_always_on_scanners_run_before_the_lib_tests_of_a_batch():
    """The 11:13 batch ran `lib tests ... 363s` and only then found docs_links red for three PRs: a pass of minutes bought nothing, and each refusal repeats the pass. The scanners
    that take milliseconds (docs_links, no_local_paths) now refuse first; the full set still runs after the lib tests."""
    import inspect

    source = inspect.getsource(ps.merge_batch)
    early = source.find('stage("scanners-early")')
    lib = source.find('stage("lib-tests")')
    assert 0 <= early < lib, (early, lib)
    assert "landing_scanner_failures(changed, False)" in source[early:lib]
    assert source.find('stage("scanners")') > lib, "the full scanner set still runs after the lib tests"


def test_a_page_a_pr_leaves_stale_is_named_even_when_the_head_already_has_stale_pages():
    """#5031, #5049 and #4988 changed a `//!` header, passed alone and landed with six pages stale: the head's red docs_in_sync was excused, so no check saw a new one."""
    why, now = ps.docs_refusal({"docs/a.md"}, Path("."), docs=lambda root: {"docs/a.md", "docs/b.md", "docs/c.md"})
    assert "docs/b.md" in why and "docs/c.md" in why and "docs/a.md" not in why and now == {"docs/a.md", "docs/b.md", "docs/c.md"}
    assert ps.docs_refusal({"docs/a.md"}, Path("."), docs=lambda root: {"docs/a.md"})[0] == ""
    assert ps.docs_refusal({"docs/a.md"}, Path("."), docs=lambda root: set())[0] == "", "a PR that regenerates a page is not refused"


def test_a_pr_that_stales_a_page_is_refused_and_its_merge_reset_while_the_others_stay(tmp_path):
    """#5031, #5049 and #4988 each changed a `//!` header and landed with six pages stale: nothing refused the PR whose merge made a page new-stale."""
    def run(*args):
        return subprocess.run(["git", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    run("init", "-q", "-b", "main")
    run("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base")

    def merge_one(number):
        (tmp_path / f"pr{number}.txt").write_text(number)
        run("add", ".")
        run("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", f"pr {number}")
        return ""

    stale_after = {"1": {"docs/old.md"}, "2": {"docs/old.md", "docs/new.md"}, "3": {"docs/old.md"}}
    current = []
    gated = ps.docs_gated(lambda number: (current.append(number), merge_one(number))[1], {"docs/old.md"}, tmp_path, docs=lambda root: stale_after[current[-1]])
    assert gated("1") == ""
    why = gated("2")
    assert "docs/new.md" in why and "docs/old.md" not in why
    assert not (tmp_path / "pr2.txt").exists() and run("log", "--format=%s").splitlines() == ["pr 1", "base"], "the merge of the refused PR is reset"
    assert gated("3") == "" and (tmp_path / "pr3.txt").exists(), "a later PR still joins"


def test_a_build_error_that_names_a_file_builds_only_the_prs_touching_it_alone():
    """The 00:49 batch of 8 failed at lsr_pass_tests.rs:816; the runner built all 8 alone (about 50 s each) to find the one PR that touched it."""
    built = []
    group = [str(number) for number in range(1, 9)]
    culprits = ps.build_offenders(group, lambda number: built.append(number) or True, lambda: "error" if built[-1] == "8" else "", {}, suspects=["8"])
    assert culprits == ["8"] and built == ["8"], "one suspect at fault: one alone-build"
    built.clear()
    culprits = ps.build_offenders(group, lambda number: built.append(number) or True, lambda: "error" if built[-1] == "5" else "", {}, suspects=["8"])
    assert culprits == ["5"] and built[0] == "8" and len(built) == 8, "no suspect at fault: every other PR is built alone, as before"
    assert ps.error_files("crates/opt/llrm-transforms/src/lsr_pass_tests.rs:816:16: error[E0063]") == {"crates/opt/llrm-transforms/src/lsr_pass_tests.rs"}

def test_a_squash_github_failed_on_its_side_is_retried_once_after_a_wait_not_refused(capsys):
    """#5039 was REFUSED with `GraphQL: Something went wrong` (00:14, request A186:1A8971): GitHub's own error, the PR open, mergeable and unchanged; it cost a batch cycle."""
    run, calls = _gh(["GraphQL: Something went wrong while executing your query.", ""])
    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b"}
    assert ps.squash("5039", info, "body", run=run, settle=0, retry_wait=0) is True
    assert len([c for c in calls if c[:3] == ["gh", "pr", "merge"]]) == 2
    run, calls = _gh(["GraphQL: Something went wrong.", "GraphQL: Something went wrong."])
    assert ps.squash("5040", info, "body", run=run, settle=0, retry_wait=0) is False
    assert len([c for c in calls if c[:3] == ["gh", "pr", "merge"]]) == 2, "once, not forever"
    assert not ps.own_refusal("the squash was refused by GitHub: GraphQL: Something went wrong")


def test_a_squash_github_declines_for_a_moved_head_is_left_out_not_refused(capsys):
    """#5087's squash was declined with `Head branch was modified` (its owner pushed during the batch) and counted as a REFUSAL in the summary and the exit code; the PR is left out and re-queued."""
    run, calls = _gh(["GraphQL: Head branch was modified. Review and try the merge again. (mergePullRequest)"])
    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b"}
    ps.HEAD_MOVED.clear()
    assert ps.squash("5087", info, "body", run=run, settle=0, retry_wait=0) is False
    out = capsys.readouterr().out
    assert "#5087: LEFT OUT (head moved)" in out and "REFUSED" not in out and "5087" in ps.HEAD_MOVED
    assert ps.batch_summary(["1"], ["2"], [], ["5087"]) == "LANDED: #1  REFUSED: #2  LEFT OUT (head moved): #5087"
    assert ps.batch_summary([], [], [], ["5087"]) == "LANDED: -  REFUSED: -  LEFT OUT (head moved): #5087"


def test_a_docs_refusal_names_the_stale_pages_with_the_batch_and_head_blobs_and_the_prs_that_changed_the_source(tmp_path):
    """Four refusals of the 03:03 batch said `docs_in_sync::test_no_generated_page_is_stale` and nothing else; owners guessed which page and which PR."""
    def git(*args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    git("init", "-q", "-b", "main")
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/a.md").write_text("head\n")
    git("add", ".")
    git("commit", "-qm", "base")
    (tmp_path / "docs/a.md").write_text("<!-- src: crates/x/precse.rs sha256:0 -->\nstale in the batch\n")
    head_blob, batch_blob = git("rev-parse", "main:docs/a.md")[:9], git("hash-object", "docs/a.md")[:9]
    got = ps.docs_detail({"integration docs_in_sync::test_no_generated_page_is_stale"}, tmp_path, "main", stale=lambda root: {"docs/a.md", "docs/gone.md"}, members=lambda: {"5094": ["crates/x/precse.rs"], "5071": ["other.rs"]})
    assert got == f" [stale pages: docs/a.md (batch tree {batch_blob}, head {head_blob}; source crates/x/precse.rs, changed by #5094); docs/gone.md (batch tree absent, head absent)]", got
    assert ps.docs_detail({"integration docs_links::t"}, tmp_path, "main", stale=lambda root: pytest.fail("not a docs refusal")) == ""


def test_an_alone_refusal_is_not_replayed_once_the_base_moved(tmp_path):
    """#5092 was refused alone at 02:31 for two scanners the head already had red, and the cached refusal (keyed by the PR head only) was replayed at 02:53 and 03:03 while parity moved."""
    ran = []

    def refusals(head, base):
        ran.append(base)
        return ["scanner red alone: x"] if base == "old" else []

    heads = {"5092": "h" * 40}
    kept, dropped = ps.alone(["5092"], heads, refusals=refusals, cache=tmp_path, base="old", base_sha="a" * 40)
    assert dropped == {"5092": "scanner red alone: x"} and ran == ["old"]
    assert ps.alone(["5092"], heads, refusals=refusals, cache=tmp_path, base="old", base_sha="a" * 40)[1] and ran == ["old"], "the same base replays the refusal"
    kept, dropped = ps.alone(["5092"], heads, refusals=refusals, cache=tmp_path, base="new", base_sha="b" * 40)
    assert kept == ["5092"] and not dropped and ran == ["old", "new"], "a moved base checks the PR again"
    assert ps.alone(["5092"], heads, refusals=refusals, cache=tmp_path, base="new", base_sha="c" * 40)[0] == ["5092"] and ran == ["old", "new"], "a pass is kept per head"


def test_a_cached_pass_is_rechecked_for_stale_pages_against_the_base_as_it_is_now(tmp_path):
    """#5062, #5085 and #5088 passed alone against an older parity, the verdict was reused per head sha, and each made a generated page fail once merged with what landed since."""
    cache = tmp_path / "cache"
    cache.mkdir()
    (cache / "h1").write_text("")  # passed alone earlier
    (cache / "h2").write_text("")
    checked = []
    kept, dropped = ps.alone(["1", "2"], {"1": "h1", "2": "h2"}, refusals=lambda head, base: checked.append(head) or [], cache=cache, jobs=1,
                             recheck=lambda head, base: "generated docs out of date: docs/a.md" if head == "h2" else "")
    assert kept == ["1"] and dropped == {"2": "generated docs out of date: docs/a.md"} and checked == [], "a cached pass runs only the docs recheck"
    assert (cache / "h2").read_text() == "", "the recheck does not overwrite the cached verdict"


def test_a_head_whose_header_changed_without_its_page_is_named_once_merged_with_the_base(tmp_path):
    """The recheck on a real merge: a head that changes a `//!` header and leaves its page alone."""


def test_a_pr_the_cap_cuts_off_is_reported_unchecked_not_checked():
    """The 00:32 batch's line said "9 checked, 0 dropped" while a single scanners check took 620 s against ALONE_CAP=600: #5050 and #5051, red alone, went into the batch."""
    import threading

    release = threading.Event()
    kept, dropped = ps.alone(["1", "2"], {"1": "h1", "2": "h2"}, refusals=lambda head, base: [] if head == "h1" else (release.wait(5), [])[1], jobs=2, cap=0.2)
    release.set()
    assert kept == ["1", "2"] and dropped == {}, "kept unfiltered, as designed"
    assert ps.UNCHECKED == ["2"], "but named as unchecked"


def test_a_pr_whose_head_moved_during_a_batch_is_merged_at_its_new_head():
    """The 02:34 sub-batch merged #5088's 01:18 head (sinkcast.md stale) 30 min after w12 pushed the fix, and the squash of #5085 was refused for a head w14 had moved."""
    infos = {"5088": {"headRefOid": "old" * 13 + "o", "title": "t"}, "5087": {"headRefOid": "same" * 10, "title": "t"}}
    views = {"5088": {"headRefOid": "new" * 13 + "n", "title": "t2"}, "5087": {"headRefOid": "same" * 10, "title": "t"}}
    moved = ps.refresh_heads(infos, ["5088", "5087"], view=lambda number: views[number])
    assert list(moved) == ["5088"] and moved["5088"][1].startswith("newnew")
    assert infos["5088"]["headRefOid"].startswith("newnew") and infos["5088"]["title"] == "t2", "infos holds the new read"
    assert ps.refresh_heads(infos, ["5088", "5087"], view=lambda number: views[number]) == {}, "nothing moved the second time"


def test_a_pr_whose_head_moved_is_left_out_and_not_held_as_its_own_refusal():
    """A halved batch kept merging a head the owner had replaced, or (with the re-read alone) a new head that had not waited its 15 minutes."""
    infos = {"5088": {"headRefOid": "o" * 40}, "5087": {"headRefOid": "s" * 40}}
    views = {"5088": {"headRefOid": "n" * 40}, "5087": {"headRefOid": "s" * 40}}
    left_out = {}
    moved = ps.defer_moved(infos, ["5088", "5087"], left_out, view=lambda number: views[number])
    assert list(moved) == ["5088"] and list(left_out) == ["5088"] and "waits its own head age" in left_out["5088"]
    assert not ps.own_refusal(left_out["5088"]), "a moved head is not the PR's own refusal"


def test_a_merge_batch_older_than_twenty_minutes_is_expired():
    """The 01:45 batch ran 65+ minutes on the heads it assembled with, refusing PRs whose owners had fixed them."""
    assert not ps.batch_expired(100.0, 100.0 + ps.BATCH_MAX_SECONDS) and ps.batch_expired(100.0, 100.0 + ps.BATCH_MAX_SECONDS + 1)
    assert ps.BATCH_MAX_SECONDS == 1200


def test_a_docs_refusal_keeps_the_head_the_status_and_the_check_output_of_the_tree_it_judged(tmp_path):
    """docs_in_sync was refused four times on trees the next check reset and nobody could reproduce: no evidence was kept."""
    def run(*args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    run("init", "-q", "-b", "main")
    (tmp_path / "tools/docs/docsbuild").mkdir(parents=True)
    (tmp_path / "tools/docs/docsbuild/build.py").write_text(
        "import sys, pathlib\nheader = pathlib.Path('a.rs').read_text().strip()\npage = pathlib.Path('docs/a.md').read_text().strip()\n"
        "if header != page:\n    print('generated docs are out of date')\n    print('  docs/a.md')\n    sys.exit(1)\n"
    )
    (tmp_path / "docs").mkdir()
    (tmp_path / "a.rs").write_text("//! v1\n")
    (tmp_path / "docs/a.md").write_text("//! v1\n")
    run("add", ".")
    run("commit", "-q", "-m", "base")
    base = run("rev-parse", "HEAD")
    run("checkout", "-q", "-b", "pr")
    (tmp_path / "a.rs").write_text("//! v2\n")
    run("commit", "-qam", "header changed, page not regenerated")
    head = run("rev-parse", "HEAD")
    run("checkout", "-q", "--detach", base)
    assert "docs/a.md" in ps.docs_recheck(head, base, tmp_path, set())
    assert ps.docs_recheck(head, base, tmp_path, {"docs/a.md"}) == "", "a page the base already has stale is not the PR's"


    (tmp_path / "tools/docs/docsbuild/build.py").write_text("print('generated docs are out of date')\\nprint('  docs/a.md')\\n")
    run("add", ".")
    run("commit", "-q", "-m", "x")
    (tmp_path / "stray.txt").write_text("left over")
    kept = tmp_path.parent / (tmp_path.name + "-refused")
    ps.keep_refused_evidence("abcdef0123456789", tmp_path, ["scanner red alone: integration docs_in_sync::test_no_generated_page_is_stale"], kept)
    out = kept / "abcdef012345"
    assert f"tree {run('rev-parse', 'HEAD')}" in (out / "head.txt").read_text() and "pr head abcdef0123456789" in (out / "head.txt").read_text() and "stray.txt" in (out / "status.txt").read_text() and "docs/a.md" in (out / "check.txt").read_text()
    ps.keep_refused_evidence("1111111111111111", tmp_path, ["unit tests fail alone"], kept)
    assert not (kept / "111111111111").exists(), "only a docs refusal keeps its tree's evidence"


def test_a_batch_docs_refusal_keeps_each_members_head_and_commit_time(tmp_path):
    """"Stale heads or the runner's tree" could not be answered at the 02:26 refusal: the batch kept no list of the heads it judged."""
    def run(*args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    run("init", "-q", "-b", "main")
    (tmp_path / "tools/docs/docsbuild").mkdir(parents=True)
    (tmp_path / "tools/docs/docsbuild/build.py").write_text("print('  docs/a.md')\n")
    run("add", ".")
    run("commit", "-q", "-m", "x")
    head = run("rev-parse", "HEAD")
    kept = tmp_path.parent / (tmp_path.name + "-batch")
    ps.keep_batch_evidence({"5085": head}, tmp_path, kept, "022633")
    heads = (kept / "batch-022633" / "heads.txt").read_text()
    assert f"#5085 {head} committed 20" in heads and "docs/a.md" in (kept / "batch-022633" / "check.txt").read_text()


def test_docs_gated_with_the_default_docs_check_does_not_crash_and_a_crash_names_the_exception(tmp_path):
    """05:18: `docs_refusal` called `docs()` while the default `docs_stale` needs the root: every batch died with a TypeError and all 13 PRs were refused with an empty reason; #5073's tests mocked `docs`."""
    subprocess.run(["git", "init", "-q", "-b", "main"], cwd=tmp_path, check=True)
    subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "base"], cwd=tmp_path, check=True)
    gated = ps.docs_gated(lambda number: "", set(), tmp_path)
    assert gated("1") == "", "no build.py in the tree: nothing is stale, and nothing raises"
    line = ps.crash_line("5082", TypeError("docs_stale() missing 1 required positional argument: 'root'"))
    assert line == "#5082: REFUSED: the batch crashed (a tool fault, not this PR): TypeError: docs_stale() missing 1 required positional argument: 'root'"


def test_a_rate_limited_squash_does_not_hold_the_pr_at_its_head(tmp_path):
    """A 403 'API rate limit exceeded' at the squash read as the PR's own refusal and held it until its head changed: GitHub's quota, not the PR, cost the landing."""
    state = tmp_path / "held.state"
    reason = "the squash was refused by GitHub: GraphQL: API rate limit exceeded for user ID 1. (HTTP 403)"
    assert ps.hold_own_refusals(["5290"], {"5290": reason}, {"5290": "abc123"}, state) == []
    assert not state.exists()


def test_a_403_at_the_squash_waits_for_the_reset_and_neither_refuses_nor_holds_the_pr(monkeypatch, tmp_path, capsys):
    """09:2x: the quota ran out and a squash read `API rate limit exceeded` as the PR's own refusal: REFUSED line, held at its head. It waits for the reset and squashes."""
    monkeypatch.setattr(ps, "FAST", True)
    waits, merges = [], []
    monkeypatch.setattr(ps, "rate_limit_wait", lambda *a, **k: waits.append(1) or 0)

    def run(cmd, **_):
        merges.append(cmd)
        if len(merges) == 1:
            return subprocess.CompletedProcess(cmd, 1, "", "GraphQL: API rate limit exceeded for user ID 1. (HTTP 403)")
        return subprocess.CompletedProcess(cmd, 0, "", "")

    info = {"headRefOid": "abc123", "headRefName": "pr", "title": "t", "mergeable": "MERGEABLE"}
    assert ps.squash("5290", info, "body", run=run, settle=0) is True
    assert waits == [1] and len(merges) == 2
    assert "REFUSED" not in capsys.readouterr().out
    held = tmp_path / "held.state"
    assert ps.hold_own_refusals([], {}, {"5290": "abc123"}, held) == [] and not held.exists()


def test_pr_info_waits_out_a_403_instead_of_crashing_the_batch(monkeypatch):
    """`pr_info` ran with check=True: a 403 raised CalledProcessError and the batch died 'a tool fault' for every member."""
    waits, calls = [], []
    monkeypatch.setattr(ps, "rate_limit_wait", lambda *a, **k: waits.append(1) or 0)

    def fake(cmd, **_):
        calls.append(cmd)
        if len(calls) == 1:
            return subprocess.CompletedProcess(cmd, 1, "", "gh: API rate limit exceeded (HTTP 403)")
        return subprocess.CompletedProcess(cmd, 0, '{"headRefOid": "abc"}', "")

    monkeypatch.setattr(ps.subprocess, "run", fake)
    assert ps.pr_info("5290") == {"headRefOid": "abc"} and waits == [1]


def test_a_check_failure_that_names_no_page_is_named_by_its_problem_line(tmp_path):
    """b92ef6dd2 and #5119 were refused as 'docsbuild --check failed' while the cause was `no # Example` in a header: the sync cannot fix that, and the reason hid it."""
    tool = tmp_path / "tools/docs/docsbuild/build.py"
    tool.parent.mkdir(parents=True)
    tool.write_text("import sys\nprint('1 problem(s)')\nprint('crates/x/src/a.rs: no # Example')\nsys.exit(1)\n")
    assert ps.docs_stale(tmp_path) == {"docsbuild --check: crates/x/src/a.rs: no # Example"}


def _fake_docs_repo(tmp_path):
    """A repo whose tools/docs/docsbuild/build.py writes docs/a.md on --sync and calls it stale on --check until it exists."""
    def git(*args, cwd=tmp_path):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=cwd, capture_output=True, text=True, check=True).stdout.strip()

    (tmp_path / "tools/docs/docsbuild").mkdir(parents=True)
    (tmp_path / "docs").mkdir()
    (tmp_path / "tools/docs/docsbuild/build.py").write_text(
        "import sys\nfrom pathlib import Path\npage = Path('docs/a.md')\n"
        "if '--sync' in sys.argv:\n    page.parent.mkdir(exist_ok=True)\n    page.write_text('generated\\n')\nelif not page.exists():\n    print('1 problem(s)')\n    print('  docs/a.md')\n    sys.exit(1)\n"
    )
    git("init", "-q", "-b", "main")
    git("add", ".")
    git("commit", "-qm", "base")
    return git


def test_a_stale_generated_page_is_synced_and_committed_not_refused(tmp_path):
    """Generated docs out of date refused #5129, #5131 and #5119 (27 PRs were conflicting after each tip move): the batch regenerates the page (build.py --sync) and commits it on top."""
    git = _fake_docs_repo(tmp_path)
    assert ps.docs_stale(tmp_path) == {"docs/a.md"}
    assert ps.docs_sync_commit(tmp_path) == ""
    assert (tmp_path / "docs/a.md").read_text() == "generated\n" and git("log", "--format=%s", "-1").startswith("docs: ")
    assert ps.docs_stale(tmp_path) == set()
    git("reset", "-q", "--hard", "HEAD~1")
    gated = ps.docs_gated(lambda number: "", set(), tmp_path)
    assert gated("5129") == "" and "5129" in gated.synced and ps.docs_stale(tmp_path) == set(), "synced in the batch tree, not refused"
    git("reset", "-q", "--hard", "HEAD~1")
    refused = ps.docs_gated(lambda number: "", set(), tmp_path, sync=lambda root: "docsbuild --sync failed")
    assert "docs/a.md" in refused("5131") and ps.docs_stale(tmp_path) == {"docs/a.md"} and not refused.synced, "a failed sync still refuses and resets"


def test_the_append_only_rows_table_merges_without_a_conflict_when_two_prs_add_rows(tmp_path):
    """Two PRs each adding a row at the end of docs/working/above-main-rows.md conflicted (#5282, #5156, #5242, #5294): the repo's .gitattributes says merge=union for it."""
    def git(*args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True)

    git("init", "-q", "-b", "main")
    (tmp_path / "docs/working").mkdir(parents=True)
    (tmp_path / ".gitattributes").write_text((ps.ROOT / ".gitattributes").read_text())
    table = tmp_path / "docs/working/above-main-rows.md"
    table.write_text("| row | n |\n| a | 1 |\n")
    git("add", ".")
    git("commit", "-qm", "base")
    for name, row in (("one", "| b | 2 |\n"), ("two", "| c | 3 |\n")):
        git("checkout", "-q", "-b", name, "main")
        table.write_text(table.read_text() + row)
        git("commit", "-qam", name)
    git("checkout", "-q", "one")
    merged = git("merge", "--no-edit", "two")
    assert merged.returncode == 0, merged.stdout + merged.stderr
    assert "| b | 2 |" in table.read_text() and "| c | 3 |" in table.read_text()


def test_a_head_that_leaves_a_page_stale_is_synced_on_its_branch_before_the_squash(tmp_path):
    """The batch tree was synced, but the squash lands the PR head: the same sync is pushed to the PR's branch (as the formatted head is), or the squash would land the stale page."""
    work = tmp_path / "work"
    work.mkdir()
    git = _fake_docs_repo(work)
    origin = tmp_path / "origin.git"
    subprocess.run(["git", "init", "-q", "--bare", str(origin)], check=True)
    git("remote", "add", "origin", str(origin))
    git("branch", "-M", "feat/parity")
    git("push", "-q", "origin", "feat/parity")
    git("checkout", "-q", "-b", "pr")
    (work / "pr.txt").write_text("x\n")
    git("add", ".")
    git("commit", "-qm", "pr")
    head = git("rev-parse", "HEAD")
    sha, why = ps.sync_docs_head(head, "pr", "origin/feat/parity", work)
    assert why == "" and sha and sha != head, (sha, why)
    assert git("--git-dir", str(origin), "rev-parse", "refs/heads/pr") == sha, "pushed to the PR's branch"
    assert git("--git-dir", str(origin), "show", f"{sha}:docs/a.md") == "generated"


def _repo_with_commit(path):
    path.mkdir(parents=True)
    for cmd in (["init", "-q"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "x"]):
        subprocess.run(["git", *cmd], cwd=path, check=True)
    return subprocess.run(["git", "rev-parse", "HEAD"], cwd=path, capture_output=True, text=True, check=True).stdout.strip()


def test_the_scanner_line_names_the_tree_it_scanned_not_the_runners_checkout(tmp_path, capsys):
    """alone.log said `tree 09f5219f1` for 16 minutes after parity moved on: the line printed the tooling checkout's HEAD, not the scratch tree the scanners ran in."""
    scratch = _repo_with_commit(tmp_path / "scratch")
    ps.new_scanner_failures([], on_merged=lambda targets: set(), only=("docs_links",), root=tmp_path / "scratch")
    assert f"(tree {scratch[:9]})" in capsys.readouterr().out


def test_an_alone_slot_logs_the_base_it_checked_out(tmp_path, monkeypatch, capsys):
    """The coordinator could not tell which parity an alone slot judged a PR against: the log named no base."""
    monkeypatch.setenv("ALONE_TARGETS", str(tmp_path))
    monkeypatch.setenv("ALONE_SLOTS", "1")
    base = []
    monkeypatch.setattr(ps, "_alone_checkout", lambda work, ref, repo=None: base.append(_repo_with_commit(work)))
    monkeypatch.setattr(ps, "alone_refusals", lambda *a, **k: [])
    monkeypatch.setattr(ps, "keep_refused_evidence", lambda *a, **k: None)
    ps._alone_in_scratch("deadbeef", "origin/feat/parity")
    assert f"alone: slot 0 base {base[0][:9]}" in capsys.readouterr().err


def test_land_skip_link_skips_the_link_stage_for_a_runtime_change(monkeypatch, capsys):
    """The freeze break on feat/parity refused every batch that touched the runtime (09:42, 09:46, 09:48, 09:49); LAND_SKIP_LINK=1 is the user's way past it."""
    monkeypatch.setenv("LAND_SKIP_LINK", "1")

    def build():
        raise AssertionError("the link stage built the release bins")

    assert ps.new_link_failures(["runtime/qb/src/lib.rs"], build=build, check=lambda: ["qb x: freeze"], inputs=lambda: []) == []
    assert "link: skipped (LAND_SKIP_LINK=1)" in capsys.readouterr().out
    monkeypatch.delenv("LAND_SKIP_LINK")
    assert ps.new_link_failures(["runtime/qb/src/lib.rs"], build=lambda: None, check=lambda: ["qb x: freeze"], inputs=lambda: []) == ["qb x: freeze"]


def test_the_batch_age_limit_is_read_from_the_environment():
    """The 20-minute cap ended the 48-PR batches the user asked for; BATCH_MAX_SECONDS sets it per batch (the runner's env), 1200 by default."""
    read = lambda env: subprocess.run([sys.executable, "-c", "import parity_squash as p; print(p.BATCH_MAX_SECONDS)"], cwd=Path(__file__).parent, env={**os.environ, **env}, capture_output=True, text=True).stdout.strip()
    assert read({"BATCH_MAX_SECONDS": "2400"}) == "2400"


def test_merge_batch_takes_as_many_prs_as_batch_cap_says():
    """10:24: the runner assembled 100 PRs (BATCH_CAP=100) and merge-batch died at once with 'takes at most 16 PRs per smoke': MAX_BATCH was a literal."""
    read = lambda env: subprocess.run([sys.executable, "-c", "import parity_squash as p; print(p.MAX_BATCH)"], cwd=Path(__file__).parent, env={**{k: v for k, v in os.environ.items() if k != "BATCH_CAP"}, **env}, capture_output=True, text=True).stdout.strip()
    assert read({"LAND_FAST": "1", "BATCH_CAP": "100"}) == "100"
    assert read({"LAND_FAST": "1"}) == "16" and read({}) == "8"


def test_a_drop_after_the_scan_costs_one_rescan_for_the_rest_not_one_per_pr():
    """10:48-10:50: after a PR was dropped at the squash every later PR got its own rescan (~30 s of 35 s per PR): 141 PRs would take 80 min. The remaining members are scanned once per new drop."""
    groups, ones = [], []
    check = ps.make_rescan_check(rescan_group=lambda g, infos: groups.append(list(g)) or set(), rescan_one=lambda n, info: ones.append(n) or set())
    infos = {n: {"headRefOid": n} for n in "abcd"}
    assert check("a", infos, [], ["a", "b", "c", "d"]) == ""
    assert [check(n, infos, ["x"], ["b", "c", "d"]) for n in "bcd"] == ["", "", ""]
    assert len(groups) == 1 and not ones, "one group scan, no per-PR rescans"
    assert check("d", infos, ["x", "y"], ["d"]) == "" and len(groups) == 2, "a new drop is scanned again"


def test_a_red_group_scan_finds_its_culprit_by_halving_and_refuses_only_that_pr():
    """11:35-11:43: a red group scan fell back to a rescan of every PR in turn (60-90 s each, a PR a minute); halving finds the one in log2(n) scans."""
    scans, ones = [], []
    red = {"integration x::t"}
    rescan_group = lambda g, infos: scans.append(list(g)) or (red if "f" in g else set())
    check = ps.make_rescan_check(rescan_group=rescan_group, rescan_one=lambda n, info: ones.append(n) or (red if n == "f" else set()))
    group = list("bcdefghi")
    infos = {n: {"headRefOid": n} for n in group}
    whys = {n: check(n, infos, ["x"], group) for n in group}
    assert [n for n, why in whys.items() if why] == ["f"] and "fails integration x::t" in whys["f"]
    assert len(scans) <= 1 + 3, f"one group scan and log2(8) halvings, got {len(scans)}"
    assert ones == ["f"], f"one alone check of the culprit, not one per PR: {ones}"


def test_a_red_group_with_no_single_culprit_falls_back_to_a_rescan_per_pr():
    """An interaction: no half is red alone, so each PR is rescanned alone, as before."""
    ones = []
    check = ps.make_rescan_check(rescan_group=lambda g, infos: {"integration x::t"} if len(g) > 2 else set(), rescan_one=lambda n, info: ones.append(n) or set())
    infos = {n: {"headRefOid": n} for n in "bcd"}
    assert [check(n, infos, ["x"], list("bcd")) for n in "bcd"] == ["", "", ""] and ones[1:] == ["b", "c", "d"], "after one alone check of the last candidate"


def test_target_facts_is_a_scanner_every_batch_runs():
    """Parity went red on tests/target_facts.rs (11:09) and no batch noticed: the always-on scanners were docs_in_sync, docs_links and no_local_paths, and target_facts was in prepush only."""
    assert "target_facts" in ps.ALWAYS_SCANNERS


def test_the_files_of_a_head_pushed_after_the_fetch_are_fetched_not_a_crash(tmp_path):
    """11:04: the culprit probe ran `git diff origin/feat/parity...<head of #5183>`, a head pushed after the batch's fetch: 'Invalid symmetric difference', the merge-batch process died with no LANDED line."""
    def sh(cwd, *a):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *a], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()

    origin, clone = tmp_path / "origin", tmp_path / "clone"
    origin.mkdir()
    sh(origin, "init", "-q", "-b", "main")
    (origin / "a.txt").write_text("a\n")
    sh(origin, "add", "-A")
    sh(origin, "commit", "-q", "-m", "a")
    sh(tmp_path, "clone", "-q", str(origin), str(clone))
    (origin / "b.txt").write_text("b\n")
    sh(origin, "add", "-A")
    sh(origin, "commit", "-q", "-m", "b")
    head = sh(origin, "rev-parse", "HEAD")
    sh(origin, "config", "uploadpack.allowAnySHA1InWant", "true")
    base = sh(clone, "rev-parse", "origin/main")
    assert ps.head_files(head, base=base, cwd=clone) == ["b.txt"]


def test_the_group_rescan_regenerates_the_pages_before_it_scans(monkeypatch):
    """11:35, 11:38, 11:42, 11:49: the group scan was red while every member was green alone: the merged members' pass counts leave docs/optimizations/passes/readme.md stale, which only a sync fixes."""
    order = []
    monkeypatch.setattr(ps, "git", lambda *a, **k: "")
    monkeypatch.setattr(ps, "clean_checkout", lambda *a, **k: None)
    monkeypatch.setattr(ps.subprocess, "run", lambda cmd, **k: order.append("sync" if "--sync" in cmd else "git") or subprocess.CompletedProcess(cmd, 0))
    monkeypatch.setattr(ps, "new_scanner_failures", lambda *a, **k: order.append("scan") or set())
    assert ps.rescan_group_alone(["1", "2"], {"1": {"headRefOid": "a"}, "2": {"headRefOid": "b"}}) == set()
    assert order[-2:] == ["sync", "scan"]


def test_land_skip_rescan_is_read_where_the_rescan_check_is_made():
    """12:03-12:12: every PR the squash refused (merge conflicts) cost a ~80 s group rescan, ~1 PR a minute for 40 PRs; the bulk order skips it with LAND_SKIP_RESCAN=1."""
    import inspect  # noqa: PLC0415

    source = inspect.getsource(ps.merge_batch)
    assert 'os.environ.get("LAND_SKIP_RESCAN") == "1"' in source and "make_rescan_check()" in source


def test_the_head_red_limit_is_read_from_the_environment():
    """A target_facts red on parity (12:03) turns every batch into a refusal after HEAD_RED_HOURS (2 h, ~14:03) whatever the members do: the limit is the runner's to set."""
    read = lambda env: subprocess.run([sys.executable, "-c", "import parity_squash as p; print(p.HEAD_RED_HOURS)"], cwd=Path(__file__).parent, env={**{k: v for k, v in os.environ.items() if k != "HEAD_RED_HOURS"}, **env}, capture_output=True, text=True).stdout.strip()
    assert read({"HEAD_RED_HOURS": "6"}) == "6.0" and read({}) == "2.0"


def test_the_scanner_run_has_its_own_stage_so_workspace_check_is_not_its_cost():
    """Ten batches showed `workspace-check 58%`, but the scanners (a cargo test build of the integration targets) ran inside it with no stage line, so its split was a guess.
    The default path now prints `STAGE scanners` after the build check."""
    import inspect

    source = inspect.getsource(ps.merge_batch)
    gate = 'if os.environ.get("LAND_FAST_TESTS") != "1":\n                stage("scanners")'
    assert gate in source, "the default path has a scanners stage"
    assert source.find(gate) < source.find("landing_scanner_failures(git("), "and it comes before the always-on scanner run"


def test_the_workspace_check_runs_beside_the_scanners_and_a_broken_build_wins():
    """sqb-5342 spent 60 s on cargo check and then 100 s on the scanner build, one after the other, though they share no target dir. The default path now runs them together;
    when the tree does not build, the scanner result (taken on a tree that does not compile) is dropped and the build is the refusal."""
    import inspect

    source = inspect.getsource(ps.merge_batch)
    assert "check_and_scan(" in source and "overlap_verdict()" in source
    assert "failed_units.clear()" in source[source.find("def build_refused") :][:400]


def test_the_overlap_falls_back_to_serial_at_a_high_load_and_after_a_slower_run(tmp_path):
    """The host runs at a load near 60: two cargo builds at once could be slower than one after the other. Above OVERLAP_MAX_LOAD the checks run in a row, and an overlapped run
    that took longer than its halves' sum turns the overlap off for an hour."""
    flag = tmp_path / "overlap-off"
    assert ps.check_beside_scanners(load=lambda: (10.0, 0, 0), flag=flag, free=lambda: 64)
    assert not ps.check_beside_scanners(load=lambda: (ps.OVERLAP_MAX_LOAD + 1, 0, 0), flag=flag, free=lambda: 64)
    ps.note_overlap(100, 160, flag=flag)
    assert not flag.exists(), "a faster overlap stays on"
    ps.note_overlap(170, 160, flag=flag)
    assert flag.exists() and not ps.check_beside_scanners(load=lambda: (10.0, 0, 0), flag=flag, free=lambda: 64)
    later = flag.stat().st_mtime + ps.OVERLAP_OFF_SECONDS + 1
    assert ps.check_beside_scanners(load=lambda: (10.0, 0, 0), now=lambda: later, flag=flag, free=lambda: 64)


def test_a_batch_prints_its_stage_times_and_the_gap_after_log_is_pre_squash():
    """The stage-times table was built by hand from the logs, and it blamed `log` (a one-line file append) for the next PR's work before its squash. The batch now ends with
    `STAGES <total>s: ...`, biggest first, per-PR names without `#N`."""
    import stages

    stages._seen.clear()
    for name, at in [("workspace-check", 0), ("scanners", 60), ("squash #1", 160), ("log #1", 166), ("squash #2", 170), ("log #2", 176)]:
        stages.mark(name, at)
    line = stages.summary(end=181)
    assert line == "STAGES 181s: scanners 100s workspace-check 60s squash 12s pre-squash 9s", line
    assert stages.summary() == "", "a summary starts the next batch afresh"
    import inspect

    assert "stage_summary()" in inspect.getsource(ps.merge_batch)


def test_the_drift_prone_scanners_run_every_batch():
    """A red scanner outside the default scan hid on parity for hours (the SD registry, 12:03 on). no_todo, tracked_build_products, block_list_ratchet and no_kill_by_pattern read
    sources and the file list (0.02-5.6 s each on parity, all green) and are in the default scan."""
    assert {"no_todo", "tracked_build_products", "block_list_ratchet", "no_kill_by_pattern"} <= set(ps.ALWAYS_SCANNERS)


def test_build_script_reruns_runs_when_the_diff_touches_a_build_script_input_and_not_otherwise():
    """It runs three release bin builds (67 s at load 80) and was in no default scan; it now runs on a diff that touches a build.rs, a Cargo.toml/lock or a selector-generator input,
    and the full scan (tests=True) always runs it."""
    assert "build_script_reruns" not in ps.scanners_for(["docs/a.md", "crates/opt/llrm-transforms/src/x.rs"])
    for touched in ["crates/codegen/llrm-codegen/build.rs", "Cargo.lock", "crates/opt/llrm-x/Cargo.toml", "crates/target/llrm-x86/src/isel/patterns.isel", "crates/backend/llrm-iselgen/src/build.rs"]:
        assert "build_script_reruns" in ps.scanners_for([touched]), touched
    asked = []
    ps.landing_scanner_failures(["crates/codegen/llrm-codegen/build.rs"], False, on_merged=lambda t: asked.append(list(t)) or set(), on_head=lambda t: set())
    ps.landing_scanner_failures(["docs/a.md"], False, on_merged=lambda t: asked.append(list(t)) or set(), on_head=lambda t: set())
    assert "build_script_reruns" in asked[0] and "build_script_reruns" not in asked[1]
    asked.clear()
    ps.landing_scanner_failures(["docs/a.md"], True, on_merged=lambda t: asked.append(list(t)) or set(), on_head=lambda t: set())
    assert "build_script_reruns" in asked[0]


def test_the_overlap_says_why_it_is_serial_and_keeps_to_memory_and_the_core_count(tmp_path):
    """The audit of five landings showed check then scanners: the overlap never engaged (load 78 on 32 cores against a limit of 60) and nothing in the log said so. The limit is 3 x cores,
    the verdict names its reason, and under 8 GB free (oomd kills sessions) the builds run in a row."""
    flag = tmp_path / "overlap-off"
    assert ps.OVERLAP_MAX_LOAD == 3 * (os.cpu_count() or 1) or "OVERLAP_MAX_LOAD" in os.environ
    assert ps.overlap_verdict(load=lambda: (ps.OVERLAP_MAX_LOAD, 0, 0), flag=flag, free=lambda: 64) == "", "a load at the limit overlaps"
    assert ps.overlap_verdict(load=lambda: (ps.OVERLAP_MAX_LOAD + 5, 0, 0), flag=flag, free=lambda: 64).startswith("load ")
    assert ps.overlap_verdict(load=lambda: (1.0, 0, 0), flag=flag, free=lambda: 3.0) == "3 GB free is under 8"
    assert ps.overlap_verdict(load=lambda: (1.0, 0, 0), flag=flag, free=lambda: 64) == ""
    import inspect

    assert "check beside scanners: {" in inspect.getsource(ps.merge_batch), "the decision is logged either way"


def test_every_scanner_is_in_the_default_scan_the_conditional_one_or_nightly():
    """The SD registry sat red on parity for two hours behind a default scan of four of the fifteen scanners. A scanner added to the full map without a place in the default scan
    (or the build-input trigger, or NIGHTLY_ONLY) is now a failure here."""
    smoke = ps.parity_smoke_module()
    placed = set(ps.ALWAYS_SCANNERS) | {"build_script_reruns"} | set(smoke.NIGHTLY_ONLY)
    assert set(smoke.SCANNERS) <= placed, sorted(set(smoke.SCANNERS) - placed)


def test_the_overlapped_check_returns_its_error_text_and_the_scan_verdict_not_each_others(tmp_path, monkeypatch):
    """sq-5420.log 15:14: `broken` held the scan's True and the build's text was lost (`True[:300]` TypeError); #5420 was refused for a crash, and every later PR whose check failed would have been."""
    monkeypatch.setenv("LAND_TMP", str(tmp_path))
    assert ps.check_and_scan(lambda: "error[E0425]: cannot find value", lambda: True) == ("error[E0425]: cannot find value", True)
    assert ps.check_and_scan(lambda: "", lambda: False) == ("", False)
    assert ps.check_and_scan(lambda: "", lambda: True) == ("", True)

def test_precheck_runs_no_ratchet_the_batch_smoke_runs_them_all():
    """Precheck ran the 12 ratchets (median 80 s a PR at load 70) that every batch's smoke runs again on the merged tree and the head: 12 PRs of pre-flight held the first batch 40 min."""
    assert [name for name, _ in ps.precheck_checks(Path("."), Path("."))] == ["fmt", "docs"]


def test_a_conflict_detail_names_the_owner_and_the_conflicting_file(tmp_path):
    """#5456 and #5462 were refused at squash for merge conflicts: the line said neither who owns them nor where (dag_function_tests.rs); the coordinator ran git merge-tree by hand."""

    def git(*args):
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True, check=True).stdout.strip()

    git("init", "-q", "-b", "main")
    (tmp_path / "tests.rs").write_text("one\n")
    git("add", "."), git("commit", "-qm", "base")
    git("checkout", "-q", "-b", "pr")
    (tmp_path / "tests.rs").write_text("pr\n")
    git("commit", "-qam", "pr")
    head = git("rev-parse", "HEAD")
    git("checkout", "-q", "main")
    (tmp_path / "tests.rs").write_text("landed\n")
    git("commit", "-qam", "landed")
    assert ps.conflict_detail(head, "TL;DR\n\nOwner: w10\nRows: none", "main", tmp_path) == " [owner w10; conflicts in: tests.rs]"
    assert ps.conflict_detail(head, "", "main", tmp_path).startswith(" [owner unknown;")


def test_a_squash_refused_for_merge_conflicts_prints_the_owner_and_files(capsys, monkeypatch):
    monkeypatch.setattr(ps, "conflict_detail", lambda head, body, *rest: " [owner w10; conflicts in: dag_function_tests.rs]")
    info = {"mergeable": "MERGEABLE", "headRefOid": "a" * 40, "title": "t", "headRefName": "b", "body": "Owner: w10"}
    refuse = lambda command, **kwargs: SimpleNamespace(returncode=1, stdout="", stderr="GraphQL: Pull Request has merge conflicts (mergePullRequest)\n")  # noqa: E731
    assert ps.squash("5462", info, "body", run=refuse) is False
    assert "has merge conflicts (mergePullRequest) [owner w10; conflicts in: dag_function_tests.rs]" in capsys.readouterr().out


def test_a_scanner_refusal_names_the_prs_and_files_behind_it():
    """sqb-5378 refused on target_facts with three files and no PR; the owners were found by hand (the file list was the only clue)."""
    failing = {"integration target_facts::shared_code_holds_no_copy_of_a_target_fact: read the target's description instead of copying it: [crates/codegen/llrm-codegen/src/backend/funnel.rs: 1 copies of a target fact, none allowed; crates/codegen/llrm-codegen/src/backend/latecleanup.rs: 3 copies; tools/other.py: 1]"}
    members = lambda: {"5424": ["crates/codegen/llrm-codegen/src/backend/funnel.rs"], "5428": ["crates/codegen/llrm-codegen/src/backend/latecleanup.rs", "x.rs"], "5409": ["unrelated.rs"]}
    got = ps.offender_detail(failing, members)
    assert got == " [offending files: #5424 crates/codegen/llrm-codegen/src/backend/funnel.rs; #5428 crates/codegen/llrm-codegen/src/backend/latecleanup.rs; no PR of the batch: tools/other.py]", got
    assert ps.offender_detail({"lib x::t: failed"}, members) == ""


def test_a_link_refusal_names_the_prs_touching_the_directory_of_the_file_that_panicked():
    """sqb-5413's link refusal said `panicked at greedy/spliteditor.rs:125` and no PR; it was #5486's change in the same directory, found by w13's bisect."""
    unlinked = ["qb arena_type: llrm-c: thread 'main' (2594298) panicked at crates/codegen/llrm-codegen/src/greedy/spliteditor.rs:125:9:"]
    members = lambda: {"5443": ["crates/codegen/llrm-codegen/src/greedy/spiller.rs"], "5486": ["crates/codegen/llrm-codegen/src/greedy/spill.rs"], "5409": ["crates/x/src/other.rs"]}
    got = ps.offender_detail(unlinked, members, near=True)
    assert got == " [offending files: crates/codegen/llrm-codegen/src/greedy/spliteditor.rs: same directory touched by #5443 #5486]", got
    assert "no PR of the batch" in ps.offender_detail(unlinked, lambda: {"5409": ["crates/x/src/other.rs"]}, near=True)


def test_every_scanner_is_in_the_default_scan_the_conditional_one_or_nightly():
    """The SD registry sat red on parity for two hours behind a default scan of four of the fifteen scanners. A scanner added to the full map without a place in the default scan
    (or the build-input trigger, or NIGHTLY_ONLY) is now a failure here."""
    smoke = ps.parity_smoke_module()
    placed = set(ps.ALWAYS_SCANNERS) | {"build_script_reruns"} | set(smoke.NIGHTLY_ONLY)
    assert set(smoke.SCANNERS) <= placed, sorted(set(smoke.SCANNERS) - placed)


def test_a_diff_that_touches_a_tool_runs_that_tools_python_tests_and_a_red_one_refuses_the_batch(tmp_path):
    """test_assemble.py sat red on parity for hours: landing ran Rust tests only, so a tools/ change never ran the Python tests that guard it."""
    (tmp_path / "tools").mkdir()
    for name in ("parity_squash.py", "test_parity_squash.py", "test_other.py", "lonely.py"):
        (tmp_path / "tools" / name).write_text("")
    assert ps.tool_tests_for(["tools/parity_squash.py", "tools/test_other.py", "tools/lonely.py", "docs/a.md", "crates/x.rs"], tmp_path) == ["tools/test_other.py", "tools/test_parity_squash.py"]
    assert ps.tool_tests_for(["docs/a.md"], tmp_path) == []
    ran = []

    class Done:
        returncode = 1
        stdout = "FAILED tools/test_other.py::test_x\n1 failed"
        stderr = ""

    assert ps.tool_test_failures(["tools/test_other.py"], tmp_path, run=lambda command, **_: ran.append(command) or Done(), ensure=lambda: "") == ["FAILED tools/test_other.py::test_x", "1 failed"]
    assert "tools/test_other.py" in ran[0]
    assert ps.tool_test_failures(["docs/a.md"], tmp_path, run=lambda *a, **k: ran.append("never")) == [] and "never" not in ran


def test_the_tool_tests_wait_for_fresh_binaries_instead_of_dying_on_stale_ones(tmp_path):
    """The 02:32 batch was refused ("stale binaries ... run cargo build --release --bins") by its own tool-tests stage: pytest ran before the build stage had refreshed the release binaries."""
    (tmp_path / "tools").mkdir()
    (tmp_path / "tools" / "test_other.py").write_text("")
    order = []

    class Done:
        returncode = 0
        stdout = stderr = ""

    assert ps.tool_test_failures(["tools/test_other.py"], tmp_path, run=lambda command, **_: order.append("pytest") or Done(), ensure=lambda: order.append("ensure") or "") == []
    assert order == ["ensure", "pytest"], order
    order.clear()
    assert ps.tool_test_failures(["tools/test_other.py"], tmp_path, run=lambda *a, **k: order.append("pytest"), ensure=lambda: "binaries still older than the merged tree") == ["binaries still older than the merged tree"]
    assert "pytest" not in order
    import inspect

    assert "tool_test_failures(" in inspect.getsource(ps.merge_batch)



def test_the_rebuild_includes_llrm_c_when_the_directory_holds_one(tmp_path):
    """`cargo build --release --bins` skips llrm-c (required-features = c-legacy): the llrm-c left in coord-ps-target stayed 02:55 old and refused the 03:12 pass ("binaries still older than the merged tree")."""
    assert "wccq" not in ps.bins_command(tmp_path)
    (tmp_path / "llrm-c").write_text("")
    assert ps.bins_command(tmp_path)[-2:] == ["--features", "wccq"]


def test_the_link_and_behavioural_build_refreshes_llrm_c(tmp_path, monkeypatch):
    """_build_release ran `--bins` without the llrm-c feature, then the stages touched the old llrm-c: QB programs linked with a stale compiler ("isel: value#15 is defined 2 times") and the 04:51 pass was refused at link five times."""
    (tmp_path / "release").mkdir()
    (tmp_path / "release" / "llrm-c").write_text("")
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path))
    monkeypatch.setattr(ps, "git", lambda *args, **kw: "tree")
    monkeypatch.setattr(ps, "_BUILT", set())
    ran = []
    monkeypatch.setattr(ps.subprocess, "run", lambda command, **kw: ran.append(command))
    ps._build_release()
    assert ran and "wccq" in ran[0], ran


def test_a_failing_tool_test_keeps_its_error_text_not_only_the_last_lines(tmp_path):
    """Only the last 6 lines of pytest were kept: #5690 was refused five times with "BuildError" and the stderr naming the cause was cut."""
    (tmp_path / "tools").mkdir()
    (tmp_path / "tools" / "test_other.py").write_text("")

    class Done:
        returncode = 1
        stdout = "\n".join(["E   dosbatch.BuildError: llrm-qb: the real cause", *["    frame"] * 30, "ERROR tools/x.py::t", "1 error"])
        stderr = ""

    got = ps.tool_test_failures(["tools/test_other.py"], tmp_path, run=lambda command, **_: Done(), ensure=lambda: "")
    assert any("the real cause" in line for line in got), got


def test_the_tool_tests_of_a_landing_leave_the_dosbox_bench_programs_to_the_gate(tmp_path):
    """#5690's tool-tests stage ran every bench program in DOSBox, minutes each, and ran into the 600 s cap; the batch was refused on a test the gate owns."""
    (tmp_path / "tools").mkdir()
    (tmp_path / "tools" / "test_other.py").write_text("")
    ran = []

    class Done:
        returncode = 0
        stdout = stderr = ""

    ps.tool_test_failures(["tools/test_other.py"], tmp_path, run=lambda command, **_: ran.append(command) or Done(), ensure=lambda: "")
    assert ps.GATE_ONLY_TESTS and all(f"--deselect={one}" in ran[0] for one in ps.GATE_ONLY_TESTS), ran


def test_a_tool_test_that_times_out_refuses_the_batch_instead_of_crashing_it(tmp_path, monkeypatch):
    """#5690's pytest ran past 600 s: TimeoutExpired went uncaught, the batch died with a traceback and no REFUSED line, and the PR came back every pass."""
    monkeypatch.setenv("TOOLTEST_CAP", "600")
    (tmp_path / "tools").mkdir()
    (tmp_path / "tools" / "test_other.py").write_text("")

    def slow(command, **kw):
        raise ps.subprocess.TimeoutExpired(command, 600)

    got = ps.tool_test_failures(["tools/test_other.py"], tmp_path, run=slow, ensure=lambda: "")
    assert got and "600 s" in got[0], got


def test_a_red_naming_a_file_asks_the_prs_touching_it_before_the_rest():
    """The 06:53 batch refused a target_facts red naming fpregs.rs and then asked 37 PRs one by one, 80 s each; one PR touched that file."""
    asked = []
    alone = lambda number: asked.append(number) or number == "3"  # noqa: E731
    assert ps.red_members_suspects_first(["1", "2", "3", "4", "5"], ["3"], None, alone) == ["3"]
    assert asked == ["3"], asked
    asked.clear()
    alone = lambda number: asked.append(number) or number == "5"  # noqa: E731
    assert ps.red_members_suspects_first(["1", "2", "3", "4", "5"], ["3"], None, alone) == ["5"]
    assert asked[0] == "3", asked
    assert ps.files_named_in("[integration t::x: crates/a/src/fpregs.rs: 7 copies; crates/b/mod.rs: 2]") == {"crates/a/src/fpregs.rs", "crates/b/mod.rs"}
    assert ps.files_named_in("[tools/pairs/dyn.py: 1 copies of a target fact, none allowed]") == {"tools/pairs/dyn.py"}  # the 07:01 red named a .py file


def test_no_function_assigns_a_local_that_shadows_a_module_function_it_calls():
    """merge_batch's build branch assigned `named_files = ...` while its integration branch called the module's named_files(): UnboundLocalError crashed the 07:39 pass (#5703)."""
    import ast
    import inspect

    tree = ast.parse(inspect.getsource(ps))
    module_functions = {node.name for node in tree.body if isinstance(node, ast.FunctionDef)}
    clashes = []
    for fn in ast.walk(tree):
        if isinstance(fn, ast.FunctionDef):
            stores = {n.id for n in ast.walk(fn) if isinstance(n, ast.Name) and isinstance(n.ctx, ast.Store)}
            calls = {n.func.id for n in ast.walk(fn) if isinstance(n, ast.Call) and isinstance(n.func, ast.Name)}
            clashes += [f"{fn.name}: {name}" for name in sorted(stores & calls & module_functions)]
    assert clashes == [], clashes


def test_format_tip_commits_a_generated_page_that_is_new(tmp_path):
    """New-pass pages (rpoattrs, deadmi, calledvaluepropagation) landed without their generated .md three times: the sync wrote them untracked, `commit -a` skipped them and --check passed on the file on disk."""
    sh(tmp_path, "init", "-q", "-b", "main")
    (tmp_path / "tools" / "docs" / "docsbuild").mkdir(parents=True)
    (tmp_path / "tools" / "fmt.sh").write_text("#!/bin/sh\n")
    (tmp_path / "tools" / "fmt.sh").chmod(0o755)
    (tmp_path / "tools" / "docs" / "docsbuild" / "build.py").write_text(
        "import pathlib\npathlib.Path('docs/optimizations/passes').mkdir(parents=True, exist_ok=True)\npathlib.Path('docs/optimizations/passes/newpass.md').write_text('# newpass\\n')\n"
    )
    sh(tmp_path, "add", "-A")
    sh(tmp_path, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base")
    assert ps.format_tip(tmp_path) is True
    assert "docs/optimizations/passes/newpass.md" in sh(tmp_path, "ls-files"), "the new page is not in the tip"


def test_two_halves_that_build_apart_and_fail_together_name_the_later_pr():
    """#5784 and #5830 each passed alone and the batch failed with "missing field seh": the second half was tested on a base without the first half's PRs
    (PUSH_MODE lands nothing until the end), both halves passed, and the push met the pair; the set was assembled again each pass."""
    pending, refused, tested = [], [], []

    def passes(group):
        tested.append(list(group))
        return not {"2", "3"} <= set(group)

    ps.batch(["1", "2", "3", "4"], passes, pending.append, refused.append, pending=pending)
    assert refused == ["3"]
    assert pending == ["1", "2", "4"]
    assert ["1", "2", "3"] in tested

def test_the_integration_probe_names_what_integration_failures_returns():
    """The 16:17 batch failed `integration target_facts::shared_code_holds_no_copy_of_a_target_fact`; the halving probe held the name without `integration `, so
    `probe & integration_failures(...)` was empty for every subset and #5784 (red alone) was cleared in 37 s."""
    import parity_smoke

    output = "     Running tests/target_facts.rs (target/release/deps/target_facts-1)\ntest shared_code_holds_no_copy_of_a_target_fact ... FAILED\n"
    seen = set(parity_smoke.red_tests(output))
    probe, targets = ps.integration_probe({"integration target_facts::shared_code_holds_no_copy_of_a_target_fact", "unit llrm::x"}, set())
    assert probe & seen == seen and targets == ["target_facts"]
    assert ps.integration_probe({"integration a::t"}, {"integration a::t"}) == (set(), [])


def test_a_tool_test_run_past_the_old_cap_is_no_refusal_when_the_cap_is_raised(tmp_path, monkeypatch):
    """The tool-tests cap was a fixed 600 s: a test file that needs 15 minutes refused every batch that touched it ("ran past 600 s"). TOOLTEST_CAP sets it (default 1800)."""
    import subprocess

    (tmp_path / "tools").mkdir()
    (tmp_path / "tools/test_slow.py").write_text("")

    class Done:
        returncode = 0
        stdout = stderr = ""

    def run(command, timeout, **_):
        if timeout < 900:  # the tests need 900 s
            raise subprocess.TimeoutExpired(command, timeout)
        return Done()

    monkeypatch.delenv("TOOLTEST_CAP", raising=False)
    assert ps.tool_test_failures(["tools/test_slow.py"], tmp_path, run=run, ensure=lambda: "") == []  # the default 1800 s
    monkeypatch.setenv("TOOLTEST_CAP", "600")
    assert ps.tool_test_failures(["tools/test_slow.py"], tmp_path, run=run, ensure=lambda: "") == ["the tool tests ran past 600 s"]


def test_the_tests_get_the_clang_of_the_trees_own_pin(tmp_path):
    """#5789 bumped the pin's m16 sha in a batch while the runner's LLRM_CLANG_M16 stayed the old build: the dosbatch tests refused the batch ("not the pinned clang")."""
    (tmp_path / "tree/toolchain/clang").mkdir(parents=True)
    (tmp_path / "tree/toolchain/clang/pin").write_text("aaaa # upstream\nbbbb # m16 fork\n")
    for sha in ("bbbb", "cccc"):
        (tmp_path / "by" / sha / "bin").mkdir(parents=True)
        (tmp_path / "by" / sha / "bin/clang").write_text("")
    env = {"LLRM_CLANG_M16": "/old/clang"}
    ps.with_pinned_clang(env, tmp_path / "tree", tmp_path / "by")
    assert env["LLRM_CLANG_M16"] == str(tmp_path / "by/bbbb/bin/clang")
    (tmp_path / "tree/toolchain/clang/pin").write_text("aaaa\ndddd\n")
    env = {"LLRM_CLANG_M16": "/old/clang"}
    ps.with_pinned_clang(env, tmp_path / "tree", tmp_path / "by")
    assert env["LLRM_CLANG_M16"] == "/old/clang", "no build for the sha: the runner's value stays"


def test_a_pr_whose_description_has_no_judge_line_for_its_head_is_refused_before_anything_runs(tmp_path, monkeypatch):
    """#6095 landed with the bench judge red on 56 rows: the fast set does not run the judge, so the description must carry its line from this head."""
    monkeypatch.setattr(ps, "git", lambda *a, **k: (_ for _ in ()).throw(AssertionError("a refused PR runs nothing")))
    head = "a" * 40
    info = {"headRefOid": head, "baseRefName": "feat/parity"}
    for body in ("", None, "problems: 3 @ " + head, "problems: 0", "the judge said problems: 0 @ " + head + " today"):
        line = ps.precheck("6095", info={**info, "body": body}, cache=tmp_path / "c.json", base_sha="base")
        assert line.startswith("6095: REFUSED: the description has no judge line"), (body, line)
    other = ps.precheck("6095", info={**info, "body": "problems: 0 @ " + "b" * 40}, cache=tmp_path / "c.json", base_sha="base")
    assert other == f"6095: REFUSED: the judge line is for {'b' * 12}, the head is {'a' * 12}: run the judge on this head and update the description"


def test_a_pr_with_the_judge_line_for_its_head_passes_that_check_and_an_edited_description_is_read_again(tmp_path, monkeypatch):
    tree = Tree(set(), set())
    assert run_precheck(tmp_path, monkeypatch, tree, number="6096") == "6096: PASS"
    info = {"headRefOid": "h" * 40, "baseRefName": "feat/parity"}
    assert ps.judge_line_refusal("1", {**info, "body": "TL;DR\nproblems: 0 @ " + "h" * 40 + "\nOwner: w6"}) is None
    assert ps.judge_line_refusal("1", {**info, "body": "problems: 0 \uff20 " + "h" * 40}) is None, "the full-width at sign the order was written with"
    cache = tmp_path / "judge-cache.json"
    kwargs = dict(cache=cache, base_sha="base")
    assert "no judge line" in ps.precheck("1", info={**info, "body": ""}, **kwargs)
    assert "no judge line" not in ps.precheck("1", info={**info, "body": "problems: 0 @ " + "h" * 40}, **{**kwargs, "checks": Tree(set(), set()).checks(), "merge": Tree(set(), set()).merge, "deletions": lambda *a: [], "dev": tmp_path})


def test_a_pr_that_touches_no_compiler_path_needs_no_judge_line(tmp_path, monkeypatch):
    """Without the exemption #6102 and every tools-only or docs-only PR would be refused by the rule: they cannot move the bench judge."""
    head = "c" * 40
    info = {"headRefOid": head, "baseRefName": "feat/parity", "body": ""}
    tools_only = [{"path": "tools/parity_squash.py"}, {"path": "docs/working/pr-rules.md"}]
    assert ps.judge_line_refusal("1", {**info, "files": tools_only}) is None
    assert ps.judge_line_refusal("1", {**info, "files": [*tools_only, {"path": "crates/opt/src/lib.rs"}]}).startswith("1: REFUSED: the description has no judge line"), "one compiler file brings the rule back"
    assert ps.judge_line_refusal("1", {**info, "files": [{"path": "Cargo.lock"}]}) is not None
    assert ps.judge_line_refusal("1", info) is not None, "no file list read: the rule applies"
