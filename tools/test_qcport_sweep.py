"""The QCport sweep: a failure is grouped by its kind, only a kind no known entry covers is new, and the bisection finds the first bad squash."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import qcport_sweep as q  # noqa: E402

KNOWN = [{"match": "with no instruction choosing", "issue": 2454}]


def test_kinds_drop_the_ids_that_differ_per_module():
    """menu (block 139) and triggers (block 73) are one bug: their messages differ in function suffixes and ids only."""
    a = q.kind("llrm-c: @_menu_key@3: _menu_key@3: block 139 leaves for (133, 150, 156) with no instruction choosing")
    b = q.kind("llrm-c: @_ent_lightning@3: _ent_lightning@3: block 73 leaves for (20, 77, 75) with no instruction choosing\n")
    assert a == b and a.endswith("with no instruction choosing")
    assert q.kind("llrm-c: a jump to L1_125, which is nowhere") == q.kind("llrm-c: a jump to L2_7, which is nowhere") == "llrm-c: a jump to L, which is nowhere"


def test_only_a_kind_no_known_entry_covers_is_new():
    groups = q.grouped([("menu", "-O2", q.kind("x: block 1 leaves for (2) with no instruction choosing")), ("vid", "-Os", "llrm-c: a byte division")])
    assert list(q.new_kinds(groups, KNOWN)) == ["llrm-c: a byte division"]
    assert q.new_kinds(groups, KNOWN + [{"match": "a byte division", "issue": 2456}]) == {}


def test_the_first_bad_commit_of_a_single_flip_is_found_in_log_steps():
    commits = [f"c{i}" for i in range(100)]
    asked = []

    def bad(c):
        asked.append(c)
        return int(c[1:]) >= 37

    assert q.first_bad(commits, bad) == "c37" and len(asked) <= 7


def test_the_issue_names_the_repro_the_head_and_the_first_bad_squash():
    title, body = q.issue_text("llrm-c: a byte division", [("vid", "-O2"), ("vid", "-Os")], "a" * 40, ("b" * 40, "feat(x): y (#1)"))
    assert "vid -O2" in title and "aaaaaaaaa" in body and "-I $QCPORT" in body and "bbbbbbbbb feat(x): y (#1)" in body


def test_the_known_file_parses_and_names_issue_numbers():
    import tomllib

    assert all(isinstance(e["issue"], int) and e["match"] for e in tomllib.loads(q.KNOWN.read_text()).get("known", []))


def test_the_front_end_compiles_cover_nib_and_basic_at_both_machines_and_all_levels():
    """The smoke's `programs` step caught a Nib -m32 panic (#2531) that no QCport module compiles."""
    benches = [j for j in q.extra_jobs() if j[1].parent.name == "bintree"]
    assert len(benches) == 2 * 2 * len(q.LEVELS) and {t for t, *_ in benches} == {"llrm-nib", "llrm-qb"} and {m for _, _, m, _ in benches} == {"-m16", "-m32"}


def test_a_panic_is_keyed_by_its_place_and_message_not_by_rusts_backtrace_note():
    text = "thread 'main' (3957823) panicked at crates/opt/llrm-transforms/src/widen.rs:142:39:\nan instruction with a value\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n"
    assert q.kind(text) == "panic crates/opt/llrm-transforms/src/widen.rs:N:N: an instruction with a value"


def test_a_commit_that_does_not_build_is_passed_over_not_a_crash():
    """The bisection of the 48-compile BitCast failure died on `CalledProcessError`: feat/parity did not build for three hours (#2544 with #2564), so the
    middle commit of the window had no llrm-c. The first bad commit that builds is the answer."""
    commits = [f"c{i}" for i in range(20)]
    unbuildable = {f"c{i}" for i in range(8, 13)}

    def bad(commit):
        if commit in unbuildable:
            return None
        return int(commit[1:]) >= 15

    assert q.first_bad(commits, bad) == "c15"
    assert q.first_bad(commits, lambda c: None if c in unbuildable else int(c[1:]) >= 10) == "c13"


