from pathlib import Path
from dataclasses import replace

import pytest

from tests import corpus
from qbopt import wholeseg
from qbopt.objectfile import omf
from qbopt.frontend import blocks
from qbopt.frontend import extent
from qbopt.frontend import fppatches
from qbopt.objectfile import module as modules


def test_borland_private_procedures_are_reached() -> None:
    # qb-qrender's R_WALK_TEXT was refused before any C instruction was mapped.
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    assert module.name == "R_WALK_TEXT"
    mapped = blocks.walk(module, min(module.publics))
    assert not isinstance(mapped, str), mapped
    assert 0 in mapped.starts
    assert 0x604 in mapped.starts
    assert 0x6CB in mapped.starts
    assert all(blocks.benign(module, gap) is not None for gap in mapped.unreached)


def test_native_unknown_cleanup_is_refused_atomically() -> None:
    original = Path("fixtures/regressions/r_walk-borland.obj").read_bytes()
    result = wholeseg.emitted(original, native_fpu=True)
    assert result.data == original
    assert str(result.outcome) == "refused"
    assert "call cleanup unproved" in result.reason


def test_text_segment_is_admitted_only_for_c_and_assembly() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None

    def named(header: bytes):
        return [replace(record, body=header) if record.type == omf.THEADR else record for record in module.records]

    assert omf.code_segment(named(b"\x0ar_walk.asm")) is not None
    assert omf.code_segment(named(b"\x0ar_walk.pas")) is None


def test_multiple_c_text_segments_are_not_selected_arbitrarily() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    code = next(record for record in module.records if record.type == omf.SEGDEF)
    assert omf.code_segment([*module.records, code]) is None


def test_borland_float_patch_sites_explain_nonoperand_fixups() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.walk(module, min(module.publics))
    assert not isinstance(mapped, str)
    fields = blocks.operand_fields(module, mapped, [])
    assert fields is not None
    patches = fppatches.sites(module, mapped.starts)
    assert {8, 0x4D, 0x5B} <= patches
    assert module.sites - fields == patches


def test_float_patch_name_does_not_override_wrong_bytes() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.walk(module, min(module.publics))
    assert not isinstance(mapped, str)
    changed = replace(module, code=module.code[:8] + b"\x90" + module.code[9:])
    assert 8 not in fppatches.sites(changed, mapped.starts)


def test_borland_code_map_accounts_for_float_protocol() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str), mapped
    assert {0, 0x604, 0x6CB} <= mapped.starts
    assert not mapped.tables


@pytest.mark.parametrize("changes", [{"disp": 1}, {"selfrel": True}, {"loc": omf.LOC_PTR32}])
def test_unverified_float_fixup_forms_remain_unexplained(changes: dict[str, int | bool]) -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.walk(module, min(module.publics))
    assert not isinstance(mapped, str)
    fixups = dict(module.fixup_at)
    fixups[8] = replace(fixups[8], **changes)
    changed = replace(module, fixup_at=fixups)
    assert 8 not in fppatches.sites(changed, mapped.starts)


def test_c_procedure_partition_has_no_invented_main() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    found = extent.partition(module)
    assert not isinstance(found, str), found
    assert {body.seed for body in found.bodies} == {0, 0x2F6, 0x334, 0x604, 0x6CB}
    assert all(body.kind == extent.BodyKind.PROCEDURE for body in found.bodies)
    assert not found.unexplained
    assert not found.conflicts


def test_native_conversion_removes_only_verified_fp_patch_records() -> None:
    module = corpus.loaded(Path("fixtures/regressions/r_walk-borland.obj"))
    assert module is not None
    mapped = blocks.code_map(module)
    assert not isinstance(mapped, str)
    patches = fppatches.sites(module, mapped.starts)
    records = fppatches.native_records(module, mapped.starts)
    data = b"".join(record.emit() for record in records)
    converted = modules.of(omf.parse(data))
    assert converted is not None
    assert converted.code == module.code
    assert converted.sites == module.sites - patches
    expected = [
        fixup for fixup in omf.fixups(module.records) if not (fixup.seg == module.seg and fixup.offset in patches)
    ]
    actual = omf.fixups(converted.records)

    def signature(fixup: omf.Fixup) -> tuple:
        return (fixup.seg, fixup.offset, fixup.loc, fixup.target, fixup.index, fixup.disp, fixup.selfrel, fixup.frame)

    assert list(map(signature, actual)) == list(map(signature, expected))
    assert fppatches.native_records(converted, mapped.starts) == converted.records
