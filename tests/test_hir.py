import re
from pathlib import Path
from dataclasses import replace

import pytest

from qbopt import hir
from qbopt.model import ir
from qbopt.model import lir
from qbopt.model import mir
from qbopt.backend import masm
from qbopt.backend import frame
from qbopt.analysis import loops
from qbopt.model import floating
from qbopt.objectfile import omf
from qbopt.backend import floatalloc
from qbopt.frontend.qb import finalized
from qbopt.frontend.qb import stage_text
from qbopt.frontend.qb import physicalize
from qbopt.objectfile.module import Space
from qbopt.backend import lower as lower_mir
from qbopt.frontend.qb import __main__ as qb_main
from qbopt.frontend.qb import driver as qb_driver
from qbopt.frontend.qb import compile as qb_compile

ROOT = Path(__file__).resolve().parents[1]


def test_qb_cli_exposes_bc_row_major_array_order(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """qb-qrender is built with BC /R; the object CLI formerly hid that semantic option."""
    seen: dict[str, object] = {}

    def parse(source: Path, **options: object) -> hir.Program:
        seen.update(options)
        return program()

    monkeypatch.setattr(qb_main, "parsed", parse)
    assert qb_main.main(["probe.bas", "--array-order", "row-major"]) == 0
    capsys.readouterr()
    assert seen["array_order"] == "row-major"


def test_qb_cli_exposes_pds_huge_array_option(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """PDHUGE wrapped at 64 KiB when the object CLI silently dropped BC /Ah."""
    seen: dict[str, object] = {}

    def parse(source: Path, **options: object) -> hir.Program:
        seen.update(options)
        return program()

    monkeypatch.setattr(qb_main, "parsed", parse)
    assert qb_main.main(["probe.bas", "--huge-arrays"]) == 0
    capsys.readouterr()
    assert seen["huge_arrays"] is True


def test_qb_cli_exposes_pds_alternate_math_option(
    monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """PDFPA linked BCL71ANR but its module header still claimed BC /FPi."""
    seen: dict[str, object] = {}

    def parse(source: Path, **options: object) -> hir.Program:
        seen.update(options)
        return program()

    monkeypatch.setattr(qb_main, "parsed", parse)
    assert qb_main.main(["probe.bas", "--alternate-math"]) == 0
    capsys.readouterr()
    assert seen["alternate_math"] is True


def test_lir_stage_formats_operandless_x87_store_as_intel() -> None:
    """The SYS stage showcase crashed when FSTP carried its target through effects only."""
    what = ir.Semantics(ir.Operation.FLOAT_STORE, "fstp")
    assert stage_text.instruction_text(what) == ("fstp",)


def program() -> hir.Program:
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    long = hir.Type(1, "long", hir.TypeKind.INTEGER, 4, signed=True)
    single = hir.Type(2, "single", hir.TypeKind.FLOAT, 4, evaluation=hir.FloatEvaluation.EXTENDED80)
    values = (hir.Value(1, 1), hir.Value(2, 1), hir.Value(3, 1), hir.Value(4, 2), hir.Value(5, 2))
    local = hir.Place(1, "total", 1, hir.Storage.LOCAL, -4, extent=4)
    entry = hir.Block(
        10,
        (
            hir.Instruction(1, hir.Op.COPY, (1,), (hir.Constant(1, 40),)),
            hir.Instruction(2, hir.Op.COPY, (2,), (hir.Constant(1, 2),)),
            hir.Instruction(3, hir.Op.ADD, (3,), (hir.ValueRef(1), hir.ValueRef(2))),
            hir.Instruction(4, hir.Op.STORE, (), (hir.PlaceRef(1), hir.ValueRef(3))),
            hir.Instruction(5, hir.Op.FADD, (5,), (hir.ValueRef(4), hir.ValueRef(4))),
        ),
        hir.Terminator(hir.TerminatorKind.BRANCH, (hir.ValueRef(3),), (20, 30)),
    )
    yes = hir.Block(20, (), hir.Terminator(hir.TerminatorKind.RETURN, (hir.ValueRef(3),)))
    no = hir.Block(30, (), hir.Terminator(hir.TerminatorKind.RETURN, (hir.Constant(1, 0),)))
    function = hir.Function(1, "step", 1, values, (local,), (entry, yes, no), 10, parameters=(4,))
    return hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "render", (void, long, single), (function,)),),
    )


def test_qb_driver_never_replays_a_stale_in_tree_release_binary(monkeypatch: pytest.MonkeyPatch) -> None:
    """The string-stage dump used old semantics after Rust sources changed."""
    monkeypatch.delenv("QBOPT_QBFRONT", raising=False)
    command = qb_driver.command()
    assert command[:4] == ("cargo", "run", "--quiet", "--release")
    assert str(qb_driver.MANIFEST) in command


@pytest.mark.full
def test_qb45_numeric_read_data_reaches_typed_hir_and_fresh_omf() -> None:
    """Q45N01 stopped at READ, then a native-only spill frame made READ report syntax error."""
    source = ROOT / "frontends/qb/compat/qb45/q45n01.bas"
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    main = program.modules[0].functions[0]
    calls = [
        instruction.callee
        for block in main.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL
    ]

    assert calls[:7] == [
        "B$RDI2",
        "B$RDI2",
        "B$RDI4",
        "B$RDI4",
        "B$RDI4",
        "B$RDR4",
        "B$RDR4",
    ]
    assert all(mir.verify(one.body) == [] for one in hir.lower(program))

    records = omf.parse(qb_compile.object_bytes(program, source.name))
    segments = omf.segments(records)
    ds_index, ds_size = next(
        (index, size)
        for index, item in enumerate(segments)
        if item is not None
        for name, size in (item,)
        if name == "BC_DS"
    )
    data = omf.segment_image(records, ds_index, ds_size)
    assert data[2:] == b" 17, 3, 100000, 3, 7, 1, 2\x00\xff\xff\x01"
    externals = omf.externals(records)
    assert {externals[fixup.index] for fixup in omf.fixups(records) if fixup.target == "external"} >= {
        "B$RDI2",
        "B$RDI4",
        "B$RDR4",
    }

    listing = masm.text(qb_compile.assembled(program))
    assert {"B$ENRA", "B$EXSA"} <= set(externals)
    assert "call far ptr B$RDI2" in listing
    assert "call far ptr B$RDI4" in listing
    assert "call far ptr B$RDR4" in listing


def test_restore_keys_select_the_labeled_serialized_data_row(tmp_path: Path) -> None:
    """Gorillas' synthetic DATA keys made B$RSTB fault before its first READ."""
    source = tmp_path / "restore.bas"
    source.write_bytes(b"restore later\r\nfirst: data 1\r\nlater: data 2\r\n")
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    assembled = qb_compile.assembled(program)
    records = omf.parse(qb_compile.object_bytes(program, source.name))
    segments = omf.segments(records)
    code_size = segments[1][1]
    ds_index, ds_size = next(
        (index, size)
        for index, item in enumerate(segments)
        if item is not None
        for name, size in (item,)
        if name == "BC_DS"
    )
    code = omf.segment_image(records, 1, code_size)
    read_data = omf.segment_image(records, ds_index, ds_size)
    first = int.from_bytes(read_data[0:2], "little")
    second_at = read_data.index(0, 2) + 1
    second = int.from_bytes(read_data[second_at : second_at + 2], "little")

    # BC emits one 90h marker per DATA row and stores those final code offsets
    # literally in BC_DS. They are not stream offsets and carry no FIXUPP.
    assert second == first + 1
    assert code[first] == code[second] == 0x90
    assert not [fixup for fixup in omf.fixups(records) if fixup.seg == ds_index]
    listing = masm.text(assembled)
    assert listing.count("xchg ax, ax") == 2
    assert "push offset" in listing
    assert "call far ptr B$RSTB" in listing


def test_inline_module_math_does_not_create_a_native_bp_frame(tmp_path: Path) -> None:
    """Gorillas' inline ATN added PUSH BP, so READ reported Out of stack space at R 0."""
    source = tmp_path / "ATNREAD.BAS"
    source.write_bytes(b"pi# = atn(1#)\r\ndata 7\r\nread value&\r\n")
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    records = omf.parse(qb_compile.object_bytes(program, source.name))
    code_size = omf.segments(records)[1][1]
    code = omf.segment_image(records, 1, code_size)

    assert code[48:51] != b"\x55\x8b\xec"
    assert b"\xd9\xf3" in code


def test_gorillas_beep_reaches_the_audited_zero_argument_runtime_call(tmp_path: Path) -> None:
    """Gorillas stopped in GETNUM because BEEP parsed but had no semantic ABI."""
    source = tmp_path / "BEEP.BAS"
    source.write_bytes(b"beep\r\n")
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    assert "call far ptr B$BEEP" in listing
    assert "add sp" not in listing


def test_gorillas_console_line_input_keeps_prompt_and_destination(tmp_path: Path) -> None:
    """Gorillas' player-name prompt was misparsed as a two-operand graphics LINE."""
    source = tmp_path / "LNINPUT.BAS"
    source.write_bytes(b'dim player as string\r\nline input "Name: "; player\r\n')
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    assert "call far ptr B$LNIN" in listing
    assert "call far ptr B$LINE" not in listing


def test_gorillas_implicit_string_suffix_drives_string_comparison(tmp_path: Path) -> None:
    """Gorillas stopped at DO WHILE Char$ = "" by treating undeclared Char$ as numeric."""
    source = tmp_path / "STRLOOP.BAS"
    source.write_bytes(b'do while char$ = ""\r\nchar$ = inkey$\r\nloop\r\n')
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    assert "call far ptr B$SCMP" in listing


def test_gorillas_print_tab_is_a_control_call_not_an_array(tmp_path: Path) -> None:
    """Gorillas' score line stopped because PRINT TAB(50) was resolved as an array."""
    source = tmp_path / "PRTAB.BAS"
    source.write_bytes(b'print "score"; tab(50); 7\r\n')
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    assert "pushw 50" in listing
    assert "call far ptr B$FTAB" in listing


def test_gorillas_point_is_resolved_from_the_intrinsic_table(tmp_path: Path) -> None:
    """Gorillas stopped at POINT(x#, y#) because it was resolved as an array."""
    source = tmp_path / "POINT.BAS"
    source.write_bytes(b"dim x as double, y as double, pixel as integer\r\npixel = point(x, y)\r\n")
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    assert "call far ptr B$PNR4" in listing


def test_gorillas_sleep_uses_the_long_runtime_abi(tmp_path: Path) -> None:
    """Gorillas reached SLEEP 1 with four typed bytes but no audited cleanup."""
    source = tmp_path / "SLEEP.BAS"
    source.write_bytes(b"sleep 1\r\n")
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    assert "call far ptr B$SLEP" in listing


def test_single_module_gosub_keeps_its_module_error_handler(tmp_path: Path) -> None:
    """Gorillas' InitVars became a fake procedure requiring QB45's nonexistent B$OEGP."""
    source = tmp_path / "GOSUBERR.BAS"
    source.write_bytes(
        b"gosub initvars\r\nend\r\ninitvars:\r\non error goto failed\r\nreturn\r\nfailed:\r\nresume next\r\n"
    )
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    main = listing.split("$QB$MAIN proc far", 1)[1].split("$QB$MAIN endp", 1)[0]
    assert "INITVARS proc far" not in listing
    assert "call far ptr B$OEGA" in main
    assert "call far ptr B$OEGP" not in listing


def test_procedure_dim_shadows_implicit_module_variable(tmp_path: Path) -> None:
    """Inlining Gorillas' GOSUB exposed module INTEGER i before a local SINGLE DIM i."""
    source = tmp_path / "SHADOW.BAS"
    source.write_bytes(b"i = 1\r\ncall probe\r\nsub probe\r\ndim i as single\r\ni = 1.5\r\nend sub\r\n")
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    assert any(function.name == "PROBE" for function in program.modules[0].functions)


def test_integer_floor_division_stays_integer_until_its_qb_single_result(tmp_path: Path) -> None:
    """Nibbles stored x87 status 16384 as arena(3,1).sister, then COLOR failed on 8224."""
    source = tmp_path / "floor.bas"
    source.write_bytes(b"dim row as integer, realRow as integer\r\nrow = 3\r\nrealRow = int((row + 1) / 2)\r\n")
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    projection = hir.mir_text(hir.lower(program)[0])

    assert " divmod 2:2" in projection
    assert "fcompare" not in projection
    assert " add 1:2" not in projection


def test_hir_json_is_deterministic_strict_and_replayable() -> None:
    text = hir.encode(program())
    assert text == hir.encode(hir.decode(text))
    assert '"schema":1' in text
    with pytest.raises(hir.InvalidHIR, match="unknown fields"):
        hir.decode(text.replace('"schema":1', '"register":"eax","schema":1'))
    with pytest.raises(hir.InvalidHIR, match="unknown Op"):
        hir.decode(text.replace('"op":"add"', '"op":"adc"'))
    projection = hir.mir_text(hir.lower(hir.decode(text))[0])
    assert "v3 <- v1:4 add v2:4" in projection
    assert "branch -> b2" in projection
    assert "[extended80,extended80->extended80;dynamic/dynamic]" in projection


def test_hir_verifier_rejects_incomplete_float_and_bad_cfg() -> None:
    source = program()
    module = source.modules[0]
    bad_float = replace(module.types[2], evaluation=hir.FloatEvaluation.NONE)
    with pytest.raises(hir.InvalidHIR, match="has no evaluation format"):
        hir.verify(replace(source, modules=(replace(module, types=(*module.types[:2], bad_float)),)))
    function = module.functions[0]
    entry = replace(function.blocks[0], terminator=hir.Terminator(hir.TerminatorKind.JUMP, targets=(999,)))
    broken = replace(function, blocks=(entry, *function.blocks[1:]))
    with pytest.raises(hir.InvalidHIR, match="unknown target"):
        hir.verify(replace(source, modules=(replace(module, functions=(broken,)),)))


