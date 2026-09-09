from pathlib import Path
from dataclasses import replace

import pytest

import corpus
from qbopt.abi import runtime
from qbopt.legacy.calls import ARITY
from qbopt.abi.runtime import Reg
from qbopt.legacy.calls import DIVIDE
from qbopt.legacy.calls import COMPARE
from qbopt.abi.runtime import EVERY
from qbopt.legacy.calls import MULTIPLY
from qbopt.abi.runtime import Memory
from qbopt.legacy.calls import REMAINDER
from qbopt.abi.runtime import Control
from qbopt.frontend.blocks import INLINE_TABLE

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


@pytest.mark.parametrize("family", ["pds71", "vbdos"])
def test_evk1_alias_keeps_event_effects(family: str) -> None:
    """Event-enabled ADDRM refused emission before executing its first statement."""
    alias = runtime.per_call({0: "B$EVK1"}, family)[0]
    original = runtime.contract("B$EVCK")
    assert replace(alias, name=original.name, evidence=original.evidence) == original
    assert runtime.barrier(alias)
    assert alias.enters_user_code
    assert alias.writes is Memory.ANY


@pytest.mark.parametrize("family", ["", "qb45", "unknown"])
def test_evk1_alias_requires_an_established_family(family: str) -> None:
    assert runtime.per_call({0: "B$EVK1"}, family)[0] == runtime.worst("B$EVK1")


def test_evk1_user_definition_overrides_runtime_alias() -> None:
    assert runtime.per_call({0: "B$EVK1"}, "pds71", frozenset({"B$EVK1"}))[0] == runtime.own("B$EVK1")


@pytest.mark.parametrize("family", ["pds71", "vbdos"])
@pytest.mark.parametrize("name", ["B$ONTA", "B$ETT0", "B$ETT1", "B$ETT2"])
def test_timer_interfaces_bound_inputs_without_claiming_preservation(family, name) -> None:
    """The real timer-handler witness refused at B$ONTA before registration."""
    routine = runtime.per_call({0: name}, family)[0]
    assert routine.inputs == frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI})
    assert replace(routine, inputs=None, evidence=runtime.worst(name).evidence) == runtime.worst(name)
    assert runtime.per_call({0: name})[0] == runtime.worst(name)


@pytest.mark.parametrize("tag", ["p-evt", "v-evt"])
def test_event_stub_near_call_has_no_register_arguments(tag: str) -> None:
    """ADDRM /V refused at 0048 before its first statement could execute."""
    from qbopt.objectfile import module
    found = module.load(Path(f"fixtures/omf/addrm-{tag}.obj"))
    routine = runtime.for_module(found)[0x48]
    assert routine.inputs == frozenset()
    assert routine.cleanup == 0
    assert routine.enters_user_code and runtime.barrier(routine)
    assert routine.reads is routine.writes is Memory.ANY


def test_changed_event_stub_remains_unknown() -> None:
    from qbopt.objectfile import module
    found = module.load(Path("fixtures/omf/addrm-p-evt.obj"))
    for at in range(0x30, 0x42):
        code = bytearray(found.code)
        code[at] ^= 1
        assert 0x48 not in runtime.for_module(replace(found, code=bytes(code)))


@pytest.mark.parametrize("field", [0x34, 0x3E])
def test_event_stub_requires_exact_relocation(field: int, monkeypatch) -> None:
    from qbopt.objectfile import module, omf
    found = module.load(Path("fixtures/omf/addrm-p-evt.obj"))
    original = omf.fixups
    def changed(records):
        return [replace(one, disp=1) if one.seg == found.seg and one.offset == field else one
                for one in original(records)]
    monkeypatch.setattr(omf, "fixups", changed)
    assert 0x48 not in runtime.for_module(found)


