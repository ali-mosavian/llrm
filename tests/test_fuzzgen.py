"""
Hermetic checks on the fuzz oracle itself: determinism, the trap filter, the
constant-fold-overflow invariant a real BC compile run caught (see fuzzgen.py's
module docstring), and the semantics the evaluator claims for the three
constructs a value can hide inside -- a FOR loop's trip count and exit value, a
BYREF parameter, a SHARED variable -- so a regression there is a fast local
failure, not a rediscovery three DOSBox launches later.

The loop, array and procedure invariants are stated as properties of every
generated program rather than of one: each is a rule the *generator* has to
keep for the evaluator to be able to say what a program means at all -- an
unmasked subscript, a counter something else writes, or a FUNCTION with a side
effect each make the answer depend on something the reference implementation
does not model.
"""

from functools import cache
from collections.abc import Iterator

import pytest
import fuzzgen
from qbprint import num
from qbprint import s16
from qbprint import s32

SEEDS = range(60)


@cache
def generated(seed: int) -> fuzzgen.Program:
    """One program per seed, shared by every test that reads one.

    test_generation_is_deterministic below is the one caller that must not
    come through here: its whole claim is that two independent generations
    from one seed agree, which a shared object would make vacuous.
    """
    return fuzzgen.generate_program(seed=seed)


INT = fuzzgen.Width.INT
LNG = fuzzgen.Width.LNG


def all_nodes(node: fuzzgen.Expr) -> Iterator[fuzzgen.Expr]:
    yield node
    match node:
        case fuzzgen.UnaryOp(_, _, operand) | fuzzgen.Index(_, _, operand):
            yield from all_nodes(operand)
        case fuzzgen.BinOp(_, _, left, right) | fuzzgen.Cmp(_, left, right):
            yield from all_nodes(left)
            yield from all_nodes(right)
        case fuzzgen.CallFn(_, _, args):
            for arg in args:
                yield from all_nodes(arg)


def all_stmts(stmts: tuple[fuzzgen.Stmt, ...]) -> Iterator[fuzzgen.Stmt]:
    for stmt in stmts:
        yield stmt
        if isinstance(stmt, fuzzgen.ForLoop):
            yield from all_stmts(stmt.body)


def every_stmt(program: fuzzgen.Program) -> Iterator[fuzzgen.Stmt]:
    yield from all_stmts(program.stmts)
    for proc in program.procs:
        yield from all_stmts(proc.body)


def stmt_exprs(stmt: fuzzgen.Stmt) -> Iterator[fuzzgen.Expr]:
    match stmt:
        case fuzzgen.Assign(_, expr) | fuzzgen.Result(_, _, expr) | fuzzgen.Print(_, expr):
            yield expr
        case fuzzgen.IfPrint(_, expr):
            yield expr
        case fuzzgen.SetElem(_, index, expr):
            yield index
            yield expr
        case fuzzgen.CallSub(_, args):
            yield from args


def every_node(program: fuzzgen.Program) -> Iterator[fuzzgen.Expr]:
    for stmt in every_stmt(program):
        for expr in stmt_exprs(stmt):
            yield from all_nodes(expr)


def written_names(stmt: fuzzgen.Stmt) -> Iterator[str]:
    match stmt:
        case fuzzgen.Assign(name, _) | fuzzgen.Result(name, _, _) | fuzzgen.ForLoop(name, _, _, _, _, _):
            yield name


def test_wrap_matches_measured_boundaries() -> None:
    # measured on VBDOS directly: see docs/handover.md's probe. 32767 + 1
    # wraps to -32768 and 300 * 300 wraps to 24464, with no /D in play.
    assert s16(32767 + 1) == -32768
    assert s16(300 * 300) == 24464
    assert s32(2147483647 + 1) == -2147483648


@pytest.mark.parametrize("seed", SEEDS)
def test_generation_is_deterministic(seed: int) -> None:
    a = fuzzgen.generate_program(seed=seed)
    b = fuzzgen.generate_program(seed=seed)
    assert fuzzgen.render_program(a) == fuzzgen.render_program(b)
    assert fuzzgen.golden_lines(a) == fuzzgen.golden_lines(b)


@pytest.mark.parametrize("seed", SEEDS)
def test_no_generated_program_traps(seed: int) -> None:
    # a program that hits the idiv fault case, or a subscript past the end of
    # its array, is not a golden -- it is a gap in the generator's own filters
    program = generated(seed)
    fuzzgen.golden_lines(program)


