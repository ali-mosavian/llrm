//! What a block whose address is taken, or that an `indirectbr` ends, forbids.

use llrm_mir::module::GlobalKind;

use crate::testing::parsed;

/// Before: `SplitCriticalEdge` rewrote the `indirectbr`'s own destination, so
/// the block the address names was no longer the one the branch reached.
#[test]
fn test_an_edge_out_of_an_indirectbr_is_not_split() {
    let mut module = parsed(
        "@t = internal constant [2 x ptr] [ptr blockaddress(@f, %a), ptr blockaddress(@f, %b)]

define void @f(ptr %p) {
entry:
  indirectbr ptr %p, [label %a, label %b]

a:
  br label %b

b:
  ret void
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    let (entry, b) = (function.layout()[0], function.layout()[2]);
    assert!(crate::edges::is_critical_edge(function, entry, b));
    assert_eq!(crate::edges::split_critical_edge(function, entry, b), None);
}

/// Before: a callee with an address-taken block was inlined, and its label
/// named code in the caller (InlineCost.cpp:3328).
#[test]
fn test_a_callee_with_an_address_taken_block_is_not_inlinable() {
    let module = parsed(
        "@t = internal constant [1 x ptr] [ptr blockaddress(@g, %b)]

define void @g(ptr %p) {
entry:
  indirectbr ptr %p, [label %b]

b:
  ret void
}
",
    );
    let (id, global) = module
        .globals
        .iter()
        .enumerate()
        .find(|(_, global)| global.name.as_deref() == Some("g"))
        .map(|(at, global)| (llrm_mir::context::GlobalId(at as u32), global))
        .expect("@g");
    let GlobalKind::Function(callee) = &global.kind else { panic!("a function") };
    assert!(crate::inlinecost::inline_viable(&module, callee, id).is_err());
}
