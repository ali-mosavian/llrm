import pytest

import nested


def test_a_wrapper_listed_with_its_part_is_refused():
    """Together (Sroa + Mem2Reg in one slot) was listed beside Sroa: 9.3 M twice."""
    edges = {"Together::run": {"Sroa::run", "Mem2Reg::run"}, "Sroa::run": {"alloc"}}
    with pytest.raises(SystemExit, match="Together::run calls Sroa::run"):
        nested.check(edges, {"scalar": {"Together::run", "Sroa::run", "Gvn::run"}})


def test_a_call_through_an_unlisted_function_is_still_nested():
    edges = {"A": {"glue"}, "glue": {"B"}}
    assert nested.nested_pairs(edges, {"A", "B"}) == [("A", "B")]


def test_unrelated_and_recursive_functions_pass():
    edges = {"A": {"A", "x"}, "B": {"x"}}
    assert nested.check(edges, {"one": {"A"}, "two": {"B"}}) == "double-count check: 0 overlaps over 2 buckets"


def test_a_function_in_two_buckets_is_refused():
    with pytest.raises(SystemExit, match="'one' and 'two'"):
        nested.check({}, {"one": {"A"}, "two": {"A"}})


def test_a_wrapper_in_one_bucket_over_a_part_in_another_is_refused():
    edges = {"Together::run": {"Sroa::run"}}
    with pytest.raises(SystemExit, match="Together::run calls Sroa::run"):
        nested.check(edges, {"scalar": {"Together::run"}, "other": {"Sroa::run"}})


def test_the_calling_tree_is_parsed(tmp_path):
    text = (
        "     900 ( 9.0%)  *  ???:W [/bin/x]\n"
        "     600 ( 6.0%)  >   ???:S (2x) [/bin/x]\n"
        "     600 ( 6.0%)  *  ???:S [/bin/x]\n"
    )
    path = tmp_path / "t.calling"
    path.write_text(text)
    assert nested.parse_calling(path) == {"W": {"S"}, "S": set()}


def test_a_nested_pair_on_the_pin_side_is_refused(tmp_path):
    """The pin's names carry a file prefix; the call graph and the bucket lists
    must clean them the same way, or a wrapper over a part goes unseen."""
    text = (
        "     900 ( 9.0%)  *  /llvm/lib/Passes/A.cpp:llvm::WrapperPass::run(int) [/bin/clang]\n"
        "     600 ( 6.0%)  >   /llvm/lib/Passes/B.cpp:llvm::PartPass::run(int) (2x) [/bin/clang]\n"
    )
    path = tmp_path / "pin.calling"
    path.write_text(text)
    edges = nested.parse_calling(path, pin=True)
    assert edges == {"llvm::WrapperPass::run(int)": {"llvm::PartPass::run(int)"}}
    with pytest.raises(SystemExit, match="WrapperPass::run.int. calls llvm::PartPass"):
        nested.check(edges, {"scalar": {"llvm::WrapperPass::run(int)", "llvm::PartPass::run(int)"}})
