"""The gate's path map: a change in an area selects that area's tests, and nothing is left out by omission."""

import pytest
import sys
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import gate  # noqa: E402

ROOT = gate.ROOT


def test_a_change_in_the_c_frontend_runs_the_c_programs_and_qcport_not_the_debugger_sessions():
    p = gate.plan(["crates/frontends/llrm-c/src/lib.rs"])
    assert p.tier == "fast"
    assert p.languages == ["c"]
    assert "qcport" in p.steps and "run" in p.steps
    assert not {"turbo", "cv4", "identity"} & set(p.steps), p.steps


def test_a_change_in_the_omf_writer_runs_the_debugger_sessions():
    p = gate.plan(["crates/target/llrm-omf/src/omf.rs"])
    assert p.tier == "fast"
    assert {"turbo", "cv4"} <= set(p.steps), p.steps


def test_a_change_to_the_identity_script_runs_the_identity_gate_in_the_fast_tier():
    p = gate.plan(["tools/identity.sh"])
    assert p.tier == "fast" and "identity" in p.steps


def test_a_change_to_shared_core_takes_the_full_tier_with_every_step():
    p = gate.plan(["crates/ir/llrm-mir/src/lib.rs"])
    assert p.tier == "full" and p.packages is None
    assert {"turbo", "cv4", "identity", "qcport", "pytest-programs", "run", "bench"} <= set(p.steps)
    assert p.languages == ["qb", "c", "nib"]


def test_a_path_no_table_knows_takes_the_full_tier():
    assert gate.plan(["somewhere/new.txt"]).tier == "full"


def test_only_docs_run_nothing():
    assert gate.plan(["docs/testing.md", "readme.md"]).tier == "none"


def test_lib_tests_run_in_the_changed_crate_and_its_dependents_only():
    p = gate.plan(["crates/frontends/llrm-c/src/lib.rs"])
    pkgs = gate.packages()
    assert "llrm-c" in p.packages
    assert "llrm-mir" not in p.packages
    assert set(p.packages) == gate.dependents(pkgs, {"llrm-c"}) | {"llrm"}


def test_a_python_only_change_runs_the_python_tests_and_not_the_build():
    p = gate.plan(["tools/msp430.py"])
    assert p.tier == "fast" and "pytest" in p.steps and "build" not in p.steps


def test_every_owner_pattern_matches_a_file_that_exists():
    cfg = gate.load()
    files = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    stale = [(h["step"], o) for h in cfg["heavy"] for o in h["owners"] if not any(gate.glob(o).match(f) for f in files)]
    assert not stale, f"owners that match no file: {stale}"


def test_every_split_filter_names_a_test_in_its_binary():
    for s in gate.load()["split"]:
        assert s["filter"] in (ROOT / "tests" / f"{s['bin']}.rs").read_text(), s


def test_every_root_test_binary_is_in_exactly_one_step():
    cfg = gate.load()
    p = gate.plan(["crates/ir/llrm-mir/src/lib.rs"])
    cmds = gate.commands(p, cfg, gate.packages())
    whole = set(cfg["whole"].values()) | set(cfg["exclusive"].values())
    for name in gate.root_tests():
        if name == "timing":
            continue
        in_integration = f"--test {name} " in cmds["integration"] + " "
        assert in_integration != (name in whole), name


def test_the_test_that_writes_into_the_tree_runs_alone_after_the_run_test_could_see_it():
    """It left crates/target/llrm-x86-m32/.rerun-probe in the tree while `run` compared the tree: `run` failed, 'the run left files'."""
    cfg = gate.load()
    assert "build_script_reruns" in cfg["exclusive"].values()
    p = gate.plan(["crates/frontends/llrm-c/src/lib.rs"])
    assert "build-script-reruns" in p.steps
    assert "build_script_reruns" not in gate.commands(p, cfg, gate.packages())["integration"]


def test_a_scoped_lib_run_includes_the_root_crate_that_turns_on_the_features_the_crate_needs():
    """`cargo test -p llrm-c --lib` alone built llrm-c without its toolchain feature: 16 tests failed 'llrm was built without the toolchain feature'."""
    cfg, pkgs = gate.load(), gate.packages()
    p = gate.plan(["crates/frontends/llrm-c/src/lib.rs"])
    assert "-p llrm " in gate.commands(p, cfg, pkgs)["lib"] + " "


