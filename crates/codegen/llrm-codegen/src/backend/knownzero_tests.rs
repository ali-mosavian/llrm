//! Tests of `knownzero`: a move into a register that already holds the zero.
use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::removed;
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

fn rl(
    register: RegId,
    width: u32,
) -> Loc {
    Loc::Reg(Reg { register, width })
}

fn one(
    at: i64,
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Arc<Insn> {
    let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

fn mov(
    at: i64,
    dest: Loc,
    source: Loc,
) -> Arc<Insn> {
    one(at, Operation::Move, "mov", vec![dest], vec![source])
}

fn zero(
    at: i64,
    dest: Loc,
) -> Arc<Insn> {
    one(at, Operation::Binary, "xor", vec![dest.clone()], vec![dest.clone(), dest])
}

fn imm(value: i64) -> Loc {
    Loc::Imm(Imm { value, width: 2, address: None })
}

fn add(
    at: i64,
    dest: Loc,
    source: Loc,
) -> Arc<Insn> {
    one(at, Operation::Binary, "add", vec![dest.clone()], vec![dest, source])
}

/// Blocks `(at, successors, instructions)`, the first the entry.
fn body(blocks: Vec<(i64, Vec<i64>, Vec<Arc<Insn>>)>) -> LirBody {
    let blocks = blocks
        .into_iter()
        .map(|(at, succ, insns)| Block::from(LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }))
        .collect();
    LirBody::new("t", 0, blocks, IndexMap::default(), IndexMap::default())
}

/// The instruction names of every block, `-` for a dropped one.
fn names(body: &LirBody) -> Vec<Vec<String>> {
    body.blocks
        .iter()
        .map(|block| {
            block
                .insns
                .iter()
                .map(|one| match one.what.as_ref().and_then(|what| what.name.as_deref()) {
                    Some(name) if !name.is_empty() => name.to_owned(),
                    _ => "-".to_owned(),
                })
                .collect()
        })
        .collect()
}

const EDX: fn() -> Loc = || rl(RegId::EDX, 4);
const EAX: fn() -> Loc = || rl(RegId::EAX, 4);

/// m16 sieve -Os: the count's zero made at entry, the sum's in a block the
/// entry leads to, then `mov dx, ax` into a register that is already zero:
/// 112 B against Open Watcom's 111.
#[test]
fn test_a_copy_of_a_zero_into_a_zero_register_is_dropped_across_blocks() {
    let got = removed(&body(vec![
        (0, vec![1], vec![zero(0, EDX())]),
        (1, vec![2], vec![zero(1, EAX()), add(2, rl(RegId::CX, 2), imm(1))]),
        (2, vec![], vec![mov(3, rl(RegId::DX, 2), rl(RegId::AX, 2))]),
    ]));
    assert_eq!(names(&got)[2], ["-"]);
}

/// A register another way into the block leaves nonzero keeps its move.
#[test]
fn test_a_move_stays_where_one_way_in_leaves_the_register_nonzero() {
    let got = removed(&body(vec![
        (0, vec![1, 2], vec![zero(0, EDX()), zero(1, EAX())]),
        (1, vec![3], vec![add(2, rl(RegId::DX, 2), imm(1))]),
        (2, vec![3], vec![]),
        (3, vec![], vec![mov(3, rl(RegId::DX, 2), rl(RegId::AX, 2))]),
    ]));
    assert_eq!(names(&got)[3], ["mov"]);
}

/// A write between the zero and the move ends what is known.
#[test]
fn test_a_move_stays_after_a_write_to_its_destination() {
    let got = removed(&body(vec![(
        0,
        vec![],
        vec![
            zero(0, EDX()),
            zero(1, EAX()),
            add(2, rl(RegId::DX, 2), imm(1)),
            mov(3, rl(RegId::DX, 2), rl(RegId::AX, 2)),
        ],
    )]));
    assert_eq!(names(&got)[0], ["xor", "xor", "add", "mov"]);
}

/// A move of an immediate zero into a zero register is dropped; the `xor`
/// that sets the flags is not.
#[test]
fn test_a_zero_into_a_zero_register_is_dropped_but_not_an_xor() {
    let got = removed(&body(vec![(0, vec![], vec![zero(0, EDX()), mov(1, rl(RegId::DX, 2), imm(0)), zero(2, EDX())])]));
    assert_eq!(names(&got)[0], ["xor", "-", "xor"]);
}

/// A source that is not known zero keeps the move.
#[test]
fn test_a_copy_from_a_register_not_known_zero_stays() {
    let got = removed(&body(vec![(0, vec![], vec![zero(0, EDX()), mov(1, rl(RegId::DX, 2), rl(RegId::AX, 2))])]));
    assert_eq!(names(&got)[0], ["xor", "mov"]);
}
