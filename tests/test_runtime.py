from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt import runtime
from qbopt.calls import ARITY
from qbopt.runtime import Reg
from qbopt.calls import DIVIDE
from qbopt.calls import COMPARE
from qbopt.runtime import EVERY
from qbopt.calls import MULTIPLY
from qbopt.runtime import Memory
from qbopt.calls import REMAINDER
from qbopt.runtime import Control
from qbopt.blocks import INLINE_TABLE

NAMES = sorted(runtime.CONTRACTS)

# The four calls.py already absorbs, and what its own _arity() says each takes.
ABSORBED = [COMPARE, MULTIPLY, DIVIDE, REMAINDER]


@pytest.fixture(params=NAMES)
def routine(request: pytest.FixtureRequest) -> runtime.Contract:
    return runtime.CONTRACTS[request.param]


def test_unknown_name_is_the_worst_case() -> None:
    unknown = runtime.contract("B$NOSUCHTHING")
    assert unknown == runtime.worst("B$NOSUCHTHING")
    assert not unknown.established
    assert runtime.barrier(unknown)


def test_an_unnamed_call_is_the_worst_case() -> None:
    assert runtime.contract(None).clobbers == EVERY


def test_the_worst_case_concedes_nothing() -> None:
    blank = runtime.worst("")
    assert blank.writes is Memory.ANY
    assert blank.reads is Memory.ANY
    assert blank.clobbers == EVERY
    assert blank.control is Control.UNKNOWN
    assert blank.cleanup is None
    assert blank.enters_user_code
    assert runtime.writes_caller_memory(blank)
    assert runtime.preserves(blank) == frozenset()


def test_no_contract_claims_more_than_the_worst_case(routine: runtime.Contract) -> None:
    blank = runtime.worst(routine.name)
    assert routine.writes <= blank.writes
    assert routine.reads <= blank.reads
    assert routine.clobbers <= blank.clobbers
    assert routine.enters_user_code <= blank.enters_user_code


# An entry that says it established nothing has to concede everything, whatever
# its citation says it looked at and failed to find.
@pytest.mark.parametrize("name", [name for name in NAMES if not runtime.CONTRACTS[name].established])
def test_an_unestablished_routine_is_left_at_the_worst_case(name: str) -> None:
    routine = runtime.CONTRACTS[name]
    assert routine == replace(runtime.worst(name), evidence=routine.evidence)


@pytest.mark.parametrize("name", [name for name in NAMES if runtime.CONTRACTS[name] != runtime.worst(name)])
def test_every_claim_below_the_worst_case_cites_something(name: str) -> None:
    evidence = runtime.CONTRACTS[name].evidence
    assert any(cited in evidence for cited in (".asm", ".inc", ".py", "AGENTS.md"))


def test_preserved_and_clobbered_partition_the_registers(routine: runtime.Contract) -> None:
    assert runtime.preserves(routine) | routine.clobbers == EVERY
    assert runtime.preserves(routine) & routine.clobbers == frozenset()


# The field exists to record a header that is wrong about its own body, so an
# entry carrying one whose set matches is a field that has stopped saying that.
@pytest.mark.parametrize("name", [name for name in NAMES if runtime.CONTRACTS[name].documented is not None])
def test_a_documented_clobber_set_is_recorded_only_where_it_differs(name: str) -> None:
    routine = runtime.CONTRACTS[name]
    assert routine.documented != routine.clobbers - {Reg.FLAGS}


def test_the_table_is_keyed_by_its_own_names(routine: runtime.Contract) -> None:
    assert runtime.CONTRACTS[routine.name] is routine


# blocks.py owns the fact that B$OGTA reads its own return address to find an
# inline jump table; runtime.py reflects it rather than restating it, so this
# asserts the reflection happened, not that two tables agree.
def test_the_inline_table_routines_come_from_blocks() -> None:
    reflected = {name for name, routine in runtime.CONTRACTS.items() if routine.control is Control.INLINE_TABLE}
    assert reflected == INLINE_TABLE


@pytest.mark.parametrize("name", ["B$EVCK", "B$OEGA", "B$RESN", "B$FERR"])
def test_anything_touching_user_code_is_refused(name: str) -> None:
    assert runtime.barrier(runtime.CONTRACTS[name])


@pytest.mark.parametrize("name", ["B$EVCK", "B$OEGA", "B$RESN"])
def test_the_dispatchers_can_do_anything(name: str) -> None:
    dispatcher = runtime.CONTRACTS[name]
    assert dispatcher.enters_user_code
    assert dispatcher.writes is Memory.ANY
    assert dispatcher.reads is Memory.ANY
    assert dispatcher.clobbers == EVERY


# B$FERR is two instructions and cannot itself reach a handler; what it proves
# is that the module has ON ERROR, which is the property that refuses the body.
def test_the_err_function_refuses_the_body_without_claiming_to_dispatch() -> None:
    err = runtime.CONTRACTS["B$FERR"]
    assert not err.enters_user_code
    assert err.error_handling
    assert runtime.barrier(err)


