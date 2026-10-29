"""The per-merge tests of feat/parity: the selection fits 10 s of CPU however many crates a merge touches, an overrun or a failed step fails it, and the last line says which."""
from __future__ import annotations

import shutil
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_smoke as ps  # noqa: E402
import pytest  # noqa: E402


class Counted:
    """A meter that read `value` instructions (None: perf cannot attach)."""

    value: int | None = None

    def __enter__(self):
        return self

    def __exit__(self, *_):
        return None


@pytest.fixture(autouse=True)
def no_perf(monkeypatch):
    """Tests do not attach perf to pytest: the meter reads nothing and the budget is CPU seconds unless a test hands one."""
    monkeypatch.setattr(ps, "Meter", Counted)


def counted(instructions: int):
    return type("Meter", (Counted,), {"value": instructions})


def clocked(costs):
    """A clock and steps on virtual time: each step advances the clock by its cost and answers `verdict`."""
    now = [0.0]
    steps = [(name, lambda _bins, cost=cost, verdict=verdict, name=name: (now.__setitem__(0, now[0] + cost), (verdict, name))[1], cost) for name, cost, verdict in costs]
    return (lambda: now[0]), steps


def test_the_declared_selection_fits_the_budget_whatever_the_merge_touches():
    """Every merge pays this: a step added without dropping another, or a crate's tests added on top, pushes the total past the 10 s the user set."""
    assert ps.BUDGET <= 15.0 and ps.BUDGET - ps.fixed_cost() - ps.CARGO_OVERHEAD * ps.MAX_CRATES >= ps.UNIT_FLOOR / 2, (ps.fixed_cost(), "the fixed steps must leave the unit tests their floor")
    for crates in ([], ["a"], ["a", "b", "c"], [f"c{i}" for i in range(40)]):
        assert ps.declared(crates) <= ps.BUDGET + 1e-9, (len(crates), ps.declared(crates))