@pytest.mark.parametrize("tag", ["p-evt", "v-evt"])
def test_addrm_event_adapter_emits_without_fallback(tag: str) -> None:
    """ADDRM /V had no output: strict lowering refused the call at 0048."""
    from qbopt import wholeseg
    result = wholeseg.emitted(Path(f"fixtures/omf/addrm-{tag}.obj").read_bytes())
    assert result.outcome is wholeseg.Emission.LIR, result.reason


@pytest.mark.parametrize("family", ["qb45", "pds71", "vbdos"])
def test_command_line_has_bounded_inputs_without_optimistic_effects(family: str) -> None:
    """nbody refused strict emission at COMMAND$ before reaching its integrator."""
    routine = runtime.per_call({0: "B$FCMD"}, family)[0]
    assert routine.inputs == frozenset({Reg.AX, Reg.BX, Reg.CX, Reg.DX, Reg.SI, Reg.DI})
    assert routine.cleanup == 0
    assert replace(routine, inputs=None, cleanup=None, evidence=runtime.worst("B$FCMD").evidence) == runtime.worst("B$FCMD")
    assert runtime.per_call({0: "B$FCMD"}, "")[0].inputs is None


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
    unknown = sorted(name for name, one in runtime.CONTRACTS.items() if one.established and one.inputs is None)
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
    narrowed = [name for name, one in runtime.CONTRACTS.items() if one.writes is runtime.Memory.OWN]
    assert len(narrowed) >= 15, f"only {len(narrowed)} routines carry the measurement"
    for name in narrowed:
        assert not runtime.CONTRACTS[name].enters_user_code, f"{name} enters user code and is narrowed"


def test_every_row_in_the_table_is_loaded() -> None:
    """The contracts are data now. Nothing is transcribed on the way in."""
    import tomllib

    rows = tomllib.loads(runtime.TABLE.read_text())
    assert set(rows) == set(runtime.CONTRACTS), "the table and what loaded disagree"
    for name, row in rows.items():
        one = runtime.CONTRACTS[name]
        assert one.writes.name == row["writes"]
        assert one.reads.name == row["reads"]
        assert {r.value for r in one.clobbers} == set(row["clobbers"])
        assert one.established == row["established"]
        assert one.evidence.strip() == row["evidence"].strip()


def test_an_unestablished_row_concedes_everything() -> None:
    """`established = false` is the only safe answer to a routine nobody
    has read, and it has to concede every register and every byte."""
    for name, one in runtime.CONTRACTS.items():
        if one.established:
            continue
        assert one.writes is runtime.Memory.ANY, f"{name} is not established and narrows its writes"
        assert one.reads is runtime.Memory.ANY, f"{name} is not established and narrows its reads"


def test_the_semicolon_long_print_reads_no_register() -> None:
    """`PRINT x;` on a long is B$PSI4, and it was missing from the table.

    Absent, it read as unestablished, and the lowering refused every body
    that calls it. rt/prnval.asm:266 is `cProc B$PSI4,<PUBLIC,FAR>` /
    `MOV AX,SEMI SHL 8 + VT_I4` / `JMP SHORT B$PRINT` -- ax written
    first, no register read. The linked images agree under all three
    compilers: `mov ax,114h` at 0x189 under VBDOS, 0x5db under PDS 7.1
    and 0x1ef6 under QuickBASIC 4.5, so there is no variant.
    """
    from qbopt.abi import runtime

    one = runtime.contract("B$PSI4")
    assert one.established, "B$PSI4 is not in the table"
    assert one.inputs == frozenset(), f"it reads {one.inputs}"
    # Two words, because PRINTX pops the second unless `TEST AL,VT_SD` is
    # non-zero -- true for VT_I2 and VT_SD, false for VT_I4. B$PEI4 is
    # the same type through the same path and carries the same number.
    assert one.cleanup == 4, f"it pops {one.cleanup}"
    assert one.cleanup == runtime.contract("B$PEI4").cleanup
    assert one.clobbers == runtime.contract("B$PSI2").clobbers
    assert one.writes == runtime.contract("B$PSI2").writes
    assert one.reads == runtime.contract("B$PSI2").reads
