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
//! `rescheduleKillAboveMI` (L1095), called from `tryInstructionTransform`
//! (L1376).

use llrm_lir::registers::RegId;

use crate::commute::Facts;
use crate::mfunc::{FLAG_ALL, Inst, Kind, MFunction, Operand, Reg};
use crate::regalloc::Target;

/// The instructions after the tied one the walk looks through
/// (`rescheduleKillAboveMI`'s limit).
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

fn touches_flags(inst: &Inst) -> bool {
    inst.written_lanes() | inst.read_lanes() != 0
}

/// Whether two flag accesses may swap. Whole-flag accesses never do. Where a
/// side names lanes (`inc` keeps carry, a branch reads one lane), they swap
/// when neither reads what the other writes and the lanes both write are read
/// by nothing after them (`read_after`): the pin's register test on the
/// flags, where a def nobody reads is dead and is crossed.
fn flags_commute(
    kill: &Inst,
    other: &Inst,
    read_after: u8,
) -> bool {
    if !(touches_flags(kill) && touches_flags(other)) {
        return true;
    }
    let by_lane = [kill, other].iter().any(|inst| inst.flags_kept | inst.flags_read != 0);
    if !by_lane {
        return false;
    }
    let (kill_writes, other_writes) = (kill.written_lanes(), other.written_lanes());
    kill_writes & other.read_lanes() == 0
        && kill.read_lanes() & other_writes == 0
        && kill_writes & other_writes & read_after == 0
}

/// The flag lanes read by `insts`.
fn lanes_read(insts: &[Inst]) -> u8 {
    insts.iter().fold(0, |lanes, inst| lanes | inst.read_lanes())
}

/// Whether some block reads flags before it sets any.
fn flags_live_in<C>(function: &MFunction<C>) -> bool {
    function.blocks.iter().any(|block| {
        block.insts.iter().find(|inst| inst.sets_flags() || inst.reads_flags()).is_some_and(|inst| !inst.sets_flags())
    })
}

/// Whether `kill` can move above `other` (`isSafeToMove` and the register
/// checks): `other` neither defines what `kill` reads, nor reads or defines
/// what `kill` defines, and the two do not reorder memory or flag accesses.
fn can_pass<T: Target>(
    kill: &Inst,
    other: &Inst,
    read_after: u8,
) -> bool {
    if other.call || other.terminator || other.barrier || other.side_effects {
        return false;
    }
    if (kill.may_load && other.may_store) || (kill.may_store && (other.may_load || other.may_store)) {
        return false;
    }
    if !flags_commute(kill, other, read_after) {
        return false;
    }
    let (kill_defs, other_uses, other_defs) = (
        virt_defs(kill).collect::<Vec<_>>(),
        virt_uses(other).collect::<Vec<_>>(),
        virt_defs(other).collect::<Vec<_>>(),
    );
    if virt_uses(kill).any(|u| other_defs.contains(&u)) {
        return false;
    }
    if kill_defs.iter().any(|d| other_uses.contains(d) || other_defs.contains(d)) {
        return false;
    }
    let clash = |one: RegId, many: &[RegId]| many.iter().any(|r| T::overlaps(one, *r));
    let (other_phys_defs, other_phys_uses): (Vec<RegId>, Vec<RegId>) =
        (phys_defs(other).collect(), phys_uses(other).collect());
    if phys_uses(kill).any(|r| clash(r, &other_phys_defs)) {
        return false;
    }
    !phys_defs(kill).any(|r| clash(r, &other_phys_defs) || clash(r, &other_phys_uses))
}

/// Whether `copy` puts a value into a register that one of `readers` reads: a
/// fixed register (`lda v` before a store through `(p),y`) or a virtual one
/// that copy came through.
fn feeds<T: Target>(
    copy: &Inst,
    readers: &[Inst],
) -> bool {
    let (Kind::Copy, [Operand::Def(into), Operand::Use(Reg::Virt(_))]) = (copy.kind, &copy.operands[..]) else {
        return false;
    };
    match into {
        Reg::Phys(into) => readers.iter().flat_map(phys_uses).any(|r| T::overlaps(*into, r)),
        Reg::Virt(into) => readers.iter().flat_map(virt_uses).any(|u| u == *into),
    }
}

