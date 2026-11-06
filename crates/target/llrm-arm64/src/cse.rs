//! `MachineCSE` for arm64: the same pure computation made twice, made once.
//!
//! # How it works
//! The dominator tree is walked with a scoped table from an instruction's
//! expression (operation and operands, its result named by nothing) to the
//! value it defines. An instruction that [`candidate`] admits and finds its
//! expression in the table is removed, and every reader of its value reads the
//! table's, when [`profitable`] agrees (`ProcessBlockCSE`, `isCSECandidate`,
//! `isProfitableToCSE` of
//! [MachineCSE.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCSE.cpp)).
//!
//! ```text
//!   a:  v1 = adrp _g              a:  v1 = adrp _g
//!   b:  v2 = adrp _g        =>    b:  (reads v1)
//! ```
//!
//! # Runs
//! Before allocation, after [`crate::licm`] (the pin: `EarlyMachineLICM`, then
//! `MachineCSE`).
//!
//! DEVIATE MCSE1: no `PerformTrivialCopyPropagation` and no `PerformSimplePRE`
//! (the pin hoists a computation both arms of a branch make); the table key is
//! the operands' text. Owner arm64, exit 2026-11-27.

use std::collections::{HashMap, HashSet};

use llrm_codegen::analysis::loops;
use llrm_codegen::backend::licm_core::Block as Shape;
use llrm_spike_mf::mfunc::{Inst, Kind, MFunction, Operand, Reg};

/// `isCSECandidate`: a pure operation on virtual registers with one result.
fn candidate(inst: &Inst) -> bool {
    let defs = inst.operands.iter().filter(|operand| matches!(operand, Operand::Def(_))).count();
    defs == 1
        && inst.kind != Kind::Copy
        && inst.operands.iter().all(|operand| !matches!(operand, Operand::Def(Reg::Phys(_)) | Operand::Use(Reg::Phys(_))))
        && inst.ties.is_empty()
        && !(inst.call
            || inst.terminator
            || inst.barrier
            || (inst.may_load && !inst.invariant_load)
            || inst.may_store
            || inst.side_effects
            || inst.sets_flags
            || inst.reads_flags
            || inst.early_clobber)
}

/// What the table is keyed by: the instruction with its result blank and a
/// commutative pair of sources in order.
fn expression(inst: &Inst) -> String {
    let mut operands: Vec<Operand> = inst
        .operands
        .iter()
        .map(|operand| match operand {
            Operand::Def(_) => Operand::Imm(0),
            other => other.clone(),
        })
        .collect();
    if inst.commutable {
        let uses: Vec<usize> =
            (0..operands.len()).filter(|&at| matches!(operands[at], Operand::Use(Reg::Virt(_)))).collect();
        if let [first, second] = uses[..] {
            let number = |at: usize| match operands[at] {
                Operand::Use(Reg::Virt(n)) => n,
                _ => 0,
            };
            if number(first) > number(second) {
                operands.swap(first, second);
            }
        }
    }
    format!("{:?}{:?}", inst.kind, operands)
}

/// Where a value is read: block, instruction (`usize::MAX` for a phi), and
/// whether the reader is a copy.
type Site = (usize, usize, bool);

/// `isProfitableToCSE`.
fn profitable(
    inst: &Inst,
    block: usize,
    cs_block: usize,
    cs_uses: &[Site],
    uses: &[Site],
    successors: &[Vec<i64>],
    ids: &[u32],
) -> bool {
    // Used at every use of the new value: no longer live range.
    if uses.iter().all(|site| cs_uses.contains(site)) {
        return true;
    }
    // Heuristic 1: a cheap computation is shared with its own block or one
    // that falls or jumps to it.
    if inst.cheap_as_move && cs_block != block && !successors[cs_block].contains(&(ids[block] as i64)) {
        return false;
    }
    // Heuristic 2: no register read, and only copies read the result.
    let reads_register = inst.operands.iter().any(|operand| matches!(operand, Operand::Use(_)));
    if !reads_register && uses.iter().all(|&(_, _, copy)| copy) {
        return false;
    }
    // Heuristic 3: a value a phi reads is reused only where it is read already.
    let phi = cs_uses.iter().any(|&(_, at, _)| at == usize::MAX);
    cs_uses.iter().any(|&(user, _, _)| user == block) || !phi
}

