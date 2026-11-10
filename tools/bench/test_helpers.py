import pytest

import bench
import helpers

WRITTEN = {"power", "report", "bench_recpow"}  # recpow.c names no helper: `%` on a long is the compiler's

MAP = """
Module: /w/RECPOW.OBJ(recpow.c)
0000:0010      _power
Module: /w/C_EXT.OBJ(ext.asm)
0000:0100      _report
Module: /t/CM.LIB(LXMUL)
0000:0351      LXMUL@
Module: /t/CM.LIB(LDIV)
0000:036a*     LDIV@
0000:0373      LMOD@
"""


def stub(monkeypatch, tmp_path, externs):
    """The map above, the program's externs, and a library whose modules are their own names and sizes: LXMUL 25 B, LDIV 163 B."""
    listing = tmp_path / "a.map"
    listing.write_text(MAP)
    monkeypatch.setattr(helpers.sizes, "externs", lambda path: externs.get(path.name, []))
    monkeypatch.setattr(helpers.sizes, "code_bytes", lambda path: {"LXMUL": 25, "LDIV": 163}[path.read_text()])
    modules = []

    def extract(library, module, out):
        modules.append(module)
        out.write_text(module)
        return out

    monkeypatch.setattr(helpers, "omf_library_module", extract)
    return listing, modules


def test_a_helper_is_counted_once_with_what_it_pulls_in(monkeypatch, tmp_path):
    """tc's recpow -O1 stored 211 B: LMOD@ and LXMUL@ (188 B of CM.LIB) were in no object. Two calls of LMOD@ are one LDIV."""
    externs = {"recpow.obj": ["LMOD@", "LXMUL@", "LMOD@", "_report"]}
    listing, modules = stub(monkeypatch, tmp_path, externs)
    found = helpers.linked_helpers(tmp_path / "recpow.obj", listing, WRITTEN)
    assert sorted(found) == [("LDIV", 163), ("LXMUL", 25)] and sorted(modules) == ["LDIV", "LXMUL"]


def test_a_support_object_symbol_is_no_helper(monkeypatch, tmp_path):
    """`report` comes from the corpus's ext.asm, linked into every reference: it is no library routine."""
    externs = {"recpow.obj": ["_report"]}
    listing, modules = stub(monkeypatch, tmp_path, externs)
    assert helpers.linked_helpers(tmp_path / "recpow.obj", listing, WRITTEN) == [] and modules == []


def test_an_extern_the_map_does_not_define_refuses_the_count(monkeypatch, tmp_path):
    """A helper the map cannot place would be left out, and the reference would read smaller than the program is."""
    externs = {"recpow.obj": ["N_PCMP@"]}
    listing, _ = stub(monkeypatch, tmp_path, externs)
    with pytest.raises(helpers.HelperUnresolved):
        helpers.linked_helpers(tmp_path / "recpow.obj", listing, WRITTEN)


def test_code_fields_of_a_reference_carry_its_helpers(monkeypatch, tmp_path):
    """code_bytes is the object's plus the helpers' (211 + 188 = 399, tc recpow -O1); helper_bytes says which part. Ours, with no map, is the object's."""
    obj = tmp_path / "recpow.obj"
    (tmp_path / "recpow.c").write_text("long power(long b, int e);")
    monkeypatch.setattr(bench.sizes, "code_size", lambda path: type("S", (), {"code": 211, "align": 0, "unread": 0, "library": 0})())
    monkeypatch.setattr(bench.sizes, "externs", lambda path: ["LMOD@", "LXMUL@", "_report"])
    monkeypatch.setattr(bench.sizes, "is_elf", lambda path: False)
    monkeypatch.setattr(bench.helpers, "linked_helpers", lambda path, listing, written: [("LXMUL", 25), ("LDIV", 163)])
    reference = bench.code_fields(obj, tmp_path / "a.map", tmp_path / "recpow.c")
    assert (reference["code_bytes"], reference["helper_bytes"], reference["helper_modules"]) == (399, 188, ["LXMUL", "LDIV"])
    ours = bench.code_fields(obj)
    assert ours["code_bytes"] == 211 and "helper_bytes" not in ours


def test_a_library_function_the_source_calls_by_name_is_no_helper(monkeypatch, tmp_path):
    """jpeg_single calls `open` and `read` (through bench_io.h): tc's and Watcom's stdio is the program's own call, not a division. The first count
    added 12 KB of stdio to Watcom's jpeg_single."""
    listing = tmp_path / "a.map"
    listing.write_text(MAP + "Module: /t/CM.LIB(OPEN)\n0000:0400      _open\n")
    externs = {"p.obj": ["_open", "LMOD@"]}
    monkeypatch.setattr(helpers.sizes, "externs", lambda path: externs.get(path.name, []))
    monkeypatch.setattr(helpers.sizes, "code_bytes", lambda path: {"LDIV": 163, "OPEN": 387}[path.read_text()])
    monkeypatch.setattr(helpers, "omf_library_module", lambda library, module, out: (out.write_text(module), out)[1])
    assert helpers.linked_helpers(tmp_path / "p.obj", listing, {"open", "report"}) == [("LDIV", 163)]


def test_a_runtime_service_that_pulls_in_more_library_is_no_helper(monkeypatch, tmp_path):
    """bcc's FP87INIT (526 B, pulls the emulator and error reporting) is carried by every float program; LDIV is a routine on its own."""
    listing = tmp_path / "a.map"
    listing.write_text(MAP + "Module: /t/FP87.LIB(FP87INIT)\n0000:0500      FP87INIT@\n")

    def externs(path):
        if path.name == "p.obj":
            return ["FP87INIT@", "LMOD@"]
        return ["LXMUL@"] if path.read_text() == "FP87INIT" else []

    monkeypatch.setattr(helpers.sizes, "externs", externs)
    monkeypatch.setattr(helpers, "omf_library_module", lambda library, module, out: (out.write_text(module), out)[1])
    monkeypatch.setattr(helpers.sizes, "code_bytes", lambda path: {"LDIV": 163, "FP87INIT": 526}[path.read_text()])
    assert helpers.linked_helpers(tmp_path / "p.obj", listing, set()) == [("LDIV", 163)]
