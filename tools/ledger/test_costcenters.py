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


def test_a_symbol_matching_two_centers_is_refused_not_first_wins():
    """RandomState::hash_one and sip's Hasher::write matched both the SipHash and
    the Fx center; first-wins gave them to SipHash without saying so."""
    both = "<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&i64>"
    pattern = costcenters.CENTERS[0]
    costcenters.CENTERS.insert(1, ("a second center", r"BuildHasher", r"$^"))
    try:
        with pytest.raises(ValueError, match="2 centers"):
            costcenters.center_of(both, False)
    finally:
        costcenters.CENTERS.remove(("a second center", r"BuildHasher", r"$^"))
    assert costcenters.center_of(both, False) == pattern[0]


def test_the_real_symbols_of_both_profiles_are_in_one_center():
    for symbol in (
        "<core::hash::sip::Hasher<core::hash::sip::Sip13Rounds> as core::hash::Hasher>::write",
        "<std::hash::random::RandomState as core::hash::BuildHasher>::hash_one::<&str>",
    ):
        assert costcenters.center_of(symbol, False).startswith("SipHash")
    assert costcenters.center_of("llvm::MCSymbol::operator new(unsigned long, llvm::StringMapEntry<X>&)", True) == "malloc / free / realloc"


def test_the_memcpyopt_pass_is_not_the_memcpy_center():
    """The first table took `llrm_transforms::memcpyopt` for libc memcpy and
    reported `Slot::run` as the top memcpy caller (10.9 M, really the pass's own
    inclusive cost): a Function copy per slot that does not exist."""
    assert costcenters.center_of("<llrm_transforms::memcpyopt::MemCpyOpt as llrm_mir::passes::FunctionPass>::run", False) == "other"
    assert costcenters.center_of("./string/../sysdeps/x86_64/multiarch/memmove-vec-unaligned-erms.S:__memcpy_avx_unaligned_erms", False).startswith("memcpy")
    assert costcenters.center_of("__memset_avx2_unaligned_erms", True).startswith("memcpy")
