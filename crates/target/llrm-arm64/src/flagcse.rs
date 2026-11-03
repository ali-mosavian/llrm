//! A comparison the flags already hold is not made again: a `cmp`, `cmn`,
//! `tst` or `fcmp` identical to the last flag-setting instruction of its
//! block, with nothing between that sets the flags or redefines an operand, is
//! dropped (the pin's SelectionDAG shares one compare among its `setcc`s, and
//! `AArch64RedundantCopyElim`/MachineCSE take the rest).
//!
//! # How it works
//! Machine form, virtual registers, before allocation. Per block, the last
//! comparison is remembered with the registers it read; the next identical one
//! is deleted while those registers are unchanged and no instruction in
//! between writes the flags (another compare, a flag-setting arithmetic or
//! logic instruction, `ccmp`, a call).
//!
//! ```text
//!   fcmp d0, d1 ; cset w8, mi ; fcmp d0, d1 ; cset w9, ls
//!   fcmp d0, d1 ; cset w8, mi ;              cset w9, ls
//! ```

use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op};

/// Whether `inst` only compares: sets the flags and writes no register.
fn is_compare(inst: &Inst) -> bool {
    match inst {
        Inst::FpCompare { .. } => true,
        Inst::AddSubImm { op, rd, .. } | Inst::AddSubShifted { op, rd, .. } | Inst::AddSubExtended { op, rd, .. } => {
            op.set_flags && *rd == Gpr::R31
        }
        Inst::LogicImm { op: Logic::Ands, rd, .. } | Inst::LogicShifted { op: Logic::Ands, rd, .. } => *rd == Gpr::R31,
        _ => false,
    }
}

/// Whether `op` may write the flags.
pub(crate) fn writes_flags(op: &Op) -> bool {
    match op {
        Op::Call { .. } | Op::Return { .. } => true,
        Op::I(inst) => match inst {
            Inst::AddSubImm { op, .. } | Inst::AddSubShifted { op, .. } | Inst::AddSubExtended { op, .. } => {
                op.set_flags
            }
            Inst::LogicImm { op, .. } | Inst::LogicShifted { op, .. } => *op == Logic::Ands,
            Inst::FpCompare { .. } | Inst::CondCompare { .. } | Inst::Svc { .. } => true,
            _ => false,
        },
        _ => false,
    }
}

pub fn run(function: &mut MFunc) {
    for block in &mut function.blocks {
        // The last comparison, as text, and the registers it read.
        let mut held: Option<(String, Vec<Reg>)> = None;
        block
            .ops
            .retain(
                |op| {
                    let (defs, uses) = op.defs_and_uses();
                    if let Some((_, read)) = &held
                        && defs.iter().any(|def| read.contains(def))
                    {
                        held = None;
                    }
                    match op {
                        Op::I(inst) if is_compare(inst) => {
                            let key = format!("{inst:?}");
                            if held.as_ref().is_some_and(|(kept, _)| *kept == key) {
                                return false;
                            }
                            held = Some((key, uses));
                        }
                        _ if writes_flags(op) => held = None,
                        _ => {}
                    }
                    true
                },
            );
    }
}
