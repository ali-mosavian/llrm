"""The per-merge tests of feat/parity: the selection fits 10 s of CPU however many crates a merge touches, an overrun or a failed step fails it, and the last line says which."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import parity_smoke as ps  # noqa: E402


def clocked(costs):
    """A clock and steps on virtual time: each step advances the clock by its cost and answers `verdict`."""
    now = [0.0]
    steps = [(name, lambda _bins, cost=cost, verdict=verdict, name=name: (now.__setitem__(0, now[0] + cost), (verdict, name))[1], cost) for name, cost, verdict in costs]
    return (lambda: now[0]), steps


def test_the_declared_selection_fits_the_budget_whatever_the_merge_touches():
    """Every merge pays this: a step added without dropping another, or a crate's tests added on top, pushes the total past the 10 s the user set."""
    assert ps.BUDGET <= 10.0 and ps.BUDGET - ps.fixed_cost() - ps.CARGO_OVERHEAD * ps.MAX_CRATES >= ps.UNIT_FLOOR / 2, (ps.fixed_cost(), "the fixed steps must leave the unit tests their floor")
    for crates in ([], ["a"], ["a", "b", "c"], [f"c{i}" for i in range(40)]):
        assert ps.declared(crates) <= ps.BUDGET + 1e-9, (len(crates), ps.declared(crates))


def test_a_set_within_the_budget_ends_in_pass():
    clock, steps = clocked([("a", 2.0, "ok"), ("b", 3.0, "ok")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert ok and line.startswith("PARITY SMOKE PASS") and "5.0 s CPU of 10 s" in line


def test_a_set_over_the_budget_fails_whatever_its_steps_say():
    """A selection that grew to 14 s slows every merge of the branch: it must not read as a pass."""
    clock, steps = clocked([("a", 7.0, "ok"), ("b", 7.0, "ok")])
    ok, line = ps.run_steps(steps, Path("."), clock)
    assert not ok and line.startswith("PARITY SMOKE FAIL") and "over the budget: 14.0 s > 10 s" in line


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
        except FileNotFoundError:
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
    compiler = tmp_path / "llrm-c"
    compiler.write_text(f"#!/bin/sh\necho \"$1\" >> {seen}\ncase \"$*\" in *d_faces.c*) [ \"$1\" = \"{fail_level}\" ] && {{ echo 'llrm-c: @_d_draw_faces@3: block 964 leaves for (964) with no instruction choosing' >&2; exit 1; }};; esac\nexit 0\n")
    compiler.chmod(compiler.stat().st_mode | stat.S_IXUSR)
    return seen


def test_qcports_own_d_faces_is_compiled_at_all_four_levels_and_a_verifier_failure_at_one_fails_the_step(tmp_path, monkeypatch):
    """The flat d_faces of the unicorn step passed on 3858c5195 while QCport's d_faces.c failed the verifier at -O2/-O3: the smoke did not compile what QCport builds."""
    seen = fake_qcport(tmp_path, fail_level="-O2")
    monkeypatch.setenv("QCPORT", str(tmp_path / "src"))
    monkeypatch.setenv("QCPORT_INC", str(tmp_path / "inc"))
    verdict, detail = ps.qc_d_faces(tmp_path)
    assert verdict == "fail" and "-O2" in detail and "no instruction choosing" in detail, detail
    assert seen.read_text().split() == ["-O1", "-O2"]
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
    assert ps.workspace_check(tmp_path) != ""
    (tmp_path / "app/src/lib.rs").write_text("pub fn fold(context: &mut lib::Context) { lib::undef(context); }\n")
    assert ps.workspace_check(tmp_path) == ""


def test_a_qcport_module_that_fails_at_o2_fails_the_step_whatever_d_faces_does(tmp_path, monkeypatch):
    """#2586: `BitCast of a float` killed 48 of 260 QCport compiles (combat, mathlib, d_sprite, snd ...) and the smoke passed: its QCport steps were sc.c -O1 and d_faces."""
    import stat

    for module in ps.QC_MODULES:
        (tmp_path / "src" / module).parent.mkdir(parents=True, exist_ok=True)
        (tmp_path / "src" / module).write_text("int x;\n")
    (tmp_path / "inc").mkdir()
    compiler = tmp_path / "llrm-c"
    compiler.write_text("#!/bin/sh\ncase \"$*\" in *sound/snd.c*) echo 'llrm-c: @_snd_frame@3: BitCast of a float' >&2; exit 1;; esac\nexit 0\n")
    compiler.chmod(compiler.stat().st_mode | stat.S_IXUSR)
    monkeypatch.setenv("QCPORT", str(tmp_path / "src"))
    monkeypatch.setenv("QCPORT_INC", str(tmp_path / "inc"))
    verdict, detail = ps.qc_modules(tmp_path)
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
