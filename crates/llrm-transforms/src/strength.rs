//! Strength reduction, as LLVM's LoopStrengthReduce: a value affine in a
//! loop's counter -- a product, an address, an exact quotient -- becomes a
//! recurrence of its own, started in the preheader and stepped by an add
//! (a `getelementptr` for an address) before the latch's branch.
//!
//! Adapted from llrm-core's `optimize/strength.rs`. `induction` says which
//! values are affine and proves the counts; `ssa::SsaUpdater` places the
//! new header phi; `dead` takes what was replaced, as it did after the old
//! pass. Loops go innermost first, each decided on the function as the
//! loops inside it left it.
//!
//! Dropped, with no rich-MIR counterpart:
//! - x86 address forms: `AddressForm`, `_indexable`, `_indexed`,
//!   `_addressed`, `_legal_form`, `_paired`, `_secondary_indexes`,
//!   `_widened` and `_rebased`. Which address folds a counter as an index
//!   is isel's.
//! - Flags: `_stepping_point` and `_live_conditions` kept the step off a
//!   live compare. An `icmp` is a value, so the step goes before the
//!   latch's branch, and every candidate reads the header's value.
//! - Copies: a replaced value is replaced in its uses, so `_copying` and
//!   `_local_pointer_rebases` have nothing to do, and `_already_carried`
//!   reads operands.
//! - An operand in memory (a cell multiplier or offset): a load is its own
//!   instruction, hoisted by LICM.
//! - Ids and placement: `_next`, `_start_temporary_count`, `_woven`,
//!   `_widest` and `_width`'s word default.
//!
//! `Strength` then shares counters (`ivshare`), drops what died and sinks
//! final updates to the exits (`exitsink`), then the rest of its tail:
//! `loopexit::evaluated`, `indvars::rewound` and `indvars::simplified`.
//!
//! llrm-mir's `loopreduce` reduces, through ScalarEvolution, a sum of any
//! of a loop's recurrences, their constant multiples and an address off
//! one, when two operations or more die and no more values stay live. This
//! one reduces one counter's formulas, but also its product by an
//! invariant value, an exact quotient and an extended narrow counter; it
//! prices them against the registers and the loop's pressure, shares one
//! recurrence among equal formulas, and credits a formula that can replace
//! its counter as the loop's control. It does not reduce a sum of two
//! counters.
//!
//! A pointer recurrence is a phi stepped by a `getelementptr`, which
//! `induction::basics` does not recognise: a second run leaves it alone.
//! A new integer recurrence is a counter, and a second run may carry an
//! address off it.

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use llrm_analysis::consts::Known;
use llrm_analysis::manager::Registers;
use llrm_analysis::induction::{self, Affine, AffineMap, AffineOperand, Derived};
use llrm_analysis::occurrence::operations;
use llrm_analysis::ssa::SsaUpdater;
use llrm_analysis::{cfg, liveness, memory};
use llrm_graph::loops::Loop;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses, Unit};
use llrm_mir::types::TypeId;
use llrm_mir::{Constant, ConstantKind};
use llrm_support::hash::{HashMap, HashSet, IndexMap};
use num_bigint::BigInt;

use crate::{dead, exitsink, indvars, ivshare, loopexit};
use crate::profit::OperationCosts;

/// Strength reduction. `registers` of 0 leaves pressure unpriced;
/// `call_registers`, when not 0, is what survives a call in the loop.
#[derive(Default)]
pub struct Strength {
    pub costs: OperationCosts,
    pub registers: i64,
    pub call_registers: i64,
}

