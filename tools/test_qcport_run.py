"""`uv run --project tools python -m pytest tools/test_qcport_run.py`"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import qcport_run  # noqa: E402

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


def test_the_toolchain_files_are_found_where_the_toolchain_is_here_not_where_the_listing_was_made():
    """CI's qcport step stopped at `cannot open /home/alim/scratch/qctc/bcpp31/LIB/C0M.OBJ`: the Borland build's listing names the
    start-up and libraries by the development host's path."""
    lines = qcport_run.linked(LISTING, Path("/work/side"), [], Path("/runner/bcpp31/lib")).splitlines()
    assert "file '/runner/bcpp31/lib/C0M.OBJ'" in lines
    assert [line for line in lines if line.startswith("library")] == ["library '/runner/bcpp31/lib/FP87.LIB'", "library '/runner/bcpp31/lib/MATHM.LIB'", "library '/runner/bcpp31/lib/CM.LIB'"]
    assert "file '/work/side/bench.obj'" in lines


def test_a_run_that_drew_nothing_says_what_the_emulator_and_the_program_last_said(tmp_path):
    """CI's qcport said `a run drew nothing` for both builds and the work directory was gone with the runner: nothing to read."""
    (tmp_path / "dosbox.out").write_text("\n".join(f"line {n}" for n in range(30)))
    got = qcport_run.tails(tmp_path, lines=3)
    assert got[1:] == ["    line 27", "    line 28", "    line 29"] and "dosbox.out" in got[0]
    assert "no output at all" in qcport_run.tails(tmp_path / "..")[0]


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


def test_a_compiler_that_dies_silently_is_reported_with_its_exit_status_and_signal(tmp_path):
    """Two gates failed 'd_faces: ' and 'sc: ' with nothing after the colon: no exit status, so no telling a crash from a kill."""
    killed = tmp_path / "killed"
    killed.write_text("#!/bin/sh\nkill -9 $$\n")
    killed.chmod(0o755)
    message = qcport_run.compile_failure(killed, ["-O2"], Path("d_faces.c"), tmp_path / "d_faces.obj")
    assert message.startswith("d_faces: ") and "exit -9" in message and "SIGKILL" in message, message


def test_a_compiler_that_succeeds_is_no_failure(tmp_path):
    fine = tmp_path / "fine"
    fine.write_text("#!/bin/sh\nexit 0\n")
    fine.chmod(0o755)
    assert qcport_run.compile_failure(fine, [], Path("a.c"), tmp_path / "a.obj") is None


def fake_qcport(tmp_path, body):
    """A QCport whose tools/run.sh is `body`, run in the directory it is given."""
    script = tmp_path / "tools" / "run.sh"
    script.parent.mkdir()
    script.write_text("#!/bin/bash\ncd \"$1\"\n" + body)
    script.chmod(0o755)
    (tmp_path / "src").mkdir()
    work = tmp_path / "work"
    work.mkdir()
    return tmp_path / "src", work


def test_a_run_slow_on_the_wall_that_keeps_drawing_finishes():
    """'a run did not finish in 30s' failed a gate at load 90 and passed on the rerun: the limit measured the host."""
    import tempfile
    with tempfile.TemporaryDirectory() as raw:
        src, work = fake_qcport(Path(raw), "for i in 1 2 3 4 5 6 7 8; do echo step >> dosbox.log; sleep 0.4; done\n")
        # the stall is 50 steps of the script: a host at load 300 stretches a 0.4 s sleep to a few seconds, not to 20
        assert qcport_run.run(work, src, stall=20.0, cap=120, tick=0.1) is None


def test_a_run_that_stops_stepping_is_stopped_however_the_host_is_loaded():
    import tempfile
    with tempfile.TemporaryDirectory() as raw:
        src, work = fake_qcport(Path(raw), "echo step >> dosbox.log\nsleep 600\n")
        assert "no progress" in qcport_run.run(work, src, stall=1.0, cap=60, tick=0.1)