def test_a_backend_change_takes_the_fast_tier_with_every_codegen_step_and_no_debugger_session():
    p = gate.plan(["crates/backend/llrm-core/src/backend/isel.rs"])
    assert p.tier == "fast"
    assert {"run", "qcport", "pytest-programs", "bench", "torture"} <= set(p.steps)
    assert p.languages == ["qb", "c", "nib"]
    assert not {"turbo", "cv4", "identity"} & set(p.steps), p.steps


def test_a_change_to_the_debug_info_emitter_runs_the_debugger_sessions():
    p = gate.plan(["crates/backend/llrm-core/src/backend/debuginfo.rs"])
    assert {"turbo", "cv4"} <= set(p.steps), p.steps


def test_a_bisect_finds_the_first_commit_that_fails_and_runs_the_steps_only_log_n_times():
    commits = [f"c{i}" for i in range(20)]
    for first in (0, 7, 19):
        asked = []
        found = gate.first_bad(commits, lambda c: asked.append(c) or int(c[1:]) >= first)
        assert found == f"c{first}" and len(asked) <= 5, (first, found, asked)


def test_the_run_step_runs_only_the_languages_the_diff_can_affect():
    cfg, pkgs = gate.load(), gate.packages()
    p = gate.plan(["crates/frontends/llrm-nib/src/lib.rs"])
    assert "LLRM_RUN_ONLY='nib'" in gate.commands(p, cfg, pkgs)["run"]


def test_a_test_run_cut_short_is_incomplete_not_a_pass():
    whole = "running 2 tests\n..\ntest result: ok. 2 passed; 0 failed\n\nrunning 1 test\n.\ntest result: ok. 1 passed; 0 failed\n"
    assert gate.incomplete(whole, 2, False) is None
    cut = "running 2 tests\n..\ntest result: ok. 2 passed; 0 failed\n\nrunning 1 test\n"
    assert "1 results of 2" in gate.incomplete(cut, None, False)
    assert "2 results of 3" in gate.incomplete(whole, 3, False)
    assert gate.incomplete("", None, False) == "no test result"


def test_a_filter_that_matches_no_test_is_incomplete_not_a_pass():
    """A renamed test would make `cargo test -- old_name` pass having run nothing."""
    log = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out\n"
    assert gate.incomplete(log, 1, True) == "its filter matched no test"
    assert gate.incomplete(log, 1, False) is None


def test_a_step_that_runs_the_binaries_builds_them_first_even_when_no_rust_changed():
    """A change to tools/qcport-run.py planned `pytest qcport` with no build: qcport ran against missing or stale binaries."""
    for path in ("tools/qcport-run.py", "tools/torture/torture.py"):
        p = gate.plan([path])
        assert p.steps[0] == "build", (path, p.steps)
    assert "build" not in gate.plan(["tools/msp430.py"]).steps


def test_the_qcport_step_has_no_wall_clock_limit_of_its_own():
    """`timeout 400` around qcport-run killed a run that was still drawing at load: qcport-run stops a stall and caps a hang itself."""
    p = gate.plan(["tools/qcport-run.py"])
    assert "timeout" not in gate.commands(p, gate.load(), gate.packages())["qcport"]


def test_a_backend_change_runs_the_measure_step_and_a_frontend_only_change_does_not():
    assert "measure" in gate.plan(["crates/backend/llrm-core/src/backend/isel.rs"]).steps
    assert "measure" in gate.plan(["crates/opt/llrm-transforms/src/gvn.rs"]).steps
    assert "measure" in gate.plan(["crates/ir/llrm-mir/src/lib.rs"]).steps  # full
    assert "measure" not in gate.plan(["tests/run.rs"]).steps
    assert "measure" not in gate.plan(["crates/frontends/llrm-qb/src/lib.rs"]).steps


