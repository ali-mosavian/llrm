"""The worker-side precheck: one case per kind of refusal the runner gave in the two hours before it was written."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parent))
import precheck_pr as pc  # noqa: E402


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout.strip()


def repo(tmp_path: Path) -> Path:
    git(tmp_path, "init", "-q", "-b", "parity")
    git(tmp_path, "config", "user.email", "t@t")
    git(tmp_path, "config", "user.name", "t")
    (tmp_path / "a.txt").write_text("one\ntwo\n")
    (tmp_path / "keep.rs").write_text("fn a() {}\n#[test]\nfn t() {}\n")
    git(tmp_path, "add", "-A")
    git(tmp_path, "commit", "-q", "-m", "base")
    git(tmp_path, "checkout", "-q", "-b", "pr")
    return tmp_path


def commit(root: Path, name: str, text: str, branch: str) -> None:
    git(root, "checkout", "-q", branch)
    (root / name).write_text(text)
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", f"change {name}")


def test_a_branch_that_conflicts_with_the_tip_names_the_files(tmp_path):
    """#3436's batch refused PRs that did not merge with release/next: the owner learns it from the runner an hour later."""
    root = repo(tmp_path)
    commit(root, "a.txt", "one\nthree\n", "parity")
    commit(root, "a.txt", "one\nfour\n", "pr")
    why = pc.branch_merges(root, "parity")
    assert len(why) == 1 and "conflicts with parity in a.txt" in why[0]


def test_a_branch_without_the_tip_but_with_no_conflict_is_told_to_merge_it(tmp_path):
    root = repo(tmp_path)
    commit(root, "b.txt", "x\n", "parity")
    commit(root, "c.txt", "y\n", "pr")
    why = pc.branch_merges(root, "parity")
    assert len(why) == 1 and "does not contain parity" in why[0]
    git(root, "merge", "-q", "--no-edit", "parity")
    assert pc.branch_merges(root, "parity") == []


def test_a_description_needs_owner_and_rows_lines_at_the_start_of_a_line():
    """`3698: REFUSED: no Owner: <session> line`: a mention inside a sentence is not the line."""
    assert pc.description_failures("fix: x", "TL;DR: y\nOwner: w-8\nRows: none") == []
    why = pc.description_failures("fix: x", "TL;DR: y. Owner: w-8 said so\nrows are none")
    assert len(why) == 2 and "Owner" in why[0] and "Rows" in why[1]


def test_a_squash_that_deletes_a_file_needs_it_named_in_a_deletes_line(tmp_path):
    """`the squash deletes N lines or a test function`: unnamed, the runner refuses; named in `Deletes:`, it passes."""
    root = repo(tmp_path)
    git(root, "rm", "-q", "keep.rs")
    git(root, "commit", "-q", "-m", "remove keep")
    refused = pc.parity_squash.body_checks("HEAD", "Owner: x\nRows: none", "parity", root, "fix: remove")
    assert any("keep.rs" in line and "Deletes" in line for line in refused), refused
    assert pc.parity_squash.body_checks("HEAD", "Owner: x\nRows: none\nDeletes: keep.rs", "parity", root, "fix: remove") == []


def test_an_added_line_naming_a_config_dir_path_is_refused(tmp_path):
    root = repo(tmp_path)
    commit(root, "notes.md", "see " + "." + "clau" + "de/worktrees/x for it\n", "pr")
    why = pc.config_dir_paths(root, "parity")
    assert len(why) == 1 and "notes.md" in why[0]
    commit(root, "notes.md", "nothing here\n", "pr")
    assert pc.config_dir_paths(root, "parity") == []


def test_an_added_line_naming_the_assistant_is_refused_by_the_runners_own_check(tmp_path):
    root = repo(tmp_path)
    commit(root, "notes.md", "written by " + "Clau" + "de\n", "pr")
    why = pc.parity_squash.refusals("HEAD", "Owner: x\nRows: none", "parity", root, "docs: notes")
    assert any("names the assistant" in line for line in why), why


def test_a_dead_code_error_under_deny_warnings_is_named():
    """A landing was refused for a dead-code error `cargo check` of the PR's crates showed with -D warnings."""
    seen = {}

    def run(command, **kwargs):
        seen["rustflags"] = kwargs["env"]["RUSTFLAGS"]
        seen["command"] = command
        return SimpleNamespace(returncode=101, stdout="", stderr="error: function `unused` is never used\nerror: could not compile `llrm-codegen`\n")

    why = pc.warning_failures(Path("."), ["llrm-codegen"], run, {})
    assert seen["rustflags"] == "-D warnings" and "--tests" in seen["command"] and "llrm-codegen" in seen["command"]
    assert why and "function `unused` is never used" in why[0]
    clean = lambda command, **kwargs: SimpleNamespace(returncode=0, stdout="", stderr="")  # noqa: E731
    assert pc.warning_failures(Path("."), ["llrm-codegen"], clean, {}) == []
    assert pc.warning_failures(Path("."), [], clean, {}) == []


