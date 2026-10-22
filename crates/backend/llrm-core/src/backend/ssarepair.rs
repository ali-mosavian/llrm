//! Restores SSA after the spiller gave some values more than one definition.
//!
//! # How it works
//! A reload brings a value back under the same name, so the value has several
//! definitions, and every use after a reload must read it, or a phi where two
//! definitions meet. This is the classic construction (Cytron et al.; Braun et
//! al. 2013 reach the same result on the fly):
//!
//! 1. Phis go at the iterated dominance frontier of each redefined value's
//!    definitions, pruned to blocks where the value is live in.
//! 2. Renaming walks the dominator tree with a stack of names per value.
//!
//! A definition is a reload when the instruction says so (`spill_reload` or
//! `rematerialized`) and defines one of the redefined values. The first
//! definition keeps its name and each reload is named afresh. A block that
//! holds the value in no register on entry (`held`) reloads it before every
//! use, so it needs no phi.
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   a["<b>A: v := ...</b>"]:::mid
//!   b["<b>B</b>"]:::mid
//!   c["<b>C: ...</b>"]:::mid
//!   d["<b>D: uses v</b>"]:::mid
//!   a --> b
//!   a --> c
//!   b --> d
//!   c --> d
//! ```
//!
//! *Before: v has one definition*
//!
//! ```mermaid
//! %%{init: {'theme':'base','themeVariables':{'fontFamily':'Segoe UI, Helvetica Neue, Arial, DejaVu Sans, sans-serif','fontSize':'15px','primaryColor':'#475569','primaryTextColor':'#ffffff','primaryBorderColor':'#334155','lineColor':'#64748b','secondaryColor':'#475569','tertiaryColor':'transparent','clusterBkg':'transparent','clusterBorder':'#64748b','edgeLabelBackground':'#334155','textColor':'#64748b','titleColor':'#64748b'},'flowchart':{'curve':'basis','nodeSpacing':38,'rankSpacing':52,'padding':16,'wrappingWidth':360,'htmlLabels':true,'useMaxWidth':true}}}%%
//! flowchart TB
//!   classDef mid fill:#475569,stroke:#334155,stroke-width:2px,color:#ffffff,rx:12px,ry:12px
//!   linkStyle default stroke:#64748b,stroke-width:2.2px
//!   a["<b>A: v := ...</b>"]:::mid
//!   b["<b>B: v1 := reload</b>"]:::mid
//!   c["<b>C: ...</b>"]:::mid
//!   d["<b>D: v2 := phi(v1:B, v:C)</b><br/><small>uses v2</small>"]:::mid
//!   a --> b
//!   a --> c
//!   b --> d
//!   c --> d
//! ```
//!
//! *After: a reload in B and a phi in D*
//!
//! `simplified` removes the phis whose arguments are all one value (or the phi
//! itself) and renames every use to that value, repeated to a fixed point
//! because one removal can make another phi trivial.
//!
//! # Example
//! `v5` and `v6` are live across a call in one arm of an `if`. The spiller
//! stores them before the call and reloads them after it, and repair gives the
//! join a phi for each. Before, the spiller's input:
//!
//! ```lir
//! === FarIndirectCalls
//!     block 0x0013
//!       v15 := phi 0x001f:v12 0x001c:v14
//!       0x0019  v21 := add v20, v5  ; inserted
//!       0x001a  v22 := add v21, v6  ; inserted
//!     block 0x001c
//!       0x001c  call   ; inserted; requires v3@AX; delivers v13@EAX; clobbers ...
//!       0x001d  v14 := add v12, v13  ; inserted
//!       0x001e  jmp   ; inserted
//! ```
//!
//! After, with the reloads and the repaired phis:
//!
//! ```lir
//! === SsaSpill
//!     block 0x0013
//!       v15 := phi 0x001f:v12 0x001c:v14
//!       v23 := phi 0x001c:v26 0x001f:v5
//!       v24 := phi 0x001c:v27 0x001f:v6
//!       0x0019  v21 := add v20, v23  ; inserted
//!       0x001a  v22 := add v21, v24  ; inserted
//!     block 0x001c
//!       0x001c  [[bp-0x8]] := mov v6  ; inserted
//!       0x001c  [[bp-0x6]] := mov v5  ; inserted
//!       0x001c  call   ; inserted; requires v3@AX; delivers v13@EAX; clobbers ...
//!       0x001d  v14 := add v12, v13  ; inserted
//!       0x001e  v26 := mov [[bp-0x6]]  ; inserted; spill reload
//!       0x001e  v27 := mov [[bp-0x8]]  ; inserted; spill reload
//!       0x001e  jmp   ; inserted
//! ```
//!
//! The arm through the call reads the reloads `v26` and `v27`, the other arm
//! still has `v5` and `v6`, so the uses in the join read the phis `v23` and
//! `v24`.
//!
//! From `ISEL_DUMP=1 LLRM_CANDIDATES=spiller` of a C function with
//! `if (c) s += g(a);` and seven loads from `p`, at -O2 -m16, trimmed.
//!
//! # Runs
//! Inside [`crate::backend::ssaspill`]: `simplified` first, `repaired` last.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::loops;
use crate::backend::{allocate, spiller};
use crate::model::lir::{Block, Insn, LirBlock, LirBody, Phi};
use crate::support::hash::IndexMap;