def test_the_sweep_compiles_the_basic_corpus_at_m16_and_m32_and_never_a_64_bit_machine(tmp_path):
    """ownf_local_pds71 and ownf_local_vbdos failed at -m32 with `@TOP: ptrtoint` (140 compiles in a full-corpus run) while tests/run, BASIC on m16 only, reported 0 fail."""
    corpus = tmp_path / "tests/run/qb"
    corpus.mkdir(parents=True)
    for crate, mode in (("a", 16), ("b", 32)):
        (tmp_path / f"crates/target/{crate}/src/machines").mkdir(parents=True)
        (tmp_path / f"crates/target/{crate}/src/machines/datalayout.toml").write_text(f"mode = {mode}\n")
    (corpus / "flat.bas").write_text("PRINT 1\n")
    (corpus / "huge.bas").write_text("' targets: x86-m16\nPRINT 2\n")
    jobs = q.extra_jobs(tmp_path)
    assert sorted((s.name, m, l) for t, s, m, l in jobs if s.parent == corpus) == [("flat.bas", m, l) for m in ("-m16", "-m32") for l in ("-O1", "-O2")]
    assert {t for t, *_ in jobs} == {"llrm-nib", "llrm-qb"}


def test_a_64_bit_machine_is_never_swept(tmp_path):
    """The 64-bit targets have no Nib runtime and OMF has no 64-bit records: 238 of a sweep's failed compiles were `-m64`, none a finding."""
    for crate, mode in (("a", 16), ("b", 32), ("c", 64), ("d", 64)):
        (tmp_path / f"crates/target/{crate}/src/machines").mkdir(parents=True)
        (tmp_path / f"crates/target/{crate}/src/machines/datalayout.toml").write_text(f"mode = {mode}\n")
    (tmp_path / "tests/run/qb").mkdir(parents=True)
    (tmp_path / "tests/run/qb/flat.bas").write_text("PRINT 1\n")
    assert {m for _, _, m, _ in q.extra_jobs(tmp_path)} == {"-m16", "-m32"}


def test_a_program_is_swept_with_its_own_header_flags_and_a_by_design_refusal_is_not_a_failure(tmp_path):
    """hugelp and hugerg (`--huge-arrays`, pds71) were swept bare and failed with a backend selection error, and the m16-only far_* programs failed at -m32 with the policy
    refusal: both read as new compile failures. Seven kinds, none a finding, two of them stop-the-line rows."""
    for crate, mode in (("a", 16), ("b", 32)):
        (tmp_path / f"crates/target/{crate}/src/machines").mkdir(parents=True)
        (tmp_path / f"crates/target/{crate}/src/machines/datalayout.toml").write_text(f"mode = {mode}\n")
    corpus = tmp_path / "tests/run/qb"
    corpus.mkdir(parents=True)
    (corpus / "hugelp.bas").write_text("' dialect: pds71\n' flags: -O2 --huge-arrays\nPRINT 1\n")
    (corpus / "far_poke.bas").write_text("PRINT 2\n")
    (corpus / "broken.bas").write_text("PRINT 3\n")
    bins = tmp_path / "bins"
    bins.mkdir()
    fake = bins / "llrm-qb"
    fake.write_text(
        "#!/bin/sh\n"
        'case "$1" in\n'
        '  *hugelp*) case "$*" in *--huge-arrays*) exit 0;; *) echo "Semantics(op=MOVE) failed to select" >&2; exit 1;; esac;;\n'
        '  *far_poke*) case "$*" in *-m32*) echo "DEF SEG with a non-constant segment is not supported at one address space" >&2; exit 1;; *) exit 0;; esac;;\n'
        '  *broken*) echo "internal error" >&2; exit 1;;\n'
        "esac\nexit 0\n"
    )
    fake.chmod(0o755)
    (bins / "llrm-nib").write_text("#!/bin/sh\nexit 0\n")
    (bins / "llrm-nib").chmod(0o755)
    failed = q.sweep_extra(bins, root=tmp_path)
    assert sorted({program for program, _, _ in failed}) == ["broken.bas"], failed


