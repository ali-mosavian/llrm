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
//! - [`drop_zero_compares`]: `cmp #0` (likewise `cpx`, `cpy`) before a branch
//!   on zero or sign, after an instruction that set those flags from that
//!   register. Labels between the two do not matter: a path that jumps to one
//!   brings its own flags.
//! - [`drop_cancelling_transfers`]: `tya; tay` and its three twins: the second
//!   moves what the first just moved.
//! - [`branch_on_shift_carry`]: `ldx m; bpl` after `asl|rol c` where `c` held
//!   what `m` holds: the shift's carry is `m`'s top bit, so `bcc` replaces both.
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

use crate::asm::{Asm, Flags, Index, Inst, Line, Op, Operand, Registers};

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
            Flags::Of(_) | Flags::Sets => return true,
            Flags::Reads => return false,
        }
    }
    false
}

/// Whether the emitted instruction leaves the zero and sign flags describing
/// what `a` now holds.
pub fn sets_nz_of_a(inst: &Inst) -> bool {
    sets_nz_of(inst, Registers::A)
}

/// Whether the instruction leaves the zero and sign flags describing what
/// `register` now holds.
fn sets_nz_of(
    inst: &Inst,
    register: Registers,
) -> bool {
    matches!(inst.flags(), Flags::Of(set) if set.contains(register))
}

