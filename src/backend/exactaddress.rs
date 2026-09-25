//! Fold the chain computing an exact address into its 32-bit form.
//!
//! Where lowering proved a cell exact (`Mem::exact`), the 16-bit sum of its
//! registers equals the 32-bit sum of the leaves of the affine chain
//! (`affine::step`) that computed them, once those leaves' upper halves are
//! zero (`upperzero`): `mov si,ax; shl si,1; add cx,S[si]` becomes
//! `add cx,S[eax*2]` behind the 67h prefix, and the chain goes. A far cell
//! keeps its origin as the base; only its offset folds. Where the access's
//! loop writes a leaf only below its upper half, one `movzx` in the loop's
//! preheader makes it zero for every trip.

use std::collections::BTreeSet;
use std::sync::Arc;

use iced_x86::Register;

use crate::analysis::intervals::_graph;
use crate::analysis::loops;
use crate::backend::affine::{self, Step};
use crate::backend::cpu::{self as targets, Profile, ProfileOrName};
use crate::backend::lanes::Lanes;
use crate::backend::peephole::{DeadAfter, _flag_lanes, _lanes, _read_before_redefined, _register_effects, id};
use crate::backend::upperzero::{self, Roots, ROOTS};
use crate::backend::{liveness, regthrash, target};
use crate::model::ir::{self, Held, Loc, Mem, Operation, Reg, Semantics, Space};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::AddressForm;
use crate::support::hash::{HashMap, HashSet, IndexMap};

/// An address as a sum: each register's root, multiple and value.
type Sum = Vec<(Register, i64, u32)>;

/// One cut of the chain computing a register: the removed instructions (by
/// position), what the register equals in their place, and their cost.
#[derive(Clone)]
struct Stage {
    removed: Vec<usize>,
    sum: Sum,
    disp: i64,
    cost: i64,
}

/// A chain removed, and each reader it fed rewritten.
struct Fold {
    removed: Vec<usize>,
    /// Per reader: its id, its rewrite, and the roots whose upper half must
    /// be zero there.
    users: Vec<(usize, Arc<Insn>, Roots)>,
}

fn full32(register: Register) -> Register {
    ir::root(register)
}

/// The lanes `one` reads and writes, or None when unknown.
fn effects(one: &Insn) -> Option<(Lanes, Lanes)> {
    _register_effects(one, true, true).or_else(|| liveness::_declared(one))
}

/// Whether nothing in `between` writes any lane of `register`.
fn untouched(between: &[Arc<Insn>], register: Register) -> bool {
    let lanes = _lanes(register);
    between.iter().all(|one| effects(one).is_some_and(|(_, writes)| writes.and(&lanes).is_empty()))
}

/// Whether the last write of `register` before `at` defines `value`.
fn holds(insns: &[Arc<Insn>], at: usize, register: Register, value: u32) -> bool {
    let lanes = _lanes(register);
    insns[..at]
        .iter()
        .rev()
        .find_map(|one| match effects(one) {
            None => Some(false),
            Some((_, writes)) if !writes.and(&lanes).is_empty() => Some(one.defines.contains(&value)),
            _ => None,
        })
        .unwrap_or(false)
}

/// Adds `term` to `sum`; None where one root would hold two values.
fn add(mut sum: Sum, term: (Register, i64, u32)) -> Option<Sum> {
    match sum.iter_mut().find(|one| one.0 == term.0) {
        Some(one) if one.2 != term.2 => return None,
        Some(one) => one.1 += term.1,
        None => sum.push(term),
    }
    Some(sum)
}

