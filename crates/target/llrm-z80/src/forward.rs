//! A pair read again after the instruction that needed it in `hl`, `de` or
//! `bc` is read from that register.
//!
//! # How it works
//! `add a,(hl)` takes its pointer from `hl`, so the pointer is copied in
//! (`COPY hl, v`) and `v` goes on to be stepped (`inc`) after the load. `v`
//! overlaps the register range that copy starts, so it cannot be `hl`: the
//! loop kept it in `bc` and copied it to `hl` every trip, or spilled.
//! Where nothing writes the register before `v`'s last use in the block, the
//! later uses read a copy of the register (`COPY v', hl`) instead. `v` then
//! ends at the copy-in, `v'` begins at the copy-out, and both can be `hl`.
//!
//! # Example
//! `COPY hl, v; ADD8m a, hl; COPY w, v; INC16 w` becomes `COPY hl, v; ADD8m
//! a, hl; COPY v', hl; COPY w, v'; INC16 w`.
//!
//! DEVIATE: no LLVM pass. Reason: the pin's `InterferenceCache` takes the
//! fixed range of a register copied from the interval itself as interference
//! (the matrix's `CoalescerPair` check does not), so region splitting never
//! puts a pointer that is stepped after its load in the register that loads
//! it. Exit: none; the form is exact for a use that does not write the pair.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};
use llrm_support::hash::{HashMap, HashSet};

use crate::mfunc::MFunction;
use crate::regalloc::Class;
use crate::{REGISTER_INFO, reg};

const PAIRS: [llrm_lir::registers::RegId; 3] = [reg::HL, reg::DE, reg::BC];

/// Reads the pair again from its register wherever that spares a value its
/// overlap with the register's range.
pub fn read_pairs_from_their_registers(function: &mut MFunction) {
    // the blocks each value is mentioned in, and how often it is defined
    let mut blocks_of: HashMap<u32, HashSet<usize>> = HashMap::default();
    let mut defs: HashMap<u32, u32> = HashMap::default();
    for (at, block) in function.blocks.iter().enumerate() {
        for inst in &block.insts {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(v)) => {
                        *defs.entry(*v).or_default() += 1;
                        blocks_of.entry(*v).or_default().insert(at);
                    }
                    Operand::Use(Reg::Virt(v)) => {
                        blocks_of.entry(*v).or_default().insert(at);
                    }
                    _ => {}
                }
            }
        }
        for phi in &block.phis {
            for reg in std::iter::once(&phi.result).chain(phi.incoming.iter().map(|(_, reg)| reg)) {
                if let Reg::Virt(v) = reg {
                    blocks_of.entry(*v).or_default().insert(usize::MAX);
                }
            }
        }
    }
    for at in 0..function.blocks.len() {
        let mut from = 0;
        while let Some(done) = forward_one(function, at, from, &blocks_of, &defs) {
            from = done;
        }
    }
}

/// Whether `inst` writes `pair`, or a byte or the whole of it, or calls.
fn clobbers(
    inst: &Inst,
    pair: llrm_lir::registers::RegId,
) -> bool {
    inst.call
        || inst.operands.iter().any(|operand| {
            matches!(
                operand,
                Operand::Def(Reg::Phys(one)) if REGISTER_INFO.root(*one) == REGISTER_INFO.root(pair)
            )
        })
}

fn reads(
    inst: &Inst,
    reg: Reg,
) -> bool {
    inst.operands.iter().any(|operand| *operand == Operand::Use(reg))
}

/// Rewrites the first candidate of block `at` from instruction `from` on;
/// the index to go on from, or `None` when there is none.
fn forward_one(
    function: &mut MFunction,
    at: usize,
    from: usize,
    blocks_of: &HashMap<u32, HashSet<usize>>,
    defs: &HashMap<u32, u32>,
) -> Option<usize> {
    let insts = &function.blocks[at].insts;
    for k in from..insts.len() {
        let (Kind::Copy, [Operand::Def(Reg::Phys(pair)), Operand::Use(Reg::Virt(v))]) =
            (insts[k].kind, &insts[k].operands[..])
        else {
            continue;
        };
        let (pair, v) = (*pair, *v);
        if !PAIRS.contains(&pair) || defs.get(&v) != Some(&1) || blocks_of[&v].len() != 1 {
            continue;
        }
        // the consumer: the next reader of the pair, with the pair untouched
        let Some(m) = (k + 1..insts.len()).find(|&m| reads(&insts[m], Reg::Phys(pair))) else { continue };
        if (k + 1..m).any(|between| clobbers(&insts[between], pair)) || clobbers(&insts[m], pair) {
            continue;
        }
        // the last use of the value after it, with the pair untouched until
        // then and the value not written again
        let Some(last) = (m + 1..insts.len()).rev().find(|&j| reads(&insts[j], Reg::Virt(v))) else { continue };
        if (m + 1..last).any(|between| clobbers(&insts[between], pair))
            || (m + 1..=last).any(|j| insts[j].operands.iter().any(|operand| *operand == Operand::Def(Reg::Virt(v))))
        {
            continue;
        }
        let class: Class = function.classes[v as usize];
        let fresh = function.classes.len() as u32;
        function.classes.push(class);
        let insts = &mut function.blocks[at].insts;
        for inst in &mut insts[m + 1..=last] {
            for operand in &mut inst.operands {
                if *operand == Operand::Use(Reg::Virt(v)) {
                    *operand = Operand::Use(Reg::Virt(fresh));
                }
            }
        }
        insts.insert(m + 1, Inst::copy(Reg::Virt(fresh), Reg::Phys(pair)));
        return Some(m + 2);
    }
    None
}
