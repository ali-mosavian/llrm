import shutil
import subprocess
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]


def diff_check(tmp_path: Path, name: str) -> str:
    def git(*args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["git", "-c", "user.name=t", "-c", "user.email=t@t", *args], cwd=tmp_path, capture_output=True, text=True)

    git("init", "-q", "-b", "main")
    shutil.copy(REPO / ".gitattributes", tmp_path / ".gitattributes")
    git("add", ".")
    git("commit", "-qm", "base")
    (tmp_path / name).parent.mkdir(parents=True, exist_ok=True)
    (tmp_path / name).write_bytes(b"COMMON a AS INTEGER\r\nPRINT a\r\n")
    git("add", ".")
    return git("diff", "--cached", "--check").stdout


def test_a_crlf_bas_test_source_passes_the_diff_check(tmp_path):
    """sqb-5300 was refused by `git diff --check` flagging every CR of #5440's tests/chain/qb/chfirst.bas as trailing whitespace."""
    assert diff_check(tmp_path, "tests/chain/qb/chfirst.bas") == ""


def test_the_diff_check_still_flags_a_cr_elsewhere(tmp_path):
    assert "trailing whitespace" in diff_check(tmp_path, "crates/x/src/a.rs")