def test_hir_verifier_rejects_a_store_with_the_wrong_value_type() -> None:
    source = program()
    module = source.modules[0]
    function = module.functions[0]
    instructions = list(function.blocks[0].instructions)
    instructions[3] = replace(instructions[3], operands=(hir.PlaceRef(1), hir.ValueRef(5)))
    entry = replace(function.blocks[0], instructions=tuple(instructions))
    broken = replace(function, blocks=(entry, *function.blocks[1:]))
    with pytest.raises(hir.InvalidHIR, match="store value type"):
        hir.verify(replace(source, modules=(replace(module, functions=(broken,)),)))


def test_hir_data_relocations_are_typed_and_bounded() -> None:
    source = program()
    module = source.modules[0]
    literal = hir.DataObject(
        7,
        "$string7",
        (3, 0, 0, 0, 97, 98, 99),
        readonly=True,
        relocations=(hir.DataRelocation(2, 7, 4, hir.AddressKind.NEAR),),
    )
    hir.verify(replace(source, modules=(replace(module, data=(literal,)),)))
    bad = replace(literal, relocations=(hir.DataRelocation(6, 7, 0, hir.AddressKind.NEAR),))
    with pytest.raises(hir.InvalidHIR, match="relocation exceeds initializer"):
        hir.verify(replace(source, modules=(replace(module, data=(bad,)),)))


def test_hir_lowers_long_float_memory_and_control_to_existing_mir() -> None:
    (lowered,) = hir.lower(program())
    assert mir.verify(lowered.body) == []
    operations = [one for block in lowered.body.blocks for one in block.ops]
    assert [one.kind for one in operations[:5]] == [
        mir.Kind.COPY,
        mir.Kind.COPY,
        mir.Kind.ADD,
        mir.Kind.STORE,
        mir.Kind.FADD,
    ]
    assert operations[2].args == (mir.Held(lowered.values[1], 4), mir.Held(lowered.values[2], 4))
    assert len(operations[3].args) == 1
    assert isinstance(operations[3].results[0], mir.Cell)
    machine = lower_mir.lowered("store", lowered.body, {}, set(), {}, occurrences={})
    store = next(
        one.what
        for one in machine.insns
        if one.what is not None
        and one.what.op is ir.Operation.MOVE
        and one.what.dests
        and isinstance(one.what.dests[0], ir.Mem)
    )
    assert isinstance(store.dests[0], ir.Mem)
    assert operations[3].stores[0].width == 4
    assert operations[3].stores[0].provenance is not None
    assert operations[4].floating is not None
    assert operations[4].floating.result.value == "extended80"
    assert lowered.body.blocks[0].succ == (20, 30)
    assert lowered.body.blocks[0].ops[-2].kind is mir.Kind.SUB
    assert lowered.body.blocks[0].ops[-1].test is mir.Kind.NE


def test_hir_lowers_typed_array_index_to_whole_offset_arithmetic() -> None:
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    long = hir.Type(1, "long", hir.TypeKind.INTEGER, 4, signed=True)
    array = hir.Type(2, "longs", hir.TypeKind.ARRAY, 40, element=1, rank=1, bounds=((1, 10),))
    values = (hir.Value(1, 1), hir.Value(2, 1))
    items = hir.Place(1, "items", 2, hir.Storage.MODULE, 0, symbol=7, extent=40)
    element = hir.ArrayElement(1, (hir.ValueRef(1),))
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.LOAD, (2,), (element,)),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "lookup", 0, values, (items,), (block,), 1, parameters=(1,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (
            hir.Module(
                1,
                "array",
                (void, long, array),
                (function,),
                data=(hir.DataObject(7, "$data", (0,) * 40),),
            ),
        ),
    )

    # The Rust frontend exposed this codec defect first: ArrayElement.indices
    # is a nested tagged union, which must survive the serialized HIR boundary.
    source = hir.decode(hir.encode(source))
    (lowered,) = hir.lower(source)
    operations = lowered.body.blocks[0].ops
    assert [one.kind for one in operations[:3]] == [mir.Kind.SUB, mir.Kind.MUL, mir.Kind.LOAD]
    assert operations[2].loads[0].base == lowered.values[4]
    assert operations[2].loads[0].provenance is not None
    lir = lower_mir.lowered(lowered.name, lowered.body, {}, set(), {}, occurrences={})
    assert lir.name == "array.lookup"
    assert any(insn.what is not None for insn in lir.insns)


def test_hir_lowering_honors_qb_multidimensional_array_order(tmp_path: Path) -> None:
    """Nibbles indexed ARENA(row,col) as row*80+col and passed garbage colors to B$COLR."""
    basic = tmp_path / "ORDER.BAS"
    basic.write_text(
        "dim shared grid(1 to 2, 1 to 3) as integer\n"
        "dim row as integer, col as integer, answer as integer\n"
        "answer = grid(row, col)\n"
    )

    factors = {}
    for order in ("column-major", "row-major"):
        program = qb_driver.parsed(basic, dialect="vbdos", runtime="vbdos", array_order=order)
        body = hir.lower(program)[0].body
        factors[order] = [
            argument.n
            for block in body.blocks
            for operation in block.ops
            if operation.kind is mir.Kind.MUL
            for argument in operation.args
            if isinstance(argument, mir.Const)
        ]

    assert factors["column-major"] == [2, 2]
    assert factors["row-major"] == [3, 2]


def test_canonical_mir_dump_keeps_call_identity() -> None:
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.CALL, callee="TWICE&"),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    call = hir.CallAbi(1, (), hir.StackCleanup.CALLEE, hir.CallDistance.FAR)
    function = hir.Function(1, "caller", 0, (), (), (block,), 1, calls=(call,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "calls", (void,), (function,)),),
    )
    assert "call TWICE&()" in hir.mir_text(hir.lower(source)[0])


def test_machine_stage_dump_is_masm_intel_not_python_repr() -> None:
    """Stage output used to expose Semantics(...), hiding the actual Intel operand order."""
    namespace = __import__("runpy").run_path("tools/qbstages.py")
    body = lir.LirBody(
        "sum",
        10,
        (
            lir.LirBlock(
                10,
                (
                    lir.Insn(
                        11,
                        None,
                        ir.Semantics(
                            ir.Operation.BINARY,
                            "add",
                            (ir.Held(3, 4),),
                            (ir.Held(1, 4), ir.Held(2, 4)),
                        ),
                        (3,),
                        (1, 2),
                    ),
                ),
            ),
        ),
        {},
        {},
    )

    dumped = namespace["_lir"](body)

    assert "sum proc" in dumped
    assert "add v3, v2" in dumped
    assert "Semantics(" not in dumped

    call = replace(body.blocks[0].insns[0], what=ir.Semantics(ir.Operation.CALL, "fsin"))
    call_body = replace(body, blocks=(replace(body.blocks[0], insns=(call,)),))
    inline = namespace["_lir"](
        call_body,
        {11: masm.Callee("$inline_fsin", False, (bytes.fromhex("d9fe"),))},
    )
    assert "call $inline_fsin" not in inline
    assert "db 0d9h,0feh" in inline


def test_qb_stage_dump_reads_the_same_cp437_source_as_the_frontend(tmp_path: Path) -> None:
    """Nibbles reached HIR, but the showcase crashed while copying byte DB from its source."""
    source = tmp_path / "CP437.BAS"
    source.write_bytes(b'print "\xdb"\r\n\x1aignored')
    namespace = __import__("runpy").run_path("tools/qbstages.py")
    assert namespace["_source_text"](source) == 'print "█"\r\n'


@pytest.mark.full
def test_qb_stage_dump_ends_with_the_emitted_runtime_abi_assembly(tmp_path: Path) -> None:
    """The showcase omitted ENRA and printed encoded RETF 4 as a bare RETF."""
    namespace = __import__("runpy").run_path("tools/qbstages.py")
    namespace["dumped"](
        ROOT / "frontends/qb/fixtures/runtime-frame-basic.bas",
        tmp_path,
        dialect="vbdos",
        runtime="vbdos",
        includes=(),
    )

    emitted = (tmp_path / "99-emitted-asm.asm").read_text()
    report = emitted.split("REPORT proc far\n", 1)[1].split("REPORT endp", 1)[0]
    # B$ENRA owns BP/SI/DI. The OMF emitter strips the shared backend's native
    # push-bp shell, so the allegedly exact final stage must strip it too.
    assert report.startswith("L1_1:\n    mov     cx, 6\n")
    assert "push    bp" not in report
    assert "mov     cx, 6" in emitted
    assert "call    far ptr B$ENRA" in emitted
    assert "call    far ptr B$EXSA" in emitted
    assert "retf    4" in emitted


def test_qb_stage_dump_replaces_exact_procedure_names(tmp_path: Path) -> None:
    """GorillaIntro's final stage displayed Intro's body because INTRO is its suffix."""
    source = tmp_path / "NAMES.BAS"
    source.write_bytes(b'sub gorillaIntro\r\nprint "GORILLA"\r\nend sub\r\nsub intro\r\nprint "INTRO"\r\nend sub\r\n')
    output = tmp_path / "stages"
    namespace = __import__("runpy").run_path("tools/qbstages.py")
    namespace["dumped"](
        source,
        output,
        dialect="qb45",
        runtime="qb45",
        includes=(),
    )

    lines = (output / "99-emitted-asm.asm").read_text().splitlines()

    def procedure(name: str) -> str:
        start = lines.index(f"{name} proc far")
        stop = lines.index(f"{name} endp", start + 1)
        return "\n".join(lines[start : stop + 1])

    assert "NAMES$D3" in procedure("GORILLAINTRO")
    assert "NAMES$D4" in procedure("INTRO")


def test_qb45_input_type_table_uses_dgroup_far_pointer(tmp_path: Path) -> None:
    """Nibbles panicked because every INPUT table was mistaken for a VBDOS far literal."""
    source = tmp_path / "INPUT.BAS"
    source.write_text('dim answer as string\ninput "Number"; answer\n')
    program = qb_driver.parsed(source, dialect="vbdos", runtime="qb45")

    table = next(one for one in program.modules[0].data if one.name.startswith("$input"))
    assert table.address is hir.AddressKind.NEAR
    qb_compile.object_bytes(program, source.name)


def test_implicit_module_end_uses_cenp_not_explicit_end_entry(tmp_path: Path) -> None:
    """UCA1 printed nothing and never returned because fallthrough called B$CEND."""
    source = tmp_path / "IMPLICIT.BAS"
    source.write_bytes(b'print "DONE"\r\n')
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    main = listing.split("$QB$MAIN proc far", 1)[1].split("$QB$MAIN endp", 1)[0]

    assert "call far ptr B$CENP" in main
    assert "call far ptr B$CEND" not in main


def test_runtime_entry_reserves_the_complete_live_local_extent() -> None:
    """B$ASSN's two stack pointers must not become two simultaneous ES inputs.

    The post-rebase frontend failed this 4096-byte local before frame emission
    with ``value#12 cannot be placed in fixed 71``: both semantic far-pointer
    effects had been mistaken for encoded ES operands of the call.
    """
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/runtime-frame-stack.bas")
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("REPORT proc far", 1)[1].split("REPORT endp", 1)[0]

    assert "mov cx, 4096" in procedure
    assert "call far ptr B$ENRA" in procedure
    assert "lea ax, [bp-4116]" in procedure
    assert "call far ptr B$EXSA" in procedure


def test_module_exit_rewrite_preserves_conditional_false_edges() -> None:
    """CALL probe printed OK for -42 because rewriting RETURN erased every block successor."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/runtime-call-basic.bas")
    listing = masm.text(qb_compile.assembled(source))
    main = listing.split("$QB$MAIN proc far", 1)[1].split("$QB$MAIN endp", 1)[0]

    # The shared layout may encode the false edge as the immediately
    # following block or as an explicit jump.  Either spelling must retain
    # the edge and both arms must converge on the runtime exit.
    assert "cmp eax, 42\n    je L0_2\nL0_3:" in main or "cmp eax, 42\n    je L0_2\n    jmp L0_3" in main
    assert main.count("call far ptr B$PESD") == 2
    assert "L0_4:\n    call far ptr B$CENP" in main
    assert "L0_2:" in main and "jmp L0_4" in main


def test_pascal_formals_are_read_in_reverse_physical_stack_order() -> None:
    """SUBTRACTPAIR read 8-50: calls pushed left-to-right but formals used ascending offsets."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/runtime-call-basic.bas")
    listing = masm.text(qb_compile.assembled(source))
    function = listing.split("SUBTRACTPAIR proc far", 1)[1].split("SUBTRACTPAIR endp", 1)[0]

    assert "mov eax, dword ptr [bp+10]" in function
    assert "sub eax, dword ptr [bp+6]" in function


def test_qb_call_abi_is_materialized_only_after_semantic_mir() -> None:
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    instruction = hir.Instruction(
        1,
        hir.Op.CALL,
        operands=(hir.Constant(1, 10), hir.Constant(1, 20)),
        callee="draw",
    )
    block = hir.Block(1, (instruction,), hir.Terminator(hir.TerminatorKind.RETURN))
    call = hir.CallAbi(1, (1, 0), hir.StackCleanup.CALLEE, hir.CallDistance.FAR)
    function = hir.Function(1, "caller", 0, (), (), (block,), 1, calls=(call,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "calls", (void, integer), (function,)),),
    )
    semantic = hir.lower(source)[0]
    assert semantic.body.blocks[0].ops[0].args == (mir.Const(10, 2), mir.Const(20, 2))
    physical = physicalize(source, function, semantic)
    operations = physical.lowered.body.blocks[0].ops
    assert [one.kind for one in operations] == [mir.Kind.ARG, mir.Kind.ARG, mir.Kind.CALL, mir.Kind.RETURN]
    assert operations[0].args == (mir.Const(20, 2),)
    assert operations[1].args == (mir.Const(10, 2),)
    assert operations[2].args == ()
    assert physical.contracts[operations[2].at].cleanup == 4
    assert operations[2].at in physical.far_calls
    machine = lower_mir.lowered(
        physical.lowered.name,
        physical.lowered.body,
        physical.calls,
        set(),
        physical.contracts,
        occurrences={},
    )
    assert [one.what.op for one in machine.insns if one.what is not None][:3] == [
        ir.Operation.PUSH,
        ir.Operation.PUSH,
        ir.Operation.CALL,
    ]