/// Makes each computation the dominating one made once; whether anything went.
pub fn share<C>(function: &mut MFunction<C>) -> bool {
    let count = function.blocks.len();
    if count == 0 {
        return false;
    }
    let successors: Vec<Vec<i64>> = (0..count).map(|at| crate::licm::successors(function, at)).collect();
    let ids: Vec<u32> = function.blocks.iter().map(|block| block.id).collect();
    let slot: HashMap<i64, usize> = ids.iter().enumerate().map(|(at, &id)| (id as i64, at)).collect();
    let shapes: Vec<Shape<()>> = (0..count)
        .map(|at| Shape { at: ids[at] as i64, succ: successors[at].clone(), phis: Vec::new(), insns: Vec::new() })
        .collect();
    let tree = loops::dominance(&shapes, Some(ids[0] as i64));

    // One definition each, and every reader.
    let mut defs: HashMap<u32, u32> = HashMap::new();
    let mut uses: HashMap<u32, Vec<Site>> = HashMap::new();
    for (b, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            if let Reg::Virt(n) = phi.result {
                *defs.entry(n).or_default() += 1;
            }
            for (_, reg) in &phi.incoming {
                if let Reg::Virt(n) = reg {
                    uses.entry(*n).or_default().push((b, usize::MAX, false));
                }
            }
        }
        for (i, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                match operand {
                    Operand::Def(Reg::Virt(n)) => *defs.entry(*n).or_default() += 1,
                    Operand::Use(Reg::Virt(n)) => uses.entry(*n).or_default().push((b, i, inst.kind == Kind::Copy)),
                    _ => {}
                }
            }
        }
    }

    // Children in the dominator tree, in layout order.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); count];
    for at in 1..count {
        if let Some(parent) = tree.immediate(ids[at] as i64).and_then(|id| slot.get(&id).copied()) {
            children[parent].push(at);
        }
    }

    let mut rename: HashMap<u32, u32> = HashMap::new();
    let canon = |rename: &HashMap<u32, u32>, mut n: u32| {
        while let Some(&to) = rename.get(&n) {
            n = to;
        }
        n
    };
    let mut table: HashMap<String, (u32, usize)> = HashMap::new();
    let mut removed: HashSet<(usize, usize)> = HashSet::new();
    // Entering a block adds keys; leaving it takes them out again.
    let mut stack: Vec<(usize, bool, Vec<String>)> = vec![(0, false, Vec::new())];
    while let Some((b, leaving, added)) = stack.pop() {
        if leaving {
            for key in added {
                table.remove(&key);
            }
            continue;
        }
        let mut mine = Vec::new();
        for i in 0..function.blocks[b].insts.len() {
            // Readers see the shared values.
            let mut inst = function.blocks[b].insts[i].clone();
            for operand in inst.operands.iter_mut() {
                if let Operand::Use(Reg::Virt(n)) = operand {
                    *n = canon(&rename, *n);
                }
            }
            if !candidate(&inst) {
                continue;
            }
            let Some(def) = inst.operands.iter().find_map(|operand| match operand {
                Operand::Def(Reg::Virt(n)) if defs.get(n) == Some(&1) => Some(*n),
                _ => None,
            }) else {
                continue;
            };
            let key = expression(&inst);
            match table.get(&key) {
                Some(&(shared, cs_block))
                    if profitable(
                        &inst,
                        b,
                        cs_block,
                        uses.get(&shared).map_or(&[][..], |sites| &sites[..]),
                        uses.get(&def).map_or(&[][..], |sites| &sites[..]),
                        &successors,
                        &ids,
                    ) =>
                {
                    rename.insert(def, shared);
                    let moved = uses.remove(&def).unwrap_or_default();
                    uses.entry(shared).or_default().extend(moved);
                    removed.insert((b, i));
                }
                Some(_) => {}
                None => {
                    table.insert(key.clone(), (def, b));
                    mine.push(key);
                }
            }
        }
        stack.push((b, true, mine));
        for &child in children[b].iter().rev() {
            stack.push((child, false, Vec::new()));
        }
    }
    if removed.is_empty() {
        return false;
    }
    for (b, block) in function.blocks.iter_mut().enumerate() {
        let mut at = 0;
        block.insts.retain(|_| {
            at += 1;
            !removed.contains(&(b, at - 1))
        });
        for inst in block.insts.iter_mut() {
            for operand in inst.operands.iter_mut() {
                if let Operand::Use(Reg::Virt(n)) = operand {
                    *n = canon(&rename, *n);
                }
            }
        }
        for phi in block.phis.iter_mut() {
            for (_, reg) in phi.incoming.iter_mut() {
                if let Reg::Virt(n) = reg {
                    *n = canon(&rename, *n);
                }
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use llrm_spike_mf::mfunc::Block;

    fn page(
        dst: u32,
        name: &str,
    ) -> Inst {
        let mut inst = Inst::new(
            Kind::Op("Adr"),
            vec![Operand::Def(Reg::Virt(dst)), Operand::Symbol(name.into(), 0), Operand::Imm(7)],
        );
        inst.rematerializable = true;
        inst
    }

    fn store(reg: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("Store"), vec![Operand::Use(Reg::Virt(reg))]);
        inst.may_store = true;
        inst
    }

    fn jump(to: u32) -> Inst {
        let mut inst = Inst::new(Kind::Op("Branch"), vec![Operand::Block(to)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }

    fn block(
        id: u32,
        insts: Vec<Inst>,
    ) -> Block {
        Block { id, insts, phis: Vec::new() }
    }

    fn pages(function: &MFunction<()>) -> usize {
        function.blocks.iter().flat_map(|block| &block.insts).filter(|inst| inst.kind == Kind::Op("Adr")).count()
    }

    /// A function made the page of the same global in a block and again in a
    /// block it dominates (2 `adrp`, where the pin's `MachineCSE` leaves 1
    /// and the second store reads the first page).
    #[test]
    fn a_page_made_again_in_a_dominated_block_is_made_once() {
        let mut function = MFunction::<()>::new("f");
        function.blocks = vec![
            block(0, vec![page(1, "_g"), store(1), jump(1)]),
            block(1, vec![page(2, "_g"), store(2)]),
        ];
        assert!(share(&mut function));
        assert_eq!(pages(&function), 1);
        assert_eq!(function.blocks[1].insts[0].operands, vec![Operand::Use(Reg::Virt(1))]);
    }

    /// Two arms neither of which dominates the other keep their own page.
    #[test]
    fn a_page_made_in_two_sibling_arms_is_made_in_each() {
        let mut cond = Inst::new(Kind::Op("BranchCond"), vec![Operand::Block(2)]);
        cond.terminator = true;
        let mut function = MFunction::<()>::new("f");
        function.blocks = vec![
            block(0, vec![cond]),
            block(1, vec![page(1, "_g"), store(1), jump(3)]),
            block(2, vec![page(2, "_g"), store(2), jump(3)]),
            block(3, vec![]),
        ];
        assert!(!share(&mut function));
        assert_eq!(pages(&function), 2);
    }

    /// A different symbol is another value.
    #[test]
    fn pages_of_two_globals_stay_two() {
        let mut function = MFunction::<()>::new("f");
        function.blocks = vec![block(0, vec![page(1, "_g"), page(2, "_h"), store(1), store(2)])];
        assert!(!share(&mut function));
    }
}