def test_a_run_that_steps_forever_is_stopped_by_the_cap():
    import tempfile
    with tempfile.TemporaryDirectory() as raw:
        src, work = fake_qcport(Path(raw), "while true; do echo step >> dosbox.log; sleep 0.05; done\n")
        assert "after 2s" in qcport_run.run(work, src, stall=30, cap=2, tick=0.1)


def _attract_run(tmp_path, steps, **logs):
    (tmp_path / "cstep.txt").write_text(steps)
    for name, text in logs.items():
        (tmp_path / name).write_text(text)
    return tmp_path


PLAYED = "attract demo demo1.qdc map e1m3\r\nattract demo demo2.qdc map e1m4\r\nattract demo demo3.qdc map e1m6\r\n"


def test_an_attract_loop_that_starts_all_three_demos_has_no_problem(tmp_path):
    run = _attract_run(tmp_path, PLAYED)
    assert qcport_run.attract_problems(run) == []
    assert qcport_run.attract_over(run)


def test_an_attract_loop_that_runs_out_of_memory_after_one_demo_is_named_with_the_line(tmp_path):
    """QCport -O2 at b9e7400be drew the benchmark (300 ticks) and failed the empty command line: 329,553 bytes of code against Borland's 296,085, and `miptex.bin: out of memory` loading the third demo's level."""
    run = _attract_run(tmp_path, "attract demo demo1.qdc map e1m3\n", **{"ERROR.LOG": "miptex.bin: out of memory\n"})
    problems = qcport_run.attract_problems(run)
    assert problems == ["never started: demo2.qdc demo3.qdc", "miptex.bin: out of memory"]
    assert qcport_run.attract_over(run), "an out-of-memory run is over, not left to the stall timer"


def test_an_attract_loop_still_on_its_second_demo_is_not_over(tmp_path):
    run = _attract_run(tmp_path, "attract demo demo1.qdc map e1m3\nattract demo demo2.qdc map e1m4\n")
    assert qcport_run.attract_problems(run) == ["never started: demo3.qdc"]
    assert not qcport_run.attract_over(run)


def test_demos_the_emulator_logged_count_before_run_sh_has_written_cstep(tmp_path):
    """run.sh writes cstep.txt when the emulator exits; a run stopped while it plays has only dosbox.log. The first attract run read cstep.txt, found it missing and called Borland's good run 'never started'."""
    (tmp_path / "dosbox.log").write_text("Bochs port E9h: " + PLAYED.replace("\r\n", "\nBochs port E9h: "))
    assert qcport_run.attract_problems(tmp_path) == []


def test_the_attract_verdict_names_each_run_that_misses_a_demo():
    """The attract loop must play all three demos on both builds; QCport -O2 once ran out of memory loading miptex.bin and passed the benchmark."""
    bad = ["never started: demo2.qdc demo3.qdc", "miptex.bin: out of memory"]
    assert qcport_run.attract_verdict({"borland": [], "llrm": []}) == []
    assert qcport_run.attract_verdict({"borland": [], "llrm": bad}) == [f"llrm: {'; '.join(bad)}"]
    assert qcport_run.attract_verdict({"borland": ["never started: demo1.qdc"], "llrm": []}) == ["borland: never started: demo1.qdc"]


def test_a_run_that_logs_a_fault_without_end_cannot_fill_the_disk():
    """A faulting run looped on CPU exception 6 and wrote a 57 GB dosbox.log before it was stopped."""
    import tempfile
    with tempfile.TemporaryDirectory() as raw:
        src, work = fake_qcport(Path(raw), "head -c 8388608 /dev/zero >> dosbox.log\n")
        qcport_run.run(work, src, stall=30, cap=30, tick=0.1, file_limit=1 << 20)  # the script writes 8 MiB
        assert (work / "dosbox.log").stat().st_size <= 1 << 20
