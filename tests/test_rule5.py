"""
Rule 5, checked rather than asserted.

`AGENTS.md` says every pass between the raise and lowering takes MIR and
returns MIR, naming nothing about the machine. It said so for a while
before it was true: five of twelve passes reasoned about x86 outright, and
the count only came out when someone walked the AST for it.

So this walks the AST. A pass that names a register fails here, and a new
one cannot be added quietly.

What is allowed, and why. Source placement is private to raising and leaves
that boundary as `AllocationHints`; no optimization or shared-analysis pass
may reach it. Two frontend recognition helpers still inspect the private
raise-time view:

  pairs.py    which register pair a long arrived in -- ax:dx or cx:bx is
              BC's convention and the only evidence a load pair is one long
  wide.py     widening a register operand to its own root

Everything else in the list is a defect with a name attached.
"""

import ast
import inspect
from pathlib import Path

import pytest

from qbopt.model import mir
from qbopt.analysis import liveness
from qbopt.analysis import ssa

HERE = Path(__file__).resolve().parent.parent / "qbopt"

# Discover every optimization module, including new passes and subpackages.
# Shared analyses and the remaining frontend compatibility helpers are explicit.
PASSES = tuple(
    sorted(
        {path.relative_to(HERE).as_posix() for path in (HERE / "optimize").rglob("*.py") if path.name != "__init__.py"}
        | {
            "frontend/pairs.py",
            "frontend/wide.py",
            "analysis/consts.py",
            "analysis/avail.py",
            "analysis/ssa.py",
            "analysis/induction.py",
        }
    )
)

# Naming any of these is naming the machine.
NAMED = frozenset(
    {
        "Register",
        "Register_",
        "AVAILABLE",
        "ADDRESSING",
        "ROOT",
        "_AT_WIDTH",
        "_at_width",
        "Reg",
    }
)

# Functions that may, with the reason above. Narrowed as each step lands;
# a name leaving this list is progress and a name joining it needs one.
ALLOWED = {
    "frontend/pairs.py": None,  # pair identity is BC's register convention
    "frontend/wide.py": {"_wider"},
}


def _named_in(path: Path) -> dict[str, set[str]]:
    """Every function in this file that names the machine, and what it named."""
    found: dict[str, set[str]] = {}
    where = ["<module>"]

    class Walk(ast.NodeVisitor):
        def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
            for name in node.names:
                if name.name in NAMED or (node.module or "").startswith("iced_x86"):
                    found.setdefault(where[-1], set()).add(name.name)

        def visit_Import(self, node: ast.Import) -> None:
            for name in node.names:
                if name.name == "iced_x86" or name.name.startswith("iced_x86."):
                    found.setdefault(where[-1], set()).add(name.name)

        def visit_FunctionDef(self, node: ast.FunctionDef) -> None:
            where.append(node.name)
            self.generic_visit(node)
            where.pop()

        def visit_Name(self, node: ast.Name) -> None:
            if node.id in NAMED:
                found.setdefault(where[-1], set()).add(node.id)

        def visit_Attribute(self, node: ast.Attribute) -> None:
            if node.attr in NAMED:
                found.setdefault(where[-1], set()).add(node.attr)
            self.generic_visit(node)

    Walk().visit(ast.parse(path.read_text()))
    return found


@pytest.mark.parametrize(
    "source",
    [
        "from iced_x86 import Register as R\ndef f(): return R.EAX\n",
        "import iced_x86 as machine\ndef f(): return machine.Code.MOV_R16_RM16\n",
    ],
)
def test_architecture_scan_cannot_be_bypassed_by_an_import_alias(tmp_path, source):
    """The boundary instrument must not report clean after Register is renamed R."""
    path = tmp_path / "pass.py"
    path.write_text(source)
    assert _named_in(path)


@pytest.mark.parametrize("name", PASSES)
def test_a_pass_names_nothing_about_the_machine(name: str) -> None:
    allowed = ALLOWED.get(name, set())
    if allowed is None:
        return
    offending = {where: sorted(what) for where, what in _named_in(HERE / name).items() if where not in allowed}
    assert not offending, (
        f"{name}: {offending} name a register. Rule 5: only lower, regalloc and peephole see machine form."
    )


