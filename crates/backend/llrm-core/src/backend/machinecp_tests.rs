//! Tests of `machinecp`, the translation of LLVM's MachineCopyPropagation.
use std::sync::Arc;

use llrm_lir::registers::RegId;

use super::propagated;
use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Block, Insn, LirBlock, LirBody};
use crate::support::hash::IndexMap;

fn semantics(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) }
}

fn rl(
    register: RegId,
    width: u32,
) -> Loc {
    Loc::Reg(Reg { register, width })
}

fn mov(
    at: i64,
    dest: Loc,
    source: Loc,
) -> Arc<Insn> {
    Arc::new(Insn::new(
        at,
        Some((at, at)),
        Some(semantics(Operation::Move, "mov", vec![dest], vec![source])),
        vec![],
        vec![],
    ))
}

fn imm(
    value: i64,
    width: u32,
) -> Loc {
    Loc::Imm(Imm { value, width, address: None })
}

fn add(
    at: i64,
    dest: Loc,
    source: Loc,
) -> Arc<Insn> {
    Arc::new(Insn::new(
        at,
        Some((at, at)),
        Some(semantics(Operation::Binary, "add", vec![dest.clone()], vec![dest, source])),
        vec![],
        vec![],
    ))
}

/// `head` in a block that falls into one that writes every register in
/// `overwritten` and reads none: what `head` leaves in them is dead.
fn body(
    head: Vec<Arc<Insn>>,
    overwritten: &[RegId],
) -> LirBody {
    let tail =
        overwritten.iter().enumerate().map(|(n, register)| mov(100 + n as i64, rl(*register, 2), imm(1, 2))).collect();
    let first = LirBlock { succ: vec![1].into(), ..LirBlock::new(0, head) };
    let second = LirBlock::new(1, tail);
    LirBody::new("t", 0, vec![Block::from(first), Block::from(second)], IndexMap::default(), IndexMap::default())
}

fn first(body: &LirBody) -> Vec<Arc<Insn>> {
    body.blocks[0].insns.iter().cloned().collect()
}

fn text(one: &Insn) -> String {
    let what = one.what.as_ref().unwrap();
    let show = |loc: &Loc| match loc {
        Loc::Reg(reg) => format!("{:?}", reg.register),
        Loc::Imm(_) => "imm".to_owned(),
        _ => "?".to_owned(),
    };
    let inert = what.name.as_deref().is_none_or(str::is_empty);
    if inert {
        return "-".to_owned();
    }
    format!(
        "{} {}",
        what.name.as_deref().unwrap(),
        what.dests
            .iter()
            .chain(&what.sources)
            .skip(if what.dests.is_empty() { 0 } else { 0 })
            .map(show)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn lines(body: &LirBody) -> Vec<String> {
    first(body).iter().map(|one| text(one)).collect()
}

/// A read of a copy's destination reads its source, and the copy is dead.
#[test]
fn test_a_read_of_a_copys_destination_reads_its_source_and_the_copy_dies() {
    let got = propagated(&body(
        vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2)), add(1, rl(RegId::CX, 2), rl(RegId::BX, 2))],
        &[RegId::BX, RegId::CX],
    ));
    assert_eq!(lines(&got), ["-", "add CX,CX,AX"]);
}

/// `bx = ax` twice with neither side written between: the second is dropped.
#[test]
fn test_a_copy_that_repeats_an_available_one_is_dropped() {
    let got = propagated(&body(
        vec![
            mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2)),
            add(1, rl(RegId::CX, 2), rl(RegId::DX, 2)),
            mov(2, rl(RegId::BX, 2), rl(RegId::AX, 2)),
        ],
        &[],
    ));
    assert_eq!(lines(&got), ["mov BX,AX", "add CX,CX,DX", "-"]);
}

/// `bx = ax` then `ax = bx`: the second moves nothing.
#[test]
fn test_the_reverse_of_an_available_copy_is_dropped() {
    let got = propagated(&body(
        vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2)), mov(1, rl(RegId::AX, 2), rl(RegId::BX, 2))],
        &[],
    ));
    assert_eq!(lines(&got), ["mov BX,AX", "-"]);
}

