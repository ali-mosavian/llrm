//! Where each value is defined and read in a body, kept as the body changes.
//!
//! A spill or split rewrites the instructions that name its values and keeps every other `Arc<Insn>`, so
//! the answers of the spiller's scans, which look for the few values a rewrite is about, need not walk the
//! body: they read the occurrences of those values. The postings follow the body the allocator asks about:
//! asked of a body, they compare each block's instructions by identity with the body they were made of and
//! redo only the blocks that differ (all of them, for another function).

use std::cell::RefCell;
use std::sync::Arc;

use crate::model::ir::{Loc, Space};
use crate::model::lir::{Insn, LirBody};
use crate::support::hash::HashMap;

/// An instruction by its block's position in the body and its own in the block.
pub type At = (u32, u32);

/// The instructions that define and read each value, in body order: one entry for each time the
/// instruction's `defines` or `uses` names it.
#[derive(Debug, Default, PartialEq)]
pub struct Postings {
    defs: HashMap<u32, Vec<At>>,
    uses: HashMap<u32, Vec<At>>,
    /// The instructions that `require` the value be in a register.
    needs: HashMap<u32, Vec<At>>,
    /// For each block, the positions of its instructions with a frame cell among their operands.
    frames: Vec<Vec<u32>>,
}

impl Postings {
    /// Every occurrence in `body`, worked out whole.
    pub fn of(body: &LirBody) -> Self {
        let mut found = Self::default();
        for (block, one) in body.blocks.iter().enumerate() {
            found.add(block as u32, &one.insns);
        }
        found
    }

    pub fn defs(
        &self,
        value: u32,
    ) -> &[At] {
        self.defs.get(&value).map_or(&[], Vec::as_slice)
    }

    pub fn uses(
        &self,
        value: u32,
    ) -> &[At] {
        self.uses.get(&value).map_or(&[], Vec::as_slice)
    }

    /// The positions in block `block` of the instructions that read or write a frame cell.
    pub fn frames(
        &self,
        block: usize,
    ) -> &[u32] {
        self.frames.get(block).map_or(&[], Vec::as_slice)
    }

    pub fn needs(
        &self,
        value: u32,
    ) -> &[At] {
        self.needs.get(&value).map_or(&[], Vec::as_slice)
    }

    /// The occurrences of the instructions of `insns`, block `block`, appended: that block is the last made.
    fn add(
        &mut self,
        block: u32,
        insns: &[Arc<Insn>],
    ) {
        let framed = |one: &Insn| {
            one.what
                .as_ref()
                .is_some_and(
                    |what| what.dests
                        .iter()
                        .chain(&what.sources)
                        .any(
                            |operand| matches!(
                                operand,
                                Loc::Mem(cell) if cell.addr.is_some_and(|addr| addr.space == Space::Frame)
                            ),
                        ),
                )
        };
        let list = insns.iter().enumerate().filter(|(_, one)| framed(one)).map(|(at, _)| at as u32).collect();
        if self.frames.len() <= block as usize {
            self.frames.resize(block as usize + 1, Vec::new());
        }
        self.frames[block as usize] = list;
        for (at, one) in insns.iter().enumerate() {
            for value in &one.defines {
                self.defs.entry(*value).or_default().push((block, at as u32));
            }
            for value in &one.uses {
                self.uses.entry(*value).or_default().push((block, at as u32));
            }
            for (held, _) in &one.requires {
                self.needs.entry(held.value).or_default().push((block, at as u32));
            }
        }
    }

    /// Block `block` as `old` was, as `new` is.
    fn replaced(
        &mut self,
        block: u32,
        old: &[Arc<Insn>],
        new: &[Arc<Insn>],
    ) {
        let mut touched: Vec<u32> = old
            .iter()
            .chain(new)
            .flat_map(|one| {
                one.defines.iter().chain(&one.uses).copied().chain(one.requires.iter().map(|(held, _)| held.value))
            })
            .collect();
        touched.sort_unstable();
        touched.dedup();
        let mut fresh = Postings::default();
        fresh.add(block, new);
        if self.frames.len() <= block as usize {
            self.frames.resize(block as usize + 1, Vec::new());
        }
        self.frames[block as usize] = fresh.frames.pop().unwrap_or_default();
        for value in touched {
            for (mine, theirs) in
                [(&mut self.defs, &fresh.defs), (&mut self.uses, &fresh.uses), (&mut self.needs, &fresh.needs)]
            {
                let list = mine.entry(value).or_default();
                let from = list.partition_point(|at| at.0 < block);
                let to = list.partition_point(|at| at.0 <= block);
                list.splice(from..to, theirs.get(&value).into_iter().flatten().copied());
                if list.is_empty() {
                    mine.remove(&value);
                }
            }
        }
    }
}

/// The body the postings were made of, held so that an instruction's address is not reused while it is
/// compared with.
struct Followed {
    blocks: Vec<(i64, Vec<Arc<Insn>>)>,
    found: Postings,
}

thread_local! {
    static CURRENT: RefCell<Option<Followed>> = const { RefCell::new(None) };
    static REDONE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blocks this thread has worked the occurrences of again, for a test that a rewrite redoes only
/// the blocks it changed.
pub fn redone() -> usize {
    REDONE.with(std::cell::Cell::get)
}

fn check() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| llrm_support::env_set("LLRM_CHECK_POSTINGS"))
}

/// Whether the postings already are those of `body`: its blocks, each of the same instructions.
fn current(
    held: &Followed,
    body: &LirBody,
) -> bool {
    held.blocks.len() == body.blocks.len()
        && held.blocks.iter().zip(&body.blocks).all(|((at, before), block)| {
            *at == block.at
                && before.len() == block.insns.len()
                && before.iter().zip(&block.insns).all(|(old, new)| Arc::ptr_eq(old, new))
        })
}

/// `read` of the postings of `body`, made to follow it first. A read may ask again of the same body.
pub fn following<R>(
    body: &LirBody,
    read: impl FnOnce(&Postings) -> R,
) -> R {
    let up_to_date =
        CURRENT.with(|current_state| current_state.borrow().as_ref().is_some_and(|held| current(held, body)));
    if !up_to_date {
        CURRENT.with(|state| {
            let mut state = state.borrow_mut();
            let same_shape = state
                .as_ref()
                .is_some_and(
                    |held| held.blocks.len() == body.blocks.len()
                        && held.blocks.iter().zip(&body.blocks).all(|((at, _), block)| *at == block.at),
                );
            if !same_shape {
                REDONE.with(|redone| redone.set(redone.get() + body.blocks.len()));
                *state = Some(Followed {
                    blocks: body.blocks.iter().map(|block| (block.at, block.insns.to_vec())).collect(),
                    found: Postings::of(body),
                });
            } else {
                let held = state.as_mut().expect("checked");
                for (block, (one, (_, before))) in body.blocks.iter().zip(&mut held.blocks).enumerate() {
                    let unchanged = before.len() == one.insns.len()
                        && before.iter().zip(&one.insns).all(|(old, new)| Arc::ptr_eq(old, new));
                    if unchanged {
                        continue;
                    }
                    REDONE.with(|redone| redone.set(redone.get() + 1));
                    held.found.replaced(block as u32, before, &one.insns);
                    *before = one.insns.to_vec();
                }
            }
            if check() {
                let held = state.as_ref().expect("made");
                assert!(
                    held.found == Postings::of(body),
                    "{}: the postings that followed the body differ from working them out",
                    body.name
                );
            }
        });
    }
    CURRENT.with(|state| read(&state.borrow().as_ref().expect("made").found))
}
