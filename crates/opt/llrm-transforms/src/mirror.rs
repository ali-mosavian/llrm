//! A pointer that follows a counter which only some trips step: the address
//! `base + k * i` of a target whose addresses take no scale `k`.
//!
//! `i` is stepped on some paths of the loop and joined (`if (a[j] < p) { ...;
//! ++i; }`), so it is no counter for LSR, which reduces `i * k` only for the
//! unconditional ones: every `a[i]` is then `lea bx,[i+i]` and a base
//! register, where the other accesses of the loop want that register too
//! (quicksort's partition at -m16 shuffles bx between its two accesses). The
//! value `base + k * i` moves with `i`: a pointer phi made of the same phis,
//! with the step `k * c` where `i` steps by `c`. The accesses read the pointer.
//! Its geps are not `inbounds`: the pointer moves with every step of `i`, one
//! of which may leave the object where no access follows.
//!
//! The analysis is `induction::step_cycle`; this makes the pointers, as a step
//! of `lsr` (it runs per loop before the counters are chosen). Where the target
//! takes the scale in its addresses (`AddressForm::scales`)
//! there is nothing to save and the loop is left. The counter itself stays, for
//! whatever else reads it; a loop that reads it nowhere loses it to `Dead`.

use std::collections::BTreeSet;

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, StepCycle};
use llrm_analysis::{cfg, memory};
use llrm_mir::context::{Context, signed};
use llrm_mir::dense::IdSet;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, Flags, Opcode};
use llrm_mir::target::AddressForm;
use llrm_mir::types::Type;

use crate::edges;
use crate::lcssa::{arms, from_arms};

/// One pointer family: `base + k * f` for the counters of a cycle.
pub(crate) struct Family {
    base: Operand,
    k: i128,
    /// The `getelementptr`s that read it, and the node each reads.
    reads: Vec<(InstId, ValueId)>,
}

/// A loop's counter, stepped on some paths, and the pointers that follow it.
pub(crate) struct Plan {
    header: BlockId,
    preheader: BlockId,
    cycle: StepCycle,
    family: Family,
    bits: u32,
}

/// The pointers `loop_` can have for a counter some trips step, where the
/// target's address takes no scale `forms[0]` names; none at all for a build
/// for size or a loop with another loop in it.
pub(crate) fn found(
    view: &memory::Unit,
    loop_: &Loop,
    forms: &[AddressForm],
) -> Option<Plan> {
    let function = view.function;
    let form = forms.first()?;
    let bits = (form.index_width * 8) as u32;
    let header = cfg::block(loop_.header);
    let body: IdSet<BlockId> = loop_.body.iter().map(|&at| cfg::block(at)).collect();
    if view.shape().loops.iter().any(|other| other.header != loop_.header && loop_.body.contains(&other.header)) {
        return None;
    }
    let outside: Vec<BlockId> = function.predecessors(header).into_iter().filter(|one| !body.contains(one)).collect();
    let [preheader] = outside[..] else { return None };
    if function.successors(preheader) != [header] {
        return None;
    }
    for phi in edges::phis(function, header) {
        if view.int_bits(Operand::Value(function.instruction(phi).result?)) != Some(bits) {
            continue;
        }
        let Some(cycle) = induction::step_cycle(view, loop_, phi) else { continue };
        if let Some(family) = family_of(view, &body, &cycle, &form.scales, bits) {
            return Some(Plan { header, preheader, cycle, family, bits });
        }
    }
    None
}

/// The pointers the cycle's values are scaled into: `gep inbounds base, y * k`
/// for one base and one `k` the target's addresses do not take.
fn family_of(
    view: &memory::Unit,
    body: &IdSet<BlockId>,
    cycle: &StepCycle,
    scales: &BTreeSet<i64>,
    bits: u32,
) -> Option<Family> {
    let function = view.function;
    let invariant = |operand: Operand| match operand {
        Operand::Value(value) => match function.value(value).def {
            ValueDef::Instruction(inst) => function.parent(inst).is_some_and(|block| !body.contains(&block)),
            _ => true,
        },
        _ => true,
    };
    let constant = |operand: Operand| view.int_constant(operand).map(|value| signed(value, bits));
    let mut families: Vec<Family> = Vec::new();
    for &y in &cycle.nodes {
        for one in function.users(y) {
            let user = function.instruction(one.user);
            if !function.parent(one.user).is_some_and(|block| body.contains(&block)) {
                continue;
            }
            let k = match user.opcode {
                Opcode::Binary(BinaryOp::Mul) => constant(user.operands[1 - one.index as usize]),
                Opcode::Binary(BinaryOp::Shl) if one.index == 0 => {
                    constant(user.operands[1]).and_then(|s| (0..16).contains(&s).then(|| 1i128 << s))
                }
                _ => None,
            };
            let Some(k) = k.filter(|&k| k > 1 && !scales.contains(&(k as i64))) else { continue };
            let Some(scaled) = user.result else { continue };
            let readers: Vec<InstId> = function.users(scaled).iter().map(|one| one.user).collect();
            if readers.is_empty() {
                continue;
            }
            let mut geps = Vec::new();
            let mut base = None;
            for gep in readers {
                let op = function.instruction(gep);
                let Opcode::GetElementPtr { source } = op.opcode else { break };
                if op.operands.len() != 2
                    || op.operands[1] != Operand::Value(scaled)
                    || view.context.types.int_bits(source) != Some(8)
                {
                    break;
                }
                if base.is_some_and(|base| base != op.operands[0]) || !invariant(op.operands[0]) {
                    break;
                }
                // A pointer that is its offset alone: a far or huge one
                // carries.
                let flat =
                    match function.operand_type(view.context, op.operands[0]).map(|ty| view.context.types.get(ty)) {
                        Some(Type::Pointer(space)) => {
                            !view.layout.is_pair(*space) && view.layout.offset_bits(*space) == bits
                        }
                        _ => false,
                    };
                if !flat {
                    break;
                }
                base = Some(op.operands[0]);
                geps.push((gep, y));
            }
            let Some(base) = base else { continue };
            if geps.len() != function.users(scaled).len() {
                continue;
            }
            match families.iter_mut().find(|one| one.base == base && one.k == k) {
                Some(family) => family.reads.extend(geps),
                None => families.push(Family { base, k, reads: geps }),
            }
        }
    }
    families.into_iter().max_by_key(|one| one.reads.len())
}