/// The one exact cell `one` addresses, where `one` reads its registers
/// nowhere else.
fn cell_of(one: &Insn) -> Option<&Mem> {
    let what = one.what.as_ref()?;
    let mut cells = what.dests.iter().chain(&what.sources).filter_map(|operand| match operand {
        Loc::Mem(cell) => Some(cell),
        _ => None,
    });
    let (Some(cell), None) = (cells.next(), cells.next()) else {
        return None;
    };
    let roots = [cell.through, cell.index_through].map(full32);
    let elsewhere = what.dests.iter().chain(&what.sources).any(|operand| matches!(operand, Loc::Address(_)))
        || what.sources.iter().any(|operand| matches!(operand, Loc::Reg(read) if read.register != Register::None && roots.contains(&full32(read.register))));
    let space = cell.addr?.space;
    (cell.exact && !elsewhere && matches!(space, Space::Far | Space::Literal | Space::Segment | Space::External)).then_some(cell)
}

/// `cell`'s registers as a sum, each with the value it holds before `at`.
///
/// The allocator may name a 16-bit cell's base and index registers in
/// either order; the reaching writes say which holds which.
fn sum_of(insns: &[Arc<Insn>], at: usize, cell: &Mem) -> Option<Sum> {
    let registers: Vec<(Register, i64)> =
        [(cell.through, 1), (cell.index_through, cell.scale)].into_iter().filter(|(register, _)| *register != Register::None).collect();
    let held: Vec<Held> = [cell.base, cell.index].into_iter().flatten().collect();
    if registers.len() != held.len() || registers.iter().any(|(register, _)| !target::WIDTHS.contains_key(register)) {
        return None;
    }
    let values: Vec<u32> = match (registers.as_slice(), held.as_slice()) {
        ([(first, _), (second, _)], [base, index]) if target::width_of(*first) == Some(2) && base.value != index.value => {
            let direct = holds(insns, at, *first, base.value) || holds(insns, at, *second, index.value);
            let swapped = holds(insns, at, *first, index.value) || holds(insns, at, *second, base.value);
            match (direct, swapped) {
                (true, false) => vec![base.value, index.value],
                (false, true) => vec![index.value, base.value],
                _ => return None,
            }
        }
        _ => held.iter().map(|one| one.value).collect(),
    };
    registers.iter().zip(values).try_fold(Vec::new(), |sum, ((register, multiple), value)| add(sum, (full32(*register), *multiple, value)))
}

/// The roots of `cell`'s offset: every register of a symbol's cell, and a
/// far cell's register other than its origin.
fn offsets(cell: &Mem, sum: &Sum) -> Vec<Register> {
    match cell.addr.map(|addr| addr.space) {
        Some(Space::Segment | Space::External) => sum.iter().map(|one| one.0).collect(),
        _ => {
            let (Some(base), Some(index)) = (cell.base, cell.index) else {
                return Vec::new();
            };
            sum.iter().filter(|one| one.2 == index.value && one.2 != base.value).map(|one| one.0).collect()
        }
    }
}

/// The readers of `register` from `first` on, where every one addresses an
/// exact cell through it, up to where it is written or dies.
fn readers(block: &LirBlock, first: usize, register: Register, dead_after: &DeadAfter) -> Option<Vec<usize>> {
    let lanes = _lanes(register);
    let root = full32(register);
    let mut found = Vec::new();
    for (at, one) in block.insns.iter().enumerate().skip(first) {
        let (reads, writes) = effects(one)?;
        if !reads.and(&lanes).is_empty() {
            let cell = cell_of(one)?;
            let sum = sum_of(&block.insns, at, cell)?;
            if !offsets(cell, &sum).contains(&root) {
                return None;
            }
            found.push(at);
        }
        let dead = &dead_after[&id(one)];
        if !writes.and(&lanes).is_empty() {
            return lanes.minus(&writes).minus(dead).is_empty().then_some(found);
        }
        if lanes.minus(dead).is_empty() {
            return Some(found);
        }
    }
    None
}

