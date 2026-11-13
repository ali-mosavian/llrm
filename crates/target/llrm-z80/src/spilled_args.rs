//! A stack argument the allocator spilled is stored straight into the call's
//! area, where the push form would reload it to push it.
//!
//! # How it works
//! Call operands are computed before the call sequence opens, and the last
//! argument is pushed first, so arguments that arrive in source order are
//! live in reverse and, with `hl`, `de` and `bc` the only pairs, spill: load,
//! spill, reload, push is 17 B a word. The store form opens the area where the
//! first such value is spilled and turns each `SPILL16` of a stack word into
//! the store to its place; the reload and the store after it are gone. Words
//! that were never spilled cost 6 B stored where a push is 1 B, and the
//! reserve is paid once, so the form is chosen per call frame by price on the
//! allocation already made: no second allocation.
//!
//! DEVIATE Z80-ARGPUSH-SPILL (owner z80-argpush, exit 2026-11-16): the pin
//! folds the reload into `push m`
//! (`X86CallFrameOptimization::canFoldIntoRegPush`); the Z80 has no push from
//! memory.
//!
//! Complexity: one backward scan per stored word, O(n) per frame.

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Assignment;

use crate::expand::{Line, lanes, price};
use crate::mfunc::{Inst, Kind, Operand, Reg};
use crate::reg;

/// A stack word whose value went through a spill slot.
struct Spilled {
    /// The `SPILL16` of the value, and the `RELOAD16` and `STSP16` after it.
    spill: usize,
    reload: usize,
    store: usize,
}

/// Bytes and T-states.
type Cost = (i64, i64);

fn cost(lines: &[&str]) -> Cost {
    let lines: Vec<Line> = lines.iter().map(|text| Line::Inst((*text).to_owned())).collect();
    let (bytes, clocks) = price(&lines);
    (i64::from(bytes), i64::from(clocks))
}

fn add(
    one: Cost,
    other: Cost,
    times: i64,
) -> Cost {
    (one.0 + times * other.0, one.1 + times * other.1)
}

/// The frame at `open` with the spilled words stored in place, when that
/// costs less than pushing them. `stores` are the frame's `STSP16`, which
/// `crate::expand::push_places` found pushable. `bytes_first`: bytes decide,
/// clocks may not grow (-O1, -Os, -Oz); otherwise 1 B = 3 T. `below`: bytes
/// between IX and SP before the area opens (the frame and the spills pushed
/// by the blocks before this one); the stores are `(ix)` stores only while the
/// whole area lies within -128 of IX, else each is `ld iy, #k; add iy, sp`
/// and two stores (fcmp_all -Oz grew 288 B), so a deeper frame keeps pushes.
pub(crate) fn store_spilled(
    insts: &[Inst],
    open: usize,
    stores: &[usize],
    assignment: &Assignment,
    bytes_first: bool,
    below: i64,
) -> Option<Vec<Inst>> {
    let physical = |register: Reg| match register {
        Reg::Phys(id) => id,
        Reg::Virt(v) => assignment.0[v as usize],
    };
    let touches = |inst: &Inst, pair: RegId| {
        let (lo, hi) = lanes(pair).expect("a pair");
        inst.operands
            .iter()
            .any(
                |operand| match operand {
                    Operand::Def(r) | Operand::Use(r) => [pair, lo, hi].contains(&physical(*r)),
                    _ => false,
                },
            )
    };
    let frame_slot = |inst: &Inst| {
        inst.operands
            .iter()
            .find_map(
                |operand| match operand {
                    Operand::Frame(slot) => Some(*slot),
                    _ => None,
                },
            )
    };
    let mut spilled = Vec::new();
    for &store in stores {
        let Some(Operand::Use(value)) = insts[store].operands.first() else { continue };
        let pair = physical(*value);
        // the nearest earlier mention of the pair is the reload that made it
        let Some(reload) = (0..store).rev().find(|&at| touches(&insts[at], pair)) else { continue };
        let Kind::Op("RELOAD16") = insts[reload].kind else { continue };
        let Some(slot) = frame_slot(&insts[reload]) else { continue };
        // and the pair is dead after the store: the next mention defines it
        // whole
        let dead = insts[store + 1..]
            .iter()
            .find(|inst| touches(inst, pair))
            .is_some_and(
                |inst| inst.operands.iter().all(|operand| !matches!(operand, Operand::Use(r) if physical(*r) == pair))
                    && inst.operands.iter().any(|operand| matches!(operand, Operand::Def(r) if physical(*r) == pair)),
            );
        // the slot is written once and read once in the whole function
        let mentions: Vec<usize> = (0..insts.len()).filter(|&at| frame_slot(&insts[at]) == Some(slot)).collect();
        let spill = mentions.iter().copied().find(|&at| matches!(insts[at].kind, Kind::Op("SPILL16")));
        let (Some(spill), true, 2) = (spill, dead, mentions.len()) else { continue };
        let Some(Operand::Use(source)) = insts[spill].operands.get(1) else { continue };
        if ![reg::BC, reg::DE, reg::HL].contains(&physical(*source)) || spill > reload {
            continue;
        }
        spilled.push(Spilled { spill, reload, store });
    }
    if spilled.is_empty() {
        return None;
    }
    let first = spilled.iter().map(|word| word.spill).min()?.min(open);
    // the area opens earlier: nothing in between may depend on the stack
    // pointer
    let in_the_way = insts[first..open]
        .iter()
        .any(
            |inst| inst.call
                || matches!(
                    inst.kind,
                    Kind::Op("ADJCALLSTACKDOWN" | "ADJCALLSTACKUP" | "PUSHP" | "POPP" | "PEEKP" | "STSP8")
                ),
        );
    if in_the_way {
        return None;
    }
    let Some(Operand::Imm(bytes)) = insts[open].operands.first() else { return None };
    let pushed: i64 = insts[..first]
        .iter()
        .map(|inst| match inst.kind {
            Kind::Op("PUSHP") => 2,
            Kind::Op("POPP") => -2,
            _ => 0,
        })
        .sum();
    if below + pushed + 2 * ((*bytes + 1) / 2) > 128 {
        return None;
    }
    let words = stores.len() as i64;
    let (push, reload, store) =
        (cost(&["push bc"]), cost(&["ld c, -2(ix)", "ld b, -1(ix)"]), cost(&["ld -2(ix), c", "ld -1(ix), b"]));
    let reserve =
        if *bytes / 2 < 8 { add((0, 0), push, *bytes / 2) } else { cost(&["ld iy, #-18", "add iy, sp", "ld sp, iy"]) };
    let held = words - spilled.len() as i64;
    // store form less push form
    let delta = add(add(reserve, add(store, push, -1), held), add(reload, push, 1), -(spilled.len() as i64));
    let wins = if bytes_first { delta.0 <= 0 && delta.1 <= 0 && delta != (0, 0) } else { 3 * delta.0 + delta.1 < 0 };
    if !wins {
        return None;
    }
    let mut out = Vec::with_capacity(insts.len());
    for (at, inst) in insts.iter().enumerate() {
        if at == first {
            out.push(insts[open].clone());
        }
        if at == open || spilled.iter().any(|word| word.reload == at || word.store == at) {
            continue;
        }
        if let Some(word) = spilled.iter().find(|word| word.spill == at) {
            let Some(Operand::Imm(offset)) = insts[word.store].operands.get(1) else { return None };
            let mut place = insts[word.store].clone();
            place.operands = vec![inst.operands[1].clone(), Operand::Imm(*offset)];
            out.push(place);
        } else {
            out.push(inst.clone());
        }
    }
    Some(out)
}
