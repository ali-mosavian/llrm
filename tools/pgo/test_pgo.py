"""`uv run --project tools python -m pytest tools/pgo/test_pgo.py`"""

import sys
import json
import time
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import pytest  # noqa: E402
import pgo  # noqa: E402


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", "-C", str(root), "-c", "user.name=t", "-c", "user.email=t@t", *args], capture_output=True, text=True, check=True).stdout.strip()


def repo(tmp_path: Path) -> tuple[Path, str]:
    git(tmp_path, "init", "-q", "-b", "main")
    (tmp_path / "crates").mkdir()
    (tmp_path / "crates/a.rs").write_text("0")
    git(tmp_path, "add", "-A")
    git(tmp_path, "commit", "-q", "-m", "0")
    return tmp_path, git(tmp_path, "rev-parse", "HEAD")


def profile(directory: Path, commit: str, created: float) -> Path:
    directory.mkdir(exist_ok=True)
    (directory / "profile.profdata").write_bytes(b"x")
    (directory / "profile.json").write_text(json.dumps({"commit": commit, "created": created, "compiles": 1, "failed": 0}))
    return directory


def edit(root: Path, name: str) -> None:
    (root / "crates" / name).write_text(name)
    git(root, "add", "-A")
    git(root, "commit", "-q", "-m", name)


def test_the_training_corpus_leaves_out_what_is_measured():
    """The first PGO figure trained on d_faces and measured d_faces: a held-out program is the only honest one."""
    everything = {name for name, *_ in pgo.corpus()}
    assert {"lru", "bintree", "queens"} <= everything
    left = {name for name, *_ in pgo.corpus(exclude=frozenset({"lru", "bintree", "queens"}))}
    assert left == everything - {"lru", "bintree", "queens"}


def test_every_tests_run_c_program_is_trained_at_its_own_flags():
    jobs = [job for job in pgo.corpus() if job[2].parent.name == "c" and job[2].parent.parent.name == "run"]
    assert len({job[2] for job in jobs}) >= 90 and all(job[1] for job in jobs)
    header = next(line for line in (pgo.ROOT / "tests/run/c/bitfield_long_long.c").read_text().splitlines()[:3] if line.startswith("// flags:"))
    assert len([job for job in jobs if job[0] == "bitfield_long_long"]) == len(header.split("|"))


def test_a_compile_of_the_corpus_that_fails_fails_the_job_with_what_it_said(tmp_path):
    """The first CI run counted '63 failed' and went on to merge 3 profiles: QCport's modules need headers the runner lacks."""
    compiler = tmp_path / "cc"
    compiler.write_text('#!/bin/bash\n[[ "$*" == *bad.c* ]] && { echo "Error! E1055: Unable to open string.h" >&2; exit 1; }\nexit 0\n')
    compiler.chmod(0o755)
    good, bad = tmp_path / "good.c", tmp_path / "bad.c"
    ok, failed = pgo.train(compiler, [("good", ["-O2"], good, False), ("bad", ["-O2"], bad, False)])
    assert ok == 1 and len(failed) == 1 and "bad -O2: Error! E1055: Unable to open string.h" in failed[0]


def test_the_profile_directory_is_made_before_the_merge_writes_into_it(monkeypatch, tmp_path):
    """The first CI run: 'llvm-profdata merge failed: target/pgo-profile/profile.profdata: No such file or directory'."""
    merge = tmp_path / "llvm-profdata"
    merge.write_text('#!/bin/bash\n[ "$1" = merge ] && [ "$2" = -o ] && [ -d "$(dirname "$3")" ] && echo x > "$3"\n')
    merge.chmod(0o755)
    (tmp_path / "repo").mkdir()
    root, _ = repo(tmp_path / "repo")

    def instrumented(root, target, flags, binary="llrm-llc"):
        raw = Path(flags.split("=", 1)[1])
        (raw / "a.profraw").write_bytes(b"x")
        return subprocess.CompletedProcess([], 0, stdout="", stderr="")

    monkeypatch.setattr(pgo, "cargo", instrumented)
    monkeypatch.setattr(pgo, "train", lambda binary, jobs, driver=None: (1, []))
    monkeypatch.setattr(pgo, "profdata", lambda: merge)
    monkeypatch.setattr(pgo, "corpus", lambda *a, **k: [])
    pgo.make_profile(root, tmp_path / "out" / "profile", frozenset(), None)
    assert (tmp_path / "out" / "profile" / "profile.profdata").is_file() and (tmp_path / "out" / "profile" / "profile.json").is_file()


