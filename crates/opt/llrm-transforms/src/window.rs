//! A huge pointer a counted loop steps through less than the target's
//! window: normalized once before the loop (`Intrinsic::Window`), the loop
//! runs on the far pointer that makes, and no step carries into its
//! selector. A loop that may reach past the window keeps its huge steps.
//!
//! The trips are `induction`'s, the recurrences its `pointers`; the window
//! is the target's (`Machine::huge_window`).

use llrm_analysis::graph::loops::Loop;
use llrm_analysis::induction::{self, PointerRecurrence};
use llrm_analysis::manager::Registers;
use llrm_analysis::{cfg, memory};
use llrm_mir::context::{Constant, ConstantKind, Context, GlobalId};
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{CallInfo, Flags, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::counting;

pub struct Window;

impl FunctionPass for Window {
    fn name(&self) -> &'static str {
        "window"
    }

    fn run(&mut self, unit: &mut Unit, analyses: &mut Analyses) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if windowed(unit, analyses, &outer) { PreservedAnalyses::none() } else { PreservedAnalyses::all() }
    }
}

/// One recurrence to rewrite: where its accesses read it, and the bytes it
/// reaches from its start, `low` inclusive.
struct Found {
    preheader: BlockId,
    header: BlockId,
    recurrence: PointerRecurrence,
    /// Each access through the phi or a constant step from it, by that step.
    reads: Vec<(InstId, usize, i64)>,
    low: i64,
}

/// Each loop's huge recurrences that stay in one window, made far.
pub fn windowed(unit: &mut Unit, analyses: &Analyses, outer: &Outer) -> bool {
    let Some((far, window)) = outer.target().huge_window() else { return false };
    let mut changed = false;
    loop {
        let facts = analyses.fresh().get::<Registers>(unit.context, unit.layout, unit.function);
        let found = {
            let view = memory::Unit::within(unit.context, unit.layout, unit.function, outer).with_registers(&facts);
            let loops = view.shape().loops.clone();
            loops.iter().find_map(|one| _found(&view, one, window))
        };
        let Some(found) = found else { break };
        _rewritten(unit, far, &found);
        crate::dead::dead(unit.context, outer.callees(), unit.function);
        changed = true;
    }
    changed
}

fn _preheader(function: &Function, loop_: &Loop) -> Option<BlockId> {
    let header = cfg::block(loop_.header);
    let outside = function.predecessors(header).into_iter().filter(|&one| !loop_.body.contains(&cfg::id(one))).collect::<Vec<_>>();
    match outside[..] {
        [one] if function.successors(one) == [header] => Some(one),
        _ => None,
    }
}

fn _found(view: &memory::Unit, loop_: &Loop, window: i64) -> Option<Found> {
    let function = view.function;
    let preheader = _preheader(function, loop_)?;
    let trips = induction::trip_count(view, loop_, &view.registers())?.to_i64()?;
    let within = |inst: InstId| function.parent(inst).is_some_and(|block| loop_.body.contains(&cfg::id(block)));
    induction::pointers(view, loop_).into_iter().find_map(|recurrence| {
        let space = view.space(Operand::Value(recurrence.value))?;
        if !view.layout.carries(space) {
            return None;
        }
        // The step is read by the phi alone.
        let stepped = function.instruction(recurrence.stepping).result?;
        if function.users(stepped).iter().any(|one| one.user != recurrence.phi) {
            return None;
        }
        let mut reads = Vec::new();
        let (mut low, mut high) = (i64::MAX, i64::MIN);
        let mut read = |user: InstId, index: usize, by: i64, reads: &mut Vec<(InstId, usize, i64)>| -> Option<()> {
            let op = function.instruction(user);
            let ty = match (&op.opcode, index) {
                (Opcode::Load { .. }, 0) => op.ty,
                (Opcode::Store { .. }, 1) => function.operand_type(view.context, op.operands[0])?,
                _ => return None,
            };
            let width = i64::try_from(view.layout.store_size(&view.context.types, ty)).ok()?;
            (low, high) = (low.min(by), high.max(by + width));
            reads.push((user, index, by));
            Some(())
        };
        for one in function.users(recurrence.value) {
            if one.user == recurrence.stepping {
                continue;
            }
            if !within(one.user) {
                return None;
            }
            let op = function.instruction(one.user);
            match op.opcode {
                Opcode::GetElementPtr { source } if one.index == 0 => {
                    let indices = op.operands[1..].iter().map(|&index| view.int_constant(index).map(|bits| llrm_mir::context::signed(bits, view.int_bits(index).unwrap_or(128)))).collect::<Vec<_>>();
                    let (by, variable) = view.layout.collect_offset(&view.context.types, source, &indices);
                    if !variable.is_empty() || indices.iter().any(Option::is_none) {
                        return None;
                    }
                    let address = op.result?;
                    for access in function.users(address) {
                        read(access.user, access.index as usize, i64::try_from(by).ok()?, &mut reads)?;
                    }
                }
                _ => read(one.user, one.index as usize, 0, &mut reads)?,
            }
        }
        if reads.is_empty() {
            return None;
        }
        // Every trip's value, and the one the exit leaves with.
        let step = recurrence.step.to_i64()?;
        let last = step.checked_mul(trips)?;
        let (low, high) = (low + last.min(0), high + last.max(0));
        (high - low <= window).then(|| Found { preheader, header: cfg::block(loop_.header), recurrence, reads, low })
    })
}

