//! Work on `a` moved from before a compare to after its branch.
//!
//! # How it works
//! [`sink_past_compare`]: `tay; [clc; adc #n]; cpy #k; bcs L` is
//! `tay; cpy #k; bcs L; [clc; adc #n]` when `a` is dead at `L`: the work is
//! only wanted on the fall-through, and the compare (of `x` or `y`, not `a`)
//! does not depend on it. [`compare_the_copy`]: `tay; cpy m` is `cmp m` when
//! `y` is dead after it (likewise `tax`, `cpx`): `a` holds the byte already.
//!
//! ```text
//!   tay; clc; adc #65; cpy #26; bcs L; tax     cmp #26; bcs L; clc; adc #65; tax
//! ```
//!
//! Pass: LLVM's `MachineSink` moves an instruction into the one successor
//! that uses its result. DEVIATE SINK-COMPARE (owner 6502, exit 2026-11-24):
//! it works on the expanded 6502 text, for the instructions that write only `a`
//! and the flags, and the fall-through must leave the flags unread (it sees the
//! work's, not the compare's) and the work must not read the carry it enters
//! with. Retired by: translating `MachineSink` at the pre-expansion (MIR)
//! level, where the 6502 flags are not yet an operand of the work.
//!
//! # Runs
//! In [`crate::peephole`]'s suffix, before [`crate::reassociate`], which then
//! drops the `clc` the compare's branch made redundant.

use crate::asm::{Asm, Inst, Line, Op, Operand, Registers};
use crate::peephole::{Exits, carry_dead_from, dead_from, flags_unread_after, references};

/// An instruction that writes only `a` and the flags and may be skipped where
/// `a` is dead: no store, no volatile read.
fn only_a(inst: &Inst) -> bool {
    let readable = matches!(&inst.operand, Operand::Imm(_)) || inst.operand.is_cell();
    match inst.op {
        Op::Clc | Op::Sec | Op::Txa | Op::Tya => true,
        Op::Lda | Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor => readable,
        Op::Asl | Op::Lsr | Op::Rol | Op::Ror => matches!(inst.operand, Operand::Acc | Operand::None),
        _ => false,
    }
}

fn inst_at(
    lines: &[Line],
    at: usize,
) -> Option<&Inst> {
    match lines.get(at) {
        Some(Line::Inst(inst)) => Some(inst),
        _ => None,
    }
}

/// An operand a compare of `x` or `y` reads that no instruction of `a` writes.
fn plain(inst: &Inst) -> bool {
    matches!(&inst.operand, Operand::Imm(_)) || inst.operand.is_cell()
}

/// The overflow flag is not tracked: the work moved sets it.
fn reads_overflow(asm: &Asm) -> bool {
    asm.lines
        .iter()
        .any(
            |line| matches!(
                line,
                Line::Inst(Inst { op: Op::Bvc | Op::Bvs | Op::Php | Op::Plp, .. })
            ),
        )
}

/// Whether the first instruction of `run` that touches the carry reads it.
fn reads_carry_first(run: &[Line]) -> bool {
    for line in run {
        match line {
            Line::Inst(Inst { op: Op::Clc | Op::Sec, .. }) => return false,
            Line::Inst(Inst { op: Op::Adc | Op::Sbc | Op::Rol | Op::Ror, .. }) => return true,
            _ => {}
        }
    }
    false
}

pub fn sink_past_compare(asm: &mut Asm) {
    if reads_overflow(asm) || asm.lines.iter().any(|line| matches!(line, Line::Raw(_))) {
        return;
    }
    let mut at = 2;
    while at < asm.lines.len() {
        let moved = sinkable(&asm.lines, at, Exits::of(asm));
        if let Some(run) = moved {
            asm.lines[at - 1 - run..=at].rotate_left(run);
            at += 1;
        }
        at += 1;
    }
}

/// How many instructions before the compare at `branch - 1` move after the
/// branch at `branch`.
fn sinkable(
    lines: &[Line],
    branch: usize,
    exits: Exits,
) -> Option<usize> {
    let jump = inst_at(lines, branch)?;
    jump.op.opposite()?;
    let compare = inst_at(lines, branch - 1)?;
    if !matches!(compare.op, Op::Cpx | Op::Cpy) || !plain(compare) {
        return None;
    }
    let mut run = (0..branch - 1).rev().map_while(|at| inst_at(lines, at).filter(|inst| only_a(inst))).count();
    // the compare's carry reaches the work after the move: it may not read the
    // carry the work was entered with
    while run > 0 && reads_carry_first(&lines[branch - 1 - run..branch - 1]) {
        run -= 1;
    }
    if run == 0 {
        return None;
    }
    // the fall-through now sees the work's flags, not the compare's
    if !flags_unread_after(lines, branch + 1) || !carry_dead_from(lines, branch + 1, &mut Default::default()) {
        return None;
    }
    let label = jump.target()?;
    let to = lines.iter().position(|line| *line == Line::Label(label.to_owned()))?;
    // the fall-through is the one place that wants `a`: nothing else may enter
    // it
    let mut next = branch + 1;
    while let Some(Line::Label(name)) = lines.get(next) {
        if references(lines, name) > 0 {
            return None;
        }
        next += 1;
    }
    dead_from(lines, to, Registers::A, &mut Default::default(), exits).then_some(run)
}

pub fn compare_the_copy(asm: &mut Asm) {
    let mut at = 0;
    while at + 1 < asm.lines.len() {
        let pair = match (inst_at(&asm.lines, at), inst_at(&asm.lines, at + 1)) {
            (Some(Inst { op: Op::Tay, .. }), Some(c @ Inst { op: Op::Cpy, .. })) => Some((Registers::Y, c)),
            (Some(Inst { op: Op::Tax, .. }), Some(c @ Inst { op: Op::Cpx, .. })) => Some((Registers::X, c)),
            _ => None,
        };
        let merged = pair.filter(|(register, compare)| {
            plain(compare) && dead_from(&asm.lines, at + 2, *register, &mut Default::default(), Exits::of(asm))
        });
        if let Some((_, Inst { operand, place, .. })) = merged {
            let compare = Line::Inst(Inst { op: Op::Cmp, operand: operand.clone(), place: *place });
            asm.lines.splice(at..at + 2, [compare]);
        }
        at += 1;
    }
}
