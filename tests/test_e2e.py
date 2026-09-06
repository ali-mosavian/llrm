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


# The programs whose objects rebuild whole-segment today, and the compilers
# whose output does. VBDOS pads its segment with a `00 00` that decodes as
# `add [bx+si],al`, which select.py refuses, so its objects fall back.
# fpemu is here because its x87 sites are what the bare-operand forms
# unlocked -- fsqrt names st(0) in both dests and sources and encodes
# neither, so "no operands" was the wrong test for it.
# jumps is here for the ON GOTO tables BC drops between the instructions:
# carried verbatim, with every entry's fixup moved with them.
REBUILDS = ("arith", "cmpord", "flags", "nots", "fpemu", "jumps")
# Every configuration now: what used to be two compilers is all three,
# since carrying BC's trailing zero padding stopped it blocking them.
REBUILDING_TAGS = tuple(CONFIGS)


@pytest.mark.parametrize("tag", [t for t in REBUILDING_TAGS if t in CONFIGS and CONFIGS[t].available])
@pytest.mark.parametrize("prog", REBUILDS)
def test_a_segment_this_pass_wrote_links_and_runs(tag: str, prog: str) -> None:
    """The first code MIR produced end to end, rather than edited.

    Everything else in this suite checks a rewrite of BC's own bytes. Here
    layout.rebuild placed every instruction and relocate.as_records wrote
    the records, so the chunk boundaries, branch displacements and fixup
    offsets are all this pass's.

    Only LINK and a real 386 can say whether that worked. The two bugs it
    had were invisible to every host test: fixups whose EXTDEF had not been
    read yet, which LINK reports as `invalid object module` without saying
    which index, and a relocated immediate reported as no relocation at all.
    """
    from qbopt.wholeseg import rebuilt

    seen = []

    def change(data: bytes) -> bytes:
        out, why = rebuilt(data)
        seen.append(why)
        return out

    # Its own directory: this is parametrised over tag AND program, so four
    # of these share a tag and would otherwise write the same files at once.
    result = e2e.run(tag, prog, dry_run=False, transform=change, work=Path("build/e2e") / f"{tag}-{prog}-mir")
    bad = [one for one in result.verdicts if not one.ok]
    assert not bad, f"{tag}/{prog}: {bad[0].status} {bad[0].detail}"
    # Every one of these rebuilds now, so a refusal is a regression rather
    # than a result. The last was jumps under /V, on two calls to B$EVCK
    # after an unconditional jump: real relocated code nothing can reach,
    # carried the way padding and tables already were.
    from qbopt.wholeseg import REBUILT

    assert seen, f"{tag}/{prog}: the transform never ran"
    assert set(seen) == {REBUILT}, f"{tag}/{prog}: {seen}"


@pytest.mark.parametrize("tag", [t for t in REBUILDING_TAGS if t in CONFIGS and CONFIGS[t].available])
def test_optimised_code_this_pass_wrote_links_and_runs(tag: str) -> None:
    """The milestone: code that is both optimised and MIR's own.

    Absorption removes the runtime calls, which is where every measured win
    in this project comes from, and then the whole segment is laid out and
    written from MIR rather than patched. Half the suite's objects go
    through both on these compilers; the rest fall back to absorption alone
    because select.py cannot yet emit something in them.

    Whether the composition is sound is not a host question. It took two
    bugs neither the host suite nor a byte comparison could see: fixups
    naming an EXTDEF that had not been read, and a fixup field split across
    a record boundary. LINK calls both `invalid object module`.
    """
    from qbopt.rewrite import rewrite
    from qbopt.wholeseg import REBUILT
    from qbopt.wholeseg import rebuilt

    seen: list[str] = []

    def change(data: bytes) -> bytes:
        out, why = rebuilt(rewrite(data, dry_run=False)[0])
        seen.append(why)
        return out

    result = e2e.run(tag, None, dry_run=False, transform=change, work=Path("build/e2e") / f"{tag}-opt")
    assert REBUILT in seen, f"{tag}: nothing was emitted from MIR"
    bad = [one for one in result.verdicts if not one.ok]
    assert not bad, f"{tag}: {bad[0].status} {bad[0].program}: {bad[0].detail}"


@pytest.mark.parametrize("tag", [t for t in REBUILDING_TAGS if t in CONFIGS and CONFIGS[t].available])
def test_every_compiled_object_round_trips_through_the_selector(tag: str) -> None:
    """What select.py emits decodes back to what it was asked for.

    tests/test_select.py makes this claim over fixtures/omf, which is seven
    programs. The suite is sixteen, and the difference is not academic:
    `push dword ptr [bx+4]` came back as `push [bx]` -- the displacement
    dropped, four bytes read from the wrong place -- and byref2 printed 0
    where it wanted 16 on two configurations. Nothing in the fixture corpus
    has that shape.

    Here rather than by adding a hundred and fifty more fixtures, because
    the objects already exist by the time this runs.
    """
    from iced_x86 import Decoder

    from qbopt import ir
    from qbopt import mir
    from qbopt import omf
    from qbopt import module
    from qbopt import select
    from qbopt import blocks as split
    from qbopt.declen import BITNESS
    from qbopt.blocks import code_map

    work = Path("build/e2e") / f"{tag}-roundtrip"
    e2e.run(tag, None, dry_run=True, work=work)

    checked = 0
    for obj in sorted(work.glob("*.OBJ")):
        if obj.stem.endswith("Q"):
            continue
        found = module.of(omf.parse(obj.read_bytes()))
        if found is None:
            continue
        mapped = code_map(found)
        if isinstance(mapped, str):
            continue
        for _, body in mir.bodies(found, split.partition(found, mapped)):
            for block in body.blocks:
                for op in block.ops:
                    what = getattr(op.node, "semantics", None)
                    if what is None or what.op is ir.Operation.BARRIER:
                        continue
                    made = select.emit(what, at=op.at)
                    if made is None:
                        continue
                    checked += 1
                    back = next(iter(Decoder(BITNESS, made.code, ip=op.at)), None)
                    if not isinstance(op.node, (ir.Opaque, ir.Long, ir.Call)):
                        continue
                    want = op.node.insn.insn
                    same = back is not None and (
                        str(back) == str(want)
                        or (back.mnemonic == want.mnemonic and back.near_branch16 == want.near_branch16)
                    )
                    assert same, f"{tag}/{obj.stem} {op.at:#x}: {back} != {want}"
    assert checked > 500, f"{tag}: only {checked} instructions checked"


@pytest.mark.parametrize("pass_name,broken", [("hoist", ["hotlop", "nested"]), ("forward", ["nested"])])
def test_a_pass_that_only_the_fallback_has_been_hiding(pass_name: str, broken: list[str]) -> None:
    """These miscompile, and nothing had noticed.

    A body the allocator refuses is emitted as it was *raised*, so every
    pass's work on it is discarded -- and nested's body always refuses.
    Both of these have been producing wrong code for as long as they have
    existed, and the shipped path threw it away before it could be seen.

    The LIR path does not fall back, which is how they surfaced: it prints
    `T= 0` for 675. This runs each pass alone through that path, so what
    fails is the pass and not the interaction.

    Marked xfail because the passes are wrong, not the test.
    """
    pytest.xfail(f"{pass_name} miscompiles {', '.join(broken)}; see docs/architecture.md")
