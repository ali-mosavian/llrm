//! A compare with zero of a value made in the same block goes into the
//! instruction that made it: `add w8, w0, w1; cmp w8, #0; b.eq` is
//! `adds w8, w0, w1; b.eq`.
//!
//! Translated from `AArch64InstrInfo::optimizeCompareInstr` and its
//! `substituteCmpToZero`
//! ([AArch64InstrInfo.cpp#L1721](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64InstrInfo.cpp#L1721)),
//! which the pin's `PeepholeOptimizer` runs on every `SUBS` whose result is
//! dead.
//!
//! # How it works
//! Machine form, virtual registers, before allocation. For `cmp x, #0`, `x` is
//! defined once, earlier in the block, by an add, subtract or `and` that has a
//! flag-setting form (`convertToFlagSettingOpc`). Nothing between the two may
//! read or write the flags (`areCFlagsAccessedBetweenInstrs`), and every reader
//! of the flags after the compare, up to the next instruction that writes them,
//! must read only N and Z (`eq`, `ne`, `mi`, `pl`): the compare of zero sets C
//! and V in a way the instruction does not. The instruction becomes its
//! flag-setting form and the compare goes. The flags do not live out of a
//! block.
//!
//! ```text
//!   and w8, w0, #0xff        ands w8, w0, #0xff
//!   cmp w8, #0         =>    b.ne L
//!   b.ne L
//! ```
//!
//! Not translated: `cmp x, #imm` after an identical `subs` (`canInstrSubstituteCmpInstr`),
//! `adc`/`sbc` as the maker, and the `C`/`V` readers the pin allows after an `adds`/`subs` whose
//! flags prove them.

use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op};

/// The register `op` compares with zero (`cmp x, #0`), and its width.
fn compared_with_zero(op: &Op) -> Option<Gpr> {
    match op {
        Op::I(Inst::AddSubImm { op: AddSub { subtract: true, set_flags: true }, rd: Gpr::R31, rn, imm12: 0, .. })
            if rn.is_virtual() =>
        {
            Some(*rn)
        }
        _ => None,
    }
}

/// The condition `op` reads the flags with; `Some(None)` where it reads them
/// in a way this pass does not follow.
fn flag_reader(op: &Op) -> Option<Option<Cond>> {
    match op {
        Op::I(
            Inst::BranchCond { cond, .. } | Inst::CondSelect { cond, .. } | Inst::CondCompare { cond, .. },
        ) => Some(Some(*cond)),
        Op::I(Inst::FpSelect { cond, .. }) => Some(Some(*cond)),
        _ => None,
    }
}

/// `convertToFlagSettingOpc`: the instruction that does `op` and sets the flags.
fn flag_setting_form(op: &mut Op) -> bool {
    match op {
        Op::I(
            Inst::AddSubImm { op, .. } | Inst::AddSubShifted { op, .. } | Inst::AddSubExtended { op, .. },
        ) if !op.set_flags => {
            op.set_flags = true;
            true
        }
        Op::I(Inst::LogicImm { op, .. } | Inst::LogicShifted { op, .. }) if *op == Logic::And => {
            *op = Logic::Ands;
            true
        }
        _ => false,
    }
}

/// Whether `op` (not yet converted) is one `flag_setting_form` converts.
fn convertible(op: &Op) -> bool {
    matches!(
        op,
        Op::I(
            Inst::AddSubImm { op: AddSub { set_flags: false, .. }, .. }
                | Inst::AddSubShifted { op: AddSub { set_flags: false, .. }, .. }
                | Inst::AddSubExtended { op: AddSub { set_flags: false, .. }, .. }
                | Inst::LogicImm { op: Logic::And, .. }
                | Inst::LogicShifted { op: Logic::And, .. }
        )
    )
}

/// Whether `op` reads the flags with a condition other than N or Z.
fn reads_beyond_nz(op: &Op) -> bool {
    !matches!(flag_reader(op), None | Some(Some(Cond::Eq | Cond::Ne | Cond::Mi | Cond::Pl)))
}

/// `counts[i]` of the first `i` operations that `is` holds for.
fn prefix(
    ops: &[Op],
    is: impl Fn(&Op) -> bool,
) -> Vec<usize> {
    let mut counts = Vec::with_capacity(ops.len() + 1);
    counts.push(0);
    for op in ops {
        counts.push(counts[counts.len() - 1] + usize::from(is(op)));
    }
    counts
}

pub fn run(function: &mut MFunc) {
    let defined_once = crate::prepass::definitions(function);
    for block in &mut function.blocks {
        let ops: Vec<Op> = block.ops.iter().cloned().collect();
        let writes = prefix(&ops, crate::flagcse::writes_flags);
        let reads = prefix(&ops, |op| flag_reader(op).is_some());
        let beyond = prefix(&ops, reads_beyond_nz);
        // The first flag write at or after each operation.
        let mut next_write = vec![ops.len(); ops.len() + 1];
        for at in (0..ops.len()).rev() {
            next_write[at] = if writes[at + 1] > writes[at] { at } else { next_write[at + 1] };
        }
        let mut last_def: rustc_hash::FxHashMap<Reg, usize> = Default::default();
        let mut converted: Vec<usize> = Vec::new();
        let mut dropped = vec![false; ops.len()];
        for (at, op) in ops.iter().enumerate() {
            if let Some(register) = compared_with_zero(op)
                && defined_once.get(&Reg::Gpr(register)) == Some(&1)
                && let Some(&maker) = last_def.get(&Reg::Gpr(register))
                && convertible(&ops[maker])
                && writes[at] == writes[maker + 1]
                && reads[at] == reads[maker + 1]
                // the readers after, through the next write (which may read)
                && beyond[next_write[at + 1].saturating_add(1).min(ops.len())] == beyond[at + 1]
            {
                converted.push(maker);
                dropped[at] = true;
            }
            for register in op.defs_and_uses().0 {
                last_def.insert(register, at);
            }
        }
        for &maker in &converted {
            let done = flag_setting_form(block.ops.op_mut(maker));
            debug_assert!(done, "a convertible maker converts");
        }
        let mut index = 0;
        block.ops.retain(|_| {
            index += 1;
            !dropped[index - 1]
        });
    }
}
