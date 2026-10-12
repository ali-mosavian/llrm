"""The gate's path map: a change in an area selects that area's tests, and nothing is left out by omission."""

import re
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
    assert "llrm-transforms" not in p.packages
    assert set(p.packages) == gate.dependents(pkgs, {"llrm-c"}) | {"llrm", "llrm-mir"}


def test_a_change_in_any_crate_runs_the_lib_tests_that_read_the_sources_of_the_others():
    """facts_rewrite's audit of the readers of nsw/nuw/inbounds is in llrm-mir, which a pass in llrm-transforms does not
    reach as a dependent: #1324 added a reader, the gate ran no llrm-mir test, and main was red."""
    p = gate.plan(["crates/opt/llrm-transforms/src/algebraic.rs"])
    assert "llrm-mir" in p.packages and "lib" in p.steps


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

def test_the_torture_step_has_no_wall_clock_limit_of_its_own():
    """`timeout 600` killed a full gate's torture step at 150 of 474 programs under load: every program's budget is guest time, so
    the step's wall time is the host's load. dosbatch stops an emulator that stops reporting."""
    p = gate.plan(["tools/torture/torture.py"])
    assert "timeout" not in gate.commands(p, gate.load(), gate.packages())["torture"]


def test_every_step_the_planner_can_produce_is_in_exactly_one_ci_group():
    """A step in no group would never run in CI, silently: the matrix is built from the groups."""
    plans = [gate.plan(["crates/ir/llrm-mir/src/lib.rs"], "full"), gate.plan(["crates/ir/llrm-mir/src/lib.rs"], "auto"), gate.plan(["crates/backend/x.rs"], "fast")]  # `scans` and `measure` are fast-tier steps
    everything = [s for q in plans for s in q.steps if s != "build"]  # the build is its own job
    grouped = [s for members in gate.load()["groups"].values() for s in members]
    assert sorted(grouped) == sorted(set(grouped)), "a step is in two groups"
    assert set(everything) <= set(grouped), set(everything) - set(grouped)
    assert set(grouped) <= set(everything), set(grouped) - set(everything)


def test_a_step_whose_tool_is_missing_is_skipped_with_the_tool_named_not_run_and_not_passed(tmp_path):
    """On a runner without Turbo C++ the turbo step passed by skipping inside the test binary: green, having proved nothing."""
    env = {"HOME": str(tmp_path), "TCPP30_DIR": str(tmp_path / "none"), "GATE_ALLOW_MISSING": "1"}
    missing = gate.missing_capabilities(env)
    assert "TCPP30_DIR" in missing["turbo"] and "codeview" in missing
    skipped = gate.skipped_steps(["turbo", "cv4", "qcport", "run"], missing)
    assert set(skipped) == {"turbo", "cv4", "qcport"}
    assert "TCPP30_DIR" in skipped["turbo"]


def test_a_tool_the_caller_requires_is_never_skipped(tmp_path):
    """LLRM_REQUIRE_TURBO set means a gate that cannot find Turbo must fail, not skip."""
    env = {"HOME": str(tmp_path), "LLRM_REQUIRE_TURBO": "1", "GATE_ALLOW_MISSING": "1"}
    assert "turbo" not in gate.missing_capabilities(env)


def test_a_tool_that_is_present_is_not_missing(tmp_path):
    (tmp_path / "bin").mkdir()
    (tmp_path / "bin/TCC.EXE").touch()
    (tmp_path / "Td.exe").touch()
    env = {"HOME": str(tmp_path), "TCPP30_DIR": str(tmp_path), "TD_DIR": str(tmp_path), "GATE_ALLOW_MISSING": "1"}
    assert "turbo" not in gate.missing_capabilities(env)


def test_ci_groups_hold_only_the_steps_that_can_run_here():
    p = gate.plan(["crates/ir/llrm-mir/src/lib.rs"])
    skipped = {"turbo": "x", "cv4": "x", "qcport": "x"}
    groups = gate.groups_of(p, skipped)
    assert "reference" not in groups and "turbo" not in sum(groups.values(), [])


def test_the_plan_names_each_skipped_step_and_its_missing_tool_as_json(tmp_path):
    env = {**__import__("os").environ, "HOME": str(tmp_path), "LLRM_REQUIRE_TURBO": "", "LLRM_REQUIRE_CODEVIEW": "", "QB45_DIR": "", "TCPP30_DIR": "", "TD_DIR": "", "VBDOS_DIR": "", "QCPORT": "", "QCPORT_BORLAND": "", "GATE_ALLOW_MISSING": "1"}
    out = subprocess.run([sys.executable, str(Path(gate.__file__)), "plan", "--json", "--files", "crates/ir/llrm-mir/src/lib.rs"], capture_output=True, text=True, check=True, env=env).stdout
    got = __import__("json").loads(out)
    assert set(got["skipped"]) - {"measure"} == {"turbo", "cv4", "qcport", "bench", "dos-slow"} and "reference" not in got["groups"]
    assert "run" in got["groups"]["run"]


