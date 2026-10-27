"""A harness that gives no clocks stops ambt_pin with the harness's own message, not a null result."""
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ambt_pin  # noqa: E402


def finished(code: int, out: str, err: str) -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, out, err)


def test_stale_binaries_stop_the_run_with_the_guards_message():
    """The harness's stale-binary guard exits with a message on stderr and nothing on stdout; a swallowed one read as null clocks for 22 rows."""
    guard = "stale binaries in target/release: llrm-llc older than the source"
    with pytest.raises(SystemExit) as stopped:
        ambt_pin.answer("x_funcptr", "llrm", finished(1, "", guard))
    assert guard in str(stopped.value)


def test_an_empty_answer_is_an_error_even_on_exit_zero():
    with pytest.raises(SystemExit):
        ambt_pin.answer("x_funcptr", "llrm", finished(0, "", ""))


def test_the_last_line_is_the_answer():
    assert ambt_pin.answer("p", "v", finished(0, "log\n{\"clocks\": 7}\n", "")) == {"clocks": 7}