impl FunctionPass for Strength {
    fn name(&self) -> &'static str {
        "strength"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let facts = analyses.get::<Registers>(unit.context, unit.layout, unit.function);
        let outer = Rc::clone(analyses.outer());
        let reduced = reduced(unit, &outer, &facts, self.registers, self.call_registers, &self.costs, true);
        let shared = ivshare::shared(unit, &outer);
        let dead = dead::dead(unit.context, unit.callees, unit.function);
        let sunk = exitsink::sunk(unit.function);
        // The tail: exit values evaluated, then the control a credited
        // formula took over, so the counter it replaced dies.
        let evaluated = loopexit::evaluated(unit.context, unit.layout, unit.callees, unit.function, &outer).unwrap_or_else(|error| panic!("strength: {error}"));
        let rewound = indvars::rewound(unit.context, unit.layout, unit.function, analyses, self.registers, &self.costs);
        let simplified = indvars::simplified(unit.context, unit.layout, unit.function, analyses).unwrap_or_else(|error| panic!("strength: {error}"));
        let cleared = (rewound | simplified) && dead::dead(unit.context, unit.callees, unit.function);
        if evaluated {
            PreservedAnalyses::none()
        } else if sunk | dead | reduced | shared | rewound | simplified | cleared {
            // Blocks and edges are as they were.
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// One loop's reductions, decided before anything changes.
struct Plan {
    header: BlockId,
    preheader: BlockId,
    latch: BlockId,
    chosen: Vec<Derived>,
    /// Each credited bare root, with the rest of its stride class.
    classes: Vec<(Derived, Vec<Derived>)>,
}

/// Every multiply of a counter by an invariant, and every formula like it,
/// made an add, the replaced left for `dead`; whether anything changed.
/// `outer` is the function's module, `facts` its values consts knows.
///
/// An inner recurrence's start, in its preheader, is a formula of the
/// loop around it.
#[allow(clippy::too_many_arguments)]
pub fn reduced(
    unit: &mut Unit,
    outer: &Outer,
    facts: &IndexMap<ValueId, Known>,
    registers: i64,
    call_registers: i64,
    costs: &OperationCosts,
    control_recurrences: bool,
) -> bool {
    let mut changed = false;
    let mut found = None::<Vec<Formulas>>;
    let mut visited = BTreeSet::<i64>::new();
    loop {
        let view = memory::Unit::within(unit.context, unit.layout, unit.function, outer);
        let current = found.get_or_insert_with(|| induction::of(&view));
        let Some(index) = current.iter().position(|(one, _, _)| !visited.contains(&one.header)) else { break };
        visited.insert(current[index].0.header);
        let Some(plan) = _plan(&view, facts, &current[index..=index], registers, call_registers, costs, control_recurrences) else { continue };
        if _applied(unit, &plan) {
            changed = true;
            found = None;
        }
    }
    changed
}

/// `plan` carried out; whether it changed anything.
fn _applied(unit: &mut Unit, plan: &Plan) -> bool {
    let mut changed = false;
    // One recurrence a formula.
    let mut shared = HashMap::<(AffineOperand, AffineOperand, AffineOperand, Vec<(AffineOperand, BigInt)>, Option<Operand>, TypeId), Operand>::default();
    let mut carried = BTreeMap::<InstId, Operand>::new();
    for one in &plan.chosen {
        let instruction = unit.function.instruction(one.op);
        let (Some(answer), ty) = (instruction.result, instruction.ty) else { continue };
        let key = (one.of.start.clone(), one.of.step.clone(), one.by.clone(), one.offsets.clone(), one.pointer, ty);
        let recurrence = match shared.get(&key) {
            Some(&recurrence) => recurrence,
            None => {
                let recurrence = _recurrence(unit, plan, one, ty);
                shared.insert(key, recurrence);
                recurrence
            }
        };
        unit.function.replace_all_uses_with(answer, recurrence);
        carried.insert(one.op, recurrence);
        changed = true;
    }
    // A credited root carries its whole stride class: every other formula
    // of its counter and multiplier is an invariant plus the root, however
    // it was spelled.
    for (root, members) in &plan.classes {
        let Some(&recurrence) = carried.get(&root.op) else { continue };
        for member in members {
            if carried.contains_key(&member.op) {
                continue;
            }
            let Some(answer) = unit.function.instruction(member.op).result else { continue };
            let with = if member.offsets.is_empty() {
                recurrence
            } else if _already_carried(unit.function.instruction(member.op), member, recurrence) {
                continue;
            } else {
                let ty = unit.function.instruction(member.op).ty;
                let before = unit.function.terminator(plan.preheader).expect("a preheader's branch");
                let invariant = _starts(unit, member, false, before);
                _emitted(unit, Opcode::Binary(BinaryOp::Add), ty, vec![invariant, recurrence], member.op)
            };
            carried.insert(member.op, with);
            unit.function.replace_all_uses_with(answer, with);
            changed = true;
        }
    }
    changed
}

/// What to reduce in the one loop of `found`, where it has a preheader and a latch.
fn _plan(
    view: &memory::Unit,
    facts: &IndexMap<ValueId, Known>,
    found: &[Formulas],
    registers: i64,
    call_registers: i64,
    costs: &OperationCosts,
    control_recurrences: bool,
) -> Option<Plan> {
    let function = view.function;
    let [(loop_, basics, derived)] = found else { return None };
    // Two ways in or out is a bigger change than this.
    let (Some(preheader), [latch]) = (_preheader(function, loop_), &loop_.latches.iter().copied().collect::<Vec<_>>()[..]) else { return None };
    let groups = BTreeMap::from([(loop_.header, _candidates(function, derived))]);
    let group = &groups[&loop_.header];
    if group.is_empty() {
        return None;
    }
    let credits = if control_recurrences {
        &_replacement_credits(view, found, &groups, facts, costs) | &_control_credits(view, found, &groups, facts, costs)
    } else {
        BTreeSet::new()
    };
    let mut room = group.len() as i64;
    let mut capacity = registers;
    let calls = loop_.body.iter().flat_map(|&at| function.block(cfg::block(at)).instructions()).any(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_) | Opcode::Invoke(_)));
    if call_registers != 0 && calls {
        capacity = if capacity != 0 { capacity.min(call_registers) } else { call_registers };
    }
    if capacity != 0 {
        // The fixed reserve is an upper bound; actual liveness may expose
        // a tighter loop.
        let pressure = liveness::pressure(function, None, Some(&loop_.body)) as i64;
        room = 0.max((capacity - basics.len() as i64 - _RESERVE).min(capacity - pressure));
    }
    // A derived recurrence which can replace its source loop counter does
    // not consume another recurrence slot.
    let chosen = _formula_set(function, group, Some(room), &credits, costs, Some(&_references(function)));
    let classes = group
        .iter()
        .filter(|one| _bare(one) && credits.contains(&one.op))
        .map(|root| (root.clone(), group.iter().filter(|one| one.op != root.op && _same_stride(function, root, one)).cloned().collect()))
        .collect();
    Some(Plan { header: cfg::block(loop_.header), preheader, latch: cfg::block(*latch), chosen, classes })
}

/// The loop's one way in, where that way leads nowhere else.
fn _preheader(function: &Function, loop_: &Loop) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    match outside[..] {
        [one] if function.successors(one) == [header] => Some(one),
        _ => None,
    }
}

