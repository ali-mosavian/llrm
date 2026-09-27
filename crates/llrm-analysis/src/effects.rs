//! Adapted from llrm-core's `analysis/effects.rs`: conservative memory
//! effects not described by an instruction's own operands.
//!
//! A load or store names its bytes. A call states the rest as LLVM does,
//! with `memory(...)`, `readnone`, `readonly` or `writeonly` at the call site
//! and on its callee, as `llrm_mir::memory` reads them, where the old code read the
//! raise's `memory_complete` mark. A volatile access is the old barrier.
//!
//! Division is C's and floating exceptions are the machine's, so the old
//! list of trapping kinds and the body-wide `handles_errors` have no MIR
//! meaning: a raise reaches a handler in this body only along an `invoke`'s
//! unwind edge, and `exposes_memory` asks for that edge.

pub use llrm_mir::memory::callee;
use llrm_mir::memory::{Effects, has, stated_at};
use llrm_mir::module::{Function, GlobalValue, InstId, Module};
use llrm_mir::opcode::Opcode;
use llrm_mir::Context;

/// Every global as its declaration, by id, which a call to it reads:
/// gathered once, since a pass holds its own function mutably while it asks
/// about the others. `llrm_mir::memory` says what their attributes mean.
pub type Declarations = [GlobalValue];

pub fn declarations(module: &Module) -> Vec<GlobalValue> {
    module.globals.iter().map(GlobalValue::declaration).collect()
}

fn declared<'a>(context: &Context, declarations: &'a Declarations, function: &Function, inst: InstId) -> Option<&'a Function> {
    declarations.get(callee(context, function, inst)?.0 as usize).and_then(GlobalValue::function)
}

/// Whether the call `inst` or its callee carries the attribute `flag`.
pub fn states(context: &Context, declarations: &Declarations, function: &Function, inst: InstId, flag: &str) -> bool {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return false };
    has(&info.attrs, flag) || declared(context, declarations, function, inst).is_some_and(|one| has(&one.attrs, flag))
}

/// What the call `inst` may do to locations `counted` admits: what both the
/// call site and the callee allow.
fn call_effects(context: &Context, declarations: &Declarations, function: &Function, inst: InstId, counted: impl Fn(Option<&str>) -> bool + Copy) -> Effects {
    let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(inst).opcode else { return Effects::NONE };
    let site = stated_at(&info.attrs, counted);
    let declared = declared(context, declarations, function, inst).map_or(Effects::ANY, |one| stated_at(&one.attrs, counted));
    Effects { reads: site.reads && declared.reads, writes: site.writes && declared.writes }
}

