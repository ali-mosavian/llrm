"""
$$TYPES, decoded from real BC output: array, structure and BYREF shapes.

Each case compiles a suite probe with /Zi on all three compilers and checks
the *resolved* type name -- "ARRAY OF LONG", "BYREF LONG" -- rather than the
raw type_index, which is compiler-specific (VBDOS/PDS route a BYREF LONG
parameter through a $$TYPES chain; QB 4.5 uses its own primitive-plus-0x20
code and never touches $$TYPES for it at all). The resolved name is the one
invariant across all three; see qbopt/cvinfo.py's module docstring.
"""

from pathlib import Path

import pytest
from dosbox import launch
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin

from qbopt import omf
from qbopt import cvinfo

pytestmark = [pytest.mark.e2e, pytest.mark.skipif(dosbox_bin() is None, reason="no dosbox-x")]

ROOT = Path(__file__).resolve().parents[1]
SUITE = ROOT / "suite"

COMPILERS = ["v-g3", "p-g2", "q-noO"]

CASES = [
    pytest.param(
        tag,
        marks=pytest.mark.skipif(not CONFIGS[tag].available, reason=f"no {tag} toolchain"),
    )
    for tag in COMPILERS
]


def compile_with_debug(tag: str, program: str) -> cvinfo.DebugInfo:
    cfg = CONFIGS[tag]
    work = ROOT / "build" / "cvinfo" / tag / program
    work.mkdir(parents=True, exist_ok=True)
    dos_name = f"{program.upper()[:8]}.BAS"
    (work / dos_name).write_bytes((SUITE / f"{program}.bas").read_bytes())

    obj_name = f"{program.upper()[:8]}.OBJ"
    run = launch(
        work,
        cfg.mount,
        [f"{cfg.bc} /Zi {cfg.switches} {dos_name}, {obj_name}; > BC.OUT"],
        timeout=180,
        env={"LIB": r"V:\LIB"},
    )
    assert run.finished, "compile did not return"
    obj = work / obj_name
    assert obj.is_file(), read_dos(work, "BC.OUT")
    return cvinfo.parse(omf.parse(obj.read_bytes()))


def bare(name: str) -> str:
    """A debug name, stripped of PDS/QB45's own type-suffix sigil and upper-cased."""
    return name.upper().rstrip("&$%!#")


def variable(info: cvinfo.DebugInfo, name: str) -> cvinfo.Variable:
    (found,) = [v for v in info.variables if bare(v.name) == bare(name)]
    return found


def param(info: cvinfo.DebugInfo, proc: str, name: str) -> cvinfo.Local:
    (owner,) = [p for p in info.procedures if bare(p.name) == bare(proc)]
    (found,) = [p for p in owner.params if bare(p.name) == bare(name)]
    return found


@pytest.mark.parametrize("tag", CASES)
def test_array_of_long_carries_element_type_but_no_bounds(tag: str) -> None:
    info = compile_with_debug(tag, "arrays")
    x = variable(info, "x")
    assert x.type_name == "ARRAY OF LONG"
    # BASIC's own array bounds live in the runtime descriptor, not here --
    # confirmed by a 1-D and a 2-D DIM of the same element producing the
    # byte-identical $$TYPES record. Array carries only the element type.
    assert info.types.get(x.type_index) == cvinfo.Array(element=0x82)


@pytest.mark.parametrize("tag", CASES)
def test_struct_fields_have_names_offsets_and_types(tag: str) -> None:
    info = compile_with_debug(tag, "udt")
    c = variable(info, "c")
    assert c.type_name is not None and c.type_name.upper() == "TYPE COORD"
    entry = info.types[c.type_index]
    assert isinstance(entry, cvinfo.Struct)
    assert entry.name.upper() == "COORD"  # PDS and QB 4.5 upper-case debug names; VBDOS keeps the source's case
    assert [(f.name.lower(), f.offset, cvinfo.type_name(f.type_index, info.types)) for f in entry.fields] == [
        ("x", 0, "LONG"),
        ("y", 4, "LONG"),
    ]


@pytest.mark.parametrize("tag", CASES)
def test_array_of_struct_resolves_the_element_all_the_way_down(tag: str) -> None:
    info = compile_with_debug(tag, "udt")
    pts = variable(info, "pts")
    assert pts.type_name is not None and pts.type_name.upper() == "ARRAY OF TYPE COORD"


@pytest.mark.parametrize("tag", CASES)
def test_byref_long_parameter_resolves_through_the_pointer_wrapper(tag: str) -> None:
    info = compile_with_debug(tag, "procs")
    n = param(info, "Twice", "n")
    assert n.type_name == "BYREF LONG"


@pytest.mark.parametrize("tag", CASES)
def test_byref_string_parameter_resolves_through_the_pointer_wrapper(tag: str) -> None:
    info = compile_with_debug(tag, "procs")
    tag_param = param(info, "Report", "tag")
    assert tag_param.type_name == "BYREF STRING"


@pytest.mark.parametrize("tag", CASES)
def test_a_plain_long_local_is_still_a_primitive_not_a_types_lookup(tag: str) -> None:
    # t is Twice's own LONG local, never BYREF -- it must stay a plain
    # primitive read, not accidentally routed through $$TYPES.
    info = compile_with_debug(tag, "procs")
    (owner,) = [p for p in info.procedures if bare(p.name) == "TWICE"]
    (t,) = [loc for loc in owner.own_locals if bare(loc.name) == "T"]
    assert t.type_name == "LONG"
