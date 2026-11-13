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

/// The registers of one operation: held in place up to `INLINE`, more than
/// that on the heap. Almost every operation names fewer, so listing them
/// allocates nothing.
#[derive(Clone, Debug)]
pub struct RegList {
    length: usize,
    inline: [Reg; Self::INLINE],
    spilled: Vec<Reg>,
}

impl RegList {
    const INLINE: usize = 6;

    pub fn new() -> Self {
        Self { length: 0, inline: [Reg::Gpr(Gpr(0)); Self::INLINE], spilled: Vec::new() }
    }

    pub fn push(
        &mut self,
        register: Reg,
    ) {
        if self.spilled.is_empty() && self.length < Self::INLINE {
            self.inline[self.length] = register;
            self.length += 1;
            return;
        }
        if self.spilled.is_empty() {
            self.spilled.extend_from_slice(&self.inline);
        }
        self.spilled.push(register);
    }

    pub fn as_slice(&self) -> &[Reg] {
        if self.spilled.is_empty() { &self.inline[..self.length] } else { &self.spilled }
    }
}

impl Default for RegList {
    fn default() -> Self {
        Self::new()
    }
}

impl std::ops::Deref for RegList {
    type Target = [Reg];

    fn deref(&self) -> &[Reg] {
        self.as_slice()
    }
}

/// A [`RegList`] taken by value, one register at a time.
pub struct RegListIter {
    list: RegList,
    at: usize,
}

impl Iterator for RegListIter {
    type Item = Reg;

    fn next(&mut self) -> Option<Reg> {
        let next = self.list.as_slice().get(self.at).copied();
        self.at += 1;
        next
    }
}

impl IntoIterator for RegList {
    type IntoIter = RegListIter;
    type Item = Reg;

    fn into_iter(self) -> RegListIter {
        RegListIter { list: self, at: 0 }
    }
}

impl<'a> IntoIterator for &'a RegList {
    type IntoIter = std::slice::Iter<'a, Reg>;
    type Item = &'a Reg;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
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
            Inst::AddCarry { rd, rn, rm, .. } => {
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
            Inst::Cfi(_) | Inst::Mark(_) | Inst::Label(_) => {}
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
            Inst::FpUnary { rd, rn, .. }
            | Inst::FpToFp { rd, rn, .. }
            | Inst::FpLaneToFp { rd, rn, .. }
            | Inst::VectorMov { rd, rn } => {
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
            Inst::VectorBinary { rd, rn, rm, .. } => {
                f(Fpr(rd), Role::Def);
                f(Fpr(rn), Role::Use);
                f(Fpr(rm), Role::Use);
            }
            Inst::VectorFma { rd, rn, rm, .. } => {
                f(Fpr(rd), Role::UseDef);
                f(Fpr(rn), Role::Use);
                f(Fpr(rm), Role::Use);
            }
            Inst::VectorDupLane { rd, rn, .. } => {
                f(Fpr(rd), Role::Def);
                f(Fpr(rn), Role::Use);
            }
            Inst::VectorFpBinary { rd, rn, rm, .. } => {
                f(Fpr(rd), Role::Def);
                f(Fpr(rn), Role::Use);
                f(Fpr(rm), Role::Use);
            }
            Inst::VectorShift { rd, rn, .. } | Inst::VectorNot { rd, rn } | Inst::VectorFpUnary { rd, rn, .. } => {
                f(Fpr(rd), Role::Def);
                f(Fpr(rn), Role::Use);
            }
            Inst::VectorFill { rd, fill } => {
                f(Fpr(rd), Role::Def);
                if let Fill::Dup(_, source) = fill {
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
    pub fn defs_and_uses(&self) -> (RegList, RegList) {
        let (mut defs, mut uses) = (RegList::new(), RegList::new());
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
