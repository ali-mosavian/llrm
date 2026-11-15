//! A `jmp` taken where a flag is known: a two-byte branch.
//!
//! # How it works
//! The flags a `jmp` is reached with are known in two cases: the branch before
//! it fell through (`bne x; jmp t`: the zero flag is set), or a load of a
//! constant set them (`ldy #0; jmp t`: zero). The opposite branch to `t` is
//! then always taken, one byte shorter and as fast. A constant load of an index
//! register moves down to the `jmp` past straight instructions that leave that
//! register alone, when the flags are written again at `t` before they are
//! read.
//!
//! The target must be within reach with room to spare, as it runs on the
//! relaxed text and a branch that missed would be long.
//!
//! DEVIATE 6502-TAIL-MERGE (owner 6502, exit 2026-11-24): LLVM keeps the
//! unconditional branch and never reads the flags a fall-through leaves; on the
//! 6502 `jmp` is three bytes against a branch's two and the flags are free.
//! Exit: D-6502 of #6259 moves it to the shared post-allocation jump
//! optimisation (`jumps/branchfold`), where the flag fact is a target hook.
//!
//! # Example
//! ```text
//!   ldy #0              ldy #0
//!   lda rc3             lda rc3
//!   sta rc7        =>   sta rc7
//!   jmp t               beq t   (ldy moved down: `ldy` is the last to set Z)
//! ```
//!
//! # Runs
//! [`crate::peephole::after_layout`], after [`crate::tail_merge`].

use llrm_support::hash::HashMap;

use crate::asm::{Asm, Index, Inst, Line, Op, Operand, Registers};

/// The most bytes between a branch and its target for it to be chosen.
const REACH: usize = 100;

/// A literal constant: `#0`, `#$ff`, `#12`.
fn literal(operand: &Operand) -> Option<u32> {
    let Operand::Imm(text) = operand else { return None };
    match text.strip_prefix('$') {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

/// Whether the instruction reads or writes the index register `register`.
fn uses_index(
    inst: &Inst,
    register: Registers,
) -> bool {
    let (index, ops): (Index, &[Op]) = if register == Registers::Y {
        (Index::Y, &[Op::Ldy, Op::Sty, Op::Cpy, Op::Iny, Op::Dey, Op::Tay, Op::Tya])
    } else {
        (Index::X, &[Op::Ldx, Op::Stx, Op::Cpx, Op::Inx, Op::Dex, Op::Tax, Op::Txa])
    };
    ops.contains(&inst.op)
        || match &inst.operand {
            Operand::Indexed(_, at) => *at == index,
            Operand::IndirectY(_) => index == Index::Y,
            Operand::IndirectX(_) => index == Index::X,
            _ => false,
        }
}

/// Whether the instruction is straight work that reads no zero or sign flag.
fn plain(op: Op) -> bool {
    use Op::*;
    matches!(
        op,
        Adc | And
            | Asl
            | Clc
            | Cmp
            | Cpx
            | Cpy
            | Dec
            | Eor
            | Inc
            | Lda
            | Ldx
            | Ldy
            | Lsr
            | Nop
            | Ora
            | Rol
            | Ror
            | Sbc
            | Sec
            | Sta
            | Stx
            | Sty
            | Stz
            | Tax
            | Tay
            | Txa
            | Tya
            | Dex
            | Dey
            | Inx
            | Iny
    )
}

/// The branch that jumps where the flags say, if they are known at `lines[at]`
/// (a `jmp`): the opposite of the fall-through branch before it, or the zero
/// test of the constant load just before.
fn known_branch(
    lines: &[Line],
    at: usize,
) -> Option<Op> {
    let mut back = at;
    while back > 0 {
        back -= 1;
        let Line::Inst(inst) = &lines[back] else { return None };
        if let Some(opposite) = inst.op.opposite() {
            // the branch before fell through: its flag has the other value
            return (back + 1 == at).then_some(opposite);
        }
        match inst.op {
            Op::Lda | Op::Ldx | Op::Ldy => {
                return literal(&inst.operand).map(|value| if value == 0 { Op::Beq } else { Op::Bne });
            }
            Op::Sta | Op::Stx | Op::Sty | Op::Stz | Op::Clc | Op::Sec | Op::Nop => {}
            _ => return None,
        }
    }
    None
}

/// The bytes between the line `from` and `to`, taken as 3 for what is not an
/// instruction.
fn bytes_between(
    lines: &[Line],
    from: usize,
    to: usize,
) -> usize {
    lines[from.min(to)..from.max(to)]
        .iter()
        .map(|line| match line {
            Line::Label(_) => 0,
            Line::Inst(inst) => inst.bytes(),
            Line::Raw(_) => 3,
        })
        .sum()
}

/// The constant load of an index register before `lines[at]` (a `jmp`) that
/// only straight work that never touches that register separates from it.
fn index_load_above(
    lines: &[Line],
    at: usize,
) -> Option<usize> {
    let mut back = at;
    while back > 0 {
        back -= 1;
        let Line::Inst(inst) = &lines[back] else { return None };
        if matches!(inst.op, Op::Ldx | Op::Ldy) && literal(&inst.operand).is_some() {
            return Some(back);
        }
        if !plain(inst.op) {
            return None;
        }
    }
    None
}

/// Each `jmp` taken where a flag is known, as the branch on it.
pub fn jump_to_branch(asm: &mut Asm) -> bool {
    // no label moves: a load moved down passes no label
    let labels: HashMap<String, usize> = asm
        .lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| match line {
            Line::Label(name) => Some((name.clone(), at)),
            _ => None,
        })
        .collect();
    let mut at = 0;
    let mut changed = false;
    while at < asm.lines.len() {
        let here = at;
        at += 1;
        let Line::Inst(jump) = &asm.lines[here] else { continue };
        if jump.op != Op::Jmp {
            continue;
        }
        let Some(&target) = jump.target().and_then(|label| labels.get(label)) else { continue };
        if bytes_between(&asm.lines, here + 1, target) > REACH {
            continue;
        }
        let mut branch = known_branch(&asm.lines, here);
        if branch.is_none() {
            // the load of the constant moves down to the jump
            let Some(load) = index_load_above(&asm.lines, here) else { continue };
            let Line::Inst(loaded) = &asm.lines[load] else { continue };
            let register = if loaded.op == Op::Ldy { Registers::Y } else { Registers::X };
            let between_free = asm
                .lines[load + 1..here]
                .iter()
                .all(
                    |line| matches!(
                        line,
                        Line::Inst(inst) if plain(inst.op) && !uses_index(inst, register)
                    ),
                );
            if !between_free || !crate::peephole::flags_unread_after(&asm.lines, target) {
                continue;
            }
            let moved = asm.lines.remove(load);
            asm.lines.insert(here - 1, moved);
            branch = known_branch(&asm.lines, here);
        }
        let Some(branch) = branch else { continue };
        let Line::Inst(jump) = &asm.lines[here] else { continue };
        let operand = jump.operand.clone();
        asm.lines[here] = Line::Inst(Inst::new(branch, operand));
        changed = true;
    }
    changed
}
