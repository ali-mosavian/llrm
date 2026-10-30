//! Step 1 of tail calls (`selectiondag-tail-calls.md`): whether a call is in
//! tail position. `SelectionDAGBuilder::canTailCall` and
//! `isInTailCallPosition` (the pin's `Analysis.cpp`): the call is marked
//! `tail`, the function does not disable tail calls, the block ends in a
//! `ret`, nothing that reads or writes memory or may trap stands between the
//! call and the `ret`, and the `ret` returns the call's value (or nothing, of
//! a call that returns nothing) under the same return attributes.

use llrm_lir::registers::RegId;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{Attribute, BinaryOp, Opcode, Tail};

use super::super::super::isel::{Convention, Parameter};
use super::Builder;

impl Builder<'_> {
    /// Whether the call `inst` is in a position to become a tail call (the
    /// target-independent part; the target's eligibility is step 2).
    pub(super) fn in_tail_position(
        &self,
        inst: InstId,
    ) -> bool {
        tail_position(self.function, inst)
    }
}

impl Builder<'_> {
    /// `X86TargetLowering::isEligibleForSiblingCallOpt` for the 32-bit flat
    /// target: whether the call, in tail position, can reuse the caller's
    /// frame. `plan` is the callee's convention, `variadic` its type's.
    ///
    /// DEVIATE: a caller that returns through a hidden pointer (`sret`) is
    /// refused where the pin asks `mayBeSRetTailCallCompatible`; Win64, PIC
    /// and LFI do not exist here. Owner sdag-a (w9).
    pub(super) fn eligible_for_sibling_call(
        &self,
        info: &llrm_mir::opcode::CallInfo,
        plan: &Convention,
        variadic: bool,
        callee: Option<llrm_mir::GlobalId>,
        plan_first: i64,
        arguments: &[Operand],
    ) -> bool {
        let caller = &self.convention;
        let direct = callee.is_some();
        // the callee returns to our caller: a far callee to a far caller and a
        // near one to a near, or the return address is the wrong size
        let Some(full) = self.arch.full() else { return false };
        let (far, near) = (full.first_argument_offset(true), full.first_argument_offset(false));
        if far != near {
            let callee_far = callee.is_some_and(|id| self.module.global(id).address_space == 1);
            if (caller.first == far) != callee_far {
                return false;
            }
        }
        // a far pointer callee is stored to a cell and called through it
        // (`LLRM_FARCALL`); the jump has no such form yet
        if !direct && self.target.dag.pointer == super::super::mvt::EVT::I16 {
            return false;
        }
        // "no_caller_saved_registers": the frame cannot be replaced
        if llrm_mir::memory::has(&self.function.attrs, "no_caller_saved_registers") {
            return false;
        }
        // a stack that is dynamically re-aligned needs a special epilogue
        let stack = self.arch.stack_alignment().max(1) as u64;
        let types = &self.module.context.types;
        for value in self.info.static_allocas.keys() {
            let ValueDef::Instruction(def) = self.function.value(value).def else { continue };
            if let Opcode::Alloca { allocated, align, .. } = self.function.instruction(def).opcode {
                let wanted =
                    align.unwrap_or_else(|| crate::backend::isel::natural_align(types, self.layout, allocated) as u64);
                if wanted > stack {
                    return false;
                }
            }
        }
        // a struct result through a hidden pointer: the caller's own needs the
        // callee to return it; the callee's is popped by the callee
        let sret = |attrs: &[Vec<Attribute>]| {
            attrs.iter().any(|one| llrm_mir::opcode::argument_class(one) == Some(llrm_mir::opcode::RESULT_POINTER))
        };
        if sret(&self.function.parameter_attrs) || sret(&info.argument_attrs) {
            return false;
        }
        // varargs only through registers
        let stack_parameter = |one: &Parameter| !matches!(one, Parameter::Registers(_));
        if variadic && plan.parameters.iter().any(stack_parameter) {
            return false;
        }
        // results leave the same way
        if plan.returns != caller.returns {
            return false;
        }
        // the callee keeps every register the caller must
        let calling = full.calling();
        let preserved =
            |number: u32| calling.by_number(number).map(|one| full.convention_callee_saved(one)).unwrap_or_default();
        let caller_cc = self.function.calling_convention;
        let caller_kept = preserved(caller_cc);
        if caller_cc != info.calling_convention {
            let callee_kept = preserved(info.calling_convention);
            if !caller_kept.iter().all(|one| callee_kept.contains(one)) {
                return false;
            }
        }
        // the incoming parameter an argument is, if it is one
        let incoming = |operand: &Operand| match operand {
            Operand::Value(value) => match self.function.value(*value).def {
                ValueDef::Argument(at) => caller.parameters.get(at as usize),
                ValueDef::Instruction(_) => None,
            },
            _ => None,
        };
        let has_stack = plan.parameters.iter().any(stack_parameter);
        let eax_edx_ecx = [RegId::EAX, RegId::EDX, RegId::ECX];
        if has_stack || !plan.parameters.is_empty() {
            for (at, parameter) in plan.parameters.iter().enumerate() {
                match parameter {
                    // already laid out as the caller's incoming one
                    Parameter::Cell(_) => {
                        // offsets are from each function's own first stack
                        // argument
                        let moved = Parameter::Cell(match parameter {
                            Parameter::Cell(cell) => cell - plan_first + caller.first,
                            _ => unreachable!(),
                        });
                        if arguments.get(at).and_then(incoming) != Some(&moved) {
                            return false;
                        }
                    }
                    Parameter::Bytes(_) => return false,
                    // `parametersInCSRMatch`: a register the caller must keep
                    // holds the caller's own incoming value
                    Parameter::Registers(regs) => {
                        let kept = regs.iter().any(|reg| caller_kept.iter().any(|(whole, _)| whole == reg));
                        if kept && arguments.get(at).and_then(incoming) != Some(parameter) {
                            return false;
                        }
                    }
                }
            }
            // the callee's address needs a register of the three the arguments
            // may take
            if !direct {
                let used: Vec<RegId> = plan
                    .parameters
                    .iter()
                    .filter_map(|one| if let Parameter::Registers(regs) = one { Some(regs) } else { None })
                    .flatten()
                    .copied()
                    .collect();
                // and not one the caller's epilogue restores before the jump;
                // `TCRETURNri` (emit.rs) takes the address in a register the
                // target's own convention does not keep, so a convention that
                // keeps ecx and edx (Watcom's) has only eax, which the
                // arguments may take: then the call stays a call, as the pin's
                // `IsEligibleForTailCallOptimization` leaves it with no
                // register for the address
                let regs = llrm_lir::registers::Regs(self.arch.registers());
                let native_kept: Vec<RegId> = self
                    .arch
                    .full()
                    .map_or_else(Vec::new, |arch| arch.callee_saved())
                    .into_iter()
                    .filter(|(full, pushed)| full == pushed)
                    .map(|(full, _)| regs.root(full))
                    .collect();
                let free = eax_edx_ecx
                    .iter()
                    .any(
                        |reg| !used.contains(reg)
                            && !caller_kept.iter().any(|(whole, _)| whole == reg)
                            && !native_kept.contains(&regs.root(*reg)),
                    );
                if !free {
                    return false;
                }
            }
        } else if !direct {
            // no arguments: the address has all three registers
        }
        // bytes to pop: the callee pops what the caller does
        if caller.popped > 0 { plan.popped == caller.popped } else { plan.popped == 0 }
    }
}

