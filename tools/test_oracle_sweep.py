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
