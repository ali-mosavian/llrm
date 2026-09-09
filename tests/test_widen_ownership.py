"""Widening preserves byte ownership transferred by earlier MIR passes."""

from pathlib import Path

from qbopt import wholeseg


def test_addrm_vbdos_emits_after_reclaiming_its_index_reload() -> None:
    """ADDRM /G3 refused three bytes at 0x80 after widening discarded CSE's enlarged ownership span."""
    data = Path("fixtures/omf/addrm-v-g3.obj").read_bytes()
    result = wholeseg.emitted(data)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    assert result.data != data
