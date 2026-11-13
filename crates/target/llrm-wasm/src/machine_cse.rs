//! `MachineCSE`, the common-subexpression elimination before `MachineSink`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineCSE.cpp#L526)):
//! walking the dominator tree, an instruction equal to one in a dominating
//! block (same opcode and operands, commuted or not) is dropped and its
//! result replaced, where `isProfitableToCSE` says the longer live range pays.
//! A constant is shared only with the block it falls into or follows.
//!
//! DEVIATE 1: no `PerformSimplePRE` (needs block frequencies), and a load is
//! never a candidate where the pin takes the dereferenceable invariant ones
//! (the MIR holds no such memory operand). Exit: the shared machine CSE over
//! the neutral machine function (S4), no later than 2026-11-20.
//!
//! # See also
//! `MachineCSEImpl::{isCSECandidate, isProfitableToCSE, ProcessBlockCSE,
//! PerformTrivialCopyPropagation}`, pin d1106deb71cc.

use llrm_mir::module::BlockId;
use llrm_support::hash::HashMap;

use crate::cfg::Cfg;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::mir::{EH_LABEL, Function, IMPLICIT_DEF, Inst, Operand, PHI, VReg, is_copy};

/// `-csuses-threshold`.
const CS_USES_THRESHOLD: usize = 1024;

/// An expression: the opcode and every operand but the definitions, the two
/// register operands of a commutative instruction in order.
type Key = (u16, Vec<Operand>);

/// `isCSECandidate`.
fn is_candidate(inst: &Inst) -> bool {
    if matches!(inst.opcode, PHI | EH_LABEL | IMPLICIT_DEF) || is_copy(inst.opcode) {
        return false;
    }
    let Some(info) = desc(inst.opcode) else { return false };
    let barred = flag::MAY_STORE
        | flag::CALL
        | flag::TERMINATOR
        | flag::MAY_RAISE_FP_EXCEPTION
        | flag::UNMODELED_SIDE_EFFECTS
        | flag::MAY_LOAD
        | flag::META
        | flag::PSEUDO;
    info.flags & barred == 0 && info.implicit_uses.is_empty() && inst.defs().next().is_some()
}

fn key_of(
    inst: &Inst,
    resolve: &dyn Fn(VReg) -> VReg,
) -> Key {
    let mut operands: Vec<Operand> = inst
        .operands
        .iter()
        .filter(|o| !matches!(o, Operand::Def(_)))
        .map(|&o| if let Operand::Use(r) = o { Operand::Use(resolve(r)) } else { o })
        .collect();
    if desc(inst.opcode).is_some_and(|d| d.has(flag::COMMUTABLE))
        && let [Operand::Use(a), Operand::Use(b), ..] = operands[..]
        && a > b
    {
        operands.swap(0, 1);
    }
    (inst.opcode, operands)
}

struct Cse<'a> {
    f: &'a mut Function,
    /// A register replaced by the one it equals.
    replaced: Vec<VReg>,
    /// Where each register is read: the block and index of the instruction.
    uses: Vec<Vec<(usize, usize)>>,
    deleted: Vec<Vec<bool>>,
    /// Where each register is defined, by instruction.
    def_of: Vec<Option<(usize, usize)>>,
}

impl Cse<'_> {
    fn resolve(
        &self,
        mut r: VReg,
    ) -> VReg {
        while self.replaced[r as usize] != r {
            r = self.replaced[r as usize];
        }
        r
    }

    fn is_candidate_at(
        &self,
        (b, i): (usize, usize),
    ) -> bool {
        !self.deleted[b][i] && is_candidate(&self.f.blocks[b][i])
    }

    /// `PerformTrivialCopyPropagation`.
    fn propagate_copies(
        &mut self,
        b: usize,
        i: usize,
    ) -> bool {
        let mut changed = false;
        for at in 0..self.f.blocks[b][i].operands.len() {
            let Operand::Use(reg) = self.f.blocks[b][i].operands[at] else { continue };
            let reg = self.resolve(reg);
            let Some((db, di)) = self.def_of[reg as usize] else { continue };
            let def = &self.f.blocks[db][di];
            if self.deleted[db][di] || !is_copy(def.opcode) {
                continue;
            }
            let Some(Operand::Use(src)) = def.operands.get(1).copied() else { continue };
            if Some(src) == self.f.sp || self.def_of[src as usize].is_none() {
                continue;
            }
            let only_use = self.uses[reg as usize].len() == 1;
            self.f.blocks[b][i].operands[at] = Operand::Use(src);
            self.uses[reg as usize].retain(|&u| u != (b, i));
            self.uses[src as usize].push((b, i));
            if only_use {
                self.deleted[db][di] = true;
                self.uses[src as usize].retain(|&u| u != (db, di));
            }
            changed = true;
        }
        changed
    }

    /// `isProfitableToCSE`.
    fn is_profitable(
        &self,
        cs: VReg,
        reg: VReg,
        cs_block: usize,
        (b, i): (usize, usize),
    ) -> bool {
        let inst = &self.f.blocks[b][i];
        let cs_uses = &self.uses[cs as usize];
        let may_increase_pressure =
            cs_uses.len() > CS_USES_THRESHOLD || self.uses[reg as usize].iter().any(|u| !cs_uses.contains(u));
        if !may_increase_pressure {
            return true;
        }
        if desc(inst.opcode).is_some_and(|d| d.has(flag::CHEAP_AS_A_MOVE))
            && cs_block != b
            && !self.f.succs[cs_block].contains(&b)
        {
            return false;
        }
        let has_reg_use = inst.operands.iter().any(|o| matches!(o, Operand::Use(_)));
        if !has_reg_use && self.uses[reg as usize].iter().all(|&(ub, ui)| is_copy(self.f.blocks[ub][ui].opcode)) {
            return false;
        }
        let mut has_phi = false;
        for &(ub, ui) in cs_uses {
            has_phi |= self.f.blocks[ub][ui].opcode == PHI;
            if ub == b {
                return true;
            }
        }
        !has_phi
    }
}

