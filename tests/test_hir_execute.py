"""Executable known-answer checks for source-neutral HIR semantics."""

from pathlib import Path

from qbopt import hir
from qbopt.hir import execute
from qbopt.backend import masm
from qbopt.frontend.modern import driver
from qbopt.frontend.modern import compile as modern_compile

ROOT = Path(__file__).resolve().parents[1]
NBODY = ROOT / "frontends" / "modern" / "fixtures" / "nbody.mod"
CONTROL = ROOT / "frontends" / "modern" / "fixtures" / "control.mod"


def test_nbody_runs_from_source_through_hir_and_prints_fixed_values() -> None:
    """Local vec2i acc once failed as an indexed non-array before nbody could run."""
    result = execute.run(driver.parsed(NBODY), "nbody", (1,))

    assert result.output == (
        "PX=-14.896484375\n"
        "PY=-11.92578125\n"
        "VX=0.103515625\n"
        "VY=0.07421875\n"
        "PX=-7.97265625\n"
        "PY=-6.98046875\n"
        "VX=0.02734375\n"
        "VY=0.01953125\n"
        "PX=-1.0\n"
        "PY=-2.0\n"
        "VX=0.0\n"
        "VY=0.0\n"
        "PX=6.0\n"
        "PY=3.0\n"
        "VX=0.0\n"
        "VY=0.0\n"
        "PX=12.97265625\n"
        "PY=7.98046875\n"
        "VX=-0.02734375\n"
        "VY=-0.01953125\n"
        "PX=19.896484375\n"
        "PY=12.92578125\n"
        "VX=-0.103515625\n"
        "VY=-0.07421875\n"
        "DONE\n"
    )
    assert result.value == -10_177


def test_internal_calls_run_through_the_same_hir_executor() -> None:
    assert execute.run(driver.parsed(CONTROL), "count", (12,)).value == 11


def test_local_struct_copy_and_compound_assignment_have_value_semantics(tmp_path: Path) -> None:
    """A fieldwise store used to overwrite the source before a swapped field read."""
    source = tmp_path / "struct_values.mod"
    source.write_text(
        "struct point:\n"
        "    x: i16\n"
        "    y: i16\n"
        "fn calculate() -> i16:\n"
        "    var current: point = point { x: 1, y: 2 }\n"
        "    let snapshot = current\n"
        "    current = point { x: current.y, y: current.x }\n"
        "    current.x += snapshot.x\n"
        "    return current.x * 10 + current.y\n"
    )

    assert execute.run(driver.parsed(source), "calculate").value == 31


def test_context_typed_struct_literals_support_named_and_positional_fields(tmp_path: Path) -> None:
    """Nested aggregate initializers once repeated every nominal struct name."""
    source = tmp_path / "aggregate_literals.mod"
    source.write_text(
        "struct point:\n"
        "    x: i32\n"
        "    y: i32\n"
        "struct body:\n"
        "    pos: point\n"
        "    vel: point\n"
        "fn calculate() -> i32:\n"
        "    let bodies: [body; 2] = [\n"
        "        { pos: { x: 1, y: 2 }, vel: { x: 3, y: 4 } },\n"
        "        {{5, 6}, {7, 8}},\n"
        "    ]\n"
        "    return bodies[0].pos.x + bodies[0].pos.y * 10 + bodies[0].vel.x * 100 + "
        "bodies[0].vel.y * 1000 + bodies[1].pos.x * 10000 + bodies[1].pos.y * 100000 + "
        "bodies[1].vel.x * 1000000 + bodies[1].vel.y * 10000000\n"
    )

    assert execute.run(driver.parsed(source), "calculate").value == 87_654_321


def test_fixed_array_descriptor_methods_are_intrinsic_values(tmp_path: Path) -> None:
    """Fixed arrays once had payload storage but no language-visible descriptor ABI."""
    source = tmp_path / "array_descriptor.mod"
    source.write_text(
        "fn describe() -> u16:\n"
        "    let values: [i16; 3] = [10, 20, 30]\n"
        "    return values.len() + values.capacity() + values.dim(0)\n"
    )

    assert execute.run(driver.parsed(source), "describe").value == 9


