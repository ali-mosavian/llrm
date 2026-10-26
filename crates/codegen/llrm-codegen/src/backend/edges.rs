//! Breaks one edge of a body: a block of its own that jumps on.
//!
//! # How it works
//! `split` makes a block holding a jump to the edge's target, points the
//! source's branch (or its fall-through, by a jump after the branch) at it,
//! and makes the target's phis read it where they read the source. The
//! block's label is above every physical offset, in a namespace per body
//! entry, as the blocks `phielim` makes for copies on edges are.
//!
//! # Example
//! The edge `b1 -> b4` of `b1: je b4; b2: ...; b4:` becomes `b1 -> e -> b4`
//! with `e: jmp b4`, and `b4`'s phi that read `b1` reads `e`.
//!
//! # Used by
//! `machinesink`.

use std::sync::Arc;

use crate::model::ir::{Operation, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};

/// `body` with the edge `from -> to` through a block of its own, and that
/// block's label; None where `from` has no such edge or has two to `to`.
pub fn split(
    body: &LirBody,
    from: i64,
    to: i64,
) -> Option<(LirBody, i64)> {
    let source = body.blocks.iter().find(|block| block.at == from)?;
    if source.succ.iter().filter(|next| **next == to).count() != 1 {
        return None;
    }
    let highest = body.blocks.iter().map(|block| block.at).max()?;
    let at = ((body.entry + 1) << 32).max(highest) + 1;
    let beside = source.insns.last()?;
    let jump = Semantics { name: Some("jmp".into()), target: Some(to), ..Semantics::new(Operation::Jump) };
    let mut made = Insn::new(at, Some((at, at)), Some(jump), vec![], vec![]);
    made.call = beside.call.clone();
    let made = Arc::new(made);
    let landing = |next: i64| if next == to { at } else { next };
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            if block.at == from {
                let mut insns: Vec<Arc<Insn>> = block
                    .insns
                    .iter()
                    .map(|one| {
                        let Some(moved) = one.what.as_ref().and_then(|what| what.remapped(landing)) else {
                            return Arc::clone(one);
                        };
                        let mut out = (**one).clone();
                        out.what = Some(moved);
                        Arc::new(out)
                    })
                    .collect();
                // A conditional branch falls through to its other successor:
                // the edge that does not name a target gets an
                // explicit jump.
                if let Some(last) = block.insns.last()
                    && let Some(what) = last.what.as_ref().filter(|what| what.op == Operation::Branch)
                    && what.target != Some(to)
                {
                    let mut leave = Insn::new(
                        last.at,
                        Some((last.at, last.at)),
                        Some(Semantics {
                            name: Some("jmp".into()),
                            target: Some(at),
                            ..Semantics::new(Operation::Jump)
                        }),
                        vec![],
                        vec![],
                    );
                    leave.call = last.call.clone();
                    insns.push(Arc::new(leave));
                }
                let mut out = block.with_insns(insns);
                out.succ = block.succ.iter().map(|&next| landing(next)).collect::<Vec<_>>().into();
                out
            } else if block.at == to {
                let mut out = block.clone();
                for phi in out.phis.iter_mut() {
                    for (pred, _) in &mut phi.incoming {
                        if *pred == from {
                            *pred = at;
                        }
                    }
                }
                out
            } else {
                block.clone()
            }
        })
        .chain([LirBlock {
            at,
            insns: vec![made].into(),
            succ: vec![to].into(),
            phis: vec![].into(),
            cold: false,
            align: 0,
            max_skip: 0,
        }
        .into()])
        .collect();
    let mut out = body.clone();
    out.odds.rerouted(from, &source.succ, to, &[(at, 1.0)]);
    out.blocks = blocks;
    Some((out, at))
}