/// What an instruction may do to memory its operands do not name.
fn unmodeled(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> Effects {
    match function.instruction(inst).opcode {
        Opcode::Load { volatile: true, .. } | Opcode::Store { volatile: true, .. } => Effects::ANY,
        Opcode::Call(_) | Opcode::Invoke(_) => call_effects(context, declarations, function, inst, |location| location != Some("argmem")),
        _ => Effects::NONE,
    }
}

/// Whether an instruction may write memory its operands do not name.
pub fn unmodeled_write(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> bool {
    unmodeled(context, declarations, function, inst).writes
}

/// Whether an instruction may read memory its operands do not name.
pub fn unmodeled_read(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> bool {
    unmodeled(context, declarations, function, inst).reads
}

/// What the call `inst` may do to memory, through its arguments or
/// otherwise; none where `inst` is no call.
pub fn call(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> Effects {
    call_effects(context, declarations, function, inst, |_| true)
}

/// Whether the call `inst` may touch memory at all.
pub fn touches_memory(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> bool {
    call(context, declarations, function, inst) != Effects::NONE
}

/// Whether the call `inst` may write memory anywhere, inaccessible
/// memory included.
pub fn writes_memory(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> bool {
    call(context, declarations, function, inst).writes
}

/// Whether a raise here can reach a handler in this body, which reads memory.
pub fn exposes_memory(context: &Context, declarations: &Declarations, function: &Function, inst: InstId) -> bool {
    matches!(function.instruction(inst).opcode, Opcode::Invoke(_)) && !states(context, declarations, function, inst, "nounwind")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{function, parsed};

    #[test]
    fn a_calls_unmodeled_memory_is_what_its_attributes_leave_open() {
        let module = parsed(
            "declare void @anything()
declare void @arguments(ptr) memory(argmem: readwrite)
declare i16 @reads() memory(read)

define void @f(ptr %p) {
b:
  call void @anything()
  call void @arguments(ptr %p)
  %r = call i16 @reads()
  call void @anything() memory(none)
  store volatile i16 1, ptr %p
  store i16 1, ptr %p
  ret void
}
",
        );
        let declarations = declarations(&module);
        let f = function(&module, "f");
        let insts: Vec<InstId> = f.walk().map(|(_, inst)| inst).collect();
        let writes: Vec<bool> = insts.iter().map(|&inst| unmodeled_write(&module.context, &declarations, f, inst)).collect();
        let reads: Vec<bool> = insts.iter().map(|&inst| unmodeled_read(&module.context, &declarations, f, inst)).collect();
        assert_eq!(writes, [true, false, false, false, true, false, false]);
        assert_eq!(reads, [true, false, true, false, true, false, false]);
    }

    /// Per instruction of `@f`: (unmodeled read, unmodeled write, touches memory).
    fn answers(text: &str) -> Vec<(bool, bool, bool)> {
        let module = parsed(text);
        let declarations = declarations(&module);
        let f = function(&module, "f");
        let context = &module.context;
        f.walk()
            .map(|(_, inst)| {
                (
                    unmodeled_read(context, &declarations, f, inst),
                    unmodeled_write(context, &declarations, f, inst),
                    touches_memory(context, &declarations, f, inst),
                )
            })
            .collect()
    }

    #[test]
    fn a_call_is_as_narrow_as_both_its_site_and_its_callee_allow() {
        let found = answers(
            "declare void @arguments(ptr) memory(argmem: readwrite)
declare void @anything(ptr)

define void @f(ptr %p) {
b:
  call void @arguments(ptr %p) memory(read)
  call void @anything(ptr %p) memory(argmem: read)
  call void @anything(ptr %p) readonly
  call void @anything(ptr %p) writeonly
  ret void
}
",
        );
        assert_eq!(found[0], (false, false, true), "only its arguments, only read");
        assert_eq!(found[1], (false, false, true));
        assert_eq!(found[2], (true, false, true));
        assert_eq!(found[3], (false, true, true));
    }

    /// Effects and `memory::of` read one statement of a callee's memory: a
    /// `writeonly` call read memory for `of`, not for effects.
    #[test]
    fn effects_agree_with_what_memory_says_a_call_does() {
        let module = parsed(
            "declare void @anything(ptr)
declare void @reads(ptr) readonly
declare void @writes(ptr) memory(write)
declare void @none(ptr) readnone

define void @f(ptr %p) {
b:
  call void @anything(ptr %p) writeonly
  call void @anything(ptr %p)
  call void @reads(ptr %p)
  call void @writes(ptr %p)
  call void @none(ptr %p)
  ret void
}
",
        );
        let declarations = declarations(&module);
        let callees = llrm_mir::memory::callees(&module);
        let f = function(&module, "f");
        let context = &module.context;
        for (_, inst) in f.walk().filter(|&(_, inst)| matches!(f.instruction(inst).opcode, Opcode::Call(_))) {
            let of = llrm_mir::memory::of(context, &callees, f, inst);
            let effects = Effects { reads: unmodeled_read(context, &declarations, f, inst), writes: unmodeled_write(context, &declarations, f, inst) };
            assert_eq!(effects, of, "{inst:?}");
        }
    }

    #[test]
    fn a_readnone_or_memory_none_callee_touches_nothing() {
        let found = answers(
            "declare i16 @flag(i16) readnone
declare i16 @none(i16) memory(none)

define void @f() {
b:
  %a = call i16 @flag(i16 1)
  %b = call i16 @none(i16 1)
  ret void
}
",
        );
        assert_eq!(found[..2], [(false, false, false); 2]);
    }

    #[test]
    fn inaccessible_memory_is_unmodeled() {
        let found = answers(
            "declare void @state() memory(inaccessiblemem: write)

define void @f() {
b:
  call void @state()
  ret void
}
",
        );
        assert_eq!(found[0], (false, true, true));
    }

    #[test]
    fn an_indirect_call_may_touch_anything_unless_its_site_says_otherwise() {
        let found = answers(
            "define void @f(ptr %callee) {
b:
  call void %callee()
  call void %callee() memory(none)
  ret void
}
",
        );
        assert_eq!(found[0], (true, true, true));
        assert_eq!(found[1], (false, false, false));
    }

    #[test]
    fn plain_accesses_and_arithmetic_are_modeled_but_volatile_ones_are_not() {
        let found = answers(
            "define void @f(ptr %p, ptr addrspace(1) %far) {
b:
  %v = load i16, ptr %p
  %w = load volatile i16, ptr addrspace(1) %far
  %x = add i16 %v, %w
  store i16 %x, ptr addrspace(1) %far
  ret void
}
",
        );
        assert_eq!(found.iter().map(|&(read, write, _)| (read, write)).collect::<Vec<_>>(), [
            (false, false),
            (true, true),
            (false, false),
            (false, false),
            (false, false)
        ]);
        assert!(found.iter().all(|&(_, _, touches)| !touches), "touches_memory asks only of calls");
    }

    #[test]
    fn only_an_invoke_that_may_unwind_exposes_memory_to_a_handler() {
        let module = parsed(
            "declare void @may()
declare void @never() nounwind
declare i32 @__gxx_personality_v0(...)

define void @f() personality ptr @__gxx_personality_v0 {
b:
  call void @may()
  invoke void @may() to label %ok unwind label %pad

ok:
  invoke void @never() to label %done unwind label %pad

done:
  ret void

pad:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
}
",
        );
        let declarations = declarations(&module);
        let f = function(&module, "f");
        let exposes = f.walk().map(|(_, inst)| exposes_memory(&module.context, &declarations, f, inst)).collect::<Vec<_>>();
        assert_eq!(exposes[..3], [false, true, false]);
    }
}