/// Each cut of the chain that computed `register` for its first reader at
/// `first`, where it held `value`: one more step removed each time.
fn stages(insns: &[Arc<Insn>], first: usize, register: Reg, value: u32, dead_after: &DeadAfter, cpu: &Profile) -> Vec<Stage> {
    let root = full32(register.register);
    let lanes = _lanes(register.register);
    let flags = _flag_lanes(0xFFFF_FFFF);
    let mut found = Vec::new();
    let mut current = Stage { removed: Vec::new(), sum: vec![(root, 1, value)], disp: 0, cost: 0 };
    let mut cursor = first;
    loop {
        // The last writer before the cursor; nothing between reads it.
        let mut writer = None;
        for at in (0..cursor).rev() {
            let Some((reads, writes)) = effects(&insns[at]) else {
                return found;
            };
            if !writes.and(&lanes).is_empty() {
                writer = Some(at);
                break;
            }
            if !reads.and(&lanes).is_empty() {
                return found;
            }
        }
        let Some(at) = writer else {
            return found;
        };
        let one = &insns[at];
        let Some((dest, step, cost)) = affine::step(one, cpu) else {
            return found;
        };
        let flags_dead = effects(one).is_some_and(|(_, writes)| writes.and(&flags).minus(&dead_after[&id(one)]).is_empty());
        if dest != register || one.symbol == Some(true) || !one.requires.is_empty() || !one.delivers.is_empty() || !flags_dead {
            return found;
        }
        let multiple = current.sum.iter().find(|term| term.0 == root).map_or(0, |term| term.1);
        let mut uses: Vec<u32> = one.uses.clone();
        uses.sort_unstable();
        uses.dedup();
        let later = &insns[at + 1..first];
        let (before, leaf) = match (step, uses.as_slice()) {
            (Step::Add(_) | Step::Scale(_), [before]) => (Some(*before), None),
            (Step::Copy(source), [value]) if full32(source.register) != root && untouched(later, source.register) => {
                (None, Some((source, *value)))
            }
            (Step::AddRegister(other), [value]) if full32(other.register) != root && untouched(later, other.register) => {
                (Some(*value), Some((other, *value)))
            }
            (Step::AddRegister(other), [one_value, another]) if full32(other.register) != root && untouched(later, other.register) => {
                // Which use is the added register's: the reaching writes say.
                let mine = |value: u32| holds(insns, at, other.register, value) && !holds(insns, at, register.register, value);
                let theirs = |value: u32| holds(insns, at, register.register, value) && !holds(insns, at, other.register, value);
                match (mine(*one_value) || theirs(*another), mine(*another) || theirs(*one_value)) {
                    (true, false) => (Some(*another), Some((other, *one_value))),
                    (false, true) => (Some(*one_value), Some((other, *another))),
                    _ => return found,
                }
            }
            _ => return found,
        };
        let mut sum: Sum = current.sum.iter().filter(|term| term.0 != root).copied().collect();
        let mut disp = current.disp;
        match (step, before) {
            (Step::Add(constant), Some(before)) => {
                disp += multiple * constant;
                sum.push((root, multiple, before));
            }
            (Step::Scale(factor), Some(before)) => sum.push((root, multiple * factor, before)),
            (Step::AddRegister(_), Some(before)) => sum.push((root, multiple, before)),
            _ => {}
        }
        if let Some((source, value)) = leaf {
            let Some(summed) = add(sum, (full32(source.register), multiple, value)) else {
                return found;
            };
            sum = summed;
        }
        current = Stage { removed: [vec![at], current.removed].concat(), sum, disp, cost: current.cost + cost };
        found.push(current.clone());
        if matches!(step, Step::Copy(_)) {
            return found;
        }
        cursor = at;
    }
}

