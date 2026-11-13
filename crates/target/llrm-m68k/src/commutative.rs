//! The two-address choice for the register adds, ands and ors the pin leaves
//! unmarked commutative.
//!
//! # How it works
//! `s = s + i` over a loop-carried `s` and a counter `i` that outlives the
//! add: the tie to `i` made `move.l %d1,%d2; add.l %d0,%d2; move.l %d2,%d0`
//! out of `add.l %d1,%d0`. [`mark_commutative`] marks such an instruction
//! commutative ([`crate::foldload::commutes`]) so `commute_tied` may tie the
//! result to the source that dies, except where the result flows back into
//! the variable the tied source came from (`b' = a + b; a' = b`): there the
//! tie already coalesces, and swapping it left a copy in the rotation (fib).
//! Only a source that dies here while the tied one lives on is worth the
//! swap: when both die, the pin's later-defined rule picked an address
//! register's copy and cost a move (hex). Beyond the accumulator `s = s + x`,
//! which saves a copy per trip, the swap is for size alone: at speed it moved
//! the allocation of the big seeds both ways (68000 -O2 +86 B net).
//!
//! Both sources dying is worth the swap when the result is copied into the
//! physical register the other source was copied from (`r0 = ADD (MOV r1),
//! (MOV r0)`): `commute_tied` then ties the result to it, as the pin's
//! `SrcRegMap`/`DstRegMap` rule does, and the return copy goes.
//!
//! DEVIATE: the pin marks `ADD32dd` commutative and not its `ADD32dr` twin.
//! Reason: both are the same add, and the tie to the dying source saves the
//! copies. Exit: review 2026-11-20, a policy choice.
//!
//! # See also
//! `TwoAddressInstructionPass::isProfitableToCommute`.

use llrm_lir::registers::RegId;
use llrm_spike_mf::{Kind, Operand, Reg};
use llrm_support::hash::{HashMap, HashSet};

use crate::foldload::commutes;
use crate::mfunc::MFunction;

/// How far behind a register a copy chain is followed.
const MAX_CHAIN: usize = 10;

/// The registers a value reaches through copies, and the one a copy of it
/// came from.
struct Flow {
    copies_of: HashMap<u32, Vec<u32>>,
    copied_from: HashMap<u32, Vec<u32>>,
    /// Registers with more than one definition: a phi's variable. A chain
    /// ends there, or it would run once round the loop and reach everything.
    joins: HashSet<u32>,
    /// How many instructions read each register.
    reads: HashMap<u32, u32>,
    /// The register a register is the only copy of.
    source: HashMap<u32, u32>,
    /// The physical registers a register is copied into.
    phys_out: HashMap<u32, Vec<RegId>>,
    /// The physical registers a register is copied from.
    phys_in: HashMap<u32, Vec<RegId>>,
}

impl Flow {
    fn of(function: &MFunction) -> Flow {
        let mut flow = Flow {
            copies_of: HashMap::default(),
            copied_from: HashMap::default(),
            joins: HashSet::default(),
            reads: HashMap::default(),
            source: HashMap::default(),
            phys_out: HashMap::default(),
            phys_in: HashMap::default(),
        };
        let mut defs: HashMap<u32, u32> = HashMap::default();
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(reg)) => *defs.entry(*reg).or_default() += 1,
                    Operand::Use(Reg::Virt(reg)) => *flow.reads.entry(*reg).or_default() += 1,
                    _ => {}
                }
            }
            if let (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))]) =
                (inst.kind, &inst.operands[..])
            {
                flow.copies_of.entry(*from).or_default().push(*to);
                flow.copied_from.entry(*to).or_default().push(*from);
            }
        }
        for inst in function.blocks.iter().flat_map(|block| &block.insts) {
            match (inst.kind, &inst.operands[..]) {
                (Kind::Copy, [Operand::Def(Reg::Phys(to)), Operand::Use(Reg::Virt(from))]) => {
                    flow.phys_out.entry(*from).or_default().push(*to)
                }
                (Kind::Copy, [Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Phys(from))]) => {
                    flow.phys_in.entry(*to).or_default().push(*from)
                }
                _ => {}
            }
        }
        for (to, from) in &flow.copied_from {
            if defs.get(to) == Some(&1) && from.len() == 1 {
                flow.source.insert(*to, from[0]);
            }
        }
        flow.joins = defs.into_iter().filter(|(_, count)| *count > 1).map(|(reg, _)| reg).collect();
        flow
    }

    fn closure(
        &self,
        map: &HashMap<u32, Vec<u32>>,
        start: u32,
    ) -> HashSet<u32> {
        let mut seen: HashSet<u32> = HashSet::default();
        let mut work = vec![start];
        while let Some(reg) = work.pop() {
            if seen.insert(reg) && (reg == start || !self.joins.contains(&reg)) {
                work.extend(map.get(&reg).into_iter().flatten().copied());
            }
        }
        seen
    }

    /// Whether the value `reg` holds is read here and nowhere else: each
    /// register of the copy chain behind it has this one reader.
    fn dies_here(
        &self,
        reg: u32,
    ) -> bool {
        let mut reg = reg;
        for _ in 0..MAX_CHAIN {
            if self.reads.get(&reg) != Some(&1) {
                return false;
            }
            match self.source.get(&reg) {
                Some(from) => reg = *from,
                None => return true,
            }
        }
        false
    }

    /// Whether `result` is copied, through copies, into a physical register
    /// that `source` was copied from.
    fn returns_through_phys(
        &self,
        result: u32,
        source: u32,
    ) -> bool {
        let out: Vec<RegId> =
            self.closure(&self.copies_of, result).iter().filter_map(|r| self.phys_out.get(r)).flatten().copied().collect();
        self.closure(&self.copied_from, source)
            .iter()
            .filter_map(|r| self.phys_in.get(r))
            .flatten()
            .any(|reg| out.contains(reg))
    }

    /// Whether `result` reaches, through copies, a register `source` is a copy
    /// of.
    fn returns_to(
        &self,
        result: u32,
        source: u32,
    ) -> bool {
        let after = self.closure(&self.copies_of, result);
        self.closure(&self.copied_from, source).iter().any(|reg| *reg != source && after.contains(reg))
    }
}

pub fn mark_commutative(
    function: &mut MFunction,
    size: bool,
) {
    let flow = Flow::of(function);
    for block in &mut function.blocks {
        for inst in &mut block.insts {
            let Kind::Op(name) = inst.kind else { continue };
            if inst.commutable || !commutes(name, false) || inst.ties.len() != 1 {
                continue;
            }
            let (def, used) = inst.ties[0];
            let (Operand::Def(Reg::Virt(result)), Operand::Use(Reg::Virt(base))) =
                (&inst.operands[def], &inst.operands[used])
            else {
                continue;
            };
            let other = inst
                .operands
                .iter()
                .enumerate()
                .find_map(
                    |(at, operand)| match operand {
                        Operand::Use(Reg::Virt(reg)) if at != used && reg != base => Some(*reg),
                        _ => None,
                    },
                );
            let Some(other) = other else { continue };
            let returns_to_base = flow.returns_to(*result, *base) && !flow.returns_to(*result, other);
            let wanted = size || flow.returns_to(*result, other);
            inst.commutable = wanted && flow.dies_here(other) && !flow.dies_here(*base) && !returns_to_base
                || flow.dies_here(other) && flow.dies_here(*base) && flow.returns_through_phys(*result, other);
        }
    }
}
