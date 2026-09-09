"""
$$TYPES, decoded from real BC output: array, structure and BYREF shapes.

Each case compiles a suite probe with /Zi on all three compilers and checks
the *resolved* type name -- "ARRAY OF LONG", "BYREF LONG" -- rather than the
raw type_index, which is compiler-specific (VBDOS/PDS route a BYREF LONG
parameter through a $$TYPES chain; QB 4.5 uses its own primitive-plus-0x20
code and never touches $$TYPES for it at all). The resolved name is the one
invariant across all three; see qbopt/objectfile/cvinfo.py's module docstring.
"""

import tempfile
from pathlib import Path

import pytest
from configs import CONFIGS
from dosbox import read_dos
from dosbox import dosbox_bin
from cache import cached_launch
from cache import toolchain_identity

from qbopt.objectfile import omf
from qbopt.objectfile import cvinfo

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


def compile_with_debug(tag: str, program: str, source_dir: Path = SUITE) -> cvinfo.DebugInfo:
    cfg = CONFIGS[tag]
    (root := ROOT / "build" / "cvinfo").mkdir(parents=True, exist_ok=True)
    # unique per call, not shared by (tag, program) -- several test functions
    # here compile the same program, and a shared workdir races for real
    # under pytest-xdist. The cache keys off the workdir's *content*, so a
    # fresh directory each time costs nothing.
    work = Path(tempfile.mkdtemp(dir=root, prefix=f"{tag}-{program}-"))
    dos_name = f"{program.upper()[:8]}.BAS"
    (work / dos_name).write_bytes((source_dir / f"{program}.bas").read_bytes())

    obj_name = f"{program.upper()[:8]}.OBJ"
    run = cached_launch(
        work,
        cfg.mount,
        [f"{cfg.bc} /Zi {cfg.switches} {dos_name}, {obj_name}; > BC.OUT"],
        identity=toolchain_identity(cfg),
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


def procedure(info: cvinfo.DebugInfo, name: str) -> cvinfo.Procedure:
    (found,) = [p for p in info.procedures if bare(p.name) == bare(name)]
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


def own_local(info: cvinfo.DebugInfo, proc: str, name: str) -> cvinfo.Local:
    (owner,) = [p for p in info.procedures if bare(p.name) == bare(proc)]
    (found,) = [loc for loc in owner.own_locals if bare(loc.name) == bare(name)]
    return found


@pytest.mark.parametrize("tag", CASES)
def test_array_of_udt_is_the_same_shape_module_level_and_bp_relative(tag: str) -> None:
    # udt.bas only ever measured the module-level DIM; Inside's own lpts is
    # the BPREL half of the same shape.
    info = compile_with_debug(tag, "arrudt")
    pts = variable(info, "pts")
    assert pts.type_name is not None and pts.type_name.upper() == "ARRAY OF TYPE COORD"
    lpts = own_local(info, "Inside", "lpts")
    assert lpts.type_name is not None and lpts.type_name.upper() == "ARRAY OF TYPE COORD"
    assert lpts.type_index == pts.type_index  # the exact same $$TYPES entry, not a re-emission


@pytest.mark.parametrize("tag", CASES)
def test_a_type_field_that_is_itself_a_type_resolves_transparently(tag: str) -> None:
    # _parse_struct never needed a change for this -- a field's type_index
    # already just points at another Struct entry, and type_name already
    # recurses. Confirmed at both module and procedure scope, and TAG_FIXED_
    # STRING is what a STRING * n field needs -- it never reuses PRIMITIVES.
    info = compile_with_debug(tag, "nestud")
    for o in (variable(info, "o"), own_local(info, "Inside", "lo")):
        assert o.type_name is not None and o.type_name.upper() == "TYPE OUTER"
        entry = info.types[o.type_index]
        assert isinstance(entry, cvinfo.Struct)
        fields = [(f.name.lower(), (cvinfo.type_name(f.type_index, info.types) or "").upper()) for f in entry.fields]
        assert fields[0] == ("part", "TYPE INNER")
        assert fields[1] == ("tag", "STRING * 2")


@pytest.mark.parametrize("tag", CASES)
def test_array_of_nested_udt_resolves_all_the_way_down(tag: str) -> None:
    info = compile_with_debug(tag, "nestud")
    arr = variable(info, "arr")
    assert arr.type_name is not None and arr.type_name.upper() == "ARRAY OF TYPE OUTER"
    (proc,) = [p for p in info.procedures if bare(p.name) == "INSIDE"]
    (larr,) = [loc for loc in proc.own_locals if bare(loc.name) == "LARR"]
    assert larr.type_name is not None and larr.type_name.upper() == "ARRAY OF TYPE OUTER"


@pytest.mark.parametrize("tag", CASES)
def test_a_single_field_struct_that_is_last_in_the_segment_still_parses(tag: str) -> None:
    # Solo is used only as a bare local declared last in the source, with
    # nothing referencing it afterward -- its own Struct record ends up the
    # last entry in $$TYPES. The struct's trailing byte is still 0x69 here,
    # the same as every other shape measured; cvinfo.py does not read it.
    info = compile_with_debug(tag, "nestud")
    lastvar = variable(info, "lastvar")
    assert lastvar.type_name is not None and lastvar.type_name.upper() == "TYPE SOLO"
    entry = info.types[lastvar.type_index]
    assert isinstance(entry, cvinfo.Struct)
    assert len(entry.fields) == 1


@pytest.mark.parametrize("tag", CASES)
def test_array_parameter_is_byref_on_every_compiler(tag: str) -> None:
    # VBDOS and PDS route an array parameter through the exact same
    # TAG_BYREF-wraps-TAG_POINTER chain any other BYREF parameter uses.
    # QB 4.5 diverges a second way beyond its own primitive-plus-0x20 codes:
    # a bare TAG_POINTER, one hop instead of two.
    info = compile_with_debug(tag, "arrprm")
    nums_param = param(info, "FillNums", "arr")
    assert nums_param.type_name is not None and nums_param.type_name.upper() == "BYREF ARRAY OF LONG"
    pts_param = param(info, "FillPts", "arr")
    assert pts_param.type_name is not None and pts_param.type_name.upper() == "BYREF ARRAY OF TYPE COORD"


@pytest.mark.parametrize("tag", CASES)
def test_byref_single_and_double_parameters(tag: str) -> None:
    # procs.bas only ever measured LONG/INTEGER/STRING; QB45_BYREF_PRIMITIVES
    # gets 0xA8 (SINGLE) and 0xA9 (DOUBLE) added on the strength of this.
    info = compile_with_debug(tag, "byref2")
    single_param = param(info, "Half", "n")
    assert single_param.type_name == "BYREF SINGLE"
    double_param = param(info, "Doubled", "n")
    assert double_param.type_name == "BYREF DOUBLE"


@pytest.mark.parametrize("tag", CASES)
def test_procedure_signature_return_type_agrees_with_the_sigil(tag: str) -> None:
    # The $$SYMBOLS PROC record's own type_index names a TAG_SIGNATURE
    # record in $$TYPES, and that record's return_type agrees with the
    # sigil-derived one for every FUNCTION measured -- wired up as
    # Procedure.signature, not folded into return_type (see the module
    # docstring for why a SUB can't be told apart from an INTEGER FUNCTION
    # by this record alone).
    info = compile_with_debug(tag, "procs")
    twice = procedure(info, "Twice")
    assert twice.signature is not None
    assert cvinfo.type_name(twice.signature.return_type) == "LONG"
    if twice.return_type is not None:  # VBDOS drops the sigil from a procedure's own debug name entirely
        assert twice.return_type == "LONG"
    # Report is a SUB: its own signature carries BC's placeholder (INTEGER,
    # the DEFINT default), which is not a real return type.
    report = procedure(info, "Report")
    assert report.signature is not None
    assert cvinfo.type_name(report.signature.return_type) == "INTEGER"
    assert report.return_type is None


@pytest.mark.parametrize("tag", CASES)
def test_a_zero_parameter_procedures_signature_has_an_empty_arglist(tag: str) -> None:
    # A signature with no parameters points its own arglist at $$TYPES'
    # first (always 1-byte, 0x80) entry instead of a real TypeList, since
    # there is nothing to list -- absent entirely from a module with no
    # procedure at all (suite/arrays.bas, suite/udt.bas).
    info = compile_with_debug(tag, "arrudt")
    inside = procedure(info, "Inside")
    assert inside.signature is not None
    assert inside.signature.params == ()


BYVAL_COMPILERS = ["v-g3", "p-g2"]  # QB 4.5 rejects BYVAL outright -- see suite/cvonly/byval.bas

BYVAL_CASES = [
    pytest.param(
        tag,
        marks=pytest.mark.skipif(not CONFIGS[tag].available, reason=f"no {tag} toolchain"),
    )
    for tag in BYVAL_COMPILERS
]


@pytest.mark.parametrize("tag", BYVAL_CASES)
def test_byval_carries_the_primitive_type_index_directly(tag: str) -> None:
    # QuickBASIC 4.5's own BC.EXE rejects `BYVAL n AS LONG` on a SUB/FUNCTION
    # signature outright ("Formal parameter specification illegal"), so this
    # is a two-compiler measurement, and the probe lives outside suite/ so
    # tools/e2e.py's differential harness -- which needs every configuration,
    # QB 4.5 included, to compile -- never tries to build it.
    info = compile_with_debug(tag, "byval", source_dir=SUITE / "cvonly")
    by_ref = param(info, "AddRef", "n")
    assert by_ref.type_name == "BYREF LONG"
    by_val = param(info, "AddVal", "n")
    assert by_val.type_name == "LONG"  # no wrapper hop at all -- same as a local


def test_a_module_variable_is_where_its_fixup_says() -> None:
    """Every one of them came back at address zero.

    BC writes a module variable's offset and segment as four zero bytes and
    leaves a ptr16:16 fixup to fill them in, exactly as it does for an
    operand in the code. Read without the fixups every DIM in the module is
    at address zero, which is a number no real program has -- and the names
    are then unusable, because nothing can say which cell each one is.
    """
    from pathlib import Path

    from qbopt.objectfile import cvinfo
    from qbopt.objectfile import omf

    got = cvinfo.parse(omf.parse(Path("fixtures/omf/procs-p-g2-zi.obj").read_bytes()))
    assert got.variables, "the object carries no symbols; it was not built with /Zi"
    where = {(one.segment, one.offset) for one in got.variables}
    assert (0, 0) not in where, "a variable at address zero is an unrelocated field"
    assert len(where) == len(got.variables), "two variables cannot share one address"
    assert {one.name for one in got.variables} == {"A&", "B&", "R&"}


def test_a_parameter_is_above_the_frame_pointer_and_a_local_below() -> None:
    """Which one a slot is, is the sign of its own offset, and BC says both.

    `Twice&(n AS LONG)` has one parameter at bp+6 -- the caller pushed it --
    and one local at bp-22, below where the frame pointer landed. Nothing
    has to infer it from the prologue.
    """
    from pathlib import Path

    from qbopt.objectfile import cvinfo
    from qbopt.objectfile import omf

    got = cvinfo.parse(omf.parse(Path("fixtures/omf/procs-p-g2-zi.obj").read_bytes()))
    named = {one.name: one for one in got.procedures}
    assert "TWICE&" in named and "REPORT" in named, f"only {sorted(named)}"

    twice = named["TWICE&"]
    params = {one.name: one.bp_offset for one in twice.locals if one.bp_offset > 0}
    locals_ = {one.name: one.bp_offset for one in twice.locals if one.bp_offset < 0}
    assert params == {"N&": 6}, params
    assert locals_ == {"T&": -22}, locals_
    # Report takes two and declares no local of its own
    report = named["REPORT"]
    assert {one.name for one in report.locals if one.bp_offset > 0} == {"N&", "TAG$"}


def test_an_array_is_where_its_descriptor_points() -> None:
    """BC names an array in BC_CN and the code only ever uses BC_DATA.

    The descriptor's first four bytes are a far pointer to the elements and
    BC leaves them zero with a ptr16:16 fixup, the same way it leaves the
    symbol record's own address. Without following it the two addresses are
    in different segments and never meet, so no array access in the program
    can be named -- and nbody is nothing but array accesses.
    """
    from pathlib import Path

    from qbopt.objectfile import cvinfo
    from qbopt.objectfile import omf

    got = cvinfo.parse(omf.parse(Path("fixtures/omf/arridx-p-g2-zi.obj").read_bytes()))
    arrays = [one for one in got.variables if "ARRAY" in (one.type_name or "")]
    assert arrays, "the object declares no array, so this proves nothing"
    for one in arrays:
        assert one.data is not None, f"{one.name}: the descriptor was not followed"
        assert one.data != (one.segment, one.offset), f"{one.name}: named where its descriptor is"
        assert one.stride and one.count, f"{one.name}: {one.count} x {one.stride}"
    # a% is 21 INTEGERs -- `DIM a%(20)`, which BASIC bases at zero
    named = {one.name: one for one in arrays}
    assert named["A%"].stride == 2 and named["A%"].count == 21

    # A scalar has no descriptor and is not given one: its own address is
    # where it is, and `data` stays None.
    scalars = [one for one in got.variables if "ARRAY" not in (one.type_name or "")]
    assert scalars and all(one.data is None for one in scalars)
