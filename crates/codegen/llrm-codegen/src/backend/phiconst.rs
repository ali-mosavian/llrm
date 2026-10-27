//! One value for a constant that several edges of a phi carry.
//!
//! # How it works
//! A phi that takes the same constant from several predecessors, each making
//! it for itself (`v7 := mov 0` in one block, `v8 := mov 0` in another),
//! takes it from one value instead: made once, at the end of the nearest block
//! that dominates those predecessors. Nothing else changes; the phi's other
//! edges keep their values. This is what a source variable's constant is in
//! gcc's SSA, one name used by every phi it reaches, and it is what lets the
//! web of the phi, where it lives in a stack slot or one register, be written
//! at that definition and in the arm that assigns, and not on each edge of the
//! join: D_DRAW_FACES -Os lines 562-567 (six `lm_*` shorts zeroed at the top
//! of a loop and assigned under nested conditions) were stored on four edges.
//!
//! The constant is made there when the blocks it replaces together run at
//! least as often as that block (every path from there to the phi reads it: a
//! move on a path that does not holds a register for nothing, in bytes as in
//! clocks; priced by bytes alone, QCport -Os grew 630 B, 29 objects), when the
//! phi's own block does not dominate it (a loop's header would make it after
//! the phi), and for no relocatable constant and no x87 value.
//!
//! ```text
//!   d:  ...                         d:  ...
//!       br c, p1, p2        ==>         h := mov 0
//!   p1: v1 := mov 0                     br c, p1, p2
//!   p2: v2 := mov 0                 p1: ...                p2: ...
//!   j:  r = phi [p1: v1, p2: v2]    j:  r = phi [p1: h, p2: h]
//! ```
//!
//! # Example
//! `x = 0; ... if (a) x = g[1]; ... use(x)` in a loop: the zero is made in the
//! block under each test that does not assign, `x`'s phi takes three zeros and
//! `g[1]`. After the pass the three zeros are one `v9`, made in the block above
//! the tests, and the allocator holds `x` in one register or one slot, written
//! there and in the arm.
//!
//! ```text
//!   before                               after
//!   top:   v1 := mov 0                   top:   v9 := mov 0
//!          br a, t1, j                          br a, t1, j
//!   t1:    v2 := mov 0                   t1:    br c, t2, j
//!          br c, t2, j                   t2:    v3 := load g+2
//!   t2:    v3 := load g+2                       br j
//!          br j                          j:     x = phi [top: v9, t1: v9, t2: v3]
//!   j:     x = phi [top: v1, t1: v2, t2: v3]
//! ```
//!
//! # Runs
//! Phase 1 of the machine phases, before `SsaSpill`, so both allocation routes
//! take it.

use std::sync::Arc;

use crate::backend::phielim::{_constant_made_in, _nothing};
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBody, Phi};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct SharedConstants;

impl LIRTransform for SharedConstants {
    fn class_name(&self) -> &'static str {
        "SharedConstants"
    }

    fn name(&self) -> &str {
        "phiconst"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(shared(&body))
    }
}