@pytest.mark.parametrize("seed", range(300))
def test_every_binop_width_matches_its_own_natural_type(seed: int) -> None:
    # BASIC decides a BinOp's width from its own two immediate operands, not
    # from what the generator wanted several levels up -- found the hard way
    # (see natural_width()'s own docstring and fuzzgen.py's module docstring):
    # a LONG-tagged node whose real leaves were both INTEGER-typed is INTEGER
    # arithmetic in the BASIC BC actually compiles, wrapped at 16 bits, not
    # the 32 this generator's AST would otherwise wrap it at. 185 of a first
    # 200-program sample violated this before _ensure_valid_operand existed.
    program = generated(seed)
    for node in every_node(program):
        if isinstance(node, fuzzgen.BinOp | fuzzgen.UnaryOp):
            assert fuzzgen.natural_width(node, program.widths) == node.width


@pytest.mark.parametrize("seed", SEEDS)
def test_no_if_condition_contains_not(seed: int) -> None:
    # measured on VBDOS under /O: "IF <expr> THEN..ELSE.." swaps branches on
    # <expr>'s own truthiness whenever NOT appears anywhere in it, not the
    # arithmetic NOT's -- a real BC finding (see AGENTS.md's "A BC compiler
    # behavior, not a qbopt one"), not something this grammar can fuzz safely
    program = generated(seed)
    for stmt in every_stmt(program):
        if isinstance(stmt, fuzzgen.IfPrint):
            assert not fuzzgen._contains_not(stmt.cond)


@pytest.mark.parametrize("seed", SEEDS)
def test_every_binop_has_a_variable_operand(seed: int) -> None:
    # BC constant-folds a BinOp of two pure literals at compile time and that
    # fold is overflow-checked, unlike the runtime instruction -- a generated
    # program that violates this fails to compile with "Math overflow" on an
    # unrelated later line (BC's own recovery attributes it there). Measured:
    # `(-1970530648 * -13199)` alone rejects; the same values via variables
    # (suite/divmod.bas's MULOVF) wrap silently at runtime.
    program = generated(seed)
    for node in every_node(program):
        if isinstance(node, fuzzgen.BinOp):
            assert fuzzgen._contains_var(node.left) or fuzzgen._contains_var(node.right)


@pytest.mark.parametrize("seed", SEEDS)
def test_every_subscript_is_masked_to_the_declared_bound(seed: int) -> None:
    # a subscript is `<expr> AND 15` against a `DIM arr(15)`, which is in
    # bounds for every value at either width -- the alternative is range
    # reasoning over wraparound, and an out-of-range subscript is a runtime
    # error rather than a divergence worth hunting
    program = generated(seed)
    subscripts = [n.index for n in every_node(program) if isinstance(n, fuzzgen.Index)]
    subscripts += [s.index for s in every_stmt(program) if isinstance(s, fuzzgen.SetElem)]
    assert subscripts
    for index in subscripts:
        assert isinstance(index, fuzzgen.BinOp)
        assert (index.width, index.op, index.right) == (INT, "AND", fuzzgen.Lit(INT, fuzzgen.ARRAY_LIMIT))


@pytest.mark.parametrize("seed", SEEDS)
def test_no_loop_body_writes_its_own_counter(seed: int) -> None:
    # the trip count has to be a property of the three literals alone: a body
    # that assigns the counter, nests a FOR on it, or hands it to a SUB BYREF
    # makes it a property of the body instead
    program = generated(seed)
    for stmt in every_stmt(program):
        if not isinstance(stmt, fuzzgen.ForLoop):
            continue
        for inner in all_stmts(stmt.body):
            assert stmt.var not in set(written_names(inner))
            if isinstance(inner, fuzzgen.CallSub):
                assert stmt.var not in {a.name for a in inner.args if isinstance(a, fuzzgen.Var)}


@pytest.mark.parametrize("seed", SEEDS)
def test_every_call_passes_distinct_unshared_variables(seed: int) -> None:
    # copy-in/copy-out is only equal to BASIC's own BYREF when nothing aliases:
    # two parameters bound to one variable, or a parameter bound to a variable
    # the callee also reaches through SHARED, break the equivalence
    program = generated(seed)
    shared = {d.name for d in program.declared if d.shared}
    for stmt in every_stmt(program):
        if not isinstance(stmt, fuzzgen.CallSub):
            continue
        names = [a.name for a in stmt.args if isinstance(a, fuzzgen.Var)]
        assert len(names) == len(stmt.args)
        assert len(set(names)) == len(names)
        assert not set(names) & shared


