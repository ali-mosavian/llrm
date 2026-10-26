//! A chain of one integer operation rebuilt so the operand that arrives last
//! joins last: LLVM's `MachineCombiner`
//! ([MachineCombiner.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCombiner.cpp))
//! with the generic reassociation patterns of `TargetInstrInfo`, the only ones
//! i486 has.
//!
//! # How it works
//! Each block is walked in order. An instruction is a root when it is
//! `add`, `and`, `or`, `xor` or `imul` on two registers
//! (`X86InstrInfo::isAssociativeAndCommutative`) with no live flags, and one
//! of its operands is the result of the same operation (`prev`) that this
//! root alone reads, also with no live flags, defined in the block
//! (`getMachineCombinerPatterns`, `isReassociationCandidate`,
//! `hasReassociableSibling`, `hasReassociableOperands`). With
//! `B = A op X` and `C = B op Y` the four patterns (`REASSOC_AX_BY`,
//! `XA_BY`, `AX_YB`, `XA_YB`) say which of `prev`'s operands is `A`; the
//! alternative is `N = X op Y`, `C = A op N` (`reassociateOps`,
//! `mustSwapOperands` for the operand order), both placed where the root was.
//! It replaces the pair when the depth of the new root is smaller than the old
//! root's (`improvesCriticalPathLen`, `MustReduceDepth`). A value's depth is
//! the largest `depth + latency` over the values its definition reads
//! (`getDepth`): a load takes 4 cycles, a copy 0, anything else 1
//! (`defaultDefLatency`, no scheduling model for i486).
//!
//! ```text
//!   t1 = a + load        n  = a + b
//!   t2 = t1 + b    ==>   t2 = n + load     (the load, 4 cycles, joins last)
//! ```
//!
//! # Example
//! The unit tests `a_chain_whose_last_operand_arrives_late_is_rebuilt` and
//! `an_intermediate_two_instructions_read_stays`.
//!
//! # Runs
//! On SSA, after `leaopt` and before `phielim` (the pin's `addILPOpts`, after
//! `MachineSink`), at every -O level but -O0.
//!
//! # Limits
//! `preservesResourceLen` is true at once (no scheduling model), and
//! `reduceRegisterPressure` and the throughput and accumulator patterns are
//! not X86's. Optimizing for size does not take the pin's shortcut.
//!
//! DEVIATE MC1 (owner funcattrs-port, exit 2026-11-13): the trace is the
//! block alone (`MachineTraceMetrics` `TS_MinInstrCount` also follows the
//! predecessors and the physical registers: the stack pointer around calls
//! orders two calls there, and here nothing does); a value made outside the
//! block has depth 0 and the latency of its definition. Exit: a trace analysis
//! on LIR.
//!
//! DEVIATE MC2 (owner funcattrs-port, exit 2026-11-13): the latency table is
//! the default one written above, read from the operands (a memory source is a
//! load); the pin asks the opcode's `mayLoad`.
//!
//! DEVIATE MC3 (owner funcattrs-port, exit 2026-11-13): reads of a value are
//! counted from the operands and phis of the body (`hasOneNonDBGUse`), and a
//! value's one definition is found in a table made once, not
//! `MachineRegisterInfo`'s use-def chains. No kill flags exist in LIR, so none
//! are carried over, and the new instructions copy the old ones' fields except
//! the operands.

use std::sync::Arc;

use llrm_lir::registers::Regs;

use crate::backend::precse::{flags_read_after, sets_flags};
use crate::model::ir::{Held, Loc, Operation, Semantics};
use crate::model::lir::{Block, Insn, LirBody};
use crate::model::passes::LIRTransform;
use crate::support::hash::{HashMap, HashSet};

/// The phase.
pub struct MachineCombiner;

impl LIRTransform for MachineCombiner {
    fn class_name(&self) -> &'static str {
        "MachineCombiner"
    }

    fn name(&self) -> &str {
        "combiner"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        Ok(combined(body))
    }
}