def test_a_gate_run_makes_a_target_dir_that_does_not_exist_yet(tmp_path, monkeypatch):
    """On a fresh CI runner CARGO_TARGET_DIR is not there: `run` died FileNotFoundError on target/gate-logs before any step."""
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path / "fresh" / "target"))
    monkeypatch.setenv("LLRM_BIN", str(tmp_path / "bin"))  # the gate sets it; a plain shell does not, and snapshots a release dir that is not there
    code, failed = gate.execute(gate.Plan("fast", "test", steps=[]))
    assert code == 0 and (tmp_path / "fresh/target/gate-logs").is_dir()


def test_a_moved_tool_directory_is_not_skipped_unless_the_caller_allows_it(tmp_path):
    """Skipping on any absent tool turned a red gate (TD_DIR moved) into SKIPPED and PASS on the host that has the tools."""
    env = {"HOME": str(tmp_path), "TCPP30_DIR": str(tmp_path / "moved")}
    assert gate.missing_capabilities(env) == {}
    assert "turbo" in gate.missing_capabilities({**env, "GATE_ALLOW_MISSING": "1"})


def test_without_quickbasic_the_qb_programs_and_the_differential_binary_are_left_out_not_failed(tmp_path):
    """Every qb program died 'Bad command or filename V:\\LINK' on a runner without QuickBASIC: run and integration red for a missing tool."""
    env = {"HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1"}
    missing = gate.missing_capabilities(env)
    langs, bins = gate.unusable(missing)
    assert langs == ["qb"] and "differential" in bins
    p = gate.plan(["crates/ir/llrm-mir/src/lib.rs"])
    cmds = gate.commands(p, gate.load(), gate.packages(), bins)
    assert "--test differential " not in cmds["integration"] + " " and "--test farbss" in cmds["integration"]
    assert gate.skipped_steps(["run", "bench"], missing, ["qb"]).keys() == {"run", "bench"}
    assert "run" not in gate.skipped_steps(["run"], missing, ["qb", "c"])


def test_a_step_run_by_the_gate_does_not_see_a_requirement_for_a_tool_the_host_lacks(tmp_path, monkeypatch):
    """The unset reached the build but not the pool: tests/turbo.rs failed 'LLRM_REQUIRE_TURBO is set' on a runner with no Turbo C++."""
    for name, value in {"CARGO_TARGET_DIR": str(tmp_path / "t"), "HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1", "TCPP30_DIR": "", "TD_DIR": ""}.items():
        monkeypatch.setenv(name, value)
    monkeypatch.delenv("LLRM_REQUIRE_TURBO", raising=False)
    monkeypatch.setenv("LLRM_BIN", str(tmp_path / "bin"))
    monkeypatch.setattr(gate, "commands", lambda *a: {"probe": 'echo "[$LLRM_REQUIRE_TURBO]"'})
    assert gate.execute(gate.Plan("fast", "test", steps=["probe"]))[0] == 0
    assert (tmp_path / "t/gate-logs/probe.log").read_text().strip() == "[]"


def test_a_dropped_test_binary_is_named_in_the_run_and_does_not_make_the_step_incomplete(tmp_path):
    """The step reports one binary fewer: the expected count must follow, and the output must say which was left out."""
    cfg, pkgs = gate.load(), gate.packages()
    p = gate.plan(["crates/ir/llrm-mir/src/lib.rs"])
    full = gate.expected(p, cfg, pkgs)["integration"][0]
    cut = gate.expected(p, cfg, pkgs, frozenset({"differential"}))["integration"][0]
    assert cut == full - 1
    one = "running 1 test\n.\ntest result: ok. 1 passed; 0 failed\n"
    assert gate.incomplete(one * cut, cut, False) is None
    assert "results of" in gate.incomplete(one * cut, full, False)
    env = {**__import__("os").environ, "HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1", "CARGO_TARGET_DIR": str(tmp_path / "t"), "QB45_DIR": ""}
    code = "import sys; sys.path.insert(0, 'tools/gate'); import gate; gate.execute(gate.Plan('fast', 'x', steps=[]))"
    out = subprocess.run([sys.executable, "-c", code], cwd=ROOT, capture_output=True, text=True, env=env).stdout
    assert "[dropped] bin:differential" in out and "[dropped] lang:qb" in out, out


def test_without_the_opt_in_the_plan_and_the_commands_are_what_they_were(tmp_path):
    """Skipping is CI's: on a host that did not ask, nothing is skipped, dropped or reworded."""
    env = {"HOME": str(tmp_path)}
    assert gate.missing_capabilities(env) == {}
    assert gate.unusable({}) == ([], frozenset())
    p = gate.plan(["crates/ir/llrm-mir/src/lib.rs"])
    cfg, pkgs = gate.load(), gate.packages()
    assert gate.commands(p, cfg, pkgs) == gate.commands(p, cfg, pkgs, frozenset())
    assert gate.skipped_steps(p.steps, gate.missing_capabilities(env), p.languages) == {}
    assert "--test differential " in gate.commands(p, cfg, pkgs)["integration"] + " "


