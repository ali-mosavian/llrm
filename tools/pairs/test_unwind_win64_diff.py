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


NAMED = """
Function Table:
  Start Address: bench_hanoi
  End Address: bench_hanoi + 0x0014
  Unwind Info Address: $unwind$bench_hanoi
    Version: 1
    Flags: 0
    Size of prolog: 4
    Number of Codes: 1
    No frame pointer used
    Unwind Codes:
      0x04: UOP_AllocSmall 40
"""


def test_a_function_our_object_names_by_symbol_is_read_not_dropped():
    """`-u` names our functions by symbol: the parser read none, and the harness reported our object as having no unwind data at all (SKIPPED), blaming rows that had landed."""
    symbols = d.symbols_of("[ 4](sec  1)(fl 0x00)(ty  20)(scl   2) (nx 0) 0x00000030 bench_hanoi\n[ 5](sec  2)(fl 0x00)(ty 0)(scl 3) (nx 0) 0x00000000 other\n")
    assert symbols == {"bench_hanoi": 0x30}
    assert d.tables_of(NAMED, symbols) == {0x30: (0x44, 4, "No frame pointer used", ("UOP_AllocSmall 40",))}
    assert d.tables_of(NAMED) == {}, "a name nothing resolves is dropped, not guessed"


def test_the_instructions_of_a_function_are_read_not_an_empty_list_that_equals_every_other():
    """`code_of` matched no instruction line (spaces before the tab), so every function had no code and all compared as the same code: the harness 'compared' functions it had not looked at."""
    text = "0000000000000020 <bench_hanoi>:\n      20:      \tmovl\t$0x1, %edx\n      25:      \tjmp\t0x2a <bench_hanoi+0xa>\n"
    assert d.code_of(text) == {0x20: ["0 movl $0x1, %edx", "5 jmp 0x2a"]}
