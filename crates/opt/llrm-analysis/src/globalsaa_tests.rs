//! What a call leaves in a global, as the manager's constants see it; and
//! the BC raise's `spared` tests in llrm-qb, on MIR.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::passes::{Analyses, ModuleAnalyses, Outer};
use llrm_mir::program::{Exports, Program, ProgramAnalyses};
use llrm_mir::target::Neutral;

use crate::consts::{Calls, Known, known};
use crate::manager::{GlobalsAA, ProgramSummaries, Summaries, ThroughMemory, call_effects};
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed, value};

/// What `@f` returns as the manager's `ThroughMemory` knows it, `@g`
/// stored 7 before `call`.
fn kept(globals: &str, call: &str) -> Option<Known> {
    kept_by(globals, call, true)
}

/// `kept`, or without `Summaries` what `manager::call_effects` leaves.
fn kept_by(globals: &str, call: &str, summarized: bool) -> Option<Known> {
    kept_under(globals, "", call, summarized)
}

/// `kept_by`, the program linked against the declarations `runtime`.
fn kept_under(globals: &str, runtime: &str, call: &str, summarized: bool) -> Option<Known> {
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
    let program = Program::new(vec![module], Rc::new(Neutral)).unwrap().with_runtime(parsed(&format!("{DOS}{runtime}"))).unwrap();
    let module = &program.modules[0];
    let mut modules = ModuleAnalyses::new(ProgramAnalyses::default().proxy(&program, 0));
    modules.require::<GlobalsAA>();
    if summarized {
        modules.require::<Summaries>();
    }
    let outer = modules.outer(module);
    let f = function(module, "f");
    if !summarized {
        let unit = Unit::within(&module.context, &layout, f, &outer);
        let calls: Calls = call_effects(&unit, &outer).unwrap().into_iter().map(|(at, effect)| (at, effect.stores)).collect();
        return known(&unit, Some(&calls), None, None).get(&value(f, "r")).cloned();
    }
    let known = Analyses::new(outer).get::<ThroughMemory>(&module.context, &layout, f);
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

/// A global's far address stored through a pointer parameter, as a struct
/// returned by value holds it, left `-O2` reading `@g` as it was before
/// the call that wrote it through that struct: Nib printed 0, not 5.
#[test]
fn a_global_whose_far_address_is_stored_through_a_parameter_is_forgotten() {
    let globals = format!(
        "{PRIVATE}
define internal void @mk(ptr addrspace(1) %out) addrspace(1) {{
b0:
  %far = addrspacecast ptr @g to ptr addrspace(1)
  store ptr addrspace(1) %far, ptr addrspace(1) %out
  ret void
}}

define internal void @set(ptr addrspace(1) nocapture %s) addrspace(1) {{
b0:
  %p = load ptr addrspace(1), ptr addrspace(1) %s
  store i16 5, ptr addrspace(1) %p
  ret void
}}
"
    );
    let call = "%a = alloca [4 x i8]\n  %s = addrspacecast ptr %a to ptr addrspace(1)\n  call addrspace(1) void @mk(ptr addrspace(1) %s)\n  call addrspace(1) void @set(ptr addrspace(1) %s)";
    assert_eq!(kept(&globals, call), None);
}

/// QB's `REDIM` descriptor held every array's address in a private constant
/// nothing read: the array was untracked, and a runtime call that writes
/// through a pointer it loaded then reloaded it (deedlines, +128 memory operands).
#[test]
fn a_private_initializer_nothing_names_does_not_leak_the_address_it_holds() {
    let dead = format!("{PRIVATE}@held = internal constant ptr @g\n");
    assert_eq!(kept(&dead, "call void @outside(ptr null)"), seven());
    let chained = format!("{PRIVATE}@held = internal constant ptr @g\n@outer = internal constant ptr @held\n");
    assert_eq!(kept(&chained, "call void @outside(ptr null)"), seven());
    let named = format!("{PRIVATE}@held = internal constant ptr @g\n");
    assert_eq!(kept(&named, "%h = load ptr, ptr @held\n  call void @outside(ptr null)"), None);
    let behind = format!("{PRIVATE}@held = internal constant ptr @g\n@outer = internal constant ptr @held\n");
    assert_eq!(kept(&behind, "%h = load ptr, ptr @outer\n  call void @outside(ptr null)"), None);
}

/// A descriptor holding a global's address, handed only to a routine that
/// `noretain`s it, leaves the global tracked; `nocapture` alone does not.
#[test]
fn a_global_held_only_by_a_noretain_argument_stays_tracked() {
    let held = |attrs: &str| {
        let globals = format!("{PRIVATE}@desc = internal constant ptr @g\n\ndeclare void @erase(ptr {attrs}) nocallback\n");
        kept(&globals, "call void @erase(ptr @desc)\n  store i16 7, ptr @g\n  call void @outside(ptr null)")
    };
    assert_eq!(held("nocapture noretain"), seven());
    assert_eq!(held("nocapture"), None);
}

/// `noretain` says what a call keeps, not what it writes: `ERASE` of a `$STATIC`
/// array zeroes it through the descriptor, so a 9 stored before the call was read
/// back after it (Q45S34).
#[test]
fn a_noretain_call_clobbers_a_global_its_argument_holds() {
    let globals = format!("{PRIVATE}@desc = internal constant ptr @g\n\ndeclare void @erase(ptr nocapture noretain) nocallback\n");
    assert_eq!(kept(&globals, "call void @erase(ptr @desc)"), None);
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

/// A runtime cell outside code names but never hands out, as QB's b$seg,
/// and the routines the program calls.
const USED: &str = "@g = external global i16

declare void @inkey()
declare void @defseg()
declare void @unlisted()
";

/// What the runtime module promises of them.
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
    let kept = |call| kept_under(USED, NAMED, call, true);
    assert_eq!(kept("call void @inkey()"), seven());
    assert_eq!(kept("call void @defseg()"), None);
    assert_eq!(kept("call void @unlisted()"), None);
}

/// A routine that may run the program's code writes what it does.
#[test]
fn a_named_global_is_forgotten_across_a_routine_that_may_call_back() {
    let calling_back = NAMED.replace("declare void @inkey() nocallback", "declare void @inkey()");
    let program = format!("{USED}\ndefine void @set() {{\nb0:\n  call void @defseg()\n  ret void\n}}\n");
    assert_eq!(kept_under(&program, &calling_back, "call void @inkey()", true), None);
}

/// What `@f` of a program's second module returns, `@g` stored 7 before
/// `call`, the first module holding `first`.
fn kept_in(first: &str, globals: &str, call: &str, exports: Exports) -> Option<Known> {
    let second = format!("{DOS}{globals}\ndefine i16 @f() {{\nb0:\n  store i16 7, ptr @g\n  {call}\n  %r = load i16, ptr @g\n  ret i16 %r\n}}\n");
    let program = Program::new(vec![parsed(&format!("{DOS}{first}")), parsed(&second)], Rc::new(Neutral)).unwrap().exporting(exports);
    let mut analyses = ProgramAnalyses::default();
    analyses.get::<ProgramSummaries>(&program);
    let module = &program.modules[1];
    let mut modules = ModuleAnalyses::new(analyses.proxy(&program, 1));
    modules.require::<GlobalsAA>();
    modules.require::<Summaries>();
    let f = function(module, "f");
    let known = Analyses::new(modules.outer(module)).get::<ThroughMemory>(&module.context, &program.layout, f);
    Result::as_ref(&*known).unwrap().get(&value(f, "r")).cloned()
}

/// A call to a body of another module was a call to an unknown routine.
#[test]
fn a_body_another_module_defines_is_summarized() {
    let second = "@g = global i16 0\ndeclare void @quiet()\n";
    let quiet = "define void @quiet() {\nb0:\n  ret void\n}\n";
    assert_eq!(kept_in(quiet, second, "call void @quiet()", Exports::default()), seven());
    assert_eq!(kept_in("", second, "call void @quiet()", Exports::default()), None);
}

/// A global no outside code names is the program's alone, whichever module
/// defines it; one whose address a module lets out is not.
#[test]
fn a_global_the_program_does_not_export_is_kept_across_the_runtime() {
    let second = "@g = external global i16\ndeclare void @outside(ptr) nocallback\n";
    let closed = Exports::closed(BTreeSet::from(["f".to_owned()]));
    let call = "call void @outside(ptr null)";
    assert_eq!(kept_in("@g = global i16 0\n", second, call, closed.clone()), seven());
    assert_eq!(kept_in("@g = global i16 0\n", second, call, Exports::default()), None);
    assert_eq!(kept_in("@g = global i16 0\n@slot = global ptr @g\n", second, call, closed), None);
}

/// A call of a routine a pass declared after the outer facts were taken:
/// it may call back. Indexing the outer globals by its id panicked.
#[test]
fn a_callee_declared_after_the_outer_facts_may_call_back() {
    let taken = parsed("@g = internal global i16 0\n\ndefine void @f() {\nb0:\n  ret void\n}\n");
    let now = parsed("@g = internal global i16 0\n\ndefine void @f() {\nb0:\n  call void @late()\n  ret void\n}\n\ndeclare void @late() nocallback\n");
    let mut outer = Outer::of(&taken, None);
    outer.require::<GlobalsAA>(&taken);
    let layout = layout(&now);
    let f = function(&now, "f");
    let unit = Unit::within(&now.context, &layout, f, &outer);
    let (_, call) = f.walk().next().expect("the call");
    assert!(crate::globalsaa::calls_back(&unit, call));
}
