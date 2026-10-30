"""The gate's path map: a change in an area selects that area's tests, and nothing is left out by omission."""

import os
import re
import pytest
import sys
import subprocess
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import gate  # noqa: E402

ROOT = gate.ROOT



def ci_needs(workflow: str) -> list[str]:
    """The last three jobs `ci:` waits for: the ones before them (static, toolchain, pintools, build) come and go."""
    return re.search(r"\n  ci:\n    needs: \[([^\]]*)\]", workflow)[1].split(", ")[-3:]


def test_a_change_in_the_c_frontend_runs_the_c_programs_and_qcport_not_the_debugger_sessions():
    p = gate.plan(["crates/driver/llrm-cc/src/lib.rs"])
    assert p.tier == "fast"
    assert p.languages == ["c"]
    assert "qcport" in p.steps and "run" in p.steps
    assert not {"turbo", "cv4", "identity"} & set(p.steps), p.steps


def test_a_change_in_the_omf_writer_runs_the_debugger_sessions():
    p = gate.plan(["crates/mc/llrm-omf/src/omf.rs"])
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
    assert gate.plan(["docs/working/testing.md", "readme.md"]).tier == "none"


def test_lib_tests_run_in_the_changed_crate_and_its_dependents_only():
    p = gate.plan(["crates/driver/llrm-cc/src/lib.rs"])
    pkgs = gate.packages()
    assert "llrm-cc" in p.packages
    assert "llrm-transforms" not in p.packages
    assert set(p.packages) == gate.dependents(pkgs, {"llrm-cc"}) | {"llrm", "llrm-mir"}


def test_a_change_in_any_crate_runs_the_lib_tests_that_read_the_sources_of_the_others():
    """facts_rewrite's audit of the readers of nsw/nuw/inbounds is in llrm-mir, which a pass in llrm-transforms does not
    reach as a dependent: #1324 added a reader, the gate ran no llrm-mir test, and main was red."""
    p = gate.plan(["crates/opt/llrm-transforms/src/algebraic.rs"])
    assert "llrm-mir" in p.packages and "lib" in p.steps


def test_a_python_only_change_runs_the_python_tests_and_not_the_build():
    p = gate.plan(["tools/targets/msp430.py"])
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


def test_a_root_test_named_like_a_members_runs_only_the_root_one():
    """gate run 3 (c1d651c7b976) said "INCOMPLETE: 2 results of 1 test binaries": a bare `cargo test --test run` also ran llrm-arm64's
    tests/run.rs (24 tests, all filtered out by the step's filter), and the step counts one binary."""
    pkgs = gate.packages()
    cmds = gate.commands(gate.plan(["crates/ir/llrm-mir/src/lib.rs"]), gate.load(), pkgs)
    shared = {t for _, t in gate.crate_tests(pkgs)} & set(gate.root_tests())
    assert "run" in shared, shared
    for name in shared:
        for step, cmd in cmds.items():
            if step != "crate-tests" and re.search(rf"--test {name}(\s|$)", cmd):
                assert f"-p llrm --test {name}" in cmd or re.search(rf"-p llrm (--test \w+ )*--test {name}(\s|$)", cmd), (step, cmd)


def test_the_test_that_writes_into_the_tree_runs_alone_after_the_run_test_could_see_it():
    """It left crates/target/llrm-x86-m32/.rerun-probe in the tree while `run` compared the tree: `run` failed, 'the run left files'."""
    cfg = gate.load()
    assert "build_script_reruns" in cfg["exclusive"].values()
    p = gate.plan(["crates/driver/llrm-cc/src/lib.rs"])
    assert "build-script-reruns" in p.steps
    assert "build_script_reruns" not in gate.commands(p, cfg, gate.packages())["integration"]


def test_a_scoped_lib_run_includes_the_root_crate_that_turns_on_the_features_the_crate_needs():
    """`cargo test -p llrm-c --lib` alone built llrm-c without its toolchain feature: 16 tests failed 'llrm was built without the toolchain feature'."""
    cfg, pkgs = gate.load(), gate.packages()
    p = gate.plan(["crates/driver/llrm-cc/src/lib.rs"])
    assert "-p llrm " in gate.commands(p, cfg, pkgs)["lib"] + " "


def test_a_backend_change_takes_the_fast_tier_with_every_codegen_step_and_no_debugger_session():
    p = gate.plan(["crates/codegen/llrm-codegen/src/backend/isel.rs"])
    assert p.tier == "fast"
    assert {"run", "qcport", "pytest-programs", "bench", "torture"} <= set(p.steps)
    assert p.languages == ["qb", "c", "nib"]
    assert not {"turbo", "cv4", "identity"} & set(p.steps), p.steps


def test_a_change_to_the_debug_info_emitter_runs_the_debugger_sessions():
    p = gate.plan(["crates/codegen/llrm-codegen/src/backend/debuginfo.rs"])
    assert {"turbo", "cv4"} <= set(p.steps), p.steps


def test_a_bisect_finds_the_first_commit_that_fails_and_runs_the_steps_only_log_n_times():
    commits = [f"c{i}" for i in range(20)]
    for first in (0, 7, 19):
        asked = []
        found = gate.first_bad(commits, lambda c: asked.append(c) or int(c[1:]) >= first)
        assert found == f"c{first}" and len(asked) <= 5, (first, found, asked)


def test_the_run_step_runs_only_the_languages_the_diff_can_affect():
    cfg, pkgs = gate.load(), gate.packages()
    p = gate.plan(["crates/frontends/nib/src/lib.rs"])
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
    """A change to tools/qcport-run-cc.py planned `pytest qcport` with no build: qcport ran against missing or stale binaries."""
    for path in ("tools/qcport-run-cc.py", "tools/torture/torture.py"):
        p = gate.plan([path])
        assert p.steps[0] == "build", (path, p.steps)
    assert "build" not in gate.plan(["tools/targets/msp430.py"]).steps


def test_the_qcport_step_has_no_wall_clock_limit_of_its_own():
    """`timeout 400` around qcport-run-cc killed a run that was still drawing at load: qcport-run-cc stops a stall and caps a hang itself."""
    p = gate.plan(["tools/qcport-run-cc.py"])
    assert "timeout" not in gate.commands(p, gate.load(), gate.packages())["qcport"]


def test_the_torture_step_has_no_wall_clock_limit_of_its_own():
    """`timeout 600` killed a full gate's torture step at 150 of 474 programs under load: every program's budget is guest time, so
    the step's wall time is the host's load. dosbatch stops an emulator that stops reporting."""
    p = gate.plan(["tools/torture/torture.py"])
    assert "timeout" not in gate.commands(p, gate.load(), gate.packages())["torture"]


def test_every_step_the_planner_can_produce_is_in_exactly_one_ci_group():
    """A step in no group would never run in CI, silently: the matrix is built from the groups."""
    plans = [gate.plan(["crates/ir/llrm-mir/src/lib.rs"], "full"), gate.plan(["crates/ir/llrm-mir/src/lib.rs"], "auto"), gate.plan(["crates/backend/x.rs"], "fast"), gate.plan(["tools/gate/gate.py"]), gate.plan(["crates/backend/x.rs", "docs/a.md"], "smoke")]  # `scans` and `measure` are fast-tier steps
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


def test_a_require_variable_set_to_zero_does_not_require_the_tool(tmp_path):
    """LLRM_REQUIRE_TURBO=0 counted as set (a truthy string), so a host without Turbo failed instead of skipping; the compilers read `0` as off (#2279)."""
    env = {"HOME": str(tmp_path), "LLRM_REQUIRE_TURBO": "0", "GATE_ALLOW_MISSING": "1"}
    assert "turbo" in gate.missing_capabilities(env)


def test_a_tool_that_is_present_is_not_missing(tmp_path):
    (tmp_path / "bin").mkdir()
    (tmp_path / "bin/TCC.EXE").touch()
    (tmp_path / "Td.exe").touch()
    env = {"HOME": str(tmp_path), "TCPP30_DIR": str(tmp_path), "TD_DIR": str(tmp_path), "GATE_ALLOW_MISSING": "1"}
    assert "turbo" not in gate.missing_capabilities(env)


def test_ci_groups_hold_only_the_steps_that_can_run_here():
    p = gate.plan(["crates/ir/llrm-mir/src/lib.rs"])
    skipped = {"turbo": "x", "cv4": "x", "qcport": "x", "oracle-qcport": "x"}
    groups = gate.groups_of(p, skipped)
    assert "reference" not in groups and "turbo" not in sum(groups.values(), [])


def test_the_plan_names_each_skipped_step_and_its_missing_tool_as_json(tmp_path):
    env = {**__import__("os").environ, "HOME": str(tmp_path), "LLRM_REQUIRE_TURBO": "", "LLRM_REQUIRE_CODEVIEW": "", "QB45_DIR": "", "TCPP30_DIR": "", "TD_DIR": "", "VBDOS_DIR": "", "QCPORT": "", "QCPORT_BORLAND": "", "GATE_ALLOW_MISSING": "1"}
    out = subprocess.run([sys.executable, str(Path(gate.__file__)), "plan", "--json", "--files", "crates/ir/llrm-mir/src/lib.rs"], capture_output=True, text=True, check=True, env=env).stdout
    got = __import__("json").loads(out)
    assert set(got["skipped"]) - {"measure", "pytest-perf", "oracle-premise"} == {"turbo", "cv4", "qcport", "oracle-qcport", "bench", "dos-slow", "integration"} and "reference" not in got["groups"]
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


def _fake_step(tmp_path, monkeypatch, body: str):
    for name, value in {"CARGO_TARGET_DIR": str(tmp_path / "t"), "HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1"}.items():
        monkeypatch.setenv(name, value)
    monkeypatch.setenv("LLRM_BIN", str(tmp_path / "bin"))
    monkeypatch.setattr(gate, "commands", lambda *a: {"probe": body})
    return gate.execute(gate.Plan("fast", "test", steps=["probe"]))[0]


