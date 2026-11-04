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
    /// Clocks the `and` saves over the `movzx`.
    pub clocks_saved: i64,
    /// Thousandths of a clock a byte of code is worth
    /// (`speed_milliclocks_per_byte`).
    pub milliclocks_per_byte: i64,
}

/// Bytes `and r32,imm32` takes over `movzx r32,r8|r16` (`eax` takes one less).
const EXTRA_BYTES: i64 = 3;

impl MaskByAnd {
    /// Whether a site executed `frequency` times per entry pays for the bytes.
    fn pays(
        &self,
        frequency: f64,
    ) -> bool {
        self.clocks_saved > 0
            && frequency * self.clocks_saved as f64 * 1000.0 >= (self.milliclocks_per_byte * EXTRA_BYTES) as f64
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
        if self.milliclocks_per_byte <= 0 || self.clocks_saved <= 0 || body.bits != 32 {
            return Ok(body);
        }
        let frequencies = MachineBlockFrequencyInfo::of(&body);
        let regs = body.regs();
        let reads = read_counts(&body);
        let mut out = body.clone();
        for (index, block) in out.blocks.iter_mut().enumerate() {
            if !self.pays(frequencies.get_block_freq_relative_to_entry_block(index)) {
                continue;
            }
            let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
            let mut changed = false;
            for at in 0..insns.len() {
                if let Some(new) =
                    masked(&insns[at], &reads).filter(|_| flags_dead_after(regs, body.bits, &insns[at + 1..]))
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
) -> Option<Insn> {
    let what = one.what.as_ref()?;
    if what.name.as_deref() != Some("movzx") || what.op != Operation::Extend {
        return None;
    }
    let ([Loc::Held(dest)], [Loc::Held(source)]) = (what.dests.as_slice(), what.sources.as_slice()) else {
        return None;
    };
    if dest.width != 4 || source.width != 1 || dest.value == source.value {
        return None;
    }
    // The `and` overwrites its source's register: a source read elsewhere
    // costs a `mov` and loses the clocks it was to save.
    if reads.of(source.value) != 1 {
        return None;
    }
    let mask = 0xff;
    let mut new = (**one).clone();
    let made = new.what.as_mut()?;
    made.name = Some("and".into());
    made.op = Operation::Binary;
    made.sources =
        vec![Loc::Held(Held { value: source.value, width: 4 }), Loc::Imm(Imm { value: mask, width: 4, address: None })];
    new.widths.retain(|(value, _)| *value != source.value);
    Some(new)
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