/// The pointer cycle for the plan, and its accesses reading it.
pub(crate) fn applied(
    context: &mut Context,
    function: &mut Function,
    plan: &Plan,
) {
    let Plan { header, preheader, cycle, family, bits } = plan;
    let (header, preheader) = (*header, *preheader);
    let Some(pointer) = function.operand_type(context, family.base) else { return };
    let index = function.value(cycle.nodes[0]).ty;
    let i8 = context.types.int(8);
    let gep = Opcode::GetElementPtr { source: i8 };
    let k = Operand::Constant(context.int(index, family.k));
    let init = cycle.start;
    // The start: the base moved on by the counter's start times k.
    let end = function.terminator(preheader).expect("a terminated preheader");
    let offset = match init {
        Operand::Constant(id) => match context.get(id).kind {
            llrm_mir::context::ConstantKind::Int(value) => {
                Operand::Constant(context.int(index, signed(value, *bits) * family.k))
            }
            _ => return,
        },
        other => {
            let made = function.create_instruction(
                Opcode::Binary(BinaryOp::Mul),
                index,
                vec![other, k],
                Flags::default(),
                None,
            );
            function.insert(made, Position::Before(end)).expect("the preheader");
            Operand::Value(function.instruction(made).result.expect("a value"))
        }
    };
    let start = function.create_instruction(gep.clone(), pointer, vec![family.base, offset], Flags::default(), None);
    function.insert(start, Position::Before(end)).expect("the preheader");
    let start = Operand::Value(function.instruction(start).result.expect("a value"));
    // Every value of the cycle has its pointer: a phi where it is a phi, a step
    // where it is an add. The phis first, so the steps can name them.
    let mut mirror: Vec<(ValueId, ValueId)> = Vec::new();
    let mut shells: Vec<(InstId, ValueId)> = Vec::new();
    for &node in &cycle.nodes {
        if cycle.adds.iter().any(|(added, _, _)| *added == node) {
            continue;
        }
        let ValueDef::Instruction(original) = function.value(node).def else { continue };
        let block = function.parent(original).expect("a placed phi");
        let phi = function.create_instruction(Opcode::Phi, pointer, Vec::new(), Flags::default(), None);
        let first = function.block(block).instructions()[0];
        function.insert(phi, Position::Before(first)).expect("a block");
        mirror.push((node, function.instruction(phi).result.expect("a phi's value")));
        shells.push((phi, node));
    }
    // The steps, once what each steps has its pointer (a step may step a step).
    loop {
        let mut grew = false;
        for (added, stepped, step) in &cycle.adds {
            if mirror.iter().any(|(node, _)| node == added) {
                continue;
            }
            let Some(&(_, on)) = mirror.iter().find(|(node, _)| node == stepped) else { continue };
            let ValueDef::Instruction(original) = function.value(*added).def else { continue };
            let block = function.parent(original).expect("a placed add");
            let step = i128::try_from(step).unwrap_or(0);
            let amount = Operand::Constant(context.int(index, step * family.k));
            let made = function.create_instruction(
                gep.clone(),
                pointer,
                vec![Operand::Value(on), amount],
                Flags::default(),
                None,
            );
            let before = function.terminator(block).expect("a terminated block");
            function.insert(made, Position::Before(before)).expect("a block");
            mirror.push((*added, function.instruction(made).result.expect("a value")));
            grew = true;
        }
        if !grew {
            break;
        }
    }
    let of = |node: Operand, mirror: &[(ValueId, ValueId)]| match node {
        Operand::Value(value) => {
            mirror.iter().find(|(original, _)| *original == value).map(|&(_, made)| Operand::Value(made))
        }
        _ => None,
    };
    for (phi, node) in shells {
        let ValueDef::Instruction(original) = function.value(node).def else { continue };
        let block = function.parent(original).expect("a placed phi");
        let operands: Option<Vec<(Operand, BlockId)>> = arms(function, original)
            .into_iter()
            .map(|(value, from)| {
                // The way in from before the loop.
                if block == header && !matches!(value, Operand::Value(one) if cycle.nodes.contains(&one)) {
                    return Some((start, from));
                }
                of(value, &mirror).map(|made| (made, from))
            })
            .collect();
        let Some(operands) = operands else { continue };
        function.set_operands(phi, from_arms(&operands));
    }
    for &(read, y) in &family.reads {
        let Some(result) = function.instruction(read).result else { continue };
        let Some(&(_, made)) = mirror.iter().find(|(node, _)| *node == y) else { continue };
        function.replace_all_uses_with(result, Operand::Value(made));
    }
}

#[cfg(test)]
#[path = "mirror_tests.rs"]
mod tests;
