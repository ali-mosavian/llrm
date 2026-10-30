//! A call through a table of functions is a switch of direct calls: LLVM's
//! `JumpTableToSwitch` (`jump-table-to-switch`;
//! [JumpTableToSwitch.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/JumpTableToSwitch.cpp)
//! at `d1106deb71cc`).
//!
//! # How it works
//! A call of `load (gep @table, 0, %i)` is a candidate when `@table` is a
//! constant global with its initializer (`parseJumpTable`), the GEP has one
//! variable index and no constant offset (`collectOffset`), the table's size
//! is a whole number of strides and at most 10 entries
//! (`jump-table-to-switch-size-threshold`), and every entry is a function with
//! a body, of the call's type, of at most 50 instructions
//! (`jump-table-to-switch-function-size-threshold`). The call's block is split
//! at the call; the head ends in `switch %i` with a case `call.k` for each
//! entry and `default.switch.case.unreachable` as its default; `call.k` holds
//! a copy of the call of entry `k` and jumps to the tail, whose phi (for a
//! call with a result) takes the copies' results.
//!
//! ```text
//!   %p = gep [4 x ptr], @t, 0, %i        switch %i, unreachable [0: call.0 ...]
//!   %f = load ptr, %p            ==>     call.0: %r0 = call @f0(%x); br tail
//!   %r = call %f(%x)                     tail:   %r = phi [%r0, call.0] ...
//! ```
//!
//! The pass sits after `cvp` in the simplify phase (PassBuilderPipelines.cpp
//! [L672](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L672)),
//! so the inliner then takes the direct calls.
//!
//! DEVIATE JT-1 (no number, owner w-6): no profile data (`!prof` value
//! profile) and no `!prof` on the switch: llrm has none.
//!
//! # Example
//! The unit test `a_call_through_a_table_is_a_switch_of_direct_calls`.
//!
//! # Runs
//! Slot `jumptableswitch@672`, after `cvp@669`. The pin reads
//! `getInstructionCount` of each callee, which a function pass cannot see
//! here: the pass `wants_sizes`, and the manager counts the functions a global
//! initializer names (a table's entries, the only ones asked about) before the
//! pass runs, and again for each it changes. A program with no function in a
//! constant counts nothing.

use llrm_mir::constantfolding::constant_fold_load_from_const;
use llrm_mir::context::{ConstantId, ConstantKind, Context, GlobalId, signed};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, GlobalKind, InstId, Linkage, Operand, ValueDef};
use llrm_mir::opcode::{Flags, Opcode, Tail};
use llrm_mir::passes::{self, Analyses, FunctionPass, PreservedAnalyses, Size};

use crate::edges;

/// `jump-table-to-switch-size-threshold`.
const TABLE_SIZE: u64 = 10;
/// `jump-table-to-switch-function-size-threshold`.
const FUNCTION_SIZE: usize = 50;

/// `JumpTableToSwitchPass`.
pub struct JumpTableToSwitch;

impl FunctionPass for JumpTableToSwitch {
    fn name(&self) -> &'static str {
        "jumptableswitch"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("jump-table-to-switch")
    }

    fn wants_sizes(&self) -> bool {
        true
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        let declared = &*unit.declared;
        if converted(unit.context, unit.layout, unit.function, &outer, &|id| declared.size_of(id)) {
            PreservedAnalyses::none()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Every call of `function` through a table, as a switch; whether any was.
/// `size` is how big a function is: `Unknown` where no constant names it.
pub fn converted(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &passes::Outer,
    size: &dyn Fn(GlobalId) -> Size,
) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        let mut current = Some(block);
        while let Some(here) = current {
            let Some((call, table)) = function.iter_block(here).collect::<Vec<_>>().into_iter().find_map(|inst| {
                parse_jump_table(context, layout, function, outer, size, inst).map(|table| (inst, table))
            }) else {
                break;
            };
            current = Some(expand_to_switch(function, context, call, &table));
            changed = true;
        }
    }
    changed
}

/// `JumpTableTy`: the index and the function of each entry.
struct Table {
    index: Operand,
    functions: Vec<ConstantId>,
}

