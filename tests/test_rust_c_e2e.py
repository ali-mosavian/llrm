"""Rust C objects must survive Microsoft LINK and a real DOS 386."""

import shutil
import subprocess
from pathlib import Path

import pytest
from dosbox import launch
from configs import CONFIGS
from dosbox import dos_file
from dosbox import read_dos
from dosbox import dosbox_bin
from qbopt.objectfile import omf

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "fixtures" / "c" / "parity" / "scalar.cgs"
HARNESS = ROOT / "fixtures" / "c" / "parity" / "scalar-start.asm"
PARITY_SOURCE = ROOT / "fixtures" / "c" / "parity" / "parity.cgs"
PARITY_HARNESS = ROOT / "fixtures" / "c" / "parity" / "parity-start.asm"
ALGEBRA_SOURCE = ROOT / "fixtures" / "c" / "parity" / "algebra.cgs"
ALGEBRA_HARNESS = ROOT / "fixtures" / "c" / "parity" / "algebra-start.asm"
BRANCH_SOURCE = ROOT / "fixtures" / "c" / "parity" / "branch.cgs"
BRANCH_HARNESS = ROOT / "fixtures" / "c" / "parity" / "branch-start.asm"
MEMORY_SOURCE = ROOT / "fixtures" / "c" / "parity" / "memory.cgs"
MEMORY_HARNESS = ROOT / "fixtures" / "c" / "parity" / "memory-start.asm"
LOOP_SOURCE = ROOT / "fixtures" / "c" / "parity" / "loop.cgs"
LOOP_HARNESS = ROOT / "fixtures" / "c" / "parity" / "loop-start.asm"
CONTROL_SOURCE = ROOT / "fixtures" / "c" / "parity" / "control.cgs"
CONTROL_HARNESS = ROOT / "fixtures" / "c" / "parity" / "control-start.asm"
QLIGHT_SOURCE = ROOT / "fixtures" / "c" / "parity" / "qlight.cgs"
QLIGHT_HARNESS = ROOT / "fixtures" / "c" / "parity" / "qlight-start.asm"
QMOVE_SOURCE = ROOT / "fixtures" / "c" / "parity" / "qmove.cgs"
QMOVE_HARNESS = ROOT / "fixtures" / "c" / "parity" / "qmove-start.asm"
QBSP_SOURCE = ROOT / "fixtures" / "c" / "parity" / "qbsp.cgs"
QBSP_HARNESS = ROOT / "fixtures" / "c" / "parity" / "qbsp-start.asm"
CELLS_SOURCE = ROOT / "fixtures" / "c" / "cells.cgs"
CELLS_HARNESS = ROOT / "fixtures" / "c" / "cells-start.asm"
JWASM = shutil.which("jwasm") or str(Path.home() / "work/other/d32x/toolchains/native/bin/jwasm")
CFG = CONFIGS["v-g3"]

pytestmark = [
    pytest.mark.e2e,
    pytest.mark.skipif(shutil.which("cargo") is None, reason="cargo is not installed"),
    pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x"),
    pytest.mark.skipif(not Path(JWASM).is_file(), reason="jwasm is not installed"),
    pytest.mark.skipif(not CFG.available, reason="no v-g3 DOS linker toolchain"),
]


