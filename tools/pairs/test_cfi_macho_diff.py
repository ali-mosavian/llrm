import cfi_macho_diff as c

DUMP = """m.pin.o:\tfile format mach-o 64-bit x86-64

.eh_frame contents:

00000000 00000014 00000000 CIE
  Version:               1
  DW_CFA_def_cfa: reg7 +8
  DW_CFA_offset: reg16 -8

00000018 00000024 0000001c FDE cie=00000000 pc=00000000...0000000d
  Format:       DWARF32
  DW_CFA_advance_loc: 1 to 0x1
  DW_CFA_def_cfa_offset: +16
  DW_CFA_offset: reg6 -16
  DW_CFA_advance_loc: 3 to 0x4

00000040 00000014 00000044 FDE cie=00000000 pc=00000010...00000020
  DW_CFA_nop:
"""


def test_padding_and_absolute_addresses_are_not_rows():
    """All 130 functions read as different, 0 same, as the stripped control did: the padding after the rows and the address each advance
    reaches differ in any two objects."""
    assert c.frames_of(DUMP)[0][-1] == "DW_CFA_advance_loc: 3"
    assert c.frames_of(DUMP)[1] == []


def test_each_fde_gives_its_instructions_and_the_cie_none():
    """The CIE's initial rows are not a function's; they were counted into the first FDE."""
    assert c.frames_of(DUMP) == [
        ["DW_CFA_advance_loc: 1", "DW_CFA_def_cfa_offset: +16", "DW_CFA_offset: reg6 -16", "DW_CFA_advance_loc: 3"],
        [],
    ]


def test_no_eh_frame_gives_no_functions():
    assert c.frames_of(".eh_frame contents:\n") == []
