//! What the base reference table keeps and what it drops.

use llrm_mir::module::{Module, Operand};

use super::*;
use crate::testing::{DOS, function, layout, parsed, value};

/// A load through `%a = gep %p, 4` and a store through `%p`.
fn module() -> Module {
    parsed(&format!(
        "{DOS}define i16 @f(ptr %p, ptr %q, i16 %i) {{
b0:
  %a = getelementptr i8, ptr %p, i16 4
  %x = load i16, ptr %a
  %y = add i16 %x, %i
  store i16 %y, ptr %p
  ret i16 %y
}}
"
    ))
}

/// The load and the store of `@f`, and what asking for both once and again
/// decomposes.
fn asked(module: &Module) -> (InstId, InstId, usize) {
    let (function, layout) = (function(module, "f"), layout(module));
    let unit = Unit::of(module, &layout, function);
    let before = built();
    let found: Vec<InstId> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| {
            matches!(
                function.instruction(inst).opcode,
                llrm_mir::opcode::Opcode::Load { .. } | llrm_mir::opcode::Opcode::Store { .. }
            )
        })
        .collect();
    for _ in 0..3 {
        for &inst in &found {
            MemRef::of(&unit, inst).expect("an access");
        }
    }
    (found[0], found[1], built() - before)
}

/// 181 solves each decomposed every access afresh: the second and third ask of
/// an access make nothing.
#[test]
fn test_an_access_is_decomposed_once_however_often_it_is_asked() {
    let module = module();
    assert_eq!(asked(&module).2, 2);
}

/// A rewritten operand of the pointer chain re-decomposes the access that reads
/// it, and only that one; an unrelated rewrite re-decomposes nothing.
#[test]
fn test_a_changed_pointer_chain_decomposes_again_and_an_unchanged_one_does_not() {
    let mut module = module();
    assert_eq!(asked(&module).2, 2);
    // The load's chain is `%a`, whose base is `%p`: point it at `%q`.
    {
        let (_, function) = module.function_mut("f").unwrap();
        let q = value(function, "q");
        let a = value(function, "a");
        let gep =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(a)).unwrap();
        function.set_operand(gep, 0, Operand::Value(q));
    }

    // The load decomposes again; the store, whose chain is `%p`, does not.
    assert_eq!(asked(&module).2, 1);
    // An add nothing reads a pointer through changes no reference.
    {
        let (_, function) = module.function_mut("f").unwrap();
        let y = value(function, "y");
        let add =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(y)).unwrap();
        function.set_operand(add, 1, Operand::Value(value(function, "x")));
    }
    assert_eq!(asked(&module).2, 0);
}

/// The kept reference is the one a fresh decomposition makes, after changes.
#[test]
fn test_a_kept_reference_equals_a_fresh_one_after_a_change() {
    let mut module = module();
    let _ = asked(&module);
    {
        let (_, function) = module.function_mut("f").unwrap();
        let (q, a) = (value(function, "q"), value(function, "a"));
        let gep =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(a)).unwrap();
        function.set_operand(gep, 0, Operand::Value(q));
    }
    let (function, layout) = (function(&module, "f"), layout(&module));
    let unit = Unit::of(&module, &layout, function);
    for (_, inst) in function.walk() {
        if let Some(kept) = MemRef::of(&unit, inst) {
            let fresh = MemRef::decomposed(&unit, inst, &mut Vec::new()).unwrap();
            assert_eq!(kept, fresh);
        }
    }
}

/// A value replaced by another everywhere (`replace_value`, which logs
/// `Substituted` before the uses): an access whose chain read it decomposes
/// again, and the kept reference is the fresh one.
#[test]
fn test_a_value_replaced_by_another_drops_the_accesses_that_read_it() {
    let mut module = module();
    let _ = asked(&module);
    {
        let (_, function) = module.function_mut("f").unwrap();
        let (p, q) = (value(function, "p"), value(function, "q"));
        function.replace_value(p, Operand::Value(q));
    }
    assert!(asked(&module).2 >= 1, "the accesses that read %p decompose again");
    let (function, layout) = (function(&module, "f"), layout(&module));
    let unit = Unit::of(&module, &layout, function);
    for (_, inst) in function.walk() {
        if let Some(kept) = MemRef::of(&unit, inst) {
            assert_eq!(kept, MemRef::decomposed(&unit, inst, &mut Vec::new()).unwrap());
        }
    }
}

/// The spaces of the layout `DOS` names: 1 far, 2 selector alone.
fn dos_spaces() -> llrm_mir::spaces::Spaces {
    llrm_mir::spaces::Spaces { far: 1, segment: Some(2), ..llrm_mir::spaces::Spaces::FLAT }
}

/// An access through a far pointer made of a selector (`inttoptr` to the
/// segment space, `addrspacecast` to the far space) reads the selector's
/// definition, two instructions off its pointer: the chain held only %far. The
/// selector replaced by a constant left the kept reference naming it (`segment`
/// a value, no `selector`) while a fresh one has the constant: bench/textfill
/// panicked `LLRM_CHECK_REFERENCES` at every level, and a stale reference
/// drives any transform that asks.
#[test]
fn test_a_selector_replaced_by_a_constant_drops_the_far_access_that_read_it() {
    let mut module = parsed(&format!(
        "{DOS}define i8 @f(i16 %sel) {{
b0:
  %s = inttoptr i16 %sel to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %v = load volatile i8, ptr addrspace(1) %far
  ret i8 %v
}}
"
    ));
    {
        let (function, layout) = (function(&module, "f"), layout(&module));
        let unit = Unit { spaces: dos_spaces(), ..Unit::of(&module, &layout, function) };
        for (_, inst) in function.walk() {
            let _ = MemRef::of(&unit, inst);
        }
    }
    let constant = {
        let (context, function) = module.function_mut("f").unwrap();
        let sel = value(function, "sel");
        let constant = Operand::Constant(context.int(function.value(sel).ty, 0xB800));
        function.replace_value(sel, constant);
        constant
    };
    let (function, layout) = (function(&module, "f"), layout(&module));
    let unit = Unit { spaces: dos_spaces(), ..Unit::of(&module, &layout, function) };
    let load = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(
            function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { .. }
        ))
        .unwrap();
    let kept = MemRef::of(&unit, load).unwrap();
    assert_eq!(kept, MemRef::decomposed(&unit, load, &mut Vec::new()).unwrap());
    assert_eq!(kept.segment, Some(constant), "the kept reference names the selector it was made from");
}