def test_a_failed_step_prints_the_lines_it_tagged_for_the_job_log(tmp_path, monkeypatch, capsys):
    """Run 6's dos step failed and its job log had neither lscpu nor the x87 verdict nor the hashes: they were printed before the tests, into the
    step's own log file, and the job log shows only a failed step's failing tests and the tail of its log."""
    body = 'echo "[diag!] Model name: Test CPU"; echo "[diag] hash abc demo conf"; for i in $(seq 60); do echo "filler $i"; done; printf "######## t.py: exit 1\\n___ test_x ___\\nassert 1 == 2\\n"; exit 1'
    assert _fake_step(tmp_path, monkeypatch, body) == 1
    shown = capsys.readouterr().out
    assert "[diag!] Model name: Test CPU" in shown and "[diag] hash abc demo conf" in shown


def test_a_passing_step_prints_only_the_always_tagged_lines(tmp_path, monkeypatch, capsys):
    assert _fake_step(tmp_path, monkeypatch, 'echo "[diag!] x87 probe: all 92 lines match"; echo "[diag] hash abc"; exit 0') == 0
    shown = capsys.readouterr().out
    assert "[diag!] x87 probe: all 92 lines match" in shown and "[diag] hash abc" not in shown


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
    assert sorted(gate.python_tests_unusable(missing)) == ["crates/target/llrm-x86-m32/vsgcc/test_scaling.py", "crates/target/llrm-x86-m32/vsgcc/test_scaling_gate.py", "tools/test_parity_smoke_perf.py", "tools/test_scoreboard_perf.py"]  # by name: a new counter test is a new name here
    p = gate.plan(["tools/linkrecipe.py"])
    cmds = gate.commands(p, gate.load(), gate.packages(), frozenset(), tuple(gate.python_tests_unusable(missing)))
    assert "--ignore=crates/target/llrm-x86-m32/vsgcc/test_scaling.py" in cmds["pytest"] and "--ignore=crates/target/llrm-x86-m32/vsgcc/test_scaling_gate.py" in cmds["pytest"]
    whole = gate.commands(p, gate.load(), gate.packages())
    assert "--ignore=crates/target/llrm-x86-m32/vsgcc/test_scaling.py" in whole["pytest"], "the perf files are the pytest-perf step's"
    assert "vsgcc/test_scaling.py" in whole["pytest-perf"] and "vsgcc/test_scaling_gate.py" in whole["pytest-perf"] and "--ignore" not in whole["pytest-perf"]


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
    assert "measure" in gate.plan(["crates/codegen/llrm-codegen/src/backend/isel.rs"]).steps
    assert "measure" in gate.plan(["crates/opt/llrm-transforms/src/gvn.rs"]).steps
    assert "measure" in gate.plan(["crates/ir/llrm-mir/src/lib.rs"]).steps  # full
    assert "measure" not in gate.plan(["tests/run.rs"]).steps
    assert "measure" not in gate.plan(["crates/frontends/qb/src/lib.rs"]).steps


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
    for touched in ("crates/opt/llrm-analysis/src/ranges.rs", "crates/codegen/llrm-codegen/src/backend/spiller.rs", "crates/driver/llrm-cc/src/lib.rs"):
        p = gate.plan([touched])
        assert p.tier == "fast" and "scans" in p.steps, (touched, p.steps)
    command = gate.commands(gate.plan(["crates/opt/llrm-analysis/src/ranges.rs"]), cfg, gate.packages())["scans"]
    for one in cfg["scan"]:
        if "package" in one:
            assert one["package"] in command and (one.get("lib") or one["test"]) in command, one
    assert "scans" not in gate.plan(["tools/measure.py"]).steps and "scans" not in gate.plan(["docs/working/testing.md"]).steps


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
    assert "&&" not in command.replace("cargo test", "") and command.count("|| rc=1") == command.count("cargo test") > 0 and command.endswith("exit $rc")


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
    assert "fmt" not in gate.plan(["tools/measure.py"]).steps and "fmt" not in gate.plan(["docs/working/testing.md"]).steps


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
    assert "rfmt-post" in gate.SERIAL_STEPS


def test_the_qcport_runs_wait_for_the_other_steps_to_finish():
    """Gate 4 ran qcport and oracle-qcport beside cv4 (541 CPU-s) and turbo: the emulator was starved and the runs drew differently from
    Borland's (oracle-qcport: 219 frames, 0 polys against 62 and 37637) or stalled for 300 s (qcport). Pinned to two cores under busy
    loops the run draws 0 frames on both sides. Their verdict is a count of frames in real time, so nothing else runs beside them."""
    assert {"qcport", "oracle-qcport"} <= gate.SERIAL_STEPS


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


def test_a_failed_step_keeps_its_log_past_the_next_run_of_the_step(tmp_path, monkeypatch):
    """A pytest failure under host load was lost: the gate prints the last 30 lines of the log and a rerun overwrote it, so the
    failing test was never named (#1184)."""
    monkeypatch.delenv("GATE_ALLOW_MISSING", raising=False)  # CI sets it, and a step that skips itself there fails
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
    p = gate.plan(["crates/driver/llrm-cc/src/lib.rs"])
    monkeypatch.delenv("LLRM_SHUFFLE_SEED", raising=False)
    command = gate.commands(p, cfg, pkgs)["lib"]
    assert command.endswith("-Zunstable-options --shuffle"), command
    monkeypatch.setenv("LLRM_SHUFFLE_SEED", "42")
    assert gate.commands(p, cfg, pkgs)["lib"].endswith("-Zunstable-options --shuffle-seed 42")


def test_every_step_runs_with_the_flags_the_build_used_and_the_base_build_with_none():
    """`integration` (`cargo test --test X`) without the build step's RUSTFLAGS rebuilt llrm-c and replaced target/release/llrm-c while
    `torture` was starting it ("No such file or directory"): the steps share one set of flags, and the base the measure builds, which
    is historical, has none."""
    assert gate.STEP_ENV == {"RUSTFLAGS": gate.WARNING_FLAGS, "RUSTC_BOOTSTRAP": "1", "CARGO_INCREMENTAL": "0"}
    assert f"RUSTFLAGS='{gate.WARNING_FLAGS}'" in gate.BUILD
    assert "RUSTFLAGS" in gate.MEASURE_BUILD and "env -u RUSTFLAGS cargo" in gate.MEASURE_BUILD
    assert "RUSTFLAGS" not in gate.MEASURE_BUILD.replace("env -u RUSTFLAGS", "")


def test_the_slow_dos_tier_is_a_heavy_step_selected_by_the_runtime_and_the_dos32_harness():
    """The demos and graphics sweeps (minutes) ran in the `pytest` step on every change; now they are a step of their own."""
    assert "dos-slow" in gate.plan(["runtime/qb/gpaint.c"]).steps
    assert "dos-slow" in gate.plan(["tools/dosbatch/qb32demos.py"]).steps
    assert "dos-slow" not in gate.plan(["crates/driver/llrm-cc/src/lib.rs"]).steps
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
    p = gate.plan(["crates/driver/llrm-cc/src/lib.rs"], "full")
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
    command = gate.commands(gate.plan(["crates/driver/llrm-cc/src/lib.rs"], "full"), gate.load(), gate.packages())["doc"]
    assert command.startswith("if python3 tools/gate/doctests.py") and "--doc" in command, command
    skipped = subprocess.run(["bash", "-c", command.replace("python3 tools/gate/doctests.py crates src tests", "false")], capture_output=True, text=True)
    assert skipped.returncode == 0 and gate.incomplete(skipped.stdout, None, False) is None, "a skipped doc step reads as a cut-short run: " + skipped.stdout


def test_an_indented_block_in_a_doc_comment_is_a_doctest_the_doc_step_must_run(tmp_path):
    """#738 (issue #1646): a module comment's pseudo-code line indented four spaces after a blank line is a Rust code block to rustdoc, and failed
    `cargo test --doc`; the gate's check looked at fences only, skipped the step, and nothing ran it. A list item's indented
    continuation is not a code block, nor is a block inside a ```text fence."""
    import doctests

    def has(text):
        (tmp_path / "a.rs").write_text(text)
        return doctests.has_doctests([tmp_path])

    assert has("//! Top.\n//!\n//!     OP(...) -> Y\n")
    assert has("/// Item.\n///\n///     f()\nfn f() {}\n")
    assert not has("//! - an item\n//!\n//!     its continuation\n")
    assert not has("//! ```text\n//!\n//!     indented in a text fence\n//! ```\n")
    assert not has("//! Top.\n//!     a continuation line, no blank before\n")


def test_the_build_makes_one_set_of_crates_for_the_binaries_and_the_tests():
    """`cargo build --bins` and `cargo test --no-run` each built every crate (238 and 136 CPU-s) and the second linked its llrm-c over
    the first's: the steps ran the test build's binary and the plain build was waste."""
    assert "cargo build" not in gate.BUILD and "cargo test --release -q --workspace --no-run" in gate.BUILD


