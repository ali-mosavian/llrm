"""Paired source programs must survive either frontend and share one oracle."""

import sys
import json
import shutil
import subprocess
from pathlib import Path
from collections import Counter

import pytest

from qbopt import wholeseg
from qbopt.objectfile import omf
from qbopt.backend import omfwrite
from qbopt.objectfile import module
from qbopt.cfront import compile as cfront

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "bench" / "parity"
FIXTURE = ROOT / "fixtures" / "parity" / "parity-v-g3.obj"
SCALAR_FIXTURE = ROOT / "fixtures" / "parity" / "scalar-v-g3.obj"
LOOP_FIXTURE = ROOT / "fixtures" / "parity" / "loop-v-g3.obj"
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")


def _expected() -> int:
    return json.loads((SOURCE / "expected.json").read_text())["parity"]


def _expected_for(name: str) -> int:
    return json.loads((SOURCE / "expected.json").read_text())[name]


def test_both_frontends_reach_the_shared_optimizer_and_fresh_omf_writer() -> None:
    """BC aggregate code once refused allocation while equivalent C emitted.

    This is the bounded host-side parity gate: both independently raised
    programs must finish the shared optimization, allocation, layout, and
    fresh OMF path.  Runtime tests below bind both objects to the same answer.
    """
    basic = wholeseg.emitted(FIXTURE.read_bytes())
    assert basic.outcome is wholeseg.Emission.LIR, basic.reason
    basic_module = module.of(omf.parse(basic.data))
    assert basic_module is not None and basic_module.code
    assert "B$MUI4" not in basic_module.calls.values()

    source = SOURCE / "parity.c"
    c_module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    c_object = omfwrite.written(c_module, source.name)
    assert module.of(omf.parse(c_object)).code


def test_basic_aggregate_drops_unobserved_dynamic_array_contents() -> None:
    """PARITY returned 1789 but still initialized all 16 erased array fields.

    Exact SROA proves the loads are constants.  The owning allocation never
    escapes and B$ERAS discards it, so retaining stores to its unread contents
    is a frontend-dependent code-generation difference, not BASIC semantics.
    """
    import corpus
    from qbopt.model import mir
    from qbopt.optimize import transform

    found = corpus.loaded(FIXTURE)
    partitioned = corpus.partitioned(FIXTURE)
    raised = mir.bodies(found, partitioned)
    _name, body = next((name, body) for name, body in raised if "PARITYKERNEL" in name)
    optimized = transform.applied(body, found.dgroup, found.calls, blocks=partitioned, found=found)

    assert not [
        ref
        for block in optimized.blocks
        for op in block.ops
        for ref in (*op.loads, *op.stores)
        if ref.allocation is not None
    ]


def test_c_aggregate_exact_nonzero_loops_leave_only_the_constant_result() -> None:
    """C PARITY still compared ``base`` with ``base + 32`` after peeling.

    Both loops have the independently proven exact trip count eight.  A full
    expansion must not retain the original zero-trip guard, its address
    calculations, or the now-private array traffic.
    """
    from qbopt.model import ir

    source = SOURCE / "parity.c"
    built = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    procedure = next(one for one in built.procedures if one.name == "_parity_kernel")
    real = [
        one.what
        for block in procedure.body.blocks
        for one in block.insns
        if one.what is not None and one.what.op not in {ir.Operation.NOTHING, ir.Operation.RETURN}
    ]

    assert [one.name for one in real] == ["mov", "mov"]
    assert [one.sources for one in real] == [(ir.Imm(1789, 2),), (ir.Imm(0, 2),)]


def test_basic_scalar_frontend_does_not_preserve_runtime_scratch_as_program_data() -> None:
    """SCALAR kept its eight-iteration loop and B$MUI4 while C returned 1789.

    VBDOS B$EXSA has conservative survivor inputs for hidden error-transfer
    paths. Those machine-only values must not make B$MUI4's BX/CX clobbers
    semantic on the ordinary function-return edge.
    """
    import corpus
    from qbopt.model import mir
    from qbopt.optimize import transform

    found = corpus.loaded(SCALAR_FIXTURE)
    partitioned = corpus.partitioned(SCALAR_FIXTURE)
    raised = mir.bodies(found, partitioned)
    _name, body = next((name, body) for name, body in raised if "PARITYSCALAR" in name)
    assert any(
        op.kind is mir.Kind.MUL and op.results and op.results[0].width == 4 for block in body.blocks for op in block.ops
    )
    assert not any(
        op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$MUI4" for block in body.blocks for op in block.ops
    )

    optimized = transform.applied(body, found.dgroup, found.calls, blocks=partitioned, found=found)
    assert not any(op.kind is mir.Kind.MUL for block in optimized.blocks for op in block.ops)
    assert not any(
        op.kind is mir.Kind.CALL and found.calls.get(op.at) == "B$MUI4"
        for block in optimized.blocks
        for op in block.ops
    )
    assert not any(len(block.succ) > 1 for block in optimized.blocks)
    constants = {
        (arg.n, arg.width)
        for block in optimized.blocks
        for op in block.ops
        for arg in op.args
        if op.kind is mir.Kind.COPY and isinstance(arg, mir.Const)
    }
    expected = _expected_for("scalar")
    assert (expected & 0xFFFF, 2) in constants
    assert ((expected >> 16) & 0xFFFF, 2) in constants