/// A new recurrence taking `one`'s values: a header phi from its start in
/// the preheader, stepped before the latch's branch.
fn _recurrence(unit: &mut Unit, plan: &Plan, one: &Derived, ty: TypeId) -> Operand {
    let before = unit.function.terminator(plan.preheader).expect("a preheader's branch");
    let width = one.of.start.width();
    let int = unit.context.types.int(width);
    let stride = match _times(&one.of.step, &one.by) {
        Some(stride) => _operand(unit, &stride),
        None => {
            let (step, by) = (_operand(unit, &one.of.step), _operand(unit, &one.by));
            _emitted(unit, Opcode::Binary(BinaryOp::Mul), int, vec![step, by], before)
        }
    };
    let start = _starts(unit, one, true, before);
    let poison = Operand::Constant(unit.context.constant(Constant { ty, kind: ConstantKind::Poison }));
    let opcode = match one.pointer {
        Some(_) => Opcode::GetElementPtr { source: unit.context.types.int(8) },
        None => Opcode::Binary(BinaryOp::Add),
    };
    let back = unit.function.terminator(plan.latch).expect("a latch's branch");
    let step = unit.function.create_instruction(opcode, ty, vec![poison, stride], Flags::default(), Some("lsr.iv.next"));
    unit.function.insert(step, Position::Before(back)).expect("a placed branch");
    let mut updater = SsaUpdater::new(ty, Some("lsr.iv"));
    updater.add_available_value(plan.preheader, start);
    updater.add_available_value(plan.latch, Operand::Value(unit.function.instruction(step).result.expect("a step's value")));
    let phi = updater.value_in_middle_of_block(unit.context, unit.function, plan.header);
    unit.function.set_operand(step, 0, phi);
    phi
}

/// `opcode` of `operands`, placed before `before`.
pub(crate) fn _emitted(unit: &mut Unit, opcode: Opcode, ty: TypeId, operands: Vec<Operand>, before: InstId) -> Operand {
    let inst = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
    unit.function.insert(inst, Position::Before(before)).expect("a placed instruction");
    Operand::Value(unit.function.instruction(inst).result.expect("a value"))
}

/// A term as an operand.
pub(crate) fn _operand(unit: &mut Unit, term: &AffineOperand) -> Operand {
    match term {
        AffineOperand::Value(value, _) => Operand::Value(*value),
        AffineOperand::Const(known) => {
            let ty = unit.context.types.int(known.width);
            let bits = u128::try_from(&known.n).expect("a masked constant");
            Operand::Constant(unit.context.constant(Constant { ty, kind: ConstantKind::Int(bits) }))
        }
    }
}

/// Which formulas are worth a recurrence: a multiply somewhere in them, an
/// address, a quotient, a shifted sum, or a sum with an invariant value.
fn _candidates(function: &Function, derived: &[Derived]) -> Vec<Derived> {
    // A counter's own step reads as the counter plus its stride.
    let steps = llrm_analysis::occurrence::phis(function)
        .flat_map(|(_, _, phi)| {
            let result = phi.result.expect("a phi's value");
            phi.operands.iter().filter_map(move |operand| match operand {
                Operand::Value(value) => Some((result, *value)),
                _ => None,
            })
        })
        .collect::<BTreeSet<_>>();
    derived
        .iter()
        .filter(|one| {
            let opcode = &function.instruction(one.op).opcode;
            _answer(function, one.op).is_some_and(|answer| !steps.contains(&(one.of.value, answer)))
                && (_multiplies(function, one, derived)
                    || one.pointer.is_some()
                    || *opcode == Opcode::Binary(BinaryOp::SDiv)
                    || !one.offsets.is_empty() && *opcode == Opcode::Binary(BinaryOp::Shl)
                    || *opcode == Opcode::Binary(BinaryOp::Add) && one.offsets.iter().any(|(offset, _)| matches!(offset, AffineOperand::Value(..))))
        })
        .cloned()
        .collect()
}

/// How many invariants die because every read of them is a carried formula.
///
/// A pointer carried in place of `base + counter` holds the register the
/// base held, so it adds no pressure.
fn _released(function: &Function, candidates: &[Derived], selected: &BTreeSet<InstId>, references: &BTreeMap<ValueId, i64>) -> i64 {
    let mut reads = BTreeMap::<ValueId, i64>::new();
    for one in candidates.iter().filter(|one| selected.contains(&one.op)) {
        let bases = one.offsets.iter().filter_map(|(offset, _)| _value(offset)).collect::<BTreeSet<_>>();
        let operands = _operands(function.instruction(one.op)).collect::<BTreeSet<_>>();
        for value in bases.intersection(&operands) {
            *reads.entry(*value).or_insert(0) += 1;
        }
    }
    reads.iter().filter(|(value, count)| references.get(value) == Some(*count)).count() as i64
}

fn _value(term: &AffineOperand) -> Option<ValueId> {
    match term {
        AffineOperand::Value(value, _) => Some(*value),
        AffineOperand::Const(_) => None,
    }
}

/// The values an instruction reads.
fn _operands(instruction: &Instruction) -> impl Iterator<Item = ValueId> + '_ {
    instruction.operands.iter().filter_map(|operand| match operand {
        Operand::Value(value) => Some(*value),
        _ => None,
    })
}

