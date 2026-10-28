"""The landing runner's stale stage: LLRM_CHECK_STALE over the sweep's programs on the merged tree's release bins. bintree, fpbench and
jpeg_single failed that oracle for ~2989 commits (798dba289 to #5963) because only the gate ran it."""

import sys

import parity_squash as ps

PANIC = "thread 'main' panicked at passes.rs:3294:9:\\npipeline added a memory operation: the frozen globals-aa no longer covers the module"


def fake_release(tmp_path, panics: bool):
    """A release dir whose compilers reach the oracle (the `[checks]` line) and, when `panics`, fail it as the batch with 798dba289 did."""
    release = tmp_path / "release"
    release.mkdir(exist_ok=True)
    for tool in ("llrm-qb", "llrm-nib"):
        script = release / tool
        script.write_text(
            "#!/bin/sh\n"
            'if [ -n "$LLRM_CHECK_STALE" ]; then echo "[checks] STALE 1" >&2; '
            + (f'printf "{PANIC}\\n" >&2; exit 101; ' if panics else "")
            + "fi\nexit 0\n"
        )
        script.chmod(0o755)
    return release


def test_a_batch_whose_compiler_fails_the_stale_oracle_is_refused_and_a_clean_one_is_not(tmp_path, monkeypatch):
    """Would have refused the batch that landed 798dba289: the stage printed nothing and the landing went through."""
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path))
    fake_release(tmp_path, panics=True)
    red = ps.pre_merge_stale(["crates/ir/llrm-mir/src/passes.rs"], build=lambda: None)
    assert red and any("frozen globals-aa" in line for line in red), red
    fake_release(tmp_path, panics=False)
    assert ps.pre_merge_stale(["crates/ir/llrm-mir/src/passes.rs"], build=lambda: None) == []


def test_a_batch_with_no_crate_source_builds_and_runs_nothing():
    built = []
    assert ps.pre_merge_stale(["docs/x.md", "tools/x.py", "crates/a/src/x_tests.rs"], build=lambda: built.append(1), run=lambda: built.append(2)) == [] and not built


def test_a_compiler_that_never_reaches_the_oracle_is_not_a_clean_run(tmp_path, monkeypatch):
    """A stand-in that ignores LLRM_CHECK_STALE exits 0 everywhere; the stage must fail it (sweep exit 2), as the sweep does."""
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path))
    release = tmp_path / "release"
    release.mkdir()
    for tool in ("llrm-qb", "llrm-nib"):
        (release / tool).write_text("#!/bin/sh\nexit 0\n")
        (release / tool).chmod(0o755)
    red = ps.pre_merge_stale(["crates/ir/llrm-mir/src/passes.rs"], build=lambda: None)
    assert red and any("STALE" in line for line in red), red


def test_a_finding_the_base_already_has_is_no_prs_to_blame():
    """A red base fails every PR merged alone; the halving blames only the kinds a PR adds (counts and example lists differ run to run)."""
    base = ["282 compiles / 1 new finding kinds / 0 expected / 0 expected entries paid (delete them)", "   2  exit 1: $QB$MAIN: a call with no callee", "      e.g. ['dot.bas -m16']"]
    more = ["282 compiles / 2 new finding kinds / 0 expected / 0 expected entries paid (delete them)", "   9  exit 1: $QB$MAIN: a call with no callee", "      e.g. ['a.bas -m16']", "   3  exit 101: x added a memory operation: the frozen globals-aa no longer covers the module"]
    assert ps.stale_kinds(base) == {"exit 1: $QB$MAIN: a call with no callee"}
    assert ps.stale_kinds(more) - ps.stale_kinds(base) == {"exit 101: x added a memory operation: the frozen globals-aa no longer covers the module"}


def test_a_timeout_of_the_sweep_is_a_finding_not_a_crash_of_the_batch():
    """An uncaught TimeoutExpired escaped release_pass and refused every member as a tool fault (#5696)."""
    import subprocess

    def slow():
        raise subprocess.TimeoutExpired("oracle_sweep", 900)

    assert ps.pre_merge_stale(["crates/a/src/x.rs"], build=lambda: None, run=slow) == ["the stale oracle ran past 900 s"]


GLOBALS = "   3  exit 101: x added a memory operation: the frozen globals-aa no longer covers the module"
DOT = "   2  exit 1: $QB$MAIN: a call with no callee"


def blamed(adds: dict[str, list[str]], base: set[str]):
    """The PRs named for a batch of `adds` (PR -> finding lines it brings) over a base with the kinds `base`."""
    files = {n: [f"crates/{n}/src/x.rs"] for n in adds}
    by_file = {f"crates/{n}/src/x.rs": n for n in adds}
    return ps.stale_culprits(
        list(adds),
        lambda n: files[n],
        base,
        merge_all=lambda subset: True,
        merge_alone=lambda n: True,
        check=lambda fs: [line for f in fs for line in adds[by_file[f]]],
    )


def test_only_the_pr_that_adds_a_kind_over_a_red_base_is_blamed():
    """A red base fails every PR merged alone; blaming them all refused the whole batch for the base's own finding."""
    on_base = ps.stale_kinds([DOT])
    assert blamed({"1": [DOT], "2": [DOT, GLOBALS], "3": [DOT]}, on_base) == ["2"]


def test_a_red_base_with_nothing_added_names_nobody():
    assert blamed({"1": [DOT], "2": [DOT]}, ps.stale_kinds([DOT])) == []
