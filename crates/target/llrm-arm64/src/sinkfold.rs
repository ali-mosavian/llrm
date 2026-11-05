//! Moves a cheap computation to the copy into a fixed register, or into the
//! address of a load or store, that is its only reader (LLVM's
//! `PerformSinkAndFold`, which AArch64 turns on).
//!
//! # How it works
//! An instruction that makes one value from at most two registers and a
//! constant is followed through the copies of its result. When every end of
//! the chain is either a copy into a fixed register (the instruction is as
//! cheap as a move: it is made again there) or the address of a load or
//! store the instruction folds into (`canFoldIntoAddrMode`), the instruction
//! and the copies go, and each end gets its own. A block that already holds as
//! many registers as there are refuses the extra operands.
//!
//! ```text
//!  pre:   add w9, w1, #1                 pre:   ...
//!         mov w10, w9                    loop:  add w1, w1', #1   // made at the call
//!  loop:  mov w1, w10 ; bl f      ->            bl f
//! ```
//! The folds: `add`/`sub` of an immediate into an offset, a register add
//! (shifted, extended) into a register offset, `sxtw`/`mov w` into the index's
//! extension.
//!
//! # Example
//! `place(a, n, m)` in the queens benchmark makes `n + 1` once for the search
//! below it, then reads it in the loop as the argument of a call. The add is
//! made at the call and its register is free across the loop: two callee-saved
//! saves go.
//!
//! # Runs
//! In [`crate::prepass`], after the sink, on the function the selector made.
//!
//! # See also
//! LLVM's `MachineSinking::PerformSinkAndFold`
//! ([MachineSink.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineSink.cpp#L407)),
//! `AArch64InstrInfo::canFoldIntoAddrMode`.
//!
//! DEVIATE SF1: the pin runs this on SSA before phi elimination; here a value
//! qualifies when it and its operands are each defined once (a phi copy
//! defines a register several times). Exit: 11-12, with the core machine passes
//! on arm64.
//! DEVIATE SF2: the register pressure check counts live virtual registers
//! against the allocatable ones, not the pin's per-set limits. Exit: 11-12.

use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

use crate::mc::inst::*;
use crate::mc::visit::{Reg, RegRef, Role};
use crate::mfunc::{MFunc, Op};
use crate::prepass::{cfg, definitions, live_in, peak_pressure};

type At = (usize, usize);

/// One end of a chain, and what takes the place of the op there.
struct Site {
    at: At,
    replacement: Op,
}

pub fn run(function: &mut MFunc) {
    for _ in 0..4 {
        if !round(function) {
            return;
        }
    }
}

/// The registers of `op` that make it a value: its virtual uses, the zero
/// register left out.
fn operands(op: &Op) -> Option<Vec<Reg>> {
    let mut found = Vec::new();
    for register in op.defs_and_uses().1 {
        match register {
            Reg::Gpr(Gpr(31)) => {}
            one if one.is_virtual() => found.push(one),
            _ => return None,
        }
    }
    Some(found)
}

/// An instruction `PerformSinkAndFold` takes: one definition, no effect, at
/// most two register operands, and whether it is as cheap as a move.
fn candidate(op: &Op) -> Option<(Reg, bool)> {
    let cheap = match op {
        Op::Imm { .. } => true,
        Op::I(Inst::AddSubImm { op: AddSub { set_flags: false, .. }, rd, rn, .. })
            if rd.is_virtual() && rn.is_virtual() =>
        {
            true
        }
        Op::I(
            Inst::MoveWide { op: Wide::Not | Wide::Zero, .. }
            | Inst::LogicImm { op: Logic::And | Logic::Orr | Logic::Eor, .. },
        ) => true,
        Op::I(
            Inst::AddSubShifted { op: AddSub { set_flags: false, .. }, .. }
            | Inst::AddSubExtended { op: AddSub { set_flags: false, .. }, .. }
            | Inst::BitfieldMove { op: Bitfield::Sbfm, .. }
            | Inst::LogicShifted { op: Logic::Orr, invert: false, .. },
        ) => false,
        _ => return None,
    };
    let (defs, uses) = op.defs_and_uses();
    let [def] = defs.as_slice() else { return None };
    (def.is_virtual() && !uses.contains(def)).then_some((*def, cheap))
}