fn _reads(instruction: &Instruction, value: ValueId) -> bool {
    instruction.operands.contains(&Operand::Value(value))
}

/// How many instructions other than phis read each value.
fn _references(function: &Function) -> BTreeMap<ValueId, i64> {
    let mut references = BTreeMap::new();
    for (_, _, op) in operations(function) {
        for value in _operands(op).collect::<BTreeSet<_>>() {
            *references.entry(value).or_insert(0) += 1;
        }
    }
    references
}

#[derive(Clone, Debug)]
struct _Replacement {
    loop_: Loop,
    root: Derived,
    mapping: AffineMap,
    domain: (BigInt, BigInt),
    aliases: BTreeSet<ValueId>,
    allowed: BTreeSet<InstId>,
    rank: (i64, i64, i64),
}

/// A formula that is its counter times its multiplier and nothing more.
fn _bare(one: &Derived) -> bool {
    one.offsets.is_empty() && one.pointer.is_none()
}

/// Whether `one` is `root` plus an invariant: the same counter, multiplier and type.
fn _same_stride(function: &Function, root: &Derived, one: &Derived) -> bool {
    one.of == root.of && one.by == root.by && one.pointer.is_none() && function.instruction(one.op).ty == function.instruction(root.op).ty
}

/// Whether nothing but its value comes of `instruction`.
fn _pure(instruction: &Instruction) -> bool {
    matches!(instruction.opcode, Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_) | Opcode::FCmp(_) | Opcode::GetElementPtr { .. } | Opcode::Select | Opcode::FNeg | Opcode::Freeze)
}

/// What carrying bare `root` replaces: its stride class, and every pure
/// operation of the loop read only by that, which dies with it -- `i - 1`
/// under `(i - 1) * 2`.
fn _stride_cover(function: &Function, loop_: &Loop, root: &Derived, formulas: &[Derived]) -> BTreeSet<InstId> {
    let mut covered = formulas.iter().filter(|one| _same_stride(function, root, one)).map(|one| one.op).collect::<BTreeSet<_>>();
    let mut readers = BTreeMap::<ValueId, Vec<InstId>>::new();
    for (inst, _, op) in operations(function) {
        for value in _operands(op) {
            readers.entry(value).or_default().push(inst);
        }
    }
    let in_phis = llrm_analysis::occurrence::phis(function).flat_map(|(_, _, phi)| _operands(phi).collect::<Vec<_>>()).collect::<BTreeSet<_>>();
    loop {
        let before = covered.len();
        for (inst, block, op) in operations(function) {
            let Some(result) = op.result else { continue };
            if covered.contains(&inst) || !loop_.body.contains(&cfg::id(block)) || !_pure(op) {
                continue;
            }
            let read = readers.get(&result);
            if !in_phis.contains(&result) && read.is_some_and(|read| read.iter().all(|at| covered.contains(at))) {
                covered.insert(inst);
            }
        }
        if covered.len() == before {
            return covered;
        }
    }
}

fn _formula_descendants(function: &Function, root: &Derived, formulas: &[Derived]) -> BTreeSet<InstId> {
    let mut pending = vec![function.instruction(root.op).result.expect("a root has a result")];
    let mut seen = BTreeSet::<ValueId>::new();
    let mut operations = BTreeSet::from([root.op]);
    while let Some(value) = pending.pop() {
        if !seen.insert(value) {
            continue;
        }
        for formula in formulas {
            let op = function.instruction(formula.op);
            if let Some(result) = op.result
                && _reads(op, value)
            {
                operations.insert(formula.op);
                pending.push(result);
            }
        }
    }
    operations
}

/// A loop, its counters, and what they derive: `induction::of`'s item.
type Formulas = (Loop, IndexMap<ValueId, Affine>, Vec<Derived>);

/// The formulas of `candidates` no other of them is computed from.
fn _roots<'a>(function: &Function, candidates: &'a [Derived]) -> Vec<&'a Derived> {
    let results = candidates.iter().filter_map(|one| function.instruction(one.op).result).collect::<BTreeSet<_>>();
    candidates.iter().filter(|one| !_operands(function.instruction(one.op)).any(|value| results.contains(&value))).collect()
}

/// Formulas whose source index is proved replaceable as loop control.
///
/// Separate from `_replacement_credits`: here the formula replaces every
/// data use of `i` and the counted-loop proof says the source is otherwise
/// control-only, so its net pressure cost is zero.
fn _control_credits(
    view: &memory::Unit,
    found: &[Formulas],
    groups: &BTreeMap<i64, Vec<Derived>>,
    facts: &IndexMap<ValueId, Known>,
    costs: &OperationCosts,
) -> BTreeSet<InstId> {
    let function = view.function;
    let mut selected = BTreeMap::<ValueId, ((i64, i64, i64), Derived)>::new();
    for (loop_, _basics, _derived) in found {
        let proofs = induction::counted(view, loop_, Some(facts), true);
        let [proof] = &proofs[..] else { continue };
        let candidates = groups[&loop_.header].iter().filter(|one| one.of == proof.counter).cloned().collect::<Vec<_>>();
        let covers = _roots(function, &candidates)
            .into_iter()
            .map(|root| {
                let descendants = if _bare(root) { _stride_cover(function, loop_, root, &candidates) } else { _formula_descendants(function, root, &candidates) };
                (root, descendants)
            })
            .collect::<Vec<_>>();
        // Roots that together cover `i` free it one per round: each reduced
        // one leaves the rest a smaller cover, until the last covers alone.
        // Worth it only when one of them can take control at a constant
        // bias; a symbolic one costs a register in every address it bases.
        let constant = |root: &Derived| {
            proof.count.is_some()
                && proof.first.is_some()
                && matches!(root.by, AffineOperand::Const(_))
                && root.offsets.iter().all(|(term, _)| matches!(term, AffineOperand::Const(_)))
                && root.pointer.is_none()
        };
        let together = covers.iter().flat_map(|(_, descendants)| descendants.iter().copied()).collect();
        let whole = covers.len() > 1 && covers.iter().any(|(root, _)| constant(root)) && induction::control_replacement(view, loop_, proof, &together).is_some();
        for (order, (root, descendants)) in covers.iter().enumerate() {
            if !whole && induction::control_replacement(view, loop_, proof, descendants).is_none() {
                continue;
            }
            let rank = (descendants.len() as i64, _recompute_cost(function, root, costs), -(order as i64));
            let previous = selected.get(&proof.counter.value);
            if previous.is_none_or(|previous| rank > previous.0) {
                selected.insert(proof.counter.value, (rank, (*root).clone()));
            }
        }
    }
    selected.values().map(|(_rank, root)| root.op).collect()
}