@pytest.mark.parametrize("name", [name for name in PASSES if not name.startswith("frontend/")])
def test_a_pass_cannot_read_or_write_allocation_hints(name: str) -> None:
    """Placement history belongs to the raise/lower side table, not MIR.

    Checking both attributes and keyword arguments prevents a pass from
    silently propagating the metadata with ``replace(body, origin=...)`` even
    when it never branches on the physical register itself.
    """
    tree = ast.parse((HERE / name).read_text())
    found = [
        (node.lineno, node.attr)
        for node in ast.walk(tree)
        if isinstance(node, ast.Attribute) and node.attr in {"origin", "pins"}
    ]
    found += [
        (node.lineno, node.arg)
        for node in ast.walk(tree)
        if isinstance(node, ast.keyword) and node.arg in {"origin", "pins"}
    ]
    assert not found, f"{name}: allocation metadata crosses the MIR pass boundary at {found}"


def test_the_allow_list_names_nothing_that_has_already_gone() -> None:
    """A permission for a function that no longer names anything is a
    permission nobody is checking, and it hides the next one."""
    for name, allowed in ALLOWED.items():
        if allowed is None:
            continue
        actual = set(_named_in(HERE / name))
        stale = allowed - actual
        assert not stale, f"{name}: {sorted(stale)} no longer name a register; take them off the list"


def test_mir_has_no_slot_for_selected_machine_semantics() -> None:
    """MIR used to carry ``Op.made`` and let backend encodings leak into passes."""
    assert "made" not in mir.Op.__dataclass_fields__


def test_mir_has_no_slot_for_decoded_instruction_nodes() -> None:
    """Decoded x86 nodes are raise provenance, not program semantics."""
    assert "node" not in mir.Op.__dataclass_fields__


def test_mir_has_no_slot_for_source_byte_ranges() -> None:
    """Raw object-byte ownership belongs to raise provenance and allocated LIR.

    Keeping ``covers`` on public MIR let every new transform accidentally make
    physical layout decisions.  Opaque occurrence identities are the complete
    ownership vocabulary above lowering; concrete ranges exist only on the
    private raising occurrence and the LIR instruction that will be emitted.
    """
    assert "covers" not in mir.Op.__dataclass_fields__
    assert "extra_covers" not in mir.Op.__dataclass_fields__


def test_optimization_passes_never_mention_source_byte_ranges() -> None:
    """Passes transfer opaque occurrence ids; only lowering resolves byte ranges."""
    offending: dict[str, list[tuple[int, str]]] = {}
    for path in sorted((HERE / "optimize").rglob("*.py")):
        found = []
        for node in ast.walk(ast.parse(path.read_text())):
            if isinstance(node, ast.keyword) and node.arg in {"covers", "extra_covers"}:
                found.append((node.lineno, node.arg))
            if isinstance(node, ast.Attribute) and node.attr in {"covers", "extra_covers"}:
                found.append((node.lineno, node.attr))
        if found:
            offending[path.relative_to(HERE).as_posix()] = found
    assert not offending, f"MIR passes named source byte ranges: {offending}"


def _merged_ranges(spans: tuple[tuple[int, int], ...]) -> tuple[tuple[int, int], ...]:
    out: list[tuple[int, int]] = []
    for low, high in sorted(span for span in spans if span[0] < span[1]):
        if out and low <= out[-1][1]:
            out[-1] = (out[-1][0], max(high, out[-1][1]))
        else:
            out.append((low, high))
    return tuple(out)


@pytest.mark.parametrize(
    ("path", "only"),
    [
        (Path("fixtures/omf/lngmix-p-g2.obj"), None),
        (Path("fixtures/omf/fpemu-p-g2.obj"), "fold"),
    ],
)
def test_mir_names_owned_source_occurrences_without_repeating_their_byte_ranges(path: Path, only: str | None) -> None:
    """Folded integer and FP companions must not claim the source bytes twice."""
    import corpus
    from qbopt.optimize import transform

    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    raised = mir.bodies(found, blocks)
    for _name, original in raised:
        original_ids = {source_id for block in original.blocks for op in block.ops for source_id in op.absorbed}
        optimized = transform.applied(
            original,
            found.dgroup,
            found.calls,
            blocks=blocks,
            found=found,
            coverage=raised.source.coverage,
            only=only,
        )
        for body in (original, optimized):
            owners: dict[int, int] = {}
            for block in body.blocks:
                for op in block.ops:
                    if body is original:
                        actual = _merged_ranges(
                            tuple(span for source_id in op.absorbed for span in raised.source.occurrences[source_id])
                        )
                        assert not op.absorbed or actual, f"{op.at:#06x}: {op.absorbed} resolves to no source bytes"
                    for source_id in op.absorbed:
                        assert source_id not in owners, (
                            f"source occurrence {source_id} is owned by both {owners[source_id]:#06x} and {op.at:#06x}"
                        )
                        owners[source_id] = op.at
            assert set(owners) == original_ids


