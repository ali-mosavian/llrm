//! A float load that only one arithmetic instruction reads is that
//! instruction's memory operand: `fld [m]; faddp` is `fadd [m]`.
//!
//! x87 arithmetic has a memory form for a float or double (`fadd qword ptr
//! [m]`), and LLVM folds a single-use load into
//! it (`X86InstrInfo::foldMemoryOperand`); gcc's output has it everywhere. The
//! allocator folds only a spilled value's reload (`floatassign::fused`). This
//! is the same fold for any load, before the values are placed: it saves the
//! load and the x87 register the loaded value held. Sound when nothing between
//! the load and its reader may write what it reads.

use std::sync::Arc;

use crate::backend::floatalloc::{_loads_memory, _two_values, cell_of, name_is, semantics};
use crate::backend::storedhomes::may_write;
use crate::model::ir::{Loc, Operation};
use crate::model::lir::{Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet, IndexSet};

pub struct FloatFold;

impl LIRTransform for FloatFold {
    fn class_name(&self) -> &'static str {
        "FloatFold"
    }

    fn name(&self) -> &str {
        "floatfold"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(folded(&body))
    }
}

/// `body` with each float load its one reader takes from memory read there.
pub fn folded(body: &LirBody) -> LirBody {
    let mut uses: HashMap<u32, usize> = HashMap::default();
    let mut floating: HashSet<u32> = HashSet::default();
    for one in body.blocks.iter().flat_map(|block| &block.insns) {
        for (arg, read) in one.what.iter().flat_map(|what| {
            what.sources.iter().map(|arg| (arg, true)).chain(what.dests.iter().map(|arg| (arg, false)))
        }) {
            if let Loc::Held(held) = arg
                && held.width == 10
            {
                floating.insert(held.value);
                *uses.entry(held.value).or_default() += usize::from(read);
            }
        }
    }
    let mut blocks = Vec::new();
    let mut changed = false;
    for block in &body.blocks {
        let mut insns: Vec<Arc<Insn>> = block.insns.to_vec();
        let mut at = 0;
        while at < insns.len() {
            let one = Arc::clone(&insns[at]);
            let Some(what) = one.what.as_ref().filter(|what| what.op == Operation::FloatArith && what.dests.len() == 1)
            else {
                at += 1;
                continue;
            };
            let Some((name, left, right)) = _two_values(Some(what)) else {
                at += 1;
                continue;
            };
            if left.value == right.value {
                at += 1;
                continue;
            }
            // The right operand's load first: the left is on the stack top, as
            // `fadd st, [m]` has it.
            let found = [(right.value, false, left), (left.value, true, right)]
                .into_iter()
                .find_map(
                    |(value, cell_is_left, other)| {
                        if uses.get(&value) != Some(&1) {
                            return None;
                        }
                        let home = (0..at)
                            .rev()
                            .find(
                                |&back| insns[back].what.as_ref().is_some_and(|made| {
                                    made.dests.iter().any(|d| matches!(d, Loc::Held(held) if held.value == value))
                                }),
                            )?;
                        let load = &insns[home];
                        if !_loads_memory(load.what.as_ref())
                            || load.volatile
                            || !load.clobbers.is_empty()
                            || load.group.is_some()
                        {
                            return None;
                        }
                        let cell = cell_of(load);
                        if insns[home + 1..at].iter().any(|between| {
                            between.volatile
                                || between.call.is_some()
                                || may_write(between, cell, false)
                                || between.what.as_ref().is_none_or(|w| {
                                    matches!(w.op, Operation::Call | Operation::Barrier | Operation::Escape)
                                })
                        }) {
                            return None;
                        }
                        let operation = memory_form(&name, cell_is_left, load)?;
                        Some((home, value, operation, other))
                    },
                );
            let Some((home, value, operation, other)) = found else {
                at += 1;
                continue;
            };
            let load = Arc::clone(&insns[home]);
            let cell = Loc::Mem(cell_of(&load).clone());
            let mut fused = (*one).clone();
            fused.what =
                Some(semantics(Operation::FloatArith, &operation, what.dests.clone(), vec![Loc::Held(other), cell]));
            let foreign =
                |values: &[u32]| values.iter().copied().filter(|one| !floating.contains(one)).collect::<Vec<u32>>();
            fused.uses = one
                .uses
                .iter()
                .copied()
                .filter(|one| *one != value)
                .chain(foreign(&load.uses))
                .collect::<IndexSet<u32>>()
                .into_iter()
                .collect();
            fused.widths = one
                .widths
                .iter()
                .chain(&load.widths)
                .copied()
                .filter(|(one, _)| *one != value && !(floating.contains(one) && load.defines.contains(one)))
                .collect::<IndexSet<(u32, u32)>>()
                .into_iter()
                .collect();
            fused.requires =
                load.requires.iter().chain(&one.requires).copied().collect::<IndexSet<_>>().into_iter().collect();
            fused.symbol = if one.covers.is_some_and(|(start, end)| start != end) { Some(false) } else { one.symbol };
            insns[at] = Arc::new(fused);
            insns.remove(home);
            changed = true;
            // `at` now names the instruction after the fused one's old place.
        }
        blocks.push(block.with_insns(insns));
    }
    if changed { body.with_blocks(blocks) } else { body.clone() }
}

/// The memory form of `name` reading `load`'s cell: `fadd qword ptr [m]`,
/// `fsubr dword ptr [m]`; none for an integer or a cell that is not a float or
/// double. Where the cell is the left operand, the operation reversed.
fn memory_form(
    name: &str,
    cell_is_left: bool,
    load: &Insn,
) -> Option<String> {
    let reversed = |name: &str| -> String {
        match name {
            "fsub" => "fsubr".to_owned(),
            "fdiv" => "fdivr".to_owned(),
            other => other.to_owned(),
        }
    };
    let name = if cell_is_left { reversed(name) } else { name.to_owned() };
    // An integer load stays: `fild` then `fadd` is faster than `fiadd` on the
    // 486, and the other operand may take the memory form instead.
    if name_is(load.what.as_ref()?, "fild") || !matches!(cell_of(load).width, 4 | 8) {
        return None;
    }
    Some(name)
}