/// The phi in `header` defining `value`.
fn _header_phi(function: &Function, header: i64, value: ValueId) -> Option<InstId> {
    let ValueDef::Instruction(inst) = function.value(value).def else { return None };
    (function.instruction(inst).opcode == Opcode::Phi && function.parent(inst) == Some(cfg::block(header))).then_some(inst)
}

/// The value `phi` takes from `from`.
fn _incoming(function: &Function, phi: InstId, from: i64) -> Option<Operand> {
    function.instruction(phi).operands.chunks(2).find(|pair| pair[1] == Operand::Block(cfg::block(from))).map(|pair| pair[0])
}

/// Root formulas proved to replace their source control recurrences.
///
/// The proof is relational: an equality with another counter is allowed
/// when that counter has a candidate with the identical injective map.
/// Candidate pairs are retained to a fixed point.
fn _replacement_credits(
    view: &memory::Unit,
    found: &[Formulas],
    groups: &BTreeMap<i64, Vec<Derived>>,
    facts: &IndexMap<ValueId, Known>,
    costs: &OperationCosts,
) -> BTreeSet<InstId> {
    let function = view.function;
    let width_of = |inst: InstId| view.context.types.int_bits(function.instruction(inst).ty);
    let bits = |value: ValueId| view.int_bits(Operand::Value(value));
    let mut nodes = Vec::<_Replacement>::new();

    for (loop_, _basics, _derived) in found {
        let Some(count) = induction::trip_count(view, loop_, facts) else { continue };
        let header = cfg::block(loop_.header);
        let branch = function.terminator(header).map(|inst| function.instruction(inst));
        if loop_.latches.len() != 1 || !branch.is_some_and(|branch| branch.opcode == Opcode::Br && branch.operands.len() == 3) {
            continue;
        }
        let latch = *loop_.latches.first().expect("one latch");
        let candidates = &groups[&loop_.header];
        let mut affines = Vec::<&Affine>::new();
        for one in candidates {
            if !affines.contains(&&one.of) {
                affines.push(&one.of);
            }
        }
        for affine in affines {
            let phi = _header_phi(function, loop_.header, affine.value);
            let proof = induction::controlling(view, loop_, affine, facts);
            let (Some(phi), Some(proof)) = (phi, proof) else { continue };
            let Some(Operand::Value(update)) = _incoming(function, phi, latch) else { continue };
            if proof.posttested || proof.width() != affine.start.width() {
                continue;
            }
            let Some(domain) = proof.span() else { continue };
            let ValueDef::Instruction(update) = function.value(update).def else { continue };
            let aliases = BTreeSet::from([affine.value]);

            let same = candidates.iter().filter(|one| &one.of == affine).cloned().collect::<Vec<_>>();
            for (order, root) in _roots(function, &same).into_iter().enumerate() {
                let Some(mapping) = induction::derived_map(root, facts) else { continue };
                let Some(width) = width_of(root.op) else { continue };
                let stride = _times(&root.of.step, &root.by);
                let Some(signed) = stride.as_ref().and_then(|stride| induction::_signed(stride, facts, width)) else { continue };
                if signed == BigInt::from(0) {
                    continue;
                }
                let modulus = BigInt::from(1) << width;
                let magnitude = BigInt::from(signed.magnitude().clone());
                if count >= &modulus / induction::gcd(magnitude, modulus.clone()) {
                    continue;
                }
                let descendants = _formula_descendants(function, root, &same);
                let mut allowed = &descendants | &BTreeSet::from([proof.compare]);
                allowed.insert(update);
                nodes.push(_Replacement {
                    loop_: loop_.clone(),
                    root: root.clone(),
                    mapping,
                    domain: domain.clone(),
                    aliases: aliases.clone(),
                    allowed,
                    rank: (descendants.len() as i64, _recompute_cost(function, root, costs), -(order as i64)),
                });
            }
        }
    }

    let mut aliases = BTreeMap::<ValueId, BTreeSet<ValueId>>::new();
    let mut domains = BTreeMap::<ValueId, (BigInt, BigInt)>::new();
    let mut existing = BTreeMap::<ValueId, HashSet<AffineMap>>::new();
    for (loop_, basics, _derived) in found {
        let latch = if loop_.latches.len() == 1 { loop_.latches.first().copied() } else { None };
        for affine in basics.values() {
            let domain = induction::domain(view, loop_, affine, facts);
            let (Some(domain), Some(_)) = (domain, _header_phi(function, loop_.header, affine.value)) else { continue };
            domains.insert(affine.value, domain);
            aliases.entry(affine.value).or_default().insert(affine.value);
            for alternative in basics.values() {
                if alternative.value == affine.value {
                    continue;
                }
                let relation = induction::relation(affine, alternative, facts);
                let alternative_phi = _header_phi(function, loop_.header, alternative.value);
                let (Some(relation), Some(alternative_phi), Some(latch)) = (relation, alternative_phi, latch) else { continue };
                let Some(update) = _incoming(function, alternative_phi, latch) else { continue };
                let read = operations(function)
                    .filter(|(_, block, _)| loop_.body.contains(&cfg::id(*block)))
                    .any(|(_, _, op)| _reads(op, alternative.value) && op.result.map(Operand::Value) != Some(update));
                if read {
                    existing.entry(affine.value).or_default().insert(relation);
                }
            }
        }
    }

    for node in &nodes {
        for value in &node.aliases {
            aliases.entry(*value).or_default().insert(node.root.of.value);
        }
        domains.insert(node.root.of.value, node.domain.clone());
    }
    let mut by_source = BTreeMap::<ValueId, Vec<usize>>::new();
    for (index, node) in nodes.iter().enumerate() {
        by_source.entry(node.root.of.value).or_default().push(index);
    }

    // An `icmp eq` or `ne` only branches read, of an alias and another counter's.
    let equality = |node: &_Replacement, op: &Instruction, active: &BTreeSet<usize>| -> bool {
        let (Opcode::ICmp(IntPredicate::Eq | IntPredicate::Ne), Some(result)) = (&op.opcode, op.result) else { return false };
        let users = function.users(result);
        if users.is_empty() || users.iter().any(|one| function.instruction(one.user).opcode != Opcode::Br) {
            return false;
        }
        let positions = op
            .operands
            .iter()
            .enumerate()
            .filter(|(_, operand)| matches!(operand, Operand::Value(value) if node.aliases.contains(value) && bits(*value) == Some(node.mapping.width)))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [position] = positions[..] else { return false };
        let Operand::Value(other) = op.operands[1 - position] else { return false };
        if bits(other) != Some(node.mapping.width) {
            return false;
        }
        let mut partners = aliases.get(&other).cloned().unwrap_or_default();
        partners.remove(&node.root.of.value);
        partners.iter().any(|partner| {
            domains
                .get(partner)
                .is_some_and(|domain| node.mapping.injective((&node.domain.0).min(&domain.0), (&node.domain.1).max(&domain.1)))
                && (existing.get(partner).is_some_and(|maps| maps.contains(&node.mapping))
                    || by_source
                        .get(partner)
                        .is_some_and(|indexes| indexes.iter().any(|candidate| active.contains(candidate) && nodes[*candidate].mapping == node.mapping)))
        })
    };

    let valid = |node: &_Replacement, active: &BTreeSet<usize>| -> bool {
        operations(function).filter(|(_, block, _)| node.loop_.body.contains(&cfg::id(*block))).all(|(inst, _, op)| {
            !_operands(op).any(|value| node.aliases.contains(&value)) || node.allowed.contains(&inst) || equality(node, op, active)
        })
    };

    let mut active = (0..nodes.len()).collect::<BTreeSet<_>>();
    loop {
        let rejected = active.iter().copied().filter(|index| !valid(&nodes[*index], &active)).collect::<BTreeSet<_>>();
        if rejected.is_empty() {
            break;
        }
        active = &active - &rejected;
    }

    let mut selected = BTreeMap::<ValueId, usize>::new();
    for index in active {
        let source = nodes[index].root.of.value;
        if selected.get(&source).is_none_or(|chosen| nodes[index].rank > nodes[*chosen].rank) {
            selected.insert(source, index);
        }
    }
    // A final selection may choose a different map from the one which made
    // an equality viable. Remove either side rather than taking speculative
    // pressure credit; the ordinary formula price remains available.
    loop {
        let chosen = selected.values().copied().collect::<BTreeSet<_>>();
        let rejected = selected.iter().filter(|(_, index)| !valid(&nodes[**index], &chosen)).map(|(source, _)| *source).collect::<Vec<_>>();
        if rejected.is_empty() {
            break;
        }
        for source in rejected {
            selected.remove(&source);
        }
    }
    selected.values().map(|index| nodes[*index].root.op).collect()
}