def test_a_python_test_file_that_needs_a_missing_probe_is_left_out_of_pytest_by_name():
    """test_scaling_gate.py read float('<not supported>') on a runner without counters and failed the python group on main; test_scaling.py failed 'Access to performance monitoring ... is limited' on a runner whose VM has no instruction counters."""
    missing = {"perf": "perf: `perf stat` fails here"}
    assert sorted(gate.python_tests_unusable(missing)) == ["crates/target/llrm-x86-m32/vsgcc/test_scaling.py", "crates/target/llrm-x86-m32/vsgcc/test_scaling_gate.py"]
    p = gate.plan(["tools/linkrecipe.py"])
    cmds = gate.commands(p, gate.load(), gate.packages(), frozenset(), tuple(gate.python_tests_unusable(missing)))
    assert "--ignore=crates/target/llrm-x86-m32/vsgcc/test_scaling.py" in cmds["pytest"] and "--ignore=crates/target/llrm-x86-m32/vsgcc/test_scaling_gate.py" in cmds["pytest"]
    assert "test_scaling" not in gate.commands(p, gate.load(), gate.packages())["pytest"]


def test_a_new_test_file_declares_its_own_need_and_is_ignored_where_it_is_missing(tmp_path):
    """Each counter-reading file needed its own row in tiers.toml; test_scaling_gate.py had none and failed the python group on main."""
    subprocess.run(["git", "init", "-q"], cwd=tmp_path, check=True)
    (tmp_path / "test_reads_counter.py").write_text('REQUIRES = ["perf"]\n\ndef test_x():\n    pass\n')
    (tmp_path / "test_plain.py").write_text("def test_y():\n    pass\n")
    assert gate.declared_requirements(tmp_path) == {"test_reads_counter.py": ["perf"]}
    assert gate.python_tests_unusable({"perf": "no counter"}, tmp_path) == ["test_reads_counter.py"]
    assert gate.python_tests_unusable({}, tmp_path) == []


def test_a_probe_that_fails_makes_the_capability_missing_and_one_that_succeeds_does_not(tmp_path, monkeypatch):
    cfg = {"capability": {"x": {"require": [], "needs": [{"run": "false"}]}, "y": {"require": [], "needs": [{"run": "true"}]}}}
    monkeypatch.setattr(gate, "load", lambda: cfg)
    got = gate.missing_capabilities({"HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1", "PATH": "/usr/bin:/bin"})
    assert list(got) == ["x"] and "false" in got["x"]


def test_the_perf_probe_reads_a_count_not_the_exit_status(tmp_path):
    """`perf stat` exits 0 and prints '<not supported>' on a VM: the probe said the counters were there and test_scaling.py died on float('<not supported>')."""
    fake = tmp_path / "perf"
    fake.write_text("#!/bin/sh\necho '<not supported>,,instructions:u,0,100.00,,'\n")
    fake.chmod(0o755)
    env = {"HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1", "PATH": f"{tmp_path}:/usr/bin:/bin", "QB45_DIR": ""}
    assert "perf" in gate.missing_capabilities(env)
    fake.write_text("#!/bin/sh\necho '123456,,instructions:u,100,100.00,,'\n")
    assert "perf" not in gate.missing_capabilities(env)


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
    known = set(gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages()))
    p = gate.plan(["tools/gate/gate.py"], "full")
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


def test_a_one_line_change_to_any_rust_file_selects_the_fmt_step_in_the_fast_tier():
    """The formatter was not in the gate at all: a tree could drift from tools/fmt.sh and nothing said so."""
    for touched in ("crates/opt/llrm-analysis/src/ranges.rs", "tests/run.rs", "tools/rfmt-post/src/main.rs"):
        assert "fmt" in gate.plan([touched]).steps, touched
    assert gate.plan(["crates/opt/llrm-analysis/src/ranges.rs"]).tier == "fast"
    assert "fmt" in gate.plan(["rustfmt.toml"]).steps and "fmt" in gate.plan(["tools/fmt.sh"]).steps
    assert "fmt" not in gate.plan(["tools/measure.py"]).steps and "fmt" not in gate.plan(["docs/testing.md"]).steps


def test_the_build_rejects_compiler_warnings():
    assert gate.WARNINGS_AS_ERRORS == "RUSTFLAGS='-D warnings'"
    assert gate.BUILD.count(gate.WARNINGS_AS_ERRORS) == 3
    assert "cargo check --workspace --all-targets -q" in gate.BUILD
    assert "cargo check --release --workspace --all-targets -q" in gate.BUILD


def test_a_formatter_change_runs_its_tests_and_neither_step_needs_the_compiler_built():
    """`fmt` and `rfmt-post` format text; a build before them is minutes spent on binaries they never run."""
    for touched in ("tools/rfmt-post/src/lib.rs", "tools/fmt.sh", "rustfmt.toml"):
        p = gate.plan([touched])
        assert {"fmt", "rfmt-post"} <= set(p.steps), (touched, p.steps)
    assert "build" not in gate.plan(["tools/rfmt-post/src/lib.rs"]).steps
    assert "rfmt-post" not in gate.plan(["crates/opt/llrm-analysis/src/ranges.rs"]).steps


def test_the_formatter_helper_tests_run_after_the_formatter():
    assert gate.SERIAL_STEPS == frozenset({"rfmt-post"})