@pytest.mark.parametrize("name", ["B$CENP", "B$CEND", "B$RESN"])
def test_the_routines_that_do_not_come_back_say_so(name: str) -> None:
    assert runtime.CONTRACTS[name].control is Control.NEVER


# Two independent readings of the same fact: calls.py counted long arguments off
# BC's own emitted call sites, this counted parameter bytes off cProc's own parm
# declarations in rt/helpi4.asm. They have to land on the same number.
@pytest.mark.parametrize("name", ABSORBED)
def test_cleanup_agrees_with_the_arity_calls_py_measured(name: str) -> None:
    assert runtime.CONTRACTS[name].cleanup == 4 * ARITY.get(name, 2)


@pytest.mark.parametrize("name", ABSORBED)
def test_the_absorbed_four_touch_no_caller_memory(name: str) -> None:
    absorbed = runtime.CONTRACTS[name]
    assert not runtime.writes_caller_memory(absorbed)
    assert absorbed.reads <= Memory.ARGUMENTS
    assert absorbed.control is Control.RETURNS
    assert not absorbed.enters_user_code
    assert absorbed.clobbers <= {Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.FLAGS}


# calls.py absorbs a compare into a bare cmp precisely because a real call
# changes no register at all -- its header's "Uses: ax,cx,dx,bx" overstates it.
def test_compare_clobbers_only_the_flags_it_returns_in() -> None:
    compare = runtime.CONTRACTS[COMPARE]
    assert compare.clobbers == {Reg.FLAGS}
    assert compare.documented == {Reg.AX, Reg.BX, Reg.CX, Reg.DX}


FIXTURES = Path(__file__).resolve().parents[1] / "fixtures" / "omf"


def _runtime_targets() -> set[str]:
    named: set[str] = set()
    for path in sorted(FIXTURES.glob("*.obj")):
        found = corpus.loaded(path)
        if found is not None:
            named |= {name for name in found.calls.values() if name.startswith("B$")}
    return named


@pytest.mark.parametrize("name", sorted(_runtime_targets()))
def test_every_runtime_routine_the_corpus_calls_has_an_entry(name: str) -> None:
    assert runtime.contract(name).established


# 87bhelp.asm's six, read out of the shipped libraries by tools/libdump.py
# because the math library is not in the QuickBASIC 4.5 source drop.
X87 = ("B$FCMP", "B$FILD", "B$FIL2", "B$FIST", "B$FIS2", "B$FUST")


@pytest.mark.parametrize("name", X87)
def test_the_x87_helpers_keep_the_index_registers(name: str) -> None:
    """None of the six names si, di, bx or cx anywhere in its body.

    This is what the contracts buy: a value held in one of those survives a
    float conversion, where the worst case they had before said it did not.
    """
    kept = runtime.preserves(runtime.CONTRACTS[name])
    assert {runtime.Reg.SI, runtime.Reg.DI, runtime.Reg.BX, runtime.Reg.CX} <= kept


@pytest.mark.parametrize("name", X87)
def test_the_x87_helpers_touch_no_caller_memory(name: str) -> None:
    """Each keeps its own frame and writes only scratch below sp. B$FCMP's
    fnstsw writes one word of DGROUP -- the runtime's own, not anything the
    caller can name, exactly as B$DSEG does."""
    assert not runtime.writes_caller_memory(runtime.CONTRACTS[name])
    assert not runtime.barrier(runtime.CONTRACTS[name])


def test_the_two_forms_differ_only_by_the_sign_extension() -> None:
    """B$FIL2 is one `cwd` in front of B$FILD, and falls through into it.

    So it is the INTEGER form and clobbers dx where the LONG form does not
    -- the one register difference between them, and the reason both are
    here rather than one standing for both.
    """
    long_form = runtime.CONTRACTS["B$FILD"]
    int_form = runtime.CONTRACTS["B$FIL2"]
    assert runtime.Reg.DX not in long_form.clobbers
    assert runtime.Reg.DX in int_form.clobbers
    assert int_form.clobbers - {runtime.Reg.DX} == long_form.clobbers


def test_the_result_registers_are_the_clobbered_ones() -> None:
    """A conversion out of the x87 stack returns through dx:ax or ax, and
    that is exactly what each one is recorded as changing."""
    assert runtime.Reg.AX in runtime.CONTRACTS["B$FIST"].clobbers
    assert runtime.Reg.DX in runtime.CONTRACTS["B$FIST"].clobbers
    for name in ("B$FIS2", "B$FUST"):
        assert runtime.Reg.AX in runtime.CONTRACTS[name].clobbers
        assert runtime.Reg.DX not in runtime.CONTRACTS[name].clobbers, f"{name} returns one word"