def test_a_diff_without_compiler_source_plans_only_the_steps_that_cover_its_files():
    """The user's rule: a PR that changes nothing under crates/ or in the Cargo files does not run the compiler gate, it merges on narrow
    tests of what it changed. A gate.py edit planned the full tier (every step, 4,340 CPU-s); its own tests are the pytest step."""
    assert gate.plan(["tools/gate/gate.py", "tools/gate/test_gate.py"]).steps == ["pytest", "pytest-perf", "facts"]
    assert gate.plan(["tools/measure.py"]).steps[-1] == "measure" and "lib" not in gate.plan(["tools/measure.py"]).steps
    narrow = gate.plan(["tests/target_facts.rs"])
    assert narrow.tier == "fast" and narrow.steps == ["build", "pytest", "pytest-perf", "integration", "fmt"], narrow
    assert gate.plan(["tools/torture/torture.py"]).steps == ["build", "pytest", "pytest-perf", "facts", "torture"]
    assert "build" not in gate.plan(["tools/gate/tiers.toml"]).steps
    # compiler source, or a Cargo file anywhere, takes the tiers as before
    assert gate.plan(["crates/ir/llrm-mir/src/lib.rs", "tools/gate/gate.py"]).tier == "full"
    assert gate.plan(["Cargo.toml"]).tier == "full" and gate.plan(["crates/opt/llrm-analysis/Cargo.toml"]).steps[0] == "build"
    assert "lib" in gate.plan(["crates/opt/llrm-analysis/src/ranges.rs"]).steps
    # asking for the full tier still runs every step: the gate's own PRs run the gate
    assert gate.plan(["tools/gate/gate.py"], "full").tier == "full" and len(gate.plan(["tools/gate/gate.py"], "full").steps) > 8


def test_a_runtime_change_alone_plans_the_bench_whose_counts_it_moves():
    """#1669 changed runtime/qb/console.c and nothing else; its gate planned no bench, so the ten bq32 rows (which link that runtime and
    count the kernel's calls into it) moved by 1 to 13 instructions in 10^8 unseen, and the next four gates failed on main as their base."""
    assert "bench" in gate.plan(["runtime/qb/console.c"]).steps
    assert "bench" in gate.plan(["runtime/c/x86-m32/ext.asm"]).steps
    assert "bench" not in gate.plan(["docs/working/gate.md"]).steps


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
    # A sleep no other run of this test (the gate runs files side by side) has: the leak check looks for it in `ps`.
    sleeper = f"sleep 63.{__import__('os').getpid()}"
    _, code, wall, _ = gate.run_step("hang", f"echo before the hang; {sleeper}", tmp_path, env, stall=2)
    assert code == gate.STALL_CODE and wall < 120 and _time.time() - began < 120, (code, wall)
    log = (tmp_path / "hang.failed.log").read_text()
    assert "before the hang" in log and "STALLED" in log
    assert "the tree when it was killed:" in log and sleeper in log and "WCHAN" in log and "PPID" in log, "run 5's bench-base was killed and the log said not what it waited on"
    # writing keeps a step alive without CPU; computing keeps it alive without writing
    assert gate.run_step("chatty", "for i in 1 2 3 4 5 6; do echo $i; sleep 0.5; done", tmp_path, env, stall=2)[1] == 0
    spin = 'python3 -c "import time; t = time.process_time()\nwhile time.process_time() - t < 4: pass"'
    assert gate.run_step("spin", spin, tmp_path, env, stall=2)[1] == 0
    # the killed step took its subprocesses with it
    assert sleeper not in sp_out(["ps", "-eo", "args"]), "the stalled step's sleep outlived it"


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


def test_every_failing_pytest_file_shows_its_failure_and_a_log_without_tests_shows_its_head():
    """Gate 3's python group had nine red files and the log named the failure of the first four: the 80-line cap went to the files that sort
    first, and tools/test_oracles.py's text never reached CI. A step with no test output (oracle-qcport) showed only the tail of the
    emulator's logs, not the verdict line above them."""
    section = lambda name: [f"######## {name}: exit 1", "= FAILURES =", f"___ test_{name} ___", *[f"E   line {i} of {name}" for i in range(40)], "= short test summary info ="]
    names = [f"tools/test_{c}.py" for c in "abcdefghi"]
    shown = gate.failure_text("\n".join(line for name in names for line in section(name)))
    assert all(f"line 0 of {name}" in shown for name in names)
    head = gate.log_head("QCport built by llrm-c draws differently from the Borland build:\n  a run drew nothing\n" + "\n".join(f"tail {i}" for i in range(100)))
    assert "a run drew nothing" in head and "tail 99" not in head


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


def test_a_change_to_a_tool_the_integration_tests_run_plans_the_integration_step():
    """A diff of toolchain/wdump.sh (CI run 38234232991) planned unit, run and python and not integration, so the green run did not show
    the three tests/dwarf.rs tests that read the wdump it builds: they read WDUMP, and no test names the script."""
    for touched in ("toolchain/wdump.sh", "toolchain/jwbuild.sh", "tools/debug-gate.env"):
        assert "integration" in gate.plan([touched]).steps, touched
    assert "integration" not in gate.plan(["tools/measure.py"]).steps


def test_the_gate_runs_in_its_own_high_weight_scope_once_and_only_when_systemd_is_there():
    """Without it the gate shared one cgroup with every session's builds and was starved (17 min wall)."""
    cmd = gate.scoped(["python3", "gate.py", "run"], {}, "/usr/bin/systemd-run")
    assert cmd[:3] == ["/usr/bin/systemd-run", "--user", "--scope"] and cmd[cmd.index("-p") + 1] == "CPUWeight=1000" and cmd[-3:] == ["python3", "gate.py", "run"]
    assert any(c == "--setenv=GATE_SCOPED=1" for c in cmd)
    assert "CPUWeight=300" in gate.scoped(["x"], {"GATE_CPU_WEIGHT": "300"}, "/s")
    assert gate.scoped(["x"], {"GATE_SCOPED": "1"}, "/s") is None, "the re-executed gate must not wrap itself again"
    assert gate.scoped(["x"], {"GATE_CPU_WEIGHT": "0"}, "/s") is None
    assert gate.scoped(["x"], {}, "") is None


def test_steps_at_once_scale_with_the_cores_not_a_fixed_four():
    """JOBS defaulted to 4: a full gate ran four steps at a time on 32 cores, and its wall was the sum of the queue."""
    assert gate.default_jobs({}, 32) == 16 and gate.default_jobs({}, 4) == 4 and gate.default_jobs({"JOBS": "6"}, 32) == 6


def _shards(tmp_path, files: dict[str, str], *extra: str):
    import subprocess as sp
    import sys as _sys

    for name, body in files.items():
        (tmp_path / name).write_text(body)
    return sp.run([_sys.executable, str(Path(gate.HERE) / "pytest_shards.py"), "-j", "3", "test_a.py", "test_b.py", "test_c.py", *extra], capture_output=True, text=True, cwd=tmp_path, timeout=60)


def test_a_failing_test_in_any_shard_fails_the_run_and_is_printed(tmp_path):
    """Sharding must not hide a red: the failing file's output is printed whole and the exit is 1."""
    done = _shards(tmp_path, {"test_a.py": "def test_fine():\n    pass\n", "test_b.py": "def test_broken_in_b():\n    assert 1 == 2\n", "test_c.py": "def test_ok():\n    pass\n"})
    assert done.returncode == 1, done.stdout
    assert "test_broken_in_b" in done.stdout and "assert 1 == 2" in done.stdout and "FAILED files: test_b.py" in done.stdout
    assert "3 files, 3 runs, 1 failed" in done.stdout


def test_a_collection_error_in_a_shard_fails_the_run(tmp_path):
    """A file that cannot be imported collects no test; it must be a red, not a missing file."""
    done = _shards(tmp_path, {"test_a.py": "def test_fine():\n    pass\n", "test_b.py": "import no_such_module_anywhere\n", "test_c.py": "def test_ok():\n    pass\n"})
    assert done.returncode == 1 and "no_such_module_anywhere" in done.stdout


def test_all_shards_green_exits_zero_and_records_every_files_seconds(tmp_path):
    rec = tmp_path / "rec.json"
    done = _shards(tmp_path, {f"test_{x}.py": "def test_ok():\n    pass\n" for x in "abc"}, "--durations", str(rec))
    assert done.returncode == 0, done.stdout
    import json as _json

    assert sorted(_json.loads(rec.read_text())) == ["test_a.py", "test_b.py", "test_c.py"]


def test_a_must_pass_file_that_skips_every_test_fails_the_run(tmp_path):
    """Run 5's NIBBLES red and run 4's 58 s dos PASS: a demo file whose inputs were missing skipped every test and the step went green.
    A --must-pass file has to pass a test and skip none."""
    files = {"test_a.py": "def test_fine():\n    pass\n", "test_b.py": "import pytest\n\n\n@pytest.mark.skip(reason='no demos')\ndef test_demo():\n    pass\n", "test_c.py": "def test_ok():\n    pass\n"}
    assert _shards(tmp_path, files).returncode == 0
    done = _shards(tmp_path, files, "--must-pass", "test_b.py")
    assert done.returncode == 1 and "test_b.py is a must-pass file and ran 0 passed, 1 skipped" in done.stdout, done.stdout


def test_a_failed_rerun_file_is_run_once_more_and_both_verdicts_are_printed(tmp_path):
    """NIBBLES m16 failed in CI run 4 and 5 and passed locally: a red that cannot say whether it fails again cannot be told from a flaky one."""
    marker = tmp_path / "seen"
    body = f"import pathlib\n\n\ndef test_first_time_only():\n    m = pathlib.Path({str(marker)!r})\n    seen = m.exists()\n    m.write_text('x')\n    assert seen\n"
    done = _shards(tmp_path, {"test_a.py": "def test_fine():\n    pass\n", "test_b.py": body, "test_c.py": "def test_ok():\n    pass\n"}, "--rerun", "test_b.py")
    assert done.returncode == 1 and "test_b.py failed, rerun passed" in done.stdout, done.stdout


def test_the_shard_summary_counts_the_tests_passed_and_skipped(tmp_path):
    """A vacuous green must be visible in the log: the summary says how many tests passed and how many skipped."""
    done = _shards(tmp_path, {"test_a.py": "def test_fine():\n    pass\n\n\ndef test_two():\n    pass\n", "test_b.py": "import pytest\n\n\n@pytest.mark.skip\ndef test_demo():\n    pass\n", "test_c.py": "def test_ok():\n    pass\n"})
    assert "3 passed, 1 skipped" in done.stdout, done.stdout