fn round(function: &mut MFunc) -> bool {
    let defined = definitions(function);
    let mut readers: HashMap<Reg, Vec<At>> = HashMap::default();
    for (b, block) in function.blocks.iter().enumerate() {
        for (i, op) in block.ops.iter().enumerate() {
            for register in op.defs_and_uses().1 {
                readers.entry(register).or_default().push((b, i));
            }
        }
    }
    let graph = cfg(function);
    let live = live_in(function, &graph);
    let mut peak: HashMap<usize, usize> = HashMap::default();
    let limit = crate::regalloc::pool::GPR_CALLEE_SAVED.len() + crate::lir_view::gpr_caller_saved_count();

    let mut touched: HashSet<At> = HashSet::default();
    let mut removed: HashSet<At> = HashSet::default();
    let mut replaced: Vec<Site> = Vec::new();
    for b in 0..function.blocks.len() {
        for i in (0..function.blocks[b].ops.len()).rev() {
            let op = &function.blocks[b].ops[i];
            let Some((def, cheap)) = candidate(op) else { continue };
            let Some(from) = operands(op) else { continue };
            if from.len() > 2 || defined.get(&def) != Some(&1) || from.iter().any(|one| defined.get(one) != Some(&1)) {
                continue;
            }
            if touched.contains(&(b, i)) {
                continue;
            }
            // Follow the copies of the value to every end.
            let (mut work, mut chain, mut ends) = (vec![def], Vec::<At>::new(), Vec::<Site>::new());
            let mut ok = true;
            'walk: while let Some(register) = work.pop() {
                for &at in readers.get(&register).map(Vec::as_slice).unwrap_or(&[]) {
                    let use_op = &function.blocks[at.0].ops[at.1];
                    match use_op {
                        Op::Copy { dst, src } if *src == register => {
                            if dst.is_virtual() {
                                if defined.get(dst) != Some(&1) {
                                    ok = false;
                                    break 'walk;
                                }
                                chain.push(at);
                                work.push(*dst);
                            } else {
                                let fixed = match dst {
                                    Reg::Gpr(Gpr(number)) if *number < 31 => Some(Gpr(*number)),
                                    _ => None,
                                };
                                let (Some(fixed), true) = (fixed, cheap) else {
                                    ok = false;
                                    break 'walk;
                                };
                                ends.push(Site { at, replacement: made_in(op, fixed) });
                            }
                        }
                        Op::I(inst @ (Inst::Load { .. } | Inst::Store { .. })) => {
                            let Reg::Gpr(used) = register else {
                                ok = false;
                                break 'walk;
                            };
                            let times = use_op.defs_and_uses().1.iter().filter(|one| **one == register).count();
                            match (times == 1).then(|| folded(inst, used, op)).flatten() {
                                Some(new) => ends.push(Site { at, replacement: Op::I(new) }),
                                None => {
                                    ok = false;
                                    break 'walk;
                                }
                            }
                        }
                        _ => {
                            ok = false;
                            break 'walk;
                        }
                    }
                }
            }
            if !ok || ends.is_empty() {
                continue;
            }
            // Operands held longer where the ends are in another block.
            let far: HashSet<usize> = ends.iter().map(|site| site.at.0).filter(|&block| block != b).collect();
            if far.iter().any(|&block| {
                let held = *peak
                    .entry(block)
                    .or_insert_with(|| peak_pressure(function, &[block], &graph, &live, &HashMap::default()).0);
                held + from.len() > limit
            }) {
                continue;
            }
            let all: Vec<At> =
                std::iter::once((b, i)).chain(chain.iter().copied()).chain(ends.iter().map(|s| s.at)).collect();
            if all.iter().any(|at| touched.contains(at)) {
                continue;
            }
            touched.extend(all);
            removed.insert((b, i));
            removed.extend(chain);
            replaced.extend(ends);
        }
    }
    if touched.is_empty() {
        return false;
    }
    for site in replaced {
        *function.blocks[site.at.0].ops.op_mut(site.at.1) = site.replacement;
    }
    for (b, block) in function.blocks.iter_mut().enumerate() {
        let mut index = 0;
        block
            .ops
            .retain(
                |_| {
                    let keep = !removed.contains(&(b, index));
                    index += 1;
                    keep
                },
            );
    }
    true
}

/// `op` with the register it defines made the fixed `into` (`reMaterialize`).
fn made_in(
    op: &Op,
    into: Gpr,
) -> Op {
    let mut copy = op.clone();
    match &mut copy {
        Op::Imm { dst, .. } => *dst = into,
        _ => copy.visit(&mut |register, role| {
            if role == Role::Def
                && let RegRef::Gpr(slot) = register
            {
                *slot = into;
            }
        }),
    }
    copy
}

