"""A DEVIATE or plan row whose `owner NAME` is nobody on the bus is never removed: 181 rows named w-6, funcattrs-port and 40 more sessions that were gone (2026-11-08)."""
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import deviate_owners as owners  # noqa: E402


def dead(line: str) -> list[str]:
    known = owners.live()
    return [name for _, name, _ in owners.mentions(line) if name not in known]


def test_a_row_owned_by_a_worker_id_nobody_runs_is_dead():
    """Fails first: with no check, `owner w-6` on a DEVIATE row went unnoticed for weeks."""
    assert dead("//! DEVIATE X1 (owner w-6, exit 2026-12-01): the pin does it") == ["w-6"]
    assert dead("Row R1, owner funcattrs-port, 2026-11-09: tuned") == ["funcattrs-port"]


def test_a_live_owner_with_or_without_a_trailing_date_is_not_dead():
    assert dead("DEVIATE X1 (owner m16 2026-11-20): x") == []
    assert dead("DEVIATE X1 (owner anyisa, exit 2026-11-20)") == []
    assert dead("owner: w10-instcombine, review 2026-11-14") == []


def test_prose_after_owner_is_not_a_name():
    assert dead("the owner of the pass decides; no owner (start gated on parity)") == []


def test_the_fixture_allow_list_cannot_grow_silently():
    """The files that name a made-up owner on purpose. A new entry is a hole in the check: add it here with a reason."""
    assert len(owners.FIXTURES) == 5
    root = Path(__file__).resolve().parent.parent
    assert all((root / f).is_file() for f in owners.FIXTURES)


def test_no_row_in_the_tree_names_an_owner_that_is_not_in_owners_txt():
    run = subprocess.run([sys.executable, str(Path(owners.__file__))], capture_output=True, text=True)
    assert run.returncode == 0, run.stdout