def test_qb_memory_argument_uses_its_reference_width() -> None:
    """STR$(single) failed before LIR because a Cell keeps width on its MemRef."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    single = hir.Type(1, "single", hir.TypeKind.FLOAT, 4, evaluation=hir.FloatEvaluation.EXTENDED80)
    argument = hir.Place(1, "$str4", 1, hir.Storage.LOCAL, -4, extent=4)
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.CALL, operands=(hir.PlaceRef(1),), callee="B$STR4"),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    call = hir.CallAbi(1, (0,), hir.StackCleanup.CALLEE, hir.CallDistance.FAR)
    function = hir.Function(1, "str_single", 0, (), (argument,), (block,), 1, calls=(call,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "strings", (void, single), (function,)),),
    )
    physical = physicalize(source, function, hir.lower(source)[0])
    operations = physical.lowered.body.blocks[0].ops
    assert isinstance(operations[0].args[0], mir.Cell)
    assert operations[0].args[0].ref.width == 4
    assert physical.contracts[operations[1].at].cleanup == 4


def test_dynamic_string_array_near_offset_reaches_physical_mir() -> None:
    """common.bas's whole-pointer string element failed HIR comparison verification."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/dynamic_strings.bas")
    function = source.modules[0].functions[0]
    semantic = hir.lower(source)[0]
    text = hir.mir_text(semantic)
    assert '"op":"ptr_offset"' in hir.encode(source)
    assert "call B$SCMP(" in text
    physical = physicalize(source, function, semantic)
    assert lower_mir.lowered(
        physical.lowered.name,
        physical.lowered.body,
        physical.calls,
        set(),
        physical.contracts,
        occurrences={},
        pointer_model=physical.pointer_model,
    ).insns


def test_single_line_if_exit_does_not_connect_its_unreachable_continuation() -> None:
    """LS_SELFTEST used values from the true arm after EXIT FUNCTION returned."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/early-exit.bas")
    classify = next(
        function for module in source.modules for function in module.functions if function.name.upper() == "CLASSIFY"
    )
    assert all(block.terminator.kind is not hir.TerminatorKind.UNREACHABLE for block in classify.blocks)
    lowered = hir.lower(source)
    assert all(not mir.verify(one.body) for one in lowered)


def test_qb_numeric_procedure_emits_a_fresh_far_pascal_object() -> None:
    """The source frontend formerly stopped at allocated LIR and could not link anything."""
    from qbopt.objectfile import omf

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/emission.bas")
    encoded = qb_compile.object_bytes(source, "emission.bas")
    records = omf.parse(encoded)
    public = omf.public_definitions(records)
    assert "ADDONE" in public
    code = omf.segment_image(records, 1, omf.segments(records)[1][1])
    assert code[:10] == b"blEMISSION"
    bc_sa = next(index for index, segment in enumerate(omf.segments(records)) if segment and segment[0] == "BC_SA")
    assert any(
        fixup.seg == bc_sa and fixup.offset == 0 and fixup.loc == 3 and fixup.target == "segment" and fixup.index == 1
        for fixup in omf.fixups(records)
    )
    segment, offset = public["ADDONE"]
    image = omf.segment_image(records, segment, omf.segments(records)[segment][1])
    procedure = image[offset:]
    statement_table = procedure.find(bytes.fromhex("558bec"), 3)
    assert statement_table > 0
    assert procedure[:statement_table].endswith(bytes.fromhex("ca0400"))


def test_source_procedure_names_match_all_three_microsoft_omf_dialects() -> None:
    """Cross-module calls must use BC's uppercase, unsuffixed Pascal symbols."""
    from qbopt.objectfile import omf

    expected = {"REPORT", "TWICE"}
    for fixture in (
        "procs-q-O-zi.obj",
        "procs-p-ot.obj",
        "procs-v-g3-zi.obj",
    ):
        records = omf.read(ROOT / "fixtures/omf" / fixture.lower())
        assert set(omf.public_definitions(records)) == expected
        assert expected <= set(omf.externals(records))

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/interop.bas")
    records = omf.parse(qb_compile.object_bytes(source, "interop.bas"))
    assert set(omf.public_definitions(records)) == expected

    caller = qb_driver.parsed(ROOT / "frontends/qb/fixtures/interop-external.bas")
    records = omf.parse(qb_compile.object_bytes(caller, "interop-external.bas"))
    assert expected <= set(omf.externals(records))


def test_def_fn_keeps_bcs_private_symbol_scope(tmp_path: Path) -> None:
    """Gorillas exported FNRAN even though BC keeps its DEF FN label private."""
    source = tmp_path / "SYMBOLS.BAS"
    source.write_bytes(
        b"def fnPrivate(value) = value + 1\r\nfunction Public(value)\r\npublic = fnPrivate(value)\r\nend function\r\n"
    )
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    functions = {function.name: function for function in program.modules[0].functions}
    assert functions["FNPRIVATE"].linkage is hir.FunctionLinkage.INTERNAL
    assert functions["PUBLIC"].linkage is hir.FunctionLinkage.EXTERNAL

    records = omf.parse(qb_compile.object_bytes(program, source.name))
    assert set(omf.public_definitions(records)) == {"PUBLIC"}


def test_module_globals_keep_bcs_effective_type_suffixes(tmp_path: Path) -> None:
    """QB45 /Zi calls implicit `implicit` IMPLICIT!, not an anonymous data offset."""
    source = tmp_path / "SYMNAM.BAS"
    source.write_bytes(
        b"dim shared implicit\r\n"
        b"dim shared explicitInteger as integer\r\n"
        b"dim shared explicitLong as long\r\n"
        b"dim shared explicitSingle as single\r\n"
        b"dim shared explicitDouble as double\r\n"
        b"dim shared explicitString as string * 8\r\n"
        b"dim shared implicitArray(1 to 2)\r\n"
        b"implicit = 1\r\n"
    )
    program = qb_driver.parsed(source, dialect="qb45", runtime="qb45")
    listing = masm.text(qb_compile.assembled(program))
    for name in (
        "IMPLICIT!",
        "EXPLICITINTEGER%",
        "EXPLICITLONG&",
        "EXPLICITSINGLE!",
        "EXPLICITDOUBLE#",
        "EXPLICITSTRING$",
        "IMPLICITARRAY!",
    ):
        assert f"{name} label byte" in listing
    assert "mov dword ptr IMPLICIT!, 1065353216" in listing


def test_qb_stage_assembly_compacts_typed_zero_globals(tmp_path: Path) -> None:
    """The readable stage must retain QB's named zero bytes, not anonymous DB runs."""
    source = tmp_path / "SYMNAM.BAS"
    source.write_bytes(
        b"dim shared implicit\r\n"
        b"dim shared explicitInteger as integer\r\n"
        b"dim shared explicitLong as long\r\n"
        b"dim shared explicitDouble as double\r\n"
        b"dim shared implicitArray(1 to 2)\r\n"
        b"implicit = 1\r\n"
    )
    output = tmp_path / "stages"
    namespace = __import__("runpy").run_path("tools/qbstages.py")
    namespace["dumped"](
        source,
        output,
        dialect="qb45",
        runtime="qb45",
        includes=(),
    )

    readable = (output / "99-emitted-asm.asm").read_text()
    raw = (output / "99-emitted-asm.raw.asm").read_text()

    assert "; QB source globals: BC-compatible effective names" in readable
    assert "IMPLICIT!            dd 0" in readable
    assert "EXPLICITINTEGER%     dw 0" in readable
    assert "EXPLICITLONG&        dd 0" in readable
    assert "EXPLICITDOUBLE#      dq 0" in readable
    assert "IMPLICITARRAY!       dq 0" in readable
    assert "\n    db 000h,000h,000h,000h" not in readable  # zero globals use typed declarations
    assert "\t" not in readable
    assert "IMPLICIT! label byte\ndb 000h,000h,000h,000h" in raw
    assert "EXPLICITDOUBLE# label byte\ndb 000h,000h,000h,000h,000h,000h,000h,000h" in raw


def test_qb_stage_assembly_aligns_code_and_hides_only_unreferenced_labels() -> None:
    """A display-only fall-through label obscured code; a jump target must stay visible."""
    namespace = __import__("runpy").run_path("tools/qbstages.py")
    displayed = namespace["_display_assembly"](
        "ONE proc far\n"
        "L1_1:\n"
        "    mov ax, 1\n"
        "L1_2:\n"
        "    add ax, 2\n"
        "    jne L1_4\n"
        "L1_3:\n"
        "    retf\n"
        "L1_4:\n"
        "    retf\n"
        "ONE endp\n"
    )

    assert "; Procedure: ONE" in displayed
    assert "L1_1:" in displayed  # procedure entry is an externally useful anchor
    assert "L1_2:" not in displayed
    assert "L1_3:" not in displayed
    assert "L1_4:" in displayed  # `jne` has a machine-code reference
    assert "    mov     ax, 1" in displayed
    assert "    jne     L1_4" in displayed
    assert "\t" not in displayed


@pytest.mark.parametrize("dialect,runtime", [("qb45", "qb45"), ("pds71", "pds71"), ("vbdos", "vbdos")])
def test_default_typed_len_emits_for_each_microsoft_runtime(dialect: str, runtime: str) -> None:
    """The default-type probe emitted only for VBDOS: PDS/QB rejected B$FLEN's missing ABI."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/fixtures/default-types.bas",
        dialect=dialect,
        runtime=runtime,
    )
    assert qb_compile.object_bytes(source, "default-types.bas")


def test_vbdos_module_header_records_the_measured_compiler_switches() -> None:
    """A fresh MAIN failed runtime initialization when U_FLAG claimed /FPa instead of /FPi."""
    from qbopt.objectfile import omf

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/emission.bas")
    source = replace(source, array_order=hir.ArrayOrder.ROW_MAJOR)
    records = omf.parse(qb_compile.object_bytes(source, "emission.bas"))
    code = omf.segment_image(records, 1, omf.segments(records)[1][1])

    # Measured from VBDOS BC /O /FPi /R /G3 /E. The runtime reads this word
    # during module initialization, so approximate or invented bits are ABI.
    assert int.from_bytes(code[46:48], "little") == 0x13C4


def test_fresh_basic_object_does_not_predeclare_the_c_data_class() -> None:
    """QGL stayed in the local heap scanner: empty `_DATA` moved BC_DATA behind the C runtime."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/emission.bas")
    records = omf.parse(qb_compile.object_bytes(source, "emission.bas"))
    names = [segment[0] for segment in omf.segments(records) if segment is not None]

    # A BC module owns BASIC's BC_DATA/BC_SEGS classes.  Declaring an empty C
    # DATA segment in the first link object makes LINK establish the opposite
    # DGROUP class order from VBDOS BC and breaks the runtime heap boundary.
    assert "_DATA" not in names
    assert names == [
        "EMISSION_CODE",
        "BR_DATA",
        "BR_SKYS",
        "COMMON",
        "BC_DATA",
        "NMALLOC",
        "ENMALLOC",
        "BC_FT",
        "BC_CN",
        "BC_DS",
        "BC_SAB",
        "BC_SA",
        "FDATA",
        "FSL_CONST",
    ]

    # The two BC_VARS sentinels are not decorative. The runtime uses their
    # offsets as its near-allocation range, so letting a later library member
    # introduce them after BC_CN makes local-heap initialization scan data.
    segments = omf.segments(records)
    assert [segments[index][0] for index in omf.groups(records)["DGROUP"]] == names[1:-2]


@pytest.mark.full
def test_pds_alternate_math_module_header_records_the_measured_switch() -> None:
    """PDFPA reached LINK, then BCL71ANR rejected the module during initialization."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/pds71/pdfpa.bas",
        dialect="pds71",
        runtime="pds71",
        alternate_math=True,
    )
    records = omf.parse(qb_compile.object_bytes(source, "PDFPA.BAS"))
    code = omf.segment_image(records, 1, omf.segments(records)[1][1])

    # Measured from PDS 7.1 BC /O /G2 /FPa. The only difference from the
    # otherwise identical /FPi invocation is U_FLAG 0x1088 vs 0x1084.
    assert source.float_mode is hir.FloatMode.ALTERNATE
    assert int.from_bytes(code[46:48], "little") == 0x1088


def test_command_line_is_resolved_as_the_zero_argument_runtime_intrinsic() -> None:
    """The full source build printed usage because COMMAND$ became an empty implicit local."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/command-line.bas")
    lowered = hir.lower(source)[0]
    calls = [op.name for block in lowered.body.blocks for op in block.ops if op.kind is mir.Kind.CALL]

    assert calls[:3] == ["B$FCMD", "B$LTRM", "B$RTRM"]


def test_timer_loads_the_single_returned_by_the_runtime_clock() -> None:
    """Fresh SYS_TIME_INIT loaded an implicit local forever instead of calling B$TIMR."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/timer-basic.bas")
    lowered = hir.lower(source)[0]
    operations = [op for block in lowered.body.blocks for op in block.ops]

    assert sum(op.kind is mir.Kind.CALL and op.name == "B$TIMR" for op in operations) == 3
    for index, operation in enumerate(operations):
        if operation.kind is mir.Kind.CALL and operation.name == "B$TIMR":
            following = operations[index + 1]
            assert following.kind is mir.Kind.FLOAD
            assert following.floating is not None
            assert following.uses == operation.defines
            assert following.results[0].width == 10


@pytest.mark.parametrize("dialect,runtime", [("qb45", "qb45"), ("pds71", "pds71"), ("vbdos", "vbdos")])
def test_timer_emits_for_each_microsoft_runtime(dialect: str, runtime: str) -> None:
    """PDS TIMER had the measured zero-byte ABI but no table entry, so object emission refused it."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/fixtures/timer-basic.bas",
        dialect=dialect,
        runtime=runtime,
    )
    assert qb_compile.object_bytes(source, "timer-basic.bas")