def test_the_dos_step_requires_the_demo_tests_to_pass():
    cmd = gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages())["dos-slow"]
    assert "--must-pass tools/dosbatch/test_qbdemos.py" in cmd and "--rerun tools/dosbatch/test_qbdemos.py" in cmd


def test_the_dos_step_prints_the_host_cpu_and_the_emulator_it_ran_on():
    """Run 4 and 5 failed NIBBLES m16 on a hosted runner and the log had no CPU model, no DOSBox build and no core: nothing to compare with a green local run."""
    cmd = gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages())["dos-slow"]
    assert "lscpu" in cmd and "dosbox-x" in cmd and "qb32.DEMO_CONF" in cmd
    assert "[diag!] " in cmd and "[diag] " in cmd  # tagged, so the gate copies them into the job log
    assert "rustc -vV" in cmd and "demo-hashes.txt" in cmd and "exit $rc" in cmd  # the build's hashes are printed on green and red
    assert "tools/dosbatch/x87probe.py || true;" in cmd  # the host FPU's verdict is printed, never a red of its own


def test_the_dos_step_plays_nibbles_under_a_conf_matrix_after_the_tests_and_names_the_emulators_compiler():
    """Run 6 died at 38 pixels with nothing to say whether the emulator or the compiler did it: the matrix lines and the compiler of the
    CI's dosbox-x go to the job log, and the matrix runs after the tests so their verdict stays the step's."""
    cmd = gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages())["dos-slow"]
    assert cmd.index("pytest_shards.py") < cmd.index("rc=$?") < cmd.index("tools/dosbatch/nibprobe.py") < cmd.index("exit $rc")
    assert "nibprobe.py 2>&1 | sed" in cmd and "readelf -p .comment" in cmd  # the .comment section names the build's compiler; a `strings` grep found a bundled library's


def test_the_pytest_step_runs_through_the_sharder_with_the_ignores_and_the_cpu_limit():
    cmd = gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages(), skip_py=("tests/x.py",))["pytest"]
    assert "pytest_shards.py" in cmd and "--ignore=tests/x.py" in cmd and "-- -q" in cmd and "-p pytest_cpulimit" in cmd


def test_a_file_recorded_slow_is_cut_into_parts_that_together_hold_every_test_once(monkeypatch):
    """One file (test_scaling_gate.py, 429 s) was the floor of the whole sharded run: it is dealt out over several processes."""
    import pytest_shards

    monkeypatch.setattr(pytest_shards, "SPLITTABLE", frozenset({"big.py"}))
    ids = [f"big.py::test_{i}" for i in range(10)]
    work = pytest_shards.units({"big.py": ids, "small.py": ["small.py::test_x"]}, {"big.py": 300.0, "small.py": 2.0})
    big = [args for label, args in work if label.startswith("big.py")]
    assert len(big) == 10 and sorted(sum(big, [])) == sorted(ids)
    assert [a for l, a in work if l == "small.py"] == [["small.py"]]
    assert work[0][0].startswith("big.py"), "longest first"


def test_a_run_that_reports_fewer_tests_than_were_collected_fails(tmp_path):
    """A shard killed or filtered to nothing must not read as green: the counts must agree."""
    (tmp_path / "test_a.py").write_text("def test_ok():\n    pass\n\n\ndef test_two():\n    pass\n")
    (tmp_path / "conftest.py").write_text("def pytest_collection_modifyitems(config, items):\n    if not config.option.collectonly:\n        del items[1:]\n")
    import subprocess as sp
    import sys as _sys

    done = sp.run([_sys.executable, str(Path(gate.HERE) / "pytest_shards.py"), "-j", "2", "test_a.py"], capture_output=True, text=True, cwd=tmp_path, timeout=60)
    assert done.returncode == 1 and "2 tests collected, 1 reported" in done.stdout, done.stdout


def test_a_file_with_a_module_fixture_is_never_cut_into_parts():
    """test_qbrt.py's module fixture runs every program in one DOSBox launch: cut in five it ran five launches, 185 s each against 134 s whole."""
    import pytest_shards

    ids = [f"tools/dosbatch/test_qbrt.py::test_{i}" for i in range(40)]
    assert [a for _, a in pytest_shards.units({"tools/dosbatch/test_qbrt.py": ids}, {"tools/dosbatch/test_qbrt.py": 500.0})] == [["tools/dosbatch/test_qbrt.py"]]


def test_flags_after_the_double_dash_reach_pytest_with_their_values(tmp_path):
    """`-- -m slow_dos` made `slow_dos` a path and `-m` a flag with no value: nothing was collected."""
    (tmp_path / "test_a.py").write_text("import pytest\n\n\n@pytest.mark.marked\ndef test_in():\n    pass\n\n\ndef test_out():\n    pass\n")
    done = _shards(tmp_path, {"test_b.py": "def test_ok():\n    pass\n", "test_c.py": "def test_ok():\n    pass\n"}, "--", "-m", "marked", "-q")
    assert done.returncode == 0 and "1 files" in done.stdout, done.stdout


def test_the_dos_slow_step_runs_through_the_sharder_with_its_marker():
    """One pytest process played every slow DOS test in turn: 283 s."""
    cmd = gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages())["dos-slow"]
    assert "pytest_shards.py" in cmd and "-- -m slow_dos" in cmd and "tools/dosbatch" in cmd


def test_the_turbo_binary_runs_whole_in_the_turbo_step_and_not_in_integration():
    """Its five Turbo Debugger tests sleep-polled for 168 s inside `integration`, which every code change runs."""
    cmds = gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages())
    assert cmds["turbo"].endswith("--test turbo") and "--skip" not in cmds["turbo"]
    assert "--test turbo" not in cmds["integration"].replace("--test turbo_", "")
    assert "--test cv4" not in cmds["integration"]


def test_a_change_to_a_python_or_shell_file_in_a_scanned_tree_plans_the_target_facts_test_alone():
    """tests/target_facts.rs reads them: a tools-only PR planned only pytest and landed four copies of a target fact; main went red."""
    for path in ("tools/gate/pytest_shards.py", "tools/dosbatch/qbplay.py"):
        assert "facts" in gate.plan([path]).steps, path
    assert "integration" in gate.plan(["crates/target/x/run.sh"]).steps, "with compiler source the integration step has the test"
    assert "facts" not in gate.plan(["tools/readme.md"]).steps and "build" not in gate.plan(["tools/gate/pytest_shards.py"]).steps


NAME, MAKER = "Clau" + "de", "Anthro" + "pic"  # spelt in halves: the smoke tier refuses a diff that carries them


def _repo(tmp_path):
    import subprocess as sp

    def git(*args, cwd=tmp_path):
        return sp.run(["git", *args], cwd=cwd, capture_output=True, text=True, check=True, env={**__import__("os").environ, "GIT_AUTHOR_NAME": "a", "GIT_AUTHOR_EMAIL": "a@a", "GIT_COMMITTER_NAME": "a", "GIT_COMMITTER_EMAIL": "a@a"}).stdout.strip()

    git("init", "-q", "-b", "main")
    (tmp_path / "f").write_text("one\n")
    git("add", "f")
    git("commit", "-q", "-m", "base")
    git("update-ref", "refs/remotes/origin/main", "HEAD")
    return git


def test_smoke_plans_the_fast_tiers_steps_cut_to_the_smoke_list_with_docs_and_the_diffs_tests():
    p = gate.plan(["crates/codegen/llrm-codegen/src/a.rs", "tools/gate/test_gate.py"], "smoke")
    assert p.tier == "smoke" and p.steps[0] == "build"
    assert set(p.steps) <= set(gate.load()["smoke"]["steps"]) and {"docs", "changed-tests", "pytest-programs", "fmt"} <= set(p.steps)
    assert not {"run", "bench", "torture", "qcport", "measure", "identity", "turbo", "cv4", "dos-slow", "integration", "pytest"} & set(p.steps)
    assert "facts" in gate.plan(["tools/gate/pytest_shards.py"], "smoke").steps and "facts" in p.steps
    docs_only = gate.plan(["docs/working/gate.md"], "smoke")
    assert docs_only.steps == ["products", "docs", "changed-tests"], "a docs-only PR still has its generated pages checked, and builds nothing"


def test_the_changed_tests_step_runs_the_test_beside_a_changed_tool_and_the_root_binary_it_changed():
    cmd = gate.changed_tests(["tools/gate/gate.py", "tests/turbo.rs", "docs/x.md"])
    assert "tools/gate/test_gate.py" in cmd and "--test turbo" in cmd
    assert gate.changed_tests(["docs/x.md"]) == "true"


def test_smoke_refuses_a_base_that_is_not_main_a_conflict_a_mention_and_a_dirty_tree(tmp_path):
    git = _repo(tmp_path)
    assert gate.refusals(tmp_path) == []
    assert any("stacked" in r for r in gate.refusals(tmp_path, pr={"baseRefName": "alim/other", "title": "t", "body": ""}))
    assert any("names the assistant" in r for r in gate.refusals(tmp_path, pr={"baseRefName": "main", "title": "t", "body": "Generated with " + NAME + " Code"}))
    (tmp_path / "f").write_text("dirty\n")
    assert any("differs" in r for r in gate.refusals(tmp_path))
    git("checkout", "-q", "f")
    git("checkout", "-q", "-b", "pr")
    (tmp_path / "g").write_text("a line by " + MAKER + "\n")
    git("add", "g")
    git("commit", "-q", "-m", "feat: g")
    assert any("diff adds" in r for r in gate.refusals(tmp_path))
    git("commit", "-q", "--amend", "-m", "feat: g\n\nCo-Authored-By: " + NAME + " <x@y>")
    assert any("commit message" in r for r in gate.refusals(tmp_path))