# The one field a wrong answer corrupts memory through, pinned by what these
# bodies literally do: B$SASS and B$STDL write the descriptor they were handed,
# and B$SCAT, B$STI2 and B$LTRM all allocate, which reaches B$STCPCT and moves
# every other live string.
@pytest.mark.parametrize("name", ["B$SASS", "B$STDL", "B$SCAT", "B$STI2", "B$LTRM"])
def test_the_string_routines_write_caller_memory(name: str) -> None:
    assert runtime.writes_caller_memory(runtime.CONTRACTS[name])


# B$DSEG writes one runtime word and B$FERR reads one; neither can name
# anything of the caller's, which is what licenses holding a value across them.
@pytest.mark.parametrize("name", [*ABSORBED, "B$DSEG", "B$FERR"])
def test_the_routines_that_touch_no_caller_memory(name: str) -> None:
    assert not runtime.writes_caller_memory(runtime.CONTRACTS[name])


def test_a_contract_says_which_registers_it_reads() -> None:
    """B$FILD takes a LONG in dx:ax, and nothing recorded that.

    Its evidence has said so since it was written -- "dx and ax are read
    and never written" -- but Contract had no field to put it in, so the
    call's use list named only the flags and the moves setting the argument
    up read as dead. Removing them printed FADD= 918528 for 1049600.

    Empty everywhere else on purpose: cmacros' cProc puts arguments on the
    stack, and claiming a call reads bx keeps bx's entry value live from
    the top of the body to the call and leaves nothing free to hoist into.
    """
    assert runtime.contract("B$FILD").inputs == frozenset({runtime.Reg.AX, runtime.Reg.DX})
    assert runtime.contract("B$FIL2").inputs == frozenset({runtime.Reg.AX})
    # A store hands its answer back and takes nothing.
    assert runtime.contract("B$FIST").inputs == frozenset()
    # And a name with no entry is worst-case, where inputs do not arise.
    assert not runtime.contract("B$NOSUCH").established


def test_on_goto_takes_its_branch_index_in_bx() -> None:
    """B$OGTA's code is not in the source tree, so this is measured.

    All 16 call sites in the corpus are `mov bx` immediately before the
    call, and nothing else is unanimous. The same kind of evidence the rest
    of its entry rests on -- what the inline table looks like was measured
    too.

    It matters because a use list is what says an operation is needed: with
    inputs empty, the `mov bx` setting up the branch index read as dead.
    """
    assert runtime.contract("B$OGTA").inputs == frozenset({runtime.Reg.BX})


def test_every_established_contract_says_what_it_reads() -> None:
    """A contract established for its clobbers is not established for its
    inputs, and the two are different questions.

    `inputs=None` means unestablished and reads as every register. Empty
    means established as taking nothing in one. Defaulting to empty is what
    let dead code elimination delete a call's argument setup: B$FILD takes
    a long in dx:ax and printed FADD= 918528 for 1049600, B$OGTA takes the
    ON GOTO index in bx and the program stopped early.

    B$ENRA and B$EXSA stay unestablished on purpose. Their code is not in
    the source tree and the corpus does not settle it -- 16 of 28 sites set
    cx before B$ENRA and 12 set bx, which is correlation and not an ABI.
    """
    unknown = sorted(
        name for name, one in runtime.CONTRACTS.items() if one.established and one.inputs is None
    )
    assert unknown == ["B$ENRA", "B$EXSA"], f"unestablished inputs: {unknown}"
    assert runtime.worst("anything").inputs is None, "a name with no entry reads everything"


def test_a_routine_that_enters_user_code_still_writes_anything() -> None:
    """Narrowing `writes=ANY` to `OWN` is measured, and does not apply here.

    `tools/runtime_writes.py` reads a linked image and finds no runtime
    write naming a cell in BC_DATA -- so a routine writes its own data and
    whatever it was handed a pointer to. That holds for a routine that
    returns. One that hands control back to the program writes whatever
    that code writes, and GCC's modref gives up on an indirect call for
    exactly the same reason.
    """
    for name in ("B$CENP", "B$EVCK", "B$OEGA", "B$RESN", "B$FCMD"):
        contract = runtime.contract(name)
        assert contract.enters_user_code, f"{name} is listed here but does not enter user code"
        assert contract.writes is runtime.Memory.ANY, f"{name} was narrowed and must not be"
    assert runtime.contract("B$NOTAROUTINE").writes is runtime.Memory.ANY, (
        "an unestablished contract must concede everything"
    )


def test_the_measured_routines_are_narrowed() -> None:
    """And the ones that do return say what they actually write."""
    narrowed = [
        name
        for name, one in runtime.CONTRACTS.items()
        if one.writes is runtime.Memory.OWN
    ]
    assert len(narrowed) >= 15, f"only {len(narrowed)} routines carry the measurement"
    for name in narrowed:
        assert not runtime.CONTRACTS[name].enters_user_code, f"{name} enters user code and is narrowed"