/// `body` with each constant that several edges of one phi make for
/// themselves made once above them.
pub fn shared(body: &LirBody) -> LirBody {
    if !body.blocks.iter().any(|block| block.phis.iter().any(|phi| phi.incoming.len() >= 3)) {
        return body.clone();
    }
    let at_of: IndexMap<i64, &Block> = body.blocks.iter().map(|block| (block.at, block)).collect();
    let floating = crate::backend::regclass::floating(body);
    let mut next = body.next_value();
    // (the phi's block, the phi's result) -> where the constant is made, what
    // it is, the new value, the edges.
    let mut made: IndexMap<i64, Vec<(Held, Loc)>> = IndexMap::default();
    let mut incoming: IndexMap<(i64, u32), Vec<(Vec<i64>, u32)>> = IndexMap::default();
    let mut shared: Option<(
        std::rc::Rc<crate::analysis::loops::Dominance>,
        Arc<crate::analysis::frequency::Frequency>,
    )> = None;
    for block in &body.blocks {
        for phi in &block.phis {
            if phi.incoming.len() < 3 || floating.contains(&phi.result) {
                continue;
            }
            let mut by_constant: IndexMap<String, (Held, Loc, Vec<i64>)> = IndexMap::default();
            for &(where_, value) in &phi.incoming {
                let Some(pred) = at_of.get(&where_) else { continue };
                let Some(what) = _constant_made_in(pred, value) else { continue };
                let (Some(Loc::Held(into)), Some(Loc::Imm(imm))) = (what.dests.first(), what.sources.first()) else {
                    continue;
                };
                if imm.address.is_some() {
                    continue;
                }
                by_constant
                    .entry(format!("{}:{}:{}", imm.value, imm.width, into.width))
                    .or_insert_with(|| (*into, Loc::Imm(imm.clone()), Vec::new()))
                    .2
                    .push(where_);
            }
            for (into, imm, wheres) in by_constant.into_values() {
                if wheres.len() < 2 {
                    continue;
                }
                let (dominance, busy) = shared.get_or_insert_with(|| {
                    (
                        crate::analysis::loops::dominance(&body.blocks, Some(body.entry)),
                        crate::analysis::frequency::Frequency::of(body),
                    )
                });
                let mut above = wheres[0];
                while !wheres.iter().all(|&one| dominance.dominates(above, one)) {
                    match dominance.immediate(above) {
                        Some(up) => above = up,
                        None => break,
                    }
                }
                if !wheres.iter().all(|&one| dominance.dominates(above, one))
                    || above == block.at
                    || dominance.dominates(block.at, above)
                {
                    continue;
                }
                let replaced: f64 = wheres.iter().map(|&one| busy.block(one)).sum();
                if busy.block(above) > replaced * (1.0 - 1e-9) {
                    continue;
                }
                let value = next;
                next += 1;
                made.entry(above).or_default().push((Held { value, width: into.width }, imm));
                incoming.entry((block.at, phi.result)).or_default().push((wheres, value));
            }
        }
    }
    if made.is_empty() {
        return body.clone();
    }
    let mut out = body.clone();
    for block in &mut out.blocks {
        if let Some(constants) = made.get(&block.at) {
            let last = block.insns.last();
            let at = last.map_or(block.at, |last| last.at);
            let edge = _nothing(last.map(|last| &**last), at);
            let added: Vec<Arc<Insn>> = constants
                .iter()
                .map(|(held, imm)| {
                    Arc::new(Insn::new(
                        at,
                        Some(edge),
                        Some(Semantics {
                            name: Some("mov".into()),
                            dests: vec![Loc::Held(*held)],
                            sources: vec![imm.clone()],
                            ..Semantics::new(Operation::Move)
                        }),
                        vec![held.value],
                        Vec::new(),
                    ))
                })
                .collect();
            // Before the compare the branch reads: a `mov r, 0` between them
            // would keep the flags from being a `xor`'s to clobber.
            let mut cut = block.insns.len();
            while cut > 0
                && block.insns[cut - 1]
                    .what
                    .as_ref()
                    .is_some_and(|what| matches!(
                        what.op,
                        Operation::Jump | Operation::Branch | Operation::Return
                    ))
            {
                cut -= 1;
            }
            while cut > 0 && block.insns[cut - 1].what.as_ref().is_some_and(|what| what.op == Operation::Compare) {
                cut -= 1;
            }
            block.insns = block.insns[..cut].iter().chain(&added).chain(&block.insns[cut..]).cloned().collect();
        }
        let phis: Vec<Phi> = block
            .phis
            .iter()
            .map(|phi| match incoming.get(&(block.at, phi.result)) {
                Some(changes) => Phi {
                    result: phi.result,
                    incoming: phi
                        .incoming
                        .iter()
                        .map(|&(where_, value)| {
                            let renamed = changes.iter().find(|(wheres, _)| wheres.contains(&where_));
                            (where_, renamed.map_or(value, |(_, new)| *new))
                        })
                        .collect(),
                },
                None => phi.clone(),
            })
            .collect();
        block.phis = phis.into_iter().collect();
    }
    out
}
