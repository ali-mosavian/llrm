//! What a call leaves in a global, as the manager's constants see it; and
//! the BC raise's `spared` tests in llrm-qb, on MIR.

use std::rc::Rc;

use llrm_mir::passes::{Analyses, Outer};

use crate::consts::{Calls, Known, known};
use crate::manager::{GlobalsAA, Summaries, ThroughMemory, call_effects};
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed, value};

/// What `@f` returns as the manager's `ThroughMemory` knows it, `@g`
/// stored 7 before `call`.
fn kept(globals: &str, call: &str) -> Option<Known> {
    kept_by(globals, call, true)
}

/// `kept`, or without `Summaries` what `manager::call_effects` leaves.
fn kept_by(globals: &str, call: &str, summarized: bool) -> Option<Known> {
    let module = parsed(&format!(
        "{DOS}{globals}
define i16 @f() {{
b0:
  store i16 7, ptr @g
  {call}
  %r = load i16, ptr @g
  ret i16 %r
}}
"
    ));
    let layout = layout(&module);
    let mut outer = Outer::of(&module, None);
    outer.require::<GlobalsAA>(&module);
    let f = function(&module, "f");
    if !summarized {
        let unit = Unit::within(&module.context, &layout, f, &outer);
        let calls: Calls = call_effects(&unit, &outer).unwrap().into_iter().map(|(at, effect)| (at, effect.stores)).collect();
        return known(&unit, Some(&calls), None, None).get(&value(f, "r")).cloned();
    }
    outer.require::<Summaries>(&module);
    let known = Analyses::new(Rc::new(outer)).get::<ThroughMemory>(&module.context, &layout, f);
    Result::as_ref(&*known).unwrap().get(&value(f, "r")).cloned()
}

fn seven() -> Option<Known> {
    Some(Known::new(7, 16))
}

const PRIVATE: &str = "@g = internal global i16 0\n\ndeclare void @outside(ptr) nocallback\n";

/// Every call reached every global, so none kept a private static's value.
#[test]
fn a_private_global_is_kept_across_a_call_that_cannot_name_it() {
    assert_eq!(kept(PRIVATE, "call void @outside(ptr null)"), seven());
}

#[test]
fn a_private_global_whose_address_leaves_is_forgotten() {
    for leaves in [
        "call void @outside(ptr @g)",
        "store ptr @g, ptr @slot",
        "%n = ptrtoint ptr @g to i16\n  store i16 %n, ptr @word",
        "store i16 ptrtoint (ptr @g to i16), ptr @word",
        "%n = add i16 ptrtoint (ptr @g to i16), 1\n  store i16 %n, ptr @word",
    ] {
        let globals = format!("{PRIVATE}@slot = global ptr null\n@word = global i16 0\n");
        let call = format!("{leaves}\n  call void @outside(ptr null)");
        assert_eq!(kept(&globals, &call), None, "{leaves}");
    }
    let held = format!("{PRIVATE}@held = global ptr @g\n");
    assert_eq!(kept(&held, "call void @outside(ptr null)"), None);
    let elsewhere = format!("{PRIVATE}\ndefine void @h() {{\nb0:\n  call void @outside(ptr @g)\n  ret void\n}}\n");
    assert_eq!(kept(&elsewhere, "call void @outside(ptr null)"), None);
}

#[test]
fn an_external_global_is_forgotten() {
    assert_eq!(kept("@g = global i16 0\n\ndeclare void @outside(ptr) nocallback\n", "call void @outside(ptr null)"), None);
}

/// A callee that may call back runs `@f`, which writes `@g`.
#[test]
fn a_callee_that_may_call_back_forgets_what_the_module_writes() {
    assert_eq!(kept("@g = internal global i16 0\n\ndeclare void @outside(ptr)\n", "call void @outside(ptr null)"), None);
}

const PROCEDURES: &str = "@g = internal global i16 0
@other = internal global i16 0

declare void @outside(ptr) nocallback

define internal void @p() {
b0:
  store i16 1, ptr @other
  call void @outside(ptr null)
  ret void
}

define internal void @writes() {
b0:
  store i16 1, ptr @g
  ret void
}

define internal void @calls() {
b0:
  call void @writes()
  ret void
}
";

/// A procedure calling outside code wrote every global; now what it and
/// its callees store.
#[test]
fn a_global_is_kept_across_a_procedure_that_never_writes_it() {
    assert_eq!(kept(PROCEDURES, "call void @p()"), seven());
    assert_eq!(kept(PROCEDURES, "call void @writes()"), None);
    assert_eq!(kept(PROCEDURES, "call void @calls()"), None);
}

/// Without summaries a body was taken to write only the named globals it
/// lists, so one storing a private global left it known.
#[test]
fn a_body_no_summary_describes_may_write_every_tracked_global() {
    let quiet = PROCEDURES.replace("define internal void @writes()", "define internal void @writes() nocallback");
    assert_eq!(kept_by(&quiet, "call void @writes()", false), None);
}

/// A runtime cell outside code names but never hands out, as QB's b$seg.
const NAMED: &str = "@g = external global i16

declare void @inkey() nocallback
declare void @defseg() nocallback
declare void @unlisted() nocallback

!llrm.named = !{!0}
!llrm.writes = !{!1, !2}

!0 = !{ptr @g}
!1 = !{ptr @inkey}
!2 = !{ptr @defseg, ptr @g}
";

/// Every runtime call counted as a DEF SEG, so a POKE after INKEY$ reloaded
/// b$seg. Only b$seg's listed writers write it; a routine with no list may.
#[test]
fn a_named_global_is_kept_across_a_routine_that_does_not_write_it() {
    assert_eq!(kept(NAMED, "call void @inkey()"), seven());
    assert_eq!(kept(NAMED, "call void @defseg()"), None);
    assert_eq!(kept(NAMED, "call void @unlisted()"), None);
}

/// A routine that may run the program's code writes what it does.
#[test]
fn a_named_global_is_forgotten_across_a_routine_that_may_call_back() {
    let calling_back = NAMED.replace("declare void @inkey() nocallback", "declare void @inkey()");
    let program = format!("{calling_back}\ndefine void @set() {{\nb0:\n  call void @defseg()\n  ret void\n}}\n");
    assert_eq!(kept(&program, "call void @inkey()"), None);
}
