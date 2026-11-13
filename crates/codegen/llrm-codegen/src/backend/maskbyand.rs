//! Byte `movzx d, s.low` as `d := and s, mask`, where the CPU prices the
//! extension above the `and` and the block runs often enough to pay the extra
//! bytes.
//!
//! # How it works
//! The pin's table selects `(and x, 0xff)` as `MOVZX32rr8` with no CPU gate
//! (X86InstrCompiler.td#L1773-L1777): its CPUs extend in one clock. The 486
//! and the Pentium take three clocks for it and one for `and r,imm`, which is
//! two to three bytes longer. Before allocation the block frequency is known
//! and the `and` is three-address, so TwoAddress and the coalescer place it
//! without a `mov` where the source dies. The rewrite is made where `frequency
//! x clocks saved` is at least `rate x bytes added` (the target's speed rate;
//! none at -O1 and for size).
//!
//! Only byte sources: a halfword `and` (imm32, 6 B) after a `div` moved the
//! allocation in nbody_fixed -O3 (+25024 clocks, a loop counter spilled).
//!
//! The source must have no other reader: the `and` overwrites its register,
//! and a live source costs a `mov` that eats the clocks saved.
//!
//! The `and` writes the flags: the rewrite needs a later instruction of the
//! block that writes them all before any reads one, or the block's end
//! (flags are not carried across blocks).
//!
//! DEVIATE MA1 (owner x86-m32; exit: the rule moves into the generated table
//! when it takes a block-frequency predicate; review 2026-12-10). The
//! instruction selector cannot see how often a block runs, so the choice
//! is made here, where cold sites keep the shorter `movzx`.
//!
//! # Example
//! `movzx v1, v0.b` in a loop body of frequency 8: `v1 := and v0, 255`.
//!
//! # Runs
//! After `IvZero`, at -O2 and -O3.

use std::sync::Arc;

use crate::backend::lanes::Lanes;
use crate::backend::liveness;
use crate::liveness::blockfrequency::MachineBlockFrequencyInfo;
use crate::model::ir::{Held, Imm, Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::HashMap;

/// The prices of the rewrite, from the CPU's table.
pub struct MaskByAnd {
    /// Clocks the `and` saves over the `movzx`; none where the CPU states no
    /// clock for either (a target that prices through its cost model): the
    /// rewrite is then not priced, so it is not made.
    pub clocks_saved: Option<i64>,
    /// Thousandths of a clock a byte of code is worth
    /// (`speed_milliclocks_per_byte`).
    pub milliclocks_per_byte: i64,
    /// Clocks the `mov` of a source that stays live costs (gcc's
    /// `X86_TUNE_ZERO_EXTEND_WITH_AND` makes it `mov d,s; and d,255`); none
    /// where the CPU states no clock for it: that form is then not priced, so
    /// it is not made.
    pub copy_clocks: Option<i64>,
}

/// Bytes `and r32,imm32` takes over `movzx r32,r8|r16` (`eax` takes one less).
const EXTRA_BYTES: i64 = 3;

/// Bytes of the `mov r32,r32` a live source needs before the `and`.
const COPY_BYTES: i64 = 2;

impl MaskByAnd {
    /// Whether a site executed `frequency` times per entry pays for the bytes.
    fn pays(
        &self,
        frequency: f64,
        copied: bool,
    ) -> bool {
        let Some(saved) = self.clocks_saved else { return false };
        // A live source is copied first: the `mov` costs clocks and bytes.
        let (saved, bytes) = if copied {
            let Some(copy) = self.copy_clocks else { return false };
            (saved - copy, EXTRA_BYTES + COPY_BYTES)
        } else {
            (saved, EXTRA_BYTES)
        };
        saved > 0 && frequency * saved as f64 * 1000.0 >= (self.milliclocks_per_byte * bytes) as f64
    }
}

impl LIRTransform for MaskByAnd {
    fn class_name(&self) -> &'static str {
        "MaskByAnd"
    }

    fn name(&self) -> &str {
        "maskbyand"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if self.milliclocks_per_byte <= 0 || self.clocks_saved.is_none_or(|saved| saved <= 0) || body.bits != 32 {
            return Ok(body);
        }
        let frequencies = MachineBlockFrequencyInfo::of(&body);
        // A `movzx` of a `setcc` that `FixupSetCC` rewrites becomes a zero made
        // before the compare (`xor`, two bytes): cheaper than the `and` here in
        // both bytes and clocks, so it is left alone.
        let fixed = crate::backend::fixupsetcc::fixed(&body);
        let kept: llrm_mir::hash::HashSet<i64> =
            fixed.blocks.iter().flat_map(|b| b.insns.iter()).map(|one| one.at).collect();
        let regs = body.regs();
        let reads = read_counts(&body);
        let mut out = body.clone();
        for (index, block) in out.blocks.iter_mut().enumerate() {
            let frequency = frequencies.get_block_freq_relative_to_entry_block(index);
            if !self.pays(frequency, false) && !self.pays(frequency, true) {
                continue;
            }
            let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            let mut changed = false;
            for at in 0..insns.len() {
                if !kept.contains(&insns[at].at) {
                    continue;
                }
                if let Some(new) = masked(&insns[at], &reads, |copied, dest| {
                    self.pays(frequency, copied)
                        && !(copied
                            && crosses_call(
                                &insns,
                                at,
                                dest,
                                reads.of(dest) > insns.iter().filter(|one| reads_value(one, dest)).count(),
                            ))
                })
                .filter(|_| flags_dead_after(regs, body.bits, &insns[at + 1..]))
                {
                    insns[at] = Arc::new(new);
                    changed = true;
                }
            }
            if changed {
                *block = block.with_insns(insns);
            }
        }
        Ok(out)
    }
}