def test_qb_float_function_uses_hidden_near_result_pointer() -> None:
    """SYS_TICK_HZ left its SINGLE on x87, while BC callers passed and read a hidden result slot."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/float-function.bas")
    function = next(one for one in source.modules[0].functions if one.name == "ADDHALF")
    assert function.abi is not None
    assert function.abi.parameter_bytes == 6
    assert len(function.parameters) == 2

    listing = masm.text(qb_compile.assembled(source))
    assert re.search(r"lea ax, ([^\n]+)\n    push dword ptr ([^\n]+)\n    push ax\n    call far ptr ADDHALF", listing)
    assert "fstp dword ptr [bx]" in listing
    assert "mov ax, bx\n    call far ptr B$EXSA\n    pop bp\n    retf" in listing

    # The shared diagnostic MASM printer omits RETF's displayed immediate;
    # the emitted object and qbstages showcase preserve it exactly.
    from qbopt.objectfile import omf

    records = omf.parse(qb_compile.object_bytes(source, "float-function.bas"))
    segment, offset = omf.public_definitions(records)["ADDHALF"]
    image = omf.segment_image(records, segment, omf.segments(records)[segment][1])
    procedure = image[offset:]
    statement_table = procedure.find(bytes.fromhex("558bec"), 3)
    assert statement_table > 0
    assert procedure[:statement_table].endswith(bytes.fromhex("ca0600"))


def test_qb_long_function_boundary_uses_the_legacy_dx_ax_pair() -> None:
    """SYS_MEM_MARK saw only memAvail&'s low word: external LONG returns in DX:AX, not EAX."""
    external = qb_driver.parsed(ROOT / "frontends/qb/fixtures/external-long.bas")
    external_function = external.modules[0].functions[0]
    external_physical = physicalize(external, external_function, hir.lower(external)[0])
    call = next(
        op
        for block in external_physical.lowered.body.blocks
        for op in block.ops
        if external_physical.calls.get(op.at) == "MEMAVAIL&"
    )
    assert [result.width for result in call.results] == [2, 2]
    assert any(
        op.kind is mir.Kind.CONCAT and op.results[0].width == 4
        for block in external_physical.lowered.body.blocks
        for op in block.ops
    )

    internal = qb_driver.parsed(ROOT / "frontends/qb/fixtures/bare-function.bas")
    answer = next(function for function in internal.modules[0].functions if function.name == "ANSWER&")
    answer_body = next(
        body
        for function, body in zip(internal.modules[0].functions, hir.lower(internal), strict=True)
        if function is answer
    )
    answer_physical = physicalize(internal, answer, answer_body)
    returned = next(
        op for block in answer_physical.lowered.body.blocks for op in block.ops if op.kind is mir.Kind.RETURN
    )
    assert [argument.width for argument in returned.args] == [2, 2]


def test_qb_runtime_frame_establishes_and_zero_initializes_managed_locals() -> None:
    """SYS_PARSE_ARGS exhausted string space when a native shell preceded B$ENRA."""
    from qbopt.objectfile import omf
    from qbopt.objectfile import module as object_module

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/managed-locals.bas")
    records = omf.parse(qb_compile.object_bytes(source, "managed-locals.bas"))
    start = omf.public_definitions(records)["SHOWCOMMAND"][1]
    found = object_module.of(records)
    assert found is not None
    calls = [name for at, name in sorted(found.calls.items()) if at >= start]

    assert calls[:3] == ["B$ENRA", "B$DDIM", "B$FCMD"]
    assert calls[-1] == "B$EXSA"
    code_segment = omf.public_definitions(records)["SHOWCOMMAND"][0]
    image = omf.segment_image(records, code_segment, omf.segments(records)[code_segment][1])
    # B$ENRA owns PUSH BP/MOV BP, the BASIC frame link, SI/DI and local
    # reservation. VBDOS starts the procedure with MOV CX/MOV BX/CALL.
    assert image[start : start + 7] == bytes.fromhex("b91800bb01009a")


def test_vbdos_managed_locals_begin_below_the_runtime_frame_header() -> None:
    """SHOWCOMMAND overwrote B$EXSA's frame link and failed at 0825:0086."""
    from qbopt.backend import masm

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/managed-locals.bas")
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("SHOWCOMMAND proc far", 1)[1].split("SHOWCOMMAND endp", 1)[0]

    # VBDOS B$ENRA owns BP-2..BP-20h. Its /A listing places the array
    # descriptor at BP-26h and the scalar STRING descriptor at BP-2Ah.
    # The listing renders backpatch placeholders as zero; the raw linked
    # object says BX=1 for COMMAND$'s one live runtime STRING temporary.
    assert "mov bx, 1" in procedure
    assert "lea ax, [bp-38]" in procedure
    assert "lea ax, [bp-42]" in procedure


def test_runtime_frame_keeps_parameters_above_bp() -> None:
    """SYS read its Game argument at BP-0Eh and later raised error 64 opening the map."""
    from qbopt.backend import masm

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/emission.bas")
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("ADDONE proc far", 1)[1].split("ADDONE endp", 1)[0]

    # B$ENRA owns the frame directly, so the first far-Pascal parameter stays
    # at BP+6. The removed native BP shell used to shift this to BP+8.
    assert "dword ptr [bp+6]" in procedure
    assert "dword ptr [bp-14]" not in procedure


def test_runtime_frame_counts_owned_string_descriptors_not_runtime_temporaries() -> None:
    """LTRIM/RTRIM results were counted as local handles; raw BC emits BX=1, not 2."""
    from qbopt.backend import masm

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/managed-temporaries.bas")
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("SHOWCOMMAND proc far", 1)[1].split("SHOWCOMMAND endp", 1)[0]

    assert "mov bx, 1" in procedure


def test_source_call_releases_its_materialized_string_argument() -> None:
    """Nibbles left three Center arguments live until loop i became 0x2020."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/strtemp.bas", runtime="qb45")
    main = source.modules[0].functions[0]
    calls = [
        instruction.callee
        for block in main.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL
    ]

    show = calls.index("SHOW")
    assert calls[show - 1 : show + 2] == ["B$SASS", "SHOW", "B$STDL"]


def test_far_array_field_byref_uses_a_near_copy_in_copy_out_slot() -> None:
    """Nibbles pushed four bytes per PrintScore field, then RETF 10 left SP corrupted."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/farbyref.bas", runtime="qb45")
    module = source.modules[0]
    function = next(one for one in module.functions if one.name == "WORK")
    types = {one.id: one for one in module.types}
    values = {one.id: types[one.type] for one in function.values}
    instructions = [instruction for block in function.blocks for instruction in block.instructions]
    call = next(one for one in instructions if one.op is hir.Op.CALL and one.callee == "TOUCH")
    argument = call.operands[0]

    assert isinstance(argument, hir.ValueRef)
    assert values[argument.value].name == "near*integer"
    after = instructions[instructions.index(call) + 1 :]
    assert any(
        instruction.op is hir.Op.STORE and isinstance(instruction.operands[0], hir.IndirectPlace)
        for instruction in after
    )


def test_fixed_string_array_descriptor_carries_a_near_data_offset() -> None:
    """Fresh SYS loaded argv() as a huge pointer and DIR$ raised BASIC error 64."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/string-array-element.bas")
    module = source.modules[0]
    function = next(one for one in module.functions if one.name == "COPYFIRST")
    types = {one.id: one for one in module.types}
    pointer_types = [types[value.type] for value in function.values if types[value.type].kind is hir.TypeKind.POINTER]

    assert any(one.name == "near*string" for one in pointer_types)
    assert not any(one.name == "huge*string" for one in pointer_types)
    calls = [
        instruction.callee
        for block in function.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL
    ]
    assert "B$ERS1" in calls
    assert "B$ERAS" not in calls
    assert qb_compile.object_bytes(source, "string-array-element.bas")


def test_module_static_numeric_array_has_a_relocated_basic_descriptor() -> None:
    """Fresh SCREEN exited after `ugl`: BASIC startup zeroed its BC_DATA array descriptor."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/static-array-descriptor.bas")
    module = source.modules[0]
    main = next(one for one in module.functions if one.name == "__main")
    values = next(one for one in main.places if one.name == "VALUES")
    descriptor = next(one for one in main.places if one.name == "VALUES$descriptor")
    data = next(one for one in module.data if one.id == descriptor.symbol)

    assert data.readonly
    assert descriptor.storage.value == "static"
    assert descriptor.offset == 0
    assert bytes(data.bytes[descriptor.offset : descriptor.offset + descriptor.extent]) == bytes.fromhex(
        "00 00 00 00 00 00 00 00 01 40 00 00 04 00 04 00 00 00"
    )
    assert [(one.at, one.target, one.addend, one.address.value) for one in data.relocations] == [
        (0, values.symbol, values.offset, "far"),
        (10, values.symbol, values.offset, "near"),
    ]

    records = omf.parse(qb_compile.object_bytes(source, "static-array-descriptor.bas"))
    by_name = {
        segment[0]: (index, segment[1]) for index, segment in enumerate(omf.segments(records)) if segment is not None
    }
    constant, size = by_name["BC_CN"]
    assert omf.segment_image(records, constant, size) == bytes.fromhex(
        "06 00 00 00 00 00 00 00 01 40 06 00 04 00 04 00 00 00"
    )
    assert [(one.offset, one.loc, one.target, one.index) for one in omf.fixups(records) if one.seg == constant] == [
        (0, 1, "segment", by_name["BC_DATA"][0]),
        (2, 2, "group", 1),
        (10, 1, "segment", by_name["BC_DATA"][0]),
    ]


def test_rank_two_descriptor_matches_qb_dimension_order_and_adjusted_offset() -> None:
    """Q45A05 returned dimension 2 for LBOUND(a,1) because its descriptor was source-ordered."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/qb45/q45a05.bas",
        dialect="qb45",
        runtime="qb45",
    )
    module = source.modules[0]
    main = module.functions[0]
    values = next(one for one in main.places if one.name == "VALUES")
    descriptor = next(one for one in module.data if one.name == "VALUES$descriptor")

    assert bytes(descriptor.bytes[8:22]) == bytes.fromhex("02 40 00 00 02 00 03 00 01 00 02 00 01 00")
    assert [(one.at, one.target, one.addend, one.address.value) for one in descriptor.relocations] == [
        (0, values.symbol, values.offset, "far"),
        (10, values.symbol, values.offset - 6, "near"),
    ]


def test_static_array_formal_uses_a_lower_bound_adjusted_descriptor() -> None:
    """DYNARR wrote a(2).row, leaving a(1).row at zero after Touch a()."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/adjudt.bas")
    module = source.modules[0]
    main = module.functions[0]
    values = next(one for one in main.places if one.name == "A")
    descriptor = next(one for one in module.data if one.name == "A$descriptor")

    # QB's AD_oAdjusted is data - lower*elementWidth. The callee then adds
    # the source subscript directly; it does not subtract the lower bound.
    assert [(one.at, one.target, one.addend, one.address.value) for one in descriptor.relocations] == [
        (0, values.symbol, values.offset, "far"),
        (10, values.symbol, values.offset - 6, "near"),
    ]
    records = omf.parse(qb_compile.object_bytes(source, "ADJUDT.BAS"))
    segments = omf.segments(records)
    constants = next(index for index, one in enumerate(segments) if one and one[0] == "BC_CN")
    data = next(index for index, one in enumerate(segments) if one and one[0] == "BC_DATA")
    descriptor_fixups = [one for one in omf.fixups(records) if one.seg == constants and one.offset <= 10]
    assert [(one.offset, one.loc, one.target, one.index) for one in descriptor_fixups] == [
        (0, 1, "segment", data),
        (2, 2, "group", 1),
        (10, 1, "segment", data),
    ]


def test_dynamic_directive_makes_a_bounded_numeric_array_runtime_owned() -> None:
    """MOD_TEX first passed a static AD to RDIM, then addressed its far allocation through DGROUP."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/dynamic-bounded-array.bas")
    module = source.modules[0]
    main = next(one for one in module.functions if one.name == "__main")
    descriptor = next(one for one in main.places if one.name == "VALUES$descriptor")
    calls = [
        instruction.callee
        for block in main.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL
    ]

    assert descriptor.storage.value == "module"
    assert not any(one.readonly and one.name == "VALUES$descriptor" for one in module.data)
    assert calls == ["B$DDIM", "B$RDIM"]
    listing = masm.text(qb_compile.assembled(source))
    assert "+2]" in listing and "+10]" in listing
    assert "mov dword ptr es:[bx], 7" in listing


def test_dynamic_string_array_formal_uses_adjusted_near_descriptor_base() -> None:
    """COM_TOKENIZE passed a huge-pointer element to SASS, which reported string-space corruption."""
    from qbopt.backend import masm
    from qbopt.objectfile import omf

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/string-array-parameter.bas")
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("APPENDONE proc far", 1)[1].split("APPENDONE endp", 1)[0]

    # BC addresses a dynamic STRING array formal from the descriptor's
    # adjusted near base at +0Ah.  Loading the huge data pointer at +0 is a
    # different descriptor access path and hands SASS an invalid near address.
    assert "word ptr [bx+10]" in procedure or "word ptr [si+10]" in procedure
    assert "dword ptr [bx]" not in procedure and "dword ptr [si]" not in procedure
    # Runtime-produced descriptors live on the temporary chain; they do not
    # request an ENRA local-handle block.  BC emits BX=0 for this procedure.
    assert "mov bx, 0" in procedure
    # The fixed literal payload is in VBDOS's FSL_CONST far-data segment.  A
    # DGROUP selector makes B$ASSN copy a NUL and the program prints A\0.
    payloads = [place for place in source.modules[0].functions[1].places if place.name.endswith("$payload")]
    assert payloads and all(place.address is hir.AddressKind.FAR for place in payloads)
    records = omf.parse(qb_compile.object_bytes(source, "string-array-parameter.bas"))
    fsl = next(
        index
        for index, segment in enumerate(omf.segments(records))
        if segment is not None and segment[0] == "FSL_CONST"
    )
    assert any(fixup.seg == 1 and fixup.target == "segment" and fixup.index == fsl for fixup in omf.fixups(records))


def test_dynamic_numeric_array_formal_uses_split_adjusted_far_base() -> None:
    """D_SURF applied a one-based array's lower bound twice and hung before its first frame."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/numeric-array-parameter.bas")
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("SETFIRST proc far", 1)[1].split("SETFIRST endp", 1)[0]

    # VBDOS uses the allocation selector at AD+2 and the lower-bound-adjusted
    # offset at AD+0Ah for numeric formals, just as it does for owned arrays.
    assert "+2]" in procedure and "+10]" in procedure
    assert "dword ptr [bx]" not in procedure and "dword ptr [si]" not in procedure

    function = next(one for one in source.modules[0].functions if one.name == "SETFIRST")
    descriptor_loads = [
        operand
        for block in function.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.LOAD
        for operand in instruction.operands
        if isinstance(operand, hir.IndirectPlace)
    ]
    # AD+0Ah has already been adjusted for every declared lower bound. The
    # VBDOS listing uses index*width + AD+0Ah directly and never reads AD+10h.
    assert not any(one.offset == 16 for one in descriptor_loads)