def test_smoke_refuses_a_branch_that_conflicts_with_main(tmp_path):
    git = _repo(tmp_path)
    git("checkout", "-q", "-b", "pr")
    (tmp_path / "f").write_text("pr\n")
    git("commit", "-qam", "pr edit")
    git("checkout", "-q", "main")
    (tmp_path / "f").write_text("main\n")
    git("commit", "-qam", "main edit")
    git("update-ref", "refs/remotes/origin/main", "HEAD")
    git("checkout", "-q", "pr")
    assert any("conflicts" in r for r in gate.refusals(tmp_path))


def test_a_passed_smoke_run_leaves_a_mark_for_its_head_that_the_train_reads(tmp_path, monkeypatch):
    monkeypatch.setenv("LLRM_SMOKE_DIR", str(tmp_path / "smoke"))
    assert not gate.smoked("abc123")
    gate.record_smoke("abc123")
    assert gate.smoked("abc123") and not gate.smoked("abc124")


def test_the_bench_step_needs_zero_problems_and_has_no_batch_mode():
    """The train's batch mode (a better count passes, the train blesses it) is gone with the committed counts: a better count always passes."""
    command = gate.commands(gate.plan(["tools/gate/gate.py"], "full"), gate.load(), gate.packages())["bench"]
    assert "--batch" not in command and "BETTER" not in command and '" 0 problems"' in command


def test_a_step_that_gets_little_cpu_on_a_loaded_host_is_not_stalled(tmp_path, monkeypatch):
    """#1750's fast gate failed `spin` with STALL (79): under load a step computing for 4 s got under 0.1 s of CPU per poll, which
    read as none. Any CPU growth is computing; a hung step has none."""
    import os

    ticks = iter(range(10**6))
    monkeypatch.setattr(gate.proctree, "tree_cpu", lambda pid: next(ticks) * 0.02)
    assert gate.run_step("slow", "sleep 2.5", tmp_path, {**os.environ}, stall=1)[1] == 0
    monkeypatch.setattr(gate.proctree, "tree_cpu", lambda pid: 0.0)
    assert gate.run_step("hung", "sleep 30", tmp_path, {**os.environ}, stall=1)[1] == gate.STALL_CODE


def test_a_step_log_is_a_new_file_so_an_orphan_of_a_killed_gate_cannot_write_into_it(tmp_path):
    """#1758 failed `integration INCOMPLETE: 95 results of 83 test binaries`: a cargo left running by a killed gate wrote into the
    truncated log beside the new run's output (NUL gaps, a second run's results)."""
    import os
    import threading

    orphan = open(tmp_path / "step.log", "w")
    threading.Timer(0.3, lambda: (orphan.write("ORPHAN test result: ok. 1 passed\n"), orphan.flush())).start()
    gate.run_step("step", "echo mine; sleep 0.8; echo done", tmp_path, {**os.environ})
    assert (tmp_path / "step.log").read_text() == "mine\ndone\n"


def test_a_gate_killed_by_sigterm_kills_its_running_steps(tmp_path):
    """A gate killed by SIGTERM left its step's cargo running, which then wrote to the log of the next run."""
    import contextlib
    import os
    import signal
    import subprocess as sp
    import sys as _sys
    import time as _time

    def ps():
        return sp.run(["ps", "-eo", "args"], capture_output=True, text=True).stdout

    sleeper = f"sleep 62.{os.getpid()}"
    here = str(Path(gate.__file__).parent)
    ready = tmp_path / "ready"
    sleeper_pid = tmp_path / "sleeper.pid"
    command = f"{sleeper} & echo $! > {sleeper_pid}; touch {ready}; wait"
    code = f"import sys, os, pathlib; sys.path.insert(0, {here!r}); import gate; gate.kill_on_term(); gate.run_step('s', {command!r}, pathlib.Path({str(tmp_path)!r}), dict(os.environ), stall=600)"
    gate_proc = sp.Popen([_sys.executable, "-c", code])
    try:
        # The step touches `ready` once it runs, so the handler is in place. Not `ps`: the gate's own argv names the sleep too (SIGTERM
        # then came before the handler: -15, twice in #1763), and ps cuts a long argv to COLUMNS where that is set.
        for _ in range(300):
            if ready.exists():
                break
            _time.sleep(0.1)
        else:
            raise AssertionError("the step never started")
        gate_proc.send_signal(signal.SIGTERM)
        assert gate_proc.wait(timeout=20) == 128 + signal.SIGTERM
        _time.sleep(0.3)
        assert sleeper not in ps().splitlines(), "the step outlived the gate"
    finally:
        # Only what this test started: the gate it ran, and the sleeper whose pid the step wrote down (no kill by name or pattern).
        gate_proc.kill()
        if sleeper_pid.exists():
            with contextlib.suppress(ProcessLookupError, ValueError):
                os.kill(int(sleeper_pid.read_text()), signal.SIGKILL)


def test_a_gate_without_a_user_bus_refuses_with_the_hint_instead_of_exiting_on_a_dbus_line():
    """Under `env -i` the gate re-executed itself into systemd-run, which failed with one dbus line and no verdict (bare-environment repro, 2026-10-29)."""
    assert "GATE_SCOPED=1" in gate.scope_refusal({}, "/usr/bin/systemd-run")
    assert gate.scope_refusal({"XDG_RUNTIME_DIR": "/run/user/1"}, "/usr/bin/systemd-run") is None
    assert gate.scope_refusal({"GATE_SCOPED": "1"}, "/usr/bin/systemd-run") is None
    assert gate.scope_refusal({}, "") is None


def test_the_gate_scope_is_omitted_from_systemd_oomd(tmp_path):
    """systemd-oomd SIGKILLed two fast gates at 13:56 on memory pressure (58%, then 78%, over 50% for 20 s) with 174 G free: a gate in its
    own scope was a victim. The scope carries user.oomd_omit, which oomd honours."""
    import shutil
    import subprocess as sp
    import sys as _sys

    if not shutil.which("systemd-run"):
        pytest.skip("no systemd-run")
    probe = "import os; cg = open('/proc/self/cgroup').read().split('::')[1].strip(); print(os.getxattr('/sys/fs/cgroup' + cg, 'user.oomd_omit').decode())"
    done = sp.run(gate.scoped([_sys.executable, "-c", probe], {}), capture_output=True, text=True)
    if done.returncode and "Failed to connect" in done.stderr:
        pytest.skip("no systemd user manager")
    assert done.stdout.strip() == "1", (done.stdout, done.stderr)


def test_smoke_fails_a_pr_that_tracks_build_products_and_does_not_build_for_it(tmp_path):
    """#1777 tracked d1.o, d4.o and d8.o and was smoke-green: only the train's integration step ran tests/tracked_build_products.rs."""
    import re
    import subprocess as sp

    rust = (gate.ROOT / "tests/tracked_build_products.rs").read_text()
    assert tuple(re.search(r"PRODUCTS: &\[&str\] = &\[(.*?)\]", rust).group(1).replace('"', "").replace(" ", "").split(",")) == gate.PRODUCTS
    assert tuple(re.search(r"FIXTURES: &\[&str\] = &\[(.*?)\]", rust).group(1).replace('"', "").replace(" ", "").split(",")) == gate.FIXTURES
    assert gate.strays(["d1.o", "src/x.OBJ", "tests/inputs/a.o", "a.rs", "Makefile", "x.map"]) == ["d1.o", "src/x.OBJ", "x.map"]
    for files in (["docs/x.md"], ["tools/gate/gate.py"], ["crates/codegen/llrm-codegen/src/lib.rs"]):
        assert "products" in gate.plan(files, "smoke").steps, files
    git = _repo(tmp_path)
    assert gate.products_main(tmp_path) == 0
    (tmp_path / "d1.o").write_text("x")
    git("add", "d1.o")
    assert gate.products_main(tmp_path) == 1
    assert gate.commands(gate.plan(["docs/x.md"], "smoke"), gate.load(), gate.packages())["products"].startswith("python3 -c")


def test_smoke_runs_the_pytest_files_that_are_not_slow_and_the_full_step_does_not_run_pytest_programs_files_twice():
    """#1765 and #1766 were smoke-green and failed tools/test_identical.py (1.7 s) and tests/test_readme.py (0.3 s) in the train. And the
    pytest step passed `tests/*.py` as explicit files, which run though `--ignore` names them: test_loops.py and test_programs_compile.py
    (pytest-programs' own) ran twice."""
    cfg, pkgs = gate.load(), gate.packages()
    smoke = gate.plan(["crates/codegen/llrm-codegen/src/lib.rs"], "smoke")
    assert "pytest-quick" in smoke.steps and "pytest-quick" not in gate.plan(["docs/x.md"], "smoke").steps
    quick = gate.commands(smoke, cfg, pkgs)["pytest-quick"]
    for slow in cfg["smoke"]["slow_pytest"]:
        assert (gate.ROOT / slow).exists(), slow
        assert f"--ignore={slow}" in quick or slow in ("tests/test_loops.py",), slow
    assert "tests/test_readme.py" in quick.split() and "--ignore=tools/test_identical.py" not in quick and " tools " in quick
    assert "pytest-quick-durations.json" in quick and "pytest-durations.json" not in quick, "its own durations file, not the full step's"
    full = gate.commands(gate.plan(["crates/codegen/llrm-codegen/src/lib.rs"], "full"), cfg, pkgs)
    for step in ("pytest", "pytest-quick"):
        named = (full if step == "pytest" else {step: quick})[step].split()
        assert "tests/test_loops.py" not in named and "tests/test_programs_compile.py" not in named and "tests/*.py" not in named, step
    assert "tests/test_loops.py" in full["pytest-programs"] and "tests/test_programs_compile.py" in full["pytest-programs"]


