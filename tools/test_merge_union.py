"""Two PRs that append a test to the same `*_tests.rs` once merged with a conflict; `merge=union` merged them and dropped a closing `}` (#4570: llrm-codegen did not
compile, no conflict reported). The repo's `.gitattributes` names `tests-append`, whose driver keeps each appended block whole."""

import os

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_squash as ps  # noqa: E402

TESTS = "crates/x/src/area_tests.rs"
TEST = "#[test]\nfn {name}() {{\n    let module = parse({name});\n    let done = selected(&module);\n    assert!(done.volatile, \"volatile\");\n}}\n"
BASE = TEST.format(name="volatile_load")


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess:
    env = dict(os.environ)
    env.pop("GIT_CONFIG_COUNT", None)
    ps.with_merge_driver(env)
    env["GIT_CONFIG_VALUE_0"] = f"python3 {ROOT}/tools/merge_tests_append.py %O %A %B"
    done = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=repo, capture_output=True, text=True, env=env)
    assert not check or done.returncode == 0, done.stderr
    return done


def appended(repo: Path, name: str) -> None:
    path = repo / TESTS
    path.write_text(path.read_text() + f"\nconst {name.upper()}: &str = \"{name}\";\n\n" + TEST.format(name=name))
    git(repo, "commit", "-qam", name)


def appended_module(repo: Path, name: str) -> None:
    """A constant holding a multi-line module, then the test that parses it: the shape of #4565, where lines the two sides share were kept once and the blocks interleaved."""
    path = repo / TESTS
    module = f'const {name.upper()}: &str = "target datalayout = \\"e-p:16:16\\"\n@slot = global i16 0\ndeclare void @u(i16)\ndefine void @f() {{\nentry:\n  ret void\n}}\n";\n\n'
    path.write_text(path.read_text() + "\n" + module + TEST.format(name=name).replace("parse(" + name + ")", f"parse({name.upper()})"))
    git(repo, "commit", "-qam", name)


def two_prs(tmp_path: Path, attribute: str, append=appended) -> Path:
    repo = tmp_path / "repo"
    (repo / TESTS).parent.mkdir(parents=True)
    git(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / TESTS).write_text(BASE)
    (repo / ".gitattributes").write_text(f"*_tests.rs merge={attribute}\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "base")
    git(repo, "checkout", "-qb", "second")
    append(repo, "volatile_store")
    git(repo, "checkout", "-q", "main")
    append(repo, "volatile_copy")
    return repo


def parses(text: str, tmp_path: Path) -> bool:
    (tmp_path / "x.rs").write_text(text)
    return subprocess.run(["rustc", "-Zunpretty=normal", "--edition", "2024", str(tmp_path / "x.rs")], capture_output=True, env={**os.environ, "RUSTC_BOOTSTRAP": "1"}).returncode == 0


def test_two_tests_appended_after_the_same_test_merge_whole(tmp_path):
    """With `merge=union` the first block lost its tail (`let done`, `assert!`, `}`) to the second: 'unclosed delimiter'."""
    repo = two_prs(tmp_path, "tests-append")
    merged = git(repo, "merge", "--no-edit", "-q", "second", check=False)
    text = (repo / TESTS).read_text()
    assert merged.returncode == 0, merged.stdout + merged.stderr
    assert "<<<<<<<" not in text
    assert text.count("assert!(done.volatile") == 3 and text.count("\n}\n") == 3
    assert text.index("fn volatile_copy") < text.index("fn volatile_store")
    assert parses(text, tmp_path)


def test_union_is_what_dropped_the_brace(tmp_path):
    """The defect, kept as a fixture: the same two appends under `merge=union` merge without a conflict into text that does not parse."""
    repo = two_prs(tmp_path, "union")
    git(repo, "merge", "--no-edit", "-q", "second", check=False)
    assert not parses((repo / TESTS).read_text(), tmp_path)


def test_two_edits_of_one_line_still_conflict(tmp_path):
    repo = two_prs(tmp_path, "tests-append")
    git(repo, "reset", "-q", "--hard", "HEAD~1")
    git(repo, "checkout", "-q", "second")
    git(repo, "reset", "-q", "--hard", "HEAD~1")
    for branch, word in (("second", "one"), ("main", "two")):
        git(repo, "checkout", "-q", branch)
        path = repo / TESTS
        path.write_text(path.read_text().replace('"volatile"', f'"{word}"'))
        git(repo, "commit", "-qam", word)
    assert git(repo, "merge", "--no-edit", "-q", "second", check=False).returncode != 0


def test_gitattributes_names_the_driver_for_tests_files():
    lines = (ROOT / ".gitattributes").read_text().splitlines()
    assert "*_tests.rs merge=tests-append" in lines and "tests.rs merge=tests-append" in lines
    assert not any("tests.rs" in line and line.endswith("merge=union") for line in lines)


def test_without_the_driver_the_appends_conflict_and_never_corrupt(tmp_path):
    """A clone that never ran merge-driver.sh: git must stop at a conflict, not merge to text that does not parse."""
    repo = two_prs(tmp_path, "tests-append")
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_CONFIG_")}
    merged = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "merge", "--no-edit", "-q", "second"], cwd=repo, capture_output=True, text=True, env=env)
    assert merged.returncode != 0 and "<<<<<<<" in (repo / TESTS).read_text()


