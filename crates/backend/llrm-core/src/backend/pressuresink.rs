//! A load made where its one reader is, when that raises the pressure of no register class: LLVM's scheduler works
//! bottom-up for the same reason, a value defined late is live a short while. Before allocation, where each value's
//! class is what `regclass` says confines it (a byte register, an address base or index, a selector) or the x87 stack,
//! and the loads of different classes are told apart by nothing but those sets.
//!
//! Moving `d = load [a]` down to its reader `u` takes `d` out of its class over the stretch, and keeps each address
//! operand that died at the load live to `u`. Per class `S` that changes the count of live values confined within `S`
//! by `-[d in S] + #{operands in S that die at the load}`. The load moves where no class rises and one falls.

use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use iced_x86::Register;

use crate::backend::allocate::live;
use crate::backend::classes::RegisterClasses;
use crate::backend::regclass;
use crate::backend::storedhomes::may_write;
use crate::backend::target::Segments;
use crate::model::ir::{Loc, Mem, Operation};
use crate::model::lir::{Insn, LirBlock, LirBody};
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

/// What confines a value: the x87 stack, a set of registers, or any general register.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Class {
    Stack,
    Within(BTreeSet<Register>),
    Any,
}

impl Class {
    /// Whether a value of class `value` is counted in the class `set` stands for.
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

/// `body` with each plain load that can go and should made just before its one reader.
pub fn sunk(
    body: &LirBody,
    segments: &Segments,
    registers: &RegisterClasses,
) -> LirBody {
    let confined = regclass::classes(body, &BTreeSet::new(), segments, registers);
    let class_of = |value: u32, wide: bool| {
        if wide { Class::Stack } else { confined.get(&value).map_or(Class::Any, |set| Class::Within(set.clone())) }
    };
    let (_, live_out) = live(body);
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
    let blocks: Vec<LirBlock> = body
        .blocks
        .iter()
        .map(|block| {
            let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
            // From the last: a load moved down lands past the positions already looked at, so each is looked at once.
            for position in (0..insns.len()).rev() {
                let Some(target) = _reader(&insns, position, body, &live_out[&block.at], &defined, &class_of, &stack)
                else {
                    continue;
                };
                // It takes its reader's position: instructions of one block stay in order by it, as a compare and its
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

/// Where the load at `at` should go, as the index of its reader, if it should move.
fn _reader(
    insns: &[Arc<Insn>],
    at: usize,
    body: &LirBody,
    live_out: &BTreeSet<u32>,
    defined: &IndexMap<u32, usize>,
    class_of: &dyn Fn(u32, bool) -> Class,
    stack: &dyn Fn(u32) -> bool,
) -> Option<usize> {
    let (value, cell) = plain_load(&insns[at])?;
    if defined.get(&value) != Some(&1) || live_out.contains(&value) {
        return None;
    }
    let mut readers = insns.iter().enumerate().skip(at + 1).filter(|(_, one)| one.uses.contains(&value));
    let (reader, _) = readers.next()?;
    if readers.next().is_some() || insns[reader].group.is_some() || reader <= at + 1 {
        return None;
    }
    // Operands of one reader made one after another are in no order that matters.
    if insns[at + 1..reader]
        .iter()
        .all(|between| plain_load(between).is_some_and(|(other, _)| insns[reader].uses.contains(&other)))
    {
        return None;
    }
    let addresses: BTreeSet<u32> = insns[at].uses.iter().copied().collect();
    for between in &insns[at + 1..reader] {
        let Some(what) = &between.what else { return None };
        if crate::backend::floatregions::boundary(between)
            || between.volatile()
            || may_write(between, cell, body.sealed_arguments)
            || between.defines.iter().any(|defined| addresses.contains(defined) || *defined == value)
            || what.op == Operation::Barrier
        {
            return None;
        }
    }
    // Each address operand that dies at the load stays live to the reader: not one read at or after it.
    let dying: Vec<u32> = addresses
        .iter()
        .copied()
        .filter(|operand| !live_out.contains(operand) && !insns[reader..].iter().any(|one| one.uses.contains(operand)))
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
