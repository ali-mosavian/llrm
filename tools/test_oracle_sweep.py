"""The sweep tells a refusal in words from a panic: a run that panics under an oracle must fail it (the first sweep found 48 panics among
123 non-zero exits, 75 of them refusals)."""

import oracle_sweep


def test_a_panic_is_a_finding_and_a_refusal_in_words_is_not():
    panic = "thread 'main' panicked at crates/ir/llrm-mir/src/passes.rs:2529:9:\nrotate added a memory operation: the frozen summaries\n"
    assert oracle_sweep.classify(101, panic) == "rotate added a memory operation: the frozen summaries"
    assert oracle_sweep.classify(1, "llrm-nib: entry function 'main' does not exist") is None
    assert oracle_sweep.classify(0, "") is None
    assert oracle_sweep.classify(1, "something unexpected").startswith("exit 1")


def test_the_sound_edit_count_is_not_part_of_the_kind():
    one = "x panicked at a:1\ncall-effects: the result kept past 39 sound edits contradicts\n"
    two = "x panicked at a:1\ncall-effects: the result kept past 176 sound edits contradicts\n"
    assert oracle_sweep.classify(101, one) == oracle_sweep.classify(101, two)


def test_a_compiler_that_ignores_the_oracles_is_refused_and_each_oracle_named(tmp_path, capsys):
    """A sweep that finds nothing proves nothing where the oracles did not run: a stand-in compiler that does the same work with or
    without `LLRM_CHECK_*` must stop the sweep (exit 2) and name every oracle, not report a clean run."""
    for tool in ("llrm-qb", "llrm-nib"):
        script = tmp_path / tool
        script.write_text("#!/bin/sh\nexit 0\n")
        script.chmod(0o755)
    import sys

    sys.argv = ["oracle_sweep.py", "--bin", str(tmp_path), "--levels=-O2", "--machines=-m32"]
    assert oracle_sweep.main() == 2
    said = capsys.readouterr().out
    for name in oracle_sweep.CHECKS:
        assert name in said.split("never exercised:")[1], name


def test_a_finding_nothing_expects_fails_and_an_expected_one_that_stopped_failing_fails_too():
    """The sweep is red on main until #2193: the expected-failures list says so aloud; a new kind is news, a paid entry must be deleted."""
    expected = [{"match": "call-effects: the result kept past N sound edits", "fix": "#2193", "owner": "basicaa-port"}]
    old = {"call-effects: the result kept past N sound edits contradicts x": {"greeting.nib -O1 -m16"}}
    new, seen, unseen = oracle_sweep.judge(old, expected)
    assert (new, len(seen), unseen) == ([], 1, [])
    new, _, _ = oracle_sweep.judge({**old, "dataflow: something else": {"a.nib -O2 -m32"}}, expected)
    assert new == ["dataflow: something else"]
    _, _, unseen = oracle_sweep.judge({}, expected)
    assert unseen and "#2193" in unseen[0]


def test_a_slice_runs_only_its_programs_and_never_calls_an_expected_entry_paid(tmp_path, monkeypatch, capsys):
    """Three slices see different programs; an entry the other slices' programs show must not fail this one as 'paid'."""
    programs = [("llrm-nib", tmp_path / f"p{i}.nib") for i in range(6)]
    for _, path in programs:
        path.write_text("x")
    monkeypatch.setattr(oracle_sweep, "programs", lambda: programs)
    seen = []
    monkeypatch.setattr(oracle_sweep, "compile_one", lambda job, binaries: seen.append(job[1].name) or (job, None, {}))
    expected = tmp_path / "e.toml"
    expected.write_text('[[expected]]\nmatch = "x"\nfix = "#1"\nowner = "o"\n')
    import sys

    monkeypatch.setattr(sys, "argv", ["oracle_sweep.py", "--bin", str(tmp_path), "--no-premise", "--no-reach", "--shard", "2/3", "--levels=-O2", "--machines=-m32", "--expected", str(expected)])
    assert oracle_sweep.main() == 0
    assert sorted(seen) == ["p1.nib", "p4.nib"] and "wall " in capsys.readouterr().out


def test_a_new_finding_kind_turns_the_sweep_red_and_the_first_line_says_so(tmp_path, monkeypatch, capsys):
    """The step is red on a finding nothing expects: exit 1, and the census line leads the output."""
    path = tmp_path / "p.nib"
    path.write_text("x")
    monkeypatch.setattr(oracle_sweep, "programs", lambda: [("llrm-nib", path)])
    monkeypatch.setattr(oracle_sweep, "compile_one", lambda job, binaries: (job, "dataflow: a new kind of panic", {}))
    import sys

    monkeypatch.setattr(sys, "argv", ["oracle_sweep.py", "--bin", str(tmp_path), "--no-premise", "--no-reach", "--levels=-O2", "--machines=-m32"])
    assert oracle_sweep.main() == 1
    assert capsys.readouterr().out.splitlines()[0].startswith("1 compiles / 1 new finding kinds / 0 expected")


