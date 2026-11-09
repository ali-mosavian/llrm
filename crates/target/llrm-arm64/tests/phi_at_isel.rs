//! Selection makes phis (`PHI`, as the pin's ISel does) and `lower_phis` turns
//! them into copies. Before, both selectors copied on the edges while
//! selecting, so no shared SSA phase could run on arm64: 184 registers of the
//! -O1 corpus were defined more than once, all by copies.

use llrm_arm64::dag::emit::emit;
use llrm_arm64::dag::function::select_function;
use llrm_arm64::machine::Platform;
use llrm_arm64::mfunc::{MFunc, Op, lower_phis};

const DATALAYOUT: &str = "e-m:o-i64:64-i128:128-n32:64-S128";

/// A counted loop whose counter is carried by a phi, and a join with a
/// constant on one edge and a critical edge into it.
const LOOP: &str = "define i64 @f(i64 %n, i1 %c) {\n\
entry:\n  br i1 %c, label %head, label %join\n\
head:\n  %i = phi i64 [ 0, %entry ], [ %next, %head ]\n  %next = add i64 %i, 1\n  %done = icmp eq i64 %next, %n\n  br i1 %done, label %join, label %head\n\
join:\n  %r = phi i64 [ 7, %entry ], [ %next, %head ]\n  ret i64 %r\n}";

fn module() -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{LOOP}")).expect("parses")
}

fn by_dag() -> MFunc {
    emit("f", &select_function(&module(), "f").expect("selects"), false).expect("emits")
}

fn by_hand() -> MFunc {
    let module = module();
    let layout = llrm_mir::datalayout::DataLayout::parse(DATALAYOUT).unwrap();
    let (id, ..) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a function");
    llrm_arm64::isel::selected(&module, id, &layout, Platform::Darwin, false).expect("selects")
}

fn phis(function: &MFunc) -> Vec<(usize, Vec<String>)> {
    function
        .blocks
        .iter()
        .enumerate()
        .flat_map(|(at, block)| {
            block
                .ops
                .iter()
                .filter_map(
                    move |op| match op {
                        Op::Phi { incoming, .. } => Some((at, incoming.iter().map(|(_, from)| from.clone()).collect())),
                        _ => None,
                    },
                )
        })
        .collect()
}

/// The counter of a loop is a phi of the header with the entry and the latch
/// as predecessors, ahead of the block's other code.
fn assert_selects_the_loop_phis(function: &MFunc) {
    let found = phis(function);
    let head = &function.blocks[1];
    assert!(matches!(head.ops[0], Op::Phi { .. }), "the header starts with a phi: {head:?}");
    assert!(found.contains(&(1, vec![".Lf.0".into(), ".Lf.1".into()])), "{found:?}");
    assert!(found.iter().any(|(at, _)| *at == 2), "the join has a phi: {found:?}");
    for phi in function.blocks.iter().flat_map(|block| &block.ops).filter(|op| matches!(op, Op::Phi { .. })) {
        let Op::Phi { incoming, .. } = phi else { unreachable!() };
        assert!(incoming.iter().all(|(value, _)| value.is_virtual()), "a phi names registers: {phi:?}");
    }
}

#[test]
fn the_dag_selector_makes_phis_where_a_loop_carries_a_value() {
    assert_selects_the_loop_phis(&by_dag());
}

#[test]
fn the_hand_selector_makes_phis_where_a_loop_carries_a_value() {
    assert_selects_the_loop_phis(&by_hand());
}

/// No phi is left, every phi register has one definition per path that
/// reaches the block, and the edge from the entry to the join (the entry
/// branches two ways) has a block of its own for its copies.
fn assert_lowered(mut function: MFunc) {
    lower_phis(&mut function).expect("lowers");
    assert_eq!(phis(&function), vec![], "{function:?}");
    assert_eq!(llrm_arm64::mfunc::undefined_reads(&function), vec![], "{function:?}");
    let edges: Vec<_> = function.blocks.iter().filter(|block| block.label.contains(".e")).collect();
    assert!(!edges.is_empty(), "a critical edge keeps its copies in a block of its own: {function:?}");
    let last = edges[0].ops.last().expect("the edge ends in a branch");
    assert!(matches!(last, Op::I(llrm_arm64::mc::inst::Inst::Branch { .. })), "{edges:?}");
}

#[test]
fn lowering_the_dag_selectors_phis_leaves_copies_and_an_edge_block() {
    assert_lowered(by_dag());
}

#[test]
fn lowering_the_hand_selectors_phis_leaves_copies_and_an_edge_block() {
    assert_lowered(by_hand());
}

/// A `switch` of five cases is a tree of blocks the selector makes, and the
/// default is branched to from its leaves, not from the block the `switch`
/// is in: a phi value recorded for that block alone left the phi unwritten on
/// the paths that reached the default (`dispatch` printed `7 0 -6` for
/// `0 0 0` under `ARM64_ISEL=hand`).
#[test]
fn a_switch_tree_gives_the_default_phi_value_from_each_leaf_that_branches_to_it() {
    let ir = "define i64 @f(i64 %x) {\n\
entry:\n  switch i64 %x, label %join [ i64 1, label %a\n i64 2, label %b\n i64 3, label %c\n i64 4, label %d\n i64 5, label %e ]\n\
a:\n  br label %join\nb:\n  br label %join\nc:\n  br label %join\nd:\n  br label %join\ne:\n  br label %join\n\
join:\n  %r = phi i64 [ 9, %entry ], [ 1, %a ], [ 2, %b ], [ 3, %c ], [ 4, %d ], [ 5, %e ]\n  ret i64 %r\n}";
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    let layout = llrm_mir::datalayout::DataLayout::parse(DATALAYOUT).unwrap();
    let (id, ..) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("a function");
    let mut function = llrm_arm64::isel::selected(&module, id, &layout, Platform::Darwin, false).expect("selects");
    lower_phis(&mut function).expect("lowers");
    assert_eq!(llrm_arm64::mfunc::undefined_reads(&function), vec![], "{function:?}");
}