/// Choose whole recurrence formulas, collapsing sibling address forms.
///
/// Carry every address of a shared product, or carry the product and keep
/// the cheap additions; never some of each. `credited` formulas replace
/// their source control recurrence and add no net pressure.
pub(crate) fn _formula_set(
    function: &Function,
    candidates: &[Derived],
    room: Option<i64>,
    credited: &BTreeSet<InstId>,
    costs: &OperationCosts,
    references: Option<&BTreeMap<ValueId, i64>>,
) -> Vec<Derived> {
    let result = |one: &Derived| function.instruction(one.op).result;
    let op = |one: &Derived| function.instruction(one.op);
    // A formula only formulas read is theirs to carry; one read by
    // anything else stays live whatever they do.
    let formulas = candidates.iter().map(|one| one.op).collect::<BTreeSet<_>>();
    let leaf = |value: ValueId| {
        let readers = function.users(value);
        readers.is_empty() || readers.iter().any(|reader| !formulas.contains(&reader.user))
    };
    let mut selected = candidates.iter().filter(|one| result(one).is_some_and(leaf)).map(|one| one.op).collect::<BTreeSet<_>>();
    // A credited root is the exact map which discharges the old counter's
    // uses; carry it and leave its invariant field additions in the loop.
    for root in candidates.iter().filter(|one| credited.contains(&one.op)) {
        if _bare(root) {
            for member in candidates.iter().filter(|one| _same_stride(function, root, one)) {
                selected.remove(&member.op);
            }
            selected.insert(root.op);
            continue;
        }
        let mut pending = vec![result(root).expect("a root has a result")];
        while let Some(value) = pending.pop() {
            for child in candidates {
                if child.of != root.of || !_reads(op(child), value) {
                    continue;
                }
                selected.remove(&child.op);
                if let Some(result) = result(child) {
                    pending.push(result);
                }
            }
        }
        selected.insert(root.op);
    }
    let Some(room) = room else {
        return candidates.iter().filter(|one| selected.contains(&one.op)).cloned().collect();
    };

    let empty = BTreeMap::new();
    let references = references.unwrap_or(&empty);
    let slots = |selected: &BTreeSet<InstId>| {
        selected.iter().filter(|one| !credited.contains(one)).count() as i64 - _released(function, candidates, selected, references)
    };
    while slots(&selected) > room {
        let overflow = slots(&selected) - room;
        let mut choices = Vec::<(i64, i64, i64, &Derived, Vec<&Derived>)>::new();
        for (order, parent) in candidates.iter().enumerate() {
            let Some(value) = result(parent) else { continue };
            let children = candidates.iter().filter(|child| selected.contains(&child.op) && child.of == parent.of && _reads(op(child), value)).collect::<Vec<_>>();
            // A single child saves no pressure by replacing it with its parent.
            if children.len() < 2 {
                continue;
            }
            let gain = children.len() as i64 - if selected.contains(&parent.op) { 0 } else { 1 };
            if gain > 0 {
                let relief = gain.min(overflow);
                let mut cheapest = children.iter().filter_map(|child| result(child)).map(|value| references.get(&value).copied().unwrap_or(1)).collect::<Vec<_>>();
                cheapest.sort();
                cheapest.truncate(relief as usize);
                // Best case for retaining the leaves: allocation spills the
                // least-used children, each a latch update and a reload per
                // use. The extra add charges for exceeding the capacity.
                let spill = cheapest.iter().map(|uses| costs.memory_update + uses * costs.load + costs.add).sum::<i64>();
                let leaf = children.len() as i64 * costs.add;
                let collapsed = costs.add + children.len() as i64 * costs.address;
                let benefit = spill - (collapsed - leaf);
                choices.push((benefit, gain, -(order as i64), parent, children));
            }
        }
        let Some((_benefit, _gain, _order, parent, children)) = choices.into_iter().reduce(|best, one| if (one.0, one.1, one.2) > (best.0, best.1, best.2) { one } else { best }) else {
            break;
        };
        for child in children {
            selected.remove(&child.op);
        }
        selected.insert(parent.op);
    }

    // A lone formula over budget: compare the work it removes with the
    // memory traffic of carrying it as a spilled recurrence.
    while slots(&selected) > room {
        let overflow = candidates.iter().filter(|one| selected.contains(&one.op) && !credited.contains(&one.op)).collect::<Vec<_>>();
        let mut priced = Vec::<(i64, i64, &Derived)>::new();
        for (order, one) in overflow.into_iter().enumerate() {
            let uses = result(one).map_or(1, |value| references.get(&value).copied().unwrap_or(1));
            let spilled = costs.memory_update + uses * costs.load;
            priced.push((_recompute_cost(function, one, costs) - spilled, -(order as i64), one));
        }
        let Some((benefit, _order, loser)) = priced.into_iter().reduce(|best, one| if (one.0, one.1) < (best.0, best.1) { one } else { best }) else {
            break;
        };
        if benefit > 0 {
            break;
        }
        selected.remove(&loser.op);
    }

    candidates.iter().filter(|one| selected.contains(&one.op)).cloned().collect()
}