/// `isInTailCallPosition` over `function`.
pub(super) fn tail_position(
    function: &Function,
    inst: InstId,
) -> bool {
    let call = function.instruction(inst);
    let Opcode::Call(info) = &call.opcode else { return false };
    if !matches!(info.tail, Tail::Tail | Tail::MustTail) {
        return false;
    }
    // `canTailCall`: the caller may opt out
    if info.tail != Tail::MustTail
        && function.attrs.iter().any(|one| {
            matches!(
                one,
                Attribute::Str(key, Some(value)) if key == "disable-tail-calls" && value == "true"
            )
        })
    {
        return false;
    }
    let Some(block) = function.parent(inst) else { return false };
    let Some(ret) = function.terminator(block).filter(|one| function.instruction(*one).opcode == Opcode::Ret) else {
        return false;
    };
    // between the call and the `ret`: only what neither reads memory nor traps
    let mut at = call.link.1;
    while at != 0 {
        let between = InstId(at - 1);
        if between == ret {
            break;
        }
        let one = function.instruction(between);
        let harmless = match &one.opcode {
            Opcode::Binary(op) => !matches!(
                op,
                BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem
            ),
            Opcode::Cast(_)
            | Opcode::GetElementPtr { .. }
            | Opcode::ICmp(_)
            | Opcode::FCmp(_)
            | Opcode::Select
            | Opcode::Freeze
            | Opcode::ExtractValue(_)
            | Opcode::InsertValue(_)
            | Opcode::FNeg => true,
            _ => false,
        };
        if !harmless {
            return false;
        }
        at = one.link.1;
    }
    // `returnTypeIsEligibleForTailCall`: the `ret` returns the call's value
    let returned = function.instruction(ret).operands.first();
    match (call.result, returned) {
        // a `ret void` ignores what the call returns
        (_, None) => true,
        (Some(value), Some(Operand::Value(one))) => value == *one,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::datalayout::DataLayout;

    use super::super::{BuilderTarget, build_block};
    use crate::backend::selectiondag::function_lowering::FunctionLoweringInfo;

    /// How many calls of `f` in `text` are in tail position.
    fn count(text: &str) -> usize {
        let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
        let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
        let (id, _, function) =
            module.functions().find(|(_, g, f)| !f.is_declaration() && g.name.as_deref() == Some("f")).unwrap();
        let convention =
            crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
        let mut info = FunctionLoweringInfo::new(function);
        build_block(
            &module,
            function,
            &layout,
            &BuilderTarget::m32(),
            &llrm_x86_m32::M32,
            &convention,
            &mut info,
            function.entry().unwrap(),
        )
        .unwrap_or_else(|why| panic!("{}", why.0))
        .tail_calls
        .len()
    }

    const HEAD: &str = "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare i32 @g(i32)\n";

    /// `tail call` then `ret` of its value is in tail position; the pin's tail
    /// calls at -O2 (fib, hanoi, recchop) start here.
    #[test]
    fn a_tail_call_followed_by_its_return_is_in_tail_position() {
        assert_eq!(
            count(&format!("{HEAD}define i32 @f(i32 %a) {{\n  %r = tail call i32 @g(i32 %a)\n  ret i32 %r\n}}\n")),
            1
        );
    }

    /// Without the `tail` marker (`tailcallelim` sets it), a call is a call.
    #[test]
    fn an_unmarked_call_is_not() {
        assert_eq!(
            count(&format!("{HEAD}define i32 @f(i32 %a) {{\n  %r = call i32 @g(i32 %a)\n  ret i32 %r\n}}\n")),
            0
        );
    }

    /// A store between the call and the return needs scheduling after the call.
    #[test]
    fn a_store_between_the_call_and_the_return_stops_it() {
        let text = format!(
            "{HEAD}define i32 @f(ptr %p, i32 %a) {{\n  %r = tail call i32 @g(i32 %a)\n  store i32 1, ptr %p\n  ret i32 %r\n}}\n"
        );
        assert_eq!(count(&text), 0);
    }

    /// `ret void` after a call that returns a value: "it doesn't matter what the call's return
    /// type is" (`returnTypeIsEligibleForTailCall`). It was refused, so the pin's `puts`
    /// tail call at the end of a `void` function was a `BL` and `RET`.
    #[test]
    fn a_void_return_after_a_value_returning_call_is_in_tail_position() {
        let text = format!("{HEAD}define void @f(i32 %a) {{\n  %r = tail call i32 @g(i32 %a)\n  ret void\n}}\n");
        let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
        let (_, _, function) =
            module.functions().find(|(_, g, f)| !f.is_declaration() && g.name.as_deref() == Some("f")).unwrap();
        let call = function.iter_block(function.entry().unwrap()).next().unwrap();
        assert!(super::tail_position(function, call));
    }

    /// The return must hand back the call's value, not another.
    #[test]
    fn returning_another_value_stops_it() {
        assert_eq!(
            count(&format!("{HEAD}define i32 @f(i32 %a) {{\n  %r = tail call i32 @g(i32 %a)\n  ret i32 %a\n}}\n")),
            0
        );
    }

    /// `disable-tail-calls` (`canTailCall`) turns it off for the caller.
    #[test]
    fn a_caller_that_disables_tail_calls_has_none() {
        let text = format!(
            "{HEAD}define i32 @f(i32 %a) #0 {{\n  %r = tail call i32 @g(i32 %a)\n  ret i32 %r\n}}\nattributes #0 = {{ \"disable-tail-calls\"=\"true\" }}\n"
        );
        assert_eq!(count(&text), 0);
    }

    /// A call whose arguments are the caller's own, in the slots it received
    /// them in, reuses the caller's frame (the pin's `jmp g`).
    #[test]
    fn arguments_already_in_their_slots_are_a_sibling_call() {
        let text = "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare i32 @g(i32, i32)\n\
            define i32 @f(i32 %a, i32 %b) {\n  %r = tail call i32 @g(i32 %a, i32 %b)\n  ret i32 %r\n}\n";
        assert_eq!(count(text), 1);
    }

    /// Swapped, the arguments are in the wrong slots: the pin calls (llc -O2
    /// i486 `f(a,b) { return g(b,a); }`: `call g`), and so do we.
    #[test]
    fn arguments_in_other_slots_are_not() {
        let text = "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare i32 @g(i32, i32)\n\
            define i32 @f(i32 %a, i32 %b) {\n  %r = tail call i32 @g(i32 %b, i32 %a)\n  ret i32 %r\n}\n";
        assert_eq!(count(text), 0);
    }

    /// A callee that takes more stack than the caller received has no slot
    /// for it.
    #[test]
    fn a_callee_with_more_stack_arguments_is_not() {
        let text = "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare i32 @g(i32, i32, i32)\n\
            define i32 @f(i32 %a, i32 %b) {\n  %r = tail call i32 @g(i32 %a, i32 %b, i32 1)\n  ret i32 %r\n}\n";
        assert_eq!(count(text), 0);
    }

    /// A callee that pops a stack argument (watcall's fifth) where the
    /// caller, in cdecl, does not: the stack would be wrong on return.
    #[test]
    fn a_callee_that_pops_what_the_caller_does_not_is_not() {
        let text = "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare watcallcc i32 @g(i32, i32, i32, i32, i32)\n\
            define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e) {\n  %r = tail call watcallcc i32 @g(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e)\n  ret i32 %r\n}\n";
        assert_eq!(count(text), 0);
    }

    /// Register arguments (watcall: eax, edx, ebx, ecx, the last three kept by
    /// the callee): the kept ones must be the caller's own incoming values.
    #[test]
    fn a_kept_register_must_hold_the_incoming_value() {
        let head = "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare watcallcc i32 @g(i32, i32)\n";
        let same = format!(
            "{head}define watcallcc i32 @f(i32 %a, i32 %b) {{\n  %r = tail call watcallcc i32 @g(i32 %a, i32 %b)\n  ret i32 %r\n}}\n"
        );
        let swapped = format!(
            "{head}define watcallcc i32 @f(i32 %a, i32 %b) {{\n  %r = tail call watcallcc i32 @g(i32 %b, i32 %a)\n  ret i32 %r\n}}\n"
        );
        assert_eq!((count(&same), count(&swapped)), (1, 0));
    }

    /// With sibling calls on, an eligible call is the block's end: a
    /// `TC_RETURN` of the callee and the registers, no call sequence and no
    /// return after it. Off, the call is as it was.
    #[test]
    fn a_sibling_call_is_a_tc_return_when_asked() {
        let text = "target datalayout = \"e-p:32:32-n8:16:32\"\ndeclare i32 @g(i32, i32)\n\
            define i32 @f(i32 %a, i32 %b) {\n  %r = tail call i32 @g(i32 %a, i32 %b)\n  ret i32 %r\n}\n";
        let dump = |on: bool| {
            let module = llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}"));
            let layout = DataLayout::parse(module.datalayout.as_deref().unwrap()).unwrap();
            let (id, _, function) =
                module.functions().find(|(_, g, f)| !f.is_declaration() && g.name.as_deref() == Some("f")).unwrap();
            let convention =
                crate::backend::isel::convention(&module, &layout, id, &llrm_x86_m32::M32).expect("a convention");
            let mut info = FunctionLoweringInfo::new(function);
            build_block(
                &module,
                function,
                &layout,
                &BuilderTarget::m32().with_tail_calls(on),
                &llrm_x86_m32::M32,
                &convention,
                &mut info,
                function.entry().unwrap(),
            )
            .unwrap_or_else(|why| panic!("{}", why.0))
            .dag
            .dump()
        };
        let on = dump(true);
        assert!(on.contains("X86ISD::TC_RETURN") && !on.contains("RET_GLUE") && !on.contains("callseq"), "{on}");
        let off = dump(false);
        assert!(off.contains("X86ISD::CALL") && off.contains("RET_GLUE"), "{off}");
    }
}
