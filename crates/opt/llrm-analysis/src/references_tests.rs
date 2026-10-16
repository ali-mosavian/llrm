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

/// `MemRef::of` kept a second table of copies for each version of the function
/// (an `IdMap` of accesses, refilled after every edit): each access of a new
/// version was copied twice, once to answer and once into that table. 71,679
/// inserts, 61.8 M of d_faces' 3,295 M at -O1, for a table that held nothing
/// `references` does not. One copy an ask, whatever the version.
#[test]
fn test_an_ask_copies_the_kept_reference_once_after_an_edit_too() {
    let mut module = module();
    let _ = asked(&module);
    {
        let (_, function) = module.function_mut("f").unwrap();
        let y = value(function, "y");
        let add =
            function.walk().map(|(_, inst)| inst).find(|&inst| function.instruction(inst).result == Some(y)).unwrap();
        function.set_operand(add, 1, Operand::Value(value(function, "x")));
    }
    let before = copied();
    let (_, _, _) = asked(&module);
    // Two accesses, asked three times each.
    assert_eq!(copied() - before, 6, "an ask of a new version copied more than once");
}
