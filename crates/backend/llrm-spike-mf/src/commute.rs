//! `TwoAddressInstructionPass`'s choice of which source of a commutable
//! instruction the result is tied to (`tryInstructionCommute`,
//! `isProfitableToCommute`), made before the tie copies are inserted.
//!
//! Tying the source that dies at the instruction, or the one the result is
//! copied back into, lets the allocator give the result that source's register
//! and drop the tie copy.
//!
//! DEVIATE: kills come from block liveness over virtual registers, not
//! `LiveIntervals`; the `SrcRegMap`/`DstRegMap` rule for copies to and from
//! physical registers is not translated (so a result returned in `d0` does not
//! steer the choice); `hasCommutePreference` is not asked (M68k has none).
//! Reason: the spike has no analysis manager. Exit: the neutral-LIR task.
//!
//! # See also
//! `llvm/lib/CodeGen/TwoAddressInstructionPass.cpp`, pin d1106deb71cc:
//! `isRevCopyChain` (L320), `noUseAfterLastDef` (L351), `isKilled` (L454),
//! `isProfitableToCommute` (L648), `tryInstructionCommute` (L1285).

use std::collections::HashMap;

use crate::mfunc::{Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::{Target, successors};

/// `MaxDataFlowEdge`.
const MAX_DATA_FLOW_EDGE: usize = 10;

struct Facts {
    /// Registers live out of each block.
    live_out: Vec<Vec<bool>>,
    /// Every definition of a register: `(block, instruction)`.
    defs: HashMap<u32, Vec<(usize, usize)>>,
}

fn virt(reg: Reg) -> Option<u32> {
    if let Reg::Virt(v) = reg { Some(v) } else { None }
}

fn uses(inst: &Inst) -> impl Iterator<Item = u32> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Use(r) = o { virt(*r) } else { None })
}

fn defines(inst: &Inst) -> impl Iterator<Item = u32> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Def(r) = o { virt(*r) } else { None })
}

impl Facts {
    fn of<C>(function: &MFunction<C>) -> Facts {
        let count = function.classes.len();
        let blocks = function.blocks.len();
        let succs = successors(function);
        let mut gen_ = vec![vec![false; count]; blocks];
        let mut kill = vec![vec![false; count]; blocks];
        let mut defs: HashMap<u32, Vec<(usize, usize)>> = HashMap::new();
        for (b, block) in function.blocks.iter().enumerate() {
            for (i, inst) in block.insts.iter().enumerate() {
                for u in uses(inst) {
                    if !kill[b][u as usize] {
                        gen_[b][u as usize] = true;
                    }
                }
                for d in defines(inst) {
                    kill[b][d as usize] = true;
                    defs.entry(d).or_default().push((b, i));
                }
            }
        }
        let mut live_in = vec![vec![false; count]; blocks];
        let mut live_out = vec![vec![false; count]; blocks];
        let mut changed = true;
        while changed {
            changed = false;
            for b in (0..blocks).rev() {
                for r in 0..count {
                    let out = succs[b].iter().any(|&s| live_in[s][r]);
                    let input = gen_[b][r] || (out && !kill[b][r]);
                    if out != live_out[b][r] || input != live_in[b][r] {
                        live_out[b][r] = out;
                        live_in[b][r] = input;
                        changed = true;
                    }
                }
            }
        }
        Facts { live_out, defs }
    }

    /// `isPlainlyKilled`: nothing reads `reg` after instruction `at` of block
    /// `b`, and it does not leave the block.
    fn plainly_killed<C>(
        &self,
        function: &MFunction<C>,
        b: usize,
        at: usize,
        reg: u32,
    ) -> bool {
        let rest = &function.blocks[b].insts[at + 1..];
        let read_later = rest.iter().any(|inst| uses(inst).any(|u| u == reg));
        let redefined = rest.iter().any(|inst| defines(inst).any(|d| d == reg));
        !read_later && (redefined || !self.live_out[b][reg as usize])
    }

    /// `isKilled`: looks through copies, since a copy of a register that lives
    /// on is not coalesced into a dying one.
    fn killed<C>(
        &self,
        function: &MFunction<C>,
        b: usize,
        at: usize,
        reg: u32,
    ) -> bool {
        let (mut b, mut at, mut reg) = (b, at, reg);
        loop {
            if !self.plainly_killed(function, b, at, reg) {
                return false;
            }
            let defs = self.defs.get(&reg).map_or(&[][..], Vec::as_slice);
            let [(def_block, def_at)] = defs else { return true };
            let inst = &function.blocks[*def_block].insts[*def_at];
            let (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(source))]) = (inst.kind, &inst.operands[..])
            else {
                return true;
            };
            (b, at, reg) = (*def_block, *def_at, *source);
        }
    }

    /// `getSingleDef`: the one definition of `reg` in block `b`.
    fn single_def<'a, C>(
        function: &'a MFunction<C>,
        b: usize,
        reg: u32,
    ) -> Option<&'a Inst> {
        let mut found = None;
        for inst in &function.blocks[b].insts {
            if defines(inst).any(|d| d == reg) {
                if found.is_some() {
                    return None;
                }
                found = Some(inst);
            }
        }
        found
    }

    /// `isRevCopyChain`: whether `to` is reached from `from` by following
    /// single definitions through copies and tied uses.
    fn rev_copy_chain<C>(
        function: &MFunction<C>,
        b: usize,
        from: u32,
        to: u32,
    ) -> bool {
        let mut reg = from;
        for _ in 0..MAX_DATA_FLOW_EDGE {
            let Some(def) = Facts::single_def(function, b, reg) else { return false };
            if let (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(source))]) = (def.kind, &def.operands[..]) {
                reg = *source;
            } else if let Some(tied) = def.ties.iter().find_map(|&(d, u)| match (&def.operands[d], &def.operands[u]) {
                (Operand::Def(Reg::Virt(dst)), Operand::Use(Reg::Virt(src))) if *dst == reg => Some(*src),
                _ => None,
            }) {
                if tied == reg {
                    return false;
                }
                reg = tied;
            } else {
                return false;
            }
            if reg == to {
                return true;
            }
        }
        false
    }

    /// `noUseAfterLastDef`: nothing reads `reg` between its last definition in
    /// the block (before `dist`) and `dist`. Also the position of that
    /// definition (1-based, 0 for none).
    fn no_use_after_last_def<C>(
        function: &MFunction<C>,
        b: usize,
        reg: u32,
        dist: usize,
    ) -> (bool, usize) {
        let (mut last_def, mut last_use) = (0, dist);
        for (i, inst) in function.blocks[b].insts[..dist - 1].iter().enumerate() {
            let at = i + 1;
            if uses(inst).any(|u| u == reg) && at < last_use {
                last_use = at;
            }
            if defines(inst).any(|d| d == reg) && at > last_def {
                last_def = at;
            }
        }
        (!(last_use > last_def && last_use < dist), last_def)
    }
}

