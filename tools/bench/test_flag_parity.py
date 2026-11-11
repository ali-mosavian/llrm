"""`uv run --project tools python -m pytest tools/bench/test_flag_parity.py`"""
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))
import flag_parity as fp  # noqa: E402

OURS = fp.ours_profile(["-O2", "-fno-inline-functions", "-march=i386"])
OW = ["-zq", "-mm", "-ecc", "-s", "-DOWREF", "-4", "-fpi87", "-ox", "-oe=0"]


def test_a_difference_in_the_instruction_set_is_documented_not_refused():
    refused, documented = fp.parity(OURS, fp.ow_profile(OW))
    assert refused == [] and documented == ["isa: ours i386, reference 486"]


def test_a_reference_built_in_another_memory_model_is_refused():
    with pytest.raises(SystemExit, match="model: ours medium, reference large"):
        fp.refuse(OURS, fp.ow_profile([one if one != "-mm" else "-ml" for one in OW]), "x ow -O2")


def test_watcoms_inline_expansion_on_the_reference_only_is_refused():
    """`-ox` alone inlines the kernel into main (it is `-obmiler -s`), so the entry the harness counts from is gone and the two sides do different work;
    `-ox -oe=0` and `-os -ol` keep the call."""
    with pytest.raises(SystemExit, match="kernel_call"):
        fp.refuse(OURS, fp.ow_profile([one for one in OW if one != "-oe=0"]), "x ow -O2")
    assert fp.parity(OURS, fp.ow_profile(["-mm", "-s", "-4", "-fpi87", "-os", "-ol"]))[0] == []


def test_the_borland_switches_give_each_compilers_instruction_set():
    assert [fp.borland_profile(c, s.split())["isa"] for c, s in (("bcc", "-O1"), ("tcc", "-O -Z -r -1"), ("tcc", "-2 -O -Z -r"))] == ["386", "186", "286"]


def test_the_bench_table_has_no_refused_cell():
    pytest.importorskip("tomllib")
    assert "REFUSED" not in fp.table()


def test_check_refuses_the_run_when_a_cell_is_refused(monkeypatch, capsys):
    monkeypatch.setattr(fp, "table", lambda: "O2\tow\tREFUSED model: ours medium, reference large")
    with pytest.raises(SystemExit, match="no references measured"):
        fp.check()
    monkeypatch.setattr(fp, "table", lambda: "O2\tow\tsame")
    fp.check()


def test_every_column_names_the_linker_that_produced_it(tmp_path, monkeypatch):
    """The data axis reads the linked image, so it depends on the linker: the table header says which one made each column from the files the bench
    runs (an absent one reads `absent`, never a guess from a map's format)."""
    import dosbatch

    (tmp_path / "jwlink").write_bytes(b"j")
    monkeypatch.setattr(dosbatch, "BIN", tmp_path)
    monkeypatch.setattr(dosbatch, "QB45", tmp_path / "none")
    found = fp.linkers()
    assert found["c,nib,ow,bcc,tc,tcpp"].startswith("jwlink ") and "absent" not in found["c,nib,ow,bcc,tc,tcpp"]
    assert found["bas,bq16,bc"] == "qb45 LINK.EXE absent"


def test_a_reference_built_without_the_physical_addresses_ours_has_is_refused():
    """bench/textfill reads PHYSICAL_ addresses the platform names: a reference compiled without them runs another program."""
    ours = fp.ours_profile(["-O2", "-fno-inline-functions"], ("-DPHYSICAL_VGA=0xA0000UL",))
    with pytest.raises(SystemExit, match="defines"):
        fp.refuse(ours, fp.ow_profile(OW), "textfill ow -O2")
    fp.refuse(ours, fp.ow_profile([*OW, "-DPHYSICAL_VGA=0xA0000UL"]), "textfill ow -O2")


def test_borland_byte_packing_against_ours_is_a_documented_difference_not_a_refusal():
    refused, documented = fp.parity(OURS, fp.borland_profile("bcc", ["-O1"]))
    assert refused == [] and any(one.startswith("packing:") for one in documented)
    assert fp.borland_profile("bcc", ["-O1", "-a"])["packing"] == 2
