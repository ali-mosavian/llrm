//! `MachineLICM`, the early pass before `MachineCSE`
//! ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/MachineLICM.cpp#L827)):
//! an instruction in a loop that reads no register and is trivially
//! rematerializable (a constant) moves to the preheader of the outermost loop
//! holding it, which an edge split makes where the loop has none (inner loops
//! get none: the pin starts from `MLI->begin()`); where an identical one is
//! already there, the loop's is dropped for it. A cheap one that feeds a PHI of
//! the loop stays (`HasLoopPHIUse`).
//!
//! DEVIATE 1: register pressure is not modelled. The pin's wasm pressure
//! limits are the two reserved registers, so `CanCauseHighRegPressure` holds
//! for every defining instruction and only a rematerializable one is hoisted
//! (pin `llc -print-after=early-machinelicm` on nbody moves constants only).
//! No `ExtractHoistableLoad`, no block-hotness check. Exit: the shared
//! machine LICM over the neutral machine function (S4), no later than
//! 2026-11-20.
//!
//! # See also
//! `MachineLICMImpl::{HoistOutOfLoop, Hoist, IsProfitableToHoist,
//! HasLoopPHIUse, EliminateCSE, getOrCreatePreheader}`, pin d1106deb71cc.

use llrm_mir::module::BlockId;
use llrm_support::hash::HashMap;

use crate::cfg::Cfg;
use crate::dagisel::generated::instrinfo::{desc, flag};
use crate::machine_sink::is_safe_to_move;
use crate::mir::{Function, Inst, Operand, PHI, VReg, is_copy};

/// The expression an instruction computes: its opcode and the operands that
/// are no definition.
fn same_value(
    a: &Inst,
    b: &Inst,
) -> bool {
    let uses = |i: &Inst| i.operands.iter().filter(|o| !matches!(o, Operand::Def(_))).copied().collect::<Vec<_>>();
    a.opcode == b.opcode && uses(a) == uses(b)
}

/// `isTriviallyReMaterializable` for what is hoisted: a constant, with no
/// register to read.
fn is_hoistable(inst: &Inst) -> bool {
    let Some(info) = desc(inst.opcode) else { return false };
    info.has(flag::REMATERIALIZABLE)
        && !info.has(flag::CONVERGENT)
        && !inst.operands.iter().any(|o| matches!(o, Operand::Use(_)))
        && inst.defs().next().is_some()
        && is_safe_to_move(inst, &mut true)
}