def test_the_bench_step_prints_every_problem_row_not_the_last_three_lines(tmp_path):
    """Train 3's bench step said `424 measurements: 22 problems` under only the last lines: `tail -3` cut the 20 FAIL rows."""
    import os
    import subprocess as sp

    (tmp_path / "uv").write_text(
        "#!/bin/sh\n"
        + "".join(f"echo 'FAIL row{n}: instructions 10 > 9 (worse)'\n" for n in range(20))
        + "echo '424 measurements: 20 problems'\n"
    )
    (tmp_path / "uv").chmod(0o755)
    command = gate.commands(gate.plan(["bench/fib/fib.c"], "full"), gate.load(), gate.packages())["bench"]
    done = sp.run(["bash", "-c", command], capture_output=True, text=True, env={**os.environ, "PATH": f"{tmp_path}:{os.environ['PATH']}"})
    assert done.returncode != 0
    assert all(f"FAIL row{n}:" in done.stdout for n in range(20)) and "20 problems" in done.stdout, done.stdout


def test_smoke_runs_the_source_scans_that_are_root_test_binaries_and_the_other_tiers_do_not_twice():
    """#1792 put a std HashMap in settled.rs: smoke-green, red in the train's integration step (tests/seeded_maps.rs), a source scan only
    that step ran."""
    cfg, pkgs = gate.load(), gate.packages()
    binaries = [Path(one["file"]).stem for one in cfg["scan"] if one.get("step") == "integration" and Path(one["file"]).stem != "target_facts"]
    assert "seeded_maps" in binaries and "no_local_paths" in binaries
    smoke = gate.commands(gate.plan(["crates/codegen/llrm-codegen/src/lib.rs"], "smoke"), cfg, pkgs)["scans"]
    assert all(f"--test {name} " in smoke for name in binaries) and "--test target_facts" not in smoke, smoke
    for tier in ("fast", "full"):
        assert "--test seeded_maps" not in gate.commands(gate.plan(["crates/codegen/llrm-codegen/src/lib.rs"], tier), cfg, pkgs)["scans"], tier


def test_the_facts_step_fails_on_a_failing_target_facts_and_on_a_run_that_ran_no_test(tmp_path):
    """The step printed `facts 1 0` (1 s wall, 0 cpu) and was taken on trust: it runs tests/target_facts.rs, and a failure or a run that
    reported nothing fails it."""
    import os

    cfg, pkgs = gate.load(), gate.packages()
    plan = gate.plan(["tools/bench/bench.py"], "smoke")
    command, check = gate.commands(plan, cfg, pkgs)["facts"], gate.expected(plan, cfg, pkgs)["facts"]
    assert "--test target_facts" in command and check == (1, False)
    stubs = tmp_path / "bin"
    stubs.mkdir()
    for outcome, body in {
        "ok": "echo 'running 11 tests'; echo 'test result: ok. 11 passed; 0 failed'; exit 0",
        "failed": "echo 'test result: FAILED. 10 passed; 1 failed'; exit 101",
        "silent": "exit 0",
    }.items():
        (stubs / "cargo").write_text(f"#!/bin/sh\n{body}\n")
        (stubs / "cargo").chmod(0o755)
        logs = tmp_path / outcome
        logs.mkdir()
        _, code, _, _ = gate.run_step("facts", command, logs, {**os.environ, "PATH": f"{stubs}:{os.environ['PATH']}"}, check)
        assert code == {"ok": 0, "failed": 101, "silent": 78}[outcome], (outcome, code)


def test_a_comma_list_allows_only_those_capabilities_to_be_missing(tmp_path):
    """CI allowed any missing tool ("1") and passed Turbo, CodeView, QB 4.5 and QCport by skipping them; now it allows `perf` and nothing else."""
    env = {"HOME": str(tmp_path), "TCPP30_DIR": str(tmp_path / "none"), "GATE_ALLOW_MISSING": "perf"}
    assert "turbo" not in gate.missing_capabilities(env)
    allowed = gate.missing_capabilities({**env, "GATE_ALLOW_MISSING": "perf,turbo"}).keys()  # perf too where this host has no counters
    assert "turbo" in allowed and allowed <= {"turbo", "perf"}


def test_the_hosted_workflow_allows_only_perf_to_be_missing():
    allowed = re.findall(r"^\s+GATE_ALLOW_MISSING: *(\S+)", (ROOT / ".github/workflows/ci.yml").read_text(), re.M)
    assert allowed == ["perf"]


def test_the_groups_that_need_private_tools_are_named_in_the_plan():
    """A job reads the private checkout only for these; a fork's pull request fails them by name."""
    p = gate.plan(["crates/driver/llrm-cc/src/lib.rs"])
    private = gate.private_groups(p)
    assert {"reference", "run", "bench-1", "python"} <= set(private) and "tools" not in private, private
    assert "unit" not in private


def test_every_tool_directory_a_private_capability_needs_is_set_for_the_jobs_that_run_it():
    """A variable the checkout does not set falls back to a path on the developer's host: the job fails by "not found", or skips in a test."""
    cfg = gate.load()
    for group, steps in cfg["groups"].items():
        env = gate.private_env("/p", group)
        for step in steps:
            for name in cfg["requires"].get(step, []):
                for need in cfg["capability"][name]["needs"] if cfg["capability"][name].get("private") else []:
                    assert need["env"] in env, (group, step, name, need["env"])


def test_a_group_may_point_a_variable_somewhere_else():
    """test_identical read the qb-quake variant as QCport (10 modules of 65) when QCPORT was its src: dfaces has a variable of its own."""
    assert gate.private_env("/p", "reference")["QCPORT"] == "/p/rm-quake/src"
    assert gate.private_env("/p", "python")["QCPORT"] == "/p/rm-quake/src"
    assert gate.private_env("/p", "python")["DFACES_SRC"] == "/p/qbq-llrm/src"
    assert "QCPORT" not in gate.private_env("/p", "unit")
def test_what_ci_runs_for_a_diff_is_what_gate_run_plans_for_it():
    """CI's jobs come from `gate.py plan --json` (ci.yml lists no step): the steps of its groups, with the build, are the steps `gate.py run`
    plans for the same diff, so a green CI is the local pass. Checked for diffs of each kind, and that the workflow asks the planner."""
    import json
    import subprocess as sp

    for files in (["docs/x.md"], ["tools/gate/gate.py"], ["bench/fib/fib.c"], ["crates/codegen/llrm-codegen/src/lib.rs"], ["tests/turbo.rs"]):
        local = gate.plan(files)
        out = sp.run([sys.executable, str(Path(gate.__file__)), "plan", "--json", "--files", *files], capture_output=True, text=True, cwd=gate.ROOT)
        ci = json.loads(out.stdout)
        in_groups = sorted({step for steps in ci["groups"].values() for step in steps})  # the bench shards all run the one step
        assert ci["tier"] == local.tier, files
        assert sorted(set(in_groups) | set(ci["skipped"])) == sorted(s for s in local.steps if s != "build"), (files, ci)
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "gate.py plan --json" in workflow and "gate.py run --group ${{ matrix.group }}" in workflow, "the jobs come from the planner"
    assert all(f"--tier {t}" not in workflow for t in ("fast", "full", "smoke")), "the workflow does not choose a tier of its own"


def test_the_checks_that_build_nothing_run_alone_before_the_build():
    """#1873's gate (unit) failed on formatting after a long run: a mistake the tree alone shows must fail before any build or gate starts."""
    cfg = gate.load()
    assert set(cfg["groups"]["static"]) <= gate.NO_BINARIES | {"products"} and "fmt" in cfg["groups"]["static"]
    assert not {"fmt", "docs", "products"} & {s for g, steps in cfg["groups"].items() if g != "static" for s in steps}
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "  build:\n    needs: [plan, static]" in workflow and 'if g != "static"' in workflow, "the matrix never runs the static group"
    assert re.search(r"needs: \[plan, build[^\]]*\]\n    # A skipped ancestor", workflow) and "needs.build.result == 'success'" in workflow, "a skipped static job must not skip the gate"


def test_a_draft_runs_plan_and_static_only_and_is_never_reported_as_a_pass():
    """Forty drafts running the full gate would swamp the runners: ready_for_review starts it, the merge loop reads only `PASS at`."""
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "types: [labeled]" in workflow
    assert "needs.plan.outputs.draft != 'true' && needs.plan.outputs.any" in workflow
    assert "DRAFT at ${head:0:12}" in workflow


def test_the_ci_summary_prints_the_gate_line_with_the_head_it_is_of():
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "GATE ${{ needs.plan.outputs.tier }} PASS at ${head:0:12}" in workflow and "github.event.pull_request.head.sha" in workflow


def test_a_plan_that_builds_nothing_on_a_fresh_target_says_so_instead_of_a_traceback(tmp_path, monkeypatch, capsys):
    """#1832 (tools only: the plan has no build) on a fresh target dir died in snapshot_binaries with FileNotFoundError on target/release."""
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path / "fresh"))
    monkeypatch.delenv("LLRM_BIN", raising=False)
    plan = gate.Plan("fast", "tools only", ["pytest-quick", "docs"], None, [], ["tools/gate/train.py"])
    code, failed = gate.execute(plan)
    out = capsys.readouterr().out
    assert code == 2 and failed == ["build"] and "does not exist and this plan builds nothing" in out and "pytest-quick" in out, out
    # a plan whose steps read the tree only needs no binaries
    monkeypatch.setattr(gate, "commands", lambda *a, **k: {"docs": "true"})
    monkeypatch.setattr(gate, "expected", lambda *a, **k: {})
    code, _ = gate.execute(gate.Plan("fast", "docs", ["docs"], None, [], ["docs/x.md"]))
    assert code == 0


