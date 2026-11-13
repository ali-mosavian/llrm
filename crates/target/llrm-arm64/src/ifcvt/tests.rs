//! Early if-conversion on hand-built bodies: what converts, what stays a
//! branch, and the select it makes.

use super::*;
use crate::mc::inst::{Addr, Memory};
use crate::mfunc::{MBlock, OpList};

fn v(n: u32) -> Gpr {
    Gpr(64 + n)
}

fn block(
    label: &str,
    ops: Vec<Op>,
) -> MBlock {
    MBlock { label: label.to_owned(), ops: OpList::from(ops) }
}

fn cmp(rn: Gpr) -> Op {
    Op::I(Inst::AddSubImm {
        op: AddSub { subtract: true, set_flags: true },
        width: Width::X,
        rd: Gpr(31),
        rn,
        imm12: 5,
        shift12: false,
    })
}

fn add(
    rd: Gpr,
    rn: Gpr,
    imm12: u16,
) -> Op {
    Op::I(Inst::AddSubImm {
        op: AddSub { subtract: false, set_flags: false },
        width: Width::X,
        rd,
        rn,
        imm12,
        shift12: false,
    })
}

fn branch_if(
    cond: Cond,
    label: &str,
) -> Op {
    Op::I(Inst::BranchCond { cond, target: Target::Label(label.to_owned()) })
}

fn jump(label: &str) -> Op {
    crate::mfunc::branch_to(label)
}

fn phi(
    dst: Gpr,
    from: &[(Gpr, &str)],
) -> Op {
    Op::Phi { dst: Reg::Gpr(dst), incoming: from.iter().map(|(r, l)| (Reg::Gpr(*r), (*l).to_owned())).collect() }
}

fn function(blocks: Vec<MBlock>) -> MFunc {
    let mut f = MFunc::new("f");
    f.next_register = 64 + 32;
    for (n, block) in blocks.iter().enumerate() {
        f.block_ids.insert(block.label.clone(), n as u32);
    }
    f.blocks = blocks;
    f
}

/// `if (x > 5) y = x + 2; return y` shaped: head compares and branches over
/// `then`, which adds.
fn triangle(then_ops: Vec<Op>) -> MFunc {
    function(vec![
        block("head", vec![
            Op::Copy { dst: Reg::Gpr(v(1)), src: Reg::Gpr(Gpr(0)) },
            cmp(v(1)),
            branch_if(Cond::Hi, "tail"),
            jump("then"),
        ]),
        block("then", then_ops.into_iter().chain([jump("tail")]).collect()),
        block("tail", vec![phi(v(4), &[(v(1), "head"), (v(3), "then")]), Op::Return { uses: vec![], tail: None }]),
    ])
}

fn has_select(f: &MFunc) -> bool {
    f.blocks.iter().flat_map(|b| b.ops.iter()).any(|op| matches!(op, Op::I(Inst::CondSelect { .. })))
}

fn has_branch(f: &MFunc) -> bool {
    f.blocks.iter().flat_map(|b| b.ops.iter()).any(|op| matches!(op, Op::I(Inst::BranchCond { .. })))
}

/// A branch around one add stayed a branch and a phi: the select a phi at the
/// join turns into was never made.
#[test]
fn a_triangle_around_an_add_becomes_a_select_and_one_block() {
    let mut f = triangle(vec![add(v(3), v(1), 2)]);
    assert_eq!(run(&mut f, Mode::Stress), 1);
    assert_eq!(f.blocks.len(), 1, "the arm and the join go into the head");
    assert!(has_select(&f) && !has_branch(&f));
    assert!(!f.blocks[0].ops.iter().any(|op| matches!(op, Op::Phi { .. })));
}

/// The select reads the branch's own condition, with the then-value first.
#[test]
fn the_select_takes_the_branch_condition() {
    let mut f = triangle(vec![add(v(3), v(1), 2)]);
    run(&mut f, Mode::Stress);
    let select = f.blocks[0].ops.iter().find_map(|op| match op {
        Op::I(Inst::CondSelect { cond, rn, rm, .. }) => Some((*cond, *rn, *rm)),
        _ => None,
    });
    // `b.hi tail`: the head value (v1) when hi, the arm's (v3) otherwise.
    assert_eq!(select, Some((Cond::Hi, v(1), v(3))));
}

