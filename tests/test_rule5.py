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
accident". Four readers have one:

  pairs.py    which register pair a long arrived in -- ax:dx or cx:bx is
              BC's convention and the only evidence a load pair is one long
  consts.py   resolving a semantic operand back to the value it names
  avail.py    the same, for a cell's holder
  wide.py     widening a register operand to its own root

Everything else in the list is a defect with a name attached.
"""

import ast
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent.parent / "qbopt"

# The passes: what runs between mir.raise_body and lowering.
PASSES = ("transform.py", "pairs.py", "consts.py", "avail.py", "wide.py", "segments.py")

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
    "pairs.py": None,  # whole file: pair identity is BC's register convention
    "consts.py": None,
    "avail.py": None,
    "wide.py": None,
    # transform.py and segments.py have no blanket permission. This is the
    # baseline the AST reports today, grouped by what empties it. Every
    # entry is a defect with a name attached; the list only shrinks.
    "transform.py": {
        # S1 -- the allocator living in the hoist
        "hoisted",
        "_insertion",
        "_move",
        "_instead",
        "_reads_from",
        "_writes_to",
        "_can_reseat",
        "_mentions",
        "_named",
        "_at_width",
        # S2 -- strength, which picks an encoding
        "_reduced",
        # S3 -- segments
        "_segment_load",
        # phase D -- absorption, which moves into the raise
        "_absorbing",
        "_comparing",
        "_deleting",
        "_wide",
        "_narrow",
        # origin readers, and each has the reason Value's docstring asks for:
        # what a value's half is, where a use came from, what an operation
        # reads. These stay.
        "<module>",
        "_carried",
        "_effective",
        "_folded_op",
        "_invariant_run",
        "_leaving",
        "_reads",
        "halves",
        "root",
        "widths",
    },
    "segments.py": {"_segment_name", "_loads_a_segment"},
}


def _named_in(path: Path) -> dict[str, set[str]]:
    """Every function in this file that names the machine, and what it named."""
    found: dict[str, set[str]] = {}
    where = ["<module>"]

    class Walk(ast.NodeVisitor):
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


@pytest.mark.parametrize("name", PASSES)
def test_a_pass_names_nothing_about_the_machine(name: str) -> None:
    allowed = ALLOWED.get(name, set())
    if allowed is None:
        return
    offending = {
        where: sorted(what) for where, what in _named_in(HERE / name).items() if where not in allowed
    }
    assert not offending, (
        f"{name}: {offending} name a register. Rule 5: only lower, regalloc "
        "and peephole see machine form."
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