/// `d := movzx s.low` (a byte) as `d := and s, 255`.
fn masked(
    one: &Arc<Insn>,
    reads: &Reads,
    pays: impl Fn(bool, u32) -> bool,
) -> Option<Insn> {
    let what = one.what.as_ref()?;
    if what.name.as_deref() != Some("movzx") || what.op != Operation::Extend {
        return None;
    }
    let ([Loc::Held(dest)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
        return None;
    };
    if dest.width != 4 || source.width != 1 || source.offset != 0 || dest.value == source.value {
        return None;
    }
    // The `and` overwrites its source's register: a source read elsewhere is
    // copied first (TwoAddress), which the price counts.
    if !pays(reads.of(source.value) != 1, dest.value) {
        return None;
    }
    let mask = 0xff;
    let mut new = (**one).clone();
    let made = new.what.as_mut()?;
    made.name = Some("and".into());
    made.op = Operation::Binary;
    made.sources = vec![
        Loc::Held(Held { value: source.value, width: 4, offset: 0 }),
        Loc::Imm(Imm { value: mask, width: 4, address: None }),
    ];
    new.widths.retain(|(value, _)| *value != source.value);
    Some(new)
}

/// Whether `one` reads `value`.
fn reads_value(
    one: &Arc<Insn>,
    value: u32,
) -> bool {
    one.uses.contains(&value)
        || one.what.as_ref().is_some_and(|what| {
            what.sources.iter().any(|loc| crate::model::ir::values(loc).into_iter().any(|held| held.value == value))
        })
}

/// Whether `dest`, made at `insns[at]`, is wanted past a call of the block (a
/// read after the call here, or a read in another block): its register then
/// outlives the call, which costs a spill or a callee-saved register that the
/// clocks saved do not repay (x_int64_division -O2: one more spill across two
/// `$div64_u`, +700 clk). `elsewhere`: it is read outside this block.
fn crosses_call(
    insns: &[Arc<Insn>],
    at: usize,
    dest: u32,
    elsewhere: bool,
) -> bool {
    let reads = |one: &Arc<Insn>| reads_value(one, dest);
    let is_call = |one: &Arc<Insn>| one.what.as_ref().is_some_and(|what| what.op == Operation::Call);
    let Some(first) = (at + 1..insns.len()).find(|&index| is_call(&insns[index])) else { return false };
    elsewhere || (first + 1..insns.len()).any(|index| reads(&insns[index]))
}

/// The value a chain of plain copies starts from.
fn root(
    copies: &HashMap<u32, u32>,
    mut value: u32,
) -> u32 {
    while let Some(&from) = copies.get(&value) {
        value = from;
    }
    value
}

/// How many instructions and phis read each value, copies looked through:
/// a read of a copy is a read of the value it copies, and a copy itself is
/// none.
fn read_counts(body: &LirBody) -> Reads {
    let copies: HashMap<u32, u32> =
        body.blocks.iter().flat_map(|b| b.insns.iter()).filter_map(|one| one.copy_of()).collect();
    let mut counts: HashMap<u32, usize> = HashMap::default();
    for block in body.blocks.iter() {
        for phi in block.phis.iter() {
            for &(_, value) in &phi.incoming {
                *counts.entry(root(&copies, value)).or_default() += 1;
            }
        }
        for one in block.insns.iter().filter(|one| one.copy_of().is_none()) {
            let mut seen: Vec<u32> = one.uses.clone();
            if let Some(what) = one.what.as_ref() {
                for loc in what.sources.iter() {
                    seen.extend(crate::model::ir::values(loc).into_iter().map(|held| held.value));
                }
            }
            seen.sort_unstable();
            seen.dedup();
            for value in seen {
                *counts.entry(root(&copies, value)).or_default() += 1;
            }
        }
    }
    Reads { copies, counts }
}

struct Reads {
    copies: HashMap<u32, u32>,
    counts: HashMap<u32, usize>,
}

impl Reads {
    fn of(
        &self,
        value: u32,
    ) -> usize {
        self.counts.get(&root(&self.copies, value)).copied().unwrap_or(0)
    }
}

/// Whether the flags the `and` writes are dead after it: the first later
/// instruction to touch them writes them all, or none does.
fn flags_dead_after(
    regs: llrm_lir::registers::Regs,
    bits: u32,
    later: &[Arc<Insn>],
) -> bool {
    let all = Lanes::read_flags();
    for one in later {
        let Some(effect) = liveness::effect(regs, bits, one) else { return false };
        let reads = effect.all_reads();
        if !reads.and(&Lanes::read_flags()).is_empty() {
            return false;
        }
        if all.is_subset(&effect.writes) {
            return true;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A CPU that states neither clock (a target that prices through its cost
    /// model) stopped the whole pipeline at `cost("movzx")`. Not stated is not
    /// free: the rewrite is not priced, so it is never made.
    #[test]
    fn test_a_pass_with_no_stated_clocks_never_pays() {
        let rate = 1;
        let stated = MaskByAnd { clocks_saved: Some(1), milliclocks_per_byte: rate, copy_clocks: Some(0) };
        let unstated = MaskByAnd { clocks_saved: None, milliclocks_per_byte: rate, copy_clocks: Some(0) };
        assert!(stated.pays(1.0e9, false) && !unstated.pays(1.0e9, false));
    }
}