def test_the_fmt_step_fails_only_once_enforced_and_the_switch_is_one_line_of_tiers_toml():
    """Unformatted, the tree would fail every PR: the step is reported until PR 2 formats the tree and sets `enforced`."""
    cfg, pkgs = gate.load(), gate.packages()
    p = gate.plan(["tools/fmt.sh"])
    on = gate.commands(p, {**cfg, "fmt": {**cfg["fmt"], "enforced": True}}, pkgs)["fmt"]
    off = gate.commands(p, {**cfg, "fmt": {**cfg["fmt"], "enforced": False}}, pkgs)["fmt"]
    assert on == "tools/fmt.sh --check" and off.startswith(on) and off != on
    assert isinstance(cfg["fmt"]["enforced"], bool)


def test_the_unenforced_fmt_step_passes_on_clean_and_on_files_to_format_and_fails_on_anything_else(tmp_path):
    """`tools/fmt.sh --check || echo ...` passed a missing toolchain, a failed build and a rustfmt crash alike."""
    (tmp_path / "tools").mkdir()
    stub = tmp_path / "tools" / "fmt.sh"
    command = gate.commands(gate.plan(["tools/fmt.sh"]), {**gate.load(), "fmt": {**gate.load()["fmt"], "enforced": False}}, gate.packages())["fmt"]
    for status, want in ((0, 0), (1, 0), (2, 2), (101, 101)):
        stub.write_text(f"#!/bin/sh\nexit {status}\n")
        stub.chmod(0o755)
        got = subprocess.run(["bash", "-c", command], cwd=tmp_path, capture_output=True).returncode
        assert got == want, (status, got)


def test_edits_under_zed_plan_nothing():
    """.zed/settings.json was an unknown path: the full tier for an editor setting."""
    assert gate.plan([".zed/settings.json"]).tier == "none"


def test_the_full_tier_has_every_step_the_fast_tier_has_for_the_same_paths():
    """`scans` was fast-tier only: a diff that picked full skipped the source ratchets that a smaller diff ran."""
    for files in (["crates/ir/llrm-mir/src/lib.rs"], ["crates/backend/x.rs"], ["tools/torture/a.py"], ["tools/gate/gate.py"], ["tools/gate/gate.py", "crates/ir/llrm-mir/src/lib.rs"], ["tests/target_facts.baseline"]):
        fast, full = gate.plan(files, "fast"), gate.plan(files, "full")
        assert set(fast.steps) <= set(full.steps), (files, set(fast.steps) - set(full.steps))
    assert "scans" in gate.plan(["crates/ir/llrm-mir/src/lib.rs"], "full").steps


def test_a_failed_step_keeps_its_log_past_the_next_run_of_the_step(tmp_path):
    """A pytest failure under host load was lost: the gate prints the last 30 lines of the log and a rerun overwrote it, so the
    failing test was never named (#1184)."""
    env = {**__import__("os").environ}
    assert gate.run_step("probe", "echo FAILED tests/test_x.py::test_y; exit 1", tmp_path, env)[1] == 1
    assert gate.run_step("probe", "echo fine", tmp_path, env)[1] == 0
    assert "FAILED tests/test_x.py::test_y" in (tmp_path / "probe.failed.log").read_text()
    assert (tmp_path / "probe.log").read_text() == "fine\n"
    assert gate.run_step("skipped", "exit 77", tmp_path, env)[1] == 77
    assert not (tmp_path / "skipped.failed.log").exists()


def test_the_lib_step_runs_tests_in_a_random_order_that_a_seed_repeats(monkeypatch):
    """llrm-c's m16 tests failed at random (29 of 136) when an m32 test bound the register file first: the plain order never
    showed it. The lib step shuffles, each binary prints its seed, and LLRM_SHUFFLE_SEED repeats one."""
    cfg, pkgs = gate.load(), gate.packages()
    p = gate.plan(["crates/frontends/llrm-c/src/lib.rs"])
    monkeypatch.delenv("LLRM_SHUFFLE_SEED", raising=False)
    command = gate.commands(p, cfg, pkgs)["lib"]
    assert command.endswith("-Zunstable-options --shuffle"), command
    monkeypatch.setenv("LLRM_SHUFFLE_SEED", "42")
    assert gate.commands(p, cfg, pkgs)["lib"].endswith("-Zunstable-options --shuffle-seed 42")


def test_every_step_runs_with_the_flags_the_build_used_and_the_base_build_with_none():
    """`integration` (`cargo test --test X`) without the build step's RUSTFLAGS rebuilt llrm-c and replaced target/release/llrm-c while
    `torture` was starting it ("No such file or directory"): the steps share one set of flags, and the base the measure builds, which
    is historical, has none."""
    assert gate.STEP_ENV == {"RUSTFLAGS": gate.WARNING_FLAGS, "RUSTC_BOOTSTRAP": "1"}
    assert f"RUSTFLAGS='{gate.WARNING_FLAGS}'" in gate.BUILD
    assert "RUSTFLAGS" in gate.MEASURE_BUILD and "env -u RUSTFLAGS cargo" in gate.MEASURE_BUILD
    assert "RUSTFLAGS" not in gate.MEASURE_BUILD.replace("env -u RUSTFLAGS", "")


