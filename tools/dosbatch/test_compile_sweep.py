from pathlib import Path
import compile_sweep
import run_tests


def program(name: str) -> run_tests.Program:
    return run_tests.Program(Path(name), ["-O2"], None)


def test_a_language_the_sweep_cannot_build_is_listed_not_dropped():
    """The sweep went through `run_tests.compile_command`, which has no command for .nib: 235 programs (940 jobs) failed
    to start and were read as 'did not build', and the class they held was read as one program's."""
    jobs, skipped = compile_sweep.plan([program("a.c"), program("b.nib"), program("c/c.xyz")])
    assert [job.source.name for job in jobs] == ["a.c"] * 4 + ["b.nib"] * 4
    assert skipped == {"no compile path for .xyz in this sweep": ["c/c.xyz"]}


def test_the_header_names_what_was_skipped_and_how_many_jobs_built():
    text = compile_sweep.header(8, 8, {"no compile path for .xyz in this sweep": ["c/c.xyz"]})
    assert text.splitlines() == ["compiled 8 of 8 jobs", "skipped 1 programs: no compile path for .xyz in this sweep (first: c/c.xyz)"]
