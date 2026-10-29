"""tools/prepush.sh: one verdict line first, the gate's own commands, and a bad tree turns it red (fmt, a stale generated doc, a local path)."""

import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))

import gate  # noqa: E402
import prepush  # noqa: E402


def test_the_verdict_leads_and_names_the_failing_steps():
    results = [("fmt", 1, 3.0, "files would change"), ("docs", 0, 1.0, ""), ("scans", 1, 9.0, "boom")]
    assert prepush.verdict(results) == "PREPUSH FAIL: fmt, scans"
    assert prepush.verdict([("fmt", 0, 1.0, "")]) == "PREPUSH PASS"


def test_the_steps_and_tests_come_from_tiers_toml_not_a_list_of_its_own():
    """A scan added to [[scan]] runs in the next prepush: its `scans` command is the gate's, and the root tests are tiers.toml's."""
    cfg = gate.load()
    chosen = prepush.selected(cfg)
    assert set(chosen) == set(cfg["prepush"]["steps"]) | {f"test {name}" for name in cfg["prepush"]["tests"]} | set(cfg["prepush"]["checks"])
    assert "--test no_local_paths" in chosen["scans"] and "--test target_facts" in chosen["facts"] and "docsbuild" in chosen["docs"]
    assert "no_uncached_env_checks" in chosen["scans"] and "--test docs_links" in chosen["test docs_links"]
    assert prepush.selected(cfg, ["docs"]).keys() == {"docs"}


def prepushed(copy: Path, *only: str) -> subprocess.CompletedProcess:
    env = {**os.environ, "CARGO_TARGET_DIR": str(copy / "target")}
    return subprocess.run([str(copy / "tools/prepush.sh"), "--only", *only], cwd=copy, capture_output=True, text=True, env=env)


@pytest.fixture(scope="module")
def checkout(tmp_path_factory):
    """One copy of HEAD (tracked files only), damaged by a test and restored after it."""
    copy = tmp_path_factory.mktemp("prepush") / "tree"
    subprocess.run(["git", "worktree", "add", "-q", "--detach", str(copy), "HEAD"], cwd=ROOT, check=True)
    yield copy
    subprocess.run(["git", "worktree", "remove", "--force", str(copy)], cwd=ROOT, check=False)


@pytest.fixture
def damaged(checkout):
    yield checkout
    subprocess.run(["git", "checkout", "-q", "--", "."], cwd=checkout, check=True)


def test_a_stale_generated_doc_turns_it_red(damaged):
    """docs/optimizations/passes/*.md is generated from the `//!` header of the pass: editing the header without regenerating is the commonest red."""
    source = damaged / "crates/opt/llrm-transforms/src/algebraic.rs"
    text = source.read_text()
    source.write_text(text.replace("//!", "//! A sentence the generated page does not have yet.\n//!", 1))
    done = prepushed(damaged, "docs")
    assert done.stdout.splitlines()[0] == "PREPUSH FAIL: docs", done.stdout + done.stderr
    assert prepushed(damaged, "products").stdout.splitlines()[0] == "PREPUSH PASS"


@pytest.mark.skipif(shutil.which("rustup") is None, reason="needs the nightly rustfmt the fmt step uses")
def test_a_badly_formatted_rust_file_turns_the_fmt_step_red(damaged):
    source = damaged / "src/bin/llrm-cc.rs"
    source.write_text(source.read_text() + "\nfn   unformatted ( ) { let x   =  1 ; }\n")
    done = prepushed(damaged, "fmt")
    assert gate.load()["fmt"]["enforced"], "the fmt step must be enforced for prepush to be able to turn red on it"
    assert done.stdout.splitlines()[0] == "PREPUSH FAIL: fmt", done.stdout