def test_the_slow_dos_tier_is_a_heavy_step_selected_by_the_runtime_and_the_dos32_harness():
    """The demos and graphics sweeps (minutes) ran in the `pytest` step on every change; now they are a step of their own."""
    assert "dos-slow" in gate.plan(["runtime/qb/gpaint.c"]).steps
    assert "dos-slow" in gate.plan(["tools/dosbatch/qb32demos.py"]).steps
    assert "dos-slow" not in gate.plan(["crates/frontends/llrm-c/src/lib.rs"]).steps
    command = gate.commands(gate.plan(["runtime/qb/gpaint.c"]), gate.load(), gate.packages())["dos-slow"]
    assert "-m slow_dos" in command and "exit 77" in command, command


def test_the_slow_dos_tier_is_skipped_where_the_demo_sources_are_missing(tmp_path):
    env = {"HOME": str(tmp_path), "QB45_DEMOS_DIR": str(tmp_path / "none"), "QB45_DIR": str(tmp_path / "none"), "GATE_ALLOW_MISSING": "1"}
    skipped = gate.skipped_steps(["dos-slow"], gate.missing_capabilities(env))
    assert "QB45_DEMOS_DIR" in skipped["dos-slow"] or "QB45_DIR" in skipped["dos-slow"], skipped


def test_a_run_asked_for_another_commit_refuses_and_names_no_pass():
    """A gate started on a checkout that had failed passed on the previous run's log, and the commit it named was never run."""
    done = subprocess.run([sys.executable, str(ROOT / "tools/gate/gate.py"), "run", "--expect", "0000000"], capture_output=True, text=True, cwd=ROOT)
    assert done.returncode == 2 and "not the checkout asked for" in done.stdout, done.stdout + done.stderr
    assert "PASS" not in done.stdout


def test_a_dirty_tree_is_refused_unless_it_is_allowed_and_a_verdict_names_its_commit():
    assert gate.refusal("a" * 40, True, None, False) and "differs from HEAD" in gate.refusal("a" * 40, True, None, False)
    assert gate.refusal("a" * 40, True, None, True) is None and gate.refusal("a" * 40, False, "aaaa", False) is None
    assert gate.verdict_line("fast", [], [], 5.0, [], "abcdef0123456789").startswith("GATE fast PASS at abcdef012345")
    assert gate.verdict_line("fast", ["lib"], [], 5.0, [], "abcdef0123456789").startswith("GATE fast FAIL: lib at abcdef012345")


def test_the_steps_run_the_binaries_cargo_linked_before_them_not_the_path_cargo_relinks(tmp_path):
    """Torture died on `No such file .../release/llrm-c`: a `cargo test --release --test X` in another step removes the file and links
    its own binary there, even when nothing was rebuilt. The steps get links to the build's files, which a later link does not touch."""
    release = tmp_path / "release"
    release.mkdir()
    (release / "deps").mkdir()
    for name, text in (("llrm-c", "built"), ("jwlink", "tool"), ("llrm-c.d", "dep-info")):
        (release / name).write_text(text)
        (release / name).chmod(0o755)
    shared = gate.snapshot_binaries(release, tmp_path / "gate-bins")
    assert sorted(one.name for one in shared.iterdir()) == ["jwlink", "llrm-c"], "the dep-info or a directory was taken"
    # What cargo does for the next invocation: remove, then link another file.
    (release / "llrm-c").unlink()
    assert not (release / "llrm-c").exists() and (shared / "llrm-c").read_text() == "built"
    (release / "llrm-c").write_text("relinked")
    assert (shared / "llrm-c").read_text() == "built"
    # A second snapshot (another group of the gate) replaces the links.
    (release / "llrm-c").chmod(0o755)
    assert (gate.snapshot_binaries(release, tmp_path / "gate-bins") / "llrm-c").read_text() == "relinked"


def test_a_steps_cpu_time_is_its_process_trees_and_not_the_waiting_in_it(tmp_path):
    """The fast gate went from 170-460 s to 865-1,816 s and the wall clock could not say whether the steps grew or the host (load 52
    on 32 cores) did. A step's CPU is user+sys of its whole tree: a child's spin counts, a sleep does not."""
    env = {**__import__("os").environ}
    spin = 'python3 -c "import time; t = time.process_time()\nwhile time.process_time() - t < 0.6: pass" & wait'
    _, code, wall, cpu = gate.run_step("spin", spin, tmp_path, env)
    assert code == 0 and cpu >= 0.5, f"a child spun 0.6 s of CPU and the step read {cpu:.2f} s"
    _, code, wall, cpu = gate.run_step("sleep", "sleep 0.6", tmp_path, env)
    assert wall >= 0.6 and cpu < 0.3, f"a step that only waited read {cpu:.2f} s of CPU"
    table = gate.step_table({"a": (0, 5.0, 2.0), "b": (0, 9.0, 8.0), "skipped": (77, 0, 0)})
    assert table.splitlines()[1].startswith("[steps] b ") and "skipped" not in table and table.endswith("10 cpu")


