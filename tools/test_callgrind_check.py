"""callgrind-check.py finds a callee a profile shows under a caller whose source never spells it."""
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("callgrind_check", Path(__file__).resolve().parent / "callgrind-check.py")
cc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cc)

SOURCES = {
    "/w/crates/opt/tails.rs": ["fn tails_merged() {", "    merge(a, b);", "}"],
    "/w/crates/opt/dce.rs": ["fn eliminated() {}", "fn merge() {}"],
}


def profile(callee: str, callee_file: str) -> str:
    return (
        "fl=/w/crates/opt/tails.rs\nfn=tails_merged\n2 10\n"
        f"cfi={callee_file}\ncfn={callee}\ncalls=3 1\n2 500\n"
    )


def read(path):
    return SOURCES[path]


def test_a_callee_the_caller_never_spells_is_reported():
    """machinedce::eliminated showed under tails_merged on parity 59c834686: a split nobody could tell from a real one."""
    bad = cc.check(profile("eliminated", "/w/crates/opt/dce.rs"), read)
    assert [(row[0], row[2], row[3]) for row in bad] == [("tails_merged (tails.rs:2)", "eliminated", 3)]


def test_a_callee_the_caller_spells_passes():
    assert cc.check(profile("merge", "/w/crates/opt/dce.rs"), read) == []


def test_closures_std_and_unknown_lines_are_not_checked():
    for callee in ("tails_merged::{closure#0}", "core::ptr::drop_glue::<X>", "<alloc::vec::Vec<u8>>::push"):
        assert cc.check(profile(callee, "/w/crates/opt/dce.rs"), read) == []
    zero = profile("eliminated", "/w/crates/opt/dce.rs").replace("\n2 500", "\n0 500")
    assert cc.check(zero, read) == []


def test_short_names_a_call_site_spells():
    assert cc.short("<llrm_analysis::scev::ScalarEvolution>::get_range_ref") == "get_range_ref"
    assert cc.short("llrm_transforms::gvn::table::lookup'2") == "lookup"
    assert cc.short("<a::B as c::D>::run::<T>") == "run"
