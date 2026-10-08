"""The gate's path map: a change in an area selects that area's tests, and nothing is left out by omission."""

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

def test_every_step_the_planner_can_produce_is_in_exactly_one_ci_group():
    """A step in no group would never run in CI, silently: the matrix is built from the groups."""
    everything = [s for s in gate.plan(["crates/ir/llrm-mir/src/lib.rs"]).steps if s != "build"]  # the build is its own job
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
    assert set(got["skipped"]) == {"turbo", "cv4", "qcport", "bench"} and "reference" not in got["groups"]
    assert "run" in got["groups"]["run"]


def test_a_gate_run_makes_a_target_dir_that_does_not_exist_yet(tmp_path, monkeypatch):
    """On a fresh CI runner CARGO_TARGET_DIR is not there: `run` died FileNotFoundError on target/gate-logs before any step."""
    monkeypatch.setenv("CARGO_TARGET_DIR", str(tmp_path / "fresh" / "target"))
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
    """test_scaling.py failed 'Access to performance monitoring ... is limited' on a runner whose VM has no instruction counters."""
    missing = {"perf": "perf: `perf stat` fails here"}
    assert gate.python_tests_unusable(missing) == ["crates/target/llrm-x86-m32/vsgcc/test_scaling.py"]
    p = gate.plan(["tools/linkrecipe.py"])
    cmds = gate.commands(p, gate.load(), gate.packages(), frozenset(), tuple(gate.python_tests_unusable(missing)))
    assert "--ignore=crates/target/llrm-x86-m32/vsgcc/test_scaling.py" in cmds["pytest"]
    assert "test_scaling" not in gate.commands(p, gate.load(), gate.packages())["pytest"]


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