def test_no_cargo_step_sets_an_environment_the_others_do_not(monkeypatch):
    """RUSTC_BOOTSTRAP=1 on the lib step alone made cargo rebuild the workspace there (1,088 CPU-s of a 4,340 CPU-s gate) and again for
    the next step without it: an environment variable is an input of every crate cargo builds. The variables a cargo step needs are in
    STEP_ENV, for all of them."""
    cfg, pkgs = gate.load(), gate.packages()
    monkeypatch.delenv("LLRM_SHUFFLE_SEED", raising=False)
    p = gate.plan(["crates/frontends/llrm-c/src/lib.rs"], "full")
    for step, command in gate.commands(p, cfg, pkgs).items():
        assert not re.match(r"\s*[A-Z_]+=\S+\s+cargo", command), f"{step} sets its own environment for cargo: {command[:80]}"
    assert gate.STEP_ENV["RUSTC_BOOTSTRAP"] == "1"


def test_the_doc_step_runs_rustdoc_only_where_a_doc_comment_has_a_fence_to_run(tmp_path):
    """`cargo test --doc` took 456 CPU-s of a 4,340 CPU-s gate to run no test: every fence in the tree is ```text. Rustdoc runs when a doc
    comment has a fence it would run: an untagged one, rust, no_run, should_panic, compile_fail, ignore."""
    import doctests

    def has(text):
        (tmp_path / "a.rs").write_text(text)
        return doctests.has_doctests([tmp_path])

    assert not has("//! ```text\n//! a b\n//! ```\n")
    assert not has("/// ```sh\n/// make\n/// ```\nfn f() {}\n")
    assert has("//! ```\n//! let a = 1;\n//! ```\n")
    assert has("/// ```rust\n/// let a = 1;\n/// ```\n") and has("/// ```no_run\n/// f();\n/// ```\n")
    assert has("/// ```should_panic\n/// f();\n/// ```\n") and has("/// ```edition2021\n/// f();\n/// ```\n")
    assert not has("/// ```text\n/// x\n/// ```\n/// ```text\n/// y\n/// ```\n"), "a closing fence is not an opening one"
    command = gate.commands(gate.plan(["crates/frontends/llrm-c/src/lib.rs"], "full"), gate.load(), gate.packages())["doc"]
    assert command.startswith("if python3 tools/gate/doctests.py") and "--doc" in command, command
    skipped = subprocess.run(["bash", "-c", command.replace("python3 tools/gate/doctests.py crates src tests", "false")], capture_output=True, text=True)
    assert skipped.returncode == 0 and gate.incomplete(skipped.stdout, None, False) is None, "a skipped doc step reads as a cut-short run: " + skipped.stdout


def test_the_build_makes_one_set_of_crates_for_the_binaries_and_the_tests():
    """`cargo build --bins` and `cargo test --no-run` each built every crate (238 and 136 CPU-s) and the second linked its llrm-c over
    the first's: the steps ran the test build's binary and the plain build was waste."""
    assert "cargo build" not in gate.BUILD and "cargo test --release -q --workspace --no-run" in gate.BUILD


def test_a_diff_without_compiler_source_plans_only_the_steps_that_cover_its_files():
    """The user's rule: a PR that changes nothing under crates/ or in the Cargo files does not run the compiler gate, it merges on narrow
    tests of what it changed. A gate.py edit planned the full tier (every step, 4,340 CPU-s); its own tests are the pytest step."""
    assert gate.plan(["tools/gate/gate.py", "tools/gate/test_gate.py"]).steps == ["pytest"]
    assert gate.plan(["tools/measure.py"]).steps[-1] == "measure" and "lib" not in gate.plan(["tools/measure.py"]).steps
    narrow = gate.plan(["tests/target_facts.rs"])
    assert narrow.tier == "fast" and narrow.steps == ["build", "pytest", "integration", "fmt"], narrow
    assert gate.plan(["tools/torture/torture.py"]).steps == ["build", "pytest", "torture"]
    assert "build" not in gate.plan(["tools/gate/tiers.toml"]).steps
    # compiler source, or a Cargo file anywhere, takes the tiers as before
    assert gate.plan(["crates/ir/llrm-mir/src/lib.rs", "tools/gate/gate.py"]).tier == "full"
    assert gate.plan(["Cargo.toml"]).tier == "full" and gate.plan(["crates/opt/llrm-analysis/Cargo.toml"]).steps[0] == "build"
    assert "lib" in gate.plan(["crates/opt/llrm-analysis/src/ranges.rs"]).steps
    # asking for the full tier still runs every step: the gate's own PRs run the gate
    assert gate.plan(["tools/gate/gate.py"], "full").tier == "full" and len(gate.plan(["tools/gate/gate.py"], "full").steps) > 8


