//! A conditional branch whose compare reads a register the block just loaded
//! with a constant goes the one way the constant says.
//!
//! # How it works
//! Layout tail duplication copies a join block's `test r,r; jcc` into a
//! predecessor that ends in `mov r, imm`. The pin keeps the compare and the
//! branch. Here the block is scanned back from its compare to the last
//! instruction that writes `r`; if that is `mov r, imm` the flags the compare
//! sets are known, and the branch is a jump or nothing. The `mov` is never
//! removed: `r` is read on the other side. The compare goes when no later
//! instruction reads its flags.
//!
//! ```text
//!   mov eax, 255              mov eax, 255
//!   test eax, eax     =>      jmp L
//!   jg L
//! ```
//!
//! DEVIATE B1 (no pin counterpart; the pin leaves `test; jg` as it is):
//! O(instructions) per block, once per `Folder::rules` visit. It only removes
//! instructions, never adds one, so it has no size gate and no per-target
//! limit. `xor r, r` as a zero and a parity condition are not read.
//!
//! # See also
//! `branchfold`, whose rules call it.

use std::sync::Arc;

use super::{Exit, Folder, flags_dead_after};
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::{self, Insn, LirBlock};

/// The flags a `cmp` or `test` of two values of `bits` bits sets.
struct Flags {
    zero: bool,
    sign: bool,
    carry: bool,
    overflow: bool,
}

impl Flags {
    fn of(
        name: &str,
        left: u64,
        right: u64,
        bits: u32,
    ) -> Self {
        let mask = if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 };
        let top = 1u64 << (bits - 1);
        let (left, right) = (left & mask, right & mask);
        if name == "test" {
            let result = left & right;
            return Self { zero: result == 0, sign: result & top != 0, carry: false, overflow: false };
        }
        let result = left.wrapping_sub(right) & mask;
        Self {
            zero: result == 0,
            sign: result & top != 0,
            carry: left < right,
            overflow: (left ^ right) & (left ^ result) & top != 0,
        }
    }

    /// Whether the conditional jump `name` is taken; `None` for a condition
    /// not read (parity, the counter jumps).
    fn takes(
        &self,
        name: &str,
    ) -> Option<bool> {
        Some(match name {
            "je" | "jz" => self.zero,
            "jne" | "jnz" => !self.zero,
            "jb" | "jc" | "jnae" => self.carry,
            "jae" | "jnb" | "jnc" => !self.carry,
            "jbe" | "jna" => self.carry || self.zero,
            "ja" | "jnbe" => !self.carry && !self.zero,
            "js" => self.sign,
            "jns" => !self.sign,
            "jo" => self.overflow,
            "jno" => !self.overflow,
            "jl" | "jnge" => self.sign != self.overflow,
            "jge" | "jnl" => self.sign == self.overflow,
            "jle" | "jng" => self.zero || self.sign != self.overflow,
            "jg" | "jnle" => !self.zero && self.sign == self.overflow,
            _ => return None,
        })
    }
}

/// Whether `cond` is taken, and where its compare is in the block, when the
/// compare reads a register the block last set to a constant.
fn decided(
    block: &LirBlock,
    cond: &Arc<Insn>,
    regs: llrm_lir::registers::Regs,
    bits: u32,
) -> Option<(usize, bool)> {
    let cond_at = block.insns.iter().position(|one| Arc::ptr_eq(one, cond))?;
    let compare_at = block.insns[..cond_at].iter().rposition(|one| !one.is_meta())?;
    let compare = &block.insns[compare_at];
    let what = compare.what.as_ref().filter(|what| what.op == Operation::Compare && what.dests.is_empty())?;
    let name = what.name.as_deref().filter(|name| matches!(*name, "cmp" | "test"))?;
    let [Loc::Reg(register), other] = what.sources.as_slice() else { return None };
    let right = match other {
        Loc::Reg(again) if name == "test" && again == register => None,
        Loc::Imm(imm) if imm.address.is_none() => Some(imm.value as u64),
        _ => return None,
    };
    let zero = Loc::Imm(crate::model::ir::Imm { value: 0, width: register.width, address: None });
    let probe = Semantics {
        name: Some("mov".into()),
        dests: vec![Loc::Reg(*register)],
        sources: vec![zero],
        ..Semantics::new(Operation::Move)
    };
    let (_, lanes) = crate::backend::peephole::_register_effects_of_what(regs, bits, &probe, true, false)?;
    for one in block.insns[..compare_at].iter().rev().filter(|one| !one.is_meta()) {
        let (_, written) = crate::backend::peephole::_register_effects(regs, bits, one, true, false)?;
        if written.is_disjoint(&lanes) {
            continue;
        }
        let load = one.what.as_ref()?;
        let [Loc::Reg(into)] = load.dests.as_slice() else { return None };
        let [Loc::Imm(value)] = load.sources.as_slice() else { return None };
        if load.op != Operation::Move
            || into != register
            || value.address.is_some()
            || one.call.is_some()
            || one.group.is_some()
            || one.symbol.is_some()
            || !one.spread.is_empty()
            || !one.clobbers.is_empty()
        {
            return None;
        }
        let known = value.value as u64;
        let flags = Flags::of(name, known, right.unwrap_or(known), register.width * 8);
        return Some((compare_at, flags.takes(cond.what.as_ref()?.name.as_deref()?)?));
    }
    None
}

impl Folder<'_> {
    /// The conditional branch of `block` made a jump or dropped when its
    /// compare is decided. Whether it was.
    pub(super) fn constant_branch(
        &mut self,
        index: usize,
        exit: &Exit,
    ) -> bool {
        let (Some((regs, bits)), Some(cond), Some(taken)) = (self.regs, &exit.cond, exit.taken) else { return false };
        let block = self.blocks[index].clone();
        let Some(other) = exit.other.or_else(|| block.succ.iter().copied().find(|to| *to != taken)) else {
            return false;
        };
        let Some((compare_at, goes)) = decided(&block, cond, regs, bits) else { return false };
        let cond_at = block.insns.iter().position(|one| Arc::ptr_eq(one, cond)).expect("the branch is in its block");
        let (kept, dropped) = if goes { (taken, other) } else { (other, taken) };
        let spent = flags_dead_after(regs, bits, &block, cond_at);
        // The jump that ends the block stays last: nothing may follow it.
        let ends = exit.jump.clone().unwrap_or_else(|| Arc::clone(cond));
        let insns = block
            .insns
            .iter()
            .enumerate()
            .map(|(at, one)| {
                if goes && Arc::ptr_eq(one, &ends) {
                    Arc::new(Insn {
                        what: Some(Semantics {
                            name: Some("jmp".into()),
                            target: Some(taken),
                            ..Semantics::new(Operation::Jump)
                        }),
                        uses: Vec::new(),
                        ..(**one).clone()
                    })
                } else if at == cond_at || (at == compare_at && spent) {
                    lir::anchor(Arc::clone(one))
                } else {
                    Arc::clone(one)
                }
            })
            .collect();
        self.odds.rerouted(block.at, &block.succ, dropped, &[(kept, 1.0)]);
        if let Some(entering) = self.preds.get_mut(&dropped) {
            entering.remove(&block.at);
        }
        self.set(index, LirBlock { succ: std::iter::once(kept).collect(), ..block.with_insns(insns).into_inner() });
        // The flow the dropped edge carried is gone from everything it led to.
        let mut work = vec![dropped];
        while let Some(at) = work.pop() {
            if self.stale.insert(at)
                && let Some(&one) = self.pos.get(&at)
            {
                work.extend(self.blocks[one].succ.iter().copied());
            }
        }
        true
    }
}
