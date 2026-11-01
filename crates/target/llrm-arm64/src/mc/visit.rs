//! The registers an `Inst` names and how it uses each: the walk the register
//! allocator rewrites and reads liveness from (LLVM's `MachineOperand` list
//! with its `isDef`/`isUse` flags).

use super::inst::*;

/// A register operand, of either class.
#[derive(Debug, Eq, PartialEq)]
pub enum RegRef<'a> {
    Gpr(&'a mut Gpr),
    Fpr(&'a mut Vreg),
}

impl RegRef<'_> {
    pub fn key(&self) -> Reg {
        match self {
            RegRef::Gpr(register) => Reg::Gpr(**register),
            RegRef::Fpr(register) => Reg::Fpr(**register),
        }
    }
}

/// A register of either class, as a value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Reg {
    Gpr(Gpr),
    Fpr(Vreg),
}

impl Reg {
    pub fn is_virtual(self) -> bool {
        match self {
            Reg::Gpr(register) => register.is_virtual(),
            Reg::Fpr(register) => register.is_virtual(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Def,
    Use,
    /// Read and written: `movk`'s destination, a base register updated by a
    /// pre- or post-indexed access.
    UseDef,
}

impl Role {
    pub fn reads(self) -> bool {
        self != Role::Def
    }

    pub fn writes(self) -> bool {
        self != Role::Use
    }
}

fn address(
    addr: &mut Addr,
    f: &mut dyn FnMut(RegRef<'_>, Role),
) {
    match addr {
        Addr::Offset { base, .. } | Addr::PageOffset { base, .. } => f(RegRef::Gpr(base), Role::Use),
        Addr::Pre { base, .. } | Addr::Post { base, .. } => f(RegRef::Gpr(base), Role::UseDef),
        Addr::Register { base, index, .. } => {
            f(RegRef::Gpr(base), Role::Use);
            f(RegRef::Gpr(index), Role::Use);
        }
        Addr::Slot { .. } => {}
    }
}

fn memory(
    mem: &mut Memory,
    role: Role,
    f: &mut dyn FnMut(RegRef<'_>, Role),
) {
    match mem {
        Memory::Int { reg, .. } => f(RegRef::Gpr(reg), role),
        Memory::Fp { reg, .. } => f(RegRef::Fpr(reg), role),
    }
}

impl Inst {
    /// `f` on each register operand with its role, the destination first.
    pub fn visit(
        &mut self,
        f: &mut dyn FnMut(RegRef<'_>, Role),
    ) {
        use RegRef::{Fpr, Gpr as G};
        match self {
            Inst::AddSubImm { rd, rn, .. } | Inst::AddPageOffset { rd, rn, .. } | Inst::LogicImm { rd, rn, .. } => {
                f(G(rd), Role::Def);
                f(G(rn), Role::Use);
            }
            Inst::AddSubShifted { rd, rn, rm, .. }
            | Inst::AddSubExtended { rd, rn, rm, .. }
            | Inst::LogicShifted { rd, rn, rm, .. }
            | Inst::Divide { rd, rn, rm, .. } => {
                f(G(rd), Role::Def);
                f(G(rn), Role::Use);
                f(G(rm), Role::Use);
            }
            Inst::MoveWide { op, rd, .. } => f(G(rd), if *op == Wide::Keep { Role::UseDef } else { Role::Def }),
            Inst::BitfieldMove { op, rd, rn, .. } => {
                f(G(rd), if *op == Bitfield::Bfm { Role::UseDef } else { Role::Def });
                f(G(rn), Role::Use);
            }
            Inst::Extract { rd, rn, rm, .. } => {
                f(G(rd), Role::Def);
                f(G(rn), Role::Use);
                f(G(rm), Role::Use);
            }
            Inst::OneSource { rd, rn, .. } => {
                f(G(rd), Role::Def);
                f(G(rn), Role::Use);
            }
            Inst::ThreeSource { rd, rn, rm, ra, .. } => {
                f(G(rd), Role::Def);
                f(G(rn), Role::Use);
                f(G(rm), Role::Use);
                f(G(ra), Role::Use);
            }
            Inst::CondSelect { rd, rn, rm, .. } => {
                f(G(rd), Role::Def);
                f(G(rn), Role::Use);
                f(G(rm), Role::Use);
            }
            Inst::CondCompare { rn, rm_or_imm, .. } => {
                f(G(rn), Role::Use);
                if let CcOperand::Reg(rm) = rm_or_imm {
                    f(G(rm), Role::Use);
                }
            }
            Inst::Atomic { op, rs, rt, rn, .. } => {
                match op {
                    Atomic::Ldar | Atomic::Ldapr => f(G(rt), Role::Def),
                    Atomic::Stlr => f(G(rt), Role::Use),
                    Atomic::Cas => {
                        f(G(rs), Role::UseDef);
                        f(G(rt), Role::Use);
                    }
                    _ => {
                        f(G(rs), Role::Use);
                        f(G(rt), Role::Def);
                    }
                }
                f(G(rn), Role::Use);
            }
            Inst::Branch { .. }
            | Inst::BranchCond { .. }
            | Inst::Svc { .. }
            | Inst::Brk { .. }
            | Inst::Dmb { .. }
            | Inst::Nop => {}
            Inst::CompareBranch { rt, .. } | Inst::TestBranch { rt, .. } => f(G(rt), Role::Use),
            Inst::Cfi(_) | Inst::Mark(_) => {}
            Inst::BranchReg { rn, .. } | Inst::Ret { rn } => f(G(rn), Role::Use),
            Inst::Load { mem, addr, .. } => {
                memory(mem, Role::Def, f);
                address(addr, f);
            }
            Inst::Store { mem, addr, .. } => {
                memory(mem, Role::Use, f);
                address(addr, f);
            }
            Inst::LoadPair { first, second, addr } => {
                memory(first, Role::Def, f);
                memory(second, Role::Def, f);
                address(addr, f);
            }
            Inst::StorePair { first, second, addr } => {
                memory(first, Role::Use, f);
                memory(second, Role::Use, f);
                address(addr, f);
            }
            Inst::Adr { rd, .. } => f(G(rd), Role::Def),
            Inst::Prefetch { base, .. } => f(G(base), Role::Use),
            Inst::FpUnary { rd, rn, .. } | Inst::FpToFp { rd, rn, .. } | Inst::VectorMov { rd, rn } => {
                f(Fpr(rd), Role::Def);
                f(Fpr(rn), Role::Use);
            }
            Inst::FpBinary { rd, rn, rm, .. } | Inst::FpSelect { rd, rn, rm, .. } => {
                f(Fpr(rd), Role::Def);
                f(Fpr(rn), Role::Use);
                f(Fpr(rm), Role::Use);
            }
            Inst::FpFused { rd, rn, rm, ra, .. } => {
                f(Fpr(rd), Role::Def);
                f(Fpr(rn), Role::Use);
                f(Fpr(rm), Role::Use);
                f(Fpr(ra), Role::Use);
            }
            Inst::FpCompare { rn, rm, .. } => {
                f(Fpr(rn), Role::Use);
                if let Some(rm) = rm {
                    f(Fpr(rm), Role::Use);
                }
            }
            Inst::FpMoveImm { rd, .. } => f(Fpr(rd), Role::Def),
            Inst::VectorFill { rd, fill } => {
                f(Fpr(rd), Role::Def);
                if let Fill::Gpr(source) = fill {
                    f(G(source), Role::Use);
                }
            }
            Inst::FpInt { op, gpr, fpr, .. } | Inst::FpFixed { op, gpr, fpr, .. } => {
                let to_fp = matches!(op, FpConvert::ToFp { .. } | FpConvert::Bits { to_fp: true });
                if to_fp {
                    f(Fpr(fpr), Role::Def);
                    f(G(gpr), Role::Use);
                } else {
                    f(G(gpr), Role::Def);
                    f(Fpr(fpr), Role::Use);
                }
            }
        }
    }

    /// The registers it writes and the registers it reads.
    pub fn defs_and_uses(&self) -> (Vec<Reg>, Vec<Reg>) {
        let (mut defs, mut uses) = (Vec::new(), Vec::new());
        self.clone().visit(&mut |register, role| {
            let key = register.key();
            if role.writes() {
                defs.push(key);
            }
            if role.reads() {
                uses.push(key);
            }
        });
        (defs, uses)
    }
}