def test_a_lib_test_that_fails_here_and_passes_on_the_tip_is_named_and_one_the_tip_fails_too_is_not():
    out = "failures:\n\nfailures:\n    backend::x::test_new\n    backend::x::test_old\n\ntest result: FAILED. 1 passed; 2 failed\n"

    def run(command, **kwargs):
        return SimpleNamespace(returncode=101, stdout=out, stderr="")

    why = pc.lib_failures(Path("."), ["llrm-codegen"], ["crates/codegen/llrm-codegen/src/x.rs"], "origin/release/next", run, lambda root, failed, base: {"llrm-codegen": {"backend::x::test_old"}})
    assert why == ["lib test fails here and passes on origin/release/next: llrm-codegen::backend::x::test_new"]


def test_a_new_machine_phase_runs_the_slot_list_tests_of_llrm_transforms():
    """#3708 added a phase and not its name to parity_tests' check_backend list: the tests that fail on that live in llrm-transforms."""
    ran = []

    def run(command, **kwargs):
        ran.append(command)
        failing = "parity" in command
        out = "failures:\n\nfailures:\n    parity_tests::test_a_translated_backend_slot_that_is_not_run_fails\n\ntest result: FAILED\n"
        return SimpleNamespace(returncode=101 if failing else 0, stdout=out if failing else "", stderr="")

    why = pc.lib_failures(Path("."), ["llrm-codegen"], ["crates/codegen/llrm-codegen/src/flow.rs"], "b", run, lambda root, failed, base: {})
    assert any("parity" in command for command in ran)
    assert why == ["lib test fails here and passes on b: llrm-transforms::parity_tests::test_a_translated_backend_slot_that_is_not_run_fails"]
    ran.clear()
    assert pc.lib_failures(Path("."), ["llrm-codegen"], ["crates/codegen/llrm-codegen/src/other.rs"], "b", run, lambda root, failed, base: {}) == []
    assert not any("parity" in command for command in ran)


def test_a_clean_branch_passes_and_every_failure_is_listed_once(tmp_path, monkeypatch):
    root = repo(tmp_path)
    commit(root, "c.txt", "y\n", "pr")
    monkeypatch.setattr(pc, "changed_packages", lambda root, files: [])
    assert pc.precheck(root, "docs: c", "Owner: w-8\nRows: none", "parity", subprocess.run) == []
    why = pc.precheck(root, "docs: c", "no lines at all", "parity", subprocess.run)
    assert len(why) == 2 and why[0].startswith("no `Owner:") and why[1].startswith("no `Rows:")


def test_the_precheck_tool_itself_names_neither_the_assistant_nor_its_config_directory():
    """The runner refused the tool for the pattern it looks for: no exemption, the pattern is built from pieces and no added line of the tool matches."""
    pattern = pc.parity_squash.ASSISTANT_NAME
    for name in ("precheck_pr.py", "test_precheck_pr.py"):
        for number, line in enumerate((Path(__file__).resolve().parent / name).read_text().splitlines(), 1):
            assert not pattern.search(line), f"{name}:{number}: {line[:80]}"


def test_a_pass_slot_needs_a_pipeline_level_test(tmp_path):
    """#3732's JumpTableToSwitch passed its unit tests and never fired in a compile: x_funcptr asm was identical before and after."""
    root = repo(tmp_path)
    (root / "crates").mkdir()
    (root / "crates" / "parity.rs").write_text('entry("a@1", Backend, "x", Old, ""),\n')
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "base slots")
    git(root, "branch", "-f", "parity")
    git(root, "checkout", "-q", "-B", "pr", "parity")
    (root / "crates" / "parity.rs").write_text('entry("a@1", Backend, "x", Old, ""),\nentry("jts@2", Backend, "jump-table", Translated, ""),\n')
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "a slot")
    assert len(pc.slot_without_pipeline_test(root, "parity")) == 1
    (root / "crates" / "pipeline_tests.rs").write_text("#[test]\nfn the_pass_fires_in_a_compile() {}\n")
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", "its pipeline test")
    assert pc.slot_without_pipeline_test(root, "parity") == []