/// Moves the last read of a tied source above the tied instruction, where it is
/// the only read left and the walk between them is safe.
pub fn reschedule_kills<T: Target>(function: &mut MFunction<T::Class>) {
    let facts = Facts::of(function);
    let flags_across = flags_live_in(function);
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
                // the one later read of the source, within the window, that
                // ends it
                let readers: Vec<usize> =
                    (at + 1..insts.len()).filter(|i| virt_uses(&insts[*i]).any(|u| u == source)).collect();
                let &[last] = &readers[..] else { continue };
                if last > at + WINDOW || !facts.plainly_killed(function, b, last, source) {
                    continue;
                }
                // a compare's class copy (`d = copy source`) goes with the
                // one reader of `d` that follows it: the pin's compare reads
                // `source` itself
                let mut end = last;
                if matches!(insts[last].kind, Kind::Copy)
                    && let [Operand::Def(Reg::Virt(copied)), _] = &insts[last].operands[..]
                    && let Some(reader) = insts.get(last + 1)
                    && virt_uses(reader).any(|u| u == *copied)
                    && facts.plainly_killed(function, b, last + 1, *copied)
                    && !insts[last + 2..].iter().any(|i| virt_uses(i).any(|u| u == *copied))
                {
                    end = last + 1;
                }
                let kill = &insts[end];
                if kill.call || kill.terminator || kill.barrier || kill.side_effects || !kill.ties.is_empty() {
                    continue;
                }
                if matches!(kill.kind, Kind::Copy) {
                    // a copy is left to the coalescer
                    continue;
                }
                // the copies that feed it go with it (a value into the fixed
                // register of a store through `(p),y`): they
                // are one instruction to the allocator
                let mut start = last;
                while start > at + 1 && feeds::<T>(&insts[start - 1], &insts[start..=end]) {
                    start -= 1;
                }
                let moving = &insts[start..=end];
                let read_after = if flags_across { FLAG_ALL } else { lanes_read(&insts[end + 1..]) };
                if !insts[at..start].iter().all(|other| moving.iter().all(|one| can_pass::<T>(one, other, read_after)))
                {
                    continue;
                }
                // what the move relies on an instruction to keep, it keeps in
                // the classes that keep it
                let relied: Vec<u32> = insts[at..start]
                    .iter()
                    .filter(|other| other.flags_kept != 0)
                    .flat_map(|other| virt_uses(other).chain(virt_defs(other)))
                    .collect();
                let relied = if moving.iter().any(touches_flags) { relied } else { Vec::new() };
                let bundle: Vec<Inst> = function.blocks[b].insts.drain(start..=end).collect();
                for v in relied {
                    if let Some(narrowed) = T::keeping_flags(&function.classes[v as usize]) {
                        function.classes[v as usize] = narrowed;
                    }
                }
                let count = bundle.len();
                function.blocks[b].insts.splice(at..at, bundle);
                at += count;
                break;
            }
            at += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mfunc::{FLAG_C, FLAG_N, FLAG_V, FLAG_Z};

    fn setter(kept: u8) -> Inst {
        let mut inst = Inst::new(Kind::Op("SET"), vec![]);
        inst.sets_flags = true;
        inst.flags_kept = kept;
        inst
    }

    /// `cmp` moved above `inc` leaves `inc` between the compare and its
    /// branch: harmless for `bcc` (carry survives `inc`), wrong for `bne`
    /// (`inc` rewrites zero).
    #[test]
    fn a_compare_passes_an_increment_only_where_the_branch_reads_carry() {
        let (compare, increment) = (setter(0), setter(FLAG_C | FLAG_V));
        assert!(flags_commute(&compare, &increment, FLAG_C));
        assert!(!flags_commute(&compare, &increment, FLAG_Z));
        assert!(!flags_commute(&compare, &increment, FLAG_N));
    }

    /// A setter that names no lanes still blocks every swap.
    #[test]
    fn whole_flag_setters_never_swap() {
        assert!(!flags_commute(&setter(0), &setter(0), 0));
    }
}
