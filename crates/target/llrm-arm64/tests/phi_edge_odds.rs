//! `lower_phis` splits a critical edge as the pin does (`SplitCriticalEdge`,
//! MachineBasicBlock.cpp#L1196): the new block lies right after its source, the
//! source's edge to it has the probability of the edge it replaced, and its one
//! edge has probability 1. The block odds are keyed by block, so they follow
//! the blocks when one is placed beside its source: a table keyed by position
//! gave every later block the odds of its neighbour, and a split edge none at
//! all.

use std::collections::BTreeMap;

use llrm_arm64::dag::emit::emit;
use llrm_arm64::dag::function::select_function;
use llrm_arm64::layout::edge_odds;
use llrm_arm64::mc::inst::{Inst, Target};
use llrm_arm64::mfunc::{MBlock, MFunc, Op, lower_phis};
use llrm_mir::blockfrequency::BranchProbability;
use llrm_mir::datalayout::DataLayout;

/// `%k` is read by `head`'s phi and by `other`: live out of `entry` past the
/// phis, and not live into `head`, which is the edge the pin splits.
const IR: &str = "target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\"\n\
define i64 @f(i64 %n, i1 %c) {\n\
entry:\n  %k = mul i64 %n, 3\n  br i1 %c, label %head, label %other\n\
head:\n  %i = phi i64 [ %k, %entry ], [ %next, %head ]\n  %next = add i64 %i, 1\n  %done = icmp eq i64 %next, %n\n  br i1 %done, label %join, label %head\n\
other:\n  %q = add i64 %k, 1\n  br label %join\n\
join:\n  %r = phi i64 [ %next, %head ], [ %q, %other ]\n  ret i64 %r\n}";

/// The same function where every phi value is read only by its phi: a copy at
/// the end of `entry` would be the value's kill.
const IR_DIES: &str = "target datalayout = \"e-m:o-i64:64-i128:128-n32:64-S128\"\n\
define i64 @f(i64 %n, i1 %c) {\n\
entry:\n  br i1 %c, label %head, label %join\n\
head:\n  %i = phi i64 [ 0, %entry ], [ %next, %head ]\n  %next = add i64 %i, 1\n  %done = icmp eq i64 %next, %n\n  br i1 %done, label %join, label %head\n\
join:\n  %r = phi i64 [ 7, %entry ], [ %next, %head ]\n  ret i64 %r\n}";

type Edges = BTreeMap<(String, String), BranchProbability>;

fn branches(block: &MBlock) -> Vec<String> {
    block
        .ops
        .iter()
        .filter_map(|op| match op {
            Op::I(
                Inst::Branch { target: Target::Label(label), .. }
                | Inst::BranchCond { target: Target::Label(label), .. }
                | Inst::CompareBranch { target: Target::Label(label), .. }
                | Inst::TestBranch { target: Target::Label(label), .. },
            ) => Some(label.clone()),
            _ => None,
        })
        .collect()
}

/// The odds, by the labels of the blocks they join.
fn by_label(
    machine: &MFunc,
    module: &llrm_mir::module::Module,
) -> Edges {
    let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    edge_odds(module, function, &layout, machine)
        .into_iter()
        .map(|((from, to), (probability, _))| {
            ((machine.blocks[from as usize].label.clone(), machine.blocks[to as usize].label.clone()), probability)
        })
        .collect()
}

