"""
BC compiles, qbopt rewrites, LINK accepts, the program runs, and the answers
match -- on every configuration.

This is the only tier that proves the pass against the thing it is a pass for.
It needs dosbox-x and the three DOS toolchains; without them it skips.
"""

from pathlib import Path

import e2e
import pytest
from configs import CONFIGS
from dosbox import dosbox_bin

from qbopt.extent import BodyKind
from qbopt.bodyedit import rewritten

pytestmark = [pytest.mark.e2e, pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x")]

CASES = [
    pytest.param(
        tag,
        marks=pytest.mark.skipif(not cfg.available, reason=f"no toolchain at {cfg.mount}"),
    )
    for tag, cfg in CONFIGS.items()
]


@pytest.mark.parametrize("tag", CASES)
def test_the_rewritten_program_answers_the_same(tag: str) -> None:
    result = e2e.run(tag)
    failed = [v for v in result.verdicts if not v.ok]
    assert not failed, "; ".join(f"{v.program} {v.status}: {v.detail}" for v in failed)


# jumps.bas owns an ON GOTO inline table (B$OGTA); divmod.bas always compiles
# under /X (configs.EXTRA) and so owns a RESUME map. Both are load-bearing:
# AGENTS.md documents both as places a length change has real risk.
BODY_EDIT_PROGRAMS = ["jumps", "divmod"]


def one_nop(data: bytes) -> bytes:
    return rewritten(data, BodyKind.MAIN)


@pytest.mark.parametrize("tag", CASES)
def test_a_whole_body_edit_survives_link_and_run(tag: str) -> None:
    # A same-meaning, one-byte-longer main body -- not a region inside a
    # block, the whole Body extent.py/ir.py define -- still links and runs
    # identically. This is commit 2's own gate: not "an object was
    # produced", the full three-way differential judge() already applies to
    # every ordinary run.
    work = Path(__file__).resolve().parents[1] / "build" / "e2e" / f"{tag}-bodyedit"
    result = e2e.run(tag, transform=one_nop, names=BODY_EDIT_PROGRAMS, work=work)
    failed = [v for v in result.verdicts if not v.ok]
    assert not failed, "; ".join(f"{v.program} {v.status}: {v.detail}" for v in failed)


@pytest.mark.parametrize(
    "source", sorted(Path(__file__).resolve().parents[1].glob("suite/*.bas")), ids=lambda p: p.name
)
def test_every_suite_program_has_dos_line_endings(source: Path) -> None:
    # BC reads a lone LF as part of the line and reports a syntax error on the
    # statement after it, which looks nothing like a line-ending problem.
    text = source.read_bytes()
    assert b"\n" in text
    assert text.count(b"\r\n") == text.count(b"\n")