@pytest.mark.parametrize("seed", SEEDS)
def test_functions_write_nothing_the_caller_can_see(seed: int) -> None:
    # a FUNCTION may appear anywhere in an expression, so anything it writes
    # would make the answer depend on BC's operand order. Only its own locals
    # and its own result are writable, and it never prints.
    program = generated(seed)
    functions = [p for p in program.procs if p.kind is fuzzgen.ProcKind.FUNCTION]
    assert functions
    for proc in functions:
        allowed = {name for name, _ in proc.locals} | {proc.name}
        for stmt in all_stmts(proc.body):
            assert not isinstance(stmt, fuzzgen.Print | fuzzgen.IfPrint | fuzzgen.SetElem | fuzzgen.CallSub)
            assert set(written_names(stmt)) <= allowed


@pytest.mark.parametrize("seed", SEEDS)
def test_a_procedure_local_is_written_before_it_is_read(seed: int) -> None:
    # BC's prologue passes the frame size to the runtime, which is where a
    # zeroed frame would come from -- the generator does not lean on it: every
    # local opens the body with an initialiser that reads no local not already
    # initialised
    program = generated(seed)
    for proc in program.procs:
        ready: set[str] = set()
        for (name, _), stmt in zip(proc.locals, proc.body, strict=False):
            assert isinstance(stmt, fuzzgen.Assign) and stmt.name == name
            pending = {n for n, _ in proc.locals} - ready
            assert not {n.name for n in all_nodes(stmt.expr) if isinstance(n, fuzzgen.Var)} & pending
            ready.add(name)


@pytest.mark.parametrize("seed", SEEDS)
def test_golden_ends_with_done(seed: int) -> None:
    assert fuzzgen.golden_lines(generated(seed))[-1] == "DONE"


def test_render_never_emits_a_bare_min_literal() -> None:
    assert fuzzgen.render_lit(INT, -32768) == "(-32767 - 1)"
    assert fuzzgen.render_lit(LNG, -2147483648) == "(-2147483647 - 1)"
    assert fuzzgen.render_lit(INT, -5) == "-5"
    assert fuzzgen.render_lit(LNG, 5) == "5"


def test_unary_minus_never_glues_into_a_double_dash() -> None:
    # "--22904" confused BC's parser into misattributing a later line's error
    # (see fuzzgen.py's module docstring); a leading space after unary "-"
    # keeps the two operators apart no matter what its operand renders as.
    node = fuzzgen.UnaryOp(LNG, "-", fuzzgen.Lit(LNG, -22904))
    assert "--" not in fuzzgen.render_expr(node)


@pytest.mark.parametrize("seed", SEEDS)
def test_source_has_dos_line_endings_and_module_code_ends_in_done(seed: int) -> None:
    text = fuzzgen.render_program(generated(seed))
    assert text.count("\r\n") == text.count("\n")
    module = text.split("\r\n\r\n")[0]
    assert module.splitlines()[-1] == 'PRINT "DONE"'


@pytest.mark.parametrize("seed", SEEDS)
def test_every_for_is_closed_and_every_procedure_ends(seed: int) -> None:
    program = generated(seed)
    lines = [ln.strip() for ln in fuzzgen.render_program(program).splitlines()]
    assert sum(1 for ln in lines if ln.startswith("FOR ")) == sum(1 for ln in lines if ln.startswith("NEXT "))
    for kind in (fuzzgen.ProcKind.SUB, fuzzgen.ProcKind.FUNCTION):
        opened = sum(1 for ln in lines if ln.startswith(f"{kind} "))
        assert opened == sum(1 for ln in lines if ln == f"END {kind}")
        assert opened == sum(1 for p in program.procs if p.kind is kind)


# ---------------------------------------------------------------------------
# What the evaluator claims BASIC means, on programs written here by hand
# ---------------------------------------------------------------------------


def counted_loop(start: int, limit: int, step: int) -> fuzzgen.Program:
    counter = fuzzgen.Var("i", INT)
    body = (fuzzgen.Print("B", counter),)
    stmts = (
        fuzzgen.ForLoop("i", INT, start, limit, step, body),
        fuzzgen.Print("AFTER", counter),
        fuzzgen.Done(),
    )
    return fuzzgen.Program((fuzzgen.Decl("i", INT, False, None),), (), stmts, {"i": INT})


