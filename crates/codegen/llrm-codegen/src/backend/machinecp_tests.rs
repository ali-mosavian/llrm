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
    Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) }
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
/// was taken for dead (the loop in `test_an_exact_elements_offset_is_folded_
/// into_a_scaled_address` read an undefined `bx`).
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

/// `bx = ss` then `es = bx`: the segment register is read from `ss`.
#[test]
fn test_a_segment_register_is_loaded_from_the_source_of_the_copy() {
    let got = propagated(&body(
        vec![mov(0, rl(RegId::BX, 2), rl(RegId::SS, 2)), mov(1, rl(RegId::ES, 2), rl(RegId::BX, 2))],
        &[RegId::BX],
    ));
    assert_eq!(lines(&got), ["-", "mov ES,SS"]);
}

/// A segment register that is only ever a copy's destination is not a copy to
/// drop: `es` is read by string instructions that say nothing of it.
#[test]
fn test_a_copy_into_a_segment_register_is_kept() {
    let got = propagated(&body(vec![mov(0, rl(RegId::ES, 2), rl(RegId::BX, 2))], &[RegId::ES]));
    assert_eq!(lines(&got), ["mov ES,BX"]);
}

/// `mov es, bx` twice with neither side written between: the second reloads
/// what `es` holds (triggers.c `ent_move_trigs`, dag m16 -Os: 9 of 78 register
/// moves were these). A write to `bx` between them keeps it.
#[test]
fn test_a_segment_load_that_repeats_an_available_one_is_dropped() {
    let repeated = vec![
        mov(0, rl(RegId::ES, 2), rl(RegId::BX, 2)),
        add(1, rl(RegId::CX, 2), rl(RegId::DX, 2)),
        mov(2, rl(RegId::ES, 2), rl(RegId::BX, 2)),
    ];
    assert_eq!(lines(&propagated(&body(repeated, &[RegId::ES]))), ["mov ES,BX", "add CX,CX,DX", "-"]);
    let rewritten = vec![
        mov(0, rl(RegId::ES, 2), rl(RegId::BX, 2)),
        add(1, rl(RegId::BX, 2), rl(RegId::DX, 2)),
        mov(2, rl(RegId::ES, 2), rl(RegId::BX, 2)),
    ];
    assert_eq!(lines(&propagated(&body(rewritten, &[RegId::ES]))), ["mov ES,BX", "add BX,BX,DX", "mov ES,BX"]);
}

/// `bx` is shifted in place and then copied to `ax`, which held `bx` already:
/// the shift works on `ax` and the copy goes. (`propagateDefs` declines a
/// tied definition; the pinned output of the fixed-point test was this form.)
#[test]
fn test_a_tied_producer_works_on_the_register_that_already_holds_its_value() {
    let shift = |at| {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(semantics(Operation::Binary, "shl", vec![rl(RegId::BX, 2)], vec![rl(RegId::BX, 2), imm(3, 1)])),
            vec![],
            vec![],
        ))
    };
    let got = propagated(&body(
        vec![mov(0, rl(RegId::AX, 2), rl(RegId::BX, 2)), shift(1), mov(2, rl(RegId::AX, 2), rl(RegId::BX, 2))],
        &[RegId::BX],
    ));
    assert_eq!(lines(&got), ["mov AX,BX", "shl AX,AX,imm", "-"]);
}

/// `rep stosd` writes `di`: a read of `si` after it is not a read of `di`.
fn stosd(at: i64) -> Arc<Insn> {
    use crate::model::ir::Mem;
    let what = Semantics {
        name: Some("stosd".into()),
        dests: vec![Loc::Mem(Mem::new(None, 0)), rl(RegId::DI, 2), rl(RegId::CX, 2)],
        sources: vec![rl(RegId::EAX, 4), rl(RegId::CX, 2), rl(RegId::DI, 2), rl(RegId::ES, 2)],
        ..Semantics::new(Operation::Fill)
    };
    Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
}

/// `mov si, di; rep stosd; mov [si+4], 0` was `ss:[di+4]` after the stores of
/// lru (bench/lru, -O2 m16): `di` is what `si` held before the string store
/// moved it by 28 bytes (the cache held 1.4 vs 6.0 for the clock).
#[test]
fn test_a_copy_is_not_forwarded_across_a_string_store_that_writes_its_source() {
    let got = propagated(&body(
        vec![mov(0, rl(RegId::SI, 2), rl(RegId::DI, 2)), stosd(1), add(2, rl(RegId::BX, 2), rl(RegId::SI, 2))],
        &[RegId::BX],
    ));
    assert_eq!(lines(&got)[2], "add BX,BX,SI", "{:?}", lines(&got));
}

/// `rep movsd` writes `si` and `di` the same way: a copy of either is dead
/// after it.
#[test]
fn test_a_copy_is_not_forwarded_across_a_string_move_that_writes_its_source() {
    use crate::model::ir::Mem;
    let what = Semantics {
        name: Some("movsd".into()),
        dests: vec![Loc::Mem(Mem::new(None, 0)), rl(RegId::SI, 2), rl(RegId::DI, 2), rl(RegId::CX, 2)],
        sources: vec![Loc::Mem(Mem::new(None, 0)), rl(RegId::CX, 2), rl(RegId::SI, 2), rl(RegId::DI, 2)],
        ..Semantics::new(Operation::Copy)
    };
    let movsd = Arc::new(Insn::new(1, Some((1, 1)), Some(what), vec![], vec![]));
    for (copy, read) in [(RegId::SI, RegId::DI), (RegId::DI, RegId::SI)] {
        let got = propagated(&body(
            vec![mov(0, rl(copy, 2), rl(read, 2)), Arc::clone(&movsd), add(2, rl(RegId::BX, 2), rl(copy, 2))],
            &[RegId::BX],
        ));
        assert_eq!(lines(&got)[2], format!("add BX,BX,{copy:?}"), "{:?}", lines(&got));
    }
}

