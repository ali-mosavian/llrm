//! Removes an immediate load to a register that an identical, earlier load in
//! every path to it already left there: LLVM's `MachineLateInstrsCleanup`
//! ([MachineLateInstrsCleanup.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineLateInstrsCleanup.cpp)).
//!
//! # How it works
//! Blocks are taken in reverse post-order. Each block starts with the loads
//! every predecessor ends with identically (`RegDefs` of the first predecessor
//! that all the others hold the same entry of; a predecessor not yet walked, a
//! back edge, holds none). An instruction that writes a register's lanes drops
//! the entries on those lanes, and one with an unknown effect drops them all. A
//! candidate is a pure `mov reg, imm` writing no flag: one identical to the held
//! entry of its register is replaced by an anchor, else it becomes the entry.
//!
//! ```text
//!   b0: mov ax, 5            b0: mov ax, 5
//!       jcc b2                   jcc b2
//!   b1: mov ax, 5   ==>      b1: (anchor)
//!       ...                      ...
//! ```
//!
//! # Example
//! The unit tests `a_load_the_register_already_holds_goes` and
//! `a_load_after_another_path_changed_the_register_stays`.
//!
//! # Runs
//! After `prologue`, before `peephole`: where rematerialization and the
//! allocator's splitting have left the same constant loaded again.
//!
//! # Limits
//! DEVIATE LC1: the pin also takes a load-address instruction using the frame
//! register (frame-index anchor points made by frame elimination); here the
//! frame is addressed by `masm`, so only immediates are candidates. There are
//! no kill flags in LIR, so `clearKillsForDef` has nothing to clear.

use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::lanes::Lanes;
use crate::backend::liveness;
use crate::backend::machinedce;
use crate::backend::peephole::id;
use crate::model::ir::{Loc, Operation, Semantics};
use crate::model::lir::{self, Insn, LirBody};
use crate::model::passes::{Exception, LIRTransform};
use crate::support::hash::{HashMap, HashSet};

/// One held load: the instruction and the lanes it writes.
#[derive(Clone)]
struct Held {
    insn: Arc<Insn>,
    lanes: Lanes,
}

/// The phase.
pub struct LateCleanup;

impl LIRTransform for LateCleanup {
    fn class_name(&self) -> &'static str {
        "LateCleanup"
    }

    fn name(&self) -> &str {
        "latecleanup"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(cleaned(body))
    }

    fn transform_raising(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, Exception> {
        Ok(cleaned(body))
    }
}

/// `isCandidate`: the register `one` loads an immediate into, where it is a
/// pure move of one and writes no flag.
fn candidate(
    regs: Regs,
    one: &Insn,
) -> Option<Loc> {
    let what: &Semantics = one.what.as_ref()?;
    let ([dest], [Loc::Imm(imm)]) = (&what.dests[..], &what.sources[..]) else { return None };
    if what.op != Operation::Move || imm.address.is_some() || !matches!(dest, Loc::Reg(_)) {
        return None;
    }
    machinedce::pure(regs, one).then(|| dest.clone())
}

/// `MachineInstr::isIdenticalTo`.
fn identical(
    held: &Held,
    one: &Insn,
) -> bool {
    match (&held.insn.what, &one.what) {
        (Some(left), Some(right)) => left.same_meaning(right),
        _ => false,
    }
}

