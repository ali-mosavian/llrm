//! `llvm.va_start` of a one-pointer `va_list` as a store the mid-end can see.
//!
//! # How it works
//! Where the target's `va_list` is one pointer (x86-32 cdecl), `va_start(ap)`
//! only stores the address of the first variadic argument. The pin keeps the
//! intrinsic until selection, so `ap` is a stack slot whose address escapes
//! into it, and every `va_arg` loads and stores the slot (gcc keeps the
//! pointer in a register). Here `va_start(ap)` becomes `store
//! llrm.vararg.address(), ap`, `va_end(ap)` is dropped (it does nothing for
//! such a list) and `va_copy(d, s)` becomes a load and a store; `ap` is then
//! an ordinary slot that SROA promotes.
//!
//! DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27): the pin lowers
//! the intrinsics in selection only. Off unless
//! `Tuning::promote_va_list`. Exit: the target states `va_list` as a value
//! the pin's mem2reg understands, or the pin does.
//!
//! # Example
//! ```text
//!   call void @llvm.va_start.p0(ptr %ap)      %a = call ptr @llrm.vararg.address.p0()
//!   ...                                  =>   store ptr %a, ptr %ap
//!   call void @llvm.va_end.p0(ptr %ap)
//! ```
//!
//! # Runs
//! First in the mid-end, before `sroa`, in a variadic function.

use llrm_mir::context::{Constant, ConstantKind};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::module::{InstId, Operand};
use llrm_mir::opcode::{CallInfo, Flags, Opcode, Tail};
use llrm_mir::passes::{Analyses, FunctionPass, PreservedAnalyses, Unit};
use llrm_mir::types::Type;

pub struct VaStart;

impl FunctionPass for VaStart {
    fn name(&self) -> &'static str {
        "vastart"
    }

    fn adds_memory_operations(&self) -> bool {
        true
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = std::rc::Rc::clone(analyses.outer());
        if !outer.target().tti().is_some_and(|tti| tti.promote_va_list()) {
            return PreservedAnalyses::all();
        }
        let calls: Vec<(InstId, Intrinsic)> = unit
            .function
            .walk()
            .filter_map(|(_, inst)| {
                let one = crate::local::intrinsic_of(unit.context, &outer.globals, unit.function, inst)?;
                matches!(one, Intrinsic::VaStart | Intrinsic::VaEnd | Intrinsic::VaCopy).then_some((inst, one))
            })
            .collect();
        if calls.is_empty() {
            return PreservedAnalyses::all();
        }
        let pointer = unit.context.types.ptr(0);
        for (inst, which) in calls {
            let list = unit.function.instruction(inst).operands[0];
            match which {
                Intrinsic::VaStart => {
                    let ty = unit.context.types.intern(Type::Function {
                        returns: pointer,
                        parameters: Vec::new(),
                        variadic: false,
                    });
                    let callee = unit.declared.declare("llrm.vararg.address.p0", ty);
                    let callee =
                        Operand::Constant(unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
                    let info = CallInfo {
                        function_type: ty,
                        calling_convention: 0,
                        return_attrs: Vec::new(),
                        argument_attrs: Vec::new(),
                        attrs: Vec::new(),
                        tail: Tail::default(),
                    };
                    let first = unit.function.create_instruction(
                        Opcode::Call(Box::new(info)),
                        pointer,
                        vec![callee],
                        Flags::default(),
                        None,
                    );
                    unit.function.insert(first, Position::Before(inst)).expect("a placed call");
                    let value = Operand::Value(unit.function.instruction(first).result.expect("a pointer"));
                    store(unit, inst, value, list);
                }
                Intrinsic::VaCopy => {
                    let source = unit.function.instruction(inst).operands[1];
                    let load = unit.function.create_instruction(
                        Opcode::Load { align: None, volatile: false },
                        pointer,
                        vec![source],
                        Flags::default(),
                        None,
                    );
                    unit.function.insert(load, Position::Before(inst)).expect("a placed call");
                    let value = Operand::Value(unit.function.instruction(load).result.expect("a pointer"));
                    store(unit, inst, value, list);
                }
                _ => {}
            }
            unit.function.erase(inst).expect("a call of no value");
        }
        PreservedAnalyses::none()
    }
}

fn store(
    unit: &mut Unit,
    before: InstId,
    value: Operand,
    address: Operand,
) {
    let void = unit.function.instruction(before).ty;
    let one = unit.function.create_instruction(Opcode::Store { align: None, volatile: false }, void, vec![value, address], Flags::default(), None);
    unit.function.insert(one, Position::Before(before)).expect("a placed call");
}