/// Whether `one` brings back a value `redefined` names.
fn reloads(
    one: &Insn,
    redefined: &BTreeSet<u32>,
) -> Option<u32> {
    match one.defines.as_slice() {
        [value] if (one.spill_reload || one.rematerialized) && redefined.contains(value) => Some(*value),
        _ => None,
    }
}

/// `body` in SSA again: every value in `redefined` has one definition per name.
pub fn repaired(
    body: &LirBody,
    redefined: &BTreeSet<u32>,
    held: &IndexMap<i64, BTreeSet<u32>>,
) -> LirBody {
    let (live_in, _) = allocate::live(body);
    let graph = &body.blocks;
    let by_at: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let idom = loops::immediate_dominators(&graph, Some(body.entry));
    let mut preds: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        for to in &block.succ {
            preds.entry(*to).or_default().push(block.at);
        }
    }
    // Dominance frontiers.
    let frontier = loops::frontiers(&graph, Some(body.entry));
    // Where each value is defined, and so where it needs a phi.
    let mut defined: IndexMap<u32, BTreeSet<i64>> = IndexMap::default();
    for block in &body.blocks {
        for phi in &block.phis {
            if redefined.contains(&phi.result) {
                defined.entry(phi.result).or_default().insert(block.at);
            }
        }
        for one in &block.insns {
            for value in &one.defines {
                if redefined.contains(value) {
                    defined.entry(*value).or_default().insert(block.at);
                }
            }
        }
    }
    let mut placed: IndexMap<i64, Vec<u32>> = IndexMap::default();
    for (value, blocks) in &defined {
        let mut work: Vec<i64> = blocks.iter().copied().collect();
        let mut has: BTreeSet<i64> = BTreeSet::new();
        while let Some(at) = work.pop() {
            for next in frontier.get(&at).into_iter().flatten() {
                if has.insert(*next) {
                    // A block that does not take the value into a register
                    // reloads it before every use: no phi.
                    let registered = held.get(next).is_none_or(|set| set.contains(value));
                    if registered
                        && live_in[next].contains(value)
                        && !by_at.get(next).is_some_and(|block| block.phis.iter().any(|phi| phi.result == *value))
                    {
                        placed.entry(*next).or_default().push(*value);
                    }
                    if !blocks.contains(next) {
                        work.push(*next);
                    }
                }
            }
        }
    }
    // Rename down the dominator tree.
    let mut children: IndexMap<i64, Vec<i64>> = IndexMap::default();
    for block in &body.blocks {
        if let Some(Some(parent)) = idom.get(&block.at) {
            children.entry(*parent).or_default().push(block.at);
        }
    }
    let mut next = body.next_value();
    let mut stacks: IndexMap<u32, Vec<u32>> = IndexMap::default();
    let mut insns_of: IndexMap<i64, Vec<Arc<Insn>>> = IndexMap::default();
    let mut phis_of: IndexMap<i64, Vec<Phi>> =
        body.blocks.iter().map(|block| (block.at, block.phis.to_vec())).collect();
    // The result of the phi placed for a value in a block.
    let mut made: IndexMap<(i64, u32), u32> = IndexMap::default();
    // (block, value) whose phi still needs its operand from a predecessor, in
    // rename order. Each placed phi is named before any block is renamed: a
    // predecessor visited first hands it an operand.
    for (at, values) in &placed {
        for value in values {
            let name = next;
            next += 1;
            made.insert((*at, *value), name);
            phis_of.get_mut(at).expect("a block").push(Phi { result: name, incoming: Vec::new() });
        }
    }
    let mut todo: Vec<(i64, bool)> = vec![(body.entry, false)];
    let mut pushed: IndexMap<i64, Vec<u32>> = IndexMap::default();
    while let Some((at, leaving)) = todo.pop() {
        if leaving {
            for variable in pushed.shift_remove(&at).unwrap_or_default() {
                stacks.get_mut(&variable).expect("a stack").pop();
            }
            continue;
        }
        let block = by_at[&at];
        let mut mine: Vec<u32> = Vec::new();
        for phi in &block.phis {
            if redefined.contains(&phi.result) {
                stacks.entry(phi.result).or_default().push(phi.result);
                mine.push(phi.result);
            }
        }
        for value in placed.get(&at).into_iter().flatten() {
            stacks.entry(*value).or_default().push(made[&(at, *value)]);
            mine.push(*value);
        }
        let mut insns: Vec<Arc<Insn>> = Vec::new();
        for one in &block.insns {
            let mut rename: IndexMap<u32, u32> = IndexMap::default();
            for value in &one.uses {
                if redefined.contains(value) {
                    if let Some(top) = stacks.get(value).and_then(|stack| stack.last()) {
                        if top != value {
                            rename.insert(*value, *top);
                        }
                    }
                }
            }
            let mut one = if rename.is_empty() { Arc::clone(one) } else { spiller::_renamed(one, &rename) };
            if let Some(value) = reloads(&one, redefined) {
                let name = next;
                next += 1;
                one = spiller::_renamed(&one, &IndexMap::from_iter([(value, name)]));
                stacks.entry(value).or_default().push(name);
                mine.push(value);
            } else {
                for value in &one.defines {
                    if redefined.contains(value) {
                        stacks.entry(*value).or_default().push(*value);
                        mine.push(*value);
                    }
                }
            }
            insns.push(one);
        }
        insns_of.insert(at, insns);
        // The operands this block hands to the phis of its successors.
        for to in &block.succ {
            for value in placed.get(to).into_iter().flatten() {
                if let (Some(top), Some(name)) =
                    (stacks.get(value).and_then(|stack| stack.last()), made.get(&(*to, *value)))
                {
                    if let Some(phi) =
                        phis_of.get_mut(to).and_then(|phis| phis.iter_mut().find(|phi| phi.result == *name))
                    {
                        phi.incoming.push((at, *top));
                    }
                }
            }
            if let Some(phis) = phis_of.get_mut(to) {
                for phi in phis.iter_mut() {
                    if by_at[to].phis.iter().any(|own| own.result == phi.result) {
                        for (from, value) in &mut phi.incoming {
                            if *from == at && redefined.contains(value) {
                                if let Some(top) = stacks.get(value).and_then(|stack| stack.last()) {
                                    *value = *top;
                                }
                            }
                        }
                    }
                }
            }
        }
        pushed.insert(at, mine);
        todo.push((at, true));
        for child in children.get(&at).into_iter().flatten().rev() {
            todo.push((*child, false));
        }
    }
    let rebuilt = body
        .blocks
        .iter()
        .map(|block| {
            let insns = insns_of.get(&block.at).cloned().unwrap_or_else(|| block.insns.to_vec());
            let phis = phis_of.get(&block.at).cloned().unwrap_or_default();
            LirBlock { phis: phis.into(), ..block.with_insns(insns).into_inner() }
        })
        .collect();
    body.with_blocks(rebuilt)
}

