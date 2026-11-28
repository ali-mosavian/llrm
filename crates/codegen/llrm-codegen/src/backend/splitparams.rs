// SPDX-License-Identifier: GPL-3.0-or-later
//! Splits the live range of an argument register where the calls begin, so
//! the frame and the kept registers can wait for them. Translated from gcc's
//! `split_live_ranges_for_shrink_wrap` (gcc/ira.cc).
//!
//! # How it works
//! A function that tests its arguments and returns at once (a recursion's
//! base case) keeps an argument across its calls, so the allocator copies it
//! into a callee-saved register at the entry. That copy is a use of a kept
//! register in the entry block, and shrink wrapping then sets the frame up
//! there for every path. Here the entry keeps the register the argument
//! arrives in, and the block that dominates every call (outside any loop)
//! copies it to a new value that the rest of the body reads:
//!
//! ```text
//!   entry: n = arrive(eax)          entry: n = arrive(eax)
//!          cmp n, 2                        cmp n, 2
//!          jl  done                        jl  done
//!   body:  ... call ... n ...       body:  m = mov n
//!   done:  ret                             ... call ... m ...
//! ```
//!
//! Refused where the dominator is the entry, where it lies in a loop (it climbs
//! to the nearest block that does not), where every path runs through it (no
//! path would skip the frame), or where the entry itself calls.
//!
//! # Runs
//! On SSA, before the phis go, after every pass that moves values. Only where
//! `Target::splits_params_for_shrink_wrap` is true (flow drops the phase
//! elsewhere). The coalescer keeps the copy it makes
//! (`CoalescerEnv::keeps_split_params`): joined, it would be the entry's again.
//!
//! # See also
//! [`crate::backend::shrinkwrap`], which places the set-up this makes possible.
//!
//! # Example
//! `fib(n)` tests `n < 2` and returns; the block of its calls reads `m = mov
//! n`, so the entry sets up no frame.

use std::sync::Arc;

use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

/// `split_live_ranges_for_shrink_wrap`. A target that does not state
/// `Target::splits_params_for_shrink_wrap` has no such phase.
pub struct SplitParamRanges;

impl LIRTransform for SplitParamRanges {
    fn class_name(&self) -> &'static str {
        "SplitParamRanges"
    }

    fn name(&self) -> &str {
        "splitparams"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(split(&body))
    }
}

fn calls(insn: &Insn) -> bool {
    insn.what.as_ref().is_some_and(|what| what.op == Operation::Call)
}