/// The arm's operations run ahead of the compare, so the flags the select
/// reads are still the compare's: an arm that sets flags came after it and
/// the select read the arm's.
#[test]
fn an_arm_that_sets_flags_runs_before_the_compare() {
    let setter = Op::I(Inst::AddSubImm {
        op: AddSub { subtract: true, set_flags: true },
        width: Width::X,
        rd: v(3),
        rn: v(1),
        imm12: 2,
        shift12: false,
    });
    let mut f = triangle(vec![setter]);
    assert_eq!(run(&mut f, Mode::Stress), 1);
    let ops = &f.blocks[0].ops;
    let arm = ops.iter().position(|op| matches!(op, Op::I(Inst::AddSubImm { rd, .. }) if *rd == v(3))).unwrap();
    let compare = ops.iter().position(|op| matches!(op, Op::I(Inst::AddSubImm { rd, .. }) if *rd == Gpr(31))).unwrap();
    let select = ops.iter().position(|op| matches!(op, Op::I(Inst::CondSelect { .. }))).unwrap();
    assert!(arm < compare && compare < select, "arm {arm}, compare {compare}, select {select}");
}

/// A load is never speculated: it may fault on the path that did not want it.
#[test]
fn an_arm_with_a_load_stays_a_branch() {
    let load = Op::I(Inst::Load {
        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: v(3) },
        addr: Addr::Offset { base: v(1), imm: 0 },
        mmo: Default::default(),
    });
    let mut f = triangle(vec![load]);
    assert_eq!(run(&mut f, Mode::Stress), 0);
    assert_eq!(f.blocks.len(), 3);
    assert!(has_branch(&f) && !has_select(&f));
}

/// A store is never speculated: its effect would happen on both paths.
#[test]
fn an_arm_with_a_store_stays_a_branch() {
    let store = Op::I(Inst::Store {
        mem: Memory::Int { width: Width::X, bytes: 8, sign: false, reg: v(1) },
        addr: Addr::Offset { base: v(1), imm: 0 },
        mmo: Default::default(),
    });
    let mut f = triangle(vec![store, add(v(3), v(1), 2)]);
    assert_eq!(run(&mut f, Mode::Stress), 0);
    assert!(has_branch(&f));
}

/// More than the pin's `early-ifcvt-limit` operations in an arm is not
/// speculated, in any mode but the pin's own stress mode (which this one
/// keeps: the limit stays).
#[test]
fn an_arm_longer_than_the_limit_stays_a_branch() {
    let mut ops = vec![add(v(10), v(1), 1)];
    for n in 1..=BLOCK_INSTR_LIMIT as u32 {
        ops.push(add(v(10 + n), v(9 + n), 1));
    }
    ops.push(add(v(3), v(10 + BLOCK_INSTR_LIMIT as u32), 1));
    let mut f = triangle(ops);
    assert_eq!(run(&mut f, Mode::Stress), 0);
    assert!(has_branch(&f));
}

/// Both inputs of the phi are one value: the join is a copy, not a select.
#[test]
fn a_phi_of_one_value_is_a_copy() {
    let mut f = function(vec![
        block("head", vec![
            Op::Copy { dst: Reg::Gpr(v(1)), src: Reg::Gpr(Gpr(0)) },
            cmp(v(1)),
            branch_if(Cond::Hi, "tail"),
            jump("then"),
        ]),
        block("then", vec![add(v(3), v(1), 2), jump("tail")]),
        block("tail", vec![phi(v(4), &[(v(1), "head"), (v(1), "then")]), Op::Return { uses: vec![], tail: None }]),
    ]);
    assert_eq!(run(&mut f, Mode::Stress), 1);
    assert!(!has_select(&f), "{:?}", f.blocks[0].ops);
    assert!(f.blocks[0].ops.iter().any(|op| matches!(op, Op::Copy { dst, .. } if *dst == Reg::Gpr(v(4)))));
}

