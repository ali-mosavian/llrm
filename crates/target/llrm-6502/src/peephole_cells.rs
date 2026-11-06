//! Cells a function leaves dead at its return, and the shifts of a word that
//! end in `a`.
//!
//! - [`drop_dead_stores`]: `sta rc2` (likewise `stx`, `sty`) of a caller-saved
//!   cell that nothing reads before an `rts` that does not return it.
//! - [`store_then_index_load`]: `sta m; lda n; ldx m` is `tax; lda n` when `m`
//!   is dead after (likewise `ldy m`): the byte went through memory to leave
//!   `a`.
//! - [`chain_ends_in_a`]: `(asl m; rol n)*k; lda m` (also `lsr`/`ror`, and `lda
//!   n`) is `lda m; (asl a; rol n)*k` when the loaded byte is dead after: the
//!   byte that is returned shifts in `a`, not in memory and then loaded.
//!
//! Runs from [`crate::peephole::run`].

use std::collections::BTreeSet;

use crate::asm::{Asm, Index, Inst, Line, Op, Operand};

fn cell_number(name: &str) -> Option<u8> {
    name.strip_prefix("rc")?.parse().ok()
}

/// The caller's cells at a return: not the soft stack pointer (`rc0`, `rc1`)
/// and not the callee-saved `rc8`..`rc15`.
fn caller_saved(n: u8) -> bool {
    n >= 2 && !(8..=15).contains(&n)
}

fn straight(op: Op) -> bool {
    use Op::*;
    matches!(
        op,
        Adc | And
            | Asl
            | Bit
            | Clc
            | Cmp
            | Cpx
            | Cpy
            | Dec
            | Dex
            | Dey
            | Eor
            | Inc
            | Inx
            | Iny
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
    )
}

/// Whether the cell is written before it is read, or reaches an `rts` that does
/// not return it, along the straight-line code from `lines[from]`. A label, a
/// branch, a call or an instruction not listed is not known to leave it dead.
fn cell_dead_from(
    lines: &[Line],
    from: usize,
    cell: &str,
    results: &BTreeSet<String>,
) -> bool {
    let Some(n) = cell_number(cell) else { return false };
    if !caller_saved(n) {
        return false;
    }
    for line in &lines[from..] {
        let Line::Inst(inst) = line else { return false };
        if inst.op == Op::Rts {
            return !results.contains(cell);
        }
        if !straight(inst.op) {
            return false;
        }
        match &inst.operand {
            Operand::Direct(c) if c == cell => return matches!(inst.op, Op::Sta | Op::Stx | Op::Sty | Op::Stz),
            // an index reaches the cells above its base, a pointer reads two
            Operand::Indexed(c, _) if cell_number(c).is_some() => return false,
            Operand::IndirectY(c) | Operand::IndirectX(c) | Operand::Indirect(c)
                if cell_number(c).is_some_and(|m| m == n || m + 1 == n) =>
            {
                return false;
            }
            _ => {}
        }
    }
    false
}

pub fn drop_dead_stores(asm: &mut Asm) {
    let mut at = 0;
    while at < asm.lines.len() {
        if let Line::Inst(Inst { op: Op::Sta | Op::Stx | Op::Sty, operand: Operand::Direct(cell), .. }) = &asm.lines[at]
            && cell_dead_from(&asm.lines, at + 1, cell, &asm.result_cells)
        {
            asm.lines.remove(at);
            continue;
        }
        at += 1;
    }
}

/// Whether the flags are set again before a read, from `lines[from]`.
fn flags_unread(
    lines: &[Line],
    from: usize,
) -> bool {
    crate::peephole::flags_unread_after(lines, from)
}

pub fn store_then_index_load(asm: &mut Asm) {
    while store_then_index_once(asm) {}
}

fn store_then_index_once(asm: &mut Asm) -> bool {
    let lines = &asm.lines;
    for (s, line) in lines.iter().enumerate() {
        let Line::Inst(Inst { op: Op::Sta, operand: Operand::Direct(m), .. }) = line else { continue };
        for (load, transfer, index) in [(Op::Ldx, Op::Tax, Index::X), (Op::Ldy, Op::Tay, Index::Y)] {
            let mut found = None;
            for (t, line) in lines.iter().enumerate().skip(s + 1) {
                let Line::Inst(Inst { op, operand, .. }) = line else { break };
                if *op == load && *operand == Operand::Direct(m.clone()) {
                    found = Some(t);
                    break;
                }
                let own_cell = matches!(operand, Operand::Direct(c) if c == m);
                let other_index = if index == Index::X { (Op::Ldy, Op::Sty) } else { (Op::Ldx, Op::Stx) };
                let quiet = !own_cell
                    && match op {
                        Op::Clc | Op::Sec | Op::Nop => true,
                        Op::Lda | Op::Sta => matches!(operand, Operand::Direct(_) | Operand::Imm(_)),
                        o if *o == other_index.0 || *o == other_index.1 => {
                            matches!(operand, Operand::Direct(_) | Operand::Imm(_))
                        }
                        _ => false,
                    };
                if !quiet {
                    break;
                }
            }
            let Some(t) = found else { continue };
            if cell_dead_from(lines, t + 1, m, &asm.result_cells) && flags_unread(lines, t + 1) {
                asm.lines[s] = Line::Inst(Inst { op: transfer, operand: Operand::None, place: 0 });
                asm.lines.remove(t);
                return true;
            }
        }
    }
    false
}

