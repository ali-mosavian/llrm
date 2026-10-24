//! Step 1 of tail calls (`selectiondag-tail-calls.md`): whether a call is in
//! tail position. `SelectionDAGBuilder::canTailCall` and
//! `isInTailCallPosition` (the pin's `Analysis.cpp`): the call is marked
//! `tail`, the function does not disable tail calls, the block ends in a
//! `ret`, nothing that reads or writes memory or may trap stands between the
//! call and the `ret`, and the `ret` returns the call's value (or nothing, of
//! a call that returns nothing) under the same return attributes.

use llrm_mir::module::{Function, InstId, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp, Opcode, Tail};

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
        (None, None) => true,
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
}