/// Target-neutral cost of rebuilding a complete affine formula.
///
/// `Derived.op` is only the formula's leaf; `by` and `offsets` are the
/// whole formula.
fn _recompute_cost(function: &Function, one: &Derived, costs: &OperationCosts) -> i64 {
    let opcode = &function.instruction(one.op).opcode;
    if matches!(opcode, Opcode::Binary(BinaryOp::SDiv | BinaryOp::UDiv | BinaryOp::SRem | BinaryOp::URem)) {
        return costs.divide;
    }
    let mut work = match &one.by {
        AffineOperand::Const(constant) => {
            let scale = &constant.n;
            let (zero, unit) = (BigInt::from(0), BigInt::from(1));
            if *scale == zero || *scale == unit {
                0
            } else if (scale & (scale - &unit)) == zero {
                costs.shift
            } else {
                costs.multiply
            }
        }
        AffineOperand::Value(..) => costs.multiply,
    };
    work += one.offsets.len() as i64 * costs.add;
    if one.pointer.is_some() || matches!(opcode, Opcode::GetElementPtr { .. }) {
        work += costs.address;
    }
    work
}

/// Whether `op` already adds `member`'s one invariant to `recurrence`:
/// rewriting it would only spell the same add again.
fn _already_carried(op: &Instruction, member: &Derived, recurrence: Operand) -> bool {
    let unit = matches!(&member.by, AffineOperand::Const(constant) if constant.n == BigInt::from(1));
    let [(AffineOperand::Value(offset, _), coefficient)] = member.offsets.as_slice() else { return false };
    if !unit || member.pointer.is_some() || *coefficient != BigInt::from(1) || op.opcode != Opcode::Binary(BinaryOp::Add) {
        return false;
    }
    let offset = Operand::Value(*offset);
    op.operands[..] == [offset, recurrence] || op.operands[..] == [recurrence, offset]
}

