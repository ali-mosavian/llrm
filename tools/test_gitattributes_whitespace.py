import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def diff_check(tmp_path: Path, name: str, content: bytes = b"COMMON a AS INTEGER\r\nPRINT a\r\n") -> str:
    def git(*args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True)

    git("init", "-q", "-b", "main")
    shutil.copy(REPO / ".gitattributes", tmp_path / ".gitattributes")
    git("add", ".")
    git("commit", "-qm", "base")
    (tmp_path / name).parent.mkdir(parents=True, exist_ok=True)
    (tmp_path / name).write_bytes(content)
    git("add", ".")
    return git("diff", "--cached", "--check").stdout


def test_a_crlf_bas_test_source_passes_the_diff_check(tmp_path):
    """sqb-5300 was refused by `git diff --check` flagging every CR of #5440's tests/chain/qb/chfirst.bas as trailing whitespace."""
    assert diff_check(tmp_path, "tests/chain/qb/chfirst.bas") == ""


def test_the_diff_check_still_flags_a_cr_elsewhere(tmp_path):
    assert "trailing whitespace" in diff_check(tmp_path, "crates/x/src/a.rs")


def test_an_expected_output_file_may_end_its_lines_with_a_space(tmp_path):
    """#5440's tests/run/qb/dateserial_vbdos.out holds the trailing space printf left; the diff check refused sqb-5413 on it."""
    assert diff_check(tmp_path, "tests/run/qb/dateserial_vbdos.out", b"a \nb \n") == ""


def test_the_diff_check_still_flags_a_trailing_space_in_a_source(tmp_path):
    assert "trailing whitespace" in diff_check(tmp_path, "crates/x/src/a.rs", b"fn a() {} \n")