def test_each_bench_program_is_compiled_flat_and_in_16_bits_at_every_level():
    jobs = [job for job in pgo.corpus() if job[0] == "fib"]
    levels = len(pgo.tomllib.loads((pgo.HERE / "corpus.toml").read_text())["levels"])
    assert len(jobs) == 2 * levels and {"-m32" in job[1] for job in jobs} == {True, False}


def test_qcports_modules_are_trained_on_wrapped_and_only_when_it_is_there(tmp_path):
    (tmp_path / "render").mkdir()
    for name in ("d_faces.c", "d_alias.c"):
        (tmp_path / "render" / name).write_text("")
    jobs = [job for job in pgo.corpus(qcport=tmp_path) if job[3]]
    assert {job[0] for job in jobs} == {"d_faces", "d_alias"} and not [job for job in pgo.corpus() if job[3]]
    assert {job[0] for job in pgo.corpus(exclude=frozenset({"d_faces"}), qcport=tmp_path) if job[3]} == {"d_alias"}, "d_faces_norefresh is d_faces too: a prefix"


def test_no_profile_is_none_and_a_young_profile_of_the_same_source_is_fresh(tmp_path):
    root, commit = repo(tmp_path)
    assert pgo.stale(tmp_path / "none", root) == 2
    assert pgo.stale(profile(tmp_path / "p", commit, time.time()), root) == 0


def test_a_profile_is_stale_past_the_age_or_the_commits_to_the_compiler(tmp_path):
    """Stale means functions changed since have no profile (the build warns): reuse is for a few commits, not for ever."""
    root, commit = repo(tmp_path)
    old = profile(tmp_path / "old", commit, time.time() - (pgo.MAX_AGE_DAYS + 1) * 86400)
    assert pgo.stale(old, root) == 1
    young = profile(tmp_path / "young", commit, time.time())
    for number in range(pgo.MAX_COMMITS):
        edit(root, f"c{number}.rs")
    assert pgo.stale(young, root) == 0
    edit(root, "one-more.rs")
    assert pgo.stale(young, root) == 1


def test_a_commit_that_is_not_the_compilers_does_not_age_the_profile(tmp_path):
    root, commit = repo(tmp_path)
    young = profile(tmp_path / "young", commit, time.time())
    for number in range(pgo.MAX_COMMITS + 5):
        (root / f"doc{number}.md").write_text("x")
        git(root, "add", "-A")
        git(root, "commit", "-q", "-m", "doc")
    assert pgo.stale(young, root) == 0


def test_a_profile_from_a_commit_this_clone_lacks_is_stale(tmp_path):
    root, _ = repo(tmp_path)
    assert pgo.stale(profile(tmp_path / "p", "0" * 40, time.time()), root) == 1


def test_the_unmatched_functions_are_listed_from_the_builds_warnings():
    text = "warning: x: no profile data available for function _ZN3foo Hash = 1 up to 0 count discarded\nwarning: no profile data available for function _ZN3bar Hash = 2\n"
    assert pgo.WARNING.findall(text) == ["_ZN3foo", "_ZN3bar"]


def test_a_summary_names_the_profile_its_age_and_the_functions_without_data(tmp_path):
    root, commit = repo(tmp_path)
    text = pgo.summary([f"f{n}" for n in range(25)], profile(tmp_path / "p", commit, time.time()))
    assert commit[:12] in text and "25 functions have no profile data" in text and "never ran them" in text and "... and 5 more" in text


