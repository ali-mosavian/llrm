//! Redundant loads, compares, transfers and jumps removed from a function's
//! instructions.
//!
//! # How it works
//! Five passes walk the lines of an [`Asm`] once each. What an instruction does
//! to `a` and to the zero and sign flags comes from [`crate::asm::Op::flags`],
//! so a pass asks "does this instruction set the flags" and does not list
//! mnemonics:
//!
//! - [`drop_reloads`]: `sta m; lda m` (likewise `x`, `y`) of a zero-page or
//!   spill cell, when the load's flags are unread. A carry flag op or a store
//!   to another cell in between leaves the register and the cell as they were.
//! - [`drop_known_loads`]: `ldx #n` of what the register already holds, tracked
//!   through immediates and transfers, forgotten at a label, a call or an
//!   instruction not listed.
//! - [`drop_zero_compares`]: `cmp #0` before a branch on zero or sign, after an
//!   instruction that set those flags from `a`. Labels between the two do not
//!   matter: a path that jumps to one brings its own flags.
//! - [`drop_cancelling_transfers`]: `tya; tay` and its three twins: the second
//!   moves what the first just moved.
//! - [`jump_to_return`]: `jmp l` where `l:` is a lone `rts` becomes `rts`.
//!
//! DEVIATE: LLVM's `PeepholeOptimizer` and `MachineCSE` work on allocated
//! machine instructions before they are expanded; these work on the expanded
//! 6502 instructions of one function, because the 6502 emitter expands as it
//! allocates. Exit: D-6502 of the greedy PR #6259 puts the 6502 on the shared
//! pipeline; the passes that need only registers and flags move there.
//!
//! # Example
//! ```text
//!   sta rc2          sta rc2
//!   lda rc2     =>   ora rc3
//!   ora rc3
//! ```
//!
//! # Runs
//! At the end of `alloc::emit_with`, before branch relaxation.

use std::collections::HashSet;

use crate::asm::{Asm, Flags, Inst, Line, Op, Operand};

/// Whether the zero and sign flags are set again before anything reads them,
/// from `lines[from]` on.
fn flags_unread_after(
    lines: &[Line],
    from: usize,
) -> bool {
    for line in &lines[from..] {
        let Line::Inst(inst) = line else { return false };
        match inst.flags() {
            Flags::Keeps => {}
            Flags::OfA | Flags::Sets => return true,
            Flags::Reads => return false,
        }
    }
    false
}

/// Whether the emitted instruction leaves the zero and sign flags describing
/// what `a` now holds.
pub fn sets_nz_of_a(inst: &Inst) -> bool {
    inst.flags() == Flags::OfA
}

/// The passes, in the order they feed each other.
pub fn run(asm: &mut Asm) {
    drop_reloads(asm);
    drop_known_loads(asm);
    drop_zero_compares(asm);
    drop_cancelling_transfers(asm);
    jump_to_return(asm);
}

/// The first instruction at or after `lines[from]`, past labels.
fn next_instruction(
    lines: &[Line],
    from: usize,
) -> Option<&Inst> {
    for line in &lines[from.min(lines.len())..] {
        match line {
            Line::Inst(inst) => return Some(inst),
            Line::Label(_) => {}
            Line::Raw(_) => return None,
        }
    }
    None
}

/// `cmp #0` before a branch on zero or sign, after an instruction that set
/// those flags from `a` (stores in between leave them): the branch reads them
/// as they are.
pub fn drop_zero_compares(asm: &mut Asm) {
    let lines = &asm.lines;
    let keep: Vec<bool> = (0..lines.len())
        .map(|at| {
            let compare = matches!(
                &lines[at],
                Line::Inst(Inst { op: Op::Cmp, operand: Operand::Imm(v) }) if v == "0"
            );
            let branch = next_instruction(lines, at + 1)
                .is_some_and(|next| matches!(next.op, Op::Beq | Op::Bne | Op::Bmi | Op::Bpl));
            let tested = compare
                && branch
                && lines[..at]
                    .iter()
                    .rev()
                    .find(|before| !before.inst().is_some_and(|i| i.flags() == Flags::Keeps))
                    .and_then(Line::inst)
                    .is_some_and(sets_nz_of_a);
            !tested
        })
        .collect();
    retain(asm, &keep);
}

/// The lines of `asm` whose entry in `keep` is true.
fn retain(
    asm: &mut Asm,
    keep: &[bool],
) {
    let mut keep = keep.iter();
    asm.lines.retain(|_| *keep.next().unwrap());
}