def test_lowering_resolves_source_byte_ownership_without_mir_ranges() -> None:
    """LNGMIX LIR ranges must be exactly those named by opaque MIR identities."""
    import corpus
    from qbopt.abi import runtime
    from qbopt.backend import lower

    path = Path("fixtures/omf/lngmix-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    contracts = runtime.for_module(found)
    raised = mir.bodies(found, blocks, contracts)
    name, body = raised[0]
    lowered = lower.lowered(
        name,
        body,
        found.calls,
        raised.source.absorbed,
        contracts,
        raised.source.coverage,
        nodes=raised.source.nodes,
        occurrences=raised.source.occurrences,
    )
    for insn in lowered.insns:
        source = insn.source
        if source is None or not source.absorbed:
            continue
        expected = _merged_ranges(
            tuple(span for source_id in source.absorbed for span in raised.source.occurrences[source_id])
        )
        actual = insn.spread or ((insn.covers,) if insn.covers is not None else ())
        assert actual == expected


def test_raise_returns_external_allocation_hints() -> None:
    """SSA renumbering must not make passes copy physical registers.

    Every version of one raised variable has one historical home, while a hard
    result pin belongs to one source definition.  Both survive SSA renumbering
    without putting machine metadata on MIR values.
    """
    from dataclasses import replace

    import corpus

    path = Path("fixtures/omf/lngmix-p-g2.obj")
    found = corpus.loaded(path)
    raised = mir.bodies(found, corpus.partitioned(path))
    assert raised.hints
    for _name, body in raised:
        hints = raised.hints[body.entry]
        values = tuple(ssa.values(body))
        assert hints.origins
        assert set(hints.origins) <= {value.variable for value in values}
        for variable, register in hints.origins.items():
            value = next(value for value in values if value.variable == variable)
            assert hints.origin_of(value) == register
            renamed = replace(value, id=value.id + 100_000, version=value.version + 100)
            assert hints.origin_of(renamed) == register
        definitions = {
            (op.id, index): (op, index)
            for block in body.blocks
            for op in block.ops
            if op.id is not None
            for index, value in enumerate(op.defines)
        }
        assert set(hints.pins) <= set(definitions)
        for key, register in hints.pins.items():
            op, index = definitions[key]
            assert hints.pin_of(op, index) == register
            assert hints.pin_of(replace(op, at=op.at + 100_000), index) == register


def test_public_mir_body_has_no_allocation_fields() -> None:
    """Placement history must leave at the raise/MIR boundary.

    Keeping an external copy while public MIR still exposed ``origin`` and
    ``pins`` let the next pass silently make them semantic again.  The type
    itself must make that architecture violation impossible.
    """
    assert "origin" not in mir.MirBody.__dataclass_fields__
    assert "pins" not in mir.MirBody.__dataclass_fields__


@pytest.mark.parametrize("stem", ["addrm-q-noO", "fpdeep-q-noO"])
def test_public_mir_names_values_observable_at_a_machine_exit(stem: str) -> None:
    """Removing ``origin`` must not make a body exit semantically unreadable.

    ADDRM printed 264 instead of 210 and FPDEEP printed SQ 2 = 0 when dead
    code and lowering could no longer see the values held at the terminal
    runtime call.  Raising knows those live-outs from source placement; it
    must express them as semantic MIR uses before placement becomes private,
    without turning them into operands of the terminal machine instruction.
    """
    import corpus
    from qbopt import wholeseg

    path = Path(f"fixtures/omf/{stem}.obj")
    found = corpus.loaded(path)
    raised = mir.bodies(found, corpus.partitioned(path))
    exits = [block for _name, body in raised for block in body.blocks if not block.succ]

    assert exits
    assert all(block.ops and mir.exit_values(block.ops[-1]) for block in exits)
    assert all(
        set(mir.exit_values(block.ops[-1])).isdisjoint(mir.ordinary_uses(block.ops[-1]))
        for block in exits
    )
    for _name, body in raised:
        live = liveness.live(body)
        for block in body.blocks:
            if not block.succ:
                assert set(mir.exit_values(block.ops[-1])) <= set(live.live_out[block.at])

    lowered = []
    result = wholeseg.emitted(
        path.read_bytes(),
        watch=lambda stage, _name, state: lowered.append(state) if stage == "lowered" else None,
    )
    assert result.outcome is wholeseg.Emission.LIR
    tagged = [
        insn
        for body in lowered
        for block in body.blocks
        for insn in block.insns
        if insn.op is not None and insn.op.exits
    ]
    assert tagged
    for insn in tagged:
        exiting = {value.id for value in mir.exit_values(insn.op)}
        delivered = {held.value for held, _register in insn.delivers}
        assert exiting.isdisjoint(insn.uses)
        assert exiting.isdisjoint(delivered)


def test_a_pin_belongs_to_one_definition_not_every_ssa_version() -> None:
    """r_walk was refused because its loop phi inherited a DX result pin.

    Both definitions are versions of variable seven, but only the first is
    the result of the source occurrence whose ABI fixes it in DX.  Treating a
    hard pin like the variable-wide origin preference pins the loop's joined
    pointer to DX, outside the 16-bit addressing register class.
    """
    from iced_x86 import Register

    first = mir.Value(1, 10, variable=7, version=1)
    second = mir.Value(2, 20, variable=7, version=2)
    first_op = mir.Op(10, mir.Synth.CONCAT_LOW, "first", (first,), (), id=100)
    second_op = mir.Op(20, mir.Synth.CONCAT_LOW, "second", (second,), (), id=200)
    hints = mir.AllocationHints(pins={(first_op.id, 0): Register.EDX})
    assert hints.pin_of(first_op, 0) == Register.EDX
    assert hints.pin_of(second_op, 0) is None


def test_lowering_consumes_external_allocation_hints() -> None:
    """The machine boundary must not need placement fields on MIR itself."""
    import corpus
    from qbopt.abi import runtime
    from qbopt.backend import lower

    path = Path("fixtures/omf/lngmix-p-g2.obj")
    found = corpus.loaded(path)
    contracts = runtime.for_module(found)
    raised = mir.bodies(found, corpus.partitioned(path), contracts)
    name, body = raised[0]
    hints = raised.hints[body.entry]
    low = lower.lowered(
        name,
        body,
        found.calls,
        raised.source.absorbed,
        contracts,
        raised.source.coverage,
        nodes=raised.source.nodes,
        occurrences=raised.source.occurrences,
        hints=hints,
    )
    actual = {value.variable: register for value, register in low.origin.items()}
    assert actual and all(
        actual[variable] == register for variable, register in hints.origins.items() if variable in actual
    )


@pytest.mark.parametrize("relative", ["flow.py", "wholeseg.py", "cfront/compile.py"])
def test_production_lowering_supplies_external_allocation_hints(relative: str) -> None:
    """A compatibility fallback must not become the production data path."""
    tree = ast.parse((HERE / relative).read_text())
    calls = [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr == "lowered"
    ]
    assert calls
    assert all(any(keyword.arg == "hints" for keyword in call.keywords) for call in calls)


def test_a_pass_is_a_transform_and_nothing_else() -> None:
    """The contract, as a fact rather than a convention.

    Before this, a pass was a function and its signature was where the
    machine got in: `hoisted(body, dgroup, calls, bounds)` grew the module's
    layout as arguments and ended up choosing registers with it. A class
    whose only entry point is `transform(body)` cannot.
    """
    from qbopt.model.passes import Where
    from qbopt.model.passes import MIRTransform
    from qbopt.optimize.transform import pipeline
    from qbopt.optimize.transform import PASSES as ORDER

    every = pipeline(Where())
    assert [one.name for one in every] == list(ORDER)
    assert all(isinstance(one, MIRTransform) for one in every)

    # one method, and it is the contract
    assert [n for n in vars(MIRTransform) if not n.startswith("_")] == ["name", "transform"]
    for one in every:
        assert type(one).transform is not MIRTransform.transform, f"{one.name} overrides nothing"
        implementation = Path(inspect.getfile(type(one))).relative_to(HERE).as_posix()
        assert implementation in PASSES, f"{one.name}: {implementation} is outside the architecture scan"
