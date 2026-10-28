"""classify_isel puts every FAIL line in one cause row, and refuses what no rule names."""

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import classify_isel  # noqa: E402

LOG = """\
stamp x
FAIL  c/a.c [-O2 -m32]: compile: llrm-c: @f_: --isel=dag: CannotSelect("MergeValues")
FAIL  c/b.c [-O2 -m32]: compile: llrm-c: @g_: --isel=dag: CannotSelect("MergeValues")
FAIL  c/c.c [-Os -m32]: compile: llrm-c: @h_: --isel=dag: Emit(Unsupported("frame index -2"))
FAIL  c/d.c: link: llrm-c: llrm-c: @i_: --isel=dag: Emit(Unsupported("frame index 6"))
FAIL  c/e.c [-O2 -m32]: line 2: want '12', got '21'
FAIL  c/f.c [-O2 -m32]: link: jwlink: Error! E2028: div64_s32 is an undefined reference
FAIL  c/g.c [-O2 -m32]: compile: llrm-c: @j_: --isel=dag: some new builder refusal
449 programs: 442 pass, 0 known, 7 fail
"""


def test_every_failure_is_in_exactly_one_row_and_the_rows_sum():
    rows, unclassified = classify_isel.classify(LOG)
    assert unclassified == []
    assert sum(len(programs) for programs in rows.values()) == 7
    assert rows[("unselected node MergeValues", "sdag-c")] == ["c/a.c [-O2 -m32]", "c/b.c [-O2 -m32]"]
    assert len(rows[("emit: frame index N", "sdag-d")]) == 2
    assert rows[("wrong output", "triage")] == ["c/e.c [-O2 -m32]"]
    assert ("builder refusal: some new builder refusal", "sdag-a") in rows
    assert classify_isel.table(LOG).splitlines()[-1] == "| total | | 7 | |"


def test_a_failure_no_rule_names_is_refused():
    with pytest.raises(SystemExit, match="unclassified"):
        classify_isel.table(LOG + "FAIL  c/z.c: something nobody anticipated\n")


M16 = """\
FAIL  c/a.c [-O0]: compile: thread 'main' (1) panicked at crates/codegen/llrm-codegen/src/backend/selectiondag/dag.rs:570:9:
integer constant of Simple(Far)
note: run with `RUST_BACKTRACE=1`
FAIL  c/b.c [-O2]: compile: llrm-c: _main: Semantics(op=<Operation.CALL: 'call'>, name='call', dests=(), sources=(Imm(value=0, width=2, address=[seg:0+0x0]),), target=None, indirect=True)
FAIL  c/c.c [-O0]: compile: llrm-c: isel: 0x000a names 25 at width 4, which is not the width that register is
FAIL  c/d.c [-O0]: compile: llrm-c: isel: value#28 is read but never defined or supplied by the caller
FAIL  c/e.c [-O0]: compile: llrm-c: @_main: _main: the greedy allocator could not place every range
FAIL  c/f.c [-O0]: compile: llrm-c: @_main: floatalloc: _main reads edx before every path writes it, and the entry does not deliver it
"""


def test_the_real_mode_failures_have_causes_and_owners():
    rows, unclassified = classify_isel.classify(M16)
    assert unclassified == []
    assert sum(len(programs) for programs in rows.values()) == 6
    assert ("panic in selectiondag/dag.rs: integer constant of Simple(Far)", "sdag-a") in rows
    assert ("lir: a direct call is emitted as an indirect call of an immediate", "sdag-d") in rows
    assert ("floatalloc: a register read before any path writes it", "sdag-d") in rows


def test_ssa_and_address_verify_failures_have_causes():
    log = (
        "FAIL  c/a.c [-O0]: compile: llrm-c: isel: value#44 is defined 2 times in a body that should be in SSA\n"
        "FAIL  c/b.c [-O0]: compile: llrm-c: _main: Semantics(op=<Operation.ADDRESS: 'addr'>, name='lea', dests=())\n"
    )
    rows, unclassified = classify_isel.classify(log)
    assert unclassified == [] and len(rows) == 2


def test_a_value_read_before_it_is_defined_is_an_emit_row_of_sdag_d():
    """The check lives in the emitter (dag_function.rs), not the builder: 39 programs of run 13 were put in sdag-a's rows."""
    log = 'FAIL  c/a.c [-O2]: compile: llrm-c: @_f: --isel=dag: --isel=dag: value 59 is read and not defined on every path (Some("mov"))\n'
    rows, unclassified = classify_isel.classify(log)
    assert unclassified == []
    assert list(rows) == [('emit: a value read before it is defined on every path (Some("mov"))', "sdag-d")]
