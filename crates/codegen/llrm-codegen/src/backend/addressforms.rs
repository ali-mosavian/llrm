//! Widens word definitions so a 67h address can take them as dword
//! components.
//!
//! # How it works
//! The selector folds `base + index*scale` of word values into one
//! 32-bit, address-size-prefixed form such as `[ebx+esi*4]` (`widened` in
//! `isel.rs`). That form reads whole dwords, so each word value it names
//! must be zero above bit 15. `promote` makes it so at the definition and
//! adds no register: the value keeps its one definition and its live range.
//!
//! Two definitions can be widened. A plain `mov r16,m16` or `mov r16,r16`
//! becomes `movzx r32,...`. A far-pointer load (`les`, `lfs`, `lgs`) must
//! keep its form, so its offset goes to a fresh word temporary and a
//! `movzx` into the original value follows it. Any other definition is an
//! error: the selector only marks values that are one of these.
//!
//! ```text
//!  plain load                far-pointer load
//!  v1:2 = mov m16            v1:2, sel = les m32
//!     |                         |
//!     v                         v
//!  v1:4 = movzx m16          t:2, sel = les m32
//!                            v1:4 = movzx t:2
//! ```
//!
//! Definitions are rewritten from the last in each block to the first, so
//! inserting a follower does not move one still waiting.
//!
//! # Example
//! No honest example: no dump or test prints the body before and after
//! `promote`. `isel_tests.rs`
//! `test_a_non_negative_typed_index_is_scaled_in_the_67h_form` asserts the
//! `movzx` it makes and the `[ebx+eax*2]` form that reads it.
//!
//! # Used by
//! `isel.rs`, after a function is selected, for the values the selector
//! marked; with none marked the blocks pass through unchanged.

use std::collections::BTreeSet;
use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::model::ir::{self, Loc, Operation, Semantics};
use crate::model::lir::{self, Insn};
use crate::support::hash::{IndexMap, IndexSet};

/// Make selected word definitions usable as dword address components.
///
/// A plain word load can directly select `movzx r32,m16`.  A complete far
/// pointer load must still use LES/LFS/LGS, so its offset is first delivered
/// to a short-lived temporary and then zero-extended into the original SSA
/// value.  In both cases the promoted value has one definition and remains
/// the same live range for later low-word uses.
pub fn promote(
    regs: Regs,
    blocks: &IndexMap<i64, Vec<Arc<Insn>>>,
    values: &BTreeSet<u32>,
    fresh: &mut dyn FnMut() -> u32,
) -> Result<IndexMap<i64, Vec<Arc<Insn>>>, String> {
    if values.is_empty() {
        return Ok(blocks.clone());
    }
    let mut definitions: IndexMap<u32, (i64, usize)> = IndexMap::default();
    for (&at, insns) in blocks {
        for (index, one) in insns.iter().enumerate() {
            for &value in &one.defines {
                if values.contains(&value) {
                    definitions.insert(value, (at, index));
                }
            }
        }
    }
    if definitions.keys().copied().collect::<BTreeSet<u32>>() != *values {
        let missing = values.iter().copied().filter(|value| !definitions.contains_key(value)).collect::<Vec<u32>>();
        return Err(format!("secondary address values have no definition: {missing:?}"));
    }
    let mut out: IndexMap<i64, Vec<Arc<Insn>>> = blocks.clone();
    // Work backwards within each block so inserting a follower cannot move a
    // definition still waiting to be rewritten.
    let mut ordered = definitions.into_iter().collect::<Vec<_>>();
    ordered.sort_by_key(|&(_, (at, index))| (at, -(index as i64)));
    for (value, (at, index)) in ordered {
        let one = Arc::clone(&out[&at][index]);
        let Some(what) = &one.what else {
            return Err(format!("value#{value} has no selected definition to promote"));
        };
        let mut destinations = what.dests.clone();
        let position = destinations
            .iter()
            .position(
                |destination| matches!(
                    destination,
                    Loc::Held(destination) if destination.value == value && destination.width == 2
                ),
            );
        // A parameter the convention delivers in a register: the entry's
        // marker defines it, with no destination of its own. The register
        // delivers a fresh word, and the `movzx` into the value follows.
        let delivered = one.delivers.iter().position(|(held, _)| held.value == value && held.width == 2);
        if let (None, Some(slot)) = (position, delivered) {
            let temporary = fresh();
            let rename = |found: u32| if found == value { temporary } else { found };
            let mut leader = (*one).clone();
            leader.delivers[slot].0.value = temporary;
            leader.defines = one.defines.iter().map(|&found| rename(found)).collect();
            leader.widths = one.widths.iter().map(|&(found, width)| (rename(found), width)).collect();
            let follower = extension(&one, value, temporary);
            out[&at].splice(index..=index, [Arc::new(leader), Arc::new(follower)]);
            continue;
        }
        let Some(position) = position else {
            return Err(format!("value#{value} has no word destination to promote"));
        };
        if what.op == Operation::Move
            && what.name.as_deref() == Some("mov")
            && destinations.len() == 1
            && what.sources.len() == 1
            && match &what.sources[0] {
                Loc::Mem(source) => source.width == 2,
                Loc::Held(source) => source.width == 2,
                _ => false,
            }
        {
            destinations[0] = Loc::Held(ir::Held { value, width: 4 });
            let mut changed = (*one).clone();
            changed.what = Some(Semantics {
                op: Operation::Extend,
                name: Some("movzx".into()),
                dests: destinations,
                ..what.clone()
            });
            changed.widths =
                one.widths.iter().copied().chain([(value, 4)]).collect::<IndexSet<_>>().into_iter().collect();
            out[&at][index] = Arc::new(changed);
            continue;
        }
        if what.op != Operation::Move
            || !what.name.as_deref().is_some_and(|form| regs.loads_a_selector(form))
            || position != 0
        {
            let spelled = match what.name.as_deref() {
                Some(name) if !name.is_empty() => name.to_owned(),
                _ => what.op.to_string(),
            };
            return Err(format!("value#{value} cannot be promoted from {spelled}"));
        }
        let temporary = fresh();
        destinations[0] = Loc::Held(ir::Held { value: temporary, width: 2 });
        let mut leader = (*one).clone();
        leader.what = Some(Semantics { dests: destinations, ..what.clone() });
        leader.defines = one.defines.iter().map(|&found| if found == value { temporary } else { found }).collect();
        leader.widths =
            one.widths.iter().map(|&(found, width)| (if found == value { temporary } else { found }, width)).collect();
        let follower = extension(&one, value, temporary);
        out[&at].splice(index..=index, [Arc::new(leader), Arc::new(follower)]);
    }
    Ok(out)
}

/// After `one`: `value` as the zero extension of the word `temporary`.
fn extension(
    one: &Arc<Insn>,
    value: u32,
    temporary: u32,
) -> Insn {
    let mut follower = (*lir::anchor(Arc::clone(one))).clone();
    follower.what = Some(Semantics {
        name: Some("movzx".into()),
        dests: vec![Loc::Held(ir::Held { value, width: 4 })],
        sources: vec![Loc::Held(ir::Held { value: temporary, width: 2 })],
        ..Semantics::new(Operation::Extend)
    });
    follower.defines = vec![value];
    follower.uses = vec![temporary];
    follower.widths = vec![(temporary, 2), (value, 4)];
    follower.call = None;
    follower
}
