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

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::printed;
use crate::twoentryphi::Speculation;

/// @f simplified under `options`.
fn simplify_with(
    options: &Options,
    text: &str,
) -> String {
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified_with(options, context, &spec, &mut declared, function);
    printed(&module)
}

/// Before: the arms' common `add` was hoisted out of the arm whose address a
/// table holds, and a jump to it by address skipped the add
/// (SimplifyCFG.cpp:1891).
#[test]
fn test_common_code_is_not_hoisted_out_of_a_block_whose_address_is_taken() {
    let options = Options { speculate_blocks: false, hoist_common_insts: true, ..Default::default() };
    let out = simplify_with(
        &options,
        "@t = internal constant [1 x ptr] [ptr blockaddress(@f, %a)]

define i16 @f(i1 %c, i16 %x) {
entry:
  br i1 %c, label %a, label %b

a:
  %p = add i16 %x, 1
  br label %m

b:
  %q = add i16 %x, 1
  br label %m

m:
  %r = phi i16 [ %p, %a ], [ %q, %b ]
  ret i16 %r
}
",
    );
    assert!(out.contains("\na:\n  %p = add"), "{out}");
}

/// Before: the arm was speculated into a select and its label named nothing
/// (SimplifyCFG.cpp:3977).
#[test]
fn test_an_arm_whose_address_is_taken_is_not_speculated() {
    let out = simplify_with(
        &Options::default(),
        "@t = internal constant [1 x ptr] [ptr blockaddress(@f, %a)]

define i16 @f(i1 %c, i16 %x) {
entry:
  br i1 %c, label %a, label %b

a:
  %p = add i16 %x, 1
  br label %m

b:
  %q = add i16 %x, 2
  br label %m

m:
  %r = phi i16 [ %p, %a ], [ %q, %b ]
  ret i16 %r
}
",
    );
    assert!(out.contains("\na:\n") && !out.contains("select"), "{out}");
}

/// Before: two jump-only blocks to one target were one block though the
/// address of one was taken (SimplifyCFG.cpp:8388).
#[test]
fn test_a_jump_block_whose_address_is_taken_is_not_a_candidate_to_merge() {
    let module = parsed(
        "@t = internal constant [1 x ptr] [ptr blockaddress(@f, %a)]

define void @f(i1 %c) {
entry:
  br i1 %c, label %a, label %b

a:
  br label %m

b:
  br label %m

m:
  ret void
}
",
    );
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f");
    let (a, b) = (function.layout()[1], function.layout()[2]);
    assert!(!crate::sinkcommon::can_be_merged(function, a));
    assert!(crate::sinkcommon::can_be_merged(function, b));
}