/// `MachineLateInstrsCleanup::run`.
pub fn cleaned(body: LirBody) -> LirBody {
    let regs = body.regs();
    let bits = body.bits;
    let at: HashMap<i64, usize> = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); body.blocks.len()];
    for (index, block) in body.blocks.iter().enumerate() {
        for to in block.succ.iter() {
            if let Some(&to) = at.get(to) {
                preds[to].push(index);
            }
        }
    }
    // Reverse post-order from the entry.
    let mut order = Vec::new();
    let mut seen: HashSet<usize> = HashSet::default();
    let mut stack = vec![(at[&body.entry], 0_usize)];
    seen.insert(at[&body.entry]);
    while let Some((index, next)) = stack.pop() {
        let succ = &body.blocks[index].succ;
        match succ.get(next).and_then(|to| at.get(to)).copied() {
            _ if next >= succ.len() => order.push(index),
            Some(to) => {
                stack.push((index, next + 1));
                if seen.insert(to) {
                    stack.push((to, 0));
                }
            }
            None => stack.push((index, next + 1)),
        }
    }
    order.reverse();

    // `RegDefs`, by block, by the register an entry loads.
    let mut defs: Vec<HashMap<Loc, Held>> = vec![HashMap::default(); body.blocks.len()];
    let mut redundant: HashSet<usize> = HashSet::default();
    for &index in &order {
        let mut mine: HashMap<Loc, Held> = HashMap::default();
        if let Some((&first, rest)) = preds[index].split_first() {
            for (register, held) in &defs[first] {
                let everywhere = rest.iter().all(|&pred| {
                    defs[pred].get(register).is_some_and(|other| Arc::ptr_eq(&other.insn, &held.insn) || {
                        other.insn.what.as_ref().zip(held.insn.what.as_ref()).is_some_and(|(a, b)| a.same_meaning(b))
                    })
                });
                if everywhere {
                    mine.insert(register.clone(), held.clone());
                }
            }
        }
        for one in &body.blocks[index].insns {
            let found = candidate(regs, one);
            if let Some(register) = &found
                && mine.get(register).is_some_and(|held| identical(held, one))
            {
                redundant.insert(id(one));
                continue;
            }
            match liveness::effect(regs, bits, one) {
                None => mine.clear(),
                Some(effect) => {
                    mine.retain(|_, held| held.lanes.is_disjoint(&effect.writes));
                    if let Some(register) = found {
                        mine.insert(register, Held { insn: Arc::clone(one), lanes: effect.writes });
                    }
                }
            }
        }
        defs[index] = mine;
    }
    if redundant.is_empty() {
        return body;
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            if block.insns.iter().any(|one| redundant.contains(&id(one))) {
                block.with_insns(
                    block
                        .insns
                        .iter()
                        .map(|one| if redundant.contains(&id(one)) { lir::anchor(Arc::clone(one)) } else { Arc::clone(one) })
                        .collect(),
                )
            } else {
                block.clone()
            }
        })
        .collect();
    body.with_blocks(blocks)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::RegId;

    use super::cleaned;
    use crate::model::ir::{Imm, Loc, Operation, Reg, Semantics};
    use crate::model::lir::{Block, Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn mov(
        at: i64,
        value: i64,
        register: RegId,
    ) -> Arc<Insn> {
        Arc::new(Insn::new(
            at,
            Some((at, at + 3)),
            Some(Semantics {
                name: Some("mov".to_owned()),
                dests: vec![Loc::Reg(Reg { register, width: 2 })],
                sources: vec![Loc::Imm(Imm { value, width: 2, address: None })],
                ..Semantics::new(Operation::Move)
            }),
            vec![value as u32],
            vec![],
        ))
    }

    fn block(
        at: i64,
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> Block {
        LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) }.into()
    }

    fn body(blocks: Vec<Block>) -> LirBody {
        LirBody::new("latecleanup", 0, blocks, IndexMap::default(), IndexMap::default())
    }

    fn gone(
        result: &LirBody,
        at: i64,
        index: usize,
    ) -> bool {
        let block = result.blocks.iter().find(|block| block.at == at).unwrap();
        block.insns[index].what.as_ref().unwrap().op == Operation::Nothing
    }

    /// The same constant loaded twice into a register nothing wrote between
    /// stays loaded twice (a second `mov ax, 5` per rematerialized use).
    #[test]
    fn a_load_the_register_already_holds_goes() {
        let result = cleaned(body(vec![block(0, vec![mov(0, 5, RegId::AX), mov(3, 1, RegId::BX), mov(6, 5, RegId::AX)], vec![])]));
        assert!(!gone(&result, 0, 0) && !gone(&result, 0, 1) && gone(&result, 0, 2));
    }

    /// Across a diamond: both paths leave `ax` as 5, the join's load goes; the
    /// load the diamond's other arm overwrote first stays.
    #[test]
    fn a_load_every_path_already_made_goes_and_one_another_path_changed_stays() {
        let diamond = |arm: Arc<Insn>| {
            body(vec![
                block(0, vec![mov(0, 5, RegId::AX)], vec![10, 20]),
                block(10, vec![], vec![30]),
                block(20, vec![arm], vec![30]),
                block(30, vec![mov(30, 5, RegId::AX)], vec![]),
            ])
        };
        assert!(gone(&cleaned(diamond(mov(20, 1, RegId::BX))), 30, 0), "every path holds ax = 5");
        assert!(!gone(&cleaned(diamond(mov(20, 7, RegId::AX))), 30, 0), "one path made ax 7");
    }

    /// A different constant is another load.
    #[test]
    fn a_load_of_another_constant_stays() {
        let result = cleaned(body(vec![block(0, vec![mov(0, 5, RegId::AX), mov(3, 6, RegId::AX)], vec![])]));
        assert!(!gone(&result, 0, 1));
    }
}
