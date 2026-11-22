//! `llvm.global_ctors` is an `.init_array` of the functions, lowest priority
//! first (`emitXXStructorList`): QuickrBasic `pfil.c` and `psys.c` were
//! refused "constructors and destructors are not supported".

use llrm_m68k::driver::{Level, assembly, cpu_of};

#[test]
fn constructors_are_an_init_array_in_priority_order() {
    let source = include_str!("../data/global_ctors.ll");
    let text = assembly(source, cpu_of("68000").unwrap(), Level::O2).unwrap_or_else(|why| panic!("{why}"));
    let table = text.split(".init_array").nth(1).unwrap_or_else(|| panic!("no .init_array:\n{text}"));
    let (early, late) = (table.find(".long\tearly").unwrap(), table.find(".long\tlate").unwrap());
    assert!(early < late, "priority 101 must come before 65535:\n{table}");
}