def test_an_oracle_whose_hook_was_never_reached_stops_the_sweep_naming_it(tmp_path, monkeypatch, capsys):
    """With the expected list empty, an oracle that did not run also reports no findings: the sweep must not read as clean. No perf is needed: the
    compiler counts, per oracle, the call sites asked while it was on (LLRM_DEBUG=checks)."""
    path = tmp_path / "p.nib"
    path.write_text("x")
    monkeypatch.setattr(oracle_sweep, "programs", lambda: [("llrm-nib", path)])
    everything = {name: 3 for name in oracle_sweep.CHECKS}
    monkeypatch.setattr(oracle_sweep, "compile_one", lambda job, binaries: (job, None, {k: v for k, v in everything.items() if k != "REPLAY"}))
    import sys

    monkeypatch.setattr(sys, "argv", ["oracle_sweep.py", "--bin", str(tmp_path), "--no-premise", "--levels=-O2", "--machines=-m32"])
    assert oracle_sweep.main() == 2
    assert "never reached: REPLAY" in capsys.readouterr().out
    monkeypatch.setattr(oracle_sweep, "compile_one", lambda job, binaries: (job, None, everything))
    assert oracle_sweep.main() == 0


def test_the_checks_lines_are_not_the_findings_last_line():
    """run_main prints `[checks] NAME N` after the compile's own message: a refusal in words must still read as one."""
    stderr = "llrm-nib: entry function 'main' does not exist\n[checks] REPLAY 3\n"
    assert oracle_sweep.classify(1, oracle_sweep.REACHED.sub("", stderr)) is None


def _premise(monkeypatch, tmp_path, reach, extra=None, control=100_000):
    """The premise step over a stand-in `instructions`: an oracle costs `extra[name]` (default 10) more than the control and reaches `reach[name]` (default 1) sites."""
    import sys

    path = tmp_path / "p.nib"
    path.write_text("x")
    monkeypatch.setattr(oracle_sweep, "programs", lambda: [("llrm-nib", path)])

    def fake(binaries, job, env_extra):
        name = next((key.removeprefix("LLRM_CHECK_") for key in env_extra if not key.endswith("X")), None)
        if name is None:
            return control, {}
        return control + (extra or {}).get(name, 10), {name: reach.get(name, 1)}

    monkeypatch.setattr(oracle_sweep, "instructions", fake)
    monkeypatch.setattr(sys, "argv", ["oracle_sweep.py", "--bin", str(tmp_path), "--premise-only"])


def test_an_oracle_that_costs_instructions_but_reached_no_hook_is_not_exercised(tmp_path, monkeypatch, capsys):
    """The old premise read only the cost: REPLAY showed +0.5% and 0 hook sites (its count was never reported), so a reach of zero must stay red."""
    _premise(monkeypatch, tmp_path, {"REPLAY": 0})
    assert oracle_sweep.main() == 2
    assert capsys.readouterr().out.split("never exercised:")[1].split("(")[0].strip() == "REPLAY"


def test_an_oracle_that_reached_its_hook_and_changed_nothing_is_not_exercised(tmp_path, monkeypatch, capsys):
    _premise(monkeypatch, tmp_path, {}, extra={"INTERN": 0})
    assert oracle_sweep.main() == 2
    assert capsys.readouterr().out.split("never exercised:")[1].split("(")[0].strip() == "INTERN"


def test_a_cheap_oracle_that_ran_is_exercised_and_the_old_bar_is_only_printed(tmp_path, monkeypatch, capsys):
    """INTERN costs +0.0085% on the largest programs (the old 0.05% bar failed it, #5663's gate red): reach and a positive cost pass it."""
    _premise(monkeypatch, tmp_path, {})
    assert oracle_sweep.main() == 0
    assert "under the old 0.05% bar" in capsys.readouterr().out


def test_every_checked_oracle_has_a_hook_in_the_source():
    """COALESCE stayed in the lists for ten days after its hook went with the old coalescer (902b535761): the premise could never pass."""
    source = "".join(path.read_text() for path in (oracle_sweep.ROOT / "crates").rglob("*.rs") if path.suffix == ".rs")
    for name in oracle_sweep.CHECKS:
        assert f"LLRM_CHECK_{name}" in source, name


def test_replay_reports_the_sites_that_asked_it(tmp_path):
    """LLRM_DEBUG=checks printed no `[checks] REPLAY` line though the oracle ran (+0.5% instructions): the premise read 0 sites for it."""
    import os
    import subprocess

    binary = oracle_sweep.llrmbin.target_dir(os.environ, oracle_sweep.llrmbin.REPO) / "release/llrm-qb"
    source = oracle_sweep.ROOT / "bench/bintree/bintree.bas"
    done = subprocess.run([str(binary), str(source), "-O2", "-m32", "-o", str(tmp_path / "x.o")], capture_output=True, text=True,
                          env={**os.environ, "LLRM_CHECK_REPLAY": "1", "LLRM_DEBUG": "checks"})
    assert oracle_sweep.REACHED.findall(done.stderr) and dict(oracle_sweep.REACHED.findall(done.stderr)).get("REPLAY", "0") != "0", done.stderr[-300:]