/// Both walks of a block asked `written` and the read mask of each
/// instruction again (2,440 times for d_faces' 829 walked ones, 1.7 M Ir):
/// they are worked out once when the block is read.
#[test]
fn test_a_walk_works_out_what_an_instruction_writes_once() {
    let mut head = vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2))];
    for n in 1..9 {
        head.push(add(n, rl(RegId::CX, 2), rl(RegId::DX, 2)));
    }
    let before = super::masks_made();
    propagated(&body(head, &[RegId::BX, RegId::CX]));
    let made = super::masks_made() - before;
    assert!(made <= 9 + 2, "{made} asks for 9 instructions");
}

/// `register_mask` listed the lanes of a register each time a walk named it
/// (939 times for 0.3 M Ir in d_faces): once for each register.
#[test]
fn test_a_register_s_units_are_worked_out_once_for_a_body() {
    let mut head = vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2))];
    for n in 1..9 {
        head.push(add(n, rl(RegId::CX, 2), rl(RegId::BX, 2)));
    }
    let before = super::register_masks_made();
    propagated(&body(head, &[RegId::BX, RegId::CX]));
    let made = super::register_masks_made() - before;
    assert!(made <= 6, "{made} derivations for a body of 4 registers");
}

/// Equivalence, not fail-first: `mask` read each lane through the lane table
/// (a lazy lookup per step; 0.4 M Ir of `masks_of` in d_faces) and now reads a
/// table of bits. Same units for every lane bit and for unions of them.
#[test]
fn test_the_mask_of_lanes_is_the_units_of_each_lane() {
    let at = body(vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2))], &[RegId::BX]);
    let machine = super::Machine::new(&at);
    let old = |lanes: &crate::backend::lanes::Lanes| -> u64 {
        lanes.iter().filter_map(|lane| machine.unit(lane)).fold(0, |mask, unit| mask | 1 << unit)
    };
    let mut state = 0x9e37_79b9_7f4a_7c15_u128;
    let count = crate::backend::lanes::Lanes::count();
    for bit in 0..count {
        let one = crate::backend::lanes::Lanes::from_mask(1 << bit);
        assert_eq!(machine.mask(&one), old(&one), "bit {bit}");
        state = state.wrapping_mul(0x2545_f491_4f6c_dd1d).rotate_left(29) ^ (state >> 7);
        let some = crate::backend::lanes::Lanes::from_mask(state & ((1 << count) - 1));
        assert_eq!(machine.mask(&some), old(&some), "union {state:x}");
    }
}

/// The check for a copy in a block asked `copy_of` of its instructions, and
/// the walk of a block with one asked all of them again (6,368 asks for
/// d_faces' 1,251 walked instructions): once for each.
#[test]
fn test_a_walked_block_is_asked_for_its_copies_once() {
    let mut head: Vec<_> = (0..8).map(|n| add(n, rl(RegId::CX, 2), rl(RegId::DX, 2))).collect();
    head.push(mov(8, rl(RegId::BX, 2), rl(RegId::AX, 2)));
    let before = super::copy_ofs();
    propagated(&body(head, &[RegId::BX, RegId::CX]));
    let asked = super::copy_ofs() - before;
    // 9 instructions of the head and the 2 of the tail, and each rewrite once.
    assert!(asked <= 11 + 3, "{asked} asks for 11 instructions");
}

/// x_hanoi2 `mv`: `mov bx, ax; test bx, bx` kept reading bx, one operand
/// renamed alone is another instruction, so the copy stayed in the entry and
/// shrink wrapping found a callee-saved definition there (13 instructions a
/// leaf call against the pin's 3).
#[test]
fn test_an_instruction_that_reads_the_copys_destination_twice_reads_the_source_twice() {
    let test = Arc::new(Insn::new(
        1,
        Some((1, 1)),
        Some(semantics(Operation::Compare, "test", vec![], vec![rl(RegId::BX, 2), rl(RegId::BX, 2)])),
        vec![],
        vec![],
    ));
    let got = propagated(&body(vec![mov(0, rl(RegId::BX, 2), rl(RegId::AX, 2)), test], &[RegId::BX]));
    assert_eq!(lines(&got), ["-", "test AX,AX"]);
}

/// A copy of a value and a compare of the copy: the compare reads the source
/// once the copy is forwarded, and the copy is dead. The compare kept naming
/// the copy's value, which no instruction defined any more: `LLRM_VERIFY=1`
/// stopped bench/lru at -Os with "value#89 is read but never defined".
#[test]
fn test_a_forwarded_read_names_the_value_of_the_source_not_of_the_dead_copy() {
    let valued = |one: Arc<Insn>, defines: Vec<u32>, uses: Vec<u32>| Arc::new(Insn { defines, uses, ..(*one).clone() });
    let cmp = Arc::new(Insn::new(
        2,
        Some((2, 2)),
        Some(semantics(Operation::Compare, "cmp", vec![], vec![rl(RegId::BX, 2), imm(-1, 2)])),
        vec![],
        vec![89],
    ));
    let got = propagated(&body(
        vec![
            valued(mov(0, rl(RegId::DX, 2), imm(0, 2)), vec![90], vec![]),
            valued(mov(1, rl(RegId::BX, 2), rl(RegId::DX, 2)), vec![89], vec![90]),
            cmp,
        ],
        &[RegId::BX],
    ));
    assert_eq!(lines(&got)[2], "cmp DX,imm");
    assert_eq!(first(&got)[2].uses, [90], "the cmp still reads the value of the deleted copy");
}