/// Whether anything changed.
pub fn machine_licm(f: &mut Function) -> bool {
    if f.personality || f.blocks.is_empty() {
        return false;
    }
    let mut changed = false;
    // `getOrCreatePreheader`: an edge split where the loop has none.
    loop {
        let cfg = Cfg::of(f);
        let (tree, loops) = cfg.analyses();
        if cfg.irreducible_edge(&tree).is_some() {
            return changed;
        }
        let missing = loops
            .top_level_loops()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .find_map(
                |l| {
                    let header = l.get_header();
                    if f.is_eh_pad(header.0 as usize) || l.get_loop_preheader(&cfg).is_some() {
                        return None;
                    }
                    l.get_loop_predecessor(&cfg).map(|pred| (pred.0 as usize, header.0 as usize))
                },
            );
        let Some((pred, header)) = missing else { break };
        crate::phi_elimination::split_edge(f, pred, header);
        changed = true;
    }
    let cfg = Cfg::of(f);
    let (tree, loops) = cfg.analyses();
    let n = f.blocks.len();
    let mut children = vec![Vec::new(); n];
    for b in 1..n {
        if let Some(up) = tree.immediate_dominator(BlockId(b as u32)) {
            children[up.0 as usize].push(b);
        }
    }
    let mut replaced: HashMap<VReg, VReg> = HashMap::default();
    for l in loops.top_level_loops().collect::<Vec<_>>().into_iter().rev() {
        let header = l.get_header().0 as usize;
        let Some(preheader) = l.get_loop_preheader(&cfg).map(|b| b.0 as usize) else { continue };
        if f.is_eh_pad(header) {
            continue;
        }
        let in_loop = |b: usize| l.contains(BlockId(b as u32));
        // The blocks the header dominates within the loop, in the pin's order.
        let mut scopes = Vec::new();
        let mut work = vec![header];
        while let Some(b) = work.pop() {
            if !in_loop(b) {
                continue;
            }
            scopes.push(b);
            if f.succs[b].len() >= 25 {
                continue;
            }
            work.extend(children[b].iter().rev());
        }
        let exits: Vec<usize> =
            scopes.iter().flat_map(|&b| f.succs[b].iter().copied()).filter(|&s| !in_loop(s)).collect();
        // Where each register is read, for `HasLoopPHIUse`.
        let mut users: HashMap<VReg, Vec<(usize, usize)>> = HashMap::default();
        for (b, insts) in f.blocks.iter().enumerate() {
            for (i, inst) in insts.iter().enumerate() {
                for o in &inst.operands {
                    if let Operand::Use(r) = o {
                        users.entry(*r).or_default().push((b, i));
                    }
                }
            }
        }
        let has_loop_phi_use = |f: &Function, b: usize, i: usize| {
            let mut work = vec![(b, i)];
            while let Some((b, i)) = work.pop() {
                for d in f.blocks[b][i].defs() {
                    for &(ub, ui) in users.get(&d).into_iter().flatten() {
                        let user = &f.blocks[ub][ui];
                        if user.opcode == PHI {
                            if in_loop(ub) || exits.contains(&ub) {
                                return true;
                            }
                        } else if is_copy(user.opcode) && in_loop(ub) {
                            work.push((ub, ui));
                        }
                    }
                }
            }
            false
        };
        // The instructions the preheader has, and those hoisted into it.
        let mut known: Option<Vec<Inst>> = None;
        let mut moves: Vec<(usize, usize)> = Vec::new();
        let mut dropped: Vec<(usize, usize)> = Vec::new();
        for &b in &scopes {
            for i in 0..f.blocks[b].len() {
                let inst = &f.blocks[b][i];
                if !is_hoistable(inst) {
                    continue;
                }
                let cheap = desc(inst.opcode).is_some_and(|d| d.has(flag::CHEAP_AS_A_MOVE));
                if cheap && has_loop_phi_use(f, b, i) {
                    continue;
                }
                let seen = known.get_or_insert_with(|| f.blocks[preheader].clone());
                if let Some(dup) = seen.iter().find(|p| same_value(inst, p)) {
                    for (old, new) in inst.defs().zip(dup.defs()) {
                        replaced.insert(old, new);
                    }
                    dropped.push((b, i));
                } else {
                    seen.push(inst.clone());
                    moves.push((b, i));
                }
            }
        }
        if moves.is_empty() && dropped.is_empty() {
            continue;
        }
        let hoisted: Vec<Inst> = moves.iter().map(|&(b, i)| f.blocks[b][i].clone()).collect();
        let mut gone: Vec<(usize, usize)> = moves.iter().chain(dropped.iter()).copied().collect();
        gone.sort_unstable_by(|a, b| b.cmp(a));
        for (b, i) in gone {
            f.blocks[b].remove(i);
        }
        let at = f.blocks[preheader].iter().position(|i| i.is_terminator()).unwrap_or(f.blocks[preheader].len());
        for (k, inst) in hoisted.into_iter().enumerate() {
            f.blocks[preheader].insert(at + k, inst);
        }
        changed = true;
        // Later loops read the moved instructions at their new places.
        if !replaced.is_empty() {
            let resolve = |mut r: VReg| {
                while let Some(&next) = replaced.get(&r) {
                    r = next;
                }
                r
            };
            for operand in f.blocks.iter_mut().flatten().flat_map(|inst| inst.operands.iter_mut()) {
                if let Operand::Use(r) = operand {
                    *r = resolve(*r);
                }
            }
            replaced.clear();
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dagisel::generated::ids::opcode as op;
    use crate::mir::ValType;

    fn konst(
        dst: VReg,
        value: i64,
    ) -> Inst {
        Inst::new(op::CONST_I32, &[Operand::Def(dst), Operand::Imm(value)])
    }

    /// bb1 loops on itself: `k = 7; s = k + n; br_if bb1 s; br bb2`.
    /// `entry` is bb0's code, which ends in a jump to bb1 or in a branch to
    /// bb1 or bb2.
    fn counted_loop(
        branching_entry: bool,
        entry_const: bool,
    ) -> (Function, VReg, VReg, VReg) {
        let mut f = Function::default();
        let (n, k, s) = (f.new_vreg(ValType::I32), f.new_vreg(ValType::I32), f.new_vreg(ValType::I32));
        let mut head = vec![konst(n, 3)];
        if entry_const {
            head.push(konst(f.new_vreg(ValType::I32), 7));
        }
        if branching_entry {
            head.push(Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(n)]));
            head.push(Inst::new(op::BR, &[Operand::Block(2)]));
        } else {
            head.push(Inst::new(op::BR, &[Operand::Block(1)]));
        }
        f.blocks = vec![
            head,
            vec![
                konst(k, 7),
                Inst::new(op::ADD_I32, &[Operand::Def(s), Operand::Use(k), Operand::Use(n)]),
                Inst::new(op::BR_IF, &[Operand::Block(1), Operand::Use(s)]),
                Inst::new(op::BR, &[Operand::Block(2)]),
            ],
            vec![Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![if branching_entry { vec![1, 2] } else { vec![1] }, vec![1, 2], vec![]];
        (f, n, k, s)
    }

    fn consts(block: &[Inst]) -> usize {
        block.iter().filter(|i| i.opcode == op::CONST_I32).count()
    }

    /// A constant the loop recomputes every trip was left in the loop: the
    /// pin runs it once, in the preheader.
    #[test]
    fn a_constant_in_a_loop_moves_to_the_preheader() {
        let (mut f, ..) = counted_loop(false, false);
        assert!(machine_licm(&mut f));
        assert_eq!(consts(&f.blocks[1]), 0);
        assert_eq!(consts(&f.blocks[0]), 2);
    }

    /// A loop entered from a branch has no preheader: the pin splits the
    /// edge, and the constant goes to the new block, not into the branching
    /// block (which the other path would also run).
    #[test]
    fn a_loop_entered_from_a_branch_gets_a_preheader_of_its_own() {
        let (mut f, ..) = counted_loop(true, false);
        assert!(machine_licm(&mut f));
        assert_eq!(f.blocks.len(), 4);
        assert_eq!(consts(&f.blocks[0]), 1);
        assert_eq!(consts(&f.blocks[1]), 1);
        assert_eq!(consts(&f.blocks[2]), 0);
    }

    /// An identical constant already in the preheader is reused: the loop's
    /// copy is dropped and its uses read the preheader's register.
    #[test]
    fn a_constant_the_preheader_already_has_is_not_computed_twice() {
        let (mut f, ..) = counted_loop(false, true);
        assert!(machine_licm(&mut f));
        assert_eq!(consts(&f.blocks[0]), 2);
        assert_eq!(consts(&f.blocks[1]), 0);
    }

    /// bb0 -> bb1 (outer header) -> bb2 (inner loop on itself) -> bb3 -> bb1.
    /// The inner loop's only predecessor branches twice, so it has no
    /// preheader.
    fn nested_loops() -> Function {
        let mut f = Function::default();
        let (n, k, s) = (f.new_vreg(ValType::I32), f.new_vreg(ValType::I32), f.new_vreg(ValType::I32));
        f.blocks = vec![
            vec![konst(n, 3), Inst::new(op::BR, &[Operand::Block(1)])],
            vec![Inst::new(op::BR_IF, &[Operand::Block(2), Operand::Use(n)]), Inst::new(op::BR, &[Operand::Block(4)])],
            vec![
                konst(k, 7),
                Inst::new(op::ADD_I32, &[Operand::Def(s), Operand::Use(k), Operand::Use(n)]),
                Inst::new(op::BR_IF, &[Operand::Block(2), Operand::Use(s)]),
                Inst::new(op::BR, &[Operand::Block(3)]),
            ],
            vec![Inst::new(op::BR, &[Operand::Block(1)])],
            vec![Inst::new(op::RETURN, &[])],
        ];
        f.succs = vec![vec![1], vec![2, 4], vec![2, 3], vec![1], vec![]];
        f
    }

    /// An inner loop got a block of its own for a preheader (queens -O1 had
    /// 17 blocks to the pin's 16): the pin runs the pass on the outermost
    /// loops only, and a constant of the inner loop goes to the outer
    /// preheader.
    #[test]
    fn only_the_outermost_loop_gets_a_preheader() {
        let mut f = nested_loops();
        assert!(machine_licm(&mut f));
        assert_eq!(f.blocks.len(), 5);
        assert_eq!(consts(&f.blocks[0]), 2);
        assert_eq!(consts(&f.blocks[2]), 0);
    }
}
