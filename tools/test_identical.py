"""identical.py fails where its corpus is smaller than the known one, instead of saying 'identical' of what is left."""
import importlib.util
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("identical", HERE / "identical.py")
identical = importlib.util.module_from_spec(spec)
spec.loader.exec_module(identical)


def test_the_programs_are_the_known_number(tmp_path):
    assert len(identical.corpus(tmp_path, None, None, programs_only=True)) >= identical.KNOWN_PROGRAMS


def test_without_qcport_it_fails_instead_of_skipping_it(tmp_path):
    """compile-cost.py's list drops QCport's modules when QCPORT is unset; an identity check on it compared 66 files and said so of 131."""
    with pytest.raises(SystemExit, match="QCPORT"):
        identical.corpus(tmp_path, None, None)


def test_a_qcport_with_fewer_modules_than_known_fails(tmp_path):
    (tmp_path / "host").mkdir()
    for n in range(3):
        (tmp_path / "host" / f"m{n}.c").write_text("int x;\n")
    with pytest.raises(SystemExit, match="QCport modules"):
        identical.corpus(tmp_path / "work", tmp_path, tmp_path, False) if (tmp_path / "work").mkdir() is None else None


def test_the_known_corpus_is_all_of_it(tmp_path):
    """The full corpus is programs + modules, each once: a name in both lists would silently shrink it."""
    import os
    qcport, headers = os.environ.get("QCPORT"), os.environ.get("QCPORT_INC")
    if not (qcport and headers):
        pytest.skip("no QCport")
    files = identical.corpus(tmp_path, Path(qcport).expanduser(), Path(headers).expanduser())
    assert len(files) >= identical.KNOWN_PROGRAMS + identical.KNOWN_QCPORT


def test_levels_given_on_the_command_line_are_not_read_as_options(tmp_path, monkeypatch):
    """`--levels -O2 -Os` was an argparse error (a value that starts with '-'), and a walk over forty commits printed the usage line forty
    times where it meant to say 'identical'."""
    import subprocess
    import sys
    fake = tmp_path / "llrm-c"
    fake.write_text('#!/bin/sh\nwhile [ "$1" != -o ]; do shift; done\necho x > "$2"\n')
    fake.chmod(0o755)
    done = subprocess.run([sys.executable, str(HERE / "identical.py"), str(fake), str(fake), "--levels=-O2,-Os", "--programs-only", "--no-bench"], capture_output=True, text=True, env={**__import__("os").environ, "CARGO_TARGET_DIR": str(tmp_path)})
    assert "usage:" not in done.stderr and "objects compared" in done.stdout, done.stderr[-300:] + done.stdout[-300:]


def test_every_oracle_in_the_source_is_run_on_the_corpus_but_those_named_not_run(tmp_path):
    """LLRM_CHECK_BYTES and LLRM_CHECK_GROUPS (gvn, #1342) were merged but not in identical.CHECKS: 524/0 was said of a run that never
    asked them. CHECKS is the source's oracles less NOT_RUN, so a new oracle is run with no edit (the list each PR appended to conflicted)."""
    import re

    names = {m for path in (HERE.parent / "crates").rglob("*.rs") for m in re.findall(r'"LLRM_CHECK_([A-Z_0-9]+)"', path.read_text())}
    assert set(identical.CHECKS) == names - identical.NOT_RUN
    assert identical.NOT_RUN <= names, sorted(identical.NOT_RUN - names)  # a name left over from a removed oracle
    crate = tmp_path / "crates/x/src"
    crate.mkdir(parents=True)
    (crate / "lib.rs").write_text('fn a() { env_set("LLRM_CHECK_BRANDNEW"); env_set("LLRM_CHECK_COLORS"); }\n')
    assert identical.checks(tmp_path) == ("BRANDNEW",), "a new oracle is run, one named NOT_RUN is not"


def test_the_run_programs_are_in_the_corpus_with_their_own_flags(tmp_path):
    """tests/run/c/i64_param_home.c -O2 was miscompiled by a kept result and passed identical.py with REPLAY (544 objects): the corpus did not
    hold the programs the gate's run tests do."""
    files = identical.corpus(tmp_path, None, None, programs_only=True)
    source, flags = files["run/i64_param_home#0"]
    assert source.name == "i64_param_home.c" and flags == ["-O2", "-march=i486", "-m32"]
    assert "run/i64_param_home#1" in files


def _repo(tmp_path):
    import subprocess

    for args in (["init", "-q"], ["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", "x"]):
        subprocess.run(["git", "-C", str(tmp_path), *args], check=True)


def test_a_binary_older_than_its_head_is_stale(tmp_path):
    """A baseline whose build failed silently was the previous main's binary: identical.py compared against it and said 'identical' of 544."""
    import os
    import time

    _repo(tmp_path)
    binary = tmp_path / "llrm-c"
    binary.write_text("x")
    assert identical.stale(str(binary), "HEAD", "OLD", tmp_path) is None
    os.utime(binary, (time.time() - 3600, time.time() - 3600))
    assert "stale build" in identical.stale(str(binary), "HEAD", "OLD", tmp_path)
    assert "does not exist" in identical.stale(str(tmp_path / "none"), "HEAD", "OLD", tmp_path)
    assert "no such revision" in identical.stale(str(binary), "nosuchrev", "OLD", tmp_path)


def _bench_or_skip(tmp_path):
    import os

    if not os.environ.get("LLRM_BIN") and not os.environ.get("CARGO_TARGET_DIR") and not (HERE.parent / "target/release/llrm-c").exists():
        pytest.skip("no built binaries")
    try:
        return identical.bench_compiles(tmp_path)
    except (ImportError, FileNotFoundError) as error:
        pytest.skip(f"bench harness unavailable: {error}")
    except SystemExit as error:  # only the binaries: a corpus smaller than known is the failure this test is for
        if "binaries" not in str(error):
            raise
        pytest.skip(str(error))


