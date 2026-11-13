//! Cells a function leaves dead at its return, and the shifts of a word that
//! end in `a`.
//!
//! - [`drop_dead_stores`]: `sta rc2` (likewise `stx`, `sty`, and the 65C02's
//!   `stz`) of a caller-saved cell that nothing reads before an `rts` that does
//!   not return it.
//! - [`store_then_index_load`]: `sta m; lda n; ldx m` is `tax; lda n` when `m`
//!   is dead after (likewise `ldy m`): the byte went through memory to leave
//!   `a`.
//! - [`fold_temp_operand`]: `lda x; sta t; lda y; [clc|sec]; adc t` (likewise
//!   `sbc`, `and`, `ora`, `eor`, `cmp`) is `lda y; [clc|sec]; adc x` when `t`
//!   is dead after: the first operand goes to the instruction straight from
//!   where it was loaded (a memory cell, `(p),y`, an indexed byte), not through
//!   `a` and a temporary.
//! - [`carry_stays_in_the_flag`]: `lda #0; rol a; tax` (or `sta m`), work that
//!   leaves the carry and the saved byte alone, `txa` (or `lda m`), `lsr a` is
//!   the work alone when `a` and the byte are dead after: a borrow chain that
//!   ran through loads of its next operand carried its bit in a register for
//!   nothing (a compare of two longs: 5 instructions at each word).
//! - [`chain_ends_in_a`]: `(asl m; rol n)*k; [clc]; lda m` (also `lsr`/`ror`,
//!   and `lda n`) is `lda m; (asl a; rol n)*k` when the loaded byte is dead
//!   after: the byte that is returned shifts in `a`, not in memory and then
//!   loaded.
//!
//! - [`forward_cell_copy`]: `lda s; sta t; ...; lda t` is `lda s; ...; lda s`
//!   (likewise the other loads and the ALU instructions) when nothing between
//!   writes `s` or `t` and the copy `t` is dead after: a value copied to a cell
//!   to be loaded again, one instruction early, costs the store and the cell.
//!
//! - [`fold_spill_copy`]: `ldy m; sty t; ...; sbc t` (likewise `lda`, `ldx` and
//!   the other readers) reads `m` when `m` is unchanged on every path to the
//!   reads and `t` is written again before another read: a spilled byte
//!   reloaded into a cell for its readers.
//!
//! Runs from [`crate::peephole::run`].

use std::collections::BTreeSet;

use crate::asm::{Asm, Index, Inst, Line, Op, Operand, Registers};
use crate::peephole::{Exits, dead_from};

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
            | Pla
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
/// not return it, along every path from `lines[from]`: a branch is followed
/// both ways, a `jmp` to its label, and a path that comes back to a line seen
/// reads nothing new. A call, a raw line or an instruction not listed is not
/// known to leave it dead.
///
/// A scratch cell (`rc16`..) is clobbered by a call, which reads none of them:
/// a store followed by a `jsr` before any read is dead.
///
/// A callee-saved cell (`rc8`..`rc15`) is the caller's at the return: a store
/// to it is dead only when a write to it (the epilogue's restore) comes before
/// every `rts`, and a call neither reads nor changes it (the callee saves it on
/// entry and restores it), so the scan goes on past a `jsr`.
/// `float_conversions_loop` kept a `ldy #0; sty rc10` that only the restore
/// ever followed. A `pla` is straight (the epilogue's restore); a `pha` is not:
/// `pha; pha; rts` is a call through a pointer, and the `rts` there reads the
/// argument cells (`function_table`, `static_pointers` miscompiled by 3 B of
/// stores).
///
/// The pin's analogue is the liveness of a physical register at a return
/// (`DeadMachineInstructionElim.cpp`, `LivePhysRegs`): a def with no use on any
/// path out is dead.
fn cell_dead_from(
    lines: &[Line],
    from: usize,
    cell: &str,
    results: &BTreeSet<String>,
    seen: &mut BTreeSet<usize>,
) -> bool {
    let Some(n) = cell_number(cell) else { return false };
    // the soft stack pointer is read by every frame access
    if n < 2 {
        return false;
    }
    let callee_saved = !caller_saved(n);
    let mut at = from;
    while let Some(line) = lines.get(at) {
        if !seen.insert(at) {
            return true;
        }
        at += 1;
        let inst = match line {
            Line::Label(_) => continue,
            Line::Raw(_) => return false,
            Line::Inst(inst) => inst,
        };
        if inst.op == Op::Rts {
            return !callee_saved && !results.contains(cell);
        }
        if inst.op == Op::Jsr && callee_saved {
            continue;
        }
        // a call clobbers the scratch cells above the callee-saved ones and
        // reads none of them
        if inst.op == Op::Jsr && n >= 16 {
            return true;
        }
        // a call of a routine that names no cell reads none of them
        if inst.op == Op::Jsr
            && matches!(
                &inst.operand,
                Operand::Direct(routine) if crate::routines::leaves_cells_alone(routine)
            )
        {
            continue;
        }
        let label = |inst: &Inst| {
            let name = inst.target()?;
            lines.iter().position(|l| *l == Line::Label(name.to_owned()))
        };
        if inst.op == Op::Jmp || inst.op == Op::Bra {
            return label(inst).is_some_and(|to| cell_dead_from(lines, to, cell, results, seen));
        }
        if inst.op.opposite().is_some() {
            if !label(inst).is_some_and(|to| cell_dead_from(lines, to, cell, results, seen)) {
                return false;
            }
            continue;
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
        if let Line::Inst(Inst { op: Op::Sta | Op::Stx | Op::Sty | Op::Stz, operand: Operand::Direct(cell), .. }) =
            &asm.lines[at]
            && cell_dead_from(&asm.lines, at + 1, cell, &asm.result_cells, &mut BTreeSet::new())
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
            if cell_dead_from(lines, t + 1, m, &asm.result_cells, &mut BTreeSet::new()) && flags_unread(lines, t + 1) {
                asm.lines[s] = Line::Inst(Inst { op: transfer, operand: Operand::None, place: 0 });
                asm.lines.remove(t);
                return true;
            }
        }
    }
    false
}

