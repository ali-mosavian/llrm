//! `removeRedundantDbgLocs`: drops the locations a later one makes useless.
//!
//! A backward scan drops a location whose bytes a later one in the same
//! unbroken run of locations covers; a forward scan drops one that says what
//! the variable already is; and in the entry block a location of nothing
//! before any real one goes (SelectionDAG hoists argument values over them).

use llrm_mir::{BlockId, ConstantKind, Context, Function, InstId, MetadataId, Module, Operand};

use super::{
    DebugVariable, Fragment, FunctionVarLocsBuilder, InsertPt, VarLocInfo, fragment_of, variable_size_in_bits,
};
use crate::support::hash::{HashMap, HashSet};

/// Variables above this many bytes are not scanned for covered bytes
/// (`MaxSizeBytes`): the bit vector would cost more than it saves.
const MAX_SIZE_BYTES: u64 = 2048;

/// The wedge positions of a block in order: for each instruction the
/// records standing before it, then the instruction itself.
fn positions(
    function: &Function,
    block: BlockId,
    slots: &dyn Fn(InstId) -> u32,
) -> Vec<InsertPt> {
    function.iter_block(block).flat_map(|inst| (0..=slots(inst)).map(move |slot| (inst, slot))).collect()
}

/// `isKillLocation`: the location is nothing, or an `undef` or `poison`.
fn is_kill(
    context: &Context,
    loc: &VarLocInfo,
) -> bool {
    loc.values.is_empty()
        || loc.values.iter().any(|value| match value {
            Operand::Constant(id) => matches!(
                context.get(*id).kind,
                ConstantKind::Undef | ConstantKind::Poison
            ),
            _ => false,
        })
}

/// `removeRedundantDbgLocsUsingBackwardScan`.
fn backward(
    module: &Module,
    builder: &mut FunctionVarLocsBuilder,
    order: &[InsertPt],
) -> bool {
    let mut changed = false;
    let mut defined: HashMap<MetadataId, Vec<bool>> = HashMap::default();
    let mut last = None;
    for &at in order.iter().rev() {
        // A run of consecutive locations ends at an instruction.
        if last != Some(at.0) {
            defined.clear();
            last = Some(at.0);
        }
        let Some(locs) = builder.wedge(at).map(<[VarLocInfo]>::to_vec) else { continue };
        let mut kept: Vec<VarLocInfo> = Vec::new();
        let mut changed_here = false;
        for loc in locs.iter().rev() {
            let aggregate = builder.variable(loc.variable).variable;
            let bits = variable_size_in_bits(module, aggregate).unwrap_or(0);
            let bytes = bits.div_ceil(8);
            if bytes == 0 || bytes > MAX_SIZE_BYTES {
                kept.push(loc.clone());
                continue;
            }
            let first = !defined.contains_key(&aggregate);
            let covered = defined.entry(aggregate).or_insert_with(|| vec![false; bytes as usize]);
            let fragment = fragment_of(&loc.expression).unwrap_or(Fragment { offset: 0, size: bits });
            let invalid = fragment.offset + fragment.size > bits;
            let (start, end) = ((fragment.offset / 8) as usize, (fragment.offset + fragment.size).div_ceil(8) as usize);
            if first || invalid || covered[start..end.min(covered.len())].iter().any(|&one| !one) {
                if !invalid {
                    covered[start..end].fill(true);
                }
                kept.push(loc.clone());
                continue;
            }
            changed_here = true;
        }
        if changed_here {
            kept.reverse();
            builder.set_wedge(at, kept);
            changed = true;
        }
    }
    changed
}

/// `removeUndefDbgLocsFromEntryBlock`.
fn undefs_in_entry(
    context: &Context,
    builder: &mut FunctionVarLocsBuilder,
    order: &[InsertPt],
) -> bool {
    let mut changed = false;
    let mut with_def: HashMap<MetadataId, HashSet<Fragment>> = HashMap::default();
    for &at in order {
        let Some(locs) = builder.wedge(at).map(<[VarLocInfo]>::to_vec) else { continue };
        let mut kept = Vec::new();
        let mut changed_here = false;
        for loc in locs {
            let variable: DebugVariable = *builder.variable(loc.variable);
            let fragment = variable.fragment_or_default();
            let defined = with_def.get(&variable.variable).is_some_and(|set| {
                set.iter()
                    .any(|one| one.offset < fragment.offset + fragment.size && fragment.offset < one.offset + one.size)
            });
            if is_kill(context, &loc) && !defined {
                changed_here = true;
                continue;
            }
            with_def.entry(variable.variable).or_default().insert(fragment);
            kept.push(loc);
        }
        if changed_here {
            builder.set_wedge(at, kept);
            changed = true;
        }
    }
    changed
}

/// `removeRedundantDbgLocsUsingForwardScan`.
fn forward(
    builder: &mut FunctionVarLocsBuilder,
    order: &[InsertPt],
) -> bool {
    let mut changed = false;
    let mut said: HashMap<MetadataId, (Vec<Operand>, Vec<llrm_mir::DwOp>)> = HashMap::default();
    for &at in order {
        let Some(locs) = builder.wedge(at).map(<[VarLocInfo]>::to_vec) else { continue };
        let mut kept = Vec::new();
        let mut changed_here = false;
        for loc in locs {
            let key = builder.variable(loc.variable).variable;
            let now = (loc.values.clone(), loc.expression.clone());
            if said.get(&key) == Some(&now) {
                changed_here = true;
                continue;
            }
            said.insert(key, now);
            kept.push(loc);
        }
        if changed_here {
            builder.set_wedge(at, kept);
            changed = true;
        }
    }
    changed
}

/// `removeRedundantDbgLocs` over every block.
///
/// Complexity: linear in the locations; a variable's byte vector is at most
/// [`MAX_SIZE_BYTES`] long.
pub(super) fn run(
    module: &Module,
    function: &Function,
    builder: &mut FunctionVarLocsBuilder,
    slots: &dyn Fn(InstId) -> u32,
) {
    for &block in function.layout() {
        let order = positions(function, block, slots);
        backward(module, builder, &order);
        if Some(block) == function.entry() {
            undefs_in_entry(&module.context, builder, &order);
        }
        forward(builder, &order);
    }
}
