//! Moves a load down to just before its one reader when that raises the
//! pressure of no register class and lowers it for some.
//!
//! # How it works
//! A value defined late is live a short while; LLVM's scheduler works bottom-up
//! for the same reason. The sink runs before allocation, when each value's
//! class is what [`crate::backend::regclass`] says confines it (a byte
//! register, an address base or index, a selector) or the x87 stack, and loads
//! of different classes differ in nothing but those sets.
//!
//! A call is no float-region boundary when x87 values live in registers the
//! allocator spills: a float argument is read after the call
//! instead of spilled across it.
//!
//! Within a block, from the last instruction up, a load `d = load [a]` is a
//! candidate when it is plain (a move or float load of a cell, not volatile, no
//! group, no fixed registers or clobbers, not a call), `d` is defined once and
//! is not live out of the block, and exactly one later instruction `u` reads
//! `d`, not at once after the load and not part of a group. Everything between
//! the load and `u` must be able to stay ahead of it: no float-region boundary,
//! no volatile access, no barrier, nothing that may write the loaded cell
//! (`may_write`), nothing that redefines `d` or an address operand of the load.
//! Where all that lies between are loads feeding `u` itself, the order is
//! immaterial and nothing moves.
//!
//! Moving the load takes `d` out of its class over the stretch, and keeps each
//! address operand that dies at the load live until `u`. For a class `S` the
//! count of live values confined within `S` changes by `-[d in S] + #{address
//! operands in S that die at the load}`:
//!
//! ```text
//!  before                      after
//!  d = load [a]    a dies      x = ...
//!  x = ...                     y = ...
//!  y = ...                     d = load [a]   (at u's position)
//!  u = op d, x                 u = op d, x
//! ```
//!
//! The load moves where no class rises and at least one falls; the sets tried
//! are the general set, the x87 stack and the classes of `d` and of those dying
//! address operands. The moved load takes `u`'s `at`, so a block's instructions
//! stay in order by it.
//!
//! DEVIATE P1 (#2903): no pin counterpart. LLVM orders loads by the scheduler;
//! exit: the pre-RA machine scheduler translated (target 2026-11-20), then this
//! file is deleted.
//!
//! # Example
//! None: no test fixture exercises `sunk` on a body whose LIR it prints, and
//! the sketch above is not program output. Read `sunk` for the candidate test.
//!
//! # Runs
//! A machine phase after `phielim` and before `floatfold`
//! (`flow::machine_with`), in every machine, before values are placed.
//!
//! # Needs
//! Classes from [`crate::backend::regclass`], liveness from
//! [`crate::backend::allocate`] and the store check `may_write` of
//! [`crate::backend::storedhomes`].

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use llrm_lir::registers::RegId;

use crate::backend::allocate::{LiveAt, live_rows};
use crate::backend::classes::RegisterClasses;
use crate::backend::regclass;
use crate::backend::storedhomes::may_write;
use crate::backend::target::Segments;
use crate::model::ir::{Loc, Mem, Operation};
use crate::model::lir::{Block, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::IndexMap;

pub struct PressureSink {
    pub segments: Segments,
    pub classes: Rc<RegisterClasses>,
}

impl LIRTransform for PressureSink {
    fn class_name(&self) -> &'static str {
        "PressureSink"
    }

    fn name(&self) -> &str {
        "pressuresink"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(sunk(&body, &self.segments, &self.classes))
    }
}

/// What confines a value: the x87 stack, a set of registers, or any general
/// register.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Class {
    Stack,
    Within(BTreeSet<RegId>),
    Any,
}

impl Class {
    /// Whether a value of class `value` is counted in the class `set` stands
    /// for.
    fn counts_in(
        &self,
        set: &Class,
    ) -> bool {
        match (self, set) {
            (Class::Stack, Class::Stack) => true,
            (Class::Stack, _) | (_, Class::Stack) => false,
            (_, Class::Any) => true,
            (Class::Within(value), Class::Within(set)) => value.is_subset(set),
            (Class::Any, Class::Within(_)) => false,
        }
    }
}

/// The cell a plain load reads, and the value it defines.
fn plain_load(one: &Insn) -> Option<(u32, &Mem)> {
    let what = one.what.as_ref()?;
    let ([Loc::Held(into)], [Loc::Mem(cell)]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    (matches!(what.op, Operation::Move | Operation::FloatLoad)
        && one.defines == [into.value]
        && !one.volatile()
        && one.group.is_none()
        && one.clobbers.is_empty()
        && one.delivers.is_empty()
        && one.requires.is_empty()
        && one.call.is_none())
    .then_some((into.value, cell))
}

/// The liveness rows of `body`, worked out when first asked.
struct Lazy<'a> {
    body: &'a LirBody,
    rows: std::cell::OnceCell<crate::backend::allocate::LiveRows>,
}

impl LiveAt for Lazy<'_> {
    fn live_in(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.rows.get_or_init(|| live_rows(self.body)).live_in(block, value)
    }

    fn live_out(
        &self,
        block: i64,
        value: u32,
    ) -> bool {
        self.rows.get_or_init(|| live_rows(self.body)).live_out(block, value)
    }
}

