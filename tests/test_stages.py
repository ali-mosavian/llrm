"""The dump rule 4 sends you to, and what it was not saying.

`_operand` printed a cell's address and nothing else, so a based cell --
one that names the value that computed its address, and after allocation
the register that value was placed in -- rendered identically to one BC
addressed itself. Two stage files could differ in the only field that
mattered and diff clean.
"""

import sys
from pathlib import Path
import pytest

from iced_x86 import Register

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))

import stages

from qbopt.model import ir
from qbopt.objectfile.module import Addr
from qbopt.objectfile.module import Space


def test_failed_optimization_keeps_observed_mir(tmp_path, monkeypatch):
    """Qrender PL_MOVE failed convergence and left only the initial dump, hiding all 16 rounds."""
    path = Path("fixtures/omf/addrm-q-O.obj")
    _, bodies, _ = stages._bodies(path.read_bytes())

    def fail(data, *, watch, **kwargs):
        for name, body in bodies:
            watch("mir-r01-fold", name, body)
        raise RuntimeError("MIR optimization did not converge")

    monkeypatch.setattr(stages.wholeseg, "emitted", fail)
    with pytest.raises(RuntimeError, match="did not converge"):
        stages.main([str(path), "--dump", str(tmp_path), "--quiet"])
    files = list(tmp_path.glob("*-mir-r01-fold.txt"))
    assert len(files) == 1
    assert "main (main)" in files[0].read_text()
    assert "--- mir" in files[0].read_text()
    assert not list(tmp_path.glob("*-asm-emitted.txt"))


def test_nbody_has_one_complete_file_per_machine_stage(tmp_path):
    """NBODY's last LIR stage file showed only PITSNAP, hiding the simulation body."""
    argv = ["fixtures/bench/nbody-v-g3.obj", "--dump", str(tmp_path), "--quiet"]
    assert stages.main(argv) == 0
    files = sorted(tmp_path.glob("*-lir-lowered.txt"))
    assert len(files) == 1, "bodies must share one file per phase for adjacent-stage diffs"
    text = files[0].read_text()
    assert text.count(" instructions\n") == 2
    assert "block 0x0030" in text
    for path in tmp_path.glob("*-lir-*.txt"):
        assert path.read_text().count(" instructions\n") == 2, path.name
    assert stages.main(argv) == 0
    assert files[0].read_text() == text, "a repeated dump must not append stale bodies"


def test_asm_names_relocations_instead_of_indistinguishable_zeroes(capsys):
    """ADDRM's distinct array operands and runtime calls all displayed as zero."""
    stages._asm(Path("fixtures/omf/addrm-q-O.obj").read_bytes())
    said = capsys.readouterr().out
    assert "reloc " in said
    assert "segment[" in said
    assert "B$" in said


def test_event_tail_jump_names_its_runtime_target(capsys):
    """BOOLS's event stub tail jump displayed 0:0, hiding the callback dispatcher."""
    stages._asm(Path("fixtures/omf/bools-p-evt.obj").read_bytes())
    said = capsys.readouterr().out
    jump = next(line for line in said.splitlines() if "0x003d" in line)
    assert "ptr16:16 B$EVK1+0x0" in jump


def test_asm_dumps_reachable_emulator_instructions_not_header_or_operands(capsys) -> None:
    """FPCSE's dump printed `bound` for its header and `push es` inside FLD."""
    import re
    from qbopt.frontend import blocks
    from qbopt.objectfile import module, omf

    data = Path("fixtures/omf/fpcse-p-g2.obj").read_bytes()
    found = module.of(omf.parse(data))
    reached = blocks.instructions(found)
    assert not isinstance(reached, str)
    stages._asm(data)
    said = capsys.readouterr().out
    addresses = [int(at, 16) for at in re.findall(r"^\s+(0x[0-9a-f]+)  ", said, re.M)]
    assert addresses == [insn.at for insn in reached]
    assert "fld" in said and "fadd" in said
    assert "emulator" in said


def test_a_based_cell_renders_the_value_and_the_register_it_was_placed_in() -> None:
    """`[es:bx+0x0]` said nothing about which value reached it."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    placed = ir.Mem(where, 2, Register.BX, 2, 1, base=ir.Held(17, 2))
    said = stages._operand(placed)
    assert "v17" in said, f"the cell does not say which value reached it: {said}"
    assert "BX" in said, f"the cell does not say where that value was placed: {said}"


def test_a_cell_nothing_based_renders_as_it_did() -> None:
    """Every other line in every stage file stays byte-identical."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    assert stages._operand(ir.Mem(where, 2)) == f"[{where}]"


def test_a_based_cell_nothing_placed_says_so() -> None:
    """Before allocation there is no register, and the dump has to show
    that rather than print a register that is not there."""
    where = Addr(Space.LITERAL, 0x2, base=Register.SI)
    unplaced = ir.Mem(where, 2, Register.NONE, 2, 1, base=ir.Held(17, 2))
    said = stages._operand(unplaced)
    assert "v17" in said and "BX" not in said, said


def test_the_machine_view_is_the_run_that_wrote_the_bytes() -> None:
    """s26 and the emitted asm have to be one program.

    The tool used to lower and run the phases itself to get them dumped --
    handed the absorbed set instead of the record, no coverage, no pins
    and one shared frame -- so the allocation in the dump was not the one
    that produced the object. lngmix's dump named a spill slot `[bp-18h]`
    the object never touches, and the first bad transition was not
    anywhere in the files rule 4 asks you to diff.
    """
    import re
    from pathlib import Path

    import stages as tool

    made: dict[str, str] = {}

    class Capture:
        """Every view, kept as text rather than written to a file."""

        def __init__(self) -> None:
            self.into = None

        def __call__(self, number: int, kind: str, name: str):
            import contextlib
            import io

            @contextlib.contextmanager
            def one():
                buffer = io.StringIO()
                with contextlib.redirect_stdout(buffer):
                    yield
                made[f"{kind}-{name}"] = buffer.getvalue()

            return one()

    was = tool.main
    seen = Capture()
    # --asm reaches the machine views without needing a directory.
    argv = [str(Path("fixtures/omf/lngmix-p-g2.obj")), "--quiet", "--asm"]
    got = tool.main(argv, view=seen)
    assert got == 0 and was is tool.main

    last = made.get("lir-prologue")
    emitted = made.get("asm-emitted")
    assert last and emitted, f"the machine views are missing: {sorted(made)}"
    assert "wrote these bytes" in emitted, "the dump does not say which emitter wrote them"

    slots = {int(one, 16) for one in re.findall(r"bp-0x([0-9a-f]+)", last)}
    wrote = {int(one, 16) for one in re.findall(r"bp-([0-9A-F]+)h", emitted)}
    assert slots <= wrote, f"the last stage names {sorted(slots - wrote)}, which the object never touches"
