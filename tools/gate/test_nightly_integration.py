import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))

import nightly_integration as n  # noqa: E402

SUMMARIES = (
    "     Running tests/attribute.rs (target/release/deps/attribute-1f)\n"
    "running 7 tests\ntest test_always_inline_replaces_every_call_of_a_big_routine ... FAILED\ntest other ... ok\n"
    "failures:\n    test_always_inline_replaces_every_call_of_a_big_routine\n\n"
    "test result: FAILED. 6 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s\n"
    "     Running tests/header_copy.rs (target/release/deps/header_copy-2a)\n"
    "test test_fib_with_the_header_copy_on_keeps_its_self_inlined_nest ... FAILED\n"
    "test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
)


def test_a_red_integration_test_is_named_by_its_target_and_a_summary_line_is_not_a_test():
    """94 'red' integration tests were 63: the `test result: FAILED` line of each binary was counted with the failing tests."""
    assert n.red_tests(SUMMARIES) == [
        "integration attribute::test_always_inline_replaces_every_call_of_a_big_routine",
        "integration header_copy::test_fib_with_the_header_copy_on_keeps_its_self_inlined_nest",
    ]
    assert n.red_tests("test x ... ok\ntest result: FAILED. 0 passed; 1 failed\n") == []


def test_a_red_nightly_test_names_the_prs_of_the_window_that_touch_what_it_reaches():
    """The G1 ruling keeps most root targets nightly: the morning report names the PR and its owner so the coordinator can route a red one."""
    reach = {"attribute": {"llrm-codegen", "llrm-mir"}, "header_copy": {"llrm-transforms"}}
    landed = [("a" * 40, ["crates/codegen/llrm-codegen/src/x.rs"]), ("b" * 40, ["docs/x.md"]), ("c" * 40, ["tests/header_copy.rs"])]
    crate_of = lambda path: "llrm-codegen" if "llrm-codegen" in path else None  # noqa: E731
    owners = {"a" * 40: ("3901", "w-3"), "c" * 40: ("3902", "w-8")}
    lines = n.attribution(
        ["integration attribute::t", "integration header_copy::t", "integration other::t"], reach, landed, crate_of, lambda sha: owners.get(sha, ("?", "?"))
    )
    assert lines == [
        "NIGHTLY RED integration attribute::t: #3901 (owner w-3)",
        "NIGHTLY RED integration header_copy::t: #3902 (owner w-8)",
        "NIGHTLY RED integration other::t: no PR of the window touches it: older",
    ]
