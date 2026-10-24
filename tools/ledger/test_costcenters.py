import pytest

import costcenters


def test_generic_arguments_do_not_pick_the_center():
    """`Vec<Rc<T>>::push` is Vec growth, not Rc: the first table put every
    function generic over an Rc or a hasher in that center (Hasher 58 M, really 1.7)."""
    assert costcenters.center_of("<alloc::vec::Vec<alloc::rc::Rc<T>>>::reserve_for_push", False) == "Vec / SmallVector growth"
    assert costcenters.center_of("llrm_mir::loopinfo::analyze::<alloc::rc::Rc<DomTree>>", False) == "other"
    assert costcenters.center_of("<indexmap::IndexMap<K, V, BuildHasherDefault<rustc_hash::FxHasher>>>::get", False) == "other"


def test_a_symbol_has_one_center_and_the_centers_sum_to_the_total():
    costs = {"malloc": 5, "memcpy": 7, "llvm::DenseMapBase<A, B>::find(int)": 3, "main": 85}
    ours = costcenters.by_center({"malloc": 5, "__memcpy_avx": 7, "main": 88}, 100, False)
    assert sum(ours.values()) == 100 and ours["malloc / free / realloc"] == 5
    pin = costcenters.by_center({k: v for k, v in costs.items() if k != "memcpy"}, 93, True)
    assert pin["hash table probing (hashbrown / DenseMap, StringMap)"] == 3


def test_lost_lines_are_refused():
    with pytest.raises(AssertionError, match="centers sum to"):
        costcenters.by_center({"main": 10}, 1000, False)
