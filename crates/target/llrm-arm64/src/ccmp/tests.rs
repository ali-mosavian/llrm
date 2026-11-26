//! Conditional compares on hand-built bodies: what becomes a `ccmp`, what
//! stays two branches, and the flags and condition the `ccmp` is given.

use super::*;
use crate::mc::inst::{Addr, Memory, Shift};
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

fn cmp(
    rn: Gpr,
    imm12: u16,
) -> Op {
    Op::I(Inst::AddSubImm {
        op: AddSub { subtract: true, set_flags: true },
        width: Width::W,
        rd: Gpr(31),
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

fn cbz(
    rt: Gpr,
    label: &str,
) -> Op {
    Op::I(Inst::CompareBranch { nonzero: false, width: Width::W, rt, target: Target::Label(label.to_owned()) })
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

/// `if (x == 7 || y == 9) g();`: the head branches to the call on `x == 7`,
/// the second block on `y == 9`.
fn either(second: Vec<Op>) -> MFunc {
    let mut cmp_block = second;
    cmp_block.extend([branch_if(Cond::Eq, "call"), jump("out")]);
    function(vec![
        block(
            "head",
            vec![
                Op::Copy { dst: Reg::Gpr(v(1)), src: Reg::Gpr(Gpr(0)) },
                Op::Copy { dst: Reg::Gpr(v(2)), src: Reg::Gpr(Gpr(1)) },
                cmp(v(1), 7),
                branch_if(Cond::Eq, "call"),
                jump("second"),
            ],
        ),
        block("second", cmp_block),
        block("call", vec![Op::Return { uses: vec![], tail: Some(crate::mfunc::Callee::Symbol("g".into())) }]),
        block("out", vec![Op::Return { uses: vec![], tail: None }]),
    ])
}

fn ccmps(f: &MFunc) -> Vec<(bool, CcOperand, u8, Cond)> {
    f.blocks
        .iter()
        .flat_map(|b| b.ops.iter())
        .filter_map(|op| match op {
            Op::I(Inst::CondCompare { negate, rm_or_imm, nzcv, cond, .. }) => Some((*negate, *rm_or_imm, *nzcv, *cond)),
            _ => None,
        })
        .collect()
}

/// `x == 7 || y == 9` was two compares and two branches: the second compare
/// did not become a `ccmp` that runs when `x != 7` and otherwise sets `Z`.
#[test]
fn an_or_of_two_compares_is_a_cmp_and_a_ccmp() {
    let mut f = either(vec![cmp(v(2), 9)]);
    assert_eq!(run(&mut f, false, true), 1);
    assert_eq!(ccmps(&f), vec![(false, CcOperand::Imm5(9), 4, Cond::Ne)]);
    assert_eq!(f.blocks.len(), 3, "the second block went into the head");
    assert_eq!(f.cmp_merged, vec![("head".to_owned(), "second".to_owned())]);
}

/// A compare with an immediate over 31 has no `ccmp` form.
#[test]
fn an_immediate_a_ccmp_cannot_hold_stays_a_branch() {
    let mut f = either(vec![cmp(v(2), 32)]);
    assert_eq!(run(&mut f, false, true), 0);
    assert!(ccmps(&f).is_empty());
}

/// The second block's load cannot run when the first branch is taken.
#[test]
fn a_load_in_the_second_block_stays_a_branch() {
    let load = Op::I(Inst::Load {
        mem: Memory::Int { width: Width::W, bytes: 4, sign: false, reg: v(3) },
        addr: Addr::Offset { base: v(2), imm: 0 },
        mmo: Default::default(),
    });
    let mut f = either(vec![load, cmp(v(3), 9)]);
    assert_eq!(run(&mut f, false, true), 0);
}

/// A second compare whose result is read needs the flags it makes in full.
#[test]
fn a_compare_whose_result_is_used_stays_a_branch() {
    let live = Op::I(Inst::AddSubShifted {
        op: AddSub { subtract: true, set_flags: true },
        width: Width::W,
        rd: v(5),
        rn: v(2),
        rm: v(1),
        shift: Shift::Lsl,
        amount: 0,
    });
    let mut f = either(vec![live]);
    f.blocks[2].ops = OpList::from(vec![
        Op::Copy { dst: Reg::Gpr(v(6)), src: Reg::Gpr(v(5)) },
        Op::Return { uses: vec![], tail: None },
    ]);
    assert_eq!(run(&mut f, false, true), 0);
}

/// Two `cbz` on `x == 0 && y == 0`: the head's gets a `cmp #0`, the second's
/// is `ccmp #0`.
#[test]
fn two_cbz_become_a_cmp_and_a_ccmp_against_zero() {
    let mut f = function(vec![
        block(
            "head",
            vec![
                Op::Copy { dst: Reg::Gpr(v(1)), src: Reg::Gpr(Gpr(0)) },
                Op::Copy { dst: Reg::Gpr(v(2)), src: Reg::Gpr(Gpr(1)) },
                cbz(v(1), "second"),
                jump("out"),
            ],
        ),
        block("second", vec![cbz(v(2), "call"), jump("out")]),
        block("call", vec![Op::Return { uses: vec![], tail: Some(crate::mfunc::Callee::Symbol("g".into())) }]),
        block("out", vec![Op::Return { uses: vec![], tail: None }]),
    ]);
    assert_eq!(run(&mut f, false, true), 1);
    assert_eq!(ccmps(&f), vec![(false, CcOperand::Imm5(0), 0, Cond::Eq)]);
    let head = &f.blocks[0].ops;
    assert!(head.iter().any(|op| matches!(op, Op::I(Inst::AddSubImm { imm12: 0, .. }))), "the head's cbz needs a cmp");
}

/// The model keeps a branch whose second compare would wait on a long chain.
#[test]
fn the_model_can_say_no() {
    let mut chain = Vec::new();
    for n in 0..8 {
        chain.push(Op::I(Inst::ThreeSource {
            op: crate::mc::inst::Mul3::Madd,
            width: Width::W,
            rd: v(10 + n),
            rn: if n == 0 { v(2) } else { v(9 + n) },
            rm: v(2),
            ra: Gpr(31),
        }));
    }
    chain.push(cmp(v(17), 9));
    let mut f = either(chain);
    assert_eq!(run(&mut f, false, false), 0);
}

/// Every epoch of the pass modelled every instruction of the body for the trace
/// metrics (143 of a 4-million-instruction module's 176 M for `ccmp`), though
/// at minimum size the code size decides a candidate and a body with no
/// candidate needs no trace. A body is modelled when a candidate needs it.
#[test]
fn the_body_is_modelled_only_for_a_candidate_the_size_does_not_decide() {
    let before = metrics_built();
    let mut quiet = function(vec![
        block("head", vec![cmp(v(1), 7), jump("out")]),
        block("mid", vec![jump("out")]),
        block("out", vec![Op::Return { uses: vec![], tail: None }]),
    ]);
    assert_eq!(run(&mut quiet, false, false), 0);
    assert_eq!(metrics_built(), before, "a body with no candidate was modelled");
    let mut sized = either(vec![cmp(v(2), 9)]);
    assert_eq!(run(&mut sized, true, false), 1, "premise: at minimum size the two compares become a ccmp");
    assert_eq!(metrics_built(), before, "a candidate the size decides was modelled");
    let mut timed = either(vec![cmp(v(2), 9)]);
    run(&mut timed, false, false);
    assert!(metrics_built() > before, "premise: a candidate the clocks decide is modelled");
}