/// `one`, whose cell reads `root`, with `stage` in its place, the roots it
/// then reads 32 bits wide, and what the wider address costs it.
fn rewritten(
    insns: &[Arc<Insn>],
    at: usize,
    root: Register,
    stage: &Stage,
    uses: &dyn Fn(&Insn) -> Vec<u32>,
    form: &AddressForm,
    cpu: &Profile,
) -> Option<(Arc<Insn>, Roots, i64)> {
    let one = &insns[at];
    let cell = cell_of(one)?;
    let addr = cell.addr?;
    let before = sum_of(insns, at, cell)?;
    let multiple = before.iter().find(|term| term.0 == root)?.1;
    let sum = stage
        .sum
        .iter()
        .map(|term| (term.0, term.1 * multiple, term.2))
        .try_fold(before.iter().filter(|term| term.0 != root).copied().collect(), add)?;
    let terms: Vec<(Register, i64)> = sum.iter().map(|term| (term.0, term.1)).collect();
    let address = affine::form(&terms, 0, &form.scales)?;
    let value = |register: Register| sum.iter().find(|term| term.0 == register).map(|term| Held { value: term.2, width: 4 });
    let (base, index) = (value(address.through), value(address.index));
    if matches!(addr.space, Space::Far | Space::Literal) && (index.is_none() || base.map(|one| one.value) != cell.base.map(|one| one.value)) {
        return None;
    }
    // A 32-bit EBP base selects SS where a 16-bit cell's register did not.
    if address.through == Register::EBP && addr.segment == Register::None {
        return None;
    }
    let widened = Mem {
        addr: Some(ir::Addr { disp: addr.disp + multiple * stage.disp, ..addr }),
        through: address.through,
        base,
        index_through: address.index,
        index,
        scale: address.scale,
        ..cell.clone()
    };
    let what = one.what.as_ref()?;
    let replace = |operand: &Loc| match operand {
        Loc::Mem(_) => Loc::Mem(widened.clone()),
        _ => operand.clone(),
    };
    let rewrite = Arc::new(Insn {
        what: Some(Semantics { dests: what.dests.iter().map(replace).collect(), sources: what.sources.iter().map(replace).collect(), ..what.clone() }),
        uses: uses(one),
        ..(**one).clone()
    });
    let needs = sum.iter().fold(0, |roots, term| roots | upperzero::bit(term.0).unwrap_or(0));
    let narrow = [cell.through, cell.index_through].iter().any(|register| target::width_of(*register) == Some(2));
    let cost = if narrow { form.use_cost + sum.len() as i64 * cpu.partial_register_stall } else { 0 };
    Some((rewrite, needs, cost))
}

/// The chain the cheapest to remove for the readers of `root` from `first`
/// on, where removing it saves the target cycles.
fn fold(block: &LirBlock, first: usize, root: Register, dead_after: &DeadAfter, live_out: &BTreeSet<u32>, form: &AddressForm, cpu: &Profile) -> Option<Fold> {
    let insns = &block.insns;
    let register = Reg { register: target::named(root, 2), width: 2 };
    let users = readers(block, first, register.register, dead_after)?;
    let value = sum_of(insns, first, cell_of(&insns[first])?)?.into_iter().find(|term| term.0 == root)?.2;
    let mut best: Option<(i64, Fold)> = None;
    let last = *users.last()?;
    for stage in stages(insns, first, register, value, dead_after, cpu) {
        // Every reader sees the leaves the chain read: none is written up to
        // the last of them.
        let span = &insns[stage.removed[0] + 1..last];
        if stage.sum.iter().any(|term| term.0 != root && !untouched(span, target::named(term.0, 2))) {
            continue;
        }
        let removed: BTreeSet<usize> = stage.removed.iter().copied().collect();
        let defined: BTreeSet<u32> = removed.iter().flat_map(|at| insns[*at].defines.iter().copied()).collect();
        // What the chain read from before it: the rewrites read it instead.
        let mut external = Vec::new();
        let mut earlier = BTreeSet::new();
        for at in &removed {
            for value in &insns[*at].uses {
                if !earlier.contains(value) && !external.contains(value) {
                    external.push(*value);
                }
            }
            earlier.extend(insns[*at].defines.iter().copied());
        }
        let uses = |one: &Insn| {
            let kept: Vec<u32> = one.uses.iter().copied().filter(|value| !defined.contains(value)).collect();
            let added: Vec<u32> = external.iter().copied().filter(|value| !kept.contains(value)).collect();
            [kept, added].concat()
        };
        let Some(rewrites) = users
            .iter()
            .map(|at| rewritten(insns, *at, root, &stage, &uses, form, cpu).map(|found| (*at, found)))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let saved = stage.cost - rewrites.iter().map(|(_, (_, _, cost))| cost).sum::<i64>();
        if saved < 0 || best.as_ref().is_some_and(|(most, _)| *most > saved) {
            continue;
        }
        // The chain's own identities go with it; a read of one past it
        // would lose its definition.
        let rewrites: HashMap<usize, (Arc<Insn>, Roots)> =
            rewrites.into_iter().map(|(at, (rewrite, needs, _))| (at, (rewrite, needs))).collect();
        let eliminated: BTreeSet<u32> = defined
            .iter()
            .copied()
            .filter(|value| !external.contains(value) && !rewrites.values().any(|(one, _)| one.defines.contains(value)))
            .collect();
        let after: Vec<Arc<Insn>> = (stage.removed[0] + 1..insns.len())
            .filter(|at| !removed.contains(at))
            .map(|at| rewrites.get(&at).map_or_else(|| Arc::clone(&insns[at]), |(one, _)| Arc::clone(one)))
            .collect();
        if _read_before_redefined(&eliminated, &after, live_out) {
            continue;
        }
        let users = users.iter().map(|at| (id(&insns[*at]), Arc::clone(&rewrites[at].0), rewrites[at].1)).collect();
        best = Some((saved, Fold { removed: stage.removed.iter().map(|at| id(&insns[*at])).collect(), users }));
    }
    best.map(|(_, found)| found)
}

