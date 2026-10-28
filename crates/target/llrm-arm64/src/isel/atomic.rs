//! Atomic accesses and fences, as the pin selects them for `apple-m1` (LSE and
//! RCPC): a load-acquire is `ldapr` (`ldar` for `seq_cst`), a store-release
//! `stlr`, a read-modify-write one LSE instruction (`ldadd`, `swp`, ...; `sub`
//! and `and` negate or complement the operand first), `cmpxchg` a `cas`, and a
//! fence a `dmb`. A relaxed access is an ordinary load or store.
//!
//! `nand`, the float operations and the wrapping increments have no LSE
//! instruction: LLVM writes a `ldaxr`/`stlxr` loop for them, which is refused
//! here.

use llrm_mir::Instruction;
use llrm_mir::opcode::{AtomicKind, AtomicOrdering, RmwOp};

use super::val::{AddrExpr, Base, G, Known, Val, width_of};
use super::{Selected, Selector, Src, refuse};
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::Op;

/// Whether a read-modify-write of `ordering` acquires and whether it releases.
fn acquires_and_releases(ordering: AtomicOrdering) -> (bool, bool) {
    match ordering {
        AtomicOrdering::Acquire => (true, false),
        AtomicOrdering::Release => (false, true),
        AtomicOrdering::AcqRel | AtomicOrdering::SeqCst => (true, true),
        AtomicOrdering::Unordered | AtomicOrdering::Monotonic => (false, false),
    }
}

fn log2(bytes: u8) -> u8 {
    bytes.trailing_zeros() as u8
}

