"""Runtime data symbols retain their relocation identity, not literal address zero."""

from pathlib import Path

from qbopt import wholeseg
from qbopt.objectfile import omf
from qbopt.analysis import regions
from qbopt.objectfile import module

BENCHMARK = Path("fixtures/bench/nbody-v-g3.obj")


def test_nbody_timer_external_cell_is_not_literal_zero():
    """NBODY refused at 045d because B$SEG was raised as [abs+0] with no emitted fixup field."""
    records = omf.read(BENCHMARK)
    found = module.of(records)
    address = found.resolve(0x45F, 0)
    assert address.space.value == "external"
    assert omf.externals(records)[address.index].upper() == "B$SEG"
    assert address.disp == 0


def test_nbody_timer_can_be_emitted_without_fallback():
    """Timing unchanged fallback NBODY would falsely report that optimization did nothing."""
    result = wholeseg.emitted(BENCHMARK.read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    records = omf.parse(result.data)
    assert any(
        fixup.target == "external"
        and fixup.loc == omf.LOC_OFF16
        and omf.externals(records)[fixup.index].upper() == "B$SEG"
        for fixup in omf.fixups(records)
    )


def test_external_symbols_are_not_proven_disjoint_by_their_indices():
    """Two EXTDEF names may resolve to the same runtime storage at link time."""
    first = module.Addr(module.Space.EXTERNAL, 0, 1)
    second = module.Addr(module.Space.EXTERNAL, 0, 2)
    local = module.Addr(module.Space.SEGMENT, 0, 1)
    assert first != second and first != local
    assert regions.addresses(first, 2, second, 2)
    assert regions.addresses(first, 2, local, 2)
