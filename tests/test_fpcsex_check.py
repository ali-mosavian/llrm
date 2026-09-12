import struct

import pytest

from tools.references import fpcsex_check


def test_matching_results_do_not_qualify_a_broken_floating_environment() -> None:
    # DOSBox matched all 144 pairs but ignored directed SINGLE rounding and ZE.
    record = struct.pack("<fffHH", 48, 0.75, 487.5, 11, 0)
    data = (record * 2) * (len(fpcsex_check.CONTROLS) * len(fpcsex_check.CASES))
    with pytest.raises(ValueError, match="floating environment"):
        fpcsex_check.checked(data)


def qualified_results() -> bytearray:
    record = struct.pack("<IIIHH", 0x42400000, 0x3F400000, 0x43F3C000, 11, 0)
    data = bytearray((record * 2) * (len(fpcsex_check.CONTROLS) * len(fpcsex_check.CASES)))
    for control, case, field, value in ((0x67F, 1, 0, 0x3F666666), (0xA7F, 1, 0, 0x3F666667), (0x37F, 7, 14, 4)):
        offset = (fpcsex_check.CONTROLS.index(control) * len(fpcsex_check.CASES) + case) * 32
        for side in (0, 16):
            struct.pack_into("<H" if field == 14 else "<I", data, offset + side + field, value)
    return data


def test_comparator_checks_both_kernels_after_environment_qualification() -> None:
    data = qualified_results()
    assert fpcsex_check.checked(bytes(data)) == 144
    data[16] ^= 1
    with pytest.raises(ValueError, match="control="):
        fpcsex_check.checked(bytes(data))


def test_divide_by_zero_status_is_required_independently_of_rounding() -> None:
    data = qualified_results()
    offset = (fpcsex_check.CONTROLS.index(0x37F) * len(fpcsex_check.CASES) + 7) * 32
    data[offset + 14] = data[offset + 30] = 0
    with pytest.raises(ValueError, match="divide-by-zero"):
        fpcsex_check.checked(bytes(data))


def test_incomplete_output_cannot_qualify() -> None:
    with pytest.raises(ValueError, match="result bytes"):
        fpcsex_check.checked(b"")


@pytest.mark.parametrize("sectors", [0, 18])
def test_boot_transport_refuses_payloads_outside_its_track(sectors: int) -> None:
    with pytest.raises(ValueError, match="first floppy track"):
        fpcsex_check.bootloader(sectors)


def test_unmasked_runs_must_actually_observe_each_exception_class() -> None:
    data = qualified_results()
    record = struct.pack("<IIIHH", 0x42400000, 0x3F400000, 0x43F3C000, 11, 0)
    data.extend(record * 2 * len(fpcsex_check.TRAP_CONTROLS) * len(fpcsex_check.CASES))
    with pytest.raises(ValueError, match="no observed unmasked trap"):
        fpcsex_check.checked(bytes(data), traps=True)