def test_borrowed_array_metadata_comes_from_its_prefix_descriptor(tmp_path: Path) -> None:
    """An unsized `[i16]` view must recover all dimensions from its one pointer."""
    source = tmp_path / "borrowed_descriptor.mod"
    source.write_text(
        "fn describe(values: &[i16]) -> u16:\n"
        "    return values.len() + values.capacity() + values.dim(0)\n"
        "fn calculate() -> u16:\n"
        "    let values: [i16; 3] = [10, 20, 30]\n"
        "    return describe(&values)\n"
    )

    assert execute.run(driver.parsed(source), "calculate").value == 9


def test_borrowed_fixed_array_parameters_point_at_and_mutate_payload(tmp_path: Path) -> None:
    """Array arguments must be direct data pointers, not copied payloads or descriptor pointers."""
    source = tmp_path / "array_borrow.mod"
    source.write_text(
        "fn bump(values: &mut [u16]) -> void:\n"
        "    values[1] += values.len()\n"
        "fn calculate() -> u16:\n"
        "    var values: [u16; 3] = [10, 20, 30]\n"
        "    bump(&mut values)\n"
        "    return values[0] + values[1] + values[2]\n"
    )

    assert execute.run(driver.parsed(source), "calculate").value == 63


def test_borrowed_struct_arrays_and_reborrows_keep_scoped_mutation(tmp_path: Path) -> None:
    """A borrowed struct-array element must remain a direct view through nested calls."""
    source = tmp_path / "struct_array_borrow.mod"
    source.write_text(
        "struct point:\n"
        "    x: i16\n"
        "    y: i16\n"
        "fn nudge(point: &mut point) -> void:\n"
        "    point.x += point.y\n"
        "fn update(points: &mut [point]) -> void:\n"
        "    for point in &mut points:\n"
        "        nudge(&mut point)\n"
        "fn calculate() -> i16:\n"
        "    var points: [point; 2] = [{1, 2}, {10, 20}]\n"
        "    update(&mut points)\n"
        "    return points[0].x + points[1].x\n"
    )

    program = driver.parsed(source)
    assert execute.run(program, "calculate").value == 33
    assembly = masm.text(modern_compile.assembled(program, entry="calculate"))
    assert "call far ptr _update" in assembly
    assert "call far ptr _nudge" in assembly


def test_address_of_aggregate_does_not_read_the_aggregate() -> None:
    """Addressing a local array used to attempt an impossible first-class aggregate load."""
    void = hir.Type(1, "void", hir.TypeKind.VOID, 0)
    char = hir.Type(2, "char", hir.TypeKind.INTEGER, 1, signed=False)
    pointer = hir.Type(3, "*char", hir.TypeKind.POINTER, 2, element=char.id, address=hir.AddressKind.NEAR)
    array = hir.Type(
        4,
        "[char; 2]",
        hir.TypeKind.ARRAY,
        2,
        element=char.id,
        rank=1,
        bounds=((0, 1),),
        address=hir.AddressKind.NEAR,
    )
    local = hir.Place(1, "bytes", array.id, hir.Storage.LOCAL, -2, extent=2)
    address = hir.Instruction(1, hir.Op.ADDRESS, (1,), (hir.PlaceRef(local.id),))
    block = hir.Block(1, (address,), hir.Terminator(hir.TerminatorKind.RETURN))
    function = hir.Function(1, "address_array", void.id, (hir.Value(1, pointer.id),), (local,), (block,), 1)
    module = hir.Module(1, "address_array", (void, char, pointer, array), (function,))
    program = hir.Program(hir.Dialect.MODERN, hir.RuntimeProfile.FREESTANDING, (module,))

    assert execute.run(program, "address_array") == execute.Result("", None)