/// `AArch64InstrInfo::canFoldIntoAddrMode`: the access `inst`, which reads
/// `used` in its address, with the computation `def` of `used` folded in.
fn folded(
    inst: &Inst,
    used: Gpr,
    def: &Op,
) -> Option<Inst> {
    let (mem, addr, mmo, store) = match inst {
        Inst::Load { mem, addr, mmo } => (mem, addr, mmo, false),
        Inst::Store { mem, addr, mmo } => (mem, addr, mmo, true),
        _ => return None,
    };
    let bytes = match mem {
        Memory::Int { bytes, reg, .. } => {
            if *reg == used {
                return None;
            }
            i64::from(*bytes)
        }
        Memory::Fp { bytes, .. } => i64::from(*bytes),
    };
    let Op::I(def) = def else { return None };
    let new = match *addr {
        Addr::Register { base, index, extend: Extend::Uxtx, shifted } => {
            let scale = if shifted { bytes } else { 1 };
            let other = if index == used {
                base
            } else if base == used && scale == 1 {
                index
            } else {
                return None;
            };
            match *def {
                Inst::BitfieldMove { op: Bitfield::Sbfm, width: Width::X, rn, immr: 0, imms: 31, .. } => {
                    Addr::Register { base: other, index: rn, extend: Extend::Sxtw, shifted }
                }
                Inst::LogicShifted {
                    op: Logic::Orr,
                    invert: false,
                    width: Width::W,
                    rn: Gpr(31),
                    rm,
                    shift: Shift::Lsl,
                    amount: 0,
                    ..
                } => Addr::Register { base: other, index: rm, extend: Extend::Uxtw, shifted },
                _ => return None,
            }
        }
        Addr::Offset { base, imm } if base == used => match *def {
            Inst::AddSubImm {
                op: AddSub { subtract, set_flags: false }, width: Width::X, rn, imm12, shift12, ..
            } if rn.is_virtual() => {
                let step = i64::from(imm12) << if shift12 { 12 } else { 0 };
                let new = imm + if subtract { -step } else { step };
                if !legal_offset(new, bytes) || !pair_range_kept(bytes, imm, new) {
                    return None;
                }
                Addr::Offset { base: rn, imm: new }
            }
            Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::X,
                rn,
                rm,
                shift: Shift::Lsl,
                amount,
                ..
            } if imm == 0 && rn.is_virtual() && rm.is_virtual() => {
                let scaled = scale_fits(amount, bytes)?;
                Addr::Register { base: rn, index: rm, extend: Extend::Uxtx, shifted: scaled }
            }
            Inst::AddSubExtended {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::X,
                rn,
                rm,
                extend: extend @ (Extend::Uxtw | Extend::Sxtw),
                amount,
                ..
            } if imm == 0 && rn.is_virtual() && rm.is_virtual() => {
                let scaled = scale_fits(amount, bytes)?;
                Addr::Register { base: rn, index: rm, extend, shifted: scaled }
            }
            _ => return None,
        },
        _ => return None,
    };
    Some(if store {
        Inst::Store { mem: *mem, addr: new, mmo: mmo.clone() }
    } else {
        Inst::Load { mem: *mem, addr: new, mmo: mmo.clone() }
    })
}

/// `isLegalAddressingMode(bytes, 0, 1 << amount)`: the index is unscaled or
/// scaled by the access size. Whether it is scaled.
fn scale_fits(
    amount: u8,
    bytes: i64,
) -> Option<bool> {
    if amount == 0 { Some(false) } else { (1i64.checked_shl(u32::from(amount))? == bytes).then_some(true) }
}

/// An offset a load or store encodes: scaled and unsigned, or unscaled and
/// signed 9 bits.
fn legal_offset(
    offset: i64,
    bytes: i64,
) -> bool {
    (offset >= 0 && offset % bytes == 0 && offset / bytes <= 4095) || (-256..=255).contains(&offset)
}

/// `validateOffsetForLDP`: an offset that could pair does not stop doing so.
fn pair_range_kept(
    bytes: i64,
    old: i64,
    new: i64,
) -> bool {
    let (low, high) = match bytes {
        4 => (-256, 252),
        8 => (-512, 504),
        16 => (-1024, 1008),
        _ => return true,
    };
    old < low || old > high || (low..=high).contains(&new)
}