def test_a_set_within_the_budget_ends_in_pass():
    clock, steps = clocked([("a", 2.0, "ok"), ("b", 3.0, "ok")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert ok and line.startswith("PARITY SMOKE PASS") and "5.0 s CPU of 15 s" in line


def test_a_set_over_the_budget_fails_whatever_its_steps_say():
    """A selection that grew to 16 s slows every merge of the branch: it must not read as a pass."""
    clock, steps = clocked([("a", 8.0, "ok"), ("b", 8.0, "ok")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert not ok and line.startswith("PARITY SMOKE FAIL") and "over the budget: 16.0 s > 15 s" in line


def test_a_failed_step_fails_the_set_and_a_skipped_one_is_named_not_passed():
    clock, steps = clocked([("a", 1.0, "fail"), ("b", 1.0, "skipped")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert not ok and "a: a" in line and "skipped b: b" in line
    clock, steps = clocked([("b", 1.0, "skipped")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert ok and "skipped b: b" in line


def test_the_budget_is_cpu_time_not_the_wall_clock_of_a_loaded_host():
    """The same work took 4.2 s and 8.2 s of wall on this host at load 60: a wall budget fails merges for nothing."""
    import inspect

    assert inspect.signature(ps.run_steps).parameters["clock"].default is ps.cpu


def test_the_cheapest_ranked_tests_that_fit_a_crates_share_are_chosen():
    ranking = {"llrm-x": [("a", 0.1), ("b", 0.2), ("c", 0.5), ("d", 0.6)]}
    assert ps.unit_tests("llrm-x", 0.35, ranking) == ["a", "b"]
    assert ps.unit_tests("llrm-x", 0.01, ranking) == []
    assert ps.unit_tests("llrm-none", 5.0, ranking) == [], "no ranking: all its tests, cut at the share"


def test_the_ranking_is_read_from_the_report_time_listing_cheapest_first():
    out = "test a::slow ... ok <0.900s>\ntest a::fast ... ok <0.001s>\ntest a::bad ... FAILED\ntest a::mid ... ok <0.020s>\n"
    assert ps.ranked(out) == [("a::fast", 0.001), ("a::mid", 0.02), ("a::slow", 0.9)]


def test_a_changed_file_names_the_crate_it_belongs_to():
    assert ps.crate_of("crates/opt/llrm-transforms/src/gvn.rs") == ("llrm-transforms", "crates/opt/llrm-transforms")
    assert ps.crate_of("tools/measure.py") is None and ps.crate_of("crates/opt/nowhere/src/x.rs") is None


def test_a_cut_test_run_is_killed_with_its_children_and_charged_its_whole_allowance(tmp_path):
    """The unit step killed cargo alone: its test binary ran on unseen, and the step read 0.0 s for 1,143 tests."""
    import time

    marker = tmp_path / "grandchild.pid"
    script = (
        f"import subprocess,sys,time\nc=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)'])\nopen({str(marker)!r},'w').write(str(c.pid))\ntime.sleep(60)")
    before = ps.cpu()
    code, _ = ps.run_cut([sys.executable, "-c", script], 3.0, tmp_path)
    assert code is None and ps.cpu() - before >= 3.0, "the cut is charged"
    pid = int(marker.read_text())
    def alive() -> bool:
        try:
            return Path(f"/proc/{pid}/stat").read_text().split(")")[-1].split()[0] != "Z"  # a zombie is dead, waiting for init
        except (FileNotFoundError, ProcessLookupError):  # reading /proc/PID/stat of a process that exits mid-read raises ESRCH
            return False

    for _ in range(50):
        if not alive():
            break
        time.sleep(0.02)
    assert not alive(), "the grandchild outlived the cut"
    assert ps.run_cut([sys.executable, "-c", "print('hi')"], 5.0, tmp_path) == (0, "hi\n")


def test_a_merge_touching_many_crates_tests_the_six_with_most_changed_files():
    """40 crates cost 40 cargo starts before any test ran: the line would fail the merge for the size of the diff."""
    assert len(ps.plan([f"c{i}" for i in range(40)])) == len(ps.FIXED) + ps.MAX_CRATES


def test_the_smoke_names_no_target_so_the_prepush_facts_check_stays_green():
    """tests/target_facts.rs failed feat/parity's prepush: tools/parity_smoke.py spelled the m32 target's crate directory (1 copy of a target fact)."""
    import re

    assert not re.search(r"llrm-x86-m\d+|x86_m\d+", Path(ps.__file__).read_text())


def test_a_known_red_step_is_the_only_one_skipped():
    """scev-port's scope.rs:227 panic made `sc` red for every landing; the flag skips that step and no other (#2438)."""
    ran = []
    steps = [(name, lambda _b, name=name: (ran.append(name), ("ok", name))[1], 1.0) for name in ("programs", "sc", "d_faces")]
    ok, line = ps.run_steps(steps, Path("."), clock=lambda: 0.0, known_red=frozenset({"sc"}))
    assert ran == ["programs", "d_faces"] and ok and "sc skipped" in line and "sc: known red, not run" in line


def fake_qcport(tmp_path, fail_level=None):
    """A QCport tree with render/d_faces.c and a stand-in llrm-c that fails the verifier at `fail_level` on d_faces.c only (stdout of the level it saw is kept)."""
    import stat

    (tmp_path / "src/render").mkdir(parents=True)
    (tmp_path / "src/render/d_faces.c").write_text("int x;\n")
    (tmp_path / "inc").mkdir()
    seen = tmp_path / "levels"
    compiler = tmp_path / "llrm-cc"
    compiler.write_text(f"#!/bin/sh\necho \"$1\" >> {seen}\ncase \"$*\" in *d_faces.c*) [ \"$1\" = \"{fail_level}\" ] && {{ echo 'llrm-c: @_d_draw_faces@3: block 964 leaves for (964) with no instruction choosing' >&2; exit 1; }};; esac\nexit 0\n")
    compiler.chmod(compiler.stat().st_mode | stat.S_IXUSR)
    return seen


def test_qcports_own_d_faces_is_compiled_at_all_four_levels_and_a_verifier_failure_at_one_fails_the_step(tmp_path, monkeypatch):
    """The flat d_faces of the unicorn step passed on 3858c5195 while QCport's d_faces.c failed the verifier at -O2/-O3: the smoke did not compile what QCport builds."""
    seen = fake_qcport(tmp_path, fail_level="-O2")
    monkeypatch.setenv("QCPORT", str(tmp_path / "src"))
    monkeypatch.setenv("QCPORT_INC", str(tmp_path / "inc"))
    verdict, detail, names = ps.qc_d_faces(tmp_path)
    assert verdict == "fail" and "-O2" in detail and "no instruction choosing" in detail, detail
    assert names == ["qc_d_faces -O2"] and seen.read_text().split() == ["-O1", "-O2", "-O3", "-Os"]
    seen.unlink()
    monkeypatch.setattr(ps, "compile_ok", ps.compile_ok)
    seen2 = fake_qcport(tmp_path / "ok")
    monkeypatch.setenv("QCPORT", str(tmp_path / "ok/src"))
    monkeypatch.setenv("QCPORT_INC", str(tmp_path / "ok/inc"))
    assert ps.qc_d_faces(tmp_path / "ok")[0] == "ok" and seen2.read_text().split() == ["-O1", "-O2", "-O3", "-Os"]


def test_two_halves_that_each_compile_but_not_together_fail_the_workspace_check(tmp_path):
    """#2544 (a fold calling `undef(context, ..)`) and #2564 (`undef` taking `&mut Context`, then `&Context`) each compiled; merged, feat/parity did not build
    for three hours. The smoke built only the touched crates' unit tests and passed."""
    (tmp_path / "Cargo.toml").write_text('[workspace]\nmembers = ["lib", "app"]\nresolver = "2"\n')
    (tmp_path / "lib/src").mkdir(parents=True)
    (tmp_path / "lib/Cargo.toml").write_text('[package]\nname = "lib"\nversion = "0.0.0"\nedition = "2021"\n')
    (tmp_path / "lib/src/lib.rs").write_text("pub struct Context;\npub fn undef(_context: &mut Context) {}\n")
    (tmp_path / "app/src").mkdir(parents=True)
    (tmp_path / "app/Cargo.toml").write_text('[package]\nname = "app"\nversion = "0.0.0"\nedition = "2021"\n[dependencies]\nlib = { path = "../lib" }\n')
    (tmp_path / "app/src/lib.rs").write_text("pub fn fold(context: &lib::Context) { lib::undef(context); }\n")
    assert ps.workspace_check(tmp_path, tmp_path / "t") != ""
    (tmp_path / "app/src/lib.rs").write_text("pub fn fold(context: &mut lib::Context) { lib::undef(context); }\n")
    assert ps.workspace_check(tmp_path, tmp_path / "t") == ""


def test_an_unused_mut_fails_the_land_time_workspace_check(tmp_path):
    """#3390 landed `let mut reads` with no mutation: the land-time build check ran without `-D warnings`, so feat/parity's fast-tier `build` step was red for every owner."""
    (tmp_path / "Cargo.toml").write_text('[package]\nname = "app"\nversion = "0.0.0"\nedition = "2021"\n')
    (tmp_path / "src").mkdir()
    (tmp_path / "src/lib.rs").write_text("pub fn f() -> u32 { let mut reads = 1; reads }\n")
    assert "mutable" in ps.workspace_check(tmp_path, tmp_path / "t")
    (tmp_path / "src/lib.rs").write_text("pub fn f() -> u32 { let reads = 1; reads }\n")
    assert ps.workspace_check(tmp_path, tmp_path / "t") == ""


def test_a_qcport_module_that_fails_at_o2_fails_the_step_whatever_d_faces_does(tmp_path, monkeypatch):
    """#2586: `BitCast of a float` killed 48 of 260 QCport compiles (combat, mathlib, d_sprite, snd ...) and the smoke passed: its QCport steps were sc.c -O1 and d_faces."""
    import stat

    for module in ps.QC_MODULES:
        (tmp_path / "src" / module).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / "src" / module).write_text("int x;\n")
    (tmp_path / "inc").mkdir()
    compiler = tmp_path / "llrm-cc"
    compiler.write_text("#!/bin/sh\ncase \"$*\" in *sound/snd.c*) echo 'llrm-c: @_snd_frame@3: BitCast of a float' >&2; exit 1;; esac\nexit 0\n")
    compiler.chmod(compiler.stat().st_mode | stat.S_IXUSR)
    monkeypatch.setenv("QCPORT", str(tmp_path / "src"))
    monkeypatch.setenv("QCPORT_INC", str(tmp_path / "inc"))
    verdict, detail, *_ = ps.qc_modules(tmp_path)
    assert verdict == "fail" and "sound/snd.c" in detail and "BitCast of a float" in detail, detail
    compiler.write_text("#!/bin/sh\nexit 0\n")
    assert ps.qc_modules(tmp_path)[0] == "ok"


def test_the_ratchets_run_whichever_crate_the_merge_touches_and_a_failing_one_refuses_it():
    """A reader added in llrm-transforms never ran llrm-mir's ratchets (loopdeletion +3, simpleloopunswitch +5 slipped in): the smoke ran a crate's unit tests only when
    the PR touched it."""
    commands = [" ".join(c) for c in ps.ratchet_commands()]
    assert any("-p llrm-mir --lib -- no_file_gains_a_hash_or_tree_map_keyed_by_a_dense_id" in c for c in commands), commands
    assert any("--test block_list_ratchet" in c for c in commands) and any("facts_rewrite" in c for c in commands)

    class Done:
        def __init__(self, code, err=""):
            self.returncode, self.stdout, self.stderr = code, "", err

    ran = []

    def fails_the_dense_id_ratchet(command):
        ran.append(command)
        return Done(1, "thread panicked: loopdeletion gained 3 readers") if "no_file_gains_a_hash_or_tree_map_keyed_by_a_dense_id" in " ".join(command) else Done(0)

    why = ps.ratchets(run=fails_the_dense_id_ratchet)
    assert "no_file_gains_a_hash_or_tree_map_keyed_by_a_dense_id" in why and "loopdeletion" in why, why
    assert ps.ratchets(run=lambda command: Done(0)) == ""


def test_every_failing_ratchet_is_named_not_only_the_first():
    """The subset rule compares failure sets: a head red on two ratchets must show both, or a merge that fixes one looks like it adds the other."""
    class Done:
        def __init__(self, code):
            self.returncode, self.stdout, self.stderr = code, "", "thread panicked: x" if code else ""

    failed = ps.ratchet_failures(run=lambda command: Done(1 if ("block_list_ratchet" in " ".join(command) or "facts_rewrite" in " ".join(command)) else 0))
    assert set(failed) == {"ratchet:block_list_ratchet", "ratchet:facts_rewrite"}, failed


def test_the_failed_steps_come_out_of_the_smoke_line_by_name():
    assert ps.failing_names("PARITY SMOKE FAIL qc_modules: QCport game/combat.c -O2: x; run: y: z | programs ok 0.3 s") == ["qc_modules", "run"]
    assert ps.failing_names("PARITY SMOKE PASS programs ok 0.3 s") == []


def test_a_run_over_the_cpu_budget_alone_is_run_once_more_before_it_fails():
    """#2730 (tools only, no unit test in its set) was refused 'over the budget' on a loaded host: the same steps cost more CPU seconds under load."""
    verdicts = iter([(False, "PARITY SMOKE FAIL over the budget: 11.2 s > 10 s | programs ok 0.3 s"), (True, "PARITY SMOKE PASS programs ok 0.3 s | 8.4 s CPU of 10 s")])
    ok, line = ps.run_twice_over_budget(lambda: next(verdicts))
    assert ok and "second run" in line
    calls = []
    both = lambda: (calls.append(1), (False, "PARITY SMOKE FAIL over the budget: 11 s > 10 s | x"))[1]  # noqa: E731
    assert ps.run_twice_over_budget(both)[0] is False and len(calls) == 2
    other = lambda: (calls.append(1), (False, "PARITY SMOKE FAIL run: x; over the budget: 11 s > 10 s | y"))[1]  # noqa: E731
    calls.clear()
    assert ps.run_twice_over_budget(other)[0] is False and len(calls) == 1


def test_a_compile_that_outgrows_the_memory_limit_is_reported_as_a_memory_limit(monkeypatch):
    """A runaway compile took 50 GB before a timeout stopped it; every smoke compile now runs under `ulimit -v` and says so."""
    import sys as _sys

    monkeypatch.setattr(ps.cpulimit, "MEMORY_KB", 200_000)
    why = ps.compile_ok([_sys.executable, "-c", "x = bytearray(900 * 1024 * 1024)"], dict(__import__("os").environ))
    assert why and why.startswith("memory limit"), why


def test_an_unformatted_file_refuses_a_pr_through_the_prepush_ratchet():
    """#2732 and #2735 landed with unformatted files: the smoke ran neither fmt nor the docs scans. prepush's failing steps are ratchets of the smoke."""
    class Done:
        def __init__(self, out, code):
            self.stdout, self.stderr, self.returncode = out, "", code

    failed = ps.prepush_failures(run=lambda command: Done("PREPUSH FAIL: fmt, docs\ncovers: fmt, docs", 1))
    assert set(failed) == {"ratchet:prepush-fmt", "ratchet:prepush-docs"}, failed
    assert ps.prepush_failures(run=lambda command: Done("PREPUSH PASS\n", 0)) == {}
    assert set(ps.prepush_failures(run=lambda command: Done("", 2))) == {"ratchet:prepush-did-not-run"}


def test_unused_code_fails_the_warnings_check_and_clean_code_passes(tmp_path):
    """deadstore.rs landed with unused code under -D warnings."""
    (tmp_path / "src").mkdir()
    (tmp_path / "Cargo.toml").write_text('[package]\nname = "w"\nversion = "0.0.0"\nedition = "2021"\n')
    (tmp_path / "src/lib.rs").write_text("fn unused() {}\n")
    why = ps.warnings_check(tmp_path, tmp_path / "t")
    assert why, "an unused function must fail under -D warnings"
    (tmp_path / "src/lib.rs").write_text("pub fn used() {}\n")
    assert ps.warnings_check(tmp_path, tmp_path / "t") == ""


def test_a_failed_unit_step_names_each_failed_test_not_its_crate():
    """The subset rule compared checks: a PR adding one failing test inside an already-red crate failed `unit llrm-qb`, which the red head failed too, and landed."""
    out = "running 3 tests\\nfailures:\\n\\n---- t::a stdout ----\\n    printed\\n\\nfailures:\\n    t::a\\n    t::b\\n\\ntest result: FAILED. 1 passed; 2 failed\\n".replace("\\n", "\n")
    assert ps.failed_tests(out) == ["t::a", "t::b"]
    step = lambda bins: ("fail", "llrm-qb: x", ["unit llrm-qb::t::a", "unit llrm-qb::t::b"])
    ok, line = ps.run_steps([("unit llrm-qb", step, 1.0)], Path("."), clock=lambda: 0.0)
    assert not ok and ps.failing_names(line) == ["unit llrm-qb::t::a", "unit llrm-qb::t::b"]
    assert ps.failing_names("PARITY SMOKE FAIL run: a: b | x") == ["run"]


def test_the_budget_is_instructions_so_a_loaded_host_passes_and_a_doubled_step_fails():
    """A tools-only PR failed 'over the budget' at 16.1 s > 15 s at load 32 on 32 cores: the CPU seconds of the same instructions stretch with contention."""
    steps = [("run", lambda bins: ("ok", ""), 1.0)]
    loaded = iter([0.0, 0.0, 0.0, 40.0])  # 40 CPU seconds on a loaded host
    ok, line = ps.run_steps(steps, Path("."), clock=lambda: next(loaded), meter=counted(10_000_000_000), limit=12_000_000_000)
    assert ok and "10.00 G instr of 12.00 G (40.0 s CPU for reference)" in line
    quiet = iter([0.0, 0.0, 0.0, 5.0])  # a quiet host, a step that executes twice the instructions
    ok, line = ps.run_steps(steps, Path("."), clock=lambda: next(quiet), meter=counted(24_000_000_000), limit=12_000_000_000)
    assert not ok and ps.failing_names(line) == ["over the budget"]
    fallback = iter([0.0, 0.0, 0.0, 20.0])  # no perf: the CPU seconds decide, as before
    ok, line = ps.run_steps(steps, Path("."), clock=lambda: next(fallback), meter=counted(None), limit=12_000_000_000)
    assert not ok and "perf unavailable" in line


def test_each_smoke_step_prints_its_stage_line_before_it_runs(capsys):
    """The smoke's stage times were only in the final line: a step announces itself, flushed, when it starts."""
    order = []
    step = lambda bins: (order.append(capsys.readouterr().out.strip().splitlines()[-1]), ("ok", ""))[1]
    ps.run_steps([("sc", step, 1.0)], Path("."), clock=lambda: 0.0)
    assert order and order[0].endswith(" STAGE step sc")


def test_a_header_change_without_the_docs_sync_is_refused_with_the_page_named():
    """jumps.rs landed without `docsbuild --sync`; every docs-touching PR after it was refused with no page named."""
    class Done:
        def __init__(self, out, code):
            self.stdout, self.stderr, self.returncode = out, "", code

    stale = "1 problem(s)\ngenerated docs are out of date; run python3 tools/docs/docsbuild/build.py --sync\n  docs/optimizations/backend/jumps.md\n  docs/optimizations/passes/readme.md\n"
    failed = ps.docs_failures(run=lambda command: Done(stale, 1))
    assert any("docs/optimizations/backend/jumps.md" in name for name in failed) and all("--sync" in why for why in failed.values()), failed
    assert ps.docs_failures(run=lambda command: Done("ok\n", 0)) == {}


def test_the_ratchets_run_side_by_side_and_report_in_command_order(monkeypatch):
    """A precheck spent ~45 s of its ~70 s running ten cargo test ratchets one after another; four at a time they overlap their links and runs."""
    import threading

    monkeypatch.setattr(ps, "RATCHET_WORKERS", 4)
    monkeypatch.setattr(ps, "ratchet_commands", lambda tiers=None: [["cargo", "test", "a"], ["cargo", "test", "b"], ["cargo", "test", "facts_rewrite"], ["cargo", "test", "d"]])  # an even count: the barrier takes pairs
    gate = threading.Barrier(2, timeout=5)  # a serial runner never reaches 2 parties: the barrier breaks (10 ratchets: pairs)

    class Done:
        def __init__(self, code):
            self.returncode, self.stdout, self.stderr = code, "", ""

    def run(command):
        gate.wait()
        return Done(1 if "facts_rewrite" in " ".join(command) else 0)

    failed = ps.ratchet_failures(run=run)
    assert list(failed) == ["ratchet:facts_rewrite"], failed


def test_the_ratchets_run_one_at_a_time_by_default():
    """Four at a time cost 600-750 s a PR at load 70 against ~57 s in a row (cargo's lock serializes the compiles; the links collide): serial is the default, RATCHET_WORKERS overrides."""
    assert ps.RATCHET_WORKERS == 1


FAILURES_BLOCK = "running 3 tests\n\nfailures:\n\nfailures:\n    first_red\n    second_red\n\ntest result: FAILED. 1 passed; 2 failed\n"


def test_a_red_ratchet_is_listed_by_its_failing_tests_not_whole():
    """A ratchet is a test binary: listed as `ratchet:x`, a second test going red inside it was 'the same step failing' and no merge was refused for it."""

    class Done:
        returncode, stdout, stderr = 1, FAILURES_BLOCK, ""

    failed = ps.ratchet_failures(run=lambda command: Done() if "block_list_ratchet" in " ".join(command) else type("Ok", (), {"returncode": 0, "stdout": "", "stderr": ""})())
    assert set(failed) == {"ratchet:block_list_ratchet::first_red", "ratchet:block_list_ratchet::second_red"}, failed


def test_a_red_run_step_is_listed_by_its_failing_programs():
    out = "FAIL  qb/log_exp_digits.bas [-O2]: line 1: want x\nFAIL  qb/log_exp_digits.bas [-Os]: line 1\nFAIL  c/x.c [-O1]: y\n4 programs: 1 pass"
    assert ps.failing_programs(out) == ["run qb/log_exp_digits.bas [-O2]", "run qb/log_exp_digits.bas [-Os]", "run c/x.c [-O1]"]
    assert ps.failing_programs("no failures named") == []


def test_every_failing_qcport_module_is_listed_not_the_first(tmp_path, monkeypatch):
    for sub in ("host", "render", "game", "sound", "ui", "model", "qgl"):
        (tmp_path / "src" / sub).mkdir(parents=True)
    for module in ps.QC_MODULES:
        (tmp_path / "src" / module).write_text("int x;")
    (tmp_path / "inc").mkdir()
    monkeypatch.setenv("QCPORT", str(tmp_path / "src"))
    monkeypatch.setenv("QCPORT_INC", str(tmp_path / "inc"))
    compiler = tmp_path / "llrm-cc"
    compiler.write_text("#!/bin/sh\ncase \"$*\" in *combat.c*|*snd.c*) echo broken >&2; exit 1;; esac\nexit 0\n")
    compiler.chmod(0o755)
    verdict, _, names = ps.qc_modules(tmp_path)
    assert verdict == "fail" and names == ["qc_modules game/combat.c", "qc_modules sound/snd.c"], names


def test_a_pr_touching_the_qb_frontend_x86_isel_or_the_runtime_spec_runs_every_tests_run_group():
    for path in ("crates/frontends/qb/src/compile.rs", "crates/frontends/qb/src/semantic.rs", "crates/frontends/qb/runtime.toml", "runtime/qb/strconv.c",
                 "crates/target/llrm-x86/src/select.rs", "crates/codegen/llrm-codegen/src/backend/isel.rs"):
        assert ps.runs_whole_groups([path]), path
    for path in ("crates/opt/llrm-transforms/src/gvn.rs", "docs/arch.md", "tools/gate/gate.py", "crates/frontends/llrm-c/src/lib.rs"):
        assert not ps.runs_whole_groups([path]), path
    assert ps.runs_whole_groups(["docs/arch.md", "crates/frontends/qb/src/lexer.rs"])


def test_a_planted_qb_output_diff_in_a_group_goes_red_and_refuses_the_merge():
    """The gate ran a dozen programs, so three tests/run qb programs (12 results) went red on parity unseen. A group's FAIL line is a failing name; the merged tree failing it, the head not, refuses."""
    import parity_squash as sq

    class Done:
        returncode = 1
        stdout = "[qb] 170 programs: 169 pass, 0 known, 1 fail\nFAIL  qb/log_exp_digits.bas [-O2]: line 1: want ' 2.30', got ' 2.31'\n"
        stderr = ""

    failed = ps.group_failures("/bins", run=lambda *a, **k: Done())
    assert failed == {"run qb/log_exp_digits.bas [-O2]"}, failed
    why = sq.subset_refusals(set(), failed, fixes=False)
    assert why and "qb/log_exp_digits.bas [-O2]" in why[0], why
    assert sq.subset_refusals(failed, failed, fixes=False) == []

    class Clean:
        returncode = 0
        stdout = "[qb] 170 programs: 170 pass, 0 known, 0 fail\n"
        stderr = ""

    assert ps.group_failures("/bins", run=lambda *a, **k: Clean()) == set()

    class Silent:
        returncode = 2
        stdout = ""
        stderr = "boom"

    assert ps.group_failures("/bins", run=lambda *a, **k: Silent()) == {"run-groups-did-not-finish"}


def test_every_qcport_module_is_compiled_for_a_pr_that_touches_a_crate(tmp_path):
    """#3422 broke QCport `item` at four levels and passed the gate: qc_modules compiled four modules at -O2. A crate PR now compiles all 65 at four levels, and a failing one is a name."""
    assert ps.touches_the_compiler(["crates/opt/llrm-transforms/src/simplifycfg.rs"]) and not ps.touches_the_compiler(["docs/arch.md", "tools/gate/gate.py"])
    (tmp_path / "src").mkdir()
    (tmp_path / "inc").mkdir()
    import os

    os.environ["QCPORT"], os.environ["QCPORT_INC"] = str(tmp_path / "src"), str(tmp_path / "inc")
    try:
        seen = {}

        def sweep(compiler, qcport, include):
            seen["compiler"] = compiler
            return [("item", "-O1", "a poison initializer"), ("item", "-Os", "a poison initializer")]

        assert ps.qcport_failures("/bins", sweep=sweep) == {"qc item -O1", "qc item -Os"}
        assert str(seen["compiler"]) == "/bins/llrm-cc"
        assert ps.qcport_failures("/bins", sweep=lambda *a: []) == set()
    finally:
        del os.environ["QCPORT"], os.environ["QCPORT_INC"]


def test_a_planted_qbrt_difference_is_a_failing_name_and_a_program_that_did_not_compile_is_one():
    """tests/qbrt (218 results) is the QB product's differential and the gate did not run it: #3422 broke dgroup_nearly_full at m16 (byte 11 want 104 got 13) unseen."""
    import parity_squash as sq

    class Done:
        returncode = 1
        stdout = "PROGRAMS 2 a b\nCOMPILE-FAIL b -O2: x\nNOTPASS dgroup_nearly_full x86-m16 ok ok byte 11: want 104 got 13\nRESULT 1 of 4 pass\n"
        stderr = ""

    got = ps.qbrt_failures("/bins", run=lambda *a, **k: Done())
    assert got == {"qbrt dgroup_nearly_full x86-m16", "qbrt b did-not-compile"}, got
    assert sq.subset_refusals(set(), got, fixes=False) and sq.subset_refusals(got, got, fixes=False) == []

    class Silent:
        returncode = 2
        stdout = ""
        stderr = "boom"

    assert ps.qbrt_failures("/bins", run=lambda *a, **k: Silent()) == {"qbrt-did-not-finish"}


def test_a_transforms_only_pr_runs_the_lib_tests_of_llrm_codegen_and_a_red_one_is_a_new_name():
    """Five llrm-codegen isel tests were red on parity for days: a PR touching llrm-transforms never ran llrm-codegen's lib tests. The closure comes from cargo metadata, not a list."""
    import parity_squash as sq

    packages, graph = ps.workspace_graph()
    touched = ps.touched_packages(["crates/opt/llrm-transforms/src/gvn.rs", "docs/arch.md"], packages)
    assert touched == {"llrm-transforms"}
    closure = ps.dependants(touched, graph)
    assert {"llrm-transforms", "llrm-codegen", "llrm-c", "llrm-qb"} <= closure and "llrm-support" not in closure, sorted(closure)
    head = {"unit llrm-codegen::backend::isel_tests::test_old_red"}
    merged = head | {"unit llrm-codegen::backend::isel_tests::test_made_red_by_transforms"}
    why = sq.subset_refusals(head, merged, fixes=False)
    assert why and "test_made_red_by_transforms" in why[0] and "test_old_red" not in why[0].split(", which")[0], why


def test_dependants_lib_failures_name_each_failing_test_per_package():
    class Done:
        returncode = 101
        stdout = "failures:\n\nfailures:\n    backend::isel_tests::test_x\n\ntest result: FAILED. 1 passed; 1 failed\n"
        stderr = ""

    ran = []

    def run(command, **kwargs):
        ran.append(command[-3:])
        return Done()

    assert ps.dependants_lib_failures({"llrm-codegen"}, run=run) == {"unit llrm-codegen::backend::isel_tests::test_x"}
    assert ran == [["-p", "llrm-codegen", "--lib"]]


def test_a_crate_that_did_not_run_shows_why_in_its_refusal():
    """#4269 was refused twice for `unit <crate>::did-not-run` and the log held nothing else: the compile error (or the crash's exit and last lines) of the crate's test run is kept."""

    def failing(stderr, code):
        class Done:
            returncode = code
            stdout = ""

        Done.stderr = stderr
        return lambda command, **kwargs: Done()

    ps.CAUSES.clear()
    broken = "   Compiling llrm-codegen\nerror[E0432]: unresolved import `crate::nothing`\n --> src/lib.rs:1:5\nerror: could not compile `llrm-codegen` (lib test)\n"
    assert ps.dependants_lib_failures({"llrm-codegen"}, run=failing(broken, 101)) == {"unit llrm-codegen::did-not-run"}
    assert "unresolved import `crate::nothing`" in ps.causes_of({"unit llrm-codegen::did-not-run"})
    ps.CAUSES.clear()
    crashed = "running 3 tests\nerror: test failed, to rerun pass `-p llrm-codegen --lib`\n\nCaused by:\n  process didn't exit successfully (signal: 11, SIGSEGV)\n"
    ps.dependants_lib_failures({"llrm-codegen"}, run=failing(crashed, 101))
    assert "SIGSEGV" in ps.causes_of({"unit llrm-codegen::did-not-run"})
    assert ps.causes_of({"unit llrm-codegen::some_test"}) == ""


def test_a_listed_test_that_passes_is_reported(monkeypatch):
    """tools/xfail.toml still listed 17 tests after they passed or were gone, and PRs landed: the header says a listed test that passes fails the gate."""
    import xfail

    monkeypatch.setattr(xfail, "load", lambda *a, **k: {"unit llrm-nib::test_debug::fixed": {"issue": 1}, "unit llrm-nib::test_debug::still_red": {"issue": 1}})

    class Done:
        returncode = 101
        stdout = "test test_debug::fixed ... ok\ntest test_debug::still_red ... FAILED\nfailures:\n\nfailures:\n    test_debug::still_red\n\ntest result: FAILED. 1 passed; 1 failed\n"
        stderr = ""

    assert ps.dependants_lib_failures({"llrm-nib"}, run=lambda command, **kwargs: Done()) == {"xpass unit llrm-nib::test_debug::fixed"}


def test_every_integration_target_is_mapped_and_an_unmapped_one_is_refused():
    """A transforms PR that breaks loop_rotation must run it: each root integration target is mapped from what its source imports and the compilers it runs. A new target with
    no crate behind it that is neither a scanner nor nightly-only fails here."""
    mapping = ps.integration_map()
    assert len(mapping) >= 116 and ps.unmapped_targets(mapping) == [], ps.unmapped_targets(mapping)
    assert ps.unmapped_targets({**mapping, "a_new_target_nobody_mapped": set()}) == ["a_new_target_nobody_mapped"]
    assert "loop_rotation" in ps.integration_targets_for(["crates/opt/llrm-transforms/src/gvn.rs"], {"llrm-transforms"}, mapping)
    nib_only = ps.integration_targets_for(["crates/frontends/nib/src/x.rs"], {"llrm-nib"}, mapping)
    assert "loop_rotation" not in nib_only and set(ps.SCANNERS) <= set(nib_only) and not set(ps.NIGHTLY_ONLY) & set(nib_only)
    assert "docs_links" in ps.integration_targets_for(["docs/arch.md"], set(), mapping)
    assert "header_copy" in ps.integration_targets_for(["tests/header_copy.rs"], set(), mapping)


def test_a_target_reaches_the_crates_and_compilers_of_the_helpers_it_uses(tmp_path):
    """95 targets, after C1 moved them to `common::cc::llrm_cc()`, imported no crate in their own text and were all unmapped: the helper's imports are the target's."""
    (tmp_path / "tests" / "common").mkdir(parents=True)
    (tmp_path / "tests" / "common" / "mod.rs").write_text("use llrm_driver::all;")
    (tmp_path / "tests" / "common" / "cc.rs").write_text('env!("CARGO_BIN_EXE_llrm-cc")')
    (tmp_path / "tests" / "common" / "other.rs").write_text("use llrm_nib::x;")
    text = ps.with_common("mod common;\nfn t() { common::cc::llrm_cc(); }", tmp_path)
    assert "llrm_driver" in text and "CARGO_BIN_EXE_llrm-cc" in text and "llrm_nib" not in text
    assert "llrm_driver" in ps.with_common("pub mod common;", tmp_path)
    assert ps.with_common("fn t() {}", tmp_path) == "fn t() {}"


def test_a_runtime_change_runs_the_targets_that_build_the_runtime_with_cc_and_a_docs_change_does_not():
    """posix_os, qb_dev_interface, qb_sdl and qb_dirty import no crate: unmapped, they ran for no change at all."""
    mapping = ps.integration_map()
    runtime = ps.integration_targets_for(["runtime/qb/dev/host/video.c"], set(), mapping)
    assert {"posix_os", "qb_dev_interface", "qb_sdl", "qb_dirty"} <= set(runtime)
    assert not {"posix_os", "qb_sdl"} & set(ps.integration_targets_for(["docs/arch.md"], set(), mapping))


def test_the_check_target_is_mapped_to_every_compiler_its_run_lines_drive():
    """tests/check.rs names only llrm-qb (CARGO_BIN_EXE) but runs the tool of each file's RUN line: copy_idioms.c drives llrm-c, and a change to llrm-c ran no check."""
    mapping = ps.integration_map()
    packages, _ = ps.workspace_graph()
    # the crate directories come from the workspace: the re-org moved llrm-qb to crates/frontends/qb and the hard-coded path touched no package
    for tool, changed in ((tool, f"{packages[tool].relative_to(ps.ROOT)}/src/lib.rs") for tool in ("llrm-c", "llrm-nib", "llrm-qb")):
        chosen = ps.integration_targets_for([changed], ps.touched_packages([changed], packages), mapping)
        assert "check" in chosen, f"a change to {tool} does not run tests/check"


def test_integration_failures_name_the_red_tests_less_the_listed_ones():
    """The positive control of #3542 printed `integration ?::test`: cargo's `Running tests/X.rs` lines are on stderr and the test lines on stdout, read as two blocks."""
    import subprocess as sp

    def run(command, **kwargs):
        interleaved = "     Running tests/attribute.rs (x)\ntest a_red_one ... FAILED\ntest result: FAILED. 0 passed; 1 failed\n"

        class Done:
            returncode = 101

        done = Done()
        if kwargs.get("stderr") is sp.STDOUT:
            done.stdout, done.stderr = interleaved, ""
        else:  # two pipes: the Running line is in stderr, the tests in stdout
            done.stdout, done.stderr = interleaved.split("\n", 1)[1], interleaved.split("\n", 1)[0] + "\n"
        return done

    assert ps.integration_failures(["attribute"], run=run) == {"integration attribute::a_red_one"}
    assert ps.integration_failures([], run=run) == set()


def test_a_24th_docsbuild_line_is_a_new_name_when_the_head_already_has_23():
    """docsbuild --check was red on the base with 23 lines; one `ratchet:docs` name let a 24th through."""
    import parity_squash as sq

    class Done:
        def __init__(self, text):
            self.returncode, self.stdout, self.stderr = 1, text, ""

    base = "".join(f"crates/a/f{n}.rs: no # Example\n" for n in range(23))
    head = set(ps.docs_failures(run=lambda command: Done(base)))
    merged = set(ps.docs_failures(run=lambda command: Done(base + "crates/a/new.rs: no # Example\n")))
    assert len(head) == 23 and len(merged) == 24
    why = sq.subset_refusals(head, merged, fixes=False)
    assert why and "new.rs" in why[0], why
    assert sq.subset_refusals(head, set(head), fixes=False) == []
    assert sq.new_names({"ratchet:docs"}, merged) == []  # a head cached before the lines were named covers them


def test_llrm_c_lib_tests_run_with_the_toolchain_feature_and_other_crates_without():
    """`cargo test -p llrm-c --lib` never ran the call-boundary and compile-listing tests (feature `toolchain`): an interrupt handler was refused from 10-19 to 10-23 and 16 listings went red unseen."""
    assert ps.lib_command("llrm-c") == ["cargo", "test", "--no-fail-fast", "-p", "llrm-c", "--lib", "--features", "toolchain"]
    assert ps.lib_command("llrm-codegen") == ["cargo", "test", "--no-fail-fast", "-p", "llrm-codegen", "--lib"]

    class Done:
        returncode = 0
        stdout = ""
        stderr = ""

    ran = []

    def run(command, **kwargs):
        ran.append(command)
        return Done()

    ps.dependants_lib_failures({"llrm-c", "llrm-codegen"}, run=run)
    assert any("--features" in command and "llrm-c" in command for command in ran)
    assert not any("--features" in command and "llrm-codegen" in command for command in ran)


def test_a_selectiondag_change_runs_the_selectiondag_registry():
    """#3899 landed with tests/selectiondag_registry.rs red (SD403 had no row): the target read no crate, was in neither SCANNERS nor NIGHTLY_ONLY, so no selection
    of a PR's targets ever named it."""
    mapping = ps.integration_map()
    files = ["crates/codegen/llrm-codegen/src/backend/selectiondag/dag_function.rs"]
    packages, _ = ps.workspace_graph()
    chosen = ps.integration_targets_for(files, ps.touched_packages(files, packages), mapping)
    assert "selectiondag_registry" in chosen, chosen


def test_a_compile_to_the_null_device_reaches_the_encoder(tmp_path):
    """The smoke layer compiled with `-o /dev/null`, where llrm-c writes assembly text and never encodes: a program that died in the encoder passed."""
    compiler = tmp_path / "llrm-cc"
    compiler.write_text("#!/usr/bin/env python3\nimport sys\nout = sys.argv[sys.argv.index('-o') + 1]\nif out.endswith('.obj'):\n    sys.exit('llrm-c: Semantics(op=BINARY)')\n")
    compiler.chmod(0o755)
    assert ps.compile_ok([str(compiler), "x.c", "-o", __import__("os").devnull], {}) is not None


def test_a_refusal_can_carry_the_first_panic_line_of_a_failed_test():
    """#4172 was refused three times with the test's name and nothing else; the cause was a `debug_assert_eq!` that fires in the runner's debug profile and not in release."""
    output = """running 1 test
test backend::selectiondag::tests::legalize_types::the_second_half_of_a_far_i64_load_moves_the_offset_and_keeps_the_selector ... FAILED

failures:

---- backend::selectiondag::tests::legalize_types::the_second_half_of_a_far_i64_load_moves_the_offset_and_keeps_the_selector stdout ----

thread 'backend::selectiondag::tests::legalize_types::the_second_half_of_a_far_i64_load_moves_the_offset_and_keeps_the_selector' (1924096) panicked at crates/codegen/llrm-codegen/src/backend/selectiondag/legalize_types.rs:173:13:
assertion `left == right` failed: a node on the worklist is ready
  left: Processed
 right: Ready
"""
    reasons = ps.panic_reasons(output)
    assert reasons["the_second_half_of_a_far_i64_load_moves_the_offset_and_keeps_the_selector"] == "assertion `left == right` failed: a node on the worklist is ready (left: Processed, right: Ready)"


def test_the_compile_smoke_drives_the_c_compiler_llrm_cc_names(tmp_path, monkeypatch):
    """C2 switches the C compiler to llrm-cc: a smoke that compiled with the name "llrm-c" would still pass on a dist that has only the old binary."""
    seen = []
    monkeypatch.setattr(ps, "compile_ok", lambda command, env: seen.append(Path(command[0]).name))
    for name in ("llrm-cc", "llrm-c"):
        (tmp_path / name).write_text("")
    ps.programs(tmp_path)
    assert {"llrm-cc"} <= set(seen) and "llrm-c" not in seen



def test_a_failing_scanner_names_the_stale_pages_in_its_cause():
    """docs_in_sync was refused at 01:41, 01:44 and 02:08 as a bare test name; w1 could not tell which page, and the scratch tree passed."""
    output = """\
     Running tests/docs_in_sync.rs (target/release/deps/docs_in_sync-abc)

running 1 test
test test_no_generated_page_is_stale ... FAILED

failures:

---- test_no_generated_page_is_stale stdout ----

thread 'test_no_generated_page_is_stale' (123) panicked at tests/docs_in_sync.rs:24:5:
run python3 tools/docs/docsbuild/build.py --sync:
  docs/optimizations/passes/lsr_solve.md
  docs/optimizations/passes/sinkcast.md
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
"""
    class Done:
        returncode = 101
        stdout = output

    ps.CAUSES.clear()
    failed = ps.integration_failures(["docs_in_sync"], run=lambda *a, **k: Done)
    assert failed == {"integration docs_in_sync::test_no_generated_page_is_stale"}
    cause = ps.causes_of(failed)
    assert "lsr_solve.md" in cause and "sinkcast.md" in cause and "RUST_BACKTRACE" not in cause


def test_the_crates_whose_prs_blew_the_instruction_budget_have_a_ranking():
    """llrm-codegen had no ranking, so a codegen PR ran its whole lib: 168 G of 60 G, #5325's batch refused four times."""
    ranking = ps.selection().get("units", {})
    for crate in ("llrm-codegen", "llrm-mir", "llrm-cc"):
        assert ps.unit_tests(crate, 6.4, {k: [(n, s) for n, s in v["tests"]] for k, v in ranking.items()}), crate


def test_an_unranked_unit_step_runs_one_test_thread(monkeypatch):
    """An unranked crate runs its whole lib cut by wall time; cargo's thread per core made that 32x the CPU and instructions the wall allowance was charged."""
    seen = []
    monkeypatch.setattr(ps, "run_cut", lambda command, seconds, cwd, env=None: seen.append(env) or (0, ""))
    monkeypatch.setattr(ps, "selection", lambda: {"units": {"ranked": {"tests": [["t_a", 0.1]]}}})
    monkeypatch.setattr(ps.xfail, "load_for", lambda root: {})
    ps.unit_step("unranked", 5.0)(Path("."))
    ps.unit_step("ranked", 5.0)(Path("."))
    assert seen[0]["RUST_TEST_THREADS"] == "1" and seen[1] is None, seen


def test_a_runner_whose_perf_cannot_count_still_runs_the_commands(tmp_path, monkeypatch):
    """Gate run 3: perf installed, `instructions:u event is not supported`; every command the Meter wrapped died with perf's exit 255 before it ran."""
    fake = tmp_path / "perf"
    fake.write_text("#!/bin/sh\necho 'The instructions:u event is not supported.' >&2\nexit 255\n")
    fake.chmod(0o755)
    monkeypatch.undo()  # the real Meter: the autouse fixture swaps in a stub
    monkeypatch.setenv("PATH", f"{tmp_path}{os.pathsep}{os.environ['PATH']}")
    with ps.Meter() as counted:
        done = subprocess.run([sys.executable, "-c", "print(7)"], capture_output=True, text=True)
    assert (done.returncode, done.stdout.strip(), counted.value) == (0, "7", None)