/// A write to the source ends the copy: the read stays on the destination.
#[test]
fn test_a_written_source_ends_the_copy() {
    let got = propagated(&body(
        vec![
            mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2)),
            mov(1, rl(RegId::AX, 2), imm(5, 2)),
            add(2, rl(RegId::CX, 2), rl(RegId::BX, 2)),
        ],
        &[],
    ));
    assert_eq!(lines(&got), ["mov BX,AX", "mov AX,imm", "add CX,CX,BX"]);
}

/// A copy is read in the block it is made in: across a join it is not
/// carried (LLVM's tracker is cleared at each block).
#[test]
fn test_a_copy_is_not_carried_into_the_next_block() {
    let first = LirBlock { succ: vec![1].into(), ..LirBlock::new(0, vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2))]) };
    let second = LirBlock::new(1, vec![add(1, rl(RegId::CX, 2), rl(RegId::BX, 2))]);
    let got = propagated(&LirBody::new(
        "t",
        0,
        vec![Block::from(first), Block::from(second)],
        IndexMap::default(),
        IndexMap::default(),
    ));
    let second: Vec<_> = got.blocks[1].insns.iter().map(|one| text(one)).collect();
    assert_eq!(second, ["add CX,CX,BX"]);
}

/// Defining the source in a producer whose result is copied and then dies:
/// the producer writes the copy's destination.
#[test]
fn test_a_producer_writes_the_destination_of_the_copy_of_its_result() {
    let got = propagated(&body(
        vec![mov(0, rl(RegId::AX, 2), imm(7, 2)), mov(1, rl(RegId::BX, 2), rl(RegId::AX, 2))],
        &[RegId::AX],
    ));
    assert_eq!(lines(&got), ["mov BX,imm", "-"]);
}

/// A write to one byte of a copy's destination ends the whole copy
/// (`clobberRegUnit` marks the destination register unavailable), where the
/// lane-wise `copyprop` kept the other byte.
#[test]
fn test_a_write_to_one_byte_of_the_destination_ends_the_copy() {
    let compare = Arc::new(Insn::new(
        2,
        Some((2, 2)),
        Some(semantics(Operation::Compare, "cmp", vec![], vec![rl(RegId::AL, 1), imm(3, 1)])),
        vec![],
        vec![],
    ));
    let got = propagated(&body(
        vec![mov(0, rl(RegId::AX, 2), rl(RegId::BX, 2)), mov(1, rl(RegId::AH, 1), imm(7, 1)), compare],
        &[],
    ));
    assert_eq!(lines(&got)[2], "cmp AL,imm");
}

/// The work grows with the block: the tracker visits a bounded number of
/// units per instruction, whatever the block holds.
#[test]
fn test_the_work_grows_with_the_block_not_with_its_square() {
    let work = |copies: i64| {
        let head: Vec<Arc<Insn>> = (0..copies)
            .flat_map(|n| {
                [
                    mov(3 * n, rl(RegId::BX, 2), rl(RegId::AX, 2)),
                    add(3 * n + 1, rl(RegId::CX, 2), rl(RegId::BX, 2)),
                    mov(3 * n + 2, rl(RegId::AX, 2), rl(RegId::DX, 2)),
                ]
            })
            .collect();
        let before = super::steps();
        propagated(&body(head, &[RegId::BX]));
        super::steps() - before
    };
    let (small, large) = (work(200), work(400));
    assert!(large as f64 <= 2.2 * small as f64, "{small} units for 200 copies, {large} for 400");
}

/// A constant shift reads its destination only where the result is live, so
/// `Effect::reads` leaves it out: the copy `bx = ax` that feeds `shl bx, 1`
/// was taken for dead and removed, leaving `bx` undefined.
#[test]
fn test_a_copy_read_by_a_shift_is_not_dead() {
    let shift = Arc::new(Insn::new(
        1,
        Some((1, 1)),
        Some(semantics(Operation::Binary, "shl", vec![rl(RegId::BX, 2)], vec![rl(RegId::BX, 2), imm(1, 1)])),
        vec![],
        vec![],
    ));
    let got = propagated(&body(vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2)), shift], &[RegId::BX]));
    assert_eq!(lines(&got)[0], "mov BX,AX");
}
