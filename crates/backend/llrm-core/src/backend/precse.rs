//! Shadow count of what `MachineCSE` would remove before phi elimination, where
//! every vreg has one definition. Not wired into the pipeline's output: the body
//! is returned as it came. `LLRM_CSE_POC=1` prints one line per body.
//!
//! The pin's `MachineCSE::ProcessBlockCSE` walks the dominator tree with a
//! scoped value-number table; so does this. A candidate is a pure register
//! instruction with one definition, keyed by its operation and sources. It
//! overcounts: `seta`, `cdq` and the like read implicit state a key does not
//! hold, and a repeated `mov r, imm` is a rematerialisation the pin also
//! weighs (`isProfitableToCSE`).

use llrm_mir::domtree_address::AddressDominance;

use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, IndexMap};

/// Instructions a shadow `MachineCSE` finds repeated.
#[derive(Debug, Default, PartialEq)]
pub struct Hits {
    pub candidates: usize,
    /// Repeats of an instruction that sets no flags.
    pub plain: usize,
    /// Repeats of an instruction that sets flags (the repeat's flags may be read).
    pub flagging: usize,
}

fn key_of(one: &Insn) -> Option<(bool, String)> {
    let what = one.what.as_ref()?;
    let flagging = match what.op {
        Operation::Move | Operation::Address | Operation::Extend => false,
        Operation::Binary | Operation::Unary | Operation::Multiply | Operation::Funnel => true,
        _ => return None,
    };
    let general = |place: &Loc| matches!(place, Loc::Held(_) | Loc::Imm(_) | Loc::Address(_));
    if what.target.is_some()
        || what.indirect
        || one.defines.len() != 1
        || what.dests.len() != 1
        || !matches!(what.dests[0], Loc::Held(_))
        || !what.sources.iter().all(general)
        || one.call.is_some()
        || one.group.is_some()
        || one.volatile
        || one.spill_reload
        || one.spill_store
        || !one.clobbers.is_empty()
        || !one.requires.is_empty()
        || !one.delivers.is_empty()
    {
        return None;
    }
    let Loc::Held(dest) = &what.dests[0] else { return None };
    Some((flagging, format!("{:?}|{:?}|{}|{:?}", what.op, what.name, dest.width, what.sources)))
}

pub fn hits(body: &LirBody) -> Hits {
    let dominance = AddressDominance::of(body.blocks.iter().map(|block| (block.at, &block.succ[..])), Some(body.entry));
    let by_at: IndexMap<i64, usize> = body.blocks.iter().enumerate().map(|(index, block)| (block.at, index)).collect();
    let mut children: HashMap<i64, Vec<i64>> = HashMap::default();
    for block in &body.blocks {
        if let Some(parent) = dominance.immediate(block.at) {
            children.entry(parent).or_default().push(block.at);
        }
    }
    let mut out = Hits::default();
    // Scoped table: the keys a block added, removed when its subtree is done.
    let mut table: HashMap<String, u32> = HashMap::default();
    let mut stack: Vec<(i64, Option<Vec<String>>)> = vec![(body.entry, None)];
    while let Some((at, added)) = stack.pop() {
        if let Some(added) = added {
            for key in added {
                table.remove(&key);
            }
            continue;
        }
        let mut mine = Vec::new();
        for one in &body.blocks[by_at[&at]].insns {
            let Some((flagging, key)) = key_of(one) else { continue };
            out.candidates += 1;
            if table.contains_key(&key) {
                if flagging {
                    out.flagging += 1;
                } else {
                    out.plain += 1;
                }
            } else {
                table.insert(key.clone(), one.defines[0]);
                mine.push(key);
            }
        }
        stack.push((at, Some(mine)));
        if let Some(kids) = children.get(&at) {
            stack.extend(kids.iter().map(|kid| (*kid, None)));
        }
    }
    out
}

pub struct ShadowCse;

impl LIRTransform for ShadowCse {
    fn class_name(&self) -> &'static str {
        "ShadowCse"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        if std::env::var_os("LLRM_CSE_POC").is_some() {
            let found = hits(&body);
            eprintln!("CSE pre candidates={} plain={} flagging={}", found.candidates, found.plain, found.flagging);
        }
        Ok(body)
    }
}