impl Selector<'_> {
    /// A call of a `llrm.atomic.*` function (`llrm_mir::atomic::hidden`): the
    /// instruction it stands for. What has no instruction here (`nand`,
    /// floats, a loop of `ldaxr`/`stlxr`, widths over 64 bits) is refused.
    pub(super) fn atomic_call(
        &mut self,
        name: &str,
        instruction: &Instruction,
    ) -> Selected<()> {
        let Some(info) = llrm_mir::atomic::decode(name) else { return refuse(format!("{name}: not an atomic")) };
        let ordering = info.ordering;
        match info.kind {
            AtomicKind::Load if matches!(
                ordering,
                AtomicOrdering::Unordered | AtomicOrdering::Monotonic
            ) => {
                self.load(instruction)
            }
            AtomicKind::Load => self.ordered_load(instruction, ordering),
            AtomicKind::Store if matches!(
                ordering,
                AtomicOrdering::Unordered | AtomicOrdering::Monotonic
            ) => {
                self.store(instruction)
            }
            AtomicKind::Store => self.ordered_store(instruction),
            AtomicKind::Rmw(op) => self.atomic_rmw(instruction, op, ordering),
            AtomicKind::CmpXchg { weak: _, failure: _ } => self.cmpxchg(instruction, ordering),
            // A fence within one thread orders nothing for another.
            AtomicKind::Fence if info.single_thread => Ok(()),
            AtomicKind::Fence => self.fence(ordering),
        }
    }

    /// An atomic load stronger than relaxed: `ldapr`, or `ldar` for `seq_cst`.
    pub(super) fn ordered_load(
        &mut self,
        instruction: &Instruction,
        ordering: AtomicOrdering,
    ) -> Selected<()> {
        let (bytes, float) = self.access(instruction.ty)?;
        if float {
            return refuse("an atomic load of a float");
        }
        let address = self.pointer_operand(instruction.operands[0])?;
        let rn = self.pointer_register(&address)?;
        let rt = self.mf.gpr();
        let op = if ordering == AtomicOrdering::Acquire { Atomic::Ldapr } else { Atomic::Ldar };
        self.inst(Inst::Atomic { op, size: log2(bytes), acquire: true, release: false, rs: Gpr::R31, rt, rn });
        let value = self.loaded(instruction.ty, rt);
        self.define(instruction.result, Src::Val(value))
    }

    /// An atomic store stronger than relaxed: `stlr`.
    pub(super) fn ordered_store(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let stored = instruction.operands[0];
        let (bytes, float) = self.access(self.operand_type(stored)?)?;
        if float {
            return refuse("an atomic store of a float");
        }
        let value = self.src(stored)?;
        let rt = match value {
            Src::Int { value: 0, .. } => Gpr::R31,
            other => self.gpr_of(other)?.reg,
        };
        let address = self.pointer_operand(instruction.operands[1])?;
        let rn = self.pointer_register(&address)?;
        self.inst(Inst::Atomic {
            op: Atomic::Stlr,
            size: log2(bytes),
            acquire: false,
            release: true,
            rs: Gpr::R31,
            rt,
            rn,
        });
        Ok(())
    }

    /// `atomicrmw`: the old value is the result.
    pub(super) fn atomic_rmw(
        &mut self,
        instruction: &Instruction,
        op: RmwOp,
        ordering: AtomicOrdering,
    ) -> Selected<()> {
        let (bytes, float) = self.access(instruction.ty)?;
        if float {
            return refuse("an atomicrmw of a float");
        }
        let (acquire, release) = acquires_and_releases(ordering);
        let address = self.pointer_operand(instruction.operands[0])?;
        let rn = self.pointer_register(&address)?;
        let operand = self.src(instruction.operands[1])?;
        let width = if bytes == 8 { Width::X } else { Width::W };
        let mut rs = self.gpr_of(operand)?.reg;
        let kind = match op {
            RmwOp::Xchg => Atomic::Swp,
            RmwOp::Add => Atomic::Ldadd,
            RmwOp::Or => Atomic::Ldset,
            RmwOp::Xor => Atomic::Ldeor,
            RmwOp::Max => Atomic::Ldsmax,
            RmwOp::Min => Atomic::Ldsmin,
            RmwOp::UMax => Atomic::Ldumax,
            RmwOp::UMin => Atomic::Ldumin,
            // `x - v` is `x + -v`.
            RmwOp::Sub => {
                let negated = self.mf.gpr();
                self.inst(Inst::AddSubShifted {
                    op: AddSub { subtract: true, set_flags: false },
                    width,
                    rd: negated,
                    rn: Gpr::R31,
                    rm: rs,
                    shift: Shift::Lsl,
                    amount: 0,
                });
                rs = negated;
                Atomic::Ldadd
            }
            // `x & v` is `x` with the bits clear of `!v` cleared.
            RmwOp::And => {
                let complement = self.mf.gpr();
                self.inst(Inst::LogicShifted {
                    op: Logic::Orr,
                    invert: true,
                    width,
                    rd: complement,
                    rn: Gpr::R31,
                    rm: rs,
                    shift: Shift::Lsl,
                    amount: 0,
                });
                rs = complement;
                Atomic::Ldclr
            }
            other => return refuse(format!("atomicrmw {other:?}: it takes a load-exclusive loop")),
        };
        let rt = self.mf.gpr();
        self.inst(Inst::Atomic { op: kind, size: log2(bytes), acquire, release, rs, rt, rn });
        let value = self.loaded(instruction.ty, rt);
        self.define(instruction.result, Src::Val(value))
    }

    /// `cmpxchg`: `cas` writes the old value over the expected one; the pair is
    /// that and whether they were equal.
    pub(super) fn cmpxchg(
        &mut self,
        instruction: &Instruction,
        success: AtomicOrdering,
    ) -> Selected<()> {
        let [pointer, expected, new] = instruction.operands[..instruction.operands.len() - 1] else {
            return refuse("a cmpxchg without its operands");
        };
        let ty = self.operand_type(expected)?;
        let (bytes, float) = self.access(ty)?;
        if float {
            return refuse("a cmpxchg of a float");
        }
        let (acquire, release) = acquires_and_releases(success);
        let address = self.pointer_operand(pointer)?;
        let rn = self.pointer_register(&address)?;
        let bits = match self.ty(ty) {
            llrm_mir::Type::Int(bits) => *bits,
            _ => 64,
        };
        let expected_src = self.src(expected)?;
        let expected = self.gpr_of(expected_src)?;
        let expected = self.extended(G { bits, ..expected }, Known::Zero);
        let new = match self.src(new)? {
            Src::Int { value: 0, .. } => Gpr::R31,
            other => self.gpr_of(other)?.reg,
        };
        // `cas` overwrites its first register with the old value.
        let old = self.mf.gpr();
        self.emit(Op::Copy { dst: Reg::Gpr(old), src: Reg::Gpr(expected.reg) });
        self.inst(Inst::Atomic { op: Atomic::Cas, size: log2(bytes), acquire, release, rs: old, rt: new, rn });
        let width = width_of(bits);
        self.inst(Inst::AddSubShifted {
            op: AddSub { subtract: true, set_flags: true },
            width,
            rd: Gpr::R31,
            rn: old,
            rm: expected.reg,
            shift: Shift::Lsl,
            amount: 0,
        });
        let equal = self.mf.gpr();
        // cset equal, eq = csinc equal, wzr, wzr, ne
        self.inst(Inst::CondSelect {
            op: Select::Csinc,
            width: Width::W,
            rd: equal,
            rn: Gpr::R31,
            rm: Gpr::R31,
            cond: Cond::Ne,
        });
        let old = self.loaded(ty, old);
        let flag = Val::Int(G { reg: equal, bits: 1, known: Known::Zero });
        self.define(instruction.result, Src::Val(Val::Agg(vec![old, flag])))
    }

    /// `fence`: an acquire fence orders loads before it (`dmb ishld`); the
    /// others, all accesses (`dmb ish`).
    pub(super) fn fence(
        &mut self,
        ordering: AtomicOrdering,
    ) -> Selected<()> {
        let barrier = if ordering == AtomicOrdering::Acquire { Barrier::IshLd } else { Barrier::Ish };
        self.inst(Inst::Dmb { barrier });
        Ok(())
    }

    /// The value of type `ty` an atomic instruction left in `reg`:
    /// zero-extended from its size.
    fn loaded(
        &self,
        ty: llrm_mir::TypeId,
        reg: Gpr,
    ) -> Val {
        match self.ty(ty) {
            llrm_mir::Type::Pointer(_) => Val::Ptr(AddrExpr::of(Base::Reg(reg))),
            llrm_mir::Type::Int(bits) => {
                let bits = *bits;
                let known = if bits == 64 { Known::Nothing } else { Known::Zero };
                Val::Int(G { reg, bits, known })
            }
            _ => Val::Int(G { reg, bits: 64, known: Known::Nothing }),
        }
    }
}
