"""Regression tests of the program list: a program is skipped for a recorded reason, checked, not for its name."""
import programs


def test_every_skipped_program_still_has_the_reason_it_was_skipped_for():
    """The skip was a set of names (`readme.md`, `parity` among them, which are no benchmark): a program that began to build at m32, or was removed, stayed out of the table unnoticed."""
    assert programs.stale_skips() == []


def test_a_skipped_program_that_builds_is_reported(monkeypatch):
    monkeypatch.setitem(programs.SKIPPED, "fib", ("claimed 16-bit only", "gcc_rejects"))
    assert programs.stale_skips() == ["fib: gcc -m32 builds it now (claimed 16-bit only)"]


def test_the_list_leaves_out_exactly_the_skipped_programs():
    names = set(programs.sources())
    assert not names & set(programs.SKIPPED)