/// The ALU instructions that take a memory operand of every form `lda` does.
fn alu(op: Op) -> bool {
    matches!(op, Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor | Op::Cmp)
}

pub fn fold_temp_operand(asm: &mut Asm) {
    while temp_operand_once(asm) {}
}

fn temp_operand_once(asm: &mut Asm) -> bool {
    let lines = &asm.lines;
    for at in 0..lines.len().saturating_sub(3) {
        let (Line::Inst(first), Line::Inst(store), Line::Inst(second)) = (&lines[at], &lines[at + 1], &lines[at + 2])
        else {
            continue;
        };
        let Operand::Direct(t) = &store.operand else { continue };
        if first.op != Op::Lda || store.op != Op::Sta || second.op != Op::Lda || !stable(&first.operand) {
            continue;
        }
        // neither load reads the temporary, as a cell or as a pointer
        if first.operand.name() == Some(t.as_str()) || second.operand.name() == Some(t.as_str()) {
            continue;
        }
        let mut op_at = at + 3;
        while matches!(
            lines.get(op_at),
            Some(Line::Inst(Inst { op: Op::Clc | Op::Sec, .. }))
        ) {
            op_at += 1;
        }
        let Some(Line::Inst(user)) = lines.get(op_at) else { continue };
        if !alu(user.op) || user.operand != Operand::Direct(t.clone()) {
            continue;
        }
        if !cell_dead_from(lines, op_at + 1, t, &asm.result_cells, &mut BTreeSet::new()) {
            continue;
        }
        let operand = first.operand.clone();
        asm.lines[op_at] = Line::Inst(Inst { op: user.op, operand, place: user.place });
        asm.lines.drain(at..at + 2);
        return true;
    }
    false
}

