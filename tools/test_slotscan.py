"""slotscan's detector: stores that reach the saved BP."""
import slotscan

PROC = "_f@3 proc far\n{body}\n_f@3 endp\n"


def test_a_dword_spill_into_a_word_slot_next_to_bp_is_a_hit():
    """B$FLOF and tone.c under the old and the dag selectors stored `mov dword ptr [bp-2], eax` into a `sub sp, 2` frame: the high half landed on the saved BP and the routine returned to 0000:0000."""
    assert slotscan.hits(PROC.format(body="    sub sp, 2\n    mov dword ptr [bp-2], eax")) == ["_f@3  mov dword ptr [bp-2], eax"]


def test_a_store_that_fits_and_a_read_are_not_hits():
    body = "    mov word ptr [bp-2], ax\n    mov dword ptr [bp-4], eax\n    cmp dword ptr [bp-2], 0\n    fld qword ptr [bp-4]\n    push dword ptr [bp-2]"
    assert slotscan.hits(PROC.format(body=body)) == []