def test_the_profdata_must_be_the_one_of_rustcs_llvm(monkeypatch, tmp_path):
    """llvm-profdata 20 read 'raw profile version mismatch' on rustc 1.99's LLVM 23 profiles."""
    monkeypatch.setattr(pgo, "sh", lambda command, **kw: subprocess.CompletedProcess(command, 0, stdout=str(tmp_path) if "sysroot" in command else "host: x86_64-unknown-linux-gnu\n"))
    with pytest.raises(SystemExit, match="rustup component add llvm-tools"):
        pgo.profdata()
    found = tmp_path / "lib/rustlib/x86_64-unknown-linux-gnu/bin"
    found.mkdir(parents=True)
    (found / "llvm-profdata").write_text("")
    assert pgo.profdata() == found / "llvm-profdata"


def test_training_llrm_llc_runs_the_corpus_through_llrm_cc_with_the_instrumented_backend(monkeypatch, tmp_path):
    """The profile of llrm-llc comes from its own runs, driven by llrm-cc (LLRM_LLC names the instrumented one); the target is a row."""
    seen = []

    def fake(command, check=True, **kw):
        seen.append((command, kw.get("env", {})))
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr(pgo, "sh", fake)
    source = tmp_path / "a.c"
    source.write_text("int f(void) { return 1; }\n")
    llc, driver = tmp_path / "llrm-llc", tmp_path / "llrm-cc"
    ok, failed = pgo.train(llc, [("a", ["-m32", "-mabi=sysv", "-O2", "-march=i486"], source, False), ("a", ["-m16", "-Os"], source, False)], driver=driver)
    assert (ok, failed) == (2, [])
    # train() runs the jobs on a thread pool: the order of `seen` is the scheduler's, so pick each call by its target.
    (first, env32), (second, env16) = sorted(seen, key=lambda call: call[1]["LLRM_CC_TARGET"], reverse=True)
    assert Path(first[0]) == driver and "-m32" not in first and "-c" in first and "-mabi=sysv" in first
    assert env32["LLRM_LLC"] == str(llc) and env32["LLRM_CC_TARGET"] == "x86_32-wat"
    assert env16["LLRM_CC_TARGET"] == "x86_16-wat" and "-m16" not in second


def test_the_corpus_for_llrm_llc_leaves_out_what_llrm_cc_cannot_build_and_the_other_corpus_keeps_it():
    """Training llrm-llc through llrm-cc failed the job on 19 compiles of 8 programs it has no way to build yet."""
    names = lambda jobs: {job[0] for job in jobs}  # noqa: E731
    unbuilt = {"es_across_copy", "idioms", "stackdeep", "stackover", "memcpy_runtime", "memfuncs_runtime", "overflow_builtins", "tail_calls_edges"}
    assert unbuilt <= names(pgo.corpus()) and not unbuilt & names(pgo.corpus(driver=True))


def test_llrm_c_is_built_with_its_feature_and_llrm_llc_without(monkeypatch, tmp_path):
    """`cargo build --bin llrm-c` alone fails "target llrm-c requires the features: c-legacy": the pgo job was red on parity."""
    asked = []
    monkeypatch.setattr(pgo.subprocess, "run", lambda command, **_: asked.append(command))
    pgo.cargo(tmp_path, tmp_path, "", "llrm-c")
    pgo.cargo(tmp_path, tmp_path, "", "llrm-llc")
    assert asked[0][-2:] == ["--features", "wccq"] and "--features" not in asked[1]
    builds = [line for line in (pgo.ROOT / ".github/workflows/pgo.yml").read_text().splitlines() if "cargo build" in line]
    assert builds and all("--bin llrm-llc" in line or "--bins" in line for line in builds) and not any("wccq" in line for line in builds), builds


def test_the_build_command_the_ci_job_runs_parses(monkeypatch):
    """CI's `pgo.py build --out .. --profile-dir .. --siblings ..` died in main(): `--binary` is a name, and its default str was resolved like a path ('str' object has no attribute 'resolve')."""
    monkeypatch.chdir(pgo.ROOT)
    args = pgo.parse(["build", "--out", "target/pgo-out", "--profile-dir", "target/pgo-profile", "--siblings", "target/plain"])
    assert args.binary == "llrm-llc" and args.out == pgo.ROOT / "target/pgo-out" and args.siblings == pgo.ROOT / "target/plain"
