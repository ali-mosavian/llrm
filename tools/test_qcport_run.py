"""`uv run --project tools python -m pytest tools/test_qcport_run.py`"""
import importlib.util
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location("qcport_run", Path(__file__).parent / "qcport-run.py")
qcport_run = importlib.util.module_from_spec(spec)
sys.modules["qcport_run"] = qcport_run
spec.loader.exec_module(qcport_run)

LISTING = """format dos
option quiet, map=/elsewhere/QCPORT.MAP
debug codeview
name /elsewhere/QCPORT.EXE
file '/chain/C0M.OBJ'
file '/elsewhere/bench.obj'
file '/elsewhere/b8span.obj'
library '/chain/LIB/CM.LIB'
library '/chain/LIB/MATHM.LIB'
library '/chain/LIB/FP87.LIB'
"""


def test_the_link_names_the_objects_beside_it_and_puts_the_libraries_in_the_order_that_runs():
    """CM.LIB first linked clean and stopped after sys_parse_args: three hours of an llrm-built QCport that 'does not run'."""
    text = qcport_run.linked(LISTING, Path("/work/side"), ["strlib.obj"])
    lines = text.splitlines()
    assert "file '/work/side/bench.obj'" in lines and "file '/work/side/strlib.obj'" in lines
    assert "file '/chain/C0M.OBJ'" in lines, "the startup stays the toolchain's"
    assert not any("/elsewhere/" in line for line in lines)
    assert [line for line in lines if line.startswith("library")] == ["library '/chain/LIB/FP87.LIB'", "library '/chain/LIB/MATHM.LIB'", "library '/chain/LIB/CM.LIB'"]
    assert lines[0] == "format dos" and "name /work/side/QCPORT.EXE" in lines


def test_the_same_drawing_is_no_difference():
    drawn = {"frames": "62", "polys": "37637", "md5": "ba64"}
    assert qcport_run.verdict(drawn, dict(drawn)) == []


def test_a_different_picture_or_polygon_count_is_a_difference():
    drawn = {"frames": "62", "polys": "37637", "md5": "ba64"}
    assert qcport_run.verdict(drawn, {**drawn, "md5": "ffff"}) == ["md5: Borland ba64, llrm ffff"]
    assert qcport_run.verdict(drawn, {**drawn, "polys": "37000"}) == ["polys: Borland 37637, llrm 37000"]


def test_a_run_that_drew_nothing_is_never_the_same_as_one_that_drew_nothing():
    """Two runs that both stopped before the first frame have equal, empty measurements."""
    assert qcport_run.verdict({"md5": ""}, {"md5": ""})


def test_measured_reads_the_bench_files(tmp_path):
    (tmp_path / "BENCH.TXT").write_text("frames 62\npolys 37637\ntris 107407\n")
    (tmp_path / "BENCH.BMP").write_bytes(b"BM")
    got = qcport_run.measured(tmp_path)
    assert (got["frames"], got["polys"]) == ("62", "37637") and len(got["md5"]) == 32
    assert qcport_run.measured(tmp_path / "missing").get("polys") is None
