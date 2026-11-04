import unwind_macho_diff as d

OBJDUMP = """
m.o:	file format mach-o arm64
Contents of __compact_unwind section:
  Entry at offset 0x0:
    start:                0x0 _caller
    length:               0x18
    compact encoding:     0x04000000
  Entry at offset 0x20:
    start:                0x18 _leaf
    length:               0x8
    compact encoding:     0x02000000
"""


def test_the_encodings_are_read_per_function_address():
    """The pin's object names the first function `ltmp0` and ours `_caller`: keyed by name nothing matched, every function was different."""
    assert d.encodings_of(OBJDUMP) == [("0x0", "0x04000000"), ("0x18", "0x02000000")]


def test_an_object_without_a_compact_unwind_section_has_no_encodings():
    """The stripped control: every function of the pin's is then a difference."""
    assert d.encodings_of("m.o:\tfile format mach-o arm64\n") == []


def test_two_dwarf_mode_words_are_the_same_whatever_their_fde_offsets():
    """`0x04000003` and `0x03000018` look alike to a string compare: the second is DWARF mode at FDE offset 0x18, not the first."""
    assert d.same_word("0x03000018", "0x03000040")
    assert not d.same_word("0x03000018", "0x04000003")
    assert not d.same_word("0x04000003", "0x04000000")
