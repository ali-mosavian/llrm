//! `TwoAddressInstructionPass::rescheduleKillAboveMI`: a tied instruction whose
//! source is read again later has that last read moved above it, so the source
//! dies at the instruction and the tie copy joins.
//!
//! `p2 = p + 1; x = load p` keeps `p` and `p2` apart, and a copy of `p` before
//! the increment is the price. With the load first, `p` ends at the increment
//! and the increment is done in place.
//!
//! DEVIATE: stopgap until the neutral-LIR task, which uses `llrm-codegen`'s
//! `twoaddress.rs`. Kills come from block liveness over virtual registers
//! (`commute::Facts`), not `LiveIntervals`; registers the moved instruction
//! touches are compared through [`Target::overlaps`] instead of by
//! `MachineInstr` operand flags. The window and the barrier rules are the
//! pin's. Exit: the neutral-LIR task.
//!
//! # See also
//! `llvm/lib/CodeGen/TwoAddressInstructionPass.cpp`, pin d1106deb71cc:
//! `rescheduleKillAboveMI` (L1095), called from `tryInstructionTransform` (L1376).

use llrm_lir::registers::RegId;

use crate::commute::Facts;
use crate::mfunc::{Inst, MFunction, Operand, Reg};
use crate::regalloc::Target;

/// The instructions after the tied one the walk looks through (`rescheduleKillAboveMI`'s limit).
const WINDOW: usize = 11;

fn virt_uses(inst: &Inst) -> impl Iterator<Item = u32> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Use(Reg::Virt(v)) = o { Some(*v) } else { None })
}

fn virt_defs(inst: &Inst) -> impl Iterator<Item = u32> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Def(Reg::Virt(v)) = o { Some(*v) } else { None })
}

fn phys_uses(inst: &Inst) -> impl Iterator<Item = RegId> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Use(Reg::Phys(r)) = o { Some(*r) } else { None })
}

fn phys_defs(inst: &Inst) -> impl Iterator<Item = RegId> + '_ {
    inst.operands.iter().filter_map(|o| if let Operand::Def(Reg::Phys(r)) = o { Some(*r) } else { None })
}

/// Whether `kill` can move above `other` (`isSafeToMove` and the register checks): `other` neither
/// defines what `kill` reads, nor reads or defines what `kill` defines, and the two do not reorder
/// memory or flag accesses.
fn can_pass<T: Target>(
    kill: &Inst,
    other: &Inst,
) -> bool {
    if other.call || other.terminator || other.barrier || other.side_effects {
        return false;
    }
    if (kill.may_load && other.may_store) || (kill.may_store && (other.may_load || other.may_store)) {
        return false;
    }
    if (kill.sets_flags || kill.reads_flags) && (other.sets_flags || other.reads_flags) {
        return false;
    }
    let (kill_defs, other_uses, other_defs) =
        (virt_defs(kill).collect::<Vec<_>>(), virt_uses(other).collect::<Vec<_>>(), virt_defs(other).collect::<Vec<_>>());
    if virt_uses(kill).any(|u| other_defs.contains(&u)) {
        return false;
    }
    if kill_defs.iter().any(|d| other_uses.contains(d) || other_defs.contains(d)) {
        return false;
    }
    let clash = |one: RegId, many: &[RegId]| many.iter().any(|r| T::overlaps(one, *r));
    let (other_phys_defs, other_phys_uses): (Vec<RegId>, Vec<RegId>) = (phys_defs(other).collect(), phys_uses(other).collect());
    if phys_uses(kill).any(|r| clash(r, &other_phys_defs)) {
        return false;
    }
    !phys_defs(kill).any(|r| clash(r, &other_phys_defs) || clash(r, &other_phys_uses))
}

/// Moves the last read of a tied source above the tied instruction, where it is the only read left
/// and the walk between them is safe.
pub fn reschedule_kills<T: Target>(function: &mut MFunction<T::Class>) {
    let facts = Facts::of(function);
    for b in 0..function.blocks.len() {
        let mut at = 0;
        while at < function.blocks[b].insts.len() {
            let ties = function.blocks[b].insts[at].ties.clone();
            for (def, used) in ties {
                let insts = &function.blocks[b].insts;
                let (Operand::Def(Reg::Virt(into)), Operand::Use(Reg::Virt(source))) =
                    (&insts[at].operands[def], &insts[at].operands[used])
                else {
                    continue;
                };
                let (into, source) = (*into, *source);
                if into == source || facts.plainly_killed(function, b, at, source) {
                    continue;
                }
                // the one later read of the source, within the window, that ends it
                let readers: Vec<usize> = (at + 1..insts.len()).filter(|i| virt_uses(&insts[*i]).any(|u| u == source)).collect();
                let &[last] = &readers[..] else { continue };
                if last > at + WINDOW || !facts.plainly_killed(function, b, last, source) {
                    continue;
                }
                let kill = &insts[last];
                if kill.call || kill.terminator || kill.barrier || kill.side_effects || !kill.ties.is_empty() {
                    continue;
                }
                if matches!(kill.kind, crate::mfunc::Kind::Copy) {
                    // a copy is left to the coalescer
                    continue;
                }
                if !insts[at..last].iter().all(|other| can_pass::<T>(kill, other)) {
                    continue;
                }
                let moved = function.blocks[b].insts.remove(last);
                function.blocks[b].insts.insert(at, moved);
                at += 1;
                break;
            }
            at += 1;
        }
    }
}
