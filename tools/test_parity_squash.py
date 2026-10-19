"""#2329 squashed a stale head and reverted decide's LVI step (a82b332be): the squash must refuse what a bare `gh pr merge` merged."""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

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
