//! What a call site or its callee states (`CallBase::hasFnAttr`).

use llrm_mir::context::Context;
use llrm_mir::facts::Facts;
use llrm_mir::module::{Function, InstId};
use llrm_mir::opcode::{Attribute, Opcode};
use llrm_mir::passes::Outer;

/// The attributes of the call `inst`: the site's, and its callee's where it
/// names one.
fn attributes<'a>(
    outer: &'a Outer,
    context: &Context,
    function: &'a Function,
    inst: InstId,
) -> (&'a [Attribute], Option<&'a [Attribute]>) {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return (&[], None) };
    let declared = llrm_mir::memory::callee(context, function, inst)
        .and_then(|one| outer.globals.get(one.0 as usize))
        .and_then(|one| one.function())
        .map(|one| one.attrs.as_slice());
    (&info.attrs, declared)
}

/// `CallBase::hasFnAttr(name)` for a flag.
pub fn has_flag(
    outer: &Outer,
    context: &Context,
    function: &Function,
    inst: InstId,
    name: &str,
) -> bool {
    let (site, declared) = attributes(outer, context, function, inst);
    llrm_mir::memory::has(site, name) || declared.is_some_and(|attrs| llrm_mir::memory::has(attrs, name))
}

/// `Instruction::mayThrow`: a call or invoke without `nounwind`, a resume.
pub fn may_throw(
    outer: &Outer,
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    match function.instruction(inst).opcode {
        Opcode::Resume => true,
        Opcode::Call(_) | Opcode::Invoke(_) => {
            let (site, declared) = attributes(outer, context, function, inst);
            !(Facts::of(site).no_unwind() || declared.is_some_and(|attrs| Facts::of(attrs).no_unwind()))
        }
        _ => false,
    }
}

/// `Instruction::willReturn` of a call: `willreturn` at the site or on the
/// callee.
pub fn will_return(
    outer: &Outer,
    context: &Context,
    function: &Function,
    inst: InstId,
) -> bool {
    let (site, declared) = attributes(outer, context, function, inst);
    Facts::of(site).will_return() || declared.is_some_and(|attrs| Facts::of(attrs).will_return())
}