def test_the_bench_programs_are_compiled_with_the_harnesss_flags(tmp_path):
    """The corpus compiled bench programs with default flags while the harness gives -fno-inline-functions (and builds the C rows through a
    volatile pointer): #1883 said 'identical objects' of rows it never compiled."""
    found = _bench_or_skip(tmp_path)
    tool, arguments, _ = found["bench/frames/bq16/O2#0"]
    assert tool == "llrm-qb" and "-fno-inline-functions" in arguments and "-O2" in arguments
    assert any(name.startswith("bench/frames/c/O2#1") for name in found), "the C kernel's counted twin"
    assert {name.split("/")[2] for name in found} >= {"bas", "bq16", "bq32", "c", "nib"}, "every language"


def test_a_change_to_one_bench_row_alone_fails_the_check(tmp_path):
    """A compiler that differs only for frames.bas at -O2 -fno-inline-functions (the shape of #1883's change) is 'different' on the bench row
    and was 'identical' on the same program compiled with default flags, which is all the corpus held."""
    def compiler(directory, marker):
        directory.mkdir()
        for tool in ("llrm-c", "llrm-qb"):
            script = directory / tool
            script.write_text(
                '#!/bin/sh\nout=""; hit=0; case "$*" in *frames.bas*-O2*-fno-inline-functions*) hit=1;; esac\n'
                'while [ $# -gt 0 ]; do [ "$1" = -o ] && out=$2; shift; done\n'
                f'if [ $hit = 1 ]; then echo {marker} > "$out"; else echo same > "$out"; fi\n'
            )
            script.chmod(0o755)
        return str(directory / "llrm-c")

    old, new = compiler(tmp_path / "old", "a"), compiler(tmp_path / "new", "b")
    flags = ["-O2", "-fno-inline-functions"]
    bench = ("bench/frames/bq16/O2#0", "", "llrm-qb", ["frames.bas", *flags, "-o", "OBJ"])
    default = ("vsgcc/frames", "-O2", "llrm-qb", ["-O2", "frames.bas", "-o", "OBJ"])
    assert identical.same(bench, old, new, tmp_path)[2] == "different"
    assert identical.same(default, old, new, tmp_path)[2] is None


def test_two_llrm_llc_builds_are_compared_through_the_llrm_cc_beside_each(tmp_path):
    """identical.py ran `llrm-llc -m32 ... x.c -o OBJ` (a C command line to a backend that reads IR) and called every compile failed: the PGO job's
    identity check had no way to name llrm-llc. Each side must be llrm-cc with LLRM_LLC set to that side's backend and a row for the mode."""
    import os

    for tag in ("old", "new"):
        side = tmp_path / tag
        side.mkdir()
        (side / "llrm-llc").write_text("#!/bin/sh\n")
        cc = side / "llrm-cc"
        cc.write_text(f'#!/bin/sh\nwhile [ "$1" != -o ]; do shift; done\necho "{tag} $LLRM_LLC $LLRM_CC_TARGET" > "$2"\n')
        for one in (side / "llrm-llc", cc):
            one.chmod(0o755)
    src = tmp_path / "x.c"
    src.write_text("int f(void) { return 0; }\n")
    work = tmp_path / "w"
    work.mkdir()
    name, _, why = identical.same(("p", "-O2", "llrm-c", ["-m32", "-O2", str(src), "-o", "OBJ"]), str(tmp_path / "old/llrm-llc"), str(tmp_path / "new/llrm-llc"), work)
    assert why == "different"
    assert (work / "p-O2.old.obj").read_text() == f"old {tmp_path}/old/llrm-llc x86_32-wat\n"
    assert (work / "p-O2.new.obj").read_text() == f"new {tmp_path}/new/llrm-llc x86_32-wat\n"


def test_a_bench_c_program_is_compiled_for_the_target_the_harness_builds_it_for(tmp_path):
    """48 of the pgo job's 130 'failures' were 8 bench C programs (huge, lru, jpeg_*, ...) that did not compile: the harness runs llrm-cc with
    LLRM_CC_TARGET=x86_16-wat, identical.py ran it without, so `__huge` and llrm_os.h were errors. The same row must reach both sides, and
    llrm-cc must run over the llrm-llc under test, not the one beside it."""
    for tag in ("old", "new"):
        side = tmp_path / tag
        side.mkdir()
        (side / "llrm-llc").write_text("#!/bin/sh\n")
        cc = side / "llrm-cc"
        cc.write_text(f'#!/bin/sh\nwhile [ "$1" != -o ]; do shift; done\necho "{tag} $LLRM_LLC $LLRM_CC_TARGET" > "$2"\n')
        for one in (side / "llrm-llc", cc):
            one.chmod(0o755)
    work = tmp_path / "w"
    work.mkdir()
    job = ("bench/lru/c/O2#0", "", "llrm-cc", ["x.c", "-c", "-o", "OBJ"], {"LLRM_CC_TARGET": "x86_16-wat"})
    identical.same(job, str(tmp_path / "old/llrm-llc"), str(tmp_path / "new/llrm-llc"), work)
    assert (work / "bench_lru_c_O2#0.old.obj").read_text() == f"old {tmp_path}/old/llrm-llc x86_16-wat\n"
    assert (work / "bench_lru_c_O2#0.new.obj").read_text() == f"new {tmp_path}/new/llrm-llc x86_16-wat\n"
