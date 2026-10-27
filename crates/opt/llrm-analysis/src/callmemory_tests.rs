//! What a callee no summary describes leaves in a cell, as its attributes
//! and its call site's state it: LLVM's `memory(...)`, `readonly`,
//! `writeonly`, `readnone` and `nocapture`.

use llrm_mir::passes::Outer;

use crate::alias::calls_by_attributes;
use crate::consts::{Calls, Known, known_walked};
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed, value};

/// What `@f` returns: 7 stored to `cell` before `calls`, then loaded.
fn kept(
    declarations: &str,
    cell: &str,
    calls: &str,
) -> Option<Known> {
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
    let unit =
        crate::testing::with_points(crate::testing::with_registers(Unit::within(&module.context, &layout, f, &outer)));
    let calls: Calls = calls_by_attributes(&unit).iter().map(|(at, effect)| (at, effect.stores.clone())).collect();
    known_walked(&unit, &calls).get(&value(f, "r")).cloned()
}

fn seven() -> Option<Known> {
    Some(Known::new(7, 16))
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

/// Each call to an unsummarized function listed its own copy of every global it
/// may write (a one-slice provenance set apiece): 17% of host.c's compile, 1.2
/// G of its 6.8 G call-effects instructions. Calls reaching the same bytes
/// share the one list.
#[test]
fn calls_that_reach_the_same_bytes_share_one_list_of_references() {
    let module = parsed(&format!(
        "{DOS}@g = global i16 0
@h = global i16 0

declare void @use(ptr)

define void @f() {{
b0:
  call void @use(ptr @g)
  call void @use(ptr @g)
  ret void
}}
"
    ));
    let layout = layout(&module);
    let outer = Outer::of(&module, None);
    let f = function(&module, "f");
    let unit = crate::testing::with_registers(Unit::within(&module.context, &layout, f, &outer));
    let effects: Vec<_> = calls_by_attributes(&crate::testing::with_points(unit)).values().cloned().collect();
    assert_eq!(effects.len(), 2);
    assert!(!effects[0].stores.is_empty());
    assert!(std::rc::Rc::ptr_eq(&effects[0].stores, &effects[1].stores), "each call listed its own references");
    assert!(std::rc::Rc::ptr_eq(&effects[0].loads, &effects[1].loads));
}

/// Every call resolved each reference of its list again: hashing all of a
/// shared list at every call site (12% of host.c's through-memory). Calls
/// sharing a list resolve it once.
#[test]
fn calls_sharing_a_list_resolve_it_once() {
    let calls = |count: usize| {
        crate::consts::RESOLVED.with(|asked| asked.set(0));
        let text = "call void @use(ptr @g)\n  ".repeat(count);
        kept("declare void @use(ptr)", "%a", &text);
        crate::consts::RESOLVED.with(|asked| asked.get())
    };
    assert_eq!(calls(8), calls(1), "each call resolved its list again");
}

/// A call argument the callee keeps no copy of (`nocapture`) did not leave
/// its alloca exposed, as the pin's capture tracking: every pointer passed to
/// a call exposed its alloca, so a loop around such a call could not promote
/// the slot (jpeg_single.bas -Os, +51 B of loads and stores kept in the loop).
#[test]
fn a_nocapture_argument_does_not_expose_the_alloca() {
    let exposed = |attrs: &str| {
        let module = parsed(&format!(
            "{DOS}declare void @use(ptr {attrs})

define void @f() {{
b0:
  %a = alloca i16
  call void @use(ptr %a)
  ret void
}}
"
        ));
        let layout = layout(&module);
        let f = function(&module, "f");
        let unit = Unit::of(&module, &layout, f);
        crate::memory::exposed_frames(&unit).contains(&value(f, "a"))
    };
    assert!(exposed(""), "premise: a plain argument exposes the alloca");
    assert!(!exposed("nocapture"));
}