def test_numeric_array_descriptor_snapshot_is_reused_until_an_effectful_call() -> None:
    """R_SET_FRUSTUM rebuilt the same split descriptor 38 times; one expression needs one stable base."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/numeric-array-base-reuse.bas")
    function = next(one for one in source.modules[0].functions if one.name == "COMBINE")
    loads = [
        operand
        for block in function.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.LOAD
        for operand in instruction.operands
        if isinstance(operand, hir.IndirectPlace)
    ]

    # Three accesses before and three after INSPECT collapse to one base in
    # each region. The unknown call remains an invalidation boundary.
    assert sum(one.offset == 2 for one in loads) == 2
    assert sum(one.offset == 10 for one in loads) == 2
    # The adjusted data offset already incorporates the lower bound. Rank-one
    # indexing therefore reads neither the lower bound nor a dimension count.
    assert not any(one.offset >= 14 for one in loads)


def test_string_function_copies_its_local_result_to_the_runtime_temporary_chain() -> None:
    """Qrender stopped at SYS_ERROR because COM_ARG returned descriptor bytes, not B$SCPF's AX pointer."""
    from qbopt.backend import masm

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/string-function-result.bas")
    module = source.modules[0]
    function = next(one for one in module.functions if one.name == "SECONDITEM")
    types = {one.id: one for one in module.types}
    result = types[function.result_type]
    calls = [
        instruction.callee
        for block in function.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL
    ]

    # A dynamic STRING expression is the near address of its descriptor.  A
    # source FUNCTION must copy its owned result onto the runtime temporary
    # chain before B$EXSA frees the frame, and return B$SCPF's AX pointer.
    assert result.kind is hir.TypeKind.POINTER
    assert result.width == 2
    assert types[result.element].name == "string"
    assert calls[-1] == "B$SCPF"

    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("SECONDITEM proc far", 1)[1].split("SECONDITEM endp", 1)[0]
    assert "call far ptr B$SCPF" in procedure
    assert "call far ptr B$EXSA" in procedure


def test_non_addressable_byref_string_argument_is_copied_to_an_owned_descriptor() -> None:
    """D_SURF's LS_LCHAR called LEN then ASC; a direct literal temporary was consumed and ASC raised error 5."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/asc-literal.bas")
    main = source.modules[0].functions[0]
    calls = [
        instruction.callee
        for block in main.blocks
        for instruction in block.instructions
        if instruction.op is hir.Op.CALL
    ]

    assert calls.index("B$SASS") < calls.index("FIRSTCODE")


def test_basic_procedure_arguments_use_pascal_left_to_right_push_order() -> None:
    """Fresh SYS passed COM_TOKENIZE backwards and its filled argv heap was corrupt."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/pascal-call-order.bas")
    call = source.modules[0].functions[0].calls[0]

    assert call.cleanup is hir.StackCleanup.CALLEE
    assert call.order == (0, 1)


def test_readonly_literals_use_the_measured_near_descriptor_and_far_payload() -> None:
    """MAIN's OPEN raised error 52 when a near string reference named only the far payload."""
    from qbopt.objectfile import omf

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/readonly-data.bas")
    records = omf.parse(qb_compile.object_bytes(source, "readonly-data.bas"))
    segments = omf.segments(records)
    by_name = {name: (index, size) for index, item in enumerate(segments) if item for name, size in (item,)}

    assert by_name["BC_DATA"][1] == 10  # six-byte BASIC prefix plus the writable LONG
    descriptor, size = by_name["BC_CN"]
    assert size == 6
    constant, size = by_name["FSL_CONST"]
    assert size == 8
    assert omf.combines(records)[constant] == 0  # private FAR_DATA, outside DGROUP
    descriptor_fixups = [one for one in omf.fixups(records) if one.seg == descriptor]
    assert [(one.offset, one.loc, one.target, one.index) for one in descriptor_fixups] == [
        (0, 2, "segment", constant),  # selector word
        (2, 1, "segment", constant),  # offset of the far string descriptor
        (4, 1, "segment", descriptor),  # selector-word address in DGROUP
    ]


@pytest.mark.parametrize(("dialect", "runtime"), (("qb45", "qb45"), ("pds71", "pds71")))
def test_qb_and_pds_literals_use_their_measured_near_descriptor(dialect: str, runtime: str) -> None:
    """PRINT "A" emitted VBDOS's far bridge and printed garbage under QB 4.5 and PDS 7.1."""
    from qbopt.objectfile import omf

    source = qb_driver.parsed(
        ROOT / "frontends/qb/fixtures/readonly-data.bas",
        dialect=dialect,
        runtime=runtime,
    )
    records = omf.parse(qb_compile.object_bytes(source, "readonly-data.bas"))
    segments = omf.segments(records)
    by_name = {name: (index, size) for index, item in enumerate(segments) if item for name, size in (item,)}

    descriptor, size = by_name["BC_CN"]
    assert size == 6
    # The shared writer carries the +4 target addend in the relocated word;
    # BC carries the equivalent displacement in FIXUPP. LINK resolves both to
    # the payload immediately after this four-byte descriptor.
    assert omf.segment_image(records, descriptor, size) == bytes.fromhex("01 00 04 00 41 00")
    # QB 4.5/PDS do not carry VBDOS's empty private far-data tail.  BC's
    # SEGDEF order ends at BC_SA for this source/runtime pair.
    assert "FSL_CONST" not in by_name and "FDATA" not in by_name and "QB_LINK" not in by_name
    assert [one[0] for one in segments if one is not None][1:] == [
        "BR_DATA",
        "BR_SKYS",
        "COMMON",
        "BC_DATA",
        "NMALLOC",
        "ENMALLOC",
        "BC_FT",
        "BC_CN",
        "BC_DS",
        "BC_SAB",
        "BC_SA",
    ]
    descriptor_fixups = [one for one in omf.fixups(records) if one.seg == descriptor]
    assert [(one.offset, one.loc, one.target, one.index) for one in descriptor_fixups] == [
        (2, 1, "segment", descriptor),
    ]


def test_on_error_emits_a_relocated_runtime_registration() -> None:
    """main.bas needs its handler address registered, not an ordinary CFG edge."""
    from qbopt.objectfile import omf

    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/on-error-emission.bas")
    records = omf.parse(qb_compile.object_bytes(source, "on-error-emission.bas"))
    externals = omf.externals(records)
    code = omf.segment_image(records, 1, omf.segments(records)[1][1])
    registration = code.find(bytes.fromhex("b8"), 48)
    # O_ENT is fixed at 48.  The temporary multi-entry retention CFG once
    # placed the handler's B$CEND here, so the executable exited before the
    # registration or source body ran.
    assert registration == 48  # mov ax, relocated handler offset
    assert code[registration + 3 : registration + 6] == bytes.fromhex("0e 50 9a")
    assert any(
        fixup.seg == 1 and fixup.target == "external" and externals[fixup.index] == "B$OEGA"
        for fixup in omf.fixups(records)
    )
    assert any(
        fixup.seg == 1 and fixup.target == "segment" and fixup.index == 1 and fixup.offset == registration + 1
        for fixup in omf.fixups(records)
    )


def test_on_error_registrations_follow_source_order(tmp_path: Path) -> None:
    """Gorillas ignored ON ERROR GOTO 0, sent a shot error to PaletteError, and resumed corrupt state."""
    basic = tmp_path / "ERRSTATE.BAS"
    basic.write_bytes(
        b"on error goto first\r\n"
        b'print "armed first"\r\n'
        b"on error goto second\r\n"
        b'print "armed second"\r\n'
        b"on error goto 0\r\n"
        b"error 11\r\n"
        b"end\r\n"
        b"first:\r\nresume next\r\n"
        b"second:\r\nresume next\r\n"
    )
    source = qb_driver.parsed(basic, dialect="qb45", runtime="qb45")
    records = omf.parse(qb_compile.object_bytes(source, "ERRSTATE.BAS"))
    externals = omf.externals(records)
    calls = [
        fixup
        for fixup in omf.fixups(records)
        if fixup.seg == 1 and fixup.target == "external" and externals[fixup.index] == "B$OEGA"
    ]

    assert len(calls) == 3
    code = omf.segment_image(records, 1, omf.segments(records)[1][1])
    disabled = calls[-1].offset - 6
    assert code[disabled : calls[-1].offset] == bytes.fromhex("b8 00 00 50 50 9a")


def test_resume_next_retains_runtime_statement_entries() -> None:
    """Q45R35's post-ERROR statement vanished, leaving RESUME NEXT with no target."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/qb45/q45r35.bas",
        dialect="qb45",
        runtime="qb45",
    )
    main = source.modules[0].functions[0]
    assert main.external_entries

    listing = masm.text(qb_compile.assembled(source))
    statement_table = listing.split("$QB$STAT proc near", 1)[1].split("$QB$STAT endp", 1)[0]
    assert "db 064h,000h" in statement_table
    assert statement_table.count("dw offset") >= 2

    records = omf.parse(qb_compile.object_bytes(source, "Q45R35.BAS"))
    code = omf.segment_image(records, 1, omf.segments(records)[1][1])
    header_fixup = next(one for one in omf.fixups(records) if one.seg == 1 and one.offset == 10)
    statement_at = int.from_bytes(code[10:12], "little") + header_fixup.disp
    assert code[statement_at - 3 : statement_at] == bytes.fromhex("55 8b ec")
    assert int.from_bytes(code[statement_at + 2 : statement_at + 4], "little") == 100


@pytest.mark.full
def test_resume_statement_entries_are_optimizer_roots() -> None:
    """Q45ER52 optimization made RESUME entries use values defined only from main.

    The resulting temporary-root verification failed at blocks 0x1f, 0x29,
    and 0x36 instead of emitting an object for the bounds-error test.
    """
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/qb45/q45er52.bas",
        dialect="qb45",
        runtime="qb45",
    )
    function = source.modules[0].functions[0]
    optimized = qb_compile.optimized(source, function, hir.lower(source)[0])
    entries = tuple(dict.fromkeys((*function.external_entries, function.error_handler)))
    checked, _root = qb_compile._machine_side_entry(optimized.body, entries)

    assert not mir.verify(checked)
    assert qb_compile.object_bytes(source, "Q45ER52.BAS")


def test_for_bounds_survive_resume_statement_side_entries(tmp_path: Path) -> None:
    """QGL MAIN's FOR bound was SSA-only, so RESUME side entries bypassed its definition."""
    basic = tmp_path / "FORRES.BAS"
    basic.write_bytes(
        b"on error goto handler\r\n"
        b"dim i as integer, limit as integer\r\n"
        b"limit = 2\r\n"
        b"for i = 0 to limit\r\n"
        b"print i\r\n"
        b"next i\r\n"
        b"end\r\n"
        b"handler:\r\n"
        b"resume next\r\n"
    )
    source = qb_driver.parsed(basic, dialect="vbdos", runtime="vbdos")
    lowered = hir.lower(source)
    assert all(not mir.verify(function.body) for function in lowered)
    assert qb_compile.object_bytes(source, "FORRES.BAS")


def test_inline_square_leaves_no_float_live_out(tmp_path: Path) -> None:
    """QGL HOST_RENDER's three `delta ^ 2` terms left duplicate x87 values live."""
    basic = tmp_path / "SQUARE.BAS"
    basic.write_bytes(b"dim x as single, answer as single\r\nanswer = x ^ 2\r\n")
    source = qb_driver.parsed(basic, dialect="vbdos", runtime="vbdos")
    assert qb_compile.object_bytes(source, "SQUARE.BAS")


def test_string_fre_emits_the_measured_vbdos_runtime_call(tmp_path: Path) -> None:
    """SYS_MEM_MARK stopped before HIR because FRE("") was sent through numeric lowering."""
    basic = tmp_path / "FRESTR.BAS"
    basic.write_bytes(b'dim available as long\r\navailable = fre("")\r\n')
    source = qb_driver.parsed(basic, dialect="vbdos", runtime="vbdos")

    records = omf.parse(qb_compile.object_bytes(source, "FRESTR.BAS"))
    assert "B$FRSD" in omf.externals(records)


@pytest.mark.full
def test_dynamic_array_walk_keeps_far_pointer_halves_defined(tmp_path: Path) -> None:
    """D_SURF SC_FTAKE lost far-array address definitions during secondary folding."""
    basic = tmp_path / "FARWALK.BAS"
    basic.write_bytes(
        b"option explicit\r\n"
        b"dim shared head() as integer\r\n"
        b"dim shared link() as integer\r\n"
        b"dim shared group() as integer\r\n"
        b"function take (byval order as integer, byval wanted as integer) as integer\r\n"
        b"dim block as integer, previous as integer\r\n"
        b"block = head(order)\r\n"
        b"previous = -1\r\n"
        b"while block >= 0\r\n"
        b"if group(block) = wanted then\r\n"
        b"if previous >= 0 then link(previous) = link(block) else head(order) = link(block)\r\n"
        b"take = block\r\n"
        b"exit function\r\n"
        b"end if\r\n"
        b"previous = block\r\n"
        b"block = link(block)\r\n"
        b"wend\r\n"
        b"take = -1\r\n"
        b"end function\r\n"
    )
    source = qb_driver.parsed(basic, dialect="vbdos", runtime="vbdos", array_order="row-major")

    assert qb_compile.object_bytes(source, "FARWALK.BAS")