/// Swaps the tied source of every commutable instruction with its other
/// register source where the pin's rules say the result is better tied to
/// that one.
pub fn commute_tied<T: Target>(function: &mut MFunction<T::Class>) {
    let facts = Facts::of(function);
    for b in 0..function.blocks.len() {
        for at in 0..function.blocks[b].insts.len() {
            let inst = &function.blocks[b].insts[at];
            if !inst.commutable || inst.ties.len() != 1 {
                continue;
            }
            let (def, used) = inst.ties[0];
            let (Operand::Def(Reg::Virt(a)), Operand::Use(Reg::Virt(base))) =
                (&inst.operands[def], &inst.operands[used])
            else {
                continue;
            };
            let (a, base) = (*a, *base);
            let Some(other) = inst
                .operands
                .iter()
                .enumerate()
                .position(|(n, o)| n != used && matches!(o, Operand::Use(Reg::Virt(r)) if *r != base))
            else {
                continue;
            };
            let Operand::Use(Reg::Virt(c)) = inst.operands[other] else { continue };
            // the result's class must hold the register that becomes the tied
            // one
            if T::meet(function.classes[c as usize], function.classes[a as usize]).is_none() {
                continue;
            }
            let dist = at + 1;
            let base_killed = facts.killed(function, b, at, base);
            let other_killed = facts.killed(function, b, at, c);
            let commute = (!base_killed && other_killed) || profitable(&facts, function, b, at, dist, a, base, c);
            if commute {
                function.blocks[b].insts[at].operands.swap(used, other);
                relax_demoted::<T>(function, &facts, b, at, base, other);
            }
        }
    }
}

/// `base` was the tied source, so its class was narrowed to the result's; it
/// is now the other source, which may take more. Where it is only a copy of
/// another register, read nowhere else, it takes that register's class met
/// with the operand's, so the allocator may put it where the tied one cannot
/// go (a temporary in an address register, not a data register).
///
/// DEVIATE from the pin, whose class stays narrowed (`constrainRegClass`
/// never widens). Reason: the swap is ours, and the narrowing it leaves
/// costs a data register on a rotation. Exit: the neutral-LIR task, where
/// the commute is the pin's own.
fn relax_demoted<T: Target>(
    function: &mut MFunction<T::Class>,
    facts: &Facts,
    b: usize,
    at: usize,
    base: u32,
    slot: usize,
) {
    let Kind::Op(name) = function.blocks[b].insts[at].kind else { return };
    let Some(wanted) = T::operand_class(name, slot) else { return };
    let Some([(def_block, def_at)]) = facts.defs.get(&base).map(Vec::as_slice) else { return };
    let (Kind::Copy, [Operand::Def(_), Operand::Use(Reg::Virt(source))]) =
        (function.blocks[*def_block].insts[*def_at].kind, &function.blocks[*def_block].insts[*def_at].operands[..])
    else {
        return;
    };
    let reads = function.blocks.iter().flat_map(|block| &block.insts).flat_map(uses).filter(|u| *u == base).count();
    if reads != 1 {
        return;
    }
    if let Some(class) = T::meet(function.classes[*source as usize], wanted) {
        function.classes[base as usize] = class;
    }
}

/// `isProfitableToCommute`.
#[allow(clippy::too_many_arguments)]
fn profitable<C>(
    facts: &Facts,
    function: &MFunction<C>,
    b: usize,
    at: usize,
    dist: usize,
    a: u32,
    base: u32,
    c: u32,
) -> bool {
    if !facts.plainly_killed(function, b, at, c) {
        return false;
    }
    let (clear_c, last_def_c) = Facts::no_use_after_last_def(function, b, c, dist);
    if !clear_c {
        return false;
    }
    let (clear_b, last_def_b) = Facts::no_use_after_last_def(function, b, base, dist);
    if !clear_b {
        return true;
    }
    if Facts::rev_copy_chain(function, b, c, a) {
        return true;
    }
    if Facts::rev_copy_chain(function, b, base, a) {
        return false;
    }
    last_def_b != 0 && last_def_c != 0 && last_def_c > last_def_b
}