/// `found`'s recurrence on a pointer in `space`, from the window its start makes.
fn _rewritten(unit: &mut Unit, space: u32, found: &Found) {
    let (context, declared, function, layout) = (&mut *unit.context, &mut *unit.declared, &mut *unit.function, unit.layout);
    let recurrence = &found.recurrence;
    let huge = function.value(recurrence.value).ty;
    let Type::Pointer(huge_space) = *context.types.get(huge) else { unreachable!("a pointer recurrence") };
    let far = context.types.ptr(space);
    let byte = context.types.int(8);
    let gep = Opcode::GetElementPtr { source: byte };
    let entering = Position::Before(function.terminator(found.preheader).expect("a preheader's branch"));
    let placed = |function: &mut Function, opcode: Opcode, ty: TypeId, operands: Vec<Operand>, at: Position| {
        let inst = function.create_instruction(opcode, ty, operands, Flags::default(), None);
        function.insert(inst, at).expect("a placed position");
        Operand::Value(function.instruction(inst).result.expect("a value"))
    };
    let (huge_index, far_index) = (layout.pointer(huge_space).index_bits, layout.pointer(space).index_bits);
    // The lowest byte reached, normalized, then back to the start.
    let mut start = recurrence.start;
    if found.low != 0 {
        let low = counting::constant(context, &BigInt::from(found.low), huge_index);
        start = placed(function, gep.clone(), huge, vec![start, low], entering);
    }
    let (callee, function_type) = _declared(context, declared, far, huge);
    let ty = context.types.ptr(0);
    let callee = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Global(callee) }));
    let info = CallInfo { function_type, calling_convention: 0, return_attrs: Vec::new(), argument_attrs: vec![Vec::new()], attrs: Vec::new(), tail: Default::default() };
    let mut origin = placed(function, Opcode::Call(Box::new(info)), far, vec![start, callee], entering);
    if found.low != 0 {
        let back = counting::constant(context, &BigInt::from(-found.low), far_index);
        origin = placed(function, gep.clone(), far, vec![origin, back], entering);
    }
    // The far recurrence, stepped where the huge one was.
    let first = function.block(found.header).instructions()[0];
    let phi = function.create_instruction(Opcode::Phi, far, Vec::new(), Flags::default(), Some("window"));
    function.insert(phi, Position::Before(first)).expect("a header");
    let value = Operand::Value(function.instruction(phi).result.expect("a phi's value"));
    let step = counting::constant(context, &recurrence.step, far_index);
    let next = placed(function, gep.clone(), far, vec![value, step], Position::Before(recurrence.stepping));
    let arms = function.instruction(recurrence.phi).operands.clone();
    let operands = arms.chunks(2).flat_map(|pair| [if pair[1] == Operand::Block(found.preheader) { origin } else { next }, pair[1]]).collect();
    function.set_operands(phi, operands);
    // Each access through it, at its own step.
    for &(user, index, by) in &found.reads {
        let address = if by == 0 {
            value
        } else {
            let by = counting::constant(context, &BigInt::from(by), far_index);
            placed(function, gep.clone(), far, vec![value, by], Position::Before(user))
        };
        function.set_operand(user, index, address);
    }
}

/// `Intrinsic::Window` from `huge` pointers to `far` ones, declared where the module has none.
fn _declared(context: &mut Context, declared: &mut Declared, far: TypeId, huge: TypeId) -> (GlobalId, TypeId) {
    let ty = context.types.intern(Type::Function { returns: far, parameters: vec![huge], variadic: false });
    (declared.declare(&llrm_mir::intrinsics::window_name(&context.types, far, huge), ty), ty)
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