/// Every fold one round finds, none sharing an instruction.
fn folds(body: &LirBody, form: &AddressForm, cpu: &Profile) -> Vec<Fold> {
    let exits = liveness::dead_at_exit(body);
    let (_, live_out) = crate::backend::allocate::live(body);
    let mut found = Vec::new();
    for block in &body.blocks {
        let dead_after = regthrash::_dead_after(block, exits[&block.at].clone());
        let outside = live_out.get(&block.at).cloned().unwrap_or_default();
        let mut claimed: HashSet<usize> = HashSet::default();
        for (at, one) in block.insns.iter().enumerate() {
            let Some(sum) = cell_of(one).and_then(|cell| sum_of(&block.insns, at, cell).map(|sum| offsets(cell, &sum))) else {
                continue;
            };
            for root in sum {
                let Some(candidate) = fold(block, at, root, &dead_after, &outside, form, cpu) else {
                    continue;
                };
                let touched: Vec<usize> = candidate.removed.iter().copied().chain(candidate.users.iter().map(|user| user.0)).collect();
                if touched.iter().any(|one| claimed.contains(one)) {
                    continue;
                }
                claimed.extend(touched);
                found.push(candidate);
                break;
            }
        }
    }
    found
}

/// `movzx root,word` for each root in `roots`, before `block`'s terminator.
fn extended(block: &LirBlock, roots: Roots) -> LirBlock {
    let mut insns = block.insns.clone();
    let at = insns.last().map_or(block.at, |one| one.at);
    let position = if insns.last().is_some_and(|one| liveness::_terminator(one.what.as_ref())) { insns.len() - 1 } else { insns.len() };
    for (index, root) in ROOTS.iter().enumerate() {
        if roots & 1 << index == 0 {
            continue;
        }
        let word = target::named(*root, 2);
        let what = Semantics {
            name: Some("movzx".into()),
            dests: vec![Loc::Reg(Reg { register: *root, width: 4 })],
            sources: vec![Loc::Reg(Reg { register: word, width: 2 })],
            ..Semantics::new(Operation::Extend)
        };
        insns.insert(position, Arc::new(Insn::new(at, Some((at, at)), Some(what), Vec::new(), Vec::new())));
    }
    block.with_insns(insns)
}