/// `body` without the phis that name one value: a phi whose arguments are all
/// one value `x` (or itself) is `x`, and every use of it reads `x`. Removing
/// one can make another trivial, so this runs to a fixed point.
pub fn simplified(body: &LirBody) -> Option<LirBody> {
    let mut found = false;
    let mut body = body.clone();
    loop {
        let mut rename: IndexMap<u32, u32> = IndexMap::default();
        for block in &body.blocks {
            for phi in block.phis.iter().filter(|phi| !body.pins.contains_key(&phi.result)) {
                let mut others = phi.incoming.iter().map(|(_, value)| *value).filter(|value| *value != phi.result);
                if let Some(only) = others.next().filter(|first| others.all(|value| value == *first)) {
                    rename.insert(phi.result, only);
                }
            }
        }
        if rename.is_empty() {
            return found.then_some(body);
        }
        found = true;
        // A chain of trivial phis reads through to its end.
        let ends: IndexMap<u32, u32> = rename
            .keys()
            .map(|value| {
                let mut at = rename[value];
                while let Some(next) = rename.get(&at).filter(|next| **next != *value) {
                    at = *next;
                }
                (*value, at)
            })
            .collect();
        let blocks = body
            .blocks
            .iter()
            .map(|block| {
                let insns = block
                    .insns
                    .iter()
                    .map(|one| {
                        if one.uses.iter().any(|value| ends.contains_key(value)) {
                            spiller::_renamed(one, &ends)
                        } else {
                            Arc::clone(one)
                        }
                    })
                    .collect();
                let phis = block
                    .phis
                    .iter()
                    .filter(|phi| !ends.contains_key(&phi.result))
                    .map(|phi| Phi {
                        result: phi.result,
                        incoming: phi
                            .incoming
                            .iter()
                            .map(|(from, value)| (*from, ends.get(value).copied().unwrap_or(*value)))
                            .collect(),
                    })
                    .collect();
                LirBlock { phis, ..block.with_insns(insns).into_inner() }
            })
            .collect();
        body = body.with_blocks(blocks);
    }
}