def test_a_benchmark_added_on_its_own_plans_every_step_that_reads_bench():
    """#1458 merged jpeg_single and jpeg_fixed on a run of the bench step alone and broke pytest-programs, measure and the integration tests
    that iterate bench/ (dwarf, objwriters, toolchain) on main. The steps are the ones whose tests name the directory, found in the tests."""
    steps = set(gate.plan(["bench/jpeg_single/jpeg_single.c", "bench/jpeg_single/jpeg_single.bas"]).steps)
    assert {"bench", "pytest-programs", "measure", "integration"} <= steps, steps
    assert "lib" not in steps or any("bench" in text for path, text in gate.sources().items() if path.startswith("crates/")), steps
    readers = gate.readers("bench", gate.load())
    assert "integration" in readers  # pytest-programs and measure come from tiers.toml's owners of bench/
    assert gate.step_of_source("tests/dwarf.rs", gate.load()) == "integration" and gate.step_of_source("tests/cv4.rs", gate.load()) == "cv4"
    # a directory no test names adds nothing
    assert gate.readers("no_such_" + "directory_anywhere", gate.load()) == set()
    assert "integration" in gate.plan(["examples/hello.c"]).steps or "pytest-programs" in gate.plan(["examples/hello.c"]).steps


def test_a_step_with_no_log_growth_and_no_cpu_is_killed_with_its_last_lines(tmp_path):
    """slopes' gate sat 84 minutes in pytest on a hung test, no output, nothing noticing. A step that neither writes nor computes for the
    stall time is killed and fails with a line saying why; one that writes, or computes, is not."""
    import subprocess as sp
    import time as _time

    def sp_out(command):
        return sp.run(command, capture_output=True, text=True).stdout

    env = {**__import__("os").environ}
    began = _time.time()
    _, code, wall, _ = gate.run_step("hang", "echo before the hang; sleep 63.7", tmp_path, env, stall=2)
    assert code == gate.STALL_CODE and wall < 20 and _time.time() - began < 20, (code, wall)
    log = (tmp_path / "hang.failed.log").read_text()
    assert "before the hang" in log and "STALLED" in log
    # writing keeps a step alive without CPU; computing keeps it alive without writing
    assert gate.run_step("chatty", "for i in 1 2 3 4 5 6; do echo $i; sleep 0.5; done", tmp_path, env, stall=2)[1] == 0
    spin = 'python3 -c "import time; t = time.process_time()\nwhile time.process_time() - t < 4: pass"'
    assert gate.run_step("spin", spin, tmp_path, env, stall=2)[1] == 0
    # the killed step took its subprocesses with it
    assert "sleep 63.7" not in sp_out(["ps", "-eo", "args"]), "the stalled step's sleep outlived it"


def test_a_test_over_its_cpu_limit_fails_with_its_name_and_its_subprocess_is_killed(tmp_path):
    """The pytest step limited nothing: a test that computes without end ran until the gate was killed. The plugin counts CPU, its
    subprocesses' included, as pytest-timeout cannot, and names the test."""
    import os
    import subprocess as sp
    import sys as _sys

    (tmp_path / "test_spins.py").write_text(
        "import subprocess, sys\n\ndef test_a_compile_that_never_ends():\n    subprocess.run([sys.executable, '-c', 'while True: pass  # cpulimit-test-spinner'])\n\ndef test_fine():\n    assert True\n"
    )
    here = str(Path(gate.__file__).parent)
    env = {**os.environ, "GATE_TEST_CPU_SECONDS": "1", "PYTHONPATH": here}
    done = sp.run([_sys.executable, "-m", "pytest", "-p", "pytest_cpulimit", "-p", "no:cacheprovider", "-q", str(tmp_path / "test_spins.py")], capture_output=True, text=True, env=env, timeout=60, cwd=tmp_path)
    out = done.stdout + done.stderr
    assert done.returncode == 1 and "1 failed, 1 passed" in out, out
    assert "CPU LIMIT" in out and "test_a_compile_that_never_ends" in out, out
    assert "cpulimit-test-spinner" not in sp.run(["ps", "-eo", "args"], capture_output=True, text=True).stdout, "the test's subprocess outlived the limit"
    assert "-p pytest_cpulimit" in gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages())["pytest"]


def test_a_failed_step_prints_what_its_failing_tests_printed_not_only_the_log_tail():
    """CI's integration job said `1 target failed: -p llrm --test dwarf` and nothing of why: the gate printed the last 30 lines of the log,
    and the panic message was far above them."""
    cargo = "\n".join(
        ["running 22 tests", "...", "failures:", "", "---- a_test_that_needs_a_tool stdout ----", "",
         "thread 'a_test_that_needs_a_tool' panicked at tests/dwarf.rs:16:5:", "LLRM_REQUIRE_DWARF is set, and: needs Open Watcom's wdump (WDUMP)", "",
         "failures:", "    a_test_that_needs_a_tool", "", "test result: FAILED. 21 passed; 1 failed"]
        + ["running 3 tests"] * 40
        + ["error: 1 target failed:", "    `-p llrm --test dwarf`"]
    )
    shown = gate.failure_text(cargo)
    assert "LLRM_REQUIRE_DWARF is set, and: needs Open Watcom's wdump" in shown and "a_test_that_needs_a_tool" in shown
    assert "running 3 tests" not in shown
    pytest_log = "\n".join(["= FAILURES =", "___ test_x ___", "    def test_x():", ">       assert 1 == 2", "E       assert 1 == 2", "= short test summary info =", "FAILED test_x.py::test_x"])
    assert "E       assert 1 == 2" in gate.failure_text(pytest_log)
    assert gate.failure_text("everything passed\n") == ""