def test_a_program_whose_header_makes_it_real_mode_only_is_not_compiled_at_m32(tmp_path):
    """farchk, hugelp, intrinsics_vbdos, stackdeep and their kin failed at -m32 with three messages (relocation overlaps, a far call in flat code) that were policy, not findings."""
    for crate, mode in (("a", 16), ("b", 32)):
        (tmp_path / f"crates/target/{crate}/src/machines").mkdir(parents=True)
        (tmp_path / f"crates/target/{crate}/src/machines/datalayout.toml").write_text(f"mode = {mode}\n")
    corpus = tmp_path / "tests/run/qb"
    corpus.mkdir(parents=True)
    (corpus / "flat.bas").write_text("PRINT 1\n")
    (corpus / "far.bas").write_text("' dialect: pds71\nPRINT 2\n")
    (corpus / "huge.bas").write_text("' flags: -O2 --huge-arrays\nPRINT 3\n")
    (corpus / "frames.bas").write_text("' flags: -O2 --own-frames -fsanitize=stack\nPRINT 4\n")
    (corpus / "vb.bas").write_text("' dialect: qb45 vbdos\nPRINT 5\n")
    (corpus / "ms_runtime.bas").write_text("' flags: -O1 | -O2\n' dialect: qb45\nPRINT 6\n")
    machines = {}
    for tool, source, machine, level in q.extra_jobs(tmp_path):
        if source.parent == corpus:
            machines.setdefault(source.name, set()).add(machine)
    assert machines == {"flat.bas": {"-m16", "-m32"}, "far.bas": {"-m16"}, "huge.bas": {"-m16"}, "frames.bas": {"-m16"}, "vb.bas": {"-m16"}, "ms_runtime.bas": {"-m16"}}, machines


def test_the_header_names_the_sha_the_binary_was_built_from_not_the_ref_at_print_time(tmp_path, monkeypatch, capsys):
    """origin/feat/parity moved during a sweep and the header named 864f2b3fa for a binary built from 50a88a94b."""
    binary = tmp_path / "llrm-c"
    binary.write_text("#!/bin/sh\necho llrm-c " + "a" * 40 + "\n")
    binary.chmod(0o755)
    moved = "f" * 40
    real_run = q.subprocess.run

    def run(cmd, *args, **kwargs):
        if cmd[:2] == ["git", "rev-parse"]:
            return real_run(["echo", moved], *args, **kwargs)
        return real_run(cmd, *args, **kwargs)

    monkeypatch.setattr(q.subprocess, "run", run)
    monkeypatch.setattr(q, "sweep", lambda *a, **k: [])
    monkeypatch.setattr(q, "sweep_extra", lambda *a, **k: [])
    monkeypatch.setattr(sys, "argv", ["qcport_sweep.py", "--bin", str(tmp_path)])
    q.main()
    header = capsys.readouterr().out.splitlines()[0]
    assert "aaaaaaaaa" in header and "fffffffff" not in header


def test_a_binary_with_no_stamp_is_refused(tmp_path, monkeypatch):
    binary = tmp_path / "llrm-c"
    binary.write_text("#!/bin/sh\necho usage: llrm-c [-h] source\n")
    binary.chmod(0o755)
    monkeypatch.setattr(sys, "argv", ["qcport_sweep.py", "--bin", str(tmp_path)])
    try:
        q.main()
    except SystemExit as why:
        assert "no build stamp" in str(why)
    else:
        raise AssertionError("an unstamped binary was swept")


def test_the_callconv_c_fixtures_are_in_the_front_end_compile_set():
    """An interrupt handler with Borland's nine register parameters was refused from 10-19 to 10-23 (`a fact of no parameter`): no sweep compiled a C file of that kind."""
    jobs = q.extra_jobs()
    callconv = [job for job in jobs if job[0] == "llrm-c"]
    assert {job[1].name for job in callconv} >= {"cfcallee.c", "cncallee.c"}
    assert {job[2:] for job in callconv} == {("-m16", "-O1"), ("-m16", "-O2")}