def test_a_pr_that_adds_no_slot_needs_no_pipeline_test(tmp_path):
    root = repo(tmp_path)
    commit(root, "notes.md", "nothing\n", "pr")
    assert pc.slot_without_pipeline_test(root, "parity") == []


def test_a_changed_rust_file_that_fmt_would_change_is_named(tmp_path, monkeypatch):
    """The runner formats an unformatted PR by pushing to its branch: a PR formatted before it is marked ready never races with that push."""
    root = repo(tmp_path)
    commit(root, "a.rs", "fn  a( ) {}\n", "pr")
    monkeypatch.setattr(pc.parity_squash, "fmt_differs", lambda root_, files: {f for f in files if f == "a.rs"})
    monkeypatch.setattr(pc, "changed_packages", lambda root_, files: [])
    why = pc.precheck(root, "style: x", "Owner: w-8\nRows: none", "parity", subprocess.run)
    assert any("a.rs: tools/fmt.sh would change it" in line for line in why), why


LEDGER = (
    "d_faces qbq-llrm: 1305.5 -> 1259.9 M Ir (valgrind --tool=callgrind, dist, "
    "llrm-c -m32 -mabi=sysv -march=i486 -O2 -Dfar= ... d_faces.c)"
)


def test_a_perf_pr_states_the_ledger_command_and_a_d_faces_qbq_llrm_number():
    """#3818's first pair was taken on another program: a number for QCport's d_faces next to the board's qbq-llrm one made the totals look wrong by 50 M."""
    head = "TL;DR: x\nOwner: w-6\nRows: fold@677\n"
    why = pc.description_failures("perf(pipeline): fold@677 is combine's alone", head)
    assert len(why) == 1 and "d_faces qbq-llrm" in why[0] and "docs/working/ledger-command.md" in why[0]
    assert pc.description_failures("perf(pipeline): x", head + LEDGER) == []
    other = head + "d_faces QCport: 979.6 M Ir (-m32 -mabi=sysv -march=i486 -O2)\n"
    assert pc.description_failures("perf(pipeline): x", other) != []
    assert pc.description_failures("fix(flow): x", head) == []


def test_a_docs_only_branch_is_told_of_a_scanner_it_turns_red(tmp_path, monkeypatch):
    """#3899 and the docs that name a path of one machine landed with their scanners red: a docs-only PR plans no gate step. The precheck runs the root scanners
    and names one that fails here and passes on the base; one red on both is the base's."""
    monkeypatch.setattr(pc.parity_smoke, "integration_failures", lambda targets, root=None, run=None: {"integration docs_links::test_every_markdown_link_resolves"})
    named = pc.scanner_failures(tmp_path, ["docs/working/x.md"], on_base=lambda targets: set())
    assert named == ["scanner fails here and passes on origin/release/next: integration docs_links::test_every_markdown_link_resolves"]
    assert pc.scanner_failures(tmp_path, ["docs/working/x.md"], on_base=lambda targets: {"integration docs_links::test_every_markdown_link_resolves"}) == []


def test_a_pr_touching_llrm_c_is_checked_and_tested_with_the_toolchain_feature():
    """llrm-c's lib tests are a compile_error without `--features toolchain`: the precheck's cargo check and lib run failed ("did-not-run") for any PR touching llrm-c, for a reason that was not the PR's."""

    def cargo(command, **kwargs):
        has = any("toolchain" in part for part in command)
        refused = SimpleNamespace(returncode=101, stdout="", stderr="error: llrm-c's lib tests need --features toolchain\n")
        return SimpleNamespace(returncode=0, stdout="test result: ok. 1 passed\n", stderr="") if has else refused

    assert pc.warning_failures(Path("."), ["llrm-c"], cargo, {}) == []
    assert pc._failing(Path("."), "llrm-c", None, cargo) == set()


def test_a_branch_that_leaves_a_generated_page_stale_is_told_before_it_is_marked_ready(tmp_path):
    """#5062, #5085 and #5088 changed sources, were marked ready, and failed the batch's docs check; precheck never ran build.py --check."""
    out = pc.docs_failures(tmp_path, docs=lambda root: {"docs/a.md", "docs/b.md"}, docs_on_base=lambda: {"docs/b.md"})
    assert len(out) == 1 and "docs/a.md" in out[0] and "docs/b.md" not in out[0] and "build.py --sync" in out[0]
    assert pc.docs_failures(tmp_path, docs=lambda root: {"docs/b.md"}, docs_on_base=lambda: {"docs/b.md"}) == [], "a page the tip has stale is not the branch's"
    assert pc.docs_failures(tmp_path, docs=lambda root: set()) == []
