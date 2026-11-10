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
//! an ordinary slot that SROA promotes. Where clang leaves `va_arg` an IR
//! instruction (m68k: `Tti::va_arg_slot`), it is the load, the load of the
//! value, the step by its size and the store the selector's `expandVAArg`
//! makes, written here so the list slot has no user SROA cannot see through.
//!
//! DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27; users: x86-m32, m68k with `va_arg_slot` = 4): the pin lowers
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
        let slot = outer.target().tti().and_then(|tti| tti.va_arg_slot());
        let calls: Vec<(InstId, Intrinsic)> = unit
            .function
            .walk()
            .filter_map(|(_, inst)| {
                let one = crate::local::intrinsic_of(unit.context, &outer.globals, unit.function, inst)?;
                matches!(one, Intrinsic::VaStart | Intrinsic::VaEnd | Intrinsic::VaCopy).then_some((inst, one))
            })
            .collect();
        let reads: Vec<InstId> = match slot {
            Some(_) => unit
                .function
                .walk()
                .filter_map(|(_, inst)| matches!(unit.function.instruction(inst).opcode, Opcode::VaArg).then_some(inst))
                .collect(),
            None => Vec::new(),
        };
        if calls.is_empty() && reads.is_empty() {
            return PreservedAnalyses::all();
        }
        for inst in reads {
            read_lowered(unit, inst, slot.expect("reads exist only with a slot"));
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

/// `%v = va_arg ptr %ap, ty` as `%p = load ptr, %ap; %v = load ty, %p; %n = gep i8, %p, size; store %n, %ap`: the
/// selector's `expandVAArg` with the alignment capped to the slot, so the list pointer is never rounded.
fn read_lowered(
    unit: &mut Unit,
    inst: InstId,
    slot: u32,
) {
    let (ty, list) = {
        let one = unit.function.instruction(inst);
        (one.ty, one.operands[0])
    };
    let types = &unit.context.types;
    let align = unit.layout.align(types, ty).min(u64::from(slot));
    // `expandVAArg`: a power-of-two scalar steps by its store size, else the size rounded to the alignment
    let bytes = unit.layout.store_size(types, ty);
    let step = if bytes.is_power_of_two() { bytes } else { bytes.next_multiple_of(align.max(1)) };
    let pointer = unit.context.types.ptr(0);
    let width = unit.layout.offset_bits(0);
    let (index_type, byte) = (unit.context.types.int(width), unit.context.types.int(8));
    let void = unit.context.types.void();
    let place = |unit: &mut Unit, opcode: Opcode, ty, operands: Vec<Operand>| {
        let one = unit.function.create_instruction(opcode, ty, operands, Flags::default(), None);
        unit.function.insert(one, Position::Before(inst)).expect("a placed instruction");
        unit.function.instruction(one).result.map(Operand::Value)
    };
    let at = place(unit, Opcode::Load { align: None, volatile: false }, pointer, vec![list]).expect("a pointer");
    let value = place(unit, Opcode::Load { align: Some(align), volatile: false }, ty, vec![at]).expect("a value");
    let amount = Operand::Constant(unit.context.int(index_type, step as i128));
    let next = place(unit, Opcode::GetElementPtr { source: byte }, pointer, vec![at, amount]).expect("a pointer");
    place(unit, Opcode::Store { align: None, volatile: false }, void, vec![next, list]);
    let result = unit.function.instruction(inst).result.expect("a va_arg value");
    unit.function.replace_all_uses_with(result, value);
    unit.function.erase(inst).expect("a va_arg of no uses");
}