def test_the_measure_step_needs_a_counter_and_valgrind_is_one_where_perf_reads_nothing(tmp_path):
    """CI had no counters (perf prints <not supported> on a hosted VM) and `measure` was left out of every run: callgrind counts there."""
    import shutil

    tools = tmp_path / "bin"
    tools.mkdir()
    for name in ("sh", "grep"):
        (tools / name).symlink_to(shutil.which(name))
    (tools / "perf").write_text("#!/bin/sh\necho '<not supported>,,instructions:u,0,100.00,,'\n")
    (tools / "perf").chmod(0o755)
    env = {"HOME": str(tmp_path), "GATE_ALLOW_MISSING": "1", "PATH": str(tools), "QB45_DIR": ""}
    assert {"perf", "counter"} <= gate.missing_capabilities(env).keys()
    (tools / "valgrind").write_text("#!/bin/sh\n")
    (tools / "valgrind").chmod(0o755)
    got = gate.missing_capabilities(env)
    assert "perf" in got and "counter" not in got
    assert "counter" in gate.load()["capability"]
def test_the_qcport_step_does_not_skip_itself_where_the_variables_are_set_and_fails_where_they_are_not(tmp_path):
    """CI's run of the reference group said `qcport SKIPPED` and passed: the step looked for ~/scratch/qcport-env.sh, which only the
    development host has, and exited 77 without running QCport."""
    command = gate.commands(gate.plan(["crates/driver/llrm-cc/src/lib.rs"]), gate.load(), gate.packages())["qcport"]
    command = command.replace("uv run -q --project tools python tools/qcport-run-cc.py", 'echo ran "$JWLINK"')
    base = {"HOME": str(tmp_path), "PATH": "/usr/bin:/bin", "LLRM_BIN": "/t/release"}
    both = subprocess.run(["bash", "-c", command], capture_output=True, text=True, env={**base, "QCPORT": "a", "QCPORT_INC": "b", "QCPORT_BORLAND": "c"})
    assert (both.returncode, both.stdout.strip()) == (0, "ran /t/release/jwlink"), both  # the linker the build made: a runner has none on PATH
    none = subprocess.run(["bash", "-c", command], capture_output=True, text=True, env=base)
    assert none.returncode not in (0, 77) and "QCPORT is not set" in none.stderr, none


def test_a_step_that_skips_itself_fails_where_only_named_capabilities_may_be_missing(tmp_path, monkeypatch):
    """CI passed with `qcport SKIPPED` (the step found no env file): a skip the plan did not make is a step that did not run."""
    env = {**__import__("os").environ}
    monkeypatch.delenv("GATE_ALLOW_MISSING", raising=False)
    assert gate.run_step("skipped", "exit 77", tmp_path, env)[1] == 77
    monkeypatch.setenv("GATE_ALLOW_MISSING", "perf")
    assert gate.run_step("skipped", "exit 77", tmp_path, env)[1] == 1


def test_the_bench_step_runs_as_shards_and_a_merge_judges_the_whole_and_records_the_base_on_a_push():
    """The bench step was 2400 s of a 4-thread slot on a base-cache miss (#1866 timed out at 3600 s): six shards measure a slice each."""
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "actions/cache/restore@v4" in workflow and "bench-base-$(python3 tools/bench/basecounts.py key HEAD^1" in workflow
    merge = workflow[workflow.index("  bench-merge:") : workflow.index("\n  ci:")]
    assert "bench.py --merge" in merge and "'--record'" in merge and "key: ${{ steps.basecounts.outputs.head }}" in merge
    assert ci_needs(workflow) == ["gate", "bench-merge", "setup-check"], "ci waits for the gate, the merge and the set-up check, after any jobs before them"
    assert "if: always() && github.event_name == 'push' && steps.merge.outputs.recorded == 'true'" in merge, "saved even when the verdict is a failure"


def test_the_bench_step_is_cut_into_the_configured_shards_each_with_its_own_slice():
    p = gate.plan(["bench/fib/fib.c"], "full")
    groups = gate.groups_of(p, {})
    n = gate.load()["bench"]["shards"]
    assert [g for g in groups if g.startswith("bench")] == [f"bench-base-{i}" for i in range(1, n + 1)] + [f"bench-{i}" for i in range(1, n + 1)] and groups["tools"] == ["torture"]
    assert gate.shard_of("bench-3") == ("bench", f"3/{n}") and gate.shard_of("tools") == ("tools", None)
    assert gate.shard_of("bench-base-2") == ("bench", f"base:2/{n}"), "the base is measured in slices, one job each (the one leg outlasted the 90-minute job timeout in gate run 7)"
    assert "--shard" in gate.commands(p, gate.load(), gate.packages())["bench"]


def test_the_measure_step_is_a_private_group_so_it_gets_qcports_modules():
    """Without QCPORT the cost rows of QCport's 65 modules are left out and measure judges the programs alone, as green."""
    p = gate.plan(["crates/codegen/llrm-codegen/src/backend/peephole.rs"])
    assert "measure" in p.steps and "cost" in gate.private_groups(p)
    assert gate.private_env("/p", "cost")["QCPORT"] == "/p/rm-quake/src"


def test_the_scaling_tests_run_in_their_own_step_in_their_own_group():
    """Only they need hardware counters, so only their job needs tinytim: the python group, with them in it, ran on tinytim for 12 minutes."""
    cfg = gate.load()
    assert cfg["groups"]["perf"] == ["pytest-perf", "oracle-premise"] and cfg["groups"]["oracle"] == ["oracle-sweep"] and "pytest-perf" not in cfg["groups"]["python"]
    assert gate.perf_files() == sorted(f for f, needs in gate.declared_requirements().items() if "perf" in needs) and len(gate.perf_files()) == 4
    assert cfg["requires"]["pytest-perf"] == ["perf"], "skipped whole, and allowed to be, where there are no counters"
    assert cfg["requires"]["oracle-premise"] == ["perf"], "which oracle ran at all is measured with perf; the sharded compiles need none"


def test_the_plan_keeps_the_perf_steps_the_planners_host_cannot_run_when_told_the_jobs_can():
    """The hosted planner has no counters: pytest-perf was skipped in the plan, so no `perf` group was ever made and the scaling tests ran nowhere."""
    missing = {"perf": "no counters here"}
    p = gate.plan(["tools/linkrecipe.py"])
    assert "pytest-perf" in gate.skipped_steps(p.steps, missing, p.languages)
    assert "pytest-perf" not in gate.skipped_steps(p.steps, gate.assumed(missing, {"GATE_PLAN_ASSUME": "perf"}), p.languages)
    assert "GATE_PLAN_ASSUME=perf" in (gate.ROOT / ".github/workflows/ci.yml").read_text()


def test_a_pull_request_that_changes_the_compiler_is_not_a_pass_when_the_perf_tests_ran_hosted():
    """#1890 let the scaling tests skip on a hosted fallback and print PASS; they just caught two real superlinears, so a compiler change waits for tinytim."""
    assert gate.touches_compiler(["crates/codegen/llrm-codegen/src/lib.rs"]) and not gate.touches_compiler(["tools/x.py", ".github/workflows/ci.yml", "docs/x.md"])
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert 'PERF SKIPPED at ${head:0:12}' in workflow and 'needs.plan.outputs.compiler }}" = true' in workflow
    sys.path.insert(0, str(gate.ROOT / "tools" / "ci"))
    import pick_runners
    assert pick_runners.pick(["perf", "unit"], "", False, 0)["perf_hosted"] == ["perf"], "a fork's pull request: no tinytim"
    assert pick_runners.pick(["perf", "unit"], "", True, 0)["perf_hosted"] == [], "queued on tinytim instead"


def test_the_ci_line_names_the_parent_and_a_stale_workflow_is_never_a_pass():
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "PASS at ${head:0:12}$on" in workflow and "STALE WORKFLOW (not a pass) at ${head:0:12}$on" in workflow
    assert "python3 tools/ci/stale_workflow.py" in workflow and 'BASE_REF: ${{ github.event.pull_request.base.ref }}' in workflow


def test_a_group_of_tree_only_checks_has_no_build_and_a_group_that_runs_binaries_does():
    """#1882's static job built the workspace from cold (701 s) before the build job even started: it has no target dir to reuse."""
    p = gate.plan(["crates/codegen/llrm-codegen/src/lib.rs"], "full")
    assert "build" not in gate.group_plan(p, "static").steps
    assert "build" in gate.group_plan(p, "unit").steps and "build" in gate.group_plan(p, "python").steps and gate.group_plan(p, "build").steps == ["build"]
    assert gate.TREE_ONLY == set(gate.load()["groups"]["static"])


def test_the_bench_shards_and_regroupings_keep_the_job_count_down():
    """The account runs 20 hosted jobs at once; each job costs 70-140 s of set-up. A group under ~2 minutes of work is not its own job."""
    cfg = gate.load()
    assert cfg["bench"]["shards"] == 3 and "identity" in cfg["groups"]["unit"] and cfg["groups"]["reference"] == ["turbo", "cv4", "qcport", "oracle-qcport"]


def test_the_draft_verdict_comes_from_the_api_at_plan_time_and_is_checked_again_at_the_end():
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "python3 tools/ci/pr_state.py" in workflow and "needs.plan.outputs.draft != 'true' && needs.plan.outputs.any" in workflow
    assert "DRAFT RUN OF A READY PR (not a pass)" in workflow
    assert "runs-on: ${{ fromJSON(needs.plan.outputs.runner_small || '\"ubuntu-24.04\"') }}" in workflow


def test_a_change_to_the_setup_action_is_run_on_tinytim_before_it_merges():
    """#1987 fixed the set-up there and its own diff planned no tinytim job: the live proof is a job for any change to the action."""
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "setup-check:" in workflow and "runs-on: [self-hosted, tinytim]" in workflow.split("setup-check:")[1].split("static:")[0]
    assert "\\.github/actions/setup/" in workflow and ci_needs(workflow) == ["gate", "bench-merge", "setup-check"]