pub fn chain_ends_in_a(asm: &mut Asm) {
    while chain_once(asm) {}
}

/// The step `first m; second n` at `lines[at]`: `asl`/`rol` or `lsr`/`ror` of
/// two cells.
fn step(
    lines: &[Line],
    at: usize,
) -> Option<(Op, Op, String, String)> {
    let (Line::Inst(a), Line::Inst(b)) = (lines.get(at)?, lines.get(at + 1)?) else { return None };
    let (Operand::Direct(m), Operand::Direct(n)) = (&a.operand, &b.operand) else { return None };
    matches!((a.op, b.op), (Op::Asl, Op::Rol) | (Op::Lsr, Op::Ror)).then(|| (a.op, b.op, m.clone(), n.clone()))
}

fn chain_once(asm: &mut Asm) -> bool {
    let lines = &asm.lines;
    for s in 0..lines.len() {
        let Some((first, second, m, n)) = step(lines, s) else { continue };
        if m == n {
            continue;
        }
        // the run's first step: the line before is not the same step
        if s >= 2 && step(lines, s - 2) == Some((first, second, m.clone(), n.clone())) {
            continue;
        }
        let mut end = s;
        while step(lines, end) == Some((first, second, m.clone(), n.clone())) {
            end += 2;
        }
        let Some(Line::Inst(Inst { op: Op::Lda, operand: Operand::Direct(x), .. })) = lines.get(end) else { continue };
        if x != &m && x != &n {
            continue;
        }
        if !cell_dead_from(lines, end + 1, x, &asm.result_cells) || !flags_unread(lines, end + 1) {
            continue;
        }
        let in_a = |op: Op, cell: &String| {
            let operand = if cell == x { Operand::Acc } else { Operand::Direct(cell.clone()) };
            Line::Inst(Inst { op, operand, place: 0 })
        };
        let mut out: Vec<Line> = lines[..s].to_vec();
        out.push(lines[end].clone());
        for _ in 0..(end - s) / 2 {
            out.push(in_a(first, &m));
            out.push(in_a(second, &n));
        }
        out.extend_from_slice(&lines[end + 1..]);
        asm.lines = out;
        return true;
    }
    false
}

/// The step of an arithmetic shift right of a word on two cells at `lines[at]`:
/// `lda h; cmp #$80; ror h; ror l`.
fn sra_step(
    lines: &[Line],
    at: usize,
) -> Option<(String, String)> {
    let inst = |n: usize| match lines.get(at + n) {
        Some(Line::Inst(inst)) => Some(inst),
        _ => None,
    };
    let (load, compare, high, low) = (inst(0)?, inst(1)?, inst(2)?, inst(3)?);
    let (Operand::Direct(h), Operand::Imm(sign), Operand::Direct(h2), Operand::Direct(l)) =
        (&load.operand, &compare.operand, &high.operand, &low.operand)
    else {
        return None;
    };
    (load.op == Op::Lda
        && compare.op == Op::Cmp
        && sign == "$80"
        && high.op == Op::Ror
        && low.op == Op::Ror
        && h == h2
        && h != l)
        .then(|| (h.clone(), l.clone()))
}

fn bytes_of(lines: &[Line]) -> Option<u32> {
    let mut total = 0;
    for line in lines {
        if let Line::Inst(inst) = line {
            total += crate::timing::instruction(
                crate::dagisel::hooks::Cpu::Mos6502,
                inst.op.name(),
                &inst.operand.to_string(),
            )?
            .bytes;
        }
    }
    Some(total)
}

/// `(lda h; cmp #$80; ror h; ror l)*k` keeps the high byte in `a` through the
/// run: `lda h; (cmp #$80; ror a; ror l)*k; sta h`, 8 bytes a step down to 5.
/// Taken when the stores and loads the other passes then drop leave it
/// shorter (a run of one is a byte longer where `h` is read again).
pub fn arithmetic_chain_in_a(asm: &mut Asm) {
    let mut s = 0;
    while s < asm.lines.len() {
        let Some((h, l)) = sra_step(&asm.lines, s) else {
            s += 1;
            continue;
        };
        let mut end = s;
        while sra_step(&asm.lines, end) == Some((h.clone(), l.clone())) {
            end += 4;
        }
        let inst = |op, operand| Line::Inst(Inst { op, operand, place: 0 });
        let mut candidate = Asm::new();
        candidate.lines = asm.lines[..s].to_vec();
        candidate.result_cells = asm.result_cells.clone();
        candidate.lines.push(inst(Op::Lda, Operand::Direct(h.clone())));
        for _ in 0..(end - s) / 4 {
            candidate.lines.push(inst(Op::Cmp, Operand::Imm("$80".into())));
            candidate.lines.push(inst(Op::Ror, Operand::Acc));
            candidate.lines.push(inst(Op::Ror, Operand::Direct(l.clone())));
        }
        candidate.lines.push(inst(Op::Sta, Operand::Direct(h.clone())));
        let rewritten = candidate.lines.len();
        candidate.lines.extend_from_slice(&asm.lines[end..]);
        drop_dead_stores(&mut candidate);
        store_then_index_load(&mut candidate);
        match (bytes_of(&asm.lines), bytes_of(&candidate.lines)) {
            (Some(before), Some(after)) if after < before => {
                asm.lines = candidate.lines;
                s += rewritten.min(asm.lines.len());
            }
            _ => s = end,
        }
    }
}