def test_bare_def_seg_reaches_object_emission(tmp_path: Path) -> None:
    """SCREEN stopped at ABI lowering although VBDOS B$DSG0 is a zero-argument RETF."""
    basic = tmp_path / "DEFSEG.BAS"
    basic.write_bytes(b"def seg = 40960\r\npoke 12, 34\r\ndef seg\r\n")
    source = qb_driver.parsed(basic, dialect="vbdos", runtime="vbdos")

    records = omf.parse(qb_compile.object_bytes(source, "DEFSEG.BAS"))
    assert "B$DSG0" in omf.externals(records)
    assert "B$POKE" not in omf.externals(records)


def test_constant_screen_mode_pulls_its_graphics_driver(tmp_path: Path) -> None:
    """SCN9 called B$CSCN without B$EGAUSED, so LINK omitted EGA and SCREEN 9 raised error 5."""
    basic = tmp_path / "SCN9.BAS"
    basic.write_bytes(b"screen 9\r\nscreen 0\r\n")
    source = qb_driver.parsed(basic)

    records = omf.parse(qb_compile.object_bytes(source, "SCN9.BAS"))
    assert "B$EGAUSED" in omf.externals(records)


def test_variable_screen_mode_pulls_all_graphics_drivers(tmp_path: Path) -> None:
    """Gorillas SCREEN Mode linked no graphics modules and failed before drawing its first frame."""
    basic = tmp_path / "SCNVAR.BAS"
    basic.write_bytes(b"dim mode as integer\r\nmode = 9\r\nscreen mode\r\n")
    source = qb_driver.parsed(basic)

    records = omf.parse(qb_compile.object_bytes(source, "SCNVAR.BAS"))
    assert "B$GRPUSED" in omf.externals(records)


def test_nested_integer_division_keeps_each_dividend(tmp_path: Path) -> None:
    """Gorillas emitted IDIV AX twice for 30 \\ (80 \\ MaxCol), faulting on its first shot."""
    basic = tmp_path / "NESTDIV.BAS"
    basic.write_bytes(
        b"declare function scale (maxCol)\r\n"
        b"defint a-z\r\n"
        b"print scale(80)\r\n"
        b"end\r\n"
        b"function scale (maxCol)\r\n"
        b"scale = 30 \\ (80 \\ maxCol)\r\n"
        b"end function\r\n"
    )
    source = qb_driver.parsed(basic, dialect="qb45", runtime="qb45")
    assembly = masm.text(qb_compile.assembled(source))
    scale = assembly.split("SCALE proc far\n", 1)[1].split("SCALE endp\n", 1)[0]

    assert "mov eax, 80\n" in scale
    assert "mov eax, 30\n" in scale
    assert scale.count("idiv e") == 2
    assert "idiv ax" not in scale


def test_byref_dynamic_array_field_copies_through_a_near_formal(tmp_path: Path) -> None:
    """ENT_MOVE_TRIGS passed a four-byte far field address to a two-byte scalar formal."""
    basic = tmp_path / "FARFIELD.BAS"
    basic.write_bytes(
        b"option explicit\r\n"
        b"type Item\r\npad as integer\r\nvalue as integer\r\nend type\r\n"
        b"declare sub consume (number as integer)\r\n"
        b"dim shared items() as Item\r\n"
        b"sub invoke (byval index as integer)\r\n"
        b"consume items(index).value\r\n"
        b"end sub\r\n"
    )
    source = qb_driver.parsed(basic, dialect="vbdos", runtime="vbdos", array_order="row-major")

    module = source.modules[0]
    invoke = next(function for function in module.functions if function.name == "INVOKE")
    types = {type_.id: type_ for type_ in module.types}
    values = {value.id: types[value.type] for value in invoke.values}
    instructions = [instruction for block in invoke.blocks for instruction in block.instructions]
    call = next(one for one in instructions if one.op is hir.Op.CALL and one.callee == "CONSUME")
    argument = call.operands[0]
    assert isinstance(argument, hir.ValueRef)
    assert values[argument.value].name == "near*integer"
    assert any(one.op is hir.Op.LOAD and isinstance(one.operands[0], hir.IndirectPlace) for one in instructions)
    assert any(
        one.op is hir.Op.STORE and isinstance(one.operands[0], hir.IndirectPlace)
        for one in instructions[instructions.index(call) + 1 :]
    )

    assert qb_compile.object_bytes(source, "FARFIELD.BAS")


@pytest.mark.full
def test_runtime_frame_owns_spill_reservation_without_a_native_prefix() -> None:
    """Q45N01's native SUB SP shifted B$ENRA's documented frame fields by four bytes."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/qb45/q45n01.bas",
        dialect="qb45",
        runtime="qb45",
    )
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("$QB$MAIN proc far", 1)[1].split("$QB$MAIN endp", 1)[0]
    before_runtime_frame = procedure.split("call far ptr B$ENRA", 1)[0]
    assert "sub sp" not in before_runtime_frame


def test_local_error_and_resume_label_use_their_measured_procedure_abi() -> None:
    """LOCERR retains an ABI entry before its independently resumable ERROR.

    Threading the empty language entry into that statement made late
    B$ENRA/B$OEGP insertion miss its block, so the emitted procedure began at
    ERROR 53 with no runtime frame or local handler registration.
    """
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/vbdos/locerr.bas",
        dialect="vbdos",
        runtime="vbdos",
        array_order="row-major",
    )
    recover = next(one for one in source.modules[0].functions if one.name == "RECOVER_LOCALLY")
    assert recover.error_handler is not None
    assert recover.error_handler_local

    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("RECOVER_LOCALLY proc far", 1)[1].split("RECOVER_LOCALLY endp", 1)[0]
    entered = procedure.index("call far ptr B$ENRA")
    registered = procedure.index("call far ptr B$OEGP")
    resumed = procedure.index("call far ptr B$RESA")
    assert entered < registered < resumed
    assert "mov word ptr [bp-22]" in procedure[:resumed]
    assert "call far ptr B$OEGA" not in procedure
    assert procedure.rfind("mov ax, offset", registered, resumed) != -1


def test_pds_resume_target_and_numbered_erl_survive_distinct_identity_spaces() -> None:
    """PDLOCAL reported ERL 0 and resumed at L1_9 instead of recovered L1_4."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/pds71/pdlocal.bas",
        dialect="pds71",
        runtime="pds71",
    )
    listing = masm.text(qb_compile.assembled(source))
    procedure = listing.split("MISSINGFILE proc far", 1)[1].split("MISSINGFILE endp", 1)[0]
    assert "mov ax, offset L1_4\n    call far ptr B$RESA" in procedure

    statement_table = listing.split("$QB$STAT proc near", 1)[1].split("$QB$STAT endp", 1)[0]
    rows = [line.strip() for line in statement_table.splitlines() if line.strip().startswith("db ")]
    assert len(rows) > 4
    assert set(rows[:-1]) == {"db 064h,000h"}
    assert rows[-1] == "db 000h,000h"


@pytest.mark.full
def test_pds_huge_array_uses_measured_ddim_and_hary_abi() -> None:
    """PDHUGE wrapped/aliased beyond 64 KiB when /Ah was dropped and B$HARY was guessed inline."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/pds71/pdhuge.bas",
        dialect="pds71",
        runtime="pds71",
        array_order="row-major",
        huge_arrays=True,
    )
    function = source.modules[0].functions[0]
    semantic = hir.lower(source)[0]
    optimized = qb_compile.optimized(source, function, semantic)
    physical = physicalize(source, function, optimized)
    listing = hir.mir_text(physical.lowered)

    assert "v2 <- copy 65534:2" in listing
    assert "arg v2:2\n  arg 198:2\n  arg 0:2\n  arg 200:2" in listing
    assert "arg 2:2\n  arg 514:2" in listing
    assert listing.count("call B$HARY(") == 10
    assert re.search(r"v\d+, v\d+ <- call B\$HARY\(v\d+:2\)", listing)
    assert "+v4@v5):2 <- 123:2" in listing

    assembly = masm.text(qb_compile.assembled(source))
    assert assembly.count("call far ptr B$HARY") == 10
    assert "call far ptr B$HARY\n    mov word ptr es:[bx], 123" in assembly


def test_byref_call_keeps_the_temporary_values_it_publishes() -> None:
    """Q45P04 passed uninitialized slots after optimization deleted 100000 and 23."""
    source = qb_driver.parsed(
        ROOT / "frontends/qb/compat/qb45/q45p04.bas",
        dialect="qb45",
        runtime="qb45",
    )
    function = source.modules[0].functions[0]
    semantic = hir.lower(source)[0]
    optimized = qb_compile.optimized(source, function, semantic)
    listing = hir.mir_text(optimized)
    assert "100000:4" in listing
    assert "23:4" in listing


def test_unpublished_float_conversion_temporary_does_not_hold_the_x87_stack_across_a_branch() -> None:
    """FSTKBR's ``PICK = -1/0`` formerly left a volatile FILD live over its arm jump.

    Float allocation then refused the join with ``floating stack live-out
    requires cross-block allocation``.  `$arg` is only published when an
    ADDRESS reaches a BYREF call; an ordinary conversion scratch slot must
    retain neither that publication nor an x87 value after its exact store
    folds to bits.
    """
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/fstkbr.bas")
    function = next(one for one in source.modules[0].functions if one.name == "PICK")
    semantic = hir.lower(source)[list(source.modules[0].functions).index(function)]
    physical = physicalize(source, function, semantic)

    scratch_loads = [
        operation
        for block in physical.lowered.body.blocks
        for operation in block.ops
        if operation.kind is mir.Kind.FLOAD and operation.name == "fild"
    ]
    assert scratch_loads and all(not operation.volatile for operation in scratch_loads)

    optimized = qb_compile.optimized_physical(source, function, physical.lowered)
    assert not any(
        operation.kind is mir.Kind.FLOAD and operation.name == "fild"
        for block in optimized.body.blocks
        for operation in block.ops
    )
    assert qb_compile.object_bytes(source, "FSTKBR.BAS")


def test_positioned_file_calls_have_audited_pascal_cleanup() -> None:
    """The first SEEK stage reached ABI refinement but referenced no base contract."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/positioned_io.bas")
    function = source.modules[0].functions[0]
    physical = physicalize(source, function, hir.lower(source)[0])
    contracts = {name: physical.contracts[at] for at, name in physical.calls.items()}
    assert contracts["B$SSEK"].cleanup == 6
    assert contracts["B$GET4"].cleanup == 12
    assert contracts["B$PUT4"].cleanup == 12
    assert all(contract.established for contract in contracts.values())


def test_string_builders_have_descriptor_stack_contracts() -> None:
    """screen and mod_tex need STRING$ and LEFT$ to survive physicalization."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/string_builders.bas")
    function = source.modules[0].functions[0]
    physical = physicalize(source, function, hir.lower(source)[0])
    contracts = {name: physical.contracts[at] for at, name in physical.calls.items()}
    assert contracts["B$LEFT"].cleanup == 4
    assert contracts["B$STRI"].cleanup == 4
    assert contracts["B$STRS"].cleanup == 4


def test_classic_string_stack_abis_are_measured_for_every_runtime_family() -> None:
    """Q45LE71 reached B$LEFT but emission refused the previously VBDOS-only cleanup."""
    from qbopt.frontend.qb.abi import _contract

    for family in hir.RuntimeProfile:
        for name, pushed in {
            "B$LEFT": 4,
            "B$RGHT": 4,
            "B$UCAS": 2,
            "B$FHEX": 4,
            "B$FMKS": 4,
            "B$FMKD": 8,
            "B$FMSF": 4,
            "B$FMDF": 8,
            "B$FCVI": 2,
            "B$FCVL": 2,
            "B$FCVS": 2,
            "B$FCVD": 2,
            "B$MCVS": 2,
            "B$MCVD": 2,
            "B$INS3": 6,
        }.items():
            contract = _contract(name, hir.StackCleanup.CALLEE, pushed, family)
            assert contract.established
            assert contract.cleanup == pushed
            assert contract.inputs == frozenset()


def test_vbdos_nibbles_screen_calls_have_fixed_stack_contracts() -> None:
    """Nibbles reached physical HIR and stopped at unaudited screen-call cleanup."""
    from qbopt.frontend.qb.abi import _contract

    for name, pushed in {
        "B$SCLS": 2,
        "B$VWPT": 4,
        "B$SPLY": 2,
        "B$INKY": 0,
        "B$USNG": 2,
    }.items():
        contract = _contract(name, hir.StackCleanup.CALLEE, pushed, hir.RuntimeProfile.VBDOS)
        assert contract.established
        assert contract.cleanup == pushed
        assert contract.inputs == frozenset()


def test_qb_frontend_does_not_build_speculative_peel_candidates(monkeypatch: pytest.MonkeyPatch) -> None:
    """Nibbles INITCOLORS spent minutes optimizing rejected 50x80 peel candidates."""
    program = qb_driver.parsed(ROOT / "frontends/qb/fixtures/timer-basic.bas")
    function = program.modules[0].functions[0]
    body = hir.lower(program)[0]
    seen: dict[str, object] = {}

    def applied(candidate: mir.MirBody, *args: object, **kwargs: object) -> mir.MirBody:
        seen.update(kwargs)
        return candidate

    monkeypatch.setattr(qb_compile.transform, "applied", applied)
    qb_compile.optimized(program, function, body)
    assert seen.get("peel_", True) is False


def test_double_runtime_argument_is_split_high_to_low_at_the_qb_abi_boundary() -> None:
    """Q45FP61 reached OBJ emission with one unencodable eight-byte PUSH."""
    program = qb_driver.parsed(
        ROOT / "frontends/qb/compat/qb45/q45fp61.bas",
        dialect="qb45",
        runtime="qb45",
    )
    function = program.modules[0].functions[0]
    semantic = hir.lower(program)[0]
    optimized = qb_compile.optimized(program, function, semantic)
    physical = physicalize(program, function, optimized)
    calls = {at: name for at, name in physical.calls.items()}
    double_call = next(at for at, name in calls.items() if name == "B$FMKD")
    block = next(block for block in physical.lowered.body.blocks if any(op.at == double_call for op in block.ops))
    call_index = next(index for index, op in enumerate(block.ops) if op.at == double_call)
    parts = block.ops[call_index - 2 : call_index]
    assert all(op.kind is mir.Kind.ARG for op in parts)
    assert [op.args[0].ref.width for op in parts] == [4, 4]
    assert [op.args[0].ref.addr.disp for op in parts] == sorted(
        [op.args[0].ref.addr.disp for op in parts], reverse=True
    )


def test_dynamic_fixed_field_address_reaches_lir_as_pointer_arithmetic() -> None:
    """screen's green/blue fields formerly became an unlowerable address-of far cell."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/dynamic_fixed_fields.bas")
    function = source.modules[0].functions[0]
    physical = physicalize(source, function, hir.lower(source)[0])
    lowered = lower_mir.lowered(
        physical.lowered.name,
        physical.lowered.body,
        physical.calls,
        set(),
        physical.contracts,
        occurrences={},
        pointer_model=physical.pointer_model,
    )
    assert lowered.insns
    assert all(
        source.width == 2
        for instruction in lowered.insns
        if instruction.what is not None and instruction.what.op is ir.Operation.ADDRESS
        for source in instruction.what.sources
        if isinstance(source, ir.Mem)
    )


