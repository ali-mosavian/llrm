"""The 25 bench programs on the llrm QB runtime at both targets (jwlink; the flat one under DOS/32A): each prints what bench/NAME/NAME.out says.
The probes of tests/qbrt against BCOM45 on both targets are test_qbrt.py's, and the demos test_qbdemos.py's."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

import corpus  # noqa: E402
import qb32  # noqa: E402
import qbbuild  # noqa: E402
import qbruntime  # noqa: E402


@pytest.fixture(scope="module")
def found(tmp_path_factory):
    """Each bench program built for both targets and run, a session a target: {name: {target: (status, same as its .out)}}."""
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    work = tmp_path_factory.mktemp("qb32")
    objects = qbbuild.runtimes(work)
    out: dict[str, dict] = {name: {} for name in qbruntime.MILESTONE_ONE}
    jobs: dict[str, list] = {target: [] for target in qbbuild.TARGETS}
    wanted = {}
    for at, name in enumerate(qbruntime.MILESTONE_ONE):
        source = qbruntime.dosbatch.ROOT / "bench" / name / f"{name}.bas"
        for target, built in qbbuild.build(source, work / name, objects, tolerant=True).items():
            if isinstance(built, str):
                out[name][target] = (f"build: {built}", False)
                continue
            stem = f"P{at:03d}"
            jobs[target].append(qbruntime.dosbatch.Job(stem, "exe", built.exe, files=(*built.files, *qb32.data_files(source))))
            wanted[(target, stem)] = (name, (source.parent / f"{name}.out").read_bytes())
    for target, job_list in jobs.items():
        if not job_list:
            continue
        results = qb32.run_jobs(job_list, work / f"run_{target}")
        for job in job_list:
            name, want = wanted[(target, job.stem)]
            got = qbruntime.raw_output(work / f"run_{target}", job.stem)
            out[name][target] = (results[job.stem].status, got.replace(b"\r\n", b"\n") == want.replace(b"\r\n", b"\n"))
    return out


@pytest.mark.parametrize("name", qbruntime.MILESTONE_ONE)
def test_bench_program_prints_what_it_should_on_both_targets(found, name: str):
    if name == "grep":
        try:
            corpus.path("dickens")
        except corpus.Unavailable as error:
            pytest.skip(str(error))
    for target, (status, same) in found[name].items():
        assert status == "ok", f"{target}: {status}"
        assert same, target


FLAT_PROGRAMS = ["long_strings", "paint_queue"]


@pytest.fixture(scope="module")
def flat_found(tmp_path_factory):
    if not qbruntime.dosbatch.DOSBOX.exists():
        pytest.skip("DOSBox-X is unavailable")
    return qb32.flat_programs(FLAT_PROGRAMS, tmp_path_factory.mktemp("qb32flat"))


@pytest.mark.parametrize("name", FLAT_PROGRAMS)
def test_flat_only_program_prints_what_it_should(flat_found, name: str):
    assert flat_found[name] == ""


def test_two_runs_writing_one_cached_object_at_once_both_finish(tmp_path, monkeypatch):
    """Both runs copied to the cache's one shared `.tmp` name, the first renamed it away and the second's rename raised
    FileNotFoundError: gate 69's dos-slow failed on it while other runs shared ~/.cache/llrm/qb32-objs."""
    import shutil
    import threading

    monkeypatch.setattr(qb32, "OBJECTS", tmp_path / "cache")
    both_copied = threading.Barrier(2, timeout=10)
    real_copy = shutil.copy

    def copy_then_wait(source, target, *args, **kwargs):
        done = real_copy(source, target, *args, **kwargs)
        if str(target).endswith(".tmp"):
            both_copied.wait()
        return done

    monkeypatch.setattr(shutil, "copy", copy_then_wait)
    failures = []

    def run(name: str):
        obj = tmp_path / f"{name}.obj"
        obj.write_bytes(b"object")
        try:
            qb32.cached("one key", obj, lambda: None)
        except Exception as error:  # noqa: BLE001
            failures.append(error)

    threads = [threading.Thread(target=run, args=(name,)) for name in ("a", "b")]
    for one in threads:
        one.start()
    for one in threads:
        one.join()
    assert not failures, failures
    assert [path.suffix for path in (tmp_path / "cache").iterdir()] == [".obj"]


def test_the_extenders_vector_note_is_not_part_of_a_programs_output():
    """paint_queue sets INT 43h and DOS/32A prints a warning line at exit: the flat-only test compared it as output ("byte 15: want <end> got 68") and refused #5690."""
    note = b"DOS/32A warning (9003): real mode interrupt vector has been modified: INT 43h\r\n"
    assert qb32.without_extender_notes(b" 4  15  4  4 \r\n" + note) == b" 4  15  4  4 \r\n"
    assert qb32.without_extender_notes(b"DOS/32A warning (9004): other\r\n") == b"DOS/32A warning (9004): other\r\n"