def test_what_a_runner_lacks_is_a_named_skip_and_what_it_needs_is_installed(tmp_path):
    """GitHub CI was red on every commit: `tests/dwarf.rs` failed for want of Open Watcom's wdump (LLRM_REQUIRE_DWARF made a missing tool a
    failure), the QB45 runtime-library tests errored in LIB.EXE for want of QB45 (now only the tests that compare with BCOM45 need it), and the vsgcc harness could not link gcc -m32 programs
    (no 32-bit libgcc: __divdi3). A tool the runner cannot have is a capability the gate names and skips; one it can have is installed."""
    host = {"HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1"}
    assert "wdump" in gate.missing_capabilities(host) and "LLRM_REQUIRE_WDUMP" in gate.load()["capability"]["wdump"]["require"]
    (tmp_path / "wdump").write_text("")
    assert "wdump" not in gate.missing_capabilities({**host, "WDUMP": str(tmp_path / "wdump")})
    assert "LLRM_REQUIRE_WDUMP" in (gate.ROOT / "tools/debug-gate.env").read_text()
    declared = gate.declared_requirements()
    # BCOM45 is the oracle of these; the llrm side (jwlink) of every QB test needs no QB45
    for test in ("test_qbbc.py", "test_qbgfx.py", "test_qbgfxstep.py", "test_qbdemos.py", "test_qbrt.py"):
        assert declared.get(f"tools/dosbatch/{test}") == ["qb45"], test
    for test in ("test_qb32.py", "test_qbgfxspeed.py", "test_qbgfxfill.py", "test_qbgorilla.py", "test_qbkeys.py", "test_qbstrblocks.py", "test_qbentries.py"):
        assert declared.get(f"tools/dosbatch/{test}") is None, test
    assert "gcc-multilib" in (gate.ROOT / ".github/actions/setup/action.yml").read_text()


def test_wdump_is_built_from_the_pinned_tree_cached_and_required_in_ci(tmp_path):
    """tests/dwarf.rs skipped three tests on every runner for want of Open Watcom's wdump. toolchain/wdump.sh builds it from the tree
    owshim/ow-commit pins (the one wccq is relinked from, about a second once it is cached) and prints its path; the setup action exports
    that as WDUMP with LLRM_REQUIRE_WDUMP, so a build that failed is a red job and not a skip."""
    import os
    import subprocess as sp

    commit = (gate.ROOT / "toolchain/owshim/ow-commit").read_text().strip()
    tree = tmp_path / "llrm" / f"open-watcom-v2-{commit}"
    (tree / "build/binbuild").mkdir(parents=True)
    (tree / "bld/exedump").mkdir(parents=True)
    (tree / "bld/exedump/binmake").write_text("")
    (tree / ".complete").write_text("")
    (tree / "setvars.sh").write_text("")
    # A wmake that "builds" wdump.exe, once, in the directory it is run in.
    wmake = tree / "build/binbuild/wmake"
    native = {"Linux": r"\177ELF", "Darwin": r"\317\372\355\376"}[sp.run(["uname", "-s"], capture_output=True, text=True).stdout.strip()]
    wmake.write_text(f"#!/bin/sh\necho built >> \"$OWROOT_BUILDS\"\nprintf '{native}a-wdump' > wdump.exe\n")
    wmake.chmod(0o755)
    env = {**os.environ, "XDG_CACHE_HOME": str(tmp_path), "OWROOT_BUILDS": str(tmp_path / "builds")}
    env.pop("OWROOT", None)
    script = str(gate.ROOT / "toolchain/wdump.sh")
    made = sp.run(["sh", script], capture_output=True, text=True, env=env)
    assert made.returncode == 0, made.stderr
    path = Path(made.stdout.strip())
    assert path == tmp_path / "llrm" / f"wdump-{commit}" / "wdump" and path.read_bytes().endswith(b"a-wdump")
    assert sp.run(["sh", script], capture_output=True, text=True, env=env).stdout.strip() == str(path)
    assert (tmp_path / "builds").read_text() == "built\n", "the second call built it again"
    # an image that is not the host's own format (a DOS MZ executable) is refused, and not installed
    path.parent.joinpath(".complete").unlink()
    wmake.write_text("#!/bin/sh\nprintf 'MZ-a-dos-image' > wdump.exe\n")
    refused = sp.run(["sh", script], capture_output=True, text=True, env=env)
    assert refused.returncode != 0 and "is not a" in refused.stderr and "executable" in refused.stderr, refused.stderr
    assert not path.parent.exists(), "a refused image was cached"
    # a build that fails fails the script: no path, nothing cached
    wmake.write_text("#!/bin/sh\nexit 1\n")
    assert sp.run(["sh", script], capture_output=True, text=True, env=env).returncode != 0
    action = (gate.ROOT / ".github/actions/setup/action.yml").read_text()
    assert "sh toolchain/wdump.sh" in action and 'echo "WDUMP=$wdump" >> "$GITHUB_ENV"' in action and "LLRM_REQUIRE_WDUMP=1" in action
    assert action.count("'toolchain/wdump.sh'") == 2, "both cache keys hold the script that builds it"