/// Whether anything changed.
pub fn machine_cse(f: &mut Function) -> bool {
    let n = f.blocks.len();
    if f.personality || n == 0 {
        return false;
    }
    let cfg = Cfg::of(f);
    let (tree, _) = cfg.analyses();
    let mut children = vec![Vec::new(); n];
    for b in 1..n {
        if let Some(up) = tree.immediate_dominator(BlockId(b as u32)) {
            children[up.0 as usize].push(b);
        }
    }
    let regs = f.vreg_count();
    let mut cse = Cse {
        replaced: (0..regs as VReg).collect(),
        uses: vec![Vec::new(); regs],
        deleted: f.blocks.iter().map(|b| vec![false; b.len()]).collect(),
        def_of: vec![None; regs],
        f,
    };
    for b in 0..n {
        for i in 0..cse.f.blocks[b].len() {
            for &o in &cse.f.blocks[b][i].operands.clone() {
                match o {
                    Operand::Def(r) => cse.def_of[r as usize] = Some((b, i)),
                    Operand::Use(r) => cse.uses[r as usize].push((b, i)),
                    _ => {}
                }
            }
        }
    }
    let mut table: HashMap<Key, Vec<(VReg, usize)>> = HashMap::default();
    let mut changed = false;
    // The scope stack: the keys a block put in the table, popped on exit.
    let mut scopes: Vec<Vec<Key>> = Vec::new();
    let mut work: Vec<(usize, bool)> = vec![(0, false)];
    while let Some((b, exit)) = work.pop() {
        if exit {
            for key in scopes.pop().unwrap_or_default() {
                if let Some(entries) = table.get_mut(&key) {
                    entries.pop();
                }
            }
            continue;
        }
        work.push((b, true));
        work.extend(children[b].iter().rev().map(|&c| (c, false)));
        let mut mine: Vec<Key> = Vec::new();
        for i in 0..cse.f.blocks[b].len() {
            if !cse.is_candidate_at((b, i)) {
                continue;
            }
            let resolve = |r| cse.resolve(r);
            let mut key = key_of(&cse.f.blocks[b][i], &resolve);
            if !table.get(&key).is_some_and(|e| !e.is_empty()) && cse.propagate_copies(b, i) {
                changed = true;
                let resolve = |r| cse.resolve(r);
                key = key_of(&cse.f.blocks[b][i], &resolve);
            }
            let found = table.get(&key).and_then(|e| e.last()).copied();
            let def = cse.f.blocks[b][i].defs().next().expect("a candidate defines");
            if let Some((cs, cs_block)) = found
                && cse.f.blocks[b][i].defs().count() == 1
                && cse.is_profitable(cs, def, cs_block, (b, i))
            {
                cse.replaced[def as usize] = cs;
                let moved = std::mem::take(&mut cse.uses[def as usize]);
                cse.uses[cs as usize].extend(moved);
                cse.deleted[b][i] = true;
                // This instruction's own reads are gone from the use lists.
                for &o in &cse.f.blocks[b][i].operands.clone() {
                    if let Operand::Use(r) = o {
                        cse.uses[r as usize].retain(|&u| u != (b, i));
                    }
                }
                changed = true;
                continue;
            }
            table.entry(key.clone()).or_default().push((def, b));
            mine.push(key);
        }
        scopes.push(mine);
    }
    if changed {
        let replaced = std::mem::take(&mut cse.replaced);
        let resolve = |mut r: VReg| {
            while replaced[r as usize] != r {
                r = replaced[r as usize];
            }
            r
        };
        for (b, flags) in cse.deleted.iter().enumerate() {
            let mut at = 0;
            cse.f
                .blocks[b]
                .retain(
                    |_| {
                        at += 1;
                        !flags[at - 1]
                    },
                );
        }
        for operand in cse.f.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
            if let Operand::Use(r) = operand {
                *r = resolve(*r);
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::ValType;

    /// `a = n + m` computed twice in a block (fpbench's repeated address
    /// sums): the second is a copy of the first in the pin, an instruction
    /// and its locals less.
    #[test]
    fn a_value_computed_twice_is_computed_once() {
        let mut f = Function::default();
        let regs: Vec<VReg> = (0..6).map(|_| f.new_vreg(ValType::I32)).collect();
        let [n, m, a, b, t, _] = regs[..] else { unreachable!() };
        let (def, used) = (Operand::Def, Operand::Use);
        f.blocks = vec![vec![
            Inst::new(op::CONST_I32, &[def(n), Operand::Imm(1)]),
            Inst::new(op::CONST_I32, &[def(m), Operand::Imm(2)]),
            Inst::new(op::ADD_I32, &[def(a), used(n), used(m)]),
            Inst::new(op::ADD_I32, &[def(b), used(n), used(m)]),
            Inst::new(op::ADD_I32, &[def(t), used(a), used(b)]),
            Inst::new(op::RETURN, &[used(t)]),
        ]];
        f.succs = vec![vec![]];
        assert!(machine_cse(&mut f));
        let adds = f.blocks[0].iter().filter(|i| i.opcode == op::ADD_I32).count();
        assert_eq!(adds, 2);
    }
}
