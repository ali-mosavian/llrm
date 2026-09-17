"""
Rule 5, checked rather than asserted.

`AGENTS.md` says every pass between the raise and lowering takes MIR and
returns MIR, naming nothing about the machine. It said so for a while
before it was true: five of twelve passes reasoned about x86 outright, and
the count only came out when someone walked the AST for it.

So this walks the AST. A pass that names a register fails here, and a new
one cannot be added quietly.

What is allowed, and why. `MirBody.origin` is where BC kept a value, and
`Value`'s own docstring sanctions reading it "with a reason, not by
accident". Two compatibility readers remain:

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

    def merged(spans: tuple[tuple[int, int], ...]) -> tuple[tuple[int, int], ...]:
        out: list[tuple[int, int]] = []
        for low, high in sorted(span for span in spans if span[0] < span[1]):
            if out and low == out[-1][1]:
                out[-1] = (out[-1][0], high)
            else:
                out.append((low, high))
        return tuple(out)

    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    raised = mir.bodies(found, blocks)
    for _name, original in raised:
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
                    explicit = (*((op.covers,) if op.covers is not None else ()), *op.extra_covers)
                    recorded = raised.source.coverage.get(op.id, ()) if op.id is not None else ()
                    expected = merged((*explicit, *recorded[1:]))
                    actual = merged(
                        tuple(span for source_id in op.absorbed for span in raised.source.occurrences[source_id])
                    )
                    assert actual == expected, f"{op.at:#06x}: {op.absorbed} resolves to {actual}, expected {expected}"
                    for source_id in op.absorbed:
                        assert source_id not in owners, (
                            f"source occurrence {source_id} is owned by both {owners[source_id]:#06x} and {op.at:#06x}"
                        )
                        owners[source_id] = op.at


def test_lowering_resolves_source_byte_ownership_without_mir_ranges() -> None:
    """LNGMIX lowering must reproduce its old ranges from opaque occurrence ids."""
    from dataclasses import replace

    import corpus
    from qbopt.abi import runtime
    from qbopt.backend import lower

    path = Path("fixtures/omf/lngmix-p-g2.obj")
    found = corpus.loaded(path)
    blocks = corpus.partitioned(path)
    contracts = runtime.for_module(found)
    raised = mir.bodies(found, blocks, contracts)
    name, body = raised[0]
    baseline = lower.lowered(
        name,
        body,
        found.calls,
        raised.source.absorbed,
        contracts,
        raised.source.coverage,
        nodes=raised.source.nodes,
    )
    stripped = replace(
        body,
        blocks=tuple(
            replace(
                block,
                ops=tuple(replace(op, covers=None, extra_covers=()) if op.absorbed else op for op in block.ops),
            )
            for block in body.blocks
        ),
    )
    migrated = lower.lowered(
        name,
        stripped,
        found.calls,
        raised.source.absorbed,
        contracts,
        raised.source.coverage,
        nodes=raised.source.nodes,
        occurrences=raised.source.occurrences,
    )
    old_ranges = tuple((one.covers, one.spread) for one in baseline.insns)
    new_ranges = tuple((one.covers, one.spread) for one in migrated.insns)
    assert new_ranges == old_ranges


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