def test_segmented_local_array_address_splits_frame_base_from_dynamic_offset() -> None:
    """sc_selftest's SEG array argument formerly selected illegal ``[bp+bx]``."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/segmented_local_array.bas")
    function = next(one for one in source.modules[0].functions if one.name == "PROBE")
    semantic = hir.lower(source)[list(source.modules[0].functions).index(function)]
    addresses = [
        operation for block in semantic.body.blocks for operation in block.ops if operation.kind is mir.Kind.ADDRESS
    ]
    assert addresses
    assert all(
        not isinstance(argument, mir.Cell) or argument.ref.base is None
        for operation in addresses
        for argument in operation.args
    )
    assert any(operation.kind is mir.Kind.ADD for block in semantic.body.blocks for operation in block.ops)


def test_byref_fixed_string_field_forms_far_offset_without_absolute_lea() -> None:
    """common's g.env.cam_script formerly reached selection as ``lea [abs+offset]``."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/byref_fixed_string_field.bas")
    function = next(one for one in source.modules[0].functions if one.name == "FILL")
    semantic = hir.lower(source)[list(source.modules[0].functions).index(function)]
    assert all(
        not (
            operation.kind is mir.Kind.ADDRESS
            and isinstance(argument, mir.Cell)
            and argument.ref.addr is not None
            and argument.ref.addr.space is Space.LITERAL
            and argument.ref.base is None
        )
        for block in semantic.body.blocks
        for operation in block.ops
        for argument in operation.args
    )


def test_runtime_pointer_result_used_as_memory_base_is_an_explicit_mir_use() -> None:
    """common's VAL result formerly vanished because its following load named no MIR use."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/val.bas")
    semantic = hir.lower(source)[0]
    calls = [
        operation
        for block in semantic.body.blocks
        for operation in block.ops
        if operation.kind is mir.Kind.CALL and operation.name == "B$FVAL"
    ]
    assert calls
    for call in calls:
        result = call.defines[0]
        consumers = [
            operation
            for block in semantic.body.blocks
            for operation in block.ops
            if any(isinstance(argument, mir.Cell) and argument.ref.base == result for argument in operation.args)
        ]
        assert consumers and all(result in operation.uses for operation in consumers)


def test_udt_assignment_is_scalar_memory_copy_not_wide_register_value() -> None:
    """ent copied VEC3 as a fictitious 12-byte register before aggregate lowering."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/aggregate_copy.bas")
    function = next(one for one in source.modules[0].functions if one.name == "COPYVEC")
    semantic = hir.lower(source)[list(source.modules[0].functions).index(function)]
    assert all(
        argument.width <= 4
        for block in semantic.body.blocks
        for operation in block.ops
        for argument in (*operation.args, *operation.results)
        if isinstance(argument, mir.Held)
    )


def test_byval_float_is_stored_at_declared_width_before_stack_push() -> None:
    """screen passed extended values directly to a SINGLE-by-value UGL call."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/byval_float.bas")
    function = source.modules[0].functions[0]
    physical = physicalize(source, function, hir.lower(source)[0])
    machine = lower_mir.lowered(
        physical.lowered.name,
        physical.lowered.body,
        physical.calls,
        set(),
        physical.contracts,
        occurrences={},
        pointer_model=physical.pointer_model,
    )
    allocated = floatalloc.allocated(machine, frame.of(machine, physical.calls))
    pushes = [
        one.what.sources[0] for one in allocated.insns if one.what is not None and one.what.op is ir.Operation.PUSH
    ]
    # MIR retains the declared 4-byte and 8-byte values. Machine lowering
    # expands the qword argument into two legal 386 dword pushes, so the raw
    # allocated/assembly shape is three dword pushes (12 stack bytes), not a
    # nonexistent x86 `push qword`.
    assert [one.width for one in pushes] == [4, 4, 4]


def test_redim_stack_contract_uses_typed_rank_cleanup_not_register_arguments() -> None:
    """ent.bas reached B$RDIM with stack arguments but an object-raiser GP liveness contract."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    instruction = hir.Instruction(
        1,
        hir.Op.CALL,
        operands=tuple(hir.Constant(1, value) for value in (0, 9, 4, 257, 0)),
        callee="B$RDIM",
    )
    block = hir.Block(1, (instruction,), hir.Terminator(hir.TerminatorKind.RETURN))
    call = hir.CallAbi(1, (0, 1, 2, 3, 4), hir.StackCleanup.CALLEE, hir.CallDistance.FAR)
    function = hir.Function(1, "redim", 0, (), (), (block,), 1, calls=(call,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "array", (void, integer), (function,)),),
    )
    semantic = hir.lower(source)[0]
    physical = physicalize(source, function, semantic)
    call_op = next(one for one in physical.lowered.body.blocks[0].ops if one.kind is mir.Kind.CALL)
    contract = physical.contracts[call_op.at]
    assert contract.cleanup == 10
    assert contract.inputs == frozenset()
    assert contract.established
    assert lower_mir.lowered(
        "redim",
        physical.lowered.body,
        physical.calls,
        set(),
        physical.contracts,
        occurrences={},
    ).insns


def test_vbdos_erase_uses_typed_stack_call_without_weakening_unknown_effects() -> None:
    """r_bsp reached B$ERAS, whose VBDOS object contract measured cleanup but stayed conservative."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    instruction = hir.Instruction(1, hir.Op.CALL, operands=(hir.Constant(1, 0),), callee="B$ERAS")
    block = hir.Block(1, (instruction,), hir.Terminator(hir.TerminatorKind.RETURN))
    call = hir.CallAbi(1, (0,), hir.StackCleanup.CALLEE, hir.CallDistance.FAR)
    function = hir.Function(1, "erase", 0, (), (), (block,), 1, calls=(call,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "array", (void, integer), (function,)),),
    )
    physical = physicalize(source, function, hir.lower(source)[0])
    contract = next(iter(physical.contracts.values()))
    assert contract.cleanup == 2
    assert contract.inputs == frozenset()
    assert contract.established
    assert contract.writes.name == "ANY"


def test_qb_inline_sin_reaches_allocated_lir_without_a_runtime_call() -> None:
    """d_poly's SIN must stay an inline float value, not become B$SIN or cross CALL."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    single = hir.Type(1, "single", hir.TypeKind.FLOAT, 4, evaluation=hir.FloatEvaluation.EXTENDED80)
    values = (hir.Value(1, 1), hir.Value(2, 1))
    result = hir.Place(1, "answer", 1, hir.Storage.LOCAL, -4, extent=4)
    block = hir.Block(
        1,
        (
            hir.Instruction(1, hir.Op.FSIN, (2,), (hir.ValueRef(1),)),
            hir.Instruction(2, hir.Op.STORE, (), (hir.PlaceRef(1), hir.ValueRef(2))),
        ),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "wave", 0, values, (result,), (block,), 1, parameters=(1,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "trig", (void, single), (function,)),),
    )
    semantic = hir.lower(source)[0]
    # Recognition belongs at the HIR -> MIR boundary. A previous adapter
    # hid SIN as a pseudo CALL until ABI physicalization, turning pure math
    # into an opaque control and memory barrier for every optimizer.
    assert all(op.kind is not mir.Kind.CALL for block in semantic.body.blocks for op in block.ops)
    assert any(op.name == "fsin" for block in semantic.body.blocks for op in block.ops)
    physical = physicalize(source, function, semantic)
    operations = physical.lowered.body.blocks[0].ops
    assert [one.kind for one in operations[:2]] == [mir.Kind.FLOAD, mir.Kind.FSQRT]
    assert operations[1].name == "fsin"
    assert " fsin " in hir.mir_text(physical.lowered)
    assert " fsqrt " not in hir.mir_text(physical.lowered)
    machine = lower_mir.lowered(
        "wave",
        physical.lowered.body,
        physical.calls,
        set(),
        physical.contracts,
        occurrences={},
    )
    allocated = floatalloc.allocated(machine, frame.of(machine, physical.calls))
    assert any(one.what is not None and one.what.name == "fsin" for one in allocated.insns)
    assert not any(one.what is not None and one.what.op is ir.Operation.CALL for one in allocated.insns)
    final = finalized(allocated)
    inline = next(iter(final.callees.values()))
    assert inline.code == (bytes.fromhex("d9fe"),)
    assert any(one.what is not None and one.what.op is ir.Operation.CALL for one in final.body.insns)
    assembly = masm.text(
        masm.Module(
            "TRIG_TEXT",
            {},
            (),
            ("wave",),
            (),
            (masm.Procedure("wave", True, True, final.body, 4, final.callees),),
        )
    )
    assert "db 0d9h,0feh" in assembly
    assert "call fsin" not in assembly


def test_qb_finalizer_attaches_callee_cleanup_to_far_return() -> None:
    """A fresh QB procedure must end in RETF n; semantic MIR carries no stack ABI bytes."""
    returned = lir.Insn(
        1,
        (1, 1),
        ir.Semantics(ir.Operation.RETURN, "", (), ()),
        (),
        (),
    )
    body = lir.LirBody("callee", 1, (lir.LirBlock(1, (returned,)),), {}, {})
    final = finalized(body, parameter_bytes=6)
    returned = final.body.insns[0].what
    assert returned is not None and returned.sources == (ir.Imm(6, 2),)


def test_hir_lowers_whole_pointer_indirect_memory_without_machine_registers() -> None:
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    long = hir.Type(1, "long", hir.TypeKind.INTEGER, 4, signed=True)
    pointer = hir.Type(2, "huge*long", hir.TypeKind.POINTER, 4, element=1, address=hir.AddressKind.HUGE)
    values = (hir.Value(1, 2), hir.Value(2, 1))
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.LOAD, (2,), (hir.IndirectPlace(1, 0, 1),)),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "read", 0, values, (), (block,), 1, parameters=(1,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "pointer", (void, long, pointer), (function,)),),
    )
    semantic = hir.lower(hir.decode(hir.encode(source)))[0]
    operation = semantic.body.blocks[0].ops[0]
    assert operation.kind is mir.Kind.LOAD
    assert operation.loads[0].pointer
    assert operation.loads[0].base_width == 4
    assert operation.loads[0].addr is None
    # tools/qbstages first exposed that the source ABI adapter had omitted
    # DOS's established huge-pointer model: valid far byte loads reached MIR
    # and then failed at lowering with "needs an established pointer ABI".
    physical = physicalize(source, function, semantic)
    assert lower_mir.lowered(
        "read",
        physical.lowered.body,
        physical.calls,
        set(),
        physical.contracts,
        occurrences={},
        pointer_model=physical.pointer_model,
    ).insns


def test_hir_lowers_whole_pointer_field_offset_before_memory_access() -> None:
    """QBSP expanded each far UDT field into a complete huge-pointer correction.

    A FAR pointer advances only its offset word.  Keep selector, offset, and
    constant field displacement as address components instead of packing them
    into a scalar PTR_OFFSET which lowering must normalize and unpack again.
    """
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    aggregate = hir.Type(2, "pair", hir.TypeKind.OPAQUE, 4)
    pointer = hir.Type(3, "far*pair", hir.TypeKind.POINTER, 4, element=2, address=hir.AddressKind.FAR)
    values = (hir.Value(1, 3), hir.Value(2, 1))
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.LOAD, (2,), (hir.IndirectPlace(1, 2, 1),)),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "field", 0, values, (), (block,), 1, parameters=(1,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "pointer", (void, integer, aggregate, pointer), (function,)),),
    )
    operations = hir.lower(source)[0].body.blocks[0].ops
    assert [one.kind for one in operations[:4]] == [
        mir.Kind.EXTRACT,
        mir.Kind.EXTRACT,
        mir.Kind.ADD,
        mir.Kind.LOAD,
    ]
    reference = operations[3].loads[0]
    assert reference.addr is not None and reference.addr.space is Space.FAR
    assert reference.base == operations[2].results[0].value
    assert reference.segment == operations[1].results[0].value
    assert not reference.pointer