def test_a_constant_and_its_test_appended_by_both_sides_merge_whole(tmp_path):
    """#4565: the PR's constant interleaved with parity's under union (unclosed delimiter); each side's constant and test come through whole."""
    repo = two_prs(tmp_path, "tests-append", appended_module)
    assert git(repo, "merge", "--no-edit", "-q", "second", check=False).returncode == 0
    text = (repo / TESTS).read_text()
    assert parses(text, tmp_path)
    assert text.count("const VOLATILE_COPY") == 1 and text.count("const VOLATILE_STORE") == 1 and text.count("declare void @u(i16)") == 2


def test_union_interleaves_a_constant_and_its_test(tmp_path):
    repo = two_prs(tmp_path, "union", appended_module)
    git(repo, "merge", "--no-edit", "-q", "second", check=False)
    assert not parses((repo / TESTS).read_text(), tmp_path)


def test_a_branch_whose_tree_still_says_union_merges_whole_with_the_clone_set_up(tmp_path):
    """#4567 and #4573 merged parity after #4579 landed and still got `union`: the attributes come from the branch merged into, which had not merged #4579 yet."""
    repo = two_prs(tmp_path, "union")
    git(repo, "checkout", "-q", "main")
    git(repo, "reset", "-q", "--hard", "HEAD~1")
    script = ROOT / "tools/landing/merge-driver.sh"
    env = {k: v for k, v in os.environ.items() if not k.startswith("GIT_CONFIG_")}
    subprocess.run(["bash", str(script)], cwd=repo, check=True, env=env)
    git(repo, "config", "--local", "merge.tests-append.driver", f"python3 {ROOT}/tools/merge_tests_append.py %O %A %B")
    appended(repo, "volatile_copy")
    merged = subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", "merge", "--no-edit", "-q", "second"], cwd=repo, capture_output=True, text=True, env=env)
    assert merged.returncode == 0, merged.stdout + merged.stderr
    assert parses((repo / TESTS).read_text(), tmp_path)


PARSER = "crates/frontends/qb/tests/parser.rs"


def parser_prs(tmp_path: Path, attributes: str) -> Path:
    """The shape of the 12 QB PRs: one side also edits a line high in the file, both append a test at the end."""
    repo = tmp_path / "repo"
    (repo / PARSER).parent.mkdir(parents=True)
    git(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / PARSER).write_text("// header\n\n" + BASE)
    (repo / ".gitattributes").write_text(attributes)
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "base")
    git(repo, "checkout", "-qb", "second")
    path = repo / PARSER
    path.write_text(path.read_text() + "\n" + TEST.format(name="from_second"))
    git(repo, "commit", "-qam", "second")
    git(repo, "checkout", "-q", "main")
    path.write_text(path.read_text().replace("// header", "// header, edited") + "\n" + TEST.format(name="from_main"))
    git(repo, "commit", "-qam", "main")
    return repo


def test_the_repo_attributes_merge_two_qb_pr_appends_to_parser_rs_clean(tmp_path):
    """12 QB PRs conflicted at the end of parser.rs because only `*_tests.rs` and `tests.rs` had the driver."""
    repo = parser_prs(tmp_path, (ROOT / ".gitattributes").read_text())
    assert git(repo, "merge", "--no-edit", "-q", "second", check=False).returncode == 0
    text = (repo / PARSER).read_text()
    assert "// header, edited" in text and "fn from_main" in text and "fn from_second" in text and parses(text, tmp_path)


def test_without_the_parser_rs_line_the_same_appends_conflict(tmp_path):
    lines = [line for line in (ROOT / ".gitattributes").read_text().splitlines() if "parser.rs" not in line]
    repo = parser_prs(tmp_path, "\n".join(lines) + "\n")
    assert git(repo, "merge", "--no-edit", "-q", "second", check=False).returncode != 0


def test_two_prs_adding_the_same_test_do_not_define_it_twice(tmp_path):
    """4678 and 4680 both added `a_frame_cell_with_a_wide_index_is_not_encodable_before_the_frame_is_named` to select64_tests.rs; joined, the batch failed 'defined multiple times'."""
    repo = tmp_path / "repo"
    (repo / TESTS).parent.mkdir(parents=True)
    git(tmp_path, "init", "-q", "-b", "main", str(repo))
    (repo / TESTS).write_text(BASE)
    (repo / ".gitattributes").write_text("*_tests.rs merge=tests-append\n")
    git(repo, "add", "-A")
    git(repo, "commit", "-qm", "base")
    git(repo, "checkout", "-qb", "second")
    for name, extra in (("shared", "    // second\n"), ):
        path = repo / TESTS
        path.write_text(path.read_text() + "\n" + TEST.format(name=name).replace("{\n", "{\n" + extra, 1))
        git(repo, "commit", "-qam", "second")
    git(repo, "checkout", "-q", "main")
    path = repo / TESTS
    path.write_text(path.read_text() + "\n" + TEST.format(name="shared").replace("{\n", "{\n    // main\n", 1) + "\n" + TEST.format(name="only_main"))
    git(repo, "commit", "-qam", "main")
    merged = git(repo, "merge", "--no-edit", "-q", "second", check=False)
    text = (repo / TESTS).read_text()
    assert merged.returncode != 0 or text.count("fn shared(") == 1