def test_a_push_to_main_makes_no_perf_or_cost_job():
    """Every merge queued a perf and a cost job on the six tinytim runners: 16 of the 20 jobs waiting, and the pull requests' behind them."""
    workflow = (gate.ROOT / ".github/workflows/ci.yml").read_text()
    assert "COMPILER: ${{ github.event_name == 'pull_request' && steps.plan.outputs.compiler || 'false' }}" in workflow


def test_the_oracle_sweep_is_cut_into_slices_that_run_on_their_own_runners_and_the_premise_stays_on_perf():
    """No counters needed to compile 1,350 times: hosted, sharded; only 'did each oracle run' reads perf, once, on tinytim."""
    p = gate.plan(["crates/codegen/llrm-codegen/src/backend/peephole.rs"], "full")
    groups = gate.groups_of(p, {})
    n = gate.load()["oracle"]["shards"]
    assert [g for g in groups if g.startswith("oracle")] == [f"oracle-{i}" for i in range(1, n + 1)] and groups["perf"][-1] == "oracle-premise"
    assert gate.shard_of("oracle-2") == ("oracle", f"2/{n}") and gate.shard_of("bench-1") == ("bench", f"1/{gate.load()['bench']['shards']}")
    assert "--shard" in gate.commands(p, gate.load(), gate.packages())["oracle-sweep"] and "--no-premise" in gate.commands(p, gate.load(), gate.packages())["oracle-sweep"]


def test_a_change_to_the_oracle_sweep_runs_the_sweep_and_its_premise():
    """A tools-only PR to oracle_sweep.py planned neither step: its own sharded groups never ran in its CI."""
    p = gate.plan(["tools/oracle_sweep.py"])
    assert "oracle-sweep" in p.steps and "oracle-premise" in p.steps
    assert "oracle-sweep" not in gate.plan(["tools/bench/mkci.py"]).steps


def test_the_gate_builds_as_ci_does_not_incrementally():
    """A local gate run built release incrementally (the profile's default) and test_budgets counted lir pressuresink 10.3 -> 33.3 Minstr (1.72% of the
    compile, under the share) where CI's build reads 11.3 -> 37.6 (2.03%): a debt read 'paid' on the one machine and not the other."""
    assert gate.STEP_ENV["CARGO_INCREMENTAL"] == "0"
    assert all(k in gate.STEP_ENV for k in ("RUSTFLAGS", "RUSTC_BOOTSTRAP"))


def test_a_tracked_symlink_out_of_the_tree_or_a_build_dir_is_named(tmp_path, capsys):
    """fold-poc's #2171 was red for hours: `git add -A` committed a `target` symlink and the gate said only 'tree differs from HEAD in tracked files'."""
    git = _repo(tmp_path)
    (tmp_path / "docs").mkdir()
    (tmp_path / "docs/a.md").write_text("x")
    (tmp_path / "docs/inside").symlink_to("a.md")
    git("add", "-A")
    assert gate.products_main(tmp_path) == 0, "a relative link inside the tree is fine"
    (tmp_path / "target").symlink_to("/home/someone/cargo-target")
    git("add", "-A")
    assert gate.products_main(tmp_path) == 1
    assert "tracked symlink target -> /home/someone/cargo-target" in capsys.readouterr().out
    git("rm", "-q", "--cached", "target")
    (tmp_path / "target").unlink()
    (tmp_path / "target").mkdir()
    (tmp_path / "target/x").write_text("y")
    git("add", "-A")
    assert gate.products_main(tmp_path) == 1
    assert "tracked build dir target/x" in capsys.readouterr().out


def test_a_change_to_llrm_cc_runs_qcport_measure_and_the_c_programs():
    """Only crates/frontends/llrm-c/** owned the C steps: a llrm-cc change gated without qcport, measure and the C programs."""
    plan = gate.plan(["crates/driver/llrm-cc/src/lib.rs"])
    for step in ("qcport", "oracle-qcport", "measure"):
        assert step in plan.steps, (step, plan.steps)
    assert "c" in plan.languages, plan.languages


def test_the_integration_step_waits_for_the_clang_trees_the_pins_name(tmp_path):
    """The llrm-cc tests need the pinned and the m16 clang. They were looked up in ~/scratch, so a host without them failed every test at once
    ('the target needs LLRM_OW'); now a missing tree is a missing capability: skipped with a reason locally, red in CI."""
    env = {"HOME": str(tmp_path), "XDG_CACHE_HOME": str(tmp_path / "cache"), "GATE_ALLOW_MISSING": "1", "PATH": "/usr/bin:/bin"}
    missing = gate.missing_capabilities(env)
    assert "clang" in missing and "integration" in gate.skipped_steps(["integration"], missing)
    for kind in ("pin", "m16"):
        tree = subprocess.run(["sh", str(ROOT / "toolchain/clangbuild.sh"), "--print", kind], capture_output=True, text=True, env=env).stdout.strip()
        (Path(tree) / "bin").mkdir(parents=True)
        (Path(tree) / "bin/clang").write_text("#!/bin/sh\n")
        (Path(tree) / "bin/clang").chmod(0o755)
    assert "clang" not in gate.missing_capabilities(env)


def test_a_github_actions_runner_without_a_user_bus_runs_the_gate_unscoped():
    """Run 4's gate (cost) and gate (perf) on the self-hosted podman runners died in 0 s: "no systemd user bus ... set GATE_SCOPED=1". A runner is no dev box, so it neither scopes nor refuses."""
    ci = {"GITHUB_ACTIONS": "true"}
    assert gate.scoped(["x"], ci, "/usr/bin/systemd-run") is None
    assert gate.scope_refusal(ci, "/usr/bin/systemd-run") is None
    assert gate.scope_refusal({}, "/usr/bin/systemd-run") is not None, "a dev box without a bus still refuses"


def test_a_public_repo_the_gate_clones_is_pinned_to_a_sha():
    """rm-quake was cloned at its HEAD: its 047e7a8 (lit faces with no lightmap) turned oracle-qcport red with no change in this repository. A push there must not move the gate."""
    public = gate.load()["private"]["public"]
    assert public and all(re.fullmatch(r"[\w.-]+/[\w.-]+@[0-9a-f]{40}", spec) for spec in public.values()), public


def test_a_streamed_steps_lines_reach_the_job_log_while_it_runs_and_a_cut_step_keeps_them(tmp_path, capsys):
    """Gate run 7: bench-base was cancelled at 90 minutes and the job log held none of its stage lines (a step's log was printed at its end)."""
    tail = gate.LogTail(tmp_path / "bench.log", "bench")
    tail.pump()
    (tmp_path / "bench.log").write_text("bench: [0s] base counts\nbench: [12s] half a li")
    tail.pump()
    assert capsys.readouterr().out == "[bench] bench: [0s] base counts\n", "a line is printed once it is whole"
    (tmp_path / "bench.log").write_text("bench: [0s] base counts\nbench: [12s] half a line\n")
    tail.pump()
    assert capsys.readouterr().out == "[bench] bench: [12s] half a line\n"
    gate.run_step("bench", "echo 'bench: [1s] stage' >&2", tmp_path, os.environ.copy())
    assert "[bench] bench: [1s] stage" in capsys.readouterr().out
    gate.run_step("pytest", "echo 'a test line'", tmp_path, os.environ.copy())
    assert "[pytest]" not in capsys.readouterr().out, "only the steps that print little and run long are streamed"


def test_a_streamed_step_cut_mid_run_leaves_its_stage_lines_in_the_output(tmp_path, capsys):
    """Gate run 7: the job timeout cut bench-base after 73 minutes of work and the job log held no stage line, so nobody could say where it was."""
    done = gate.run_step("bench", "echo 'bench: [3s] base counts, slice 1 of 3: 16 programs, 4 usable of 4 cores' >&2; sleep 60", tmp_path, os.environ.copy(), stall=2)
    assert done[1] == gate.STALL_CODE
    assert "[bench] bench: [3s] base counts, slice 1 of 3: 16 programs, 4 usable of 4 cores" in capsys.readouterr().out


def test_a_group_handed_the_build_jobs_target_runs_no_build_step_of_its_own(tmp_path, monkeypatch):
    """Gate run 8: every group re-ran cargo check and test --no-run on the restored target, 600-800 s in each of 17 jobs (python: 604 of 1079 s)."""
    p = gate.plan(["bench/fib/fib.c"], "full")
    head = gate.head_state()[0]
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path))
    monkeypatch.delenv("GATE_PREBUILT", raising=False)
    assert "build" in gate.group_plan(p, "python").steps, "no flag: a local run builds"
    monkeypatch.setenv("GATE_PREBUILT", "1")
    assert "build" in gate.group_plan(p, "python").steps, "the flag without the build job's stamp builds"
    (tmp_path / ".built-from").write_text("0" * 40 + "\n")
    assert "build" in gate.group_plan(p, "python").steps, "a target built from another commit builds"
    (tmp_path / ".built-from").write_text(head + "\n")
    assert "build" in gate.group_plan(p, "python").steps, "the stamp without the build job's release dir builds"
    (tmp_path / "release").mkdir()
    assert "build" not in gate.group_plan(p, "python").steps and "build" not in gate.group_plan(p, "unit").steps
    assert gate.group_plan(p, "build").steps == ["build"], "the build job still builds"


def test_ci_marks_the_restored_target_as_the_build_jobs(tmp_path):
    workflow = (Path(gate.ROOT) / ".github/workflows/ci.yml").read_text()
    restore = workflow.split("name: Restore the target dir", 2)[1].split("- id: basecounts", 1)[0]
    assert 'echo "GATE_PREBUILT=1" >> "$GITHUB_ENV"' in restore