/// `body` with the arguments the calls need read from a copy made where the
/// calls begin.
pub fn split(body: &LirBody) -> LirBody {
    let Some(entry) = body.blocks.iter().find(|block| block.at == body.entry) else { return body.clone() };
    let arrived: Vec<Held> = entry
        .insns
        .iter()
        .filter(|insn| insn.arrival())
        .flat_map(|insn| insn.delivers.iter().map(|(held, _)| *held))
        .filter(|held| held.offset == 0)
        .collect();
    if arrived.is_empty() || body.blocks.len() < 3 {
        return body.clone();
    }
    let floating = crate::backend::regclass::floating(body);
    let arrived: Vec<Held> = arrived.into_iter().filter(|held| !floating.contains(&held.value)).collect();
    let successors: HashMap<i64, Vec<i64>> =
        body.blocks.iter().map(|block| (block.at, block.succ.iter().copied().collect())).collect();
    let call_blocks: Vec<i64> =
        body.blocks.iter().filter(|block| block.insns.iter().any(|insn| calls(insn))).map(|block| block.at).collect();
    if call_blocks.is_empty() || call_blocks.contains(&body.entry) {
        return body.clone();
    }
    let mut reachable: HashSet<i64> = HashSet::default();
    let mut queue = call_blocks.clone();
    while let Some(at) = queue.pop() {
        if reachable.insert(at) {
            queue.extend(successors.get(&at).into_iter().flatten().copied());
        }
    }
    // The blocks the new value is for: the ones with a call, and the ones the
    // calls reach that read an argument.
    let mut need_new: Vec<i64> = call_blocks;
    for block in &body.blocks {
        if reachable.contains(&block.at)
            && !need_new.contains(&block.at)
            && block.insns.iter().any(|insn| arrived.iter().any(|held| insn.uses.contains(&held.value)))
        {
            need_new.push(block.at);
        }
    }
    let dominance = crate::analysis::loops::dominance(&body.blocks, Some(body.entry));
    let mut call_dom = need_new[0];
    for &one in &need_new[1..] {
        while !dominance.dominates(call_dom, one) {
            match dominance.immediate(call_dom) {
                Some(up) => call_dom = up,
                None => return body.clone(),
            }
        }
    }
    let depth = crate::analysis::loops::depth(&body.blocks, Some(body.entry));
    while depth.get(&call_dom).copied().unwrap_or(0) > 0 {
        match dominance.immediate(call_dom) {
            Some(up) => call_dom = up,
            None => return body.clone(),
        }
    }
    if call_dom == body.entry {
        return body.clone();
    }
    // Some path from the entry leaves without it: otherwise nothing waits.
    let mut seen: HashSet<i64> = HashSet::default();
    let mut walk = vec![body.entry];
    let mut skips = false;
    while let Some(at) = walk.pop() {
        if at == call_dom || !seen.insert(at) {
            continue;
        }
        let next = successors.get(&at).map(Vec::as_slice).unwrap_or(&[]);
        skips |= next.is_empty();
        walk.extend(next.iter().copied());
    }
    if !skips {
        return body.clone();
    }
    let below = |at: i64| at == call_dom || dominance.dominates(call_dom, at);
    let mut next = body.next_value();
    let mut made: Vec<(Held, Held)> = Vec::new();
    for held in arrived {
        let read_below = body
            .blocks
            .iter()
            .any(
                |block| (below(block.at) && block.insns.iter().any(|insn| insn.uses.contains(&held.value)))
                    || block
                        .phis
                        .iter()
                        .any(|phi| phi.incoming.iter().any(|&(from, value)| value == held.value && below(from))),
            );
        if read_below {
            made.push((held, Held { value: next, width: held.width, offset: 0 }));
            next += 1;
        }
    }
    if made.is_empty() {
        return body.clone();
    }
    let rename = |value: u32| made.iter().find(|(old, _)| old.value == value).map_or(value, |(_, new)| new.value);
    let mut out = body.clone();
    let mut like = (*body.class_like).clone();
    for (old, new) in &made {
        like.insert(new.value, old.value);
    }
    out.class_like = Arc::new(like);
    for block in &mut out.blocks {
        if below(block.at) {
            let mut insns: Vec<Arc<Insn>> = block.insns.iter().map(|insn| insn.renamed(&rename)).collect();
            if block.at == call_dom {
                let first = insns.first().map(|insn| &**insn);
                let at = first.map_or(block.at, |insn| insn.at);
                let covers = crate::backend::phielim::_nothing(first, at);
                let copies = made
                    .iter()
                    .map(
                        |(old, new)| Arc::new(Insn::new(
                            at,
                            Some(covers),
                            Some(Semantics {
                                name: Some("mov".into()),
                                dests: vec![Loc::Held(*new)],
                                sources: vec![Loc::Held(*old)],
                                ..Semantics::new(Operation::Move)
                            }),
                            vec![new.value],
                            vec![old.value],
                        )),
                    );
                insns.splice(0..0, copies);
            }
            block.insns = insns.into();
        }
        let phis: Vec<Phi> = block
            .phis
            .iter()
            .map(|phi| Phi {
                result: phi.result,
                incoming: phi
                    .incoming
                    .iter()
                    .map(|&(from, value)| (from, if below(from) { rename(value) } else { value }))
                    .collect(),
            })
            .collect();
        block.phis = phis.into_iter().collect();
    }
    out
}