/// `body` with each plain load that can go and should made just before its one
/// reader.
pub fn sunk(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
) -> LirBody {
    // Asked of the first load that passes the cheap tests, not of every body:
    // most bodies have none, and the classes and the liveness cost 3.2 M of
    // d_faces's 4.4 M here.
    let confined = std::cell::OnceCell::new();
    let class_of = |value: u32, wide: bool| {
        let confined = confined.get_or_init(|| regclass::classes(body, &BTreeSet::new(), segments, registers));
        if wide { Class::Stack } else { confined.get(&value).map_or(Class::Any, |set| Class::Within(set.clone())) }
    };
    // Rows of bits, asked by membership: a set of values for every block is
    // blocks times values, and the walk asks for a few.
    let live_out = Lazy { body, rows: std::cell::OnceCell::new() };
    let mut widths: IndexMap<u32, u32> = IndexMap::default();
    let mut defined: IndexMap<u32, usize> = IndexMap::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for value in &one.defines {
            *defined.entry(*value).or_default() += 1;
        }
        if let Some(what) = &one.what {
            for place in what.dests.iter().chain(&what.sources) {
                if let Loc::Held(held) = place {
                    widths.insert(held.value, held.width);
                }
            }
        }
    }
    let stack = |value: u32| widths.get(&value) == Some(&10);
    let mut changed = false;
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
            // From the last: a load moved down lands past the positions already
            // looked at, so each is looked at once.
            for position in (0..insns.len()).rev() {
                let Some(target) = _reader(&insns, position, body, block.at, &live_out, &defined, &class_of, &stack)
                else {
                    continue;
                };
                // It takes its reader's position: instructions of one block
                // stay in order by it, as a compare and its
                // branches share one.
                let mut load = (*insns.remove(position)).clone();
                load.at = insns[target - 1].at;
                insns.insert(target - 1, Arc::new(load));
                changed = true;
            }
            block.with_insns(insns)
        })
        .collect();
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// Where the load at `at` should go, as the index of its reader, if it should
/// move.
fn _reader(
    insns: &[Arc<Insn>],
    at: usize,
    body: &LirBody,
    block: i64,
    live: &impl LiveAt,
    defined: &IndexMap<u32, usize>,
    class_of: &dyn Fn(u32, bool) -> Class,
    stack: &dyn Fn(u32) -> bool,
) -> Option<usize> {
    let regs = body.regs();
    let (value, cell) = plain_load(&insns[at])?;
    if defined.get(&value) != Some(&1) || live.live_out(block, value) {
        return None;
    }
    let mut readers = insns.iter().enumerate().skip(at + 1).filter(|(_, one)| one.uses.contains(&value));
    let (reader, _) = readers.next()?;
    if readers.next().is_some() || insns[reader].group.is_some() || reader <= at + 1 {
        return None;
    }
    // Operands of one reader made one after another are in no order that
    // matters.
    if insns[at + 1..reader]
        .iter()
        .all(|between| plain_load(between).is_some_and(|(other, _)| insns[reader].uses.contains(&other)))
    {
        return None;
    }
    let addresses: BTreeSet<u32> = insns[at].uses.iter().copied().collect();
    for between in &insns[at + 1..reader] {
        let Some(what) = &between.what else { return None };
        if (crate::backend::floatregions::boundary(regs, between) && what.op != Operation::Call)
            || between.volatile()
            || may_write(between, cell, body.sealed_arguments)
            || between.defines.iter().any(|defined| addresses.contains(defined) || *defined == value)
            || what.op == Operation::Barrier
        {
            return None;
        }
    }
    // Each address operand that dies at the load stays live to the reader: not
    // one read at or after it.
    let dying: Vec<u32> = addresses
        .iter()
        .copied()
        .filter(|operand| {
            !live.live_out(block, *operand) && !insns[reader..].iter().any(|one| one.uses.contains(operand))
        })
        .collect();
    let mut sets: Vec<Class> = vec![Class::Any, Class::Stack];
    for value in dying.iter().copied().chain([value]) {
        let class = class_of(value, stack(value));
        if !sets.contains(&class) {
            sets.push(class);
        }
    }
    let (mut falls, mut rises) = (false, false);
    for set in &sets {
        let removed = i64::from(class_of(value, stack(value)).counts_in(set));
        let kept = dying.iter().filter(|operand| class_of(**operand, stack(**operand)).counts_in(set)).count() as i64;
        match kept - removed {
            change if change > 0 => rises = true,
            change if change < 0 => falls = true,
            _ => {}
        }
    }
    (falls && !rises).then_some(reader)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::model::ir::{Held, Semantics};
    use crate::model::lir::LirBlock;

    /// The sink built the liveness rows and the register classes of every body
    /// before looking for a load (d_faces -O2: 3.2 M of its 4.4 M, 7
    /// bodies, most with no load that has one later reader). A body with no
    /// such load asks for neither. Counted, not timed.
    #[test]
    fn test_a_body_with_no_load_to_move_builds_no_liveness() {
        let held = |value: u32| Loc::Held(Held { value, width: 2 });
        let add = |at: i64, result: u32| {
            let what = Semantics {
                name: Some("add".into()),
                dests: vec![held(result)],
                sources: vec![held(1), held(2)],
                ..Semantics::new(Operation::Binary)
            };
            Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![result], vec![1, 2]))
        };
        let body = LirBody::new(
            "no loads",
            0,
            vec![LirBlock::new(0, vec![add(0, 10), add(1, 11), add(2, 12)]).into()],
            IndexMap::default(),
            IndexMap::default(),
        );
        let before = llrm_mir::counts::get(llrm_mir::counts::Event::LiveRows);
        let out = sunk(&body, &crate::backend::target::BUILT_IN, &crate::backend::classes::RegisterClasses::m16());
        let built = llrm_mir::counts::get(llrm_mir::counts::Event::LiveRows) - before;
        assert_eq!(built, 0, "liveness rows built for a body with no load");
        assert_eq!(out.insns().len(), 3);
    }
}