// What a loop's body needs to compute with, beyond the recurrences it
// drives. Two is the knee of the suite sweep, and the operands of an
// expression.
const _RESERVE: i64 = 2;

/// Replace multiplication chains, not cheap shifts needing extra counters.
fn _multiplies(function: &Function, one: &Derived, derived: &[Derived]) -> bool {
    let producers = derived
        .iter()
        .filter(|item| item.of == one.of)
        .filter_map(|item| Some((function.instruction(item.op).result?, item.op)))
        .collect::<BTreeMap<_, _>>();
    let mut pending = vec![one.op];
    let mut seen = BTreeSet::new();
    while let Some(at) = pending.pop() {
        if !seen.insert(at) {
            continue;
        }
        let op = function.instruction(at);
        if op.opcode == Opcode::Binary(BinaryOp::Mul) {
            return true;
        }
        pending.extend(_operands(op).filter_map(|value| producers.get(&value).copied()));
    }
    false
}

/// The counter's value on the way in, `start * by`, computed once. A
/// multiply by one is the start.
fn _start(unit: &mut Unit, one: &Derived, before: InstId) -> Operand {
    let start = _operand(unit, &one.of.start);
    if matches!(&one.by, AffineOperand::Const(constant) if constant.n == BigInt::from(1)) {
        return start;
    }
    let (by, int) = (_operand(unit, &one.by), unit.context.types.int(one.of.start.width()));
    _emitted(unit, Opcode::Binary(BinaryOp::Mul), int, vec![start, by], before)
}

/// `scale * start` plus the invariant offsets, computed before `before`;
/// the offsets alone, the base an index is added to, where not `counted`.
pub(crate) fn _starts(unit: &mut Unit, one: &Derived, counted: bool, before: InstId) -> Operand {
    let unit_by = matches!(&one.by, AffineOperand::Const(constant) if constant.n == BigInt::from(1));
    let width = one.of.start.width();
    let int = unit.context.types.int(width);
    let address = |unit: &mut Unit, pointer: Operand, offset: Operand, ty: TypeId| {
        let i8 = unit.context.types.int(8);
        _emitted(unit, Opcode::GetElementPtr { source: i8 }, ty, vec![pointer, offset], before)
    };
    let ty = unit.function.instruction(one.op).ty;
    // A composed offset needs its initial `i * scale + invariant` built
    // before adding it to the pointer; the direct form would drop it.
    if let (true, true, Some(pointer)) = (unit_by, one.offsets.is_empty(), one.pointer) {
        let start = _operand(unit, &one.of.start);
        return address(unit, pointer, start, ty);
    }
    if one.pointer.is_none() && one.offsets.is_empty() {
        return _start(unit, one, before);
    }
    let mut current = counted.then(|| _start(unit, one, before));
    for (offset, coefficient) in &one.offsets {
        let mut offset = _operand(unit, offset);
        if *coefficient != BigInt::from(1) {
            let scale = _operand(unit, &AffineOperand::constant(coefficient.clone(), width));
            offset = _emitted(unit, Opcode::Binary(BinaryOp::Mul), int, vec![offset, scale], before);
        }
        current = Some(match current {
            None => offset,
            Some(current) => _emitted(unit, Opcode::Binary(BinaryOp::Add), int, vec![current, offset], before),
        });
    }
    let current = current.expect("an offset or the counted start");
    match one.pointer {
        Some(pointer) => address(unit, pointer, current, ty),
        None => current,
    }
}

/// The value `inst` computes, where anything reads it, directly or
/// through phis whose results are read.
fn _answer(function: &Function, inst: InstId) -> Option<ValueId> {
    let result = function.instruction(inst).result?;
    let mut seen = HashSet::<ValueId>::default();
    let mut pending = vec![result];
    while let Some(next) = pending.pop() {
        if !seen.insert(next) {
            continue;
        }
        for one in function.users(next) {
            let user = function.instruction(one.user);
            if user.opcode != Opcode::Phi {
                return Some(result);
            }
            pending.extend(user.result);
        }
    }
    None
}

/// `step * by`, where that can be said without an operation.
///
/// A step of one gives the multiplier itself; two constants give their
/// product. Anything else needs a multiply of two invariants.
fn _times(step: &AffineOperand, by: &AffineOperand) -> Option<AffineOperand> {
    let AffineOperand::Const(step) = step else { return None };
    if step.n == BigInt::from(1) {
        return Some(by.clone());
    }
    match by {
        AffineOperand::Const(by) => Some(AffineOperand::constant(&step.n * &by.n, step.width.max(by.width))),
        AffineOperand::Value(..) => None,
    }
}

#[cfg(test)]
#[path = "strength_tests.rs"]
mod tests;
