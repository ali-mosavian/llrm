//! What a callee no summary describes leaves in a cell, as its attributes
//! and its call site's state it: LLVM's `memory(...)`, `readonly`,
//! `writeonly`, `readnone` and `nocapture`.

use llrm_mir::passes::Outer;

use crate::consts::{Calls, Known, known};
use crate::manager::call_effects;
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed, value};

/// What `@f` returns: 7 stored to `cell` before `calls`, then loaded.
fn kept(declarations: &str, cell: &str, calls: &str) -> Option<Known> {
    let module = parsed(&format!(
        "{DOS}@g = global i16 0

{declarations}

define i16 @f() {{
b0:
  %a = alloca i16
  store i16 7, ptr {cell}
  {calls}
  %r = load i16, ptr {cell}
  ret i16 %r
}}
"
    ));
    let layout = layout(&module);
    let outer = Outer::of(&module, None);
    let f = function(&module, "f");
    let unit = crate::testing::with_registers(Unit::within(&module.context, &layout, f, &outer));
    let calls: Calls = call_effects(&unit, &outer).unwrap().into_iter().map(|(at, effect)| (at, effect.stores)).collect();
    known(&unit, Some(&calls), None, None).get(&value(f, "r")).cloned()
}

fn seven() -> Option<Known> {
    Some(Known::new(7, 16))
}

/// Every unsummarized call wrote what its arguments point to. A callee
/// that may keep a copy may write through it, as LLVM's `readonly` allows.
#[test]
fn a_callee_that_only_reads_its_argument_keeps_the_cell() {
    let call = "call void @use(ptr %a)";
    assert_eq!(kept("declare void @use(ptr nocapture readonly)", "%a", call), seven());
    assert_eq!(kept("declare void @use(ptr nocapture readnone)", "%a", call), seven());
    assert_eq!(kept("declare void @use(ptr)", "%a", "call void @use(ptr nocapture readonly %a)"), seven());
    assert_eq!(kept("declare void @use(ptr) memory(argmem: read)", "%a", call), seven());
    assert_eq!(kept("declare void @use(ptr nocapture writeonly)", "%a", call), None);
    assert_eq!(kept("declare void @use(ptr readonly)", "%a", call), None);
    assert_eq!(kept("declare void @use(ptr)", "%a", call), None);
}

/// Every unsummarized call wrote every global.
#[test]
fn a_callee_confined_to_its_arguments_keeps_a_global() {
    let call = "call void @use(ptr %a)";
    assert_eq!(kept("declare void @use(ptr) memory(argmem: readwrite)", "@g", call), seven());
    assert_eq!(kept("declare void @use(ptr) memory(read)", "@g", call), seven());
    assert_eq!(kept("declare void @use(ptr) readonly", "@g", call), seven());
    assert_eq!(kept("declare void @use(ptr)", "@g", "call void @use(ptr %a) memory(argmem: write)"), seven());
    assert_eq!(kept("declare void @use(ptr) memory(write)", "@g", call), None);
    assert_eq!(kept("declare void @use(ptr)", "@g", call), None);
}

/// A pointer passed to any call escaped, so every later call reached it.
#[test]
fn a_pointer_a_callee_keeps_no_copy_of_stays_local() {
    let declared = "declare void @use(ptr) memory(argmem: read)\ndeclare void @other()";
    assert_eq!(kept(declared, "%a", "call void @use(ptr nocapture %a)\n  call void @other()"), seven());
    assert_eq!(kept(&declared.replace("(ptr)", "(ptr nocapture)"), "%a", "call void @use(ptr %a)\n  call void @other()"), seven());
    assert_eq!(kept(declared, "%a", "call void @use(ptr %a)\n  call void @other()"), None);
}
