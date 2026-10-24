"""`c_include` is asked by every C program of a run, from the threads that compile them."""

import sys
import threading
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dosbatch  # noqa: E402


def test_a_header_is_never_seen_half_written_by_a_program_compiling_while_another_asks(monkeypatch, tmp_path):
    """The 23:00 sweep of 2026-10-24 linked `_llrm_os_close@3`, an undefined reference, from bench_io, the first program (T000) of the run: it
    compiled while another thread's `c_include` rewrote `llrm_os.h` in place, an empty header declares nothing, and `llrm_os_close` was
    an implicit function without the `#pragma aux` that names it LL$CLOSE. The retained T000.obj had the unmapped names; T001, the same
    program for the other target, had them mapped."""
    header = "/* the OS layer */\n" + "#define PAD 1234567890\n" * 400_000
    monkeypatch.setattr(dosbatch, "os_layer", lambda target, field, language: header[:-1])
    threads, trials = 16, 30
    torn = 0
    for trial in range(trials):
        work = tmp_path / f"trial{trial}"
        work.mkdir()
        start, seen = threading.Barrier(threads), []

        def program():
            start.wait()
            directory = dosbatch.c_include("x86-m16", work)
            # what the compiler reads once it has the directory
            seen.append((directory / "llrm_os.h").read_text() == header)

        pool = [threading.Thread(target=program) for _ in range(threads)]
        for one in pool:
            one.start()
        for one in pool:
            one.join()
        torn += seen.count(False)
    assert torn == 0, f"{torn} reads of {threads * trials} saw a header another thread was still writing"
