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


def test_an_object_of_ours_with_no_unwind_data_is_skipped_not_different_or_ok():
    """Before the Win64 rows landed every function read as different (a red that blamed the harness), and a stripped object could read as ok."""
    assert d.verdict(0, 5, ours_emit=False, strip=False) == d.SKIPPED == 77
    assert d.verdict(0, 5, ours_emit=False, strip=True) == 1, "the stripped control must stay red"
    assert d.verdict(5, 0, ours_emit=True, strip=False) == 0
    assert d.verdict(4, 1, ours_emit=True, strip=False) == 1
    assert d.verdict(0, 0, ours_emit=True, strip=False) == 2


def test_a_llc_that_refuses_the_pending_windows_platform_skips_the_run(monkeypatch, capsys):
    """Before the Win64 rows landed the harness printed three refusals and exited 0 or 2, a green or red that said nothing about unwind data."""
    def refuse(command, *a, **k):
        if "-O2" in command and "-m64" in command:
            raise RuntimeError("x86_64-pc-windows-msvc: the Win64 calling convention is not carried; -fallow-pending-abi emits it anyway")
        return ""

    monkeypatch.setattr(d, "run", refuse)
    monkeypatch.setattr(d.sys, "argv", ["w", "--pin", "/p", "--llc", "/l", "a.c", "b.c"])
    assert d.main() == d.SKIPPED
    assert "SKIPPED" in capsys.readouterr().out