@pytest.mark.parametrize(
    ("start", "limit", "step", "seen", "after"),
    [
        (1, 3, 1, [1, 2, 3], 4),
        (5, 5, 1, [5], 6),
        (1, 0, 1, [], 1),  # zero-trip: the test fails before the first pass
        (3, 1, -1, [3, 2, 1], 0),
        (-1, -1, -1, [-1], -2),
        (0, -1, 1, [], 0),
        (0, 7, 3, [0, 3, 6], 9),  # a limit the step does not land on
        (0, 6, 3, [0, 3, 6], 9),
        (2, -3, -2, [2, 0, -2], -4),
    ],
)
def test_for_trip_count_and_exit_value(start: int, limit: int, step: int, seen: list[int], after: int) -> None:
    want = [f"B={num(v)}" for v in seen] + [f"AFTER={num(after)}", "DONE"]
    assert fuzzgen.golden_lines(counted_loop(start, limit, step)) == want


def test_a_sub_writes_its_caller_through_a_byref_parameter() -> None:
    doubler = fuzzgen.Proc(
        name="proc1",
        kind=fuzzgen.ProcKind.SUB,
        result=None,
        params=(fuzzgen.Param("arg1", LNG),),
        locals=(),
        body=(fuzzgen.Assign("arg1", fuzzgen.BinOp(LNG, "+", fuzzgen.Var("arg1", LNG), fuzzgen.Var("arg1", LNG))),),
    )
    stmts = (
        fuzzgen.Assign("v0", fuzzgen.Lit(LNG, 100000)),
        fuzzgen.CallSub("proc1", (fuzzgen.Var("v0", LNG),)),
        fuzzgen.Print("V", fuzzgen.Var("v0", LNG)),
        fuzzgen.Done(),
    )
    program = fuzzgen.Program((fuzzgen.Decl("v0", LNG, False, None),), (doubler,), stmts, {"v0": LNG, "arg1": LNG})
    assert fuzzgen.golden_lines(program) == ["V= 200000", "DONE"]


def test_a_procedure_frame_does_not_outlive_the_call() -> None:
    # names are unique program-wide and a frame is spliced into the one scalar
    # map, so a parameter left behind would be readable by the next statement
    doubler = fuzzgen.Proc(
        name="proc1",
        kind=fuzzgen.ProcKind.SUB,
        result=None,
        params=(fuzzgen.Param("arg1", INT),),
        locals=(("tmp1", INT),),
        body=(fuzzgen.Assign("tmp1", fuzzgen.Var("arg1", INT)),),
    )
    env = fuzzgen.initial_env(fuzzgen.Program((fuzzgen.Decl("v0", INT, False, None),), (doubler,), (), {"v0": INT}))
    fuzzgen.run_stmt(fuzzgen.CallSub("proc1", (fuzzgen.Var("v0", INT),)), env)
    assert set(env.scalars) == {"v0"}


def test_a_function_reads_shared_state_and_returns_a_value() -> None:
    doubled = fuzzgen.Proc(
        name="func1",
        kind=fuzzgen.ProcKind.FUNCTION,
        result=LNG,
        params=(fuzzgen.Param("arg1", LNG),),
        locals=(),
        body=(fuzzgen.Result("func1", LNG, fuzzgen.BinOp(LNG, "+", fuzzgen.Var("arg1", LNG), fuzzgen.Var("g0", LNG))),),
    )
    stmts = (
        fuzzgen.Assign("g0", fuzzgen.Lit(LNG, 70000)),
        fuzzgen.Assign("v0", fuzzgen.Lit(LNG, 40000)),
        fuzzgen.Print("F", fuzzgen.CallFn(LNG, "func1", (fuzzgen.Var("v0", LNG),))),
        fuzzgen.Print("V", fuzzgen.Var("v0", LNG)),
        fuzzgen.Done(),
    )
    declared = (fuzzgen.Decl("g0", LNG, True, None), fuzzgen.Decl("v0", LNG, False, None))
    widths = {"g0": LNG, "v0": LNG, "arg1": LNG, "func1": LNG}
    program = fuzzgen.Program(declared, (doubled,), stmts, widths)
    assert fuzzgen.golden_lines(program) == ["F= 110000", "V= 40000", "DONE"]


def test_an_array_starts_at_zero_and_keeps_what_is_written() -> None:
    name, size = "arr0", 15
    index = fuzzgen.BinOp(INT, "AND", fuzzgen.Var("i", INT), fuzzgen.Lit(INT, size))
    stmts = (
        fuzzgen.Print("Z", fuzzgen.Index(LNG, name, fuzzgen.Lit(INT, 3))),
        fuzzgen.ForLoop(
            "i",
            INT,
            0,
            size,
            1,
            (fuzzgen.SetElem(name, index, fuzzgen.BinOp(LNG, "*", fuzzgen.Var("i", INT), fuzzgen.Lit(LNG, 100000))),),
        ),
        fuzzgen.Print("E", fuzzgen.Index(LNG, name, fuzzgen.Lit(INT, 15))),
        fuzzgen.Done(),
    )
    declared = (fuzzgen.Decl("i", INT, False, None), fuzzgen.Decl(name, LNG, True, size))
    program = fuzzgen.Program(declared, (), stmts, {"i": INT, name: LNG})
    assert fuzzgen.golden_lines(program) == ["Z= 0", "E= 1500000", "DONE"]


