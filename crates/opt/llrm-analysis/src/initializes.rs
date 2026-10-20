//! `initializes(...)` of the pointer parameters of a function: the bytes it
//! writes before anything reads them, as `inferInitializes` states them
//! (FunctionAttrs.cpp:1118,
//! <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/IPO/FunctionAttrs.cpp#L1118>).
//!
//! # How it works
//! The uses of a parameter are walked through constant-offset GEPs
//! (`collectArgumentUsesPerBlock`); each user gets the bytes it writes, reads
//! or clobbers (`getArgumentAccessInfo`). A block's initialized ranges are the
//! intersection of its successors', then, from its last use to its first: a
//! write adds its bytes, a read removes them, an unknown use or a call that
//! also captures the pointer clears them. The entry block's ranges are the
//! attribute. No points-to solve: a use walk and one pass over the blocks.
//!
//! DEVIATE I1: run after the SCC's argument attributes are final
//! (`addArgumentAttrs` runs it per parameter, before the argument graph is
//! settled), so a call inside the SCC sees the callee's final
//! `writeonly`/`nocapture`: the same or more precise ranges.
//! DEVIATE I2: a load or store is "simple" when it is not volatile; MIR has
//! no atomic ordering on them.
//!
//! # Example
//! `fill` writes the first two bytes of `%p` and reads nothing:
//!
//! ```mir
//! define void @fill(ptr %p) {
//! b0:
//!   store i16 1, ptr %p
//!   ret void
//! }
//! ```
//!
//! `%p` gets `initializes((0, 2))`. From unit test
//! `a_store_through_the_parameter_initializes_its_bytes` in
//! `initializes_tests.rs`.

use std::collections::BTreeSet;

use llrm_mir::constantfolding::accumulate_constant_offset;
use llrm_mir::context::{ConstantId, GlobalId};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::facts::Facts;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{BlockId, Function, InstId, Module, Operand, Use};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::types::Type;

/// Byte ranges `[low, high)`, sorted, apart and not touching: LLVM's
/// `ConstantRangeList`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Ranges(Vec<(i64, i64)>);