/// `body` with the upper half of each loop's `roots` zeroed on entry, where
/// every way in may take it: one successor, and those lanes dead there.
fn preheaded(body: &LirBody, wanted: &IndexMap<i64, Roots>) -> (LirBody, IndexMap<i64, Roots>) {
    let graph = _graph(&body.blocks);
    let found = loops::loops(&graph, Some(body.entry));
    let predecessors = loops::predecessors(&graph);
    let exits = liveness::dead_at_exit(body);
    let mut placed: IndexMap<i64, Roots> = IndexMap::default();
    let mut per_block: IndexMap<i64, Roots> = IndexMap::default();
    for (header, roots) in wanted {
        let Some(inside) = found.iter().find(|one| one.header == *header).map(|one| &one.body) else {
            continue;
        };
        let entries: Vec<i64> = predecessors.get(header).into_iter().flatten().copied().filter(|at| !inside.contains(at)).collect();
        let mut allowed = *roots;
        for (index, root) in ROOTS.iter().enumerate() {
            let upper = [(*root, 2), (*root, 3)];
            let takes = !entries.is_empty()
                && entries.iter().all(|at| {
                    body.blocks.iter().find(|block| block.at == *at).is_some_and(|block| block.succ == [*header])
                        && upper.iter().all(|lane| exits[at].contains(lane))
                });
            if !takes {
                allowed &= !(1 << index);
            }
        }
        if allowed == 0 {
            continue;
        }
        placed.insert(*header, allowed);
        for at in entries {
            *per_block.entry(at).or_default() |= allowed;
        }
    }
    let blocks = body
        .blocks
        .iter()
        .map(|block| match per_block.get(&block.at) {
            Some(roots) => extended(block, *roots),
            None => block.clone(),
        })
        .collect();
    (body.with_blocks(blocks), placed)
}

/// Fold each exact address's chain the cost model prefers, until none is left.
pub fn exact_addresses<'a>(body: &LirBody, cpu: impl Into<ProfileOrName<'a>>) -> Result<LirBody, String> {
    let profile = targets::profile(cpu)?;
    let Some(form) = profile.address_forms.iter().find(|form| form.secondary && form.index_width == 4) else {
        return Ok(body.clone());
    };
    let mut body = body.clone();
    // Each round folds one register of a cell; its other register folds in
    // the next.
    for _ in 0..4 {
        match folded(&body, form, &profile) {
            Some(changed) => body = changed,
            None => break,
        }
    }
    Ok(body)
}