/// A join with a third predecessor keeps its phi and takes the select as one
/// input: the other path still reaches it.
#[test]
fn a_join_with_another_predecessor_keeps_the_phi_over_the_select() {
    let mut f = function(vec![
        block("entry", vec![
            Op::Copy { dst: Reg::Gpr(v(1)), src: Reg::Gpr(Gpr(0)) },
            cmp(v(1)),
            branch_if(Cond::Eq, "tail"),
            jump("head"),
        ]),
        block("head", vec![cmp(v(1)), branch_if(Cond::Hi, "tail"), jump("then")]),
        block("then", vec![add(v(3), v(1), 2), jump("tail")]),
        block("tail", vec![
            phi(v(4), &[(v(1), "entry"), (v(1), "head"), (v(3), "then")]),
            Op::Return { uses: vec![], tail: None },
        ]),
    ]);
    // One conversion, by hand: `head` (block 1) around `then`.
    let mut cfg = Cfg::of(&f);
    let mut defs = Defs::of(&f);
    let candidate = IfConv::can_convert(&f, &cfg, 1, &defs).expect("the triangle converts");
    candidate.convert(&mut f, &mut cfg, &mut defs);
    let tail = f.blocks.iter().find(|b| b.label == "tail").expect("the join stays");
    let Op::Phi { incoming, .. } = &tail.ops[0] else { panic!("the join keeps its phi: {:?}", tail.ops) };
    let from: Vec<&str> = incoming.iter().map(|(_, label)| label.as_str()).collect();
    assert_eq!(from, ["entry", "head"], "the entry's input and the head's select: {incoming:?}");
    assert_ne!(incoming[1].0, Reg::Gpr(v(3)), "the head's input is the select's result, not the arm's value");
}

/// A `cbz` branch needs its own compare before the select can read it.
#[test]
fn a_compare_branch_makes_its_own_compare_before_the_select() {
    let mut f = function(vec![
        block("head", vec![
            Op::Copy { dst: Reg::Gpr(v(1)), src: Reg::Gpr(Gpr(0)) },
            Op::I(Inst::CompareBranch {
                nonzero: false,
                width: Width::X,
                rt: v(1),
                target: Target::Label("tail".to_owned()),
            }),
            jump("then"),
        ]),
        block("then", vec![add(v(3), v(1), 2), jump("tail")]),
        block("tail", vec![phi(v(4), &[(v(1), "head"), (v(3), "then")]), Op::Return { uses: vec![], tail: None }]),
    ]);
    assert_eq!(run(&mut f, Mode::Stress), 1);
    let ops = &f.blocks[0].ops;
    let compare = ops.iter().position(|op| matches!(op, Op::I(Inst::AddSubImm { rd, imm12: 0, .. }) if *rd == Gpr(31)));
    let select = ops.iter().position(|op| matches!(op, Op::I(Inst::CondSelect { cond: Cond::Eq, .. })));
    assert!(compare.is_some() && select.is_some() && compare < select, "{ops:?}");
}

/// An arm whose value is a long chain is not worth running on both paths: the
/// model keeps the branch where stress converts.
#[test]
fn the_model_keeps_a_branch_around_a_long_chain_that_stress_converts() {
    let mut ops = vec![add(v(10), v(1), 1)];
    for n in 1..=20u32 {
        ops.push(add(v(10 + n), v(9 + n), 1));
    }
    ops.push(add(v(3), v(30), 1));
    let mut f = triangle(ops.clone());
    assert_eq!(run(&mut f, Mode::Model), 0, "a chain of 22 dependent adds is 22 cycles");
    let mut g = triangle(ops);
    assert_eq!(run(&mut g, Mode::Stress), 1);
}

/// A short arm costs less than the mispredict the branch risks: the model
/// converts.
#[test]
fn the_model_converts_a_single_add() {
    let mut f = triangle(vec![add(v(3), v(1), 2)]);
    assert_eq!(run(&mut f, Mode::Model), 1);
}

/// The head's edge odds and the join: the absorbed tail is recorded for the
/// odds, as the head's.
#[test]
fn a_join_is_recorded_for_the_edge_odds() {
    let mut f = triangle(vec![add(v(3), v(1), 2)]);
    run(&mut f, Mode::Stress);
    assert_eq!(f.joined, vec![("head".to_owned(), "tail".to_owned())]);
}