impl Ranges {
    fn of(ranges: impl IntoIterator<Item = (i64, i64)>) -> Self {
        ranges.into_iter().fold(Self::default(), |all, one| all.union(&Self(vec![one])))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `unionWith`.
    fn union(
        &self,
        other: &Self,
    ) -> Self {
        let mut all: Vec<(i64, i64)> =
            self.0.iter().chain(&other.0).copied().filter(|&(low, high)| low < high).collect();
        all.sort_unstable();
        let mut out: Vec<(i64, i64)> = Vec::with_capacity(all.len());
        for (low, high) in all {
            match out.last_mut() {
                Some(last) if low <= last.1 => last.1 = last.1.max(high),
                _ => out.push((low, high)),
            }
        }
        Self(out)
    }

    /// `intersectWith`.
    fn intersect(
        &self,
        other: &Self,
    ) -> Self {
        let (mut out, mut a, mut b) = (Vec::new(), 0, 0);
        while a < self.0.len() && b < other.0.len() {
            let (low, high) = (self.0[a].0.max(other.0[b].0), self.0[a].1.min(other.0[b].1));
            if low < high {
                out.push((low, high));
            }
            if self.0[a].1 < other.0[b].1 {
                a += 1;
            } else {
                b += 1;
            }
        }
        Self(out)
    }

    /// `subtract`.
    fn subtract(
        &mut self,
        low: i64,
        high: i64,
    ) {
        let mut out = Vec::with_capacity(self.0.len() + 1);
        for &(from, to) in &self.0 {
            if to <= low || high <= from {
                out.push((from, to));
                continue;
            }
            if from < low {
                out.push((from, low));
            }
            if high < to {
                out.push((high, to));
            }
        }
        self.0 = out;
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Kind {
    Write,
    /// A call that writes the argument and also captures it.
    WriteWithSideEffect,
    Read,
    Unknown,
}

struct Access {
    kind: Kind,
    ranges: Ranges,
}

impl Access {
    fn unknown() -> Self {
        Self { kind: Kind::Unknown, ranges: Ranges::default() }
    }

    fn writes(&self) -> bool {
        matches!(self.kind, Kind::Write | Kind::WriteWithSideEffect) && !self.ranges.is_empty()
    }
}

/// `UsesPerBlockInfo`.
#[derive(Default)]
struct BlockUses {
    insts: IdMap<InstId, Access>,
    has_unknown_access: bool,
}

/// `ArgumentUsesSummary`.
#[derive(Default)]
struct Uses {
    any_write: bool,
    write_outside_entry: bool,
    per_block: IdMap<BlockId, BlockUses>,
}

/// The bytes `[offset, offset + size)` as a range, unless it overflows.
fn range_of(
    offset: Option<i64>,
    size: i64,
) -> Option<(i64, i64)> {
    let low = offset?;
    Some((low, low.checked_add(size)?)).filter(|_| size > 0)
}

/// `getArgumentAccessInfo`: what `used`, a use of the argument at constant
/// `offset`, does to the bytes it names.
fn access_info(
    module: &Module,
    layout: &DataLayout,
    function: &Function,
    used: Use,
    offset: Option<i64>,
) -> Access {
    let op = function.instruction(used.user);
    let context = &module.context;
    let ranged = |kind: Kind, range: Option<(i64, i64)>| Access { kind, ranges: Ranges::of(range) };
    // The length of a memory intrinsic, when it is a positive constant.
    let length = |at: usize| {
        llrm_mir::intrinsics::length(context, *op.operands.get(at)?).and_then(|len| i64::try_from(len).ok())
    };
    match &op.opcode {
        Opcode::Store { volatile: false, .. } if used.index == 1 => {
            let size =
                function.operand_type(context, op.operands[0]).map(|ty| layout.store_size(&context.types, ty) as i64);
            ranged(Kind::Write, size.and_then(|size| range_of(offset, size)))
        }
        Opcode::Load { volatile: false, .. } => {
            let size = layout.store_size(&context.types, op.ty) as i64;
            match range_of(offset, size) {
                Some(range) => ranged(Kind::Read, Some(range)),
                None => Access::unknown(),
            }
        }
        Opcode::Call(info) | Opcode::Invoke(info) => {
            let callee = llrm_mir::memory::callee(context, function, used.user);
            let named = callee.and_then(|id| module.global(id).name.as_deref()).and_then(Intrinsic::named);
            let volatile =
                || llrm_mir::intrinsics::length(context, op.operands.get(3).copied().unwrap_or(op.operands[0]));
            match named {
                Some(Intrinsic::MemSet) if volatile() == Some(0) => {
                    ranged(Kind::Write, length(2).and_then(|len| range_of(offset, len)))
                }
                Some(Intrinsic::MemCpy | Intrinsic::MemMove) if volatile() == Some(0) => {
                    let range = length(2).and_then(|len| range_of(offset, len));
                    match used.index {
                        0 => ranged(Kind::Write, range),
                        1 => match range {
                            Some(range) => ranged(Kind::Read, Some(range)),
                            None => Access::unknown(),
                        },
                        _ => Access::unknown(),
                    }
                }
                Some(_) => Access::unknown(),
                None => call_access(module, function, used, offset, info.argument_attrs.len()),
            }
        }
        _ => Access::unknown(),
    }
}

/// An argument of a call: a write of the callee's `initializes` bytes.
fn call_access(
    module: &Module,
    function: &Function,
    used: Use,
    offset: Option<i64>,
    arguments: usize,
) -> Access {
    let index = used.index as usize;
    let op = function.instruction(used.user);
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { return Access::unknown() };
    if index >= arguments || index >= op.operands.len() {
        return Access::unknown();
    }
    let callee = llrm_mir::memory::callee(&module.context, function, used.user)
        .and_then(|id| module.global(id).function())
        .and_then(|one| one.parameter_attrs.get(index));
    let site = info.argument_attrs.get(index).map(Vec::as_slice).unwrap_or_default();
    let byval = |attrs: &[Attribute]| {
        attrs
            .iter()
            .any(
                |one| matches!(
                    one,
                    Attribute::Flag(name) | Attribute::Type(name, _) if name == "byval"
                ),
            )
    };
    if byval(site) || callee.is_some_and(|attrs| byval(attrs)) {
        return Access::unknown();
    }
    // `paramHasAttr(ArgNo, Initializes)`: the site's, else the callee's.
    let stated = [site, callee.map(Vec::as_slice).unwrap_or_default()]
        .into_iter()
        .map(llrm_mir::memory::initializes)
        .find(|ranges| !ranges.is_empty());
    let (Some(stated), Some(base)) = (stated, offset) else { return Access::unknown() };
    // A write when the parameter is writeonly or readnone, and nocapture.
    let facts = [Facts::of(site), callee.map_or_else(|| Facts::of(&[]), |attrs| Facts::of(attrs))];
    let only_writes = facts.iter().any(|one| one.write_only() || one.read_none());
    let no_capture = facts.iter().any(Facts::no_capture);
    let kind = if only_writes && no_capture { Kind::Write } else { Kind::WriteWithSideEffect };
    let shifted = stated.iter().filter_map(|&(low, high)| Some((low.checked_add(base)?, high.checked_add(base)?)));
    Access { kind, ranges: Ranges::of(shifted) }
}

/// `collectArgumentUsesPerBlock`.
fn collect(
    module: &Module,
    layout: &DataLayout,
    function: &Function,
    value: llrm_mir::module::ValueId,
    entry: BlockId,
) -> Uses {
    let space =
        match function.operand_type(&module.context, Operand::Value(value)).map(|ty| module.context.types.get(ty)) {
            Some(Type::Pointer(space)) => *space,
            _ => 0,
        };
    let width = layout.pointer(space).index_bits;
    let mut result = Uses::default();
    // No visited set: phis are not looked through, so there are no cycles.
    let mut work: Vec<(Use, Option<i64>)> = function.users(value).into_iter().map(|used| (*used, Some(0))).collect();
    while let Some((used, offset)) = work.pop() {
        let op = function.instruction(used.user);
        if let Opcode::GetElementPtr { source } = op.opcode {
            let next = offset.and_then(|base| {
                let indices: Option<Vec<ConstantId>> = op.operands[1..]
                    .iter()
                    .map(|one| if let Operand::Constant(id) = one { Some(*id) } else { None })
                    .collect();
                let bits = accumulate_constant_offset(&module.context, layout, source, &indices?, width)?;
                // Sign-extended from the index width.
                let shift = 128 - width;
                let signed = ((bits << shift) as i128 >> shift) as i64;
                base.checked_add(signed)
            });
            if let Some(derived) = op.result {
                work.extend(function.users(derived).into_iter().map(|used| (*used, next)));
            }
            continue;
        }
        let info = access_info(module, layout, function, used, offset);
        let Some(block) = function.parent(used.user) else { continue };
        if !result.per_block.contains_key(&block) {
            result.per_block.insert(block, BlockUses::default());
        }
        let uses = result.per_block.get_mut(&block).expect("inserted above");
        if uses.insts.contains_key(&used.user) {
            // More than one use of the argument in one instruction: a clobber.
            uses.insts.insert(used.user, Access::unknown());
            uses.has_unknown_access = true;
            continue;
        }
        uses.has_unknown_access |= info.kind == Kind::Unknown;
        let writes = info.writes();
        uses.insts.insert(used.user, info);
        result.any_write |= writes;
        if writes && block != entry {
            result.write_outside_entry = true;
        }
    }
    result
}

/// The initialized ranges of the parameter `index` of `id` (`inferInitializes`
/// without stating them): None where there is nothing to state.
fn inferred(
    module: &Module,
    layout: &DataLayout,
    id: GlobalId,
    index: usize,
) -> Option<Ranges> {
    let function = module.global(id).function()?;
    let entry = function.entry()?;
    let uses = collect(module, layout, function, function.parameters()[index], entry);
    // No write anywhere in the function.
    if !uses.any_write {
        return None;
    }
    // The uses of a block in instruction order.
    let order: IdMap<InstId, usize> = uses
        .per_block
        .keys()
        .flat_map(|block| function.iter_block(block).enumerate().map(|(at, inst)| (inst, at)))
        .collect();
    let mut initialized: IdMap<BlockId, Ranges> = IdMap::default();
    let visit = |block: BlockId, initialized: &IdMap<BlockId, Ranges>| -> Ranges {
        let here = uses.per_block.get(&block);
        let mut ranges = Ranges::default();
        // Start with the intersection of the successors. A block with an
        // unknown use clears the ranges at some point anyway.
        if here.is_none_or(|one| !one.has_unknown_access) {
            let mut first = true;
            for successor in function.successors(block) {
                match initialized.get(&successor) {
                    Some(held) if first => {
                        ranges = held.clone();
                        first = false;
                    }
                    Some(held) => ranges = ranges.intersect(held),
                    None => {
                        ranges = Ranges::default();
                        break;
                    }
                }
            }
        }
        if let Some(here) = here {
            let mut insts: Vec<(InstId, &Access)> = here.insts.iter().collect();
            insts.sort_by_key(|(inst, _)| order.get(inst).copied());
            // From the end of the block to its start.
            for (_, info) in insts.into_iter().rev() {
                if matches!(info.kind, Kind::Unknown | Kind::WriteWithSideEffect) {
                    ranges = Ranges::default();
                }
                if info.ranges.is_empty() {
                    continue;
                }
                if info.kind == Kind::Read {
                    for &(low, high) in &info.ranges.0 {
                        ranges.subtract(low, high);
                    }
                } else {
                    ranges = ranges.union(&info.ranges);
                }
            }
        }
        ranges
    };
    // If every write is in the entry block, or it has an unknown use, only the
    // entry block is looked at.
    let only_entry = !uses.write_outside_entry || uses.per_block.get(&entry).is_some_and(|one| one.has_unknown_access);
    let at_entry = if only_entry {
        visit(entry, &initialized)
    } else {
        for block in post_order(function, entry) {
            let ranges = visit(block, &initialized);
            if !ranges.is_empty() {
                initialized.insert(block, ranges);
            }
        }
        initialized.get(&entry).cloned().unwrap_or_default()
    };
    (!at_entry.is_empty()).then_some(at_entry)
}

/// The blocks reachable from `entry`, successors first.
fn post_order(
    function: &Function,
    entry: BlockId,
) -> Vec<BlockId> {
    let mut seen = IdSet::default();
    seen.insert(entry);
    let mut out = Vec::new();
    let mut stack = vec![(entry, function.successors(entry).into_iter().collect::<Vec<_>>())];
    while let Some((block, pending)) = stack.last_mut() {
        match pending.pop() {
            Some(next) if seen.insert(next) => {
                let after = function.successors(next).into_iter().collect::<Vec<_>>();
                stack.push((next, after));
            }
            Some(_) => {}
            None => {
                out.push(*block);
                stack.pop();
            }
        }
    }
    out
}

/// `inferInitializes` over the pointer parameters of the members that are not
/// read-only: the members whose parameter attributes changed.
pub fn add_initializes(
    module: &mut Module,
    layout: &DataLayout,
    members: &BTreeSet<GlobalId>,
) -> BTreeSet<GlobalId> {
    let mut changed = BTreeSet::new();
    for &id in members {
        let Some(function) = module.global(id).function() else { continue };
        if function.is_declaration() || !crate::argumentattrs::exact_definition(module, id) {
            continue;
        }
        let pointers: Vec<usize> = (0..function.parameters().len())
            .filter(|&index| {
                matches!(
                    function
                        .operand_type(&module.context, Operand::Value(function.parameters()[index]))
                        .map(|ty| module.context.types.get(ty)),
                    Some(Type::Pointer(_))
                )
            })
            .collect();
        for index in pointers {
            let function = module.global(id).function().expect("a procedure");
            if Facts::of(&function.parameter_attrs[index]).read_only()
                || Facts::of(&function.parameter_attrs[index]).read_none()
            {
                continue;
            }
            let Some(found) = inferred(module, layout, id, index) else { continue };
            let attrs = match &mut module.globals[id.0 as usize].kind {
                llrm_mir::module::GlobalKind::Function(function) => &mut function.parameter_attrs[index],
                llrm_mir::module::GlobalKind::Variable(_) => unreachable!("a procedure is a function"),
            };
            let previous = llrm_mir::memory::initializes(attrs).to_vec();
            let stated = if previous.is_empty() {
                found
            } else {
                // The same ranges: nothing changed; else the union.
                let before = Ranges(previous);
                if before == found {
                    continue;
                }
                before.union(&found)
            };
            attrs.retain(|one| !matches!(one, Attribute::Initializes(_)));
            attrs.push(Attribute::Initializes(stated.0));
            changed.insert(id);
        }
    }
    changed
}

#[cfg(test)]
#[path = "initializes_tests.rs"]
mod tests;
