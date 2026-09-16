"""The C path's object, written without jwasm."""

import sys
import shutil
import subprocess
from pathlib import Path

import pytest

from qbopt.backend import masm
from qbopt.objectfile import omf
from qbopt.backend import omfwrite
from qbopt.cfront import compile as cfront

ROOT = Path(__file__).resolve().parents[1]
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")
sys.path.insert(0, str(ROOT / "tools"))
import objcmp  # noqa: E402


def test_an_obj_output_uses_the_native_writer(tmp_path: Path) -> None:
    """The command used to write jwasm text regardless of the output suffix."""
    source = ROOT / "fixtures" / "c" / "choose.cgs"
    output = tmp_path / "choose.obj"

    assert cfront.main([str(source), "-o", str(output), "--opt"]) == 0
    records = omf.read(output)
    assert records[0].type == omf.THEADR
    assert any(record.type == omf.LEDATA for record in records)


@pytest.mark.skipif(not Path(JWASM).exists(), reason="jwasm is not installed")
@pytest.mark.parametrize("module", sorted(one.stem for one in (ROOT / "fixtures" / "c").glob("*.cgs")))
def test_the_object_is_the_one_jwasm_assembles(module: str, tmp_path: Path) -> None:
    """Segments, bytes and every fixup's target, against jwasm on the printed source."""
    try:
        built = cfront.assembled((ROOT / "fixtures" / "c" / f"{module}.cgs").read_text(), module, optimise=True)
    except cfront.hir.Unsupported as refused:
        pytest.skip(reason=f"the C path refuses it: {refused}")  # ty: ignore[unknown-argument]
    (tmp_path / f"{module}.asm").write_text(masm.text(built))
    (tmp_path / "ours.obj").write_bytes(omfwrite.written(built, f"{module}.c"))
    done = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path}/theirs.obj", f"{module}.asm"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )
    assert done.returncode == 0, done.stdout
    ignored = frozenset(one.name for one in built.procedures if not one.public)
    assert objcmp.compared(str(tmp_path / "ours.obj"), str(tmp_path / "theirs.obj"), ignored) == []
    assert set(omf.public_definitions(omf.read(tmp_path / "ours.obj"))) == set(built.publics)


@pytest.mark.skipif(not Path(JWASM).exists(), reason="jwasm is not installed")
def test_externals_are_declared_in_the_order_jwasm_declares_them(tmp_path: Path) -> None:
    """jwasm declares a data external before any procedure's, and LINK pulls
    library modules in EXTDEF order: CM.LIB's FILES landed elsewhere and
    `__streams` moved four bytes in qcport's DGROUP."""
    from iced_x86 import Register

    from qbopt.model import ir
    from qbopt.model import lir
    from qbopt.objectfile.module import Addr
    from qbopt.objectfile.module import Space

    load = ir.Semantics(ir.Operation.MOVE, "mov", (ir.Reg(Register.AX, 2),), (ir.Mem(Addr(Space.EXTERNAL, 0, 7), 2),))
    call = ir.Semantics(ir.Operation.CALL, "call")
    leave = ir.Semantics(ir.Operation.RETURN, "retf")
    insns = tuple(lir.Insn(at, (at, 1), what, (), ()) for at, what in enumerate((load, call, leave), 1))
    body = lir.LirBody("get", 1, (lir.LirBlock(1, insns),), {}, {})
    built = masm.Module(
        code="GET_TEXT",
        names={(Space.EXTERNAL, 7): "_d"},
        externs=(("_f", "far"), ("_d", "byte")),
        publics=("_get",),
        data=(("_DATA", ()),),
        procedures=(masm.Procedure("_get", True, True, body, 0, {2: masm.Callee("_f", True)}),),
    )
    (tmp_path / "get.asm").write_text(masm.text(built))
    (tmp_path / "ours.obj").write_bytes(omfwrite.written(built, "get.c"))
    command = [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", "-Fotheirs.obj", "get.asm"]
    done = subprocess.run(command, cwd=tmp_path, capture_output=True, text=True)
    assert done.returncode == 0, done.stdout
    assert objcmp.compared(str(tmp_path / "ours.obj"), str(tmp_path / "theirs.obj")) == []


def test_a_jump_growing_before_a_backward_target_leaves_that_branch_short() -> None:
    """The pass measured each item after a jump in it had grown, against labels
    from before: a backward branch whose target also moved looked a byte out of
    reach. 18 of 38 qcport objects came out longer than jwasm's."""
    grows, back = omfwrite.Jump("jmp", "far"), omfwrite.Jump("jmp", "top")
    items = [
        grows,
        omfwrite.masm.Label("top"),
        omfwrite.Piece(bytes(126)),
        back,
        omfwrite.Piece(bytes(200)),
        omfwrite.masm.Label("far"),
    ]
    labels = omfwrite._relaxed(items)
    assert (grows.long, back.long) == (True, False)
    assert labels["far"] == 3 + 126 + 2 + 200


def test_the_object_comparison_sees_a_different_first_data_byte(tmp_path: Path) -> None:
    """A difference at data offset zero evaluated as false, so the instrument
    reported two different objects as the same."""
    module = masm.Module("M_TEXT", {}, (), (), (("_DATA", (b"a",)),), ())
    other = masm.Module("M_TEXT", {}, (), (), (("_DATA", (b"b",)),), ())
    ours, theirs = tmp_path / "ours.obj", tmp_path / "theirs.obj"
    ours.write_bytes(omfwrite.written(module, "m.c"))
    theirs.write_bytes(omfwrite.written(other, "m.c"))

    assert objcmp.compared(str(ours), str(theirs)) == ["_DATA: first byte differs at 0x0"]


def test_the_object_comparison_sees_a_different_public_definition(tmp_path: Path) -> None:
    """The instrument ignored PUBDEF, so 138 static procedure labels missing
    from qcport's native objects were reported as exact matches to jwasm."""
    data = (("_DATA", (masm.Label("_cell"), b"a")),)
    private = masm.Module("M_TEXT", {}, (), (), data, ())
    public = masm.Module("M_TEXT", {}, (), ("_cell",), data, ())
    ours, theirs = tmp_path / "ours.obj", tmp_path / "theirs.obj"
    ours.write_bytes(omfwrite.written(private, "m.c"))
    theirs.write_bytes(omfwrite.written(public, "m.c"))

    assert objcmp.compared(str(ours), str(theirs)) == ["publics {} != {'_cell': (2, 0)}"]