/// The four `MachineCombinerPattern::REASSOC_*`: which operand of `prev` is
/// `A`, and which operand of the root `prev` is.
#[derive(Clone, Copy)]
enum Pattern {
    AxBy,
    XaBy,
    AxYb,
    XaYb,
}

impl Pattern {
    /// Whether `prev` is the root's second operand (`OperandIndices[0]`).
    fn commuted(self) -> bool {
        matches!(self, Pattern::AxYb | Pattern::XaYb)
    }

    /// Whether `A` is `prev`'s second operand (`OperandIndices[1]`).
    fn a_second(self) -> bool {
        matches!(self, Pattern::XaBy | Pattern::XaYb)
    }

    /// `mustSwapOperands`: the root's and the new `prev`'s operand orders.
    fn swaps(self) -> (bool, bool) {
        match self {
            Pattern::AxBy => (false, false),
            Pattern::XaBy => (true, false),
            Pattern::AxYb | Pattern::XaYb => (true, true),
        }
    }
}

/// `TargetInstrInfo::getMachineCombinerPatterns`'s order for one root.
fn patterns(commuted: bool) -> [Pattern; 2] {
    if commuted { [Pattern::AxYb, Pattern::XaYb] } else { [Pattern::AxBy, Pattern::XaBy] }
}

/// What the pass knows of the body before it rebuilds a block.
struct Facts {
    /// Reads of each value (`hasOneNonDBGUse`).
    reads: HashMap<u32, u32>,
    /// The latency of the instruction defining each value.
    latency: HashMap<u32, u32>,
    /// Values some instruction or phi defines.
    defined: HashSet<u32>,
    regs: Regs,
    bits: u32,
}

/// `defaultDefLatency` for no scheduling model: a copy is transient (0), a
/// load takes `LoadLatency` (4), the rest 1.
fn latency_of(one: &Insn) -> u32 {
    let Some(what) = &one.what else { return 1 };
    if what.op == Operation::Move && matches!(what.sources.as_slice(), [Loc::Held(_)]) {
        return 0;
    }
    if what.sources.iter().any(|place| matches!(place, Loc::Mem(_))) { 4 } else { 1 }
}

/// The values an instruction's operands read: its sources, and the address
/// of a destination that is memory.
fn read_by(what: &Semantics) -> impl Iterator<Item = u32> + '_ {
    let addresses = what.dests.iter().filter(|place| matches!(place, Loc::Mem(_)));
    what.sources.iter().chain(addresses).flat_map(|place| llrm_lir::values(place).into_iter().map(|held| held.value))
}

/// How often `one` reads `value`.
fn names(
    one: &Insn,
    value: u32,
) -> u32 {
    let operands = one.what.as_ref().map_or(0, |what| read_by(what).filter(|&read| read == value).count() as u32);
    let listed = one.uses.iter().filter(|&&used| used == value).count() as u32;
    let required = one.requires.iter().filter(|(held, _)| held.value == value).count() as u32;
    operands.max(listed).max(required)
}