def test_a_local_path_in_a_tracked_markdown_turns_the_scan_red(damaged, tmp_path):
    """The scan `scans` and prepush run is tests/no_local_paths.rs; built with rustc directly it reads the damaged tree's tracked files."""
    readme = damaged / "docs" / "readme.md"
    readme.write_text(readme.read_text() + "\nsee /home/someone/notes.txt\n")
    subprocess.run(["git", "add", "-A"], cwd=damaged, check=True)
    lines = (damaged / "tests/no_local_paths.rs").read_text().splitlines()
    head = lines[: next(n for n, line in enumerate(lines) if line.startswith("fn repo("))]  # the rest are self-tests that need the tempfile crate
    source = tmp_path / "no_local_paths.rs"
    source.write_text("\n".join(head) + "\n")
    binary = tmp_path / "no_local_paths"
    built = subprocess.run(["rustc", "--edition", "2021", "--test", str(source), "-o", str(binary)], capture_output=True, text=True, env={**os.environ, "CARGO_MANIFEST_DIR": str(damaged)})
    assert built.returncode == 0, built.stderr[-300:]
    done = subprocess.run([str(binary), "test_no_tracked_markdown_names_a_path_of_one_machine"], cwd=damaged, capture_output=True, text=True)
    assert done.returncode != 0 and "/home/" in done.stdout + done.stderr, done.stdout[-400:]


def test_a_conflict_marker_left_by_a_merge_turns_the_diff_check_red(damaged):
    """basicaa-port pushed a commit with conflict markers: `git diff --check` and the marker grep see it before CI does."""
    notes = damaged / "tools/conflict_marker_fixture.txt"
    notes.write_text("a\n<<<<<<< HEAD\nb\n=======\nc\n>>>>>>> origin/main\n")
    subprocess.run(["git", "add", "-A"], cwd=damaged, check=True)
    done = prepushed(damaged, "diff check")
    assert done.stdout.splitlines()[0] == "PREPUSH FAIL: diff check", done.stdout + done.stderr
    subprocess.run(["git", "rm", "-q", "-f", "--cached", str(notes)], cwd=damaged, check=True)
    notes.unlink()
    assert prepushed(damaged, "diff check").stdout.splitlines()[0] == "PREPUSH PASS"


def test_trailing_whitespace_in_the_diff_turns_the_diff_check_red(damaged):
    notes = damaged / "tools/whitespace_fixture.txt"
    notes.write_text("a line with trailing blanks   \n")
    subprocess.run(["git", "add", "-A"], cwd=damaged, check=True)
    assert prepushed(damaged, "diff check").stdout.splitlines()[0] == "PREPUSH FAIL: diff check"
    subprocess.run(["git", "rm", "-q", "-f", "--cached", str(notes)], cwd=damaged, check=True)
    notes.unlink()


def test_a_command_whose_output_is_not_utf8_fails_the_step_not_the_prepush():
    """`git diff --check` printed a byte 0xdb of a tracked binary-ish file and run_all died with a UnicodeDecodeError: no PREPUSH line at all, so the diff-check test read an empty stdout."""
    results, _ = prepush.run_all({"bytes": "printf '\\333 bad\\n'; exit 1"}, dict(os.environ))
    assert results[0][0] == "bytes" and results[0][1] == 1 and "bad" in results[0][3]
    assert prepush.verdict(results) == "PREPUSH FAIL: bytes"


def test_the_diff_check_looks_at_the_prs_own_delta_not_at_what_parity_has_over_main(tmp_path):
    """Against origin/main the check found ~13k whitespace findings in data files that feat/parity already has, so PREPUSH PASS could not hold on a parity tree. Old whitespace in parity is
    not the PR's; a conflict marker it adds is."""
    import tomllib

    command = tomllib.loads((ROOT / "tools/gate/tiers.toml").read_text())["prepush"]["checks"]["diff check"]
    repo = tmp_path / "repo"

    def git(*args):
        subprocess.run(["git", "-c", "user.email=t@t", "-c", "user.name=t", *args], cwd=repo, check=True, capture_output=True)

    repo.mkdir()
    git("init", "-q", "-b", "main")
    (repo / "a.txt").write_text("a\n")
    git("add", "-A")
    git("commit", "-qm", "main")
    git("update-ref", "refs/remotes/origin/main", "HEAD")
    (repo / "old.tsv").write_text("row \n")  # trailing whitespace that parity has and main does not
    git("add", "-A")
    git("commit", "-qm", "parity")
    git("update-ref", "refs/remotes/origin/feat/parity", "HEAD")
    run = lambda: subprocess.run(command, shell=True, cwd=repo, capture_output=True, text=True)  # noqa: E731
    assert run().returncode == 0, run().stdout
    (repo / "marker.txt").write_text("a\n<<<<<<< HEAD\nb\n")
    git("add", "-A")
    assert run().returncode != 0
    git("rm", "-q", "-f", "--cached", "marker.txt")
    (repo / "marker.txt").unlink()
    assert run().returncode == 0