def test_huge_pointer_field_offset_retains_selector_normalization() -> None:
    """FAR field folding must not weaken /AH's distinct huge-pointer semantics."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    aggregate = hir.Type(2, "pair", hir.TypeKind.OPAQUE, 4)
    pointer = hir.Type(3, "huge*pair", hir.TypeKind.POINTER, 4, element=2, address=hir.AddressKind.HUGE)
    values = (hir.Value(1, 3), hir.Value(2, 1))
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.LOAD, (2,), (hir.IndirectPlace(1, 2, 1),)),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "field", 0, values, (), (block,), 1, parameters=(1,))
    source = hir.Program(
        hir.Dialect.PDS71,
        hir.RuntimeProfile.PDS71,
        (hir.Module(1, "pointer", (void, integer, aggregate, pointer), (function,)),),
    )

    operations = hir.lower(source)[0].body.blocks[0].ops

    assert [one.kind for one in operations[:2]] == [mir.Kind.PTR_OFFSET, mir.Kind.LOAD]
    assert operations[1].loads[0].pointer


def test_qb_module_instantiates_user_callee_modref_on_pointer_actuals() -> None:
    """RPOINTLEAF treated readonly RPLANEDIST as a write to every descriptor."""
    from qbopt.hir.model import Callable

    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    pointer = hir.Type(2, "near*integer", hir.TypeKind.POINTER, 2, element=1, address=hir.AddressKind.NEAR)
    caller = hir.Function(
        1,
        "CALLER",
        0,
        (hir.Value(1, 2),),
        (),
        (
            hir.Block(
                1,
                (hir.Instruction(1, hir.Op.CALL, operands=(hir.ValueRef(1),), callee="READ"),),
                hir.Terminator(hir.TerminatorKind.RETURN),
            ),
        ),
        1,
        parameters=(1,),
        calls=(hir.CallAbi(1, (0,), hir.StackCleanup.CALLEE, hir.CallDistance.FAR, callee=1),),
    )
    callee = hir.Function(
        2,
        "READ",
        0,
        (hir.Value(1, 2), hir.Value(2, 1)),
        (),
        (
            hir.Block(
                1,
                (hir.Instruction(1, hir.Op.LOAD, (2,), (hir.IndirectPlace(1, 0, 1),)),),
                hir.Terminator(hir.TerminatorKind.RETURN),
            ),
        ),
        1,
        parameters=(1,),
    )
    module = hir.Module(
        1,
        "modref",
        (void, integer, pointer),
        (caller, callee),
        callables=(Callable(1, "READ", None, (1,), (False,), (False,), (False,), True),),
    )
    program = hir.Program(hir.Dialect.VBDOS, hir.RuntimeProfile.VBDOS, (module,))

    bodies = qb_compile._alias_annotated(module, module.functions, hir.lower(program))
    call = next(one for block in bodies[0].body.blocks for one in block.ops if one.kind is mir.Kind.CALL)

    assert call.memory_complete
    assert call.loads
    assert call.stores == ()


def test_qb_string_comparison_abi_site_survives_alias_annotation() -> None:
    """SCMPABI's B$SCMP ABI site was dropped because STRING_EQ is not Op.CALL."""
    source = qb_driver.parsed(ROOT / "frontends/qb/fixtures/scmpabi.bas")
    function = next(one for one in source.modules[0].functions if one.name == "MATCHES")
    instruction = next(
        one for block in function.blocks for one in block.instructions if one.id == function.calls[0].instruction
    )

    assert instruction.op is hir.Op.STRING_EQ
    listing = masm.text(qb_compile.assembled(source))
    assert "call far ptr B$SCMP" in listing


def test_far_float_access_splits_selector_and_offset_for_x87_memory() -> None:
    """ent.bas used a SINGLE field through a SEG formal; generic whole-memory expansion is integral only."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    single = hir.Type(1, "single", hir.TypeKind.FLOAT, 4, evaluation=hir.FloatEvaluation.EXTENDED80)
    pointer = hir.Type(2, "far*single", hir.TypeKind.POINTER, 4, element=1, address=hir.AddressKind.FAR)
    values = (hir.Value(1, 2), hir.Value(2, 1))
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.LOAD, (2,), (hir.IndirectPlace(1, 0, 1),)),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "far_float", 0, values, (), (block,), 1, parameters=(1,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "float", (void, single, pointer), (function,)),),
    )
    body = hir.lower(source)[0].body
    assert [one.kind for one in body.blocks[0].ops[:3]] == [mir.Kind.EXTRACT, mir.Kind.EXTRACT, mir.Kind.FLOAD]
    reference = body.blocks[0].ops[2].loads[0]
    assert reference.addr is not None and reference.addr.space is Space.FAR
    assert reference.segment is not None
    assert lower_mir.lowered("far_float", body, {}, set(), {}, occurrences={}).insns


def test_far_float_compare_fuses_the_comparison_not_an_inserted_extract() -> None:
    """ENT_PLAT_TOUCHED had a synthetic EXTRACT whose numeric id collided with the source LT id."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    single = hir.Type(1, "single", hir.TypeKind.FLOAT, 4, evaluation=hir.FloatEvaluation.EXTENDED80)
    pointer = hir.Type(2, "far*single", hir.TypeKind.POINTER, 4, element=1, address=hir.AddressKind.FAR)
    boolean = hir.Type(3, "boolean", hir.TypeKind.BOOLEAN, 2, signed=True)
    values = (hir.Value(1, 2), hir.Value(2, 1), hir.Value(3, 1), hir.Value(4, 3))
    blocks = (
        hir.Block(
            1,
            (
                hir.Instruction(1, hir.Op.LOAD, (3,), (hir.IndirectPlace(1, 0, 1),)),
                hir.Instruction(2, hir.Op.LT, (4,), (hir.ValueRef(2), hir.ValueRef(3))),
            ),
            hir.Terminator(hir.TerminatorKind.BRANCH, (hir.ValueRef(4),), (2, 3)),
        ),
        hir.Block(2, (), hir.Terminator(hir.TerminatorKind.RETURN)),
        hir.Block(3, (), hir.Terminator(hir.TerminatorKind.RETURN)),
    )
    function = hir.Function(1, "far_compare", 0, values, (), blocks, 1, parameters=(1, 2))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "float", (void, single, pointer, boolean), (function,)),),
    )
    body = hir.lower(source)[0].body
    kinds = [one.kind for one in body.blocks[0].ops]
    assert kinds == [mir.Kind.EXTRACT, mir.Kind.EXTRACT, mir.Kind.FLOAD, mir.Kind.FCOMPARE, mir.Kind.BRANCH]
    assert lower_mir.lowered("far_compare", body, {}, set(), {}, occurrences={}).insns


def test_branch_comparison_reaches_existing_flag_form() -> None:
    """vid.bas initially reached LIR with an unencodable boolean EQ value."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    boolean = hir.Type(2, "boolean", hir.TypeKind.BOOLEAN, 2, signed=True)
    values = (hir.Value(1, 1), hir.Value(2, 1), hir.Value(3, 2))
    entry = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.EQ, (3,), (hir.ValueRef(1), hir.ValueRef(2))),),
        hir.Terminator(hir.TerminatorKind.BRANCH, (hir.ValueRef(3),), (2, 3)),
    )
    blocks = (
        entry,
        hir.Block(2, (), hir.Terminator(hir.TerminatorKind.RETURN)),
        hir.Block(3, (), hir.Terminator(hir.TerminatorKind.RETURN)),
    )
    function = hir.Function(1, "compare", 0, values, (), blocks, 1, parameters=(1, 2))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "flags", (void, integer, boolean), (function,)),),
    )
    body = hir.lower(source)[0].body
    assert [one.kind for one in body.blocks[0].ops] == [mir.Kind.SUB, mir.Kind.BRANCH]
    assert body.blocks[0].ops[-1].test is mir.Kind.EQ
    assert lower_mir.lowered("compare", body, {}, set(), {}, occurrences={}).insns


def test_comparison_used_as_a_value_is_materialized_as_qb_minus_one_or_zero() -> None:
    """h_frame combined two comparisons with AND; GE cannot survive as a register-producing MIR op."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    boolean = hir.Type(2, "boolean", hir.TypeKind.BOOLEAN, 2, signed=True)
    values = tuple(hir.Value(number, type_) for number, type_ in ((1, 1), (2, 1), (3, 2), (4, 2)))
    block = hir.Block(
        1,
        (
            hir.Instruction(1, hir.Op.GE, (3,), (hir.ValueRef(1), hir.ValueRef(2))),
            hir.Instruction(2, hir.Op.AND, (4,), (hir.ValueRef(3), hir.Constant(2, -1))),
        ),
        hir.Terminator(hir.TerminatorKind.BRANCH, (hir.ValueRef(4),), (2, 3)),
    )
    function = hir.Function(
        1,
        "boolean_value",
        0,
        values,
        (),
        (
            block,
            hir.Block(2, (), hir.Terminator(hir.TerminatorKind.RETURN)),
            hir.Block(3, (), hir.Terminator(hir.TerminatorKind.RETURN)),
        ),
        1,
        parameters=(1, 2),
    )
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "bool", (void, integer, boolean), (function,)),),
    )
    body = hir.lower(source)[0].body
    kinds = [one.kind for block_ in body.blocks for one in block_.ops]
    assert mir.Kind.GE not in kinds
    assert kinds.count(mir.Kind.STORE) == 2
    assert mir.Kind.LOAD in kinds
    assert lower_mir.lowered("boolean_value", body, {}, set(), {}, occurrences={}).insns


def test_remainder_lowers_as_existing_divmod_pair() -> None:
    """vid.bas reached emission with REM, while the backend accepts DIVMOD."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    values = (hir.Value(1, 1), hir.Value(2, 1), hir.Value(3, 1))
    block = hir.Block(
        1,
        (hir.Instruction(1, hir.Op.REM, (3,), (hir.ValueRef(1), hir.ValueRef(2))),),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "modulo", 0, values, (), (block,), 1, parameters=(1, 2))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "divide", (void, integer), (function,)),),
    )
    operation = hir.lower(source)[0].body.blocks[0].ops[0]
    assert operation.kind is mir.Kind.DIVMOD
    assert len(operation.results) == 2


def test_integer_to_float_conversion_uses_x87_storage_load() -> None:
    """vid.bas INTEGER-to-SINGLE conversion first survived as unencodable CONVERT."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    single = hir.Type(2, "single", hir.TypeKind.FLOAT, 4, evaluation=hir.FloatEvaluation.EXTENDED80)
    temporary = hir.Place(1, "$convert", 1, hir.Storage.LOCAL, -2, extent=2)
    values = (hir.Value(1, 2),)
    block = hir.Block(
        1,
        (
            hir.Instruction(1, hir.Op.STORE, operands=(hir.PlaceRef(1), hir.Constant(1, 7))),
            hir.Instruction(2, hir.Op.CONVERT, (1,), (hir.PlaceRef(1),)),
        ),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "to_single", 0, values, (temporary,), (block,), 1)
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "convert", (void, integer, single), (function,)),),
    )
    body = hir.lower(source)[0].body
    conversion = body.blocks[0].ops[1]
    assert conversion.kind is mir.Kind.FLOAD
    assert conversion.floating is not None
    assert lower_mir.lowered("to_single", body, {}, set(), {}, occurrences={}).insns


def test_float_conversions_and_negation_match_encodable_x87_semantics() -> None:
    """ent/h_frame exposed exact FCHS, DOUBLE-to-SINGLE rounding, and CINT materialization."""
    void = hir.Type(0, "void", hir.TypeKind.VOID, 0)
    integer = hir.Type(1, "integer", hir.TypeKind.INTEGER, 2, signed=True)
    single = hir.Type(2, "single", hir.TypeKind.FLOAT, 4, evaluation=hir.FloatEvaluation.EXTENDED80)
    double = hir.Type(3, "double", hir.TypeKind.FLOAT, 8, evaluation=hir.FloatEvaluation.EXTENDED80)
    values = (
        hir.Value(1, 3),
        hir.Value(2, 3),
        hir.Value(3, 2),
        hir.Value(4, 1),
        hir.Value(5, 3),
    )
    block = hir.Block(
        1,
        (
            hir.Instruction(1, hir.Op.FNEG, (2,), (hir.ValueRef(1),)),
            hir.Instruction(2, hir.Op.CONVERT, (3,), (hir.ValueRef(2),)),
            hir.Instruction(3, hir.Op.CONVERT, (5,), (hir.ValueRef(3),)),
            hir.Instruction(4, hir.Op.CONVERT, (4,), (hir.ValueRef(5),)),
        ),
        hir.Terminator(hir.TerminatorKind.RETURN),
    )
    function = hir.Function(1, "float_convert", 0, values, (), (block,), 1, parameters=(1,))
    source = hir.Program(
        hir.Dialect.VBDOS,
        hir.RuntimeProfile.VBDOS,
        (hir.Module(1, "float", (void, integer, single, double), (function,)),),
    )
    body = hir.lower(source)[0].body
    operations = body.blocks[0].ops
    assert operations[0].floating.precision is floating.Precision.EXACT
    assert [one.kind for one in operations[1:3]] == [mir.Kind.FSTORE, mir.Kind.FLOAD]
    assert all(one.kind is not mir.Kind.COPY for one in operations)
    assert operations[3].kind is mir.Kind.FSTORE
    assert lower_mir.lowered("float_convert", body, {}, set(), {}, occurrences={}).insns


def test_byref_loop_condition_reloads_the_published_pointee() -> None:
    """IN_KEYSTROKE held a released key forever after GVN kept its first read.

    A BYREF pointee is published storage: an interrupt or another runtime
    callback may change it without an ordinary source store.  Both the guard
    and the back-edge condition must therefore remain observable loads.
    """
    source = qb_driver.parsed(
        ROOT / "frontends/qb/fixtures/byreflp.bas",
        dialect="vbdos",
        runtime="vbdos",
    )
    function = next(one for one in source.modules[0].functions if one.name == "WAITKEY")
    semantic = next(one for one in hir.lower(source) if one.name.endswith("WAITKEY"))
    optimized = qb_compile.optimized(source, function, semantic)
    loads = [one for block in optimized.body.blocks for one in block.ops if one.kind is mir.Kind.LOAD]

    natural = loops.loops(optimized.body.blocks, optimized.body.entry)
    inside = {block for loop in natural for block in loop.body}

    assert len(loads) == 2
    assert all(one.volatile and any(reference.volatile for reference in one.loads) for one in loads)
    assert any(
        block.at in inside and one.kind is mir.Kind.LOAD and one.volatile
        for block in optimized.body.blocks
        for one in block.ops
    )


@pytest.mark.full
def test_identity_phi_edge_survives_control_flow_threading() -> None:
    """ENTPHI lost a dynamic-array address after its identity phi edge vanished.

    The source's `CASE 0, 1` join reaches a field through an address value
    which has the same physical register on both edges.  Its copies therefore
    emit nothing, but their virtual definition must survive until all LIR
    control-flow threading is complete.
    """
    source = qb_driver.parsed(
        ROOT / "frontends/qb/fixtures/entphi.bas",
        dialect="vbdos",
        runtime="vbdos",
        array_order="row-major",
    )
    assert qb_compile.object_bytes(source, "ENTPHI.BAS")