def test_a_subscript_past_the_end_is_a_trap_not_a_wrapped_read() -> None:
    # Python's own negative indexing would silently read from the far end
    stmts = (fuzzgen.Print("X", fuzzgen.Index(INT, "arr0", fuzzgen.Lit(INT, -1))),)
    program = fuzzgen.Program((fuzzgen.Decl("arr0", INT, True, 15),), (), stmts, {"arr0": INT})
    with pytest.raises(fuzzgen.Trap):
        fuzzgen.golden_lines(program)


@pytest.mark.parametrize(("width", "divisor"), [(INT, 0), (LNG, 0), (INT, -1), (LNG, -1)])
def test_the_two_cases_a_bare_idiv_faults_on_are_traps(width: fuzzgen.Width, divisor: int) -> None:
    # BC's runtime returns from both; qbopt's idiv does not (AGENTS.md, "What
    # the runtime does that an instruction does not")
    node = fuzzgen.BinOp(width, "\\", fuzzgen.Lit(width, fuzzgen.bounds(width)[0]), fuzzgen.Lit(width, divisor))
    with pytest.raises(fuzzgen.Trap):
        fuzzgen.eval_expr(node, fuzzgen.Env({}, {}, {}, []))


@pytest.mark.parametrize(
    ("op", "x", "y", "want"),
    [("\\", -7, 2, -3), ("\\", 7, -2, -3), ("MOD", -7, 2, -1), ("MOD", 7, -2, 1), ("MOD", -7, -2, -1)],
)
def test_integer_division_truncates_toward_zero(op: str, x: int, y: int, want: int) -> None:
    node = fuzzgen.BinOp(INT, op, fuzzgen.Lit(INT, x), fuzzgen.Lit(INT, y))
    assert fuzzgen.eval_expr(node, fuzzgen.Env({}, {}, {}, [])) == want


def test_float_programs_are_actually_generated() -> None:
    """A float width that never reaches a statement generates no x87 at all.

    The first version declared SINGLE and DOUBLE variables and then never
    used one, because _gen_stmt picked its width from a hardcoded
    (INT, LNG). Nothing failed -- the programs were valid and the oracle
    agreed -- which is exactly why this is checked rather than assumed.
    """
    seen = 0
    for seed in range(30):
        text = fuzzgen.render_program(fuzzgen.generate_program(seed, n_arrays=4))
        seen += text.count("CLNG(")
    assert seen > 20, f"only {seen} float results across 30 programs"


@pytest.mark.parametrize("seed", range(40))
def test_a_float_value_never_leaves_the_exactly_representable_range(seed: int) -> None:
    """The whole reason the evaluator can do integer arithmetic on floats.

    Every float this generates is an integer the format holds exactly, so
    IEEE arithmetic on it is integer arithmetic -- and so is the x87's own
    80-bit evaluation, which is why intermediate precision cannot make BC
    and the evaluator disagree. A value past that range is a Trap.
    """
    program = fuzzgen.generate_program(seed, n_arrays=4)
    for name, width in program.widths.items():
        if width in fuzzgen.FLOAT:
            lo, hi = fuzzgen.bounds(width)
            assert abs(lo) <= fuzzgen.EXACT[width] and hi <= fuzzgen.EXACT[width], name


@pytest.mark.parametrize("seed", range(40))
def test_every_float_result_is_printed_through_clng(seed: int) -> None:
    """suite/fpemu.bas's own trick, and what keeps BASIC's float PRINT
    formatting out of the oracle entirely."""
    program = fuzzgen.generate_program(seed, n_arrays=4)
    for line in fuzzgen.render_program(program).split("\r\n"):
        if not line.strip().startswith('PRINT "T'):
            continue
        # a float literal or suffix outside a CLNG() would be a value whose
        # printed form this project does not model
        shown = line.split(";", 1)[1] if ";" in line else ""
        if ("!" in shown or "#" in shown) and "CLNG(" not in shown:
            raise AssertionError(f"seed {seed}: float printed raw: {line}")
