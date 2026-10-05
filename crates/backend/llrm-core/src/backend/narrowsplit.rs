//! A byte read of a word that no register can hold at both widths.
//!
//! `trunc` reads the low byte of its word in place. Where that word is also a
//! frame array's index it must sit in si or di, which have no byte half, and
//! the class check refuses the body ("value may be in no register"). Each such
//! read takes a copy of its own, live across that read alone.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::backend::{regclass, target};
use crate::model::ir::{self, Held, Loc, Operation, Semantics};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;

pub struct NarrowSplit {
    pub segments: target::Segments,
}

impl LIRTransform for NarrowSplit {
    fn class_name(&self) -> &'static str {
        "NarrowSplit"
    }

    fn name(&self) -> &str {
        "narrow-split"
    }

    fn transform(&mut self, body: LirBody) -> Result<LirBody, String> {
        Ok(split(&body, &self.segments))
    }
}

/// `body`, each byte read of a word with no register given a word copy to read.
pub fn split(body: &LirBody, segments: &target::Segments) -> LirBody {
    let stuck: BTreeSet<u32> = regclass::classes(body, &BTreeSet::new(), segments)
        .into_iter()
        .filter(|(_, class)| target::order(Some(class), segments).is_empty())
        .map(|(value, _)| value)
        .collect();
    if stuck.is_empty() {
        return body.clone();
    }
    // The widest any instruction names it at: a phi's result has no defining operand.
    let mut widths = std::collections::BTreeMap::<u32, u32>::new();
    for held in body.insns().iter().filter_map(|one| one.what.as_ref()).flat_map(|what| what.dests.iter().chain(&what.sources)).flat_map(ir::values) {
        let wide = widths.entry(held.value).or_default();
        *wide = (*wide).max(held.width);
    }
    let mut fresh = body
        .insns()
        .iter()
        .flat_map(|one| one.defines.iter().chain(&one.uses))
        .copied()
        .chain(body.blocks.iter().flat_map(|block| block.phis.iter().map(|phi| phi.result)))
        .max()
        .unwrap_or(0)
        + 1;
    let blocks = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns: Vec<Arc<Insn>> = Vec::new();
            for one in &block.insns {
                let Some(what) = &one.what else {
                    insns.push(Arc::clone(one));
                    continue;
                };
                let mut sources = what.sources.clone();
                let mut made = (**one).clone();
                let mut replaced = Vec::new();
                for place in &mut sources {
                    let Loc::Held(held) = place else { continue };
                    let Some(&wide) = widths.get(&held.value).filter(|&&wide| held.width == 1 && wide > 1 && stuck.contains(&held.value)) else { continue };
                    let copy = Held { value: fresh, width: wide };
                    fresh += 1;
                    let mut moved = Insn::new(
                        one.at,
                        Some((one.at, one.at)),
                        Some(Semantics { name: Some("mov".to_owned()), dests: vec![Loc::Held(copy)], sources: vec![Loc::Held(Held { value: held.value, width: wide })], ..Semantics::new(Operation::Move) }),
                        vec![copy.value],
                        vec![held.value],
                    );
                    moved.call = one.call.clone();
                    insns.push(Arc::new(moved));
                    replaced.push(held.value);
                    made.uses.push(copy.value);
                    *place = Loc::Held(Held { value: copy.value, width: 1 });
                }
                for value in replaced {
                    if !sources.iter().chain(&what.dests).flat_map(ir::values).any(|one| one.value == value) {
                        made.uses.retain(|used| *used != value);
                    }
                }
                made.what = Some(Semantics { sources, ..what.clone() });
                insns.push(Arc::new(made));
            }
            block.with_insns(insns)
        })
        .collect();
    body.with_blocks(blocks)
}