/// A spill slot's name (`s_<function>_<n>`): a cell nothing else names, writes
/// through a pointer or reaches by another spelling (`buf+2+1` and `buf+3` are
/// the same byte).
fn spill_slot(name: &str) -> bool {
    name.strip_prefix("s_")
        .and_then(|rest| rest.rsplit_once('_'))
        .is_some_and(|(_, n)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        && !name.contains(['+', '-', '(', ' '])
}

/// DEVIATE SPILL-COPY-FOLD (beyond the pin; owner 6502, exit 2026-11-15): the
/// 2026-11-08 corpus A/B (mean over 4 pads, matched base 698373eaa2) lowers
/// bytes in every changed program and instructions in each cell below, whose
/// clocks are worse by page-crossing cliffs on one or two pads (the layouts of
/// the base itself differ by 20k): O2 div_constants_short +12247 (-88 B), Os
/// call_keeps +173 (-4 B), Os div_constants_short +45393 (-96 B).
pub fn fold_spill_copy(asm: &mut Asm) {
    let mut at = 0;
    while at + 1 < asm.lines.len() {
        if spill_copy_once(asm, at) {
            continue;
        }
        at += 1;
    }
}

/// `ld* m; st* t` at `at`, `t` a caller-saved cell: every read of `t` that
/// reaches from the copy reads `m` instead when `m` is unchanged on the way and
/// `t` is written again before anything else reads it. The store goes; the load
/// goes with it where its register is dead after.
fn spill_copy_once(
    asm: &mut Asm,
    at: usize,
) -> bool {
    let (Line::Inst(load), Line::Inst(store)) = (&asm.lines[at], &asm.lines[at + 1]) else { return false };
    let register = match (load.op, store.op) {
        (Op::Lda, Op::Sta) => Registers::A,
        (Op::Ldx, Op::Stx) => Registers::X,
        (Op::Ldy, Op::Sty) => Registers::Y,
        _ => return false,
    };
    let (Operand::Direct(s), Operand::Direct(t)) = (&load.operand, &store.operand) else { return false };
    let Some(n) = cell_number(t) else { return false };
    if s == t || !caller_saved(n) || !(cell_number(s).is_some() || spill_slot(s)) {
        return false;
    }
    let lines = &asm.lines;
    let covers = |c: &String| cell_number(c).is_some_and(|m| m == n || m + 1 == n);
    let covers_s = |c: &String| cell_number(c).zip(cell_number(s)).is_some_and(|(m, k)| m == k || m + 1 == k);
    let position = |name: &str| lines.iter().position(|l| *l == Line::Label(name.to_owned()));
    let mut reads = Vec::new();
    let mut arrivals: std::collections::BTreeMap<usize, usize> = Default::default();
    let mut seen = BTreeSet::new();
    let mut work = vec![at + 2];
    while let Some(mut p) = work.pop() {
        loop {
            if matches!(lines.get(p), Some(Line::Label(_))) {
                *arrivals.entry(p).or_default() += 1;
            }
            if !seen.insert(p) {
                break;
            }
            let inst = match lines.get(p) {
                Some(Line::Label(_)) => {
                    p += 1;
                    continue;
                }
                Some(Line::Inst(inst)) => inst,
                _ => return false,
            };
            p += 1;
            if inst.op == Op::Rts {
                if asm.result_cells.contains(t) {
                    return false;
                }
                break;
            }
            if matches!(inst.op, Op::Jmp | Op::Bra) {
                let Some(to) = inst.target().and_then(position) else { return false };
                work.push(to);
                break;
            }
            if inst.op.opposite().is_some() {
                let Some(to) = inst.target().and_then(position) else { return false };
                work.push(to);
                continue;
            }
            // a call clobbers a caller-saved cell and reads none above the
            // argument cells
            if inst.op == Op::Jsr {
                if n <= 7 {
                    return false;
                }
                break;
            }
            if !straight(inst.op) {
                return false;
            }
            match &inst.operand {
                Operand::Indexed(c, _) if cell_number(c).is_some() || c == s => return false,
                Operand::IndirectY(c) | Operand::IndirectX(c) | Operand::Indirect(c) if covers(c) || covers_s(c) => {
                    return false;
                }
                Operand::Direct(c) if c == t => match inst.op {
                    Op::Sta | Op::Stx | Op::Sty | Op::Stz => break,
                    o if reads_only(o) => reads.push(p - 1),
                    _ => return false,
                },
                Operand::Direct(c) if c == s && !reads_only(inst.op) => return false,
                _ => {}
            }
        }
    }
    // a label that other paths also reach, with `t` live there, joins another
    // value of `t`
    for (&label, &came) in &arrivals {
        let Line::Label(name) = &lines[label] else { continue };
        let refs = lines.iter().filter(|l| matches!(l, Line::Inst(i) if i.target() == Some(name.as_str()))).count();
        let falls = label > 0
            && matches!(
                &lines[label - 1],
                Line::Inst(i) if !matches!(i.op, Op::Jmp | Op::Bra | Op::Rts)
            )
            || label > 0 && matches!(&lines[label - 1], Line::Label(_));
        if came < refs + usize::from(falls) && !cell_dead_from(lines, label, t, &asm.result_cells, &mut BTreeSet::new())
        {
            return false;
        }
    }
    // a read of `t` the walk could not reach from here is no fold's business;
    // the store goes only if none remain
    let exits = Exits::of(asm);
    let dead_load = dead_from(lines, at + 2, register, &mut Default::default(), exits);
    if reads.is_empty() || reads.len() > if dead_load { 4 } else { 1 } {
        return false;
    }
    let source = s.clone();
    for p in reads {
        if let Line::Inst(inst) = &mut asm.lines[p] {
            inst.operand = Operand::Direct(source.clone());
        }
    }
    asm.lines.remove(at + 1);
    true
}

/// An operand a later instruction reads as the load did: the same bytes as
/// long as no store or index change comes between, which the three lines of the
/// pattern guarantee.
fn stable(operand: &Operand) -> bool {
    matches!(
        operand,
        Operand::Direct(_) | Operand::Imm(_) | Operand::Indexed(..) | Operand::IndirectY(_)
    )
}

/// Whether `operand` indexes by `x` while `x` holds the saved byte (`saved_in`
/// is no cell).
fn reads_x(
    operand: &Operand,
    saved_in: &Option<String>,
) -> bool {
    saved_in.is_none() && matches!(
        operand,
        Operand::Indexed(_, crate::asm::Index::X) | Operand::IndirectX(_)
    )
}

pub fn carry_stays_in_the_flag(asm: &mut Asm) {
    let mut at = 0;
    while at + 2 < asm.lines.len() {
        let [Line::Inst(zero), Line::Inst(rotate), Line::Inst(save)] = &asm.lines[at..at + 3] else {
            at += 1;
            continue;
        };
        let saved_in = match (&save.op, &save.operand) {
            (Op::Tax, _) => Some(None),
            (Op::Sta, Operand::Direct(m)) => Some(Some(m.clone())),
            _ => None,
        };
        let (Some(saved_in), true, true) = (
            saved_in,
            zero.op == Op::Lda && zero.operand == Operand::Imm("0".into()),
            rotate.op == Op::Rol && rotate.operand == Operand::Acc,
        ) else {
            at += 1;
            continue;
        };
        // work that changes neither the carry nor the byte in its place
        let mut end = at + 3;
        // `a` still holds the saved byte until a load replaces it
        let mut a_loaded = false;
        while let Some(Line::Inst(inst)) = asm.lines.get(end) {
            let writes_m = matches!(
                (&inst.operand, &saved_in),
                (Operand::Direct(c), Some(m)) if c == m
            );
            let restore = matches!(
                (&inst.op, &inst.operand, &saved_in),
                (Op::Lda, Operand::Direct(c), Some(m)) if c == m
            );
            let quiet = match inst.op {
                Op::Lda if restore => false,
                Op::Lda => {
                    a_loaded = true;
                    !reads_x(&inst.operand, &saved_in)
                }
                // a copy of `a` before a load is a copy of the byte
                Op::Tay => a_loaded && !writes_m,
                Op::Ldy | Op::Sty => !writes_m && !reads_x(&inst.operand, &saved_in),
                // `x` holds the byte when no cell does: storing it reads it
                Op::Stx => !writes_m && saved_in.is_some(),
                // `x` is free to change when the byte waits in a cell
                Op::Ldx => saved_in.is_some() && !writes_m,
                Op::Tax => saved_in.is_some() && a_loaded && !writes_m,
                Op::Sta => a_loaded && !writes_m && !reads_x(&inst.operand, &saved_in),
                _ => false,
            };
            if !quiet {
                break;
            }
            end += 1;
        }
        let (Some(Line::Inst(load)), Some(Line::Inst(shift))) = (asm.lines.get(end), asm.lines.get(end + 1)) else {
            at += 1;
            continue;
        };
        let restores = match &saved_in {
            None => load.op == Op::Txa,
            Some(m) => load.op == Op::Lda && load.operand == Operand::Direct(m.clone()),
        };
        // x must hold the byte in place: nothing between writes it
        let x_kept = true;
        if !restores || shift.op != Op::Lsr || shift.operand != Operand::Acc || !x_kept {
            at += 1;
            continue;
        }
        let exits = crate::peephole::Exits::of(asm);
        let a_dead =
            crate::peephole::dead_from(&asm.lines, end + 2, crate::asm::Registers::A, &mut Default::default(), exits);
        let byte_dead = match &saved_in {
            None => crate::peephole::dead_from(
                &asm.lines,
                end + 2,
                crate::asm::Registers::X,
                &mut Default::default(),
                exits,
            ),
            Some(m) => cell_dead_from(&asm.lines, end + 2, m, &asm.result_cells, &mut BTreeSet::new()),
        };
        // the `lsr` leaves zero and not-negative; the work's flags are what
        // stay
        if !a_dead || !byte_dead || !crate::peephole::flags_unread_after(&asm.lines, end + 2) {
            at += 1;
            continue;
        }
        asm.lines.drain(end..end + 2);
        asm.lines.drain(at..at + 3);
    }
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
        // a carry op between the chain and the load is kept after the chain
        let mut load = end;
        while matches!(
            lines.get(load),
            Some(Line::Inst(Inst { op: Op::Clc | Op::Sec, .. }))
        ) {
            load += 1;
        }
        let Some(Line::Inst(Inst { op: Op::Lda, operand: Operand::Direct(x), .. })) = lines.get(load) else { continue };
        if x != &m && x != &n {
            continue;
        }
        if !cell_dead_from(lines, load + 1, x, &asm.result_cells, &mut BTreeSet::new())
            || !flags_unread(lines, load + 1)
        {
            continue;
        }
        let in_a = |op: Op, cell: &String| {
            let operand = if cell == x { Operand::Acc } else { Operand::Direct(cell.clone()) };
            Line::Inst(Inst { op, operand, place: 0 })
        };
        let mut out: Vec<Line> = lines[..s].to_vec();
        out.push(lines[load].clone());
        for _ in 0..(end - s) / 2 {
            out.push(in_a(first, &m));
            out.push(in_a(second, &n));
        }
        out.extend_from_slice(&lines[end..load]);
        out.extend_from_slice(&lines[load + 1..]);
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

/// The bytes of the instructions in `text` (0 for a line that is not one).
pub fn bytes_of_text(text: &str) -> u32 {
    bytes_of(&Asm::parse(text).lines).unwrap_or(0)
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
        let mut candidate = asm.candidate_from(asm.lines[..s].to_vec());
        candidate.lines.push(inst(Op::Lda, Operand::Direct(h.clone())));
        for _ in 0..(end - s) / 4 {
            candidate.lines.push(inst(Op::Cmp, Operand::Imm("$80".into())));
            candidate.lines.push(inst(Op::Ror, Operand::Acc));
            candidate.lines.push(inst(Op::Ror, Operand::Direct(l.clone())));
        }
        candidate.lines.push(inst(Op::Sta, Operand::Direct(h.clone())));
        let rewritten = candidate.lines.len();
        // the original leaves the old high byte in `a`, the rewrite the shifted
        // one
        let exits = crate::peephole::Exits::of(asm);
        if !crate::peephole::dead_from(&asm.lines, end, crate::asm::Registers::A, &mut Default::default(), exits) {
            s = end;
            continue;
        }
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

thread_local! {
    /// The lines of every candidate the shift pass has built on this thread:
    /// the work counter its scaling test reads (wall time flakes on a loaded
    /// host).
    pub static SHIFT_CANDIDATE_LINES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// `ldx #k; L: asl m; rol n; dex; bne L` (likewise `ldy`/`dey`, `lsr`/`ror`)
/// is `lda m; (asl a; rol n)*k; sta m` when the dead stores and reloads the
/// other passes then drop leave it shorter. The counted loop costs a copy of
/// the shifted byte and, after it, a load; the run keeps the byte in `a`.
/// Taken only where `a`, the flags and the index register are dead after.
///
/// The cleanup and the size check run on the straight-line region around the
/// loop (no label, branch or return inside it), closed by a return that
/// reads every register and cell: nothing past the region is assumed dead,
/// and the cost of a candidate does not grow with the function (a whole-
/// function candidate took `select_of_constants` from 8 s to over 400 s).
///
/// DEVIATE SHIFT-LOOP-IN-A: the pin has no such choice (a shift by a constant
/// is one instruction there); this target picks the loop or the run at the
/// size level from the final text, like [`arithmetic_chain_in_a`] does for a
/// signed shift. The choice is by bytes, so it holds for every shape of
/// shift, not one multiplier. Owner 6502, exit 2026-11-15: price the in-`a`
/// run at emission, where the copy and the reload are known, and drop the
/// pass. The exit also recovers the whole-function form this region form
/// gives up (Os: `call_keeps` -11 B vs -8, `overflow_builtins` -18 vs -3,
/// `div_constants_short` -21 and `sat_builtins` -18 vs 0, `loop_interchange`
/// -6 vs -4): that needs liveness across blocks, which emission-time pricing
/// has and a region does not.
pub fn shift_loop_in_a(asm: &mut Asm) {
    let mut at = 0;
    let mut references = std::collections::HashMap::<String, usize>::new();
    for line in &asm.lines {
        if let Line::Inst(inst) = line {
            if let Some(label) = inst.target() {
                *references.entry(label.to_owned()).or_default() += 1;
            }
        }
    }
    while at < asm.lines.len() {
        match shift_loop(&asm.lines, at, &references) {
            Some((first, second, p, q, count)) => {
                let end = at + 6;
                let lines = &asm.lines;
                let exits = Exits::of(asm);
                let register = if matches!(&lines[at], Line::Inst(Inst { op: Op::Ldx, .. })) {
                    Registers::X
                } else {
                    Registers::Y
                };
                let free = dead_from(lines, end, Registers::A, &mut Default::default(), exits)
                    && dead_from(lines, end, register, &mut Default::default(), exits)
                    && flags_unread(lines, end);
                if free {
                    let ends_region = |line: &Line| match line {
                        Line::Inst(inst) => inst.target().is_some() || matches!(inst.op, Op::Rts | Op::Jmp),
                        _ => true,
                    };
                    let start = lines[..at].iter().rposition(ends_region).map_or(0, |i| i + 1);
                    let stop = lines[end..].iter().position(ends_region).map_or(lines.len(), |i| end + i);
                    // The region's end is read as it is: the function's own
                    // return keeps its meaning, and a
                    // branch or jump stays in the region, so the flags it
                    // reads and the registers of its unknown target stay live.
                    // Only a label (control falls through)
                    // is closed by a branch of our own.
                    let returns = matches!(lines.get(stop), Some(Line::Inst(Inst { op: Op::Rts, .. })));
                    let ends_in_inst = matches!(lines.get(stop), Some(Line::Inst(_)));
                    let stop = if ends_in_inst { stop + 1 } else { stop };
                    let inst = |op, operand| Line::Inst(Inst { op, operand, place: 0 });
                    let region = &lines[start..stop];
                    let mut candidate = lines[start..at].to_vec();
                    candidate.push(inst(Op::Lda, Operand::Direct(p.clone())));
                    for _ in 0..count {
                        candidate.push(inst(first, Operand::Acc));
                        candidate.push(inst(second, Operand::Direct(q.clone())));
                    }
                    candidate.push(inst(Op::Sta, Operand::Direct(p.clone())));
                    candidate.extend_from_slice(&lines[end..stop]);
                    // a return reads every register and cell: the region's end
                    // is not an exit that frees anything
                    let every = cells_named(region);
                    let close = |mut lines: Vec<Line>| {
                        if returns {
                            return asm.candidate_from(lines);
                        }
                        if !ends_in_inst {
                            lines.push(inst(Op::Bne, Operand::Direct("__region_exit".to_owned())));
                        }
                        let mut closed = asm.candidate_from(lines);
                        closed.no_a_result = false;
                        closed.no_x_result = false;
                        closed.no_y_result = false;
                        closed.result_cells = every.clone();
                        closed
                    };
                    SHIFT_CANDIDATE_LINES.with(|work| work.set(work.get() + candidate.len()));
                    let mut candidate = close(candidate);
                    crate::peephole::drop_reloads(&mut candidate);
                    forward_cell_copy(&mut candidate);
                    crate::peephole::drop_reloads(&mut candidate);
                    drop_dead_stores(&mut candidate);
                    crate::peephole::drop_dead_writes(&mut candidate);
                    store_then_index_load(&mut candidate);
                    if !returns && !ends_in_inst {
                        candidate.lines.pop();
                    }
                    if let (Some(before), Some(after)) = (bytes_of(region), bytes_of(&candidate.lines)) {
                        if after < before {
                            asm.lines.splice(start..stop, candidate.lines);
                            at = start;
                            continue;
                        }
                    }
                }
                at = end;
            }
            None => at += 1,
        }
    }
}

/// Every cell and symbol the instructions name.
fn cells_named(lines: &[Line]) -> std::collections::BTreeSet<String> {
    lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(Inst { operand, .. }) => operand.name().map(str::to_owned),
            _ => None,
        })
        .collect()
}

/// The counted shift loop at `at`: the shift of the first cell, the rotate of
/// the second, the two cells and the count.
fn shift_loop(
    lines: &[Line],
    at: usize,
    references: &std::collections::HashMap<String, usize>,
) -> Option<(Op, Op, String, String, usize)> {
    let inst = |i: usize| match lines.get(at + i) {
        Some(Line::Inst(inst)) => Some(inst),
        _ => None,
    };
    let (load, step_a, step_b, count, branch) = (inst(0)?, inst(2)?, inst(3)?, inst(4)?, inst(5)?);
    let Some(Line::Label(label)) = lines.get(at + 1) else { return None };
    let (Operand::Imm(n), Operand::Direct(p), Operand::Direct(q)) = (&load.operand, &step_a.operand, &step_b.operand)
    else {
        return None;
    };
    let index_ok = matches!((load.op, count.op), (Op::Ldx, Op::Dex) | (Op::Ldy, Op::Dey));
    let pair = matches!(
        (step_a.op, step_b.op),
        (Op::Asl, Op::Rol) | (Op::Lsr, Op::Ror)
    );
    let n: usize = n.parse().ok()?;
    (index_ok
        && pair
        && p != q
        && (2..=8).contains(&n)
        && branch.op == Op::Bne
        && branch.target() == Some(label.as_str())
        && references.get(label.as_str()) == Some(&1))
    .then(|| (step_a.op, step_b.op, p.clone(), q.clone(), n))
}

/// The instructions that read a memory operand and write no memory.
fn reads_only(op: Op) -> bool {
    matches!(op, Op::Lda | Op::Ldx | Op::Ldy | Op::Cpx | Op::Cpy | Op::Bit) || alu(op)
}

pub fn forward_cell_copy(asm: &mut Asm) {
    let mut at = 0;
    while at + 1 < asm.lines.len() {
        if forward_once(asm, at) {
            continue;
        }
        at += 1;
    }
}

/// `lda s; sta t` at `at`: the reads of `t` that follow in straight-line code
/// read `s` instead, and the store goes when that leaves `t` dead.
fn forward_once(
    asm: &mut Asm,
    at: usize,
) -> bool {
    let (Line::Inst(load), Line::Inst(store)) = (&asm.lines[at], &asm.lines[at + 1]) else { return false };
    let (Operand::Direct(s), Operand::Direct(t)) = (&load.operand, &store.operand) else { return false };
    if load.op != Op::Lda || store.op != Op::Sta || s == t || cell_number(s).is_none() || cell_number(t).is_none() {
        return false;
    }
    let (s, t) = (s.clone(), t.clone());
    // The reads are rewritten where they stand and put back if the copy stays
    // (cloning the function per `lda; sta` pair was quadratic: 15x the cost of
    // the whole compile of a nest of fills).
    let mut rewritten: Vec<(usize, Operand)> = Vec::new();
    for (index, line) in asm.lines.iter_mut().enumerate().skip(at + 2) {
        let Line::Inst(inst) = line else { break };
        if !straight(inst.op) {
            break;
        }
        let pointer_pair = |c: &String, cell: &String| {
            cell_number(c).zip(cell_number(cell)).is_some_and(|(m, n)| m == n || m + 1 == n)
        };
        match &inst.operand {
            // an index reaches the cells above its base; a pointer reads two
            Operand::Indexed(c, _) if cell_number(c).is_some() => break,
            Operand::IndirectY(c) | Operand::IndirectX(c) | Operand::Indirect(c)
                if pointer_pair(c, &t) || pointer_pair(c, &s) =>
            {
                break;
            }
            Operand::Direct(c) if *c == t => {
                if !reads_only(inst.op) {
                    break;
                }
                rewritten.push((index, std::mem::replace(&mut inst.operand, Operand::Direct(s.clone()))));
            }
            Operand::Direct(c) if *c == s && !reads_only(inst.op) => break,
            _ => {}
        }
    }
    if rewritten.is_empty() || !cell_dead_from(&asm.lines, at + 2, &t, &asm.result_cells, &mut BTreeSet::new()) {
        for (index, operand) in rewritten {
            if let Line::Inst(inst) = &mut asm.lines[index] {
                inst.operand = operand;
            }
        }
        return false;
    }
    asm.lines.remove(at + 1);
    true
}

/// Which bit a shift of a byte leaves in the carry: bit 7 of the old byte for
/// `asl`/`rol`, bit 0 for `lsr`/`ror`.
fn bit_out(op: Op) -> Option<u8> {
    match op {
        Op::Asl | Op::Rol => Some(7),
        Op::Lsr | Op::Ror => Some(0),
        _ => None,
    }
}

/// `lda m; asl a` (or `lsr a`) whose `a` the next line replaces takes a bit of
/// `m` into the carry. When the carry already holds that bit, from a shift in
/// place of a cell `c` that held a copy of `m` (`lda m; sta c; ...; rol c`),
/// the pair goes: the carry is unchanged and the next load sets the zero and
/// sign flags the shift set.
pub fn carry_holds_the_bit(asm: &mut Asm) {
    let mut at = 1;
    while at + 2 < asm.lines.len() {
        if drop_second_extraction(asm, at) {
            continue;
        }
        at += 1;
    }
}

fn drop_second_extraction(
    asm: &mut Asm,
    at: usize,
) -> bool {
    let (Line::Inst(load), Line::Inst(shift), Line::Inst(next)) =
        (&asm.lines[at], &asm.lines[at + 1], &asm.lines[at + 2])
    else {
        return false;
    };
    let Operand::Direct(m) = &load.operand else { return false };
    let wanted = match (shift.op, &shift.operand) {
        (Op::Asl, Operand::Acc) => 7,
        (Op::Lsr, Operand::Acc) => 0,
        _ => return false,
    };
    if load.op != Op::Lda || cell_number(m).is_none() || next.op != Op::Lda {
        return false;
    }
    let writes =
        |inst: &Inst, cell: &String| matches!(&inst.operand, Operand::Direct(c) if c == cell) && !reads_only(inst.op);
    // the last line before that sets the carry must shift a cell `c` in place
    let mut from = at;
    let shifted = loop {
        if from == 0 {
            return false;
        }
        from -= 1;
        let Line::Inst(inst) = &asm.lines[from] else { return false };
        if !straight(inst.op) {
            return false;
        }
        if let (Some(bit), Operand::Direct(c)) = (bit_out(inst.op), &inst.operand) {
            if bit == wanted && c != m && cell_number(c).is_some() {
                break c.clone();
            }
            return false;
        }
        if sets_carry(inst.op) || writes(inst, m) || matches!(inst.operand, Operand::Indexed(..)) {
            return false;
        }
    };
    // `c` was stored from `m` and neither changed since, but by that shift
    let mut back = from;
    loop {
        if back == 0 {
            return false;
        }
        back -= 1;
        let Line::Inst(inst) = &asm.lines[back] else { return false };
        if !straight(inst.op) || matches!(inst.operand, Operand::Indexed(..)) {
            return false;
        }
        if inst.op == Op::Sta && matches!(&inst.operand, Operand::Direct(c) if *c == shifted) {
            let Some(Line::Inst(source)) = back.checked_sub(1).map(|prior| &asm.lines[prior]) else { return false };
            if source.op == Op::Lda && source.operand == Operand::Direct(m.clone()) {
                break;
            }
            return false;
        }
        if writes(inst, &shifted) || writes(inst, m) {
            return false;
        }
    }
    asm.lines.drain(at..at + 2);
    true
}

/// The instructions that change the carry.
fn sets_carry(op: Op) -> bool {
    use Op::*;
    matches!(
        op,
        Adc | Asl | Clc | Cmp | Cpx | Cpy | Lsr | Rol | Ror | Sbc | Sec
    )
}

/// `lda s; sta t` where `s` is dead after the copy and `t` is used only in the
/// straight-line code that follows: that code uses `s` instead and the store
/// goes. A shift or step of the copy then happens in the cell it came from.
///
/// The pin's analogue is the register coalescer joining a copy whose source
/// dies (`RegisterCoalescer.cpp`, `joinCopy`).
pub fn coalesce_copy(asm: &mut Asm) {
    let mut at = 0;
    while at + 1 < asm.lines.len() {
        if coalesce_once(asm, at) {
            continue;
        }
        at += 1;
    }
}

fn coalesce_once(
    asm: &mut Asm,
    at: usize,
) -> bool {
    let (Line::Inst(load), Line::Inst(store)) = (&asm.lines[at], &asm.lines[at + 1]) else { return false };
    let (Operand::Direct(s), Operand::Direct(t)) = (&load.operand, &store.operand) else { return false };
    if load.op != Op::Lda || store.op != Op::Sta || s == t {
        return false;
    }
    let (Some(sn), Some(tn)) = (cell_number(s), cell_number(t)) else { return false };
    if !caller_saved(sn) || !caller_saved(tn) {
        return false;
    }
    let (s, t) = (s.clone(), t.clone());
    // The reads of `t` to turn into reads of `s`; nothing changes until the
    // copy is known to go.
    let mut renames = Vec::new();
    let mut end = at + 2;
    while let Some(Line::Inst(inst)) = asm.lines.get(end) {
        if !straight(inst.op) {
            break;
        }
        let pair = |c: &String, n: u8| cell_number(c).is_some_and(|m| m == n || m + 1 == n);
        match &inst.operand {
            Operand::Indexed(c, _) if cell_number(c).is_some() => return false,
            Operand::IndirectY(c) | Operand::IndirectX(c) | Operand::Indirect(c) if pair(c, sn) || pair(c, tn) => {
                return false;
            }
            Operand::Direct(c) if *c == s => return false,
            Operand::Direct(c) if *c == t => renames.push(end),
            _ => {}
        }
        end += 1;
    }
    let dead = |cell: &str| cell_dead_from(&asm.lines, end, cell, &asm.result_cells, &mut BTreeSet::new());
    if renames.is_empty() || !dead(&s) || !dead(&t) {
        return false;
    }
    for index in renames {
        if let Line::Inst(inst) = &mut asm.lines[index] {
            inst.operand = Operand::Direct(s.clone());
        }
    }
    asm.lines.remove(at + 1);
    true
}
