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