/// `ldx #n` (likewise `lda` and `ldy`) of what the register already holds: it
/// goes, unless a branch reads the flags it sets. The values known are the
/// immediates loaded and the transfers between registers, forgotten at a
/// label, a call or an instruction not listed.
pub fn drop_known_loads(asm: &mut Asm) {
    let mut known: [Option<&str>; 3] = [None; 3];
    let mut keep = vec![true; asm.lines.len()];
    for (at, line) in asm.lines.iter().enumerate() {
        let Line::Inst(Inst { op, operand }) = line else {
            known = [None; 3];
            continue;
        };
        let register = match op {
            Op::Lda => Some(0),
            Op::Ldx => Some(1),
            Op::Ldy => Some(2),
            _ => None,
        };
        let on_a = matches!(operand, Operand::None | Operand::Acc);
        match (op, register, operand) {
            (_, Some(register), Operand::Imm(value)) => {
                if known[register] == Some(value) && flags_unread_after(&asm.lines, at + 1) {
                    keep[at] = false;
                    continue;
                }
                known[register] = Some(value);
            }
            (Op::Lda | Op::Pla | Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor, ..) => known[0] = None,
            (Op::Asl | Op::Lsr | Op::Rol | Op::Ror | Op::Inc | Op::Dec, _, _) if on_a => known[0] = None,
            (Op::Ldx | Op::Inx | Op::Dex, ..) => known[1] = None,
            (Op::Ldy | Op::Iny | Op::Dey, ..) => known[2] = None,
            (Op::Txa, ..) => known[0] = known[1],
            (Op::Tya, ..) => known[0] = known[2],
            (Op::Tax, ..) => known[1] = known[0],
            (Op::Tay, ..) => known[2] = known[0],
            // a register's value survives what neither writes it nor leaves
            // the function: stores, compares, branches, memory
            // read-modify-write
            (Op::Cmp | Op::Cpx | Op::Cpy | Op::Bit | Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror, ..) => {
            }
            _ if line.inst().is_some_and(|i| i.flags() == Flags::Keeps) => {}
            _ if op.opposite().is_some() => {}
            _ => known = [None; 3],
        }
    }
    retain(asm, &keep);
}

/// Whether `line` changes neither a register nor the cell `load` reads: a
/// carry flag op, or a store to another zero-page or spill cell.
fn leaves_cell_and_registers(
    line: &Line,
    load: &Operand,
) -> bool {
    let Line::Inst(Inst { op, operand }) = line else { return false };
    match op {
        Op::Clc | Op::Sec | Op::Nop => true,
        Op::Sta | Op::Stx | Op::Sty => operand.is_cell() && operand != load,
        _ => false,
    }
}

/// `sta m` then `lda m` (likewise `x` and `y`) of a zero-page or spill cell:
/// the register still holds it, so the load goes. It also set the zero and sign
/// flags, so it stays when a branch reads them before anything sets them again.
/// A carry flag op or a store to another cell between the two changes neither.
pub fn drop_reloads(asm: &mut Asm) {
    let lines = &asm.lines;
    let keep: Vec<bool> = (0..lines.len())
        .map(|at| {
            let Line::Inst(Inst { op: load, operand }) = &lines[at] else { return true };
            let store = match load {
                Op::Lda => Op::Sta,
                Op::Ldx => Op::Stx,
                Op::Ldy => Op::Sty,
                _ => return true,
            };
            let reload = operand.is_cell()
                && lines[..at]
                    .iter()
                    .rev()
                    .find(|before| !leaves_cell_and_registers(before, operand))
                    .and_then(Line::inst)
                    .is_some_and(|before| before.op == store && before.operand == *operand)
                && flags_unread_after(lines, at + 1);
            !reload
        })
        .collect();
    retain(asm, &keep);
}

/// `tya; tay`, `tay; tya`, `txa; tax` and `tax; txa`: the second transfer
/// moves what the first just moved and sets the flags the first set.
pub fn drop_cancelling_transfers(asm: &mut Asm) {
    let mut out: Vec<Line> = Vec::with_capacity(asm.lines.len());
    let mut cancelled = false;
    for line in asm.lines.drain(..) {
        let cancels = !cancelled
            && match (out.last().and_then(Line::inst), &line) {
                (Some(Inst { op: before, .. }), Line::Inst(Inst { op, .. })) => {
                    matches!(
                        (before, op),
                        (Op::Tya, Op::Tay) | (Op::Tay, Op::Tya) | (Op::Txa, Op::Tax) | (Op::Tax, Op::Txa)
                    )
                }
                _ => false,
            };
        // the pair is gone: the next line is not paired with its first half
        cancelled = cancels;
        if !cancels {
            out.push(line);
        }
    }
    asm.lines = out;
}

/// `jmp l` where `l:` is directly followed by `rts` (past other labels): the
/// `rts` itself, one byte and one jump shorter.
pub fn jump_to_return(asm: &mut Asm) {
    let mut returns: HashSet<String> = HashSet::new();
    let mut returning = false;
    for line in asm.lines.iter().rev() {
        match line {
            Line::Inst(inst) => returning = inst.op == Op::Rts,
            Line::Label(label) => {
                if returning {
                    returns.insert(label.clone());
                }
            }
            Line::Raw(_) => returning = false,
        }
    }
    for line in &mut asm.lines {
        if matches!(
            line,
            Line::Inst(inst) if inst.op == Op::Jmp && inst.target().is_some_and(|l| returns.contains(l))
        ) {
            *line = Line::Inst(Inst::new(Op::Rts, Operand::None));
        }
    }
}