def _llrm_c() -> Path:
    binary = ROOT / "target" / "debug" / "llrm-c"
    build = subprocess.run(
        ["cargo", "build", "--quiet", "--bin", "llrm-c"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert build.returncode == 0, build.stdout + build.stderr
    assert binary.is_file()
    return binary


def _compile_and_run(
    source: Path,
    harness: Path,
    tmp_path: Path,
    *,
    timeout: int = 10,
) -> int:
    object_file = tmp_path / "PROGRAM.OBJ"
    compiled = subprocess.run(
        [str(_llrm_c()), "--emit", "obj", "-o", str(object_file), str(source)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr
    assert object_file.is_file()

    harness_file = tmp_path / "START.ASM"
    harness_file.write_bytes(harness.read_bytes())
    assembled = subprocess.run(
        [JWASM, "-q", "-c", "-Cp", "-Zg", "-omf", f"-Fo{tmp_path / 'START.OBJ'}", str(harness_file)],
        capture_output=True,
        text=True,
    )
    assert assembled.returncode == 0, assembled.stdout + assembled.stderr

    run = launch(
        tmp_path,
        CFG.mount,
        [f"{CFG.link} START.OBJ+PROGRAM.OBJ, PROGRAM.EXE,,; > LINK.OUT", "PROGRAM.EXE"],
        timeout=timeout,
    )
    assert run.finished and not run.timed_out, run
    link = read_dos(tmp_path, "LINK.OUT").lower()
    assert "error l" not in link and "unresolved external" not in link, link
    value = dos_file(tmp_path, "VALUE.BIN")
    assert value is not None
    return int.from_bytes(value.read_bytes(), "little", signed=True)


def test_rust_c_scalar_returns_the_independent_parity_answer(tmp_path: Path) -> None:
    """The first Rust C paired program must link and return scalar's 1789.

    This is the runtime milestone for the real WCC capture path: an OMF object
    emitted by ``llrm-c`` must survive JWASM's external caller, Microsoft's
    v-g3 LINK, and a DOS 386 instead of merely looking valid to host parsing.
    """
    assert _compile_and_run(SOURCE, HARNESS, tmp_path, timeout=20) == 1789


def test_rust_c_local_short_cells_returns_the_independent_argument(tmp_path: Path) -> None:
    """Python's WCC capture of local ``short cells[4]`` must return 1234.

    This exercises the newly ported local-aggregate path as a real program:
    the WCC-derived stream is compiled by ``llrm-c``, linked with a far-cdecl
    caller, and run on DOS without relying on any optimizer.
    """
    assert _compile_and_run(CELLS_SOURCE, CELLS_HARNESS, tmp_path) == 1234


def test_rust_c_aggregate_parity_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """The Rust port of Python's aggregate C path must still return 1789.

    ``tests/test_frontend_parity.py`` established this independently for the
    working Python compiler.  This is the same real WCC capture, far-cdecl
    caller, Microsoft linker, and DOS execution oracle for ``llrm-c``.
    """
    assert _compile_and_run(PARITY_SOURCE, PARITY_HARNESS, tmp_path) == 1789


def test_rust_c_algebra_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """Two far-cdecl i32 calls must retain algebra's established 702774.

    This ports the existing paired-program regression through llrm-c, the
    independent assembly caller, Microsoft LINK, and DOS execution.  It keeps
    the call-result, caller-cleanup, spill, and C callee-save work tied to the
    actual Python-era oracle rather than a Rust-only instruction pattern.
    """
    assert _compile_and_run(ALGEBRA_SOURCE, ALGEBRA_HARNESS, tmp_path, timeout=20) == 702774


def test_rust_c_branch_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """The two-arm C branch must retain the established -87904 result.

    The real WCC capture has a signed short comparison, an explicit join, and
    two far-cdecl LONG-returning calls.  Link the fresh Rust object with the
    independent caller and check the Python-era program oracle on DOS.
    """
    assert _compile_and_run(BRANCH_SOURCE, BRANCH_HARNESS, tmp_path, timeout=20) == -87904


def test_rust_c_memory_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """By-reference i16 mutation and LONG square must retain 361001.

    The real WCC capture stores through an incoming near pointer, reloads that
    cell twice as a LONG, and keeps the two far-cdecl calls on the DOS path.
    """
    assert _compile_and_run(MEMORY_SOURCE, MEMORY_HARNESS, tmp_path, timeout=20) == 361001


def test_rust_c_loop_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """A backedge and loop-carried LONG total must retain 130991.

    The real WCC capture combines a local INTEGER induction value, signed exit
    comparison, loop-carried LONG arithmetic, and two far-cdecl calls.
    """
    assert _compile_and_run(LOOP_SOURCE, LOOP_HARNESS, tmp_path, timeout=20) == 130991


def test_rust_c_control_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """Nested i16 parity control and LONG updates must retain 15007.

    The real WCC capture combines an i16 AND/equality branch inside a signed
    loop with the paired long add/sub updates and far-cdecl calls.
    """
    assert _compile_and_run(CONTROL_SOURCE, CONTROL_HARNESS, tmp_path, timeout=20) == 15007


def test_rust_c_qlight_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """By-value i16 calls and signed LONG division must retain 200100255."""
    assert _compile_and_run(QLIGHT_SOURCE, QLIGHT_HARNESS, tmp_path, timeout=20) == 200100255


def test_rust_c_qmove_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """Python's qmove C parity capture must retain the runtime answer 100405.

    This pairs the exact WCC capture with a far-cdecl ``_quake_move_demo``
    caller, Microsoft LINK, and DOS execution so the Rust port retains the
    existing Python qmove oracle rather than merely accepting floating-point
    machine IR.
    """
    assert _compile_and_run(QMOVE_SOURCE, QMOVE_HARNESS, tmp_path, timeout=20) == 100405

    records = omf.read(tmp_path / "PROGRAM.OBJ")
    code = b"".join(payload for _, segment, _, payload in omf.ledata(records) if segment == 1)
    assert b"\xd9\x46\x0a" in code
    assert b"\xd8\x4e\x0a" in code and b"\xd8\x4e\x0e" in code
    assert b"\xdb\x7e" not in code, (
        "Python keeps qmove's C float formals in their incoming binary32 cells; "
        "the Rust port must not bridge entry x87 values through m80 frame homes"
    )


def test_rust_c_qbsp_returns_the_existing_python_oracle(tmp_path: Path) -> None:
    """The QBSP C capture must link and return the established 120 result.

    This pairs Python's canonical WCC stream for ``bench/parity/qbsp.c`` with
    the existing far-cdecl DOS harness shape.  Its 120 oracle covers the
    float-returning BSP plane helper, signed branch selection, pointer-based
    aggregate access, and final LONG arithmetic without inventing a Rust-only
    expected value.
    """
    assert _compile_and_run(QBSP_SOURCE, QBSP_HARNESS, tmp_path, timeout=20) == 120


def test_rust_c_qmove_folds_constant_vector_field_addresses_before_allocation(tmp_path: Path) -> None:
    """QMOVE must retain Python's selected ``[base]``/``[base+4]`` float cells.

    Python's ``qbopt.backend.addressforms:offsets`` and ``:selected`` removed
    QMOVE's old ``copy; add 0/4`` pointer spelling when its only consumer was
    a vector-field memory cell.  The general rule is ownership: a constant
    address adjustment used solely by an encodable memory operand belongs to
    that operand, not to a separate integer instruction.  This inspects the
    initial selected QMir, before allocation can obscure the source address.
    """
    selected = tmp_path / "qmove.qmir"
    compiled = subprocess.run(
        [str(_llrm_c()), "--emit", "qmir", "-o", str(selected), str(QMOVE_SOURCE)],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    assert compiled.returncode == 0, compiled.stdout + compiled.stderr

    lines = selected.read_text().splitlines()
    procedure = next(
        index
        for index, line in enumerate(lines)
        if line.startswith("function ") and "5f706c5f67726f756e645f616363656c" in line
    )
    end = lines.index("endfunction", procedure)
    body = lines[procedure:end]

    # `.qmir` is intentionally line-oriented.  Keep these grammar positions
    # named so this acceptance test does not silently turn into a raw-string
    # proxy when the text format changes.
    INST_OPCODE = 2
    INST_OPERAND_COUNT = 4
    OPERAND_KIND = 4
    OPERAND_VALUE = 5
    OPCODE_COPY = 1
    OPCODE_ADD = 5
    OPCODE_X87_LOAD = 38

    def instructions() -> list[tuple[int, list[list[str]]]]:
        parsed = []
        cursor = 0
        while cursor < len(body):
            fields = body[cursor].split()
            if not fields or fields[0] != "inst":
                cursor += 1
                continue
            operand_count = int(fields[INST_OPERAND_COUNT])
            operands = [body[cursor + offset].split() for offset in range(1, operand_count + 1)]
            parsed.append((int(fields[INST_OPCODE]), operands))
            cursor += operand_count + 1
        return parsed

    selected_instructions = instructions()

    def register(operand: list[str]) -> int | None:
        return int(operand[OPERAND_VALUE]) if operand[OPERAND_KIND] == "vreg" else None

    def immediate(operand: list[str]) -> int | None:
        return int(operand[OPERAND_VALUE]) if operand[OPERAND_KIND] == "imm" else None

    # A folded X87Load has a bare ``vreg`` address for offset zero and a
    # ``vreg, imm`` address for a non-zero displacement.  Both forms are
    # required: the bare form catches the historical redundant add-zero;
    # +4 catches the second float of each vector.
    folded_offsets = set()
    for opcode, operands in selected_instructions:
        if opcode != OPCODE_X87_LOAD or len(operands) not in {3, 4}:
            continue
        if register(operands[2]) is None:
            continue
        if len(operands) == 3:
            folded_offsets.add(0)
        elif immediate(operands[3]) == 4:
            folded_offsets.add(4)
    assert folded_offsets == {0, 4}

    # Do not prohibit ADD generally: qmove's integer result deliberately
    # contains arithmetic.  Reject only a Copy -> Add(0/4) chain whose result
    # immediately supplies an x87 field load, the exact stale address form
    # that Python folds into the memory operand above.
    for first, second, third in zip(
        selected_instructions,
        selected_instructions[1:],
        selected_instructions[2:],
    ):
        first_opcode, first_operands = first
        second_opcode, second_operands = second
        third_opcode, third_operands = third
        if (first_opcode, second_opcode, third_opcode) != (OPCODE_COPY, OPCODE_ADD, OPCODE_X87_LOAD):
            continue
        if len(first_operands) != 2 or len(second_operands) != 2 or len(third_operands) != 3:
            continue
        copied = register(first_operands[0])
        addend = immediate(second_operands[1])
        if (
            copied is not None
            and copied == register(second_operands[0])
            and addend in {0, 4}
            and copied == register(third_operands[2])
        ):
            pytest.fail(
                "qmove retained Copy -> Add(0/4) solely to address an x87 vector field; "
                "fold the constant displacement into that memory operand"
            )