thread_local! {
    static FACTS_MADE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has tabled a body's read counts and latencies,
/// for a test that a body with no chain to rebuild tables none.
pub fn facts_made() -> usize {
    FACTS_MADE.with(std::cell::Cell::get)
}

impl Facts {
    fn of(body: &LirBody) -> Self {
        FACTS_MADE.with(|made| made.set(made.get() + 1));
        let mut reads: HashMap<u32, u32> = HashMap::default();
        let mut latency = HashMap::default();
        let mut defined = HashSet::default();
        for block in &body.blocks {
            for phi in block.phis.iter() {
                defined.insert(phi.result);
                latency.insert(phi.result, 0);
                for &(_, value) in &phi.incoming {
                    *reads.entry(value).or_default() += 1;
                }
            }
            for one in block.insns.iter() {
                let mut seen: Vec<u32> = one.uses.clone();
                seen.extend(one.what.iter().flat_map(read_by));
                seen.extend(one.requires.iter().map(|(held, _)| held.value));
                seen.sort_unstable();
                seen.dedup();
                for value in seen {
                    *reads.entry(value).or_default() += names(one, value);
                }
                for &value in &one.defines {
                    defined.insert(value);
                    latency.insert(value, latency_of(one));
                }
            }
        }
        Facts { reads, latency, defined, regs: body.regs(), bits: body.bits }
    }
}

/// One side of a reassociable instruction: `(width, left, right)`.
fn operands(one: &Insn) -> Option<(u32, Held, Held)> {
    let what = one.what.as_ref()?;
    let associative = match what.op {
        Operation::Binary => matches!(what.name.as_deref(), Some("add" | "and" | "or" | "xor")),
        Operation::Multiply => what.name.as_deref() == Some("imul"),
        _ => false,
    };
    let ([Loc::Held(dest)], [Loc::Held(left), Loc::Held(right)]) = (&what.dests[..], &what.sources[..]) else {
        return None;
    };
    let plain = associative
        && what.target.is_none()
        && !what.indirect
        && one.defines == [dest.value]
        && left.width == dest.width
        && right.width == dest.width
        && one.call.is_none()
        && one.group.is_none()
        && !one.volatile()
        && !one.spill_reload
        && !one.spill_store
        && one.clobbers_high.is_empty()
        && one.requires.is_empty()
        && one.delivers.is_empty()
        && one.spread.is_empty();
    plain.then_some((dest.width, *left, *right))
}

/// `X86InstrInfo::hasReassociableOperands`: the flags `insns[at]` sets are dead
/// (their live ranges could not be moved), and both operands are values with
/// a definition, one of them in this block.
fn reassociable(
    facts: &Facts,
    insns: &[Arc<Insn>],
    at: usize,
    here: &HashSet<u32>,
) -> Option<(u32, Held, Held)> {
    let (width, left, right) = operands(&insns[at])?;
    if !facts.defined.contains(&left.value) || !facts.defined.contains(&right.value) {
        return None;
    }
    if !here.contains(&left.value) && !here.contains(&right.value) {
        return None;
    }
    let dead = !sets_flags(facts.regs, facts.bits, &insns[at]) || !flags_read_after(facts.regs, facts.bits, insns, at);
    dead.then_some((width, left, right))
}

/// `depth + latency` of `value` as an operand: its definition's depth where it
/// is in this block, 0 elsewhere (MC1), plus its definition's latency.
fn arrives(
    facts: &Facts,
    depth: &HashMap<u32, u32>,
    value: u32,
) -> u32 {
    depth.get(&value).copied().unwrap_or(0) + facts.latency.get(&value).copied().unwrap_or(0)
}

/// `getDepth`, for one instruction reading `values`.
fn depth_of(
    facts: &Facts,
    depth: &HashMap<u32, u32>,
    values: &[u32],
) -> u32 {
    values.iter().map(|&value| arrives(facts, depth, value)).max().unwrap_or(0)
}

/// The two instructions the pattern makes from `prev` and `root`.
struct Rebuilt {
    inner: Insn,
    outer: Insn,
    /// Depth of the new root.
    depth: u32,
    /// Depth of the new inner instruction.
    inner_depth: u32,
}

/// `TargetInstrInfo::reassociateOps` with `getReassociateOperandIndices`, and
/// `improvesCriticalPathLen` for `MustReduceDepth`: the rebuilt pair where the
/// new root is shallower than the old one.
#[allow(clippy::too_many_arguments)]
fn rebuilt(
    facts: &Facts,
    depth: &HashMap<u32, u32>,
    prev: &Insn,
    root: &Insn,
    pattern: Pattern,
    new_value: u32,
    old_depth: u32,
) -> Option<Rebuilt> {
    let (width, prev_left, prev_right) = operands(prev)?;
    let (_, root_left, root_right) = operands(root)?;
    let (a, x) = if pattern.a_second() { (prev_right, prev_left) } else { (prev_left, prev_right) };
    let y = if pattern.commuted() { root_left } else { root_right };
    let (swap_root, swap_prev) = pattern.swaps();
    let (first, second) = if swap_prev { (y, x) } else { (x, y) };
    let fresh = Held { value: new_value, width };
    let inner_depth = depth_of(facts, depth, &[first.value, second.value]);
    // The new inner instruction is a register operation: latency 1.
    let depth_of_root = arrives(facts, depth, a.value).max(inner_depth + 1);
    if depth_of_root >= old_depth {
        return None;
    }
    let dest = |one: &Insn| match one.what.as_ref().map(|what| &what.dests[..]) {
        Some([Loc::Held(held)]) => *held,
        _ => unreachable!("an operand-checked instruction has one register destination"),
    };
    let make = |one: &Insn, dest: Held, left: Held, right: Held| {
        let what = one.what.as_ref().expect("checked");
        let mut uses = vec![left.value, right.value];
        if one.uses.len() != 2 {
            uses.dedup();
        }
        Insn {
            what: Some(Semantics {
                dests: vec![Loc::Held(dest)],
                sources: vec![Loc::Held(left), Loc::Held(right)],
                ..what.clone()
            }),
            defines: vec![dest.value],
            uses,
            ..one.clone()
        }
    };
    let inner = make(prev, fresh, first, second);
    let (left, right) = if swap_root { (fresh, a) } else { (a, fresh) };
    let outer = make(root, dest(root), left, right);
    Some(Rebuilt { inner, outer, depth: depth_of_root, inner_depth })
}

/// `MachineCombinerImpl::combineInstructions` for one block: the block's
/// instructions with every profitable reassociation made, or None.
fn combine_block(
    facts: &mut Facts,
    block: &Block,
    next: &mut u32,
    like: &mut Vec<(u32, u32)>,
) -> Option<Vec<Arc<Insn>>> {
    // `hasReassociableSibling` looks for the operand's definition in the
    // block: one with no associative operation reading another's result has
    // nothing to rebuild, and its instructions are not copied.
    let mut produced: HashSet<u32> = HashSet::default();
    let chain = block
        .insns
        .iter()
        .any(
            |one| {
                let Some((_, left, right)) = operands(one) else { return false };
                let reads = produced.contains(&left.value) || produced.contains(&right.value);
                produced.extend(one.defines.iter().copied());
                reads
            },
        );
    if !chain {
        return None;
    }
    let mut insns: Vec<Arc<Insn>> = block.insns.iter().cloned().collect();
    let mut here: HashSet<u32> = block.phis.iter().map(|phi| phi.result).collect();
    // Depth of the instruction defining each value of the block made so far.
    let mut depth: HashMap<u32, u32> = HashMap::default();
    let mut changed = false;
    let mut at = 0;
    while at < insns.len() {
        let root = Arc::clone(&insns[at]);
        let found = 'found: {
            let Some((_, left, right)) = reassociable(facts, &insns, at, &here) else { break 'found None };
            let old_depth = depth_of(facts, &depth, &[left.value, right.value]);
            // `hasReassociableSibling`: `prev` is the first operand's
            // definition, or the second's where only that one is the root's
            // operation.
            let defined_by = |held: &Held| {
                insns[..at]
                    .iter()
                    .position(|one| one.defines.contains(&held.value))
                    .filter(|&index| same_operation(&insns[index], &root))
            };
            let first = defined_by(&left);
            let second = defined_by(&right);
            let (index, commuted) = match (first, second) {
                (Some(index), _) => (index, false),
                (None, Some(index)) => (index, true),
                (None, None) => break 'found None,
            };
            let prev_value = if commuted { right.value } else { left.value };
            if facts.reads.get(&prev_value) != Some(&1) {
                break 'found None;
            }
            if reassociable(facts, &insns, index, &here).is_none() {
                break 'found None;
            }
            for pattern in patterns(commuted) {
                let new_value = *next;
                if let Some(made) = rebuilt(facts, &depth, &insns[index], &root, pattern, new_value, old_depth) {
                    break 'found Some((index, made, new_value));
                }
            }
            None
        };
        let Some((index, made, new_value)) = found else {
            let new_depth = depth_of(facts, &depth, &insns[at].uses);
            for &value in &insns[at].defines {
                depth.insert(value, new_depth);
                here.insert(value);
            }
            at += 1;
            continue;
        };
        *next += 1;
        let root_value = root.defines[0];
        like.push((new_value, root_value));
        facts.defined.insert(new_value);
        facts.latency.insert(new_value, 1);
        facts.reads.insert(new_value, 1);
        depth.insert(new_value, made.inner_depth);
        depth.insert(root_value, made.depth);
        here.insert(new_value);
        here.insert(root_value);
        insns.splice(at..=at, [Arc::new(made.inner), Arc::new(made.outer)]);
        insns.remove(index);
        // `prev` left before `at`: the new root is one before `at + 1`.
        at += 1;
        changed = true;
    }
    changed.then_some(insns)
}

/// `areOpcodesEqualOrInverse` and the same width: the same instruction form.
fn same_operation(
    one: &Insn,
    other: &Insn,
) -> bool {
    match (operands(one), one.what.as_ref(), operands(other), other.what.as_ref()) {
        (Some((width, ..)), Some(left), Some((other_width, ..)), Some(right)) => {
            width == other_width && left.op == right.op && left.name == right.name
        }
        _ => false,
    }
}

/// Whether an associative operation of `body` reads the result of another: the
/// shape a reassociation starts from. The pin finds a root from the use lists
/// it keeps; here the read counts and latencies are tabled only for a body that
/// has one (the table was 1.7 M of d_faces -O2's compile, 0.47 M in the pin).
fn has_chain(body: &LirBody) -> bool {
    let mut produced: HashSet<u32> = HashSet::default();
    for block in &body.blocks {
        for one in block.insns.iter() {
            let Some((_, left, right)) = operands(one) else { continue };
            if produced.contains(&left.value) || produced.contains(&right.value) {
                return true;
            }
            produced.extend(one.defines.iter().copied());
        }
    }
    false
}

/// `MachineCombiner::runOnMachineFunction`.
pub fn combined(body: LirBody) -> LirBody {
    if !has_chain(&body) {
        return body;
    }
    let mut facts = Facts::of(&body);
    let mut next = body.next_value();
    let mut like: Vec<(u32, u32)> = Vec::new();
    let found: Vec<Option<Vec<Arc<Insn>>>> =
        body.blocks.iter().map(|block| combine_block(&mut facts, block, &mut next, &mut like)).collect();
    if found.iter().all(Option::is_none) {
        return body;
    }
    let blocks: Vec<Block> = body
        .blocks
        .iter()
        .zip(found)
        .map(|(block, insns)| match insns {
            Some(insns) => block.with_insns(insns),
            None => block.clone(),
        })
        .collect();
    let mut out = body.with_blocks(blocks);
    let table = Arc::make_mut(&mut out.class_like);
    for (value, old) in like {
        table.insert(value, old);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ir::{Imm, Mem};
    use crate::model::lir::LirBlock;
    use crate::support::hash::IndexMap;

    fn held(value: u32) -> Loc {
        Loc::Held(Held { value, width: 4 })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
        defines: Vec<u32>,
        uses: Vec<u32>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at + 1)), Some(what), defines, uses))
    }

    /// `value := constant`.
    fn constant(value: u32) -> Arc<Insn> {
        let source = Loc::Imm(Imm { value: i64::from(value) * 7, width: 4, address: None });
        insn(i64::from(value), Operation::Move, "mov", vec![held(value)], vec![source], vec![value], vec![])
    }

    /// `value := [base]`: a load, four cycles.
    fn load(
        value: u32,
        base: u32,
    ) -> Arc<Insn> {
        let cell = Mem { base: Some(Held { value: base, width: 4 }), ..Mem::new(None, 4) };
        insn(i64::from(value), Operation::Move, "mov", vec![held(value)], vec![Loc::Mem(cell)], vec![value], vec![base])
    }

    /// `value := left name right`.
    fn binary(
        name: &str,
        value: u32,
        left: u32,
        right: u32,
    ) -> Arc<Insn> {
        let op = if name == "imul" { Operation::Multiply } else { Operation::Binary };
        insn(
            i64::from(value),
            op,
            name,
            vec![held(value)],
            vec![held(left), held(right)],
            vec![value],
            vec![left, right],
        )
    }

    /// `value := source`: a copy, which leaves the flags alone.
    fn copy(
        value: u32,
        source: u32,
    ) -> Arc<Insn> {
        insn(i64::from(value), Operation::Move, "mov", vec![held(value)], vec![held(source)], vec![value], vec![source])
    }

    fn body(insns: Vec<Arc<Insn>>) -> LirBody {
        LirBody::new("combiner", 0, vec![LirBlock::new(0, insns).into()], IndexMap::default(), IndexMap::default())
    }

    /// The leaves 1..=4 and a load `9` (4 cycles) of `1`, ahead of `chain`.
    fn with_leaves(chain: Vec<Arc<Insn>>) -> LirBody {
        let mut insns = vec![constant(1), constant(2), constant(3), constant(4), load(9, 1)];
        insns.extend(chain);
        insns.push(copy(50, 4));
        body(insns)
    }

    fn result(body: &LirBody) -> Vec<Arc<Insn>> {
        body.blocks[0].insns.iter().cloned().collect()
    }

    /// The instruction defining `value`, as `(name, left, right)`.
    fn defining(
        insns: &[Arc<Insn>],
        value: u32,
    ) -> (String, u32, u32) {
        let one = insns.iter().find(|one| one.defines == [value]).expect("defined");
        let what = one.what.as_ref().expect("semantics");
        let [Loc::Held(left), Loc::Held(right)] = &what.sources[..] else { panic!("registers") };
        (what.name.as_deref().map(str::to_owned).expect("named"), left.value, right.value)
    }

    /// What `value` is, the leaves taken as their numbers and the load as 1000.
    fn evaluate(
        insns: &[Arc<Insn>],
        value: u32,
    ) -> i64 {
        let one = insns.iter().find(|one| one.defines == [value]).expect("defined");
        let what = one.what.as_ref().expect("semantics");
        match (&what.sources[..], what.name.as_deref()) {
            ([Loc::Imm(imm)], _) => imm.value,
            ([Loc::Mem(_)], _) => 1000,
            ([Loc::Held(left), Loc::Held(right)], Some(name)) => {
                let (left, right) = (evaluate(insns, left.value), evaluate(insns, right.value));
                match name {
                    "add" => left.wrapping_add(right),
                    "sub" => left.wrapping_sub(right),
                    "xor" => left ^ right,
                    "imul" => left.wrapping_mul(right),
                    other => panic!("{other}"),
                }
            }
            _ => panic!("shape"),
        }
    }

    /// `((1 + load) + 2) + 3`: the load's four cycles ran down the whole
    /// chain, three adds after it.
    fn late_chain(name: &str) -> LirBody {
        with_leaves(vec![binary(name, 10, 1, 9), binary(name, 11, 10, 2), binary(name, 12, 11, 3)])
    }

    /// `((1 + load) + 2) + 3` made the load's four cycles run down the whole
    /// chain; the load joins last in `((1 + 2) + 3) + load` (5 cycles deep
    /// against 7).
    #[test]
    fn a_chain_whose_last_operand_arrives_late_is_rebuilt() {
        for name in ["add", "xor", "imul"] {
            let before = late_chain(name);
            let after = combined(before.clone());
            let (old, new) = (result(&before), result(&after));
            assert_eq!(new.len(), old.len(), "{name}: as many instructions");
            let (op, left, right) = defining(&new, 12);
            assert_eq!(op, name);
            assert!(left == 9 || right == 9, "{name}: the load is an operand of the root: {left} {right}");
            let early = if left == 9 { right } else { left };
            assert_ne!(early, 11, "{name}: the root no longer reads the old middle sum");
            assert_eq!(evaluate(&old, 12), evaluate(&new, 12), "{name}: the value is the same");
        }
    }

    /// The same where the chain is the root's second operand: the commuted
    /// patterns (`REASSOC_AX_YB`).
    #[test]
    fn the_intermediate_as_the_roots_second_operand_is_rebuilt_too() {
        let before = with_leaves(vec![binary("add", 10, 1, 9), binary("add", 11, 2, 10), binary("add", 12, 3, 11)]);
        let after = combined(before.clone());
        let (old, new) = (result(&before), result(&after));
        let (_, left, right) = defining(&new, 12);
        assert!(left == 9 || right == 9, "the load is an operand of the root: {left} {right}");
        assert_eq!(evaluate(&old, 12), evaluate(&new, 12));
    }

    /// Nothing arrives late: `(1 + 2) + 3` is as deep as `1 + (2 + 3)`, and
    /// the pin wants a strictly shallower root.
    #[test]
    fn a_chain_that_would_not_shorten_is_left() {
        let before = with_leaves(vec![binary("add", 10, 1, 2), binary("add", 11, 10, 3)]);
        assert!(result(&combined(before.clone())) == result(&before), "left alone");
    }

    /// The first sum is read again: removing it would change what the second
    /// reader sees (`hasOneNonDBGUse`). Without the reader the pair is rebuilt.
    #[test]
    fn an_intermediate_two_instructions_read_stays() {
        let pair = || vec![binary("add", 10, 1, 9), binary("add", 11, 10, 2)];
        let alone = with_leaves(pair());
        assert!(result(&combined(alone.clone())) != result(&alone), "the control: the pair is rebuilt");
        let mut insns = pair();
        insns.push(binary("xor", 13, 10, 4));
        let shared = with_leaves(insns);
        assert!(result(&combined(shared.clone())) == result(&shared), "left alone");
    }

    /// `sub` is no associative and commutative operation.
    #[test]
    fn a_subtract_chain_is_left() {
        let before = late_chain("sub");
        assert!(result(&combined(before.clone())) == result(&before), "left alone");
    }

    /// A chain of two different operations is not one.
    #[test]
    fn a_mixed_chain_is_left() {
        let before = with_leaves(vec![binary("add", 10, 1, 9), binary("xor", 11, 10, 2), binary("add", 12, 11, 3)]);
        assert!(result(&combined(before.clone())) == result(&before), "left alone");
    }

    /// The flags the middle add sets are read by a branch: new operands would
    /// change them.
    #[test]
    fn an_add_whose_flags_are_read_is_left() {
        let jump = insn(50, Operation::Branch, "jz", vec![], vec![], vec![], vec![]);
        let before =
            with_leaves(vec![binary("add", 10, 1, 9), binary("add", 11, 10, 2), jump, binary("add", 12, 11, 3)]);
        assert!(result(&combined(before.clone())) == result(&before), "left alone");
    }

    /// `inc` leaves CF alone, so a `jb` after it reads the carry of the add
    /// before it: the add counted as dead flags and was rebuilt, and `jb` read
    /// the carry of the new instruction.
    #[test]
    fn an_add_whose_carry_a_later_inc_leaves_for_a_jb_is_left() {
        let inc = insn(51, Operation::Unary, "inc", vec![held(13)], vec![held(1)], vec![13], vec![1]);
        let jump = insn(50, Operation::Branch, "jb", vec![], vec![], vec![], vec![]);
        let before =
            with_leaves(vec![binary("add", 10, 1, 9), binary("add", 11, 10, 2), inc, jump, binary("add", 12, 11, 3)]);
        assert!(result(&combined(before.clone())) == result(&before), "left alone");
    }

    /// The read counts of every instruction were tabled for every body before
    /// a chain was looked for (1.7 M of d_faces -O2, the pin's pass 0.47 M): a
    /// body whose associative operations read none of each other's results
    /// tables nothing, and one with a chain tables once.
    #[test]
    fn a_body_with_no_chain_tables_no_read_counts() {
        let before = facts_made();
        let flat = with_leaves(vec![binary("add", 10, 1, 2), binary("add", 11, 3, 4)]);
        assert!(result(&combined(flat.clone())) == result(&flat), "left alone");
        assert_eq!(facts_made() - before, 0, "tabled for a body with no chain");
        let _ = combined(late_chain("add"));
        assert_eq!(facts_made() - before, 1, "a chain tables once");
    }
}
