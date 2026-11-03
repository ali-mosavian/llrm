import unwind_win64_diff as d

OBJDUMP_U = """
w.obj:	file format coff-x86-64
Unwind info:

Function Table:
  Start Address: .text
  End Address: .text + 0x0010
  Unwind Info Address: .xdata
    Version: 1
    Flags: 0
    Size of prolog: 4
    Number of Codes: 1
    No frame pointer used
    Unwind Codes:
      0x04: UOP_AllocSmall 40

Function Table:
  Start Address: .text + 0x0010
  End Address: .text + 0x0030
  Unwind Info Address: .xdata + 0x0008
    Version: 1
    Flags: 0
    Size of prolog: 5
    Number of Codes: 2
    No frame pointer used
    Unwind Codes:
      0x05: UOP_AllocSmall 32
      0x01: UOP_PushNonVol RBP
"""


def test_the_unwind_tables_of_an_object_are_read_per_function():
    tables = d.tables_of(OBJDUMP_U)
    assert sorted(tables) == [0, 0x10]
    assert tables[0] == (0x10, 4, "No frame pointer used", ("UOP_AllocSmall 40",))
    assert tables[0x10][3] == ("UOP_AllocSmall 32", "UOP_PushNonVol RBP")


def test_an_object_with_no_unwind_data_has_no_tables():
    """The stripped control: every function of the pin's is then a difference."""
    assert d.tables_of("w.obj:\tfile format coff-x86-64\nUnwind info:\n") == {}