def test_scalar_frontends_emit_the_same_machine_core() -> None:
    """Equivalent scalar source must converge beyond merely returning 1789.

    BASIC's frame entry/exit calls are explicit language ABI scaffolding.
    Between them, its final allocated machine operations must be identical to
    C's complete function body; loops, helper calls, wider moves, spills, and
    recomputation are not normalized away.
    """
    import corpus
    from qbopt.model import ir

    found = corpus.loaded(SCALAR_FIXTURE)
    basic_bodies = {}

    def basic_watch(stage, name, body):
        if stage == "jumps":
            basic_bodies[name] = body

    basic = wholeseg.emitted(SCALAR_FIXTURE.read_bytes(), watch=basic_watch)
    assert basic.outcome is wholeseg.Emission.LIR, basic.reason
    basic_body = next(body for name, body in basic_bodies.items() if "PARITYSCALAR" in name)
    basic_real = [
        one
        for block in basic_body.blocks
        for one in block.insns
        if one.what is not None and one.what.op is not ir.Operation.NOTHING
    ]
    entry = next(
        index
        for index, one in enumerate(basic_real)
        if one.what.op is ir.Operation.CALL and one.op is not None and found.calls.get(one.op.at) == "B$ENRA"
    )
    leave = next(
        index
        for index, one in enumerate(basic_real)
        if one.what.op is ir.Operation.CALL and one.op is not None and found.calls.get(one.op.at) == "B$EXSA"
    )
    basic_core = [one.what for one in basic_real[entry + 1 : leave]]
    assert sum(one.what.op is ir.Operation.RETURN for one in basic_real) == 1

    c_bodies = {}

    def c_watch(stage, name, body):
        if stage == "lir-jumps":
            c_bodies[name] = body

    source = SOURCE / "scalar.c"
    cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True, watch=c_watch)
    c_body = next(iter(c_bodies.values()))
    c_core = [
        one.what
        for block in c_body.blocks
        for one in block.insns
        if one.what is not None and one.what.op not in {ir.Operation.NOTHING, ir.Operation.RETURN}
    ]
    assert (
        sum(
            one.what is not None and one.what.op is ir.Operation.RETURN
            for block in c_body.blocks
            for one in block.insns
        )
        == 1
    )

    assert basic_core == c_core


def test_paired_frontends_converge_on_the_same_final_machine_work() -> None:
    """Equivalent frontends once left helpers, spills, ADC pairs and reloads.

    Compare the raw final allocated listings rather than a score that can hide
    work.  Register choice and independent parameter order may differ.  LOOP's
    opposite branch orientation expresses the same recurrence, while CONTROL
    retains BASIC's inclusive ``FOR`` bound adjustment and a different valid
    association of its odd arm.  Every other paired instruction family must
    agree exactly, including the one-instruction DX:AX high extraction.
    """
    from tools.frontend_parity import pair

    def mnemonics(lines):
        return tuple(line.split()[0] for _at, line in lines)

    def work(lines, *, loop_branch=False):
        names = mnemonics(lines)
        if loop_branch:
            names = tuple("loop-jcc" if one in {"jg", "jl"} else one for one in names)
        return Counter(names)

    listings = {name: pair(name) for name in ("scalar", "algebra", "branch", "memory", "loop", "control")}
    forbidden = {"adc", "sbb", "push", "pop", "call"}
    for name, (basic, c) in listings.items():
        assert not (set(mnemonics(basic)) | set(mnemonics(c))) & forbidden, name
        assert len(basic) == len(c), name
        assert mnemonics(basic).count("shld") == mnemonics(c).count("shld") == (name != "scalar"), name

    for name in ("scalar", "algebra", "branch", "memory"):
        basic, c = listings[name]
        assert work(basic) == work(c), name
    assert mnemonics(listings["branch"][0]) == mnemonics(listings["branch"][1])
    assert mnemonics(listings["memory"][0]) == mnemonics(listings["memory"][1])

    basic_loop, c_loop = listings["loop"]
    assert work(basic_loop, loop_branch=True) == work(c_loop, loop_branch=True)

    basic_control, c_control = listings["control"]
    # Source-language semantics explain the complete remaining opcode delta:
    # BASIC decrements an inclusive FOR bound and branches <=; C branches <.
    # Their odd-arm additions/subtractions are algebraically associated in
    # opposite directions, with one C move replacing BASIC's extra add.
    assert work(basic_control) - work(c_control) == Counter({"dec": 1, "add": 1, "jle": 1})
    assert work(c_control) - work(basic_control) == Counter({"mov": 1, "sub": 1, "jl": 1})