/// The passes, in the order they feed each other.
pub fn run(asm: &mut Asm) {
    drop_reloads(asm);
    drop_known_loads(asm);
    drop_zero_compares(asm);
    drop_cancelling_transfers(asm);
    branch_on_shift_carry(asm);
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
            let compared = match &lines[at] {
                Line::Inst(Inst { op, operand: Operand::Imm(v) }) if v == "0" => match op {
                    Op::Cmp => Some(Registers::A),
                    Op::Cpx => Some(Registers::X),
                    Op::Cpy => Some(Registers::Y),
                    _ => None,
                },
                _ => None,
            };
            let branch = next_instruction(lines, at + 1)
                .is_some_and(|next| matches!(next.op, Op::Beq | Op::Bne | Op::Bmi | Op::Bpl));
            let tested = branch
                && compared.is_some_and(|register| {
                    lines[..at]
                        .iter()
                        .rev()
                        .find(|before| !before.inst().is_some_and(|i| i.flags() == Flags::Keeps))
                        .and_then(Line::inst)
                        .is_some_and(|before| sets_nz_of(before, register))
                });
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

/// Whether the instruction writes the zero-page or spill cell `cell`.
fn writes_cell(
    inst: &Inst,
    cell: &Operand,
) -> bool {
    matches!(
        inst.op,
        Op::Sta | Op::Stx | Op::Sty | Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror
    ) && inst.operand == *cell
}

/// Whether register `bit` (A, X or Y) is written before it is read, along every
/// path from `lines[from]`. A call, a return or an instruction not listed
/// counts as a read.
fn dead_from(
    lines: &[Line],
    from: usize,
    bit: Registers,
    seen: &mut HashSet<usize>,
) -> bool {
    let mut at = from;
    while let Some(line) = lines.get(at) {
        if !seen.insert(at) {
            return true;
        }
        at += 1;
        let Line::Inst(inst) = line else {
            if matches!(line, Line::Raw(_)) {
                return false;
            }
            continue;
        };
        let indexed = |index| match (&inst.operand, index) {
            (Operand::Indexed(_, i), _) => *i == index,
            (Operand::IndirectX(_), Index::X) | (Operand::IndirectY(_), Index::Y) => true,
            _ => false,
        };
        let (reads, kills) = match (inst.op, bit) {
            (Op::Lda | Op::Pla, Registers::A) => (false, true),
            (Op::Ldx | Op::Plx, Registers::X) => (indexed(Index::X), true),
            (Op::Ldy | Op::Ply, Registers::Y) => (indexed(Index::Y), true),
            (Op::Tax | Op::Tay | Op::Txa | Op::Tya, _) => {
                let (from, to) = match inst.op {
                    Op::Tax => (Registers::A, Registers::X),
                    Op::Tay => (Registers::A, Registers::Y),
                    Op::Txa => (Registers::X, Registers::A),
                    _ => (Registers::Y, Registers::A),
                };
                (bit == from, bit == to)
            }
            (Op::Sta | Op::Stx | Op::Sty | Op::Clc | Op::Sec | Op::Nop, _) => {
                let stored = match inst.op {
                    Op::Sta => Some(Registers::A),
                    Op::Stx => Some(Registers::X),
                    Op::Sty => Some(Registers::Y),
                    _ => None,
                };
                (stored == Some(bit) || indexed(if bit == Registers::X { Index::X } else { Index::Y }), false)
            }
            (Op::Cpx | Op::Inx | Op::Dex, Registers::X) | (Op::Cpy | Op::Iny | Op::Dey, Registers::Y) => (true, false),
            (Op::Adc | Op::And | Op::Ora | Op::Eor | Op::Sbc | Op::Cmp | Op::Pha, Registers::A) => {
                (true, false)
            }
            (Op::Asl | Op::Lsr | Op::Rol | Op::Ror | Op::Inc | Op::Dec, Registers::A) => {
                (matches!(inst.operand, Operand::Acc | Operand::None), false)
            }
            (
                Op::Adc
                | Op::And
                | Op::Ora
                | Op::Eor
                | Op::Sbc
                | Op::Cmp
                | Op::Bit
                | Op::Asl
                | Op::Lsr
                | Op::Rol
                | Op::Ror
                | Op::Inc
                | Op::Dec
                | Op::Pha
                | Op::Php
                | Op::Cld
                | Op::Sed
                | Op::Clv,
                _,
            ) => (indexed(if bit == Registers::X { Index::X } else { Index::Y }), false),
            (Op::Lda | Op::Ldx | Op::Ldy, _) => {
                (indexed(if bit == Registers::X { Index::X } else { Index::Y }), false)
            }
            (Op::Jmp, _) => {
                return inst.target().is_some_and(|label| {
                    lines.iter().position(|l| *l == Line::Label(label.to_owned())).is_some_and(|to| {
                        dead_from(lines, to, bit, seen)
                    })
                });
            }
            (op, _) if op.opposite().is_some() || op == Op::Bra => {
                let target = inst.target().and_then(|label| lines.iter().position(|l| *l == Line::Label(label.to_owned())));
                let taken = target.is_some_and(|to| dead_from(lines, to, bit, seen));
                if op == Op::Bra {
                    return taken;
                }
                if !taken {
                    return false;
                }
                continue;
            }
            _ => return false,
        };
        if reads {
            return false;
        }
        if kills {
            return true;
        }
    }
    false
}

/// What a sign test reads: a cell, or a register.
#[derive(PartialEq)]
enum Source<'a> {
    Cell(&'a Operand),
    Register(Registers),
}

/// Whether the instruction writes register `bit`.
fn writes_register(
    inst: &Inst,
    bit: Registers,
) -> bool {
    let acc = matches!(inst.operand, Operand::Acc | Operand::None);
    match inst.op {
        Op::Lda | Op::Pla | Op::Txa | Op::Tya | Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor => bit == Registers::A,
        Op::Asl | Op::Lsr | Op::Rol | Op::Ror | Op::Inc | Op::Dec => acc && bit == Registers::A,
        Op::Ldx | Op::Plx | Op::Tax | Op::Inx | Op::Dex | Op::Tsx => bit == Registers::X,
        Op::Ldy | Op::Ply | Op::Tay | Op::Iny | Op::Dey => bit == Registers::Y,
        _ => false,
    }
}

/// `asl|rol c` and then, with only loads, transfers and logic between, a
/// branch on the sign of what `c` held before it: `c` was a copy of a cell
/// (`ldx m; stx c`) or of a register (`stx c`) that has not changed since. The
/// shift put that top bit in the carry, so `bcc` replaces `bpl` (`bcs` for
/// `bmi`) and the test (`ldx m`, `txa`, ...) goes, when the register it set is
/// written again before anything reads it.
///
/// DEVIATE: neither LLVM's pin nor llvm-mos folds a sign test of a value into
/// the carry of its own shift (the pin has ARM Thumb1's `LSLS` compare and
/// x86's carry of `add x,-1`, both narrower). Exit: a target-independent
/// shift-with-carry-out in the MIR, so the select that tests bit 15 before
/// `c << 1` branches on the shift's carry.
pub fn branch_on_shift_carry(asm: &mut Asm) {
    let mut at = 2;
    while at < asm.lines.len() {
        let rewrite = (|| {
            let Line::Inst(branch) = &asm.lines[at] else { return None };
            let carry = match branch.op {
                Op::Bpl => Op::Bcc,
                Op::Bmi => Op::Bcs,
                _ => return None,
            };
            let Line::Inst(Inst { op: test, operand }) = &asm.lines[at - 1] else { return None };
            // what the test reads, and the register it sets
            let (source, tested) = match test {
                Op::Lda | Op::Ldx | Op::Ldy if operand.is_cell() => {
                    (Source::Cell(operand), if *test == Op::Lda { Registers::A } else if *test == Op::Ldx { Registers::X } else { Registers::Y })
                }
                Op::Tax => (Source::Register(Registers::A), Registers::X),
                Op::Tay => (Source::Register(Registers::A), Registers::Y),
                Op::Txa => (Source::Register(Registers::X), Registers::A),
                Op::Tya => (Source::Register(Registers::Y), Registers::A),
                _ => return None,
            };
            let unchanged = |inst: &Inst| match &source {
                Source::Cell(cell) => !writes_cell(inst, cell),
                Source::Register(bit) => !writes_register(inst, *bit),
            };
            // the shift, past instructions that leave the carry and the source alone
            let mut shift = at - 1;
            let copy = loop {
                shift = shift.checked_sub(1)?;
                let Line::Inst(before) = &asm.lines[shift] else { return None };
                match before.op {
                    Op::Asl | Op::Rol if before.operand.is_cell() => break &before.operand,
                    Op::Lda | Op::Ldx | Op::Ldy | Op::Tax | Op::Tay | Op::Txa | Op::Tya | Op::Eor | Op::And
                    | Op::Ora | Op::Nop | Op::Inx | Op::Iny | Op::Dex | Op::Dey | Op::Sta | Op::Stx | Op::Sty
                        if unchanged(before) => {}
                    _ => return None,
                }
            };
            if matches!(&source, Source::Cell(cell) if *cell == copy) {
                return None;
            }
            // `copy` held the source before the shift
            let mut back = shift;
            let copied = loop {
                back -= 1;
                let Line::Inst(before) = &asm.lines[back] else { return None };
                if writes_cell(before, copy) {
                    let (stored, load) = match before.op {
                        Op::Sta => (Registers::A, Op::Lda),
                        Op::Stx => (Registers::X, Op::Ldx),
                        Op::Sty => (Registers::Y, Op::Ldy),
                        _ => return None,
                    };
                    break match &source {
                        Source::Register(bit) => *bit == stored,
                        Source::Cell(cell) => {
                            matches!(asm.lines.get(back.checked_sub(1)?)?, Line::Inst(source)
                                if source.op == load && source.operand == **cell)
                        }
                    };
                }
                if before.op == Op::Jsr || before.op.is_branch() || !unchanged(before) {
                    return None;
                }
            };
            (copied && dead_from(&asm.lines, at + 1, tested, &mut HashSet::new())).then_some(carry)
        })();
        if let Some(carry) = rewrite {
            let Line::Inst(branch) = &mut asm.lines[at] else { unreachable!() };
            branch.op = carry;
            asm.lines.remove(at - 1);
        } else {
            at += 1;
        }
    }
}