def test_the_measure_step_has_a_command_and_no_baseline_lives_in_the_repository():
    p = gate.plan(["tools/measure.py"])
    assert "measure" in p.steps
    assert "tools/measure.py check" in gate.commands(p, gate.load(), gate.packages())["measure"]
    tracked = subprocess.run(["git", "ls-files", "tools/gate"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    assert not [f for f in tracked if f.endswith(".json")], tracked  # two branches from one base share no file to conflict on


def test_steps_asked_for_replace_the_planned_ones_and_the_build_comes_first():
    """Re-running a change to the measurement tools needs build and measure; the full gate took 5-7 minutes."""
    known = set(gate.commands(gate.plan(["tools/gate/gate.py"]), gate.load(), gate.packages()))
    p = gate.plan(["tools/gate/gate.py"])
    assert p.tier == "full" and len(p.steps) > 8
    one = gate.restricted(p, ["measure"], known)
    assert one.steps == ["build", "measure"] and one.tier == p.tier and one.languages == p.languages
    assert gate.restricted(p, ["pytest"], known).steps == ["pytest"]
    with pytest.raises(SystemExit):
        gate.restricted(p, ["mesure"], known)


def test_a_one_line_change_to_any_source_runs_every_source_scan_in_the_fast_tier():
    """#1057 and #1071 gated fast and skipped llrm-mir's dense-key ratchet (a lib test of a crate they did not touch); main's full run went
    red. A ratchet reads every crate's source, so any .rs change selects it, whichever crate holds the test."""
    cfg = gate.load()
    for touched in ("crates/opt/llrm-analysis/src/ranges.rs", "crates/backend/llrm-core/src/backend/spiller.rs", "crates/frontends/llrm-c/src/lib.rs"):
        p = gate.plan([touched])
        assert p.tier == "fast" and "scans" in p.steps, (touched, p.steps)
    command = gate.commands(gate.plan(["crates/opt/llrm-analysis/src/ranges.rs"]), cfg, gate.packages())["scans"]
    for one in cfg["scan"]:
        if "package" in one:
            assert one["package"] in command and (one.get("lib") or one["test"]) in command, one
    assert "scans" not in gate.plan(["tools/measure.py"]).steps and "scans" not in gate.plan(["docs/testing.md"]).steps


def test_every_scan_names_a_test_that_exists():
    for one in gate.load()["scan"]:
        text = (ROOT / one["file"]).read_text()
        name = one.get("lib") or one.get("test") or Path(one["file"]).stem
        assert one.get("step") == "integration" or f"fn {name}" in text or Path(one["file"]).stem == name, one
        if one.get("step") == "integration":
            assert Path(one["file"]).stem in gate.root_tests(), one


def test_every_test_that_reads_the_trees_sources_is_a_listed_scan():
    """A test that walks the directories for `.rs` files is a source scan; one nobody lists is skipped by a diff to any other crate."""
    listed = {one["file"] for one in gate.load()["scan"]}
    found = []
    for path in sorted(ROOT.glob("**/*.rs")):
        relative = str(path.relative_to(ROOT))
        if relative.startswith(("target/", ".git/")) or "/target/" in relative:
            continue
        in_tests = "/tests/" in relative or relative.startswith("tests/") or relative.endswith(("_tests.rs", "/tests.rs"))
        if not in_tests:
            continue
        text = path.read_text()
        if "read_dir" in text and '".rs"' in text and "#[test]" in text:
            found.append(relative)
    assert found, "the scan discovery found nothing: it is broken"
    missing = [f for f in found if f not in listed]
    assert not missing, f"source-scan tests missing from tiers.toml [[scan]]: {missing}"


def test_a_failing_scan_does_not_hide_the_ones_after_it(tmp_path):
    """&& stopped at the first red scan; the step runs every scan and fails if any did."""
    command = gate.commands(gate.plan(["crates/opt/llrm-analysis/src/ranges.rs"]), gate.load(), gate.packages())["scans"]
    assert "&&" not in command.replace("cargo test", "") and command.count("|| rc=1") == 3 and command.endswith("exit $rc")


def test_a_diff_that_selects_only_the_root_crate_runs_no_lib_or_doc_tests():
    """A change to tools/bench selected the root crate alone, which has no library: `cargo test -p llrm --lib` failed with 'no library
    targets found' and the gate went red on a bench blessing."""
    p = gate.plan(["tools/bench/bench.py"])
    assert p.packages == ["llrm"], p.packages
    steps = gate.commands(p, gate.load(), gate.packages())
    assert steps["lib"] == "true" and steps["doc"] == "true"
    expected = gate.expected(p, gate.load(), gate.packages())
    assert "lib" not in expected and "doc" not in expected
    assert "-p llrm-mir" in gate.commands(gate.plan(["crates/ir/llrm-mir/src/lib.rs"]), gate.load(), gate.packages())["lib"] or "--workspace" in gate.commands(gate.plan(["crates/ir/llrm-mir/src/lib.rs"]), gate.load(), gate.packages())["lib"]