def test_basic_quake_float_comparisons_do_not_retain_runtime_helper_calls() -> None:
    """QMOVE retained two B$FCMP calls where equivalent C emitted fcompp.

    B$FCMP's established contract is exactly an x87 comparison and status
    transfer. It must cross the raise boundary as the same FCOMPARE operation
    a source frontend produces, not remain opaque because BC spelt it as a
    runtime helper.
    """
    from qbopt.model import ir

    fixture = ROOT / "fixtures" / "parity" / "qmove-v-g3.obj"
    found = module.of(omf.read(fixture))
    assert found is not None
    seen = {}

    def watch(stage, name, body):
        if stage == "jumps" and name == "procedure PLGROUNDACCEL":
            seen[name] = body

    result = wholeseg.emitted(fixture.read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    instructions = [one for block in seen["procedure PLGROUNDACCEL"].blocks for one in block.insns]
    assert not [
        one
        for one in instructions
        if one.what is not None
        and one.what.op is ir.Operation.CALL
        and one.op is not None
        and found.calls.get(one.op.at) == "B$FCMP"
    ]
    assert sum(one.what is not None and one.what.name in {"fcom", "fcomp", "fcompp"} for one in instructions) == 2


def test_quake_move_constant_field_offsets_do_not_survive_the_memory_fold() -> None:
    """QMOVE selected every second-vector-field access as ``[base+4]`` but
    retained BASIC's now-unused ``mov ax,si / add ax,4`` address spelling.

    A whole-word congruence hint is not a semantic partial write.  Once the
    selected cells own the address computation, no frontend-specific integer
    address work may remain in this floating-point kernel.
    """
    from tools.frontend_parity import pair

    basic, c = pair("qmove")
    assert not [line for _at, line in basic if line.startswith(("add ", "lea "))]
    assert not [line for _at, line in c if line.startswith(("add ", "lea "))]


def test_quake_light_integer_kernel_converges_to_the_same_machine_work() -> None:
    """QLIGHT is a real qc-port clamp/scale kernel, not a synthetic identity.

    CodeView proves that this INTEGER function returns only AX, so no dead
    DX:AX extraction is language scaffolding.  Zeroing has two equally cheap
    spellings; apart from that, the complete selected opcode stream must be
    frontend independent.
    """
    from tools.frontend_parity import pair

    def work(lines):
        out = []
        for _at, line in lines:
            name = line.split()[0]
            if line in {"xor eax, eax", "mov ax, 0"}:
                name = "zero"
            out.append(name)
        return tuple(out)

    basic, c = pair("qlight")
    assert not [line for _at, line in basic if line.startswith("shld ")]
    assert work(basic) == work(c)


def test_quake_bsp_integer_index_does_not_preserve_a_dead_long_high_half() -> None:
    """RPOINTLEAF's CodeView signature returns INTEGER, but generic BASIC
    exit liveness exposed DX as though every procedure returned LONG.  Calls
    also discarded their exact hidden-result/copy ranges, and outgoing ARG
    traffic was allowed to alias the active frame.  Together those facts kept
    ``nodenr * 6`` as a memory reload and two-result IMUL where the C frontend
    strength-reduced the same array index.
    """
    from tools.frontend_parity import pair

    basic, _c = pair("qbsp")
    assert not [line for _at, line in basic if line.startswith("imul ")]
    assert not [line for _at, line in basic if line.startswith(("add word ptr [bp-", "shl word ptr [bp-"))]


def test_runtime_frame_is_established_before_allocator_spill_accesses() -> None:
    """Optimized frontend-parity LOOP printed 5000 instead of 130991.

    Its zero loop seed was joined to a phi destination.  Allocation expanded
    that spill web only after considering rematerialization, stored the zero
    through ``BP`` before B$ENRA established the callee frame, and reloaded an
    unrelated caller-frame word afterwards.  Spill-web expansion must expose
    cheap constants to rematerialization before any frame slot is committed.
    """
    import corpus

    found = corpus.loaded(LOOP_FIXTURE)
    seen = {}

    def watch(stage, name, body):
        if stage == "regalloc" and name == "procedure PARITYLOOP":
            seen[stage] = body

    result = wholeseg.emitted(LOOP_FIXTURE.read_bytes(), watch=watch)
    assert result.outcome is wholeseg.Emission.LIR, result.reason
    body = seen["regalloc"]
    entry = next(block for block in body.blocks if block.at == body.entry)
    runtime_entry = next(
        index for index, one in enumerate(entry.insns) if one.op is not None and found.calls.get(one.op.at) == "B$ENRA"
    )
    assert not any(one.spill_store or one.spill_reload for one in entry.insns[:runtime_entry])


def _c_start(symbol: str) -> str:
    return f"""\
.model medium
.386
extrn _{symbol}:far
.data
value dd ?
filename db 'VALUE.BIN', 0
stack_space db 1024 dup (?)
stack_top label byte
.code
start:
    mov ax, @data
    mov ds, ax
    ; Near data pointers address stack locals through DS in this ABI.  The
    ; old standalone harness left DOS's separate startup stack in SS, so a
    ; callee wrote through the wrong segment and QMOVE returned 30405.
    cli
    mov ss, ax
    mov sp, offset stack_top
    sti
    ; A real C runtime also initializes the x87 before calling user code.
    fninit
    call far ptr _{symbol}
    mov word ptr value, ax
    mov word ptr value+2, dx
    mov ah, 3ch
    xor cx, cx
    lea dx, filename
    int 21h
    jc failed
    mov bx, ax
    mov ah, 40h
    mov cx, 4
    lea dx, value
    int 21h
    jc failed
    xor al, al
    jmp finished
failed:
    mov al, 1
finished:
    mov ah, 4ch
    int 21h
end start
"""


def _runtime_available() -> bool:
    sys.path.insert(0, str(ROOT / "tools"))
    from configs import CONFIGS
    from dosbox import dosbox_bin

    return dosbox_bin() is not None and Path(JWASM).is_file() and CONFIGS["v-g3"].available


@pytest.mark.e2e
@pytest.mark.skipif(not _runtime_available(), reason="DOSBox or the DOS toolchains are unavailable")
def test_basic_frontend_returns_the_independent_parity_answer(tmp_path: Path) -> None:
    """The optimized BC object must print the same independent result as C."""
    from tools import e2e

    result = e2e.run(
        "v-g3",
        names=[
            "parity",
            "scalar",
            "algebra",
            "branch",
            "loop",
            "memory",
            "control",
            "qmove",
            "qbsp",
            "qlight",
        ],
        source_dir=SOURCE,
        golden_dir=SOURCE / "golden",
        work=tmp_path,
        timeout=30,
    )
    assert result.ok, result.verdicts
    assert _expected() == _expected_for("scalar") == 1789


@pytest.mark.parametrize(
    "name,symbol",
    [
        ("parity", "parity_kernel"),
        ("scalar", "parity_scalar"),
        ("algebra", "parity_algebra_demo"),
        ("branch", "parity_branch_demo"),
        ("loop", "parity_loop_demo"),
        ("memory", "parity_memory_demo"),
        ("control", "parity_control_demo"),
        ("qmove", "quake_move_demo"),
        ("qbsp", "quake_bsp_demo"),
        ("qlight", "quake_light_demo"),
    ],
)
@pytest.mark.e2e
@pytest.mark.skipif(not _runtime_available(), reason="DOSBox or the DOS toolchains are unavailable")
def test_c_frontend_returns_the_independent_parity_answer(tmp_path: Path, name: str, symbol: str) -> None:
    """The C frontend's fresh object must return the BASIC corpus oracle."""
    from dosbox import launch
    from configs import CONFIGS

    source = SOURCE / f"{name}.c"
    c_module = cfront.assembled(cfront.recorded(source, []), source.stem, optimise=True)
    (tmp_path / "PARITY.OBJ").write_bytes(omfwrite.written(c_module, source.name))
    start = tmp_path / "START.ASM"
    start.write_text(_c_start(symbol))
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(start)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr

    cfg = CONFIGS["v-g3"]
    run = launch(
        tmp_path,
        cfg.mount,
        [f"{cfg.link} START.OBJ+PARITY.OBJ, CPARITY.EXE,,; > LINK.OUT", "CPARITY.EXE"],
        timeout=20,
    )
    assert run.finished and not run.timed_out, run
    link = (tmp_path / "LINK.OUT").read_text(errors="replace").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    value = next((tmp_path / name for name in ("VALUE.BIN", "value.bin") if (tmp_path / name).is_file()), None)
    assert value is not None
    assert int.from_bytes(value.read_bytes(), "little", signed=True) == _expected_for(name)