#[test]
fn a_split_edge_keeps_its_probability_and_the_odds_follow_the_blocks() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let mut machine = emit("f", &select_function(&module, "f").expect("selects"), false).expect("emits");
    let before = by_label(&machine, &module);
    assert!(before.len() >= 3, "the premise: the IR's edges have odds: {before:?}");
    let labels: Vec<String> = machine.blocks.iter().map(|block| block.label.clone()).collect();
    lower_phis(&mut machine).expect("lowers");
    let after = by_label(&machine, &module);
    let added: Vec<usize> =
        (0..machine.blocks.len()).filter(|&at| !labels.contains(&machine.blocks[at].label)).collect();
    assert_eq!(
        added.len(),
        1,
        "the premise: only the edge into `head` is split: {:?}",
        machine.blocks.iter().map(|b| &b.label).collect::<Vec<_>>()
    );
    let mut replaced = 0;
    for &at in &added {
        let edge = &machine.blocks[at].label;
        let (source, source_at) = machine
            .blocks
            .iter()
            .enumerate()
            .find(|(_, block)| branches(block).contains(edge))
            .map(|(source_at, block)| (block.label.clone(), source_at))
            .expect("a block branches to the edge block");
        let [target] = &branches(&machine.blocks[at])[..] else { panic!("an edge block has one branch") };
        let target = target.clone();
        // Right after the source, or after another edge block of its own.
        assert!(
            (source_at + 1..at).all(|between| added.contains(&between)),
            "{edge} is not beside its source {source}: {:?}",
            machine.blocks.iter().map(|b| &b.label).collect::<Vec<_>>()
        );
        assert_eq!(
            after.get(&(source.clone(), edge.clone())),
            before.get(&(source.clone(), target.clone())),
            "{source} -> {edge}"
        );
        assert_eq!(
            after.get(&(edge.clone(), target.clone())),
            Some(&BranchProbability::get_one()),
            "{edge} -> {target}"
        );
        replaced += 1;
    }
    assert_eq!(replaced, added.len());
    // The edges that were not split keep theirs.
    for ((from, to), probability) in &before {
        if !added.iter().any(|&at| {
            let edge = &machine.blocks[at].label;
            after.contains_key(&(from.clone(), edge.clone())) && after.contains_key(&(edge.clone(), to.clone()))
        }) {
            assert_eq!(after.get(&(from.clone(), to.clone())), Some(probability), "{from} -> {to}");
        }
    }
}

/// A pass that puts a block in front of others (or takes one out) must not
/// move the odds of the blocks it passes. The selector's ids were a list in
/// block order that no pass kept in step: each later block took its
/// neighbour's id, and bench `jpeg_single` -O1 allocated with the wrong block
/// frequencies (143 stack reloads in one function against 69 before the
/// placement change and the pin's 50).
#[test]
fn the_odds_still_follow_the_blocks_when_a_pass_inserts_one() {
    let module = llrm_mir::parse::module(IR).expect("parses");
    let mut machine = emit("f", &select_function(&module, "f").expect("selects"), false).expect("emits");
    lower_phis(&mut machine).expect("lowers");
    let before = by_label(&machine, &module);
    assert!(before.len() >= 4, "the premise: the function has odds to move: {before:?}");
    machine.blocks.insert(1, MBlock { label: ".Lf.inserted".into(), ops: Vec::new().into() });
    assert_eq!(by_label(&machine, &module), before);
}

/// A phi value only its phi reads dies in the copy at the end of the
/// predecessor: `SplitPHIEdges` leaves the edge (`isLiveOutPastPHIs`), so no
/// block is made. Every critical edge was split, 342 blocks in jpeg_single
/// -O1 where the pin has 260.
#[test]
fn an_edge_whose_value_only_the_phi_reads_is_not_split() {
    let module = llrm_mir::parse::module(IR_DIES).expect("parses");
    let mut machine = emit("f", &select_function(&module, "f").expect("selects"), false).expect("emits");
    let blocks = machine.blocks.len();
    lower_phis(&mut machine).expect("lowers");
    assert_eq!(machine.blocks.len(), blocks, "{:?}", machine.blocks.iter().map(|b| &b.label).collect::<Vec<_>>());
}

/// `fold` reads its odds by the number a block's label ends in, but the table
/// was keyed by position, which moved once `lower_phis` put each edge block
/// beside its source: a switch tree (`tests/programs/dispatch.c`, -O2) was laid
/// out by the odds of other blocks and kept two `add; ret` tails and a jump
/// (141 instructions where release/next has 139; the pin's `opt -O2 | llc -O2`
/// on the same IR: 141).
#[test]
fn the_layout_reads_the_odds_of_the_blocks_it_places() {
    let source = include_str!("data/dispatch_switch.ll");
    let asm = llrm_arm64::driver::assembly(source, llrm_arm64::machine::Platform::Linux, llrm_arm64::driver::Level::O2)
        .expect("lowers");
    let instructions = asm.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert_eq!(instructions, 139);
}
