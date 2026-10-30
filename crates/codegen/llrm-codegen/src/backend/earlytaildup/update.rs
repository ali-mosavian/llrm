//! `MachineSSAUpdater` for the values a tail duplication copied: the uses of a
//! value that now has several definitions (the original, if the tail stays, and
//! one in each predecessor that took a copy) read the definition that reaches
//! them, with a phi where definitions meet.
//!
//! # How it works
//! `avail` says which blocks end with a definition of the value. A use in a
//! block with none reads what reaches the block's start: the one definition of
//! its only predecessor, or a new phi over the predecessors' (the walk of
//! Braun et al.; a phi that turns out to join one value is not made, unless
//! another phi of the walk already reads it). A phi's operand from a block is
//! read at that block's end.
//!
//! # Example
//! `v` is defined in `t` and copied as `v1` into `p1` and `v2` into `p2`, all
//! three reaching `s`: a use in `s` reads a new phi of `v1`, `v2` (and `v`).
//!
//! # Used by
//! `earlytaildup::duplicate`.

use crate::model::lir::{LirBlock, Phi};
use crate::support::hash::{HashMap, HashSet};

pub(super) struct Updater<'a> {
    blocks: &'a mut Vec<LirBlock>,
    value: u32,
    avail: HashMap<i64, u32>,
    next: &'a mut u32,
    start: HashMap<i64, Option<u32>>,
    referenced: HashSet<u32>,
    walking: HashMap<i64, u8>,
    added: HashMap<i64, Vec<Phi>>,
}

impl<'a> Updater<'a> {
    /// Rewrites every use of `value` in `blocks`, but the instructions of the
    /// blocks in `keep` (where `value` is defined, and kept).
    pub(super) fn rewrite(
        blocks: &'a mut Vec<LirBlock>,
        value: u32,
        avail: HashMap<i64, u32>,
        keep: &[i64],
        next: &'a mut u32,
    ) {
        let mut this = Self {
            blocks,
            value,
            avail,
            next,
            start: HashMap::default(),
            referenced: HashSet::default(),
            walking: HashMap::default(),
            added: HashMap::default(),
        };
        let places: Vec<i64> = this.blocks.iter().map(|block| block.at).collect();
        for at in places {
            let index = this.index(at);
            let reads = !keep.contains(&at)
                && this.blocks[index].insns.iter().any(|one| one.uses.contains(&value) || one.undefs.contains(&value));
            if reads && let Some(there) = this.value_at_start(at) {
                let insns: Vec<_> = this.blocks[index]
                    .insns
                    .iter()
                    .map(|one| {
                        if one.uses.contains(&value) || one.undefs.contains(&value) {
                            one.renamed(&|v| if v == value { there } else { v })
                        } else {
                            std::sync::Arc::clone(one)
                        }
                    })
                    .collect();
                this.blocks[index].insns = insns.into();
            }
            let phis: Vec<Phi> = this.blocks[index].phis.iter().cloned().collect();
            let mut changed = Vec::new();
            for mut phi in phis {
                for entry in phi.incoming.iter_mut() {
                    if entry.1 == value
                        && let Some(there) = this.value_at_end(entry.0)
                    {
                        entry.1 = there;
                    }
                }
                changed.push(phi);
            }
            this.blocks[index].phis = changed.into();
        }
        for (at, phis) in std::mem::take(&mut this.added) {
            let index = this.index(at);
            let mut all: Vec<Phi> = this.blocks[index].phis.iter().cloned().collect();
            all.extend(phis);
            this.blocks[index].phis = all.into();
        }
    }

    fn index(
        &self,
        at: i64,
    ) -> usize {
        self.blocks.iter().position(|block| block.at == at).expect("a block of the body")
    }

    fn preds(
        &self,
        at: i64,
    ) -> Vec<i64> {
        self.blocks.iter().filter(|block| block.succ.contains(&at)).map(|block| block.at).collect()
    }

    fn value_at_end(
        &mut self,
        at: i64,
    ) -> Option<u32> {
        match self.avail.get(&at) {
            Some(value) => Some(*value),
            None => self.value_at_start(at),
        }
    }

    fn value_at_start(
        &mut self,
        at: i64,
    ) -> Option<u32> {
        if let Some(known) = self.start.get(&at) {
            if let Some(value) = known {
                self.referenced.insert(*value);
            }
            return *known;
        }
        let preds = self.preds(at);
        match preds[..] {
            [] => {
                self.start.insert(at, None);
                None
            }
            [only] => {
                // A live cycle is entered by a block with two predecessors,
                // whose phi the second visit finds; a cycle of blocks with one
                // each is dead code and reads nothing.
                let depth = self.walking.entry(at).or_insert(0);
                if *depth >= 2 {
                    return None;
                }
                *depth += 1;
                let there = self.value_at_end(only);
                if let Some(depth) = self.walking.get_mut(&at) {
                    *depth -= 1;
                }
                self.start.insert(at, there);
                there
            }
            _ => {
                let made = *self.next;
                *self.next += 1;
                self.start.insert(at, Some(made));
                let mut incoming = Vec::new();
                for pred in preds {
                    // A path no definition reaches reads nothing new.
                    let there = self.value_at_end(pred).unwrap_or(self.value);
                    incoming.push((pred, there));
                }
                let mut distinct: Vec<u32> = incoming.iter().map(|(_, value)| *value).filter(|v| *v != made).collect();
                distinct.sort_unstable();
                distinct.dedup();
                if let [only] = distinct[..]
                    && !self.referenced.contains(&made)
                {
                    self.start.insert(at, Some(only));
                    return Some(only);
                }
                self.added.entry(at).or_default().push(Phi { result: made, incoming });
                Some(made)
            }
        }
    }
}
