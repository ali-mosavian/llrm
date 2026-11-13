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
use crate::mc::visit::{Reg, RegList, RegRef};
use rustc_hash::FxHashMap as HashMap;
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
            Inst::AddCarry { op, .. } => op.set_flags,
            Inst::FpCompare { .. } | Inst::CondCompare { .. } | Inst::Svc { .. } => true,
            _ => false,
        },
        _ => false,
    }
}

/// The virtual registers defined once, by a copy of another register defined
/// once: a read of one is a read of the other (`MachineCSE`'s trivial copy
/// propagation, which the pin runs before it compares instructions).
fn copy_sources(function: &MFunc) -> HashMap<Reg, Reg> {
    let mut defs: HashMap<Reg, u32> = HashMap::default();
    for block in &function.blocks {
        for op in &block.ops {
            for reg in op.defs_and_uses().0.iter().filter(|reg| reg.is_virtual()) {
                *defs.entry(*reg).or_default() += 1;
            }
        }
    }
    let once = |reg: &Reg| defs.get(reg).copied() == Some(1);
    let mut out = HashMap::default();
    for block in &function.blocks {
        for op in &block.ops {
            if let Op::Copy { dst, src } = op
                && dst.is_virtual()
                && src.is_virtual()
                && once(dst)
                && once(src)
            {
                out.insert(*dst, *src);
            }
        }
    }
    out
}

/// `inst` with each register it reads named by its copy's source.
fn canonical(
    inst: &Inst,
    sources: &HashMap<Reg, Reg>,
) -> Inst {
    if sources.is_empty() {
        return inst.clone();
    }
    let mut inst = inst.clone();
    inst.visit(&mut |reg, role| {
        if !role.reads() || role.writes() {
            return;
        }
        let mut key = reg.key();
        let mut moved = false;
        while let Some(&next) = sources.get(&key) {
            key = next;
            moved = true;
        }
        if !moved {
            return;
        }
        match (reg, key) {
            (RegRef::Gpr(r), Reg::Gpr(to)) => *r = to,
            (RegRef::Fpr(r), Reg::Fpr(to)) => *r = to,
            _ => {}
        }
    });
    inst
}

pub fn run(function: &mut MFunc) {
    let sources = copy_sources(function);
    for block in &mut function.blocks {
        // The last comparison, as text, and the registers it read.
        let mut held: Option<(Inst, RegList)> = None;
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
                            let key = canonical(inst, &sources);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mfunc::MBlock;

    fn compare(rn: u32) -> Op {
        Op::I(Inst::AddSubImm {
            op: AddSub { subtract: true, set_flags: true },
            width: Width::X,
            rd: Gpr::R31,
            rn: Gpr(rn),
            imm12: 5,
            shift12: false,
        })
    }

    fn compares(function: &MFunc) -> usize {
        function.blocks.iter().flat_map(|block| block.ops.iter()).filter(|op| matches!(op, Op::I(inst) if is_compare(inst))).count()
    }

    /// The second compare reads a copy of the first one's register: the flags
    /// already hold the answer. Both compares stayed (a `cmp` made twice in
    /// the loop of mandel when early if-conversion put them in one block).
    #[test]
    fn a_compare_of_a_copy_is_the_compare_the_flags_hold() {
        let mut function = MFunc::new("f");
        function.blocks.push(MBlock {
            label: "b".to_owned(),
            ops: vec![
                Op::Copy { dst: Reg::Gpr(Gpr(66)), src: Reg::Gpr(Gpr(65)) },
                compare(65),
                Op::I(Inst::CondSelect {
                    op: Select::Csel,
                    width: Width::X,
                    rd: Gpr(67),
                    rn: Gpr(65),
                    rm: Gpr(66),
                    cond: Cond::Hi,
                }),
                compare(66),
            ]
            .into(),
        });
        // 65 and 66 are each defined once (65 by the caller's value, here a
        // second copy).
        function.blocks[0].ops.insert(0, Op::Copy { dst: Reg::Gpr(Gpr(65)), src: Reg::Gpr(Gpr(0)) });
        run(&mut function);
        assert_eq!(compares(&function), 1);
    }

    /// A register defined twice (a phi's copies) is not one value: its copy
    /// does not stand for it.
    #[test]
    fn a_register_defined_twice_is_not_looked_through() {
        let mut function = MFunc::new("f");
        function.blocks.push(MBlock {
            label: "b".to_owned(),
            ops: vec![
                Op::Copy { dst: Reg::Gpr(Gpr(65)), src: Reg::Gpr(Gpr(0)) },
                Op::Copy { dst: Reg::Gpr(Gpr(65)), src: Reg::Gpr(Gpr(1)) },
                Op::Copy { dst: Reg::Gpr(Gpr(66)), src: Reg::Gpr(Gpr(65)) },
                compare(65),
                Op::Copy { dst: Reg::Gpr(Gpr(65)), src: Reg::Gpr(Gpr(2)) },
                compare(66),
            ]
            .into(),
        });
        run(&mut function);
        assert_eq!(compares(&function), 2);
    }
}