/// `parseJumpTable` of the call `inst`, if it is one.
fn parse_jump_table(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    outer: &passes::Outer,
    size_of: &dyn Fn(GlobalId) -> Size,
    inst: InstId,
) -> Option<Table> {
    let call = function.instruction(inst);
    let Opcode::Call(info) = &call.opcode else { return None };
    if info.tail == Tail::MustTail {
        return None;
    }
    // `getCalledFunction()` is null: the callee is a value.
    let Operand::Value(callee) = *call.operands.last()? else { return None };
    let ValueDef::Instruction(load) = function.value(callee).def else { return None };
    let load = function.instruction(load);
    // A simple load: not volatile (MIR has no atomic load).
    let Opcode::Load { volatile: false, .. } = load.opcode else { return None };
    let pointer_type = load.ty;
    let Operand::Value(address) = *load.operands.first()? else { return None };
    let ValueDef::Instruction(gep) = function.value(address).def else { return None };
    let gep = function.instruction(gep);
    let Opcode::GetElementPtr { source } = gep.opcode else { return None };
    let Operand::Constant(base) = *gep.operands.first()? else { return None };
    let ConstantKind::Global(global) = context.get(base).kind else { return None };
    let global = outer.globals.get(global.0 as usize)?;
    let GlobalKind::Variable(variable) = &global.kind else { return None };
    // `isConstant() && hasDefinitiveInitializer()`.
    let initializer = variable.initializer.filter(|_| variable.constant)?;
    if matches!(
        global.linkage,
        Linkage::Weak
            | Linkage::WeakOdr
            | Linkage::LinkOnce
            | Linkage::LinkOnceOdr
            | Linkage::Common
            | Linkage::ExternWeak
            | Linkage::AvailableExternally
    ) {
        return None;
    }
    // `collectOffset`: one variable index, no constant offset.
    let indices: Vec<Option<i128>> = gep.operands[1..]
        .iter()
        .map(|operand| match operand {
            Operand::Constant(id) => match context.get(*id).kind {
                ConstantKind::Int(bits) => {
                    let width = context.types.int_bits(context.get(*id).ty)?;
                    Some(Some(signed(bits, width)))
                }
                ConstantKind::Zero => Some(Some(0)),
                _ => None,
            },
            Operand::Value(_) => Some(None),
            Operand::Block(_) => None,
        })
        .collect::<Option<_>>()?;
    let (constant, variable_indices) = layout.collect_offset(&context.types, source, &indices);
    let [(position, stride)] = variable_indices[..] else { return None };
    if constant != 0 || stride == 0 {
        return None;
    }
    let size = layout.alloc_size(&context.types, variable.ty);
    if size % stride != 0 {
        return None;
    }
    let count = size / stride;
    if count > TABLE_SIZE {
        return None;
    }
    let call_type = info.function_type;
    let functions = (0..count)
        .map(|entry| {
            let found = constant_fold_load_from_const(context, layout, initializer, pointer_type, entry * stride)?;
            let ConstantKind::Global(id) = context.get(found).kind else { return None };
            let GlobalKind::Function(target) = &outer.globals.get(id.0 as usize)?.kind else { return None };
            let Size::Instructions(defined) = size_of(id) else { return None };
            (target.ty == call_type && defined <= FUNCTION_SIZE).then_some(found)
        })
        // Every entry is asked before any is refused: the manager counts them
        // all, once.
        .collect::<Vec<_>>()
        .into_iter()
        .collect::<Option<Vec<_>>>()?;
    Some(Table { index: gep.operands[1 + position], functions })
}

/// `expandToSwitch`: the call `call`, split out, as the switch's calls; the
/// tail block.
fn expand_to_switch(
    function: &mut Function,
    context: &mut Context,
    call: InstId,
    table: &Table,
) -> BlockId {
    let block = function.parent(call).expect("a placed call");
    let tail = edges::split_block_at(function, block, call);
    let branch = function.terminator(block).expect("the split jumps to the tail");
    function.erase(branch).expect("an erasable branch");
    let index_type = function.operand_type(context, table.index).expect("an index value");
    let void = function.instruction(call).result.is_none();
    let result_type = function.instruction(call).ty;
    let void_type = function.instruction(function.terminator(tail).expect("a terminated tail")).ty;

    let unreachable = function.create_block(Some("default.switch.case.unreachable"));
    function.insert_block_before(unreachable, tail).expect("a placed block");
    let stop = function.create_instruction(Opcode::Unreachable, void_type, Vec::new(), Flags::default(), None);
    function.insert(stop, Position::End(unreachable)).expect("a placed block");

    let mut operands = vec![table.index, Operand::Block(unreachable)];
    let mut arms = Vec::new();
    for (entry, &callee) in table.functions.iter().enumerate() {
        let case = function.create_block(Some(&format!("call.{entry}")));
        function.insert_block_before(case, tail).expect("a placed block");
        let copy = function.clone_instruction(call);
        let mut copied = function.instruction(copy).operands.clone();
        *copied.last_mut().expect("a callee") = Operand::Constant(callee);
        function.set_operands(copy, copied);
        function.insert(copy, Position::End(case)).expect("a placed block");
        let jump =
            function.create_instruction(Opcode::Br, void_type, vec![Operand::Block(tail)], Flags::default(), None);
        function.insert(jump, Position::End(case)).expect("a placed block");
        operands.push(Operand::Constant(context.int(index_type, entry as i128)));
        operands.push(Operand::Block(case));
        if let Some(result) = function.instruction(copy).result {
            arms.push((Operand::Value(result), case));
        }
    }
    let switch = function.create_instruction(Opcode::Switch, void_type, operands, Flags::default(), None);
    function.insert(switch, Position::End(block)).expect("a placed block");

    if !void {
        let phi = function.create_instruction(
            Opcode::Phi,
            result_type,
            llrm_analysis::occurrence::from_arms(&arms),
            Flags::default(),
            None,
        );
        function.insert(phi, Position::Before(call)).expect("a placed phi");
        let result = function.instruction(call).result.expect("a result");
        let phi_value = function.instruction(phi).result.expect("a phi result");
        function.replace_all_uses_with(result, Operand::Value(phi_value));
    }
    function.erase(call).expect("an erasable call");
    tail
}

#[cfg(test)]
#[path = "jumptableswitch_tests.rs"]
mod tests;