/// One round of folds, with the preheader extensions they need; None when
/// there are none.
fn folded(body: &LirBody, form: &AddressForm, profile: &Profile) -> Option<LirBody> {
    let found = folds(body, form, profile);
    if found.is_empty() {
        return None;
    }
    let block_of: HashMap<usize, i64> =
        body.blocks.iter().flat_map(|block| block.insns.iter().map(move |one| (id(one), block.at))).collect();
    let graph = _graph(&body.blocks);
    let natural = loops::loops(&graph, Some(body.entry));
    let innermost =
        |at: i64| natural.iter().filter(|one| one.body.contains(&at)).min_by_key(|one| one.body.len()).map(|one| one.header);

    // Ask each missing root of the innermost loop around the access, then
    // keep the folds the analysis proves over the result.
    let mut accepted: Vec<&Fold> = found.iter().collect();
    loop {
        let zero = upperzero::before(body);
        let mut wanted: IndexMap<i64, Roots> = IndexMap::default();
        let mut placeable = Vec::new();
        for candidate in &accepted {
            let mut fine = true;
            for (user, _, needs) in &candidate.users {
                let missing = needs & !zero[user];
                if missing == 0 {
                    continue;
                }
                match innermost(block_of[user]) {
                    Some(header) => *wanted.entry(header).or_default() |= missing,
                    None => fine = false,
                }
            }
            if fine {
                placeable.push(*candidate);
            }
        }
        let (extended, _) = preheaded(body, &wanted);
        let zero = upperzero::before(&extended);
        let proven: Vec<&Fold> = placeable
            .into_iter()
            .filter(|candidate| candidate.users.iter().all(|(user, _, needs)| needs & !zero[user] == 0))
            .collect();
        if proven.len() == accepted.len() {
            let removed: HashSet<usize> = proven.iter().flat_map(|candidate| candidate.removed.iter().copied()).collect();
            let by_user: HashMap<usize, &Arc<Insn>> =
                proven.iter().flat_map(|candidate| candidate.users.iter().map(|(user, one, _)| (*user, one))).collect();
            let blocks = extended
                .blocks
                .iter()
                .map(|block| {
                    block.with_insns(
                        block
                            .insns
                            .iter()
                            .filter(|one| !removed.contains(&id(one)))
                            .map(|one| by_user.get(&id(one)).map_or_else(|| Arc::clone(one), |rewrite| Arc::clone(rewrite)))
                            .collect(),
                    )
                })
                .collect();
            return Some(extended.with_blocks(blocks));
        }
        accepted = proven;
        if accepted.is_empty() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iced_x86::Register;

    use super::exact_addresses;
    use crate::model::ir::{Addr, Held, Imm, Loc, Mem, Operation, Reg, Semantics, Space};
    use crate::model::lir::{Insn, LirBlock, LirBody};
    use crate::support::hash::IndexMap;

    fn reg(register: Register, width: u32) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    fn one(at: i64, op: Operation, name: &str, dests: Vec<Loc>, sources: Vec<Loc>, defines: Vec<u32>, uses: Vec<u32>) -> Arc<Insn> {
        let what = Semantics { name: Some(name.to_owned()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), defines, uses))
    }

    #[test]
    fn test_the_folded_index_names_the_value_the_shift_read() {
        // Uncoalesced, `shl bx,1` reads 20 and defines 21: the address kept
        // naming 21, which lost its only definition with the shift.
        let cell = Mem {
            addr: Some(Addr { segment: Register::FS, ..Addr::new(Space::Far, 0) }),
            // The allocator names BX first for the 16-bit encoding.
            through: Register::BX,
            base: Some(Held { value: 26, width: 2 }),
            index: Some(Held { value: 21, width: 2 }),
            index_through: Register::DI,
            exact: true,
            ..Mem::new(None, 2)
        };
        let insns = vec![
            one(0, Operation::Extend, "movzx", vec![reg(Register::EBX, 4)], vec![reg(Register::BX, 2)], vec![], vec![]),
            one(1, Operation::Extend, "movzx", vec![reg(Register::EDI, 4)], vec![reg(Register::DI, 2)], vec![], vec![]),
            one(2, Operation::Binary, "shl", vec![reg(Register::BX, 2)], vec![reg(Register::BX, 2), Loc::Imm(Imm { value: 1, width: 1, address: None })], vec![21], vec![20]),
            one(3, Operation::Move, "mov", vec![reg(Register::BX, 2)], vec![Loc::Mem(cell)], vec![30], vec![26, 21]),
            one(4, Operation::Compare, "cmp", vec![], vec![reg(Register::BX, 2), Loc::Imm(Imm { value: 0, width: 2, address: None })], vec![], vec![30]),
        ];
        let body = LirBody::new("index", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());

        let result = exact_addresses(&body, "386").unwrap();

        let load = result.blocks[0].insns.iter().find(|one| one.defines == [30]).unwrap();
        assert_eq!(result.blocks[0].insns.len(), 4);
        let Loc::Mem(cell) = &load.what.as_ref().unwrap().sources[0] else { panic!("not a cell") };
        assert_eq!((cell.index, cell.scale, cell.through, cell.index_through), (Some(Held { value: 20, width: 4 }), 2, Register::EDI, Register::EBX));
        assert!(load.uses.contains(&20) && !load.uses.contains(&21));
    }

    #[test]
    fn test_a_copied_scaled_index_folds_into_every_symbolic_reader() {
        // `mov si,ax; shl si,1` fed `S%[si]` and `T%[si]`: only a far cell's
        // shift folded, never a copy, and never into a symbol's cell.
        let cell = |index: i64| Mem {
            addr: Some(Addr { index, ..Addr::new(Space::Segment, 0) }),
            through: Register::SI,
            base: Some(Held { value: 12, width: 2 }),
            exact: true,
            ..Mem::new(None, 2)
        };
        let insns = vec![
            one(0, Operation::Extend, "movzx", vec![reg(Register::EAX, 4)], vec![reg(Register::AX, 2)], vec![10], vec![10]),
            one(1, Operation::Move, "mov", vec![reg(Register::SI, 2)], vec![reg(Register::AX, 2)], vec![11], vec![10]),
            one(2, Operation::Binary, "shl", vec![reg(Register::SI, 2)], vec![reg(Register::SI, 2), Loc::Imm(Imm { value: 1, width: 1, address: None })], vec![12], vec![11]),
            one(3, Operation::Move, "mov", vec![reg(Register::CX, 2)], vec![Loc::Mem(cell(1))], vec![13], vec![12]),
            one(4, Operation::Move, "mov", vec![reg(Register::SI, 2)], vec![Loc::Mem(cell(2))], vec![14], vec![12]),
            one(5, Operation::Compare, "cmp", vec![], vec![reg(Register::CX, 2), reg(Register::SI, 2)], vec![], vec![13, 14]),
        ];
        let body = LirBody::new("copied", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());

        let result = exact_addresses(&body, "386").unwrap();

        let insns = &result.blocks[0].insns;
        assert_eq!(insns.len(), 4);
        for load in &insns[1..3] {
            let Loc::Mem(cell) = &load.what.as_ref().unwrap().sources[0] else { panic!("not a cell") };
            assert_eq!(
                (cell.base.map(|one| one.value), cell.through, cell.index_through, cell.scale),
                (Some(10), Register::EAX, Register::EAX, 1),
            );
            assert_eq!(load.uses, [10]);
        }
    }

    #[test]
    fn test_a_leaf_written_between_readers_keeps_them_16_bit() {
        // `mov ax,5` between the readers: the second one read the new AX
        // through `[eax+eax]`.
        let cell = |index: i64| Mem {
            addr: Some(Addr { index, ..Addr::new(Space::Segment, 0) }),
            through: Register::SI,
            base: Some(Held { value: 12, width: 2 }),
            exact: true,
            ..Mem::new(None, 2)
        };
        let insns = vec![
            one(0, Operation::Extend, "movzx", vec![reg(Register::EAX, 4)], vec![reg(Register::AX, 2)], vec![10], vec![10]),
            one(1, Operation::Move, "mov", vec![reg(Register::SI, 2)], vec![reg(Register::AX, 2)], vec![11], vec![10]),
            one(2, Operation::Binary, "shl", vec![reg(Register::SI, 2)], vec![reg(Register::SI, 2), Loc::Imm(Imm { value: 1, width: 1, address: None })], vec![12], vec![11]),
            one(3, Operation::Move, "mov", vec![reg(Register::CX, 2)], vec![Loc::Mem(cell(1))], vec![13], vec![12]),
            one(4, Operation::Move, "mov", vec![reg(Register::AX, 2)], vec![Loc::Imm(Imm { value: 5, width: 2, address: None })], vec![15], vec![]),
            one(5, Operation::Move, "mov", vec![reg(Register::SI, 2)], vec![Loc::Mem(cell(2))], vec![14], vec![12]),
            one(6, Operation::Compare, "cmp", vec![], vec![reg(Register::CX, 2), reg(Register::SI, 2)], vec![], vec![13, 14]),
        ];
        let body = LirBody::new("clobbered", 0, vec![LirBlock::new(0, insns)], IndexMap::default(), IndexMap::default());

        let result = exact_addresses(&body, "386").unwrap();

        let Loc::Mem(cell) = &result.blocks[0].insns.iter().find(|one| one.defines == [14]).unwrap().what.as_ref().unwrap().sources[0] else {
            panic!("not a cell")
        };
        assert_ne!(cell.through, Register::EAX);
    }
}