def test_a_c_fixture_is_compiled_with_the_header_directory(tmp_path):
    """The callconv harnesses include <stdio.h>: swept without `-I`, five files at two levels failed (10 failed compiles, one new kind) at 526f94a61."""
    for crate, mode in (("a", 16), ("b", 32)):
        (tmp_path / f"crates/target/{crate}/src/machines").mkdir(parents=True)
        (tmp_path / f"crates/target/{crate}/src/machines/datalayout.toml").write_text(f"mode = {mode}\n")
    fixtures = tmp_path / "tests/fixtures/callconv/c"
    fixtures.mkdir(parents=True)
    (fixtures / "harness.c").write_text('#include <stdio.h>\nint main(void) { return 0; }\n')
    (tmp_path / "tests/run/qb").mkdir(parents=True)
    bins = tmp_path / "bins"
    bins.mkdir()
    for tool in ("llrm-c", "llrm-qb", "llrm-nib"):
        (bins / tool).write_text('#!/bin/sh\ncase "$*" in *"-I /inc"*) exit 0;; esac\necho "Error! E1055: Unable to open stdio.h" >&2\nexit 1\n')
        (bins / tool).chmod(0o755)
    failed = q.sweep_extra(bins, root=tmp_path, include=Path("/inc"))
    assert "harness.c" not in {program for program, _, _ in failed}, failed


def test_a_failure_is_keyed_by_its_error_not_by_a_warning_printed_after_it():
    """The 22:00 sweep keyed `Unable to open 'stdio.h'` as `Warning! WN: No prototype found for function 'printf'`: the last line was a warning."""
    text = "wccq failed:\nx.c(3): Error! E1055: Unable to open 'stdio.h'\nx.c(36): Warning! W131: No prototype found for function 'printf'\n"
    assert q.kind(text) == "x.c(...): Error! EN: Unable to open 'stdio.h'"


def test_a_module_that_dies_only_at_emit_is_a_failed_compile(tmp_path):
    """The sweep compiled to /dev/null, where llrm-c writes assembly text and never encodes the object: ui/console.c died in the encoder on m16 -O1..-O3
    while the sweep read 0 failed of 768. The stand-in compiler dies when its output is an object, as llrm-c's encoder does."""
    compiler = tmp_path / "llrm-c"
    compiler.write_text(
        "#!/usr/bin/env python3\nimport sys\nout = sys.argv[sys.argv.index('-o') + 1]\n"
        "if out.endswith(('.o', '.obj')):\n    sys.exit('llrm-c: Semantics(op=BINARY, name=sub)')\n"
    )
    compiler.chmod(0o755)
    qcport = tmp_path / "qcport"
    for part in q.MODULES:
        (qcport / part).mkdir(parents=True)
    (qcport / "ui" / "console.c").write_text("int x;\n")
    failed = q.sweep(compiler, qcport, tmp_path, levels=("-O1",), jobs=1)
    assert [(module, level) for module, level, _ in failed] == [("console", "-O1")]


def test_qcport_console_encodes_at_every_level():
    """ui/console.c at m16 -O1..-Os died in the encoder on `sub dx, edi` (optLEAALU added dword registers under a word destination, #3955), found only once the sweep compiled to an object."""
    import os

    import pytest

    qcport = Path(os.environ.get("QCPORT", Path.home() / "scratch/qcport/src")).expanduser()
    include = Path(os.environ.get("QCPORT_INC", Path.home() / "scratch/qctc/inc")).expanduser()
    source = qcport / "ui" / "console.c"
    if not source.is_file() or not include.is_dir():
        pytest.skip("QCport is not on this host")
    compiler = q.llrmbin.bin_dir() / "llrm-c"
    if not compiler.exists():
        pytest.skip("llrm-c is not built (debt skip-qcport-console)")
    flags = [f for part in (*q.MODULES, "qgl") for f in ("-I", str(qcport / part))] + ["-I", str(include)]
    failed = {}
    for level in q.LEVELS:
        with q.object_file() as out:
            done = subprocess.run([str(compiler), level, *flags, str(source), "-o", out], capture_output=True, text=True)
            if done.returncode:
                failed[level] = done.stderr[-200:]
    assert not failed, failed
