//! What a call leaves in a global, as the manager's constants see it; and
//! the BC raise's `spared` tests in llrm-qb, on MIR.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::GlobalId;
use llrm_mir::module::Module;
use llrm_mir::passes::{Analyses, ModuleAnalyses, Outer};
use llrm_mir::program::{Exports, Program, ProgramAnalyses};
use llrm_mir::target::Neutral;

use crate::consts::{Calls, Known, known_walked};
use crate::manager::{GlobalsAA, ProgramSummaries, Summaries, ThroughMemory};
use crate::memory::Unit;
use crate::testing::{DOS, function, layout, parsed, value};

/// What `@f` returns as the manager's `ThroughMemory` knows it, `@g`
/// stored 7 before `call`.
fn kept(
    globals: &str,
    call: &str,
) -> Option<Known> {
    kept_by(globals, call, true)
}

/// `kept`, or without `Summaries` what `calls_by_attributes` leaves.
fn kept_by(
    globals: &str,
    call: &str,
    summarized: bool,
) -> Option<Known> {
    kept_under(globals, "", call, summarized)
}

/// `kept_by`, the program linked against the declarations `runtime`.
fn kept_under(
    globals: &str,
    runtime: &str,
    call: &str,
    summarized: bool,
) -> Option<Known> {
    let mut module = parsed(&format!(
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
    if summarized {
        attributed(&mut module);
    }
    let program =
        Program::new(vec![module], Rc::new(Neutral)).unwrap().with_runtime(parsed(&format!("{DOS}{runtime}"))).unwrap();
    let module = &program.modules[0];
    let mut modules = ModuleAnalyses::new(ProgramAnalyses::default().proxy(&program, 0));
    modules.require::<GlobalsAA>();
    modules.require::<crate::manager::GlobalsModRefAnalysis>();
    if summarized {
        modules.require::<Summaries>();
    }
    let outer = modules.outer(module);
    let f = function(module, "f");
    if !summarized {
        let unit = crate::testing::with_registers(Unit::within(&module.context, &layout, f, &outer));
        let calls: Calls = crate::alias::calls_by_attributes(&crate::testing::with_points(unit))
            .iter()
            .map(|(at, effect)| (at, effect.stores.clone()))
            .collect();
        return known_walked(&unit, &calls).get(&value(f, "r")).cloned();
    }
    let known = Analyses::new(outer).get::<ThroughMemory>(&module.context, &layout, f);
    Result::as_ref(&*known).unwrap().get(&value(f, "r")).cloned()
}

/// What the pipeline's `FunctionAttrs` has stated of `module`.
fn attributed(module: &mut Module) {
    let layout = layout(module);
    crate::functionattrs::infer_module(module, &layout, llrm_mir::target::Machine::spaces(&Neutral));
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
        "store ptr addrspace(1) addrspacecast (ptr @g to ptr addrspace(1)), ptr @slot",
        "store ptr getelementptr (i8, ptr @g, i16 1), ptr @slot",
        "call void @outside(ptr getelementptr (i8, ptr @g, i16 1))",
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
    // farstruct.nib as #3251 left it, 0 for 5: the address a constant cast,
    // which the use scan did not see.
    let constant = globals.replace(
        "%far = addrspacecast ptr @g to ptr addrspace(1)\n  store ptr addrspace(1) %far,",
        "store ptr addrspace(1) addrspacecast (ptr @g to ptr addrspace(1)),",
    );
    assert_ne!(constant, globals, "premise: the cast is a constant");
    assert_eq!(kept(&constant, call), None);
}

/// A private constant nothing names holds a global's address: GlobalsModRef,
/// as the pin's, takes an address stored in any initializer as escaped (the
/// pin's pipeline has globaldce remove a dead one first, so QB's `REDIM`
/// descriptors never reach this in a compile). `CallEffects`, deleted with
/// #3436, kept the global tracked (deedlines +128 memory operands then); row
/// AM-CA-5.
#[test]
fn a_private_initializer_nothing_names_leaks_the_address_it_holds_as_the_pin_has_it() {
    let dead = format!("{PRIVATE}@held = internal constant ptr @g\n");
    assert_eq!(kept(&dead, "call void @outside(ptr null)"), None);
    let chained = format!("{PRIVATE}@held = internal constant ptr @g\n@outer = internal constant ptr @held\n");
    assert_eq!(kept(&chained, "call void @outside(ptr null)"), None);
    let named = format!("{PRIVATE}@held = internal constant ptr @g\n");
    assert_eq!(kept(&named, "%h = load ptr, ptr @held\n  call void @outside(ptr null)"), None);
    let behind = format!("{PRIVATE}@held = internal constant ptr @g\n@outer = internal constant ptr @held\n");
    assert_eq!(kept(&behind, "%h = load ptr, ptr @outer\n  call void @outside(ptr null)"), None);
}

/// A descriptor holding a global's address, handed to a routine that
/// `noretain`s it: GlobalsModRef does not read `noretain` (ours, not the
/// pin's), so the global is untracked either way; `CallEffects` kept it. Row
/// AM-CA-5.
#[test]
fn a_global_held_by_a_noretain_argument_is_untracked_until_globalsmodref_reads_noretain() {
    let held = |attrs: &str| {
        let globals =
            format!("{PRIVATE}@desc = internal constant ptr @g\n\ndeclare void @erase(ptr {attrs}) nocallback\n");
        kept(&globals, "call void @erase(ptr @desc)\n  store i16 7, ptr @g\n  call void @outside(ptr null)")
    };
    assert_eq!(held("nocapture noretain"), None);
    assert_eq!(held("nocapture"), None);
}

/// `noretain` says what a call keeps, not what it writes: `ERASE` of a
/// `$STATIC` array zeroes it through the descriptor, so a 9 stored before the
/// call was read back after it (Q45S34).
#[test]
fn a_noretain_call_clobbers_a_global_its_argument_holds() {
    let globals = format!(
        "{PRIVATE}@desc = internal constant ptr @g\n\ndeclare void @erase(ptr nocapture noretain) nocallback\n"
    );
    assert_eq!(kept(&globals, "call void @erase(ptr @desc)"), None);
}

#[test]
fn an_external_global_is_forgotten() {
    assert_eq!(
        kept("@g = global i16 0\n\ndeclare void @outside(ptr) nocallback\n", "call void @outside(ptr null)"),
        None
    );
}

/// A callee that may call back runs `@f`, which writes `@g`.
#[test]
fn a_callee_that_may_call_back_forgets_what_the_module_writes() {
    assert_eq!(
        kept("@g = internal global i16 0\n\ndeclare void @outside(ptr)\n", "call void @outside(ptr null)"),
        None
    );
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
fn kept_in(
    first: &str,
    globals: &str,
    call: &str,
    exports: Exports,
) -> Option<Known> {
    let second = format!(
        "{DOS}{globals}\ndefine i16 @f() {{\nb0:\n  store i16 7, ptr @g\n  {call}\n  %r = load i16, ptr @g\n  ret i16 %r\n}}\n"
    );
    let (mut first, mut second) = (parsed(&format!("{DOS}{first}")), parsed(&second));
    attributed(&mut first);
    attributed(&mut second);
    let mut program = Program::new(vec![first, second], Rc::new(Neutral)).unwrap().exporting(exports);
    // Each body's attributes on the other module's declaration of it, as the
    // pipeline's `published` does.
    for at in 0..2 {
        let defined: Vec<_> = program.modules[at].functions().map(|(id, _, _)| id).collect();
        for id in defined {
            let other = 1 - at;
            for one in (0..program.modules[other].globals.len() as u32).map(GlobalId) {
                if program.definition(other, one) == Some((at, id)) {
                    program.restate(at, id, other, one);
                }
            }
        }
    }
    let mut analyses = ProgramAnalyses::default();
    analyses.get::<ProgramSummaries>(&program);
    let module = &program.modules[1];
    let mut modules = ModuleAnalyses::new(analyses.proxy(&program, 1));
    modules.require::<GlobalsAA>();
    modules.require::<crate::manager::GlobalsModRefAnalysis>();
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
    let now = parsed(
        "@g = internal global i16 0\n\ndefine void @f() {\nb0:\n  call void @late()\n  ret void\n}\n\ndeclare void @late() nocallback\n",
    );
    let mut outer = Outer::of(&taken, None);
    outer.require::<GlobalsAA>(&taken);
    let layout = layout(&now);
    let f = function(&now, "f");
    let unit = crate::testing::with_registers(Unit::within(&now.context, &layout, f, &outer));
    let (_, call) = f.walk().next().expect("the call");
    assert!(crate::globalsaa::calls_back(&unit, call));
}

/// Every edit to a body worked out what every body contributes to `GlobalsAA`
/// again: an escape analysis of each, whole (`chain-32`: 135 runs, 15% of the
/// compile). A body not edited since keeps its contribution.
#[test]
fn an_edit_to_one_body_works_out_that_body_alone() {
    let mut module = parsed(&format!(
        "{DOS}@g = internal global i16 0
define i16 @f(i16 %a) {{
b0:
  %x = add i16 %a, 1
  %y = mul i16 %x, 2
  ret i16 %y
}}
define i16 @h(i16 %a) {{
b0:
  %x = add i16 %a, 3
  %y = mul i16 %x, 4
  store i16 %y, ptr @g
  ret i16 %y
}}
"
    ));
    let program = Program::new(vec![module.clone()], Rc::new(Neutral)).unwrap();
    let mut analyses = ModuleAnalyses::new(ProgramAnalyses::default().proxy(&program, 0));
    let first = analyses.get::<GlobalsAA>(&module);
    let (f, h) = (module.named("f").unwrap(), module.named("h").unwrap());
    let before = {
        let memo = analyses.memo::<super::Bodies>();
        (Rc::clone(&memo.per[&f].1), Rc::clone(&memo.per[&h].1))
    };
    // `@f` edited: an operand now the parameter.
    let (_, function) = module.function_mut("f").unwrap();
    let (x, a) = (value(function, "x"), function.parameters()[0]);
    function.replace_all_uses_with(x, llrm_mir::module::Operand::Value(a));
    analyses.invalidate(&llrm_mir::passes::PreservedAnalyses::none());
    let second = analyses.get::<GlobalsAA>(&module);
    assert_eq!(first, second);
    let memo = analyses.memo::<super::Bodies>();
    assert!(Rc::ptr_eq(&before.1, &memo.per[&h].1), "an unedited body's contribution was worked out again");
    assert!(!Rc::ptr_eq(&before.0, &memo.per[&f].1), "an edited body's contribution was kept");
}

/// A private global's address given to `llrm.ia16.window` came back as a
/// pointer no fact follows, and the call counted as keeping the address: the
/// global was forgotten at a fresh run but held by the analysis frozen before
/// `window` ran (`LLRM_CHECK_STALE` stopped `window` on far_near.nib and
/// huge.nib). The window is the same bytes, and keeps none: @g stays private,
/// and a store through the window is still a store to @g.
#[test]
fn the_window_of_a_private_global_keeps_it_private_and_is_it() {
    let globals = format!("{PRIVATE}declare ptr @llrm.ia16.window.p0.p0(ptr)\n");
    let window = "%w = call ptr @llrm.ia16.window.p0.p0(ptr @g)";
    assert_eq!(kept(&globals, &format!("{window}\n  call void @outside(ptr null)")), seven());
    assert_ne!(kept(&globals, &format!("{window}\n  store i16 9, ptr %w\n  call void @outside(ptr null)")), seven());
}

/// Where `globalsaa`'s tracked set (each body's pointer solve) and
/// `GlobalsModRef`'s `AnalyzeGlobals` (a use scan per global) disagree, over
/// every corpus module as a one-module program, asked of the internal global
/// variables. The scan never tracks what GlobalsAA does not. GlobalsAA tracks
/// more, and each extra global is one the pin's rule calls escaped for a
/// reason this test names: its address is in the initializer of another
/// internal global (GlobalsAA reads an initializer only through its holder),
/// stored as a value, or a `phi` input. A global escaping for any other reason
/// is a difference nobody has explained.
#[test]
fn globals_aa_tracks_what_the_globals_mod_ref_scan_tracks_and_the_extras_are_explained() {
    use llrm_mir::context::{ConstantExpr, ConstantId, ConstantKind, GlobalId};
    use llrm_mir::module::{Function, GlobalKind, InstId, Linkage, Operand, ValueId};
    use llrm_mir::opcode::{CastOp, Opcode};
    let (mut compared, mut extras, mut explained) = (0usize, 0usize, 0usize);
    let mut unexplained = Vec::new();
    for (name, module) in crate::testing::corpus() {
        let Ok(program) = Program::new(vec![module], Rc::new(Neutral)) else { continue };
        let module = &program.modules[0];
        let mut modules = ModuleAnalyses::new(ProgramAnalyses::default().proxy(&program, 0));
        // `require` only keeps an analysis; `get` computes it.
        let old = modules.get::<GlobalsAA>(module);
        let old = Result::as_ref(&*old).expect("GlobalsAA of a corpus module");
        let scan = crate::globalsmodref::compute(module);
        let names = |root: ConstantId, id: GlobalId| {
            let mut work = vec![root];
            while let Some(c) = work.pop() {
                match &module.context.get(c).kind {
                    ConstantKind::Global(g) if *g == id => return true,
                    ConstantKind::Expr(ConstantExpr::GetElementPtr { operands, .. }) => {
                        work.extend(operands.iter().copied())
                    }
                    ConstantKind::Expr(ConstantExpr::Cast { value, .. }) => work.push(*value),
                    ConstantKind::Aggregate(members) => work.extend(members.iter().copied()),
                    _ => {}
                }
            }
            false
        };
        for (at, global) in module.globals.iter().enumerate() {
            let id = GlobalId(at as u32);
            if !matches!(global.kind, GlobalKind::Variable(_))
                || !matches!(global.linkage, Linkage::Internal | Linkage::Private)
            {
                continue;
            }
            compared += 1;
            let label = format!("{name}/@{}", global.name.as_deref().unwrap_or(""));
            let (tracked, scanned) = (old.tracked(id), scan.is_non_address_taken(id));
            if scanned && !tracked {
                unexplained.push(format!("{label}: only the scan tracks it"));
            }
            if !tracked || scanned {
                continue;
            }
            extras += 1;
            // The reasons, found by looking at the uses the scan stops at.
            let in_initializer = module
                .globals
                .iter()
                .any(
                    |holder| match &holder.kind {
                        GlobalKind::Variable(v) => v.initializer.is_some_and(|init| names(init, id)),
                        GlobalKind::Function(_) => false,
                    },
                );
            let mut other = false;
            let mut visit = |f: &Function, inst: InstId, index: usize| -> Option<ValueId> {
                let op = f.instruction(inst);
                match &op.opcode {
                    Opcode::Load { .. } => None,
                    Opcode::Store { .. } if index == 1 => None,
                    Opcode::Store { .. } | Opcode::Phi => None,
                    Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => op.result,
                    _ => {
                        other = true;
                        None
                    }
                }
            };
            let mut by_use = false;
            for (_, _, f) in module.functions().filter(|(_, _, f)| !f.is_declaration()) {
                for (_, inst) in f.walk() {
                    for (index, operand) in f.instruction(inst).operands.iter().enumerate() {
                        let Operand::Constant(c) = operand else { continue };
                        if !names(*c, id) {
                            continue;
                        }
                        let mut work = Vec::new();
                        work.extend(visit(f, inst, index));
                        while let Some(v) = work.pop() {
                            for user in f.users(v) {
                                let kind = &f.instruction(user.user).opcode;
                                by_use |= matches!(kind, Opcode::Store { .. } if user.index != 1)
                                    || matches!(kind, Opcode::Phi);
                                work.extend(visit(f, user.user, user.index as usize));
                            }
                        }
                    }
                }
            }
            if (in_initializer || by_use) && !other {
                explained += 1;
            } else {
                unexplained
                    .push(format!("{label}: initializer {in_initializer}, stored or phi {by_use}, other use {other}"));
            }
        }
    }
    // Premise: the corpus has internal globals, and GlobalsAA tracks some the
    // scan does not.
    assert!(compared > 0 && extras > 0, "{compared} compared, {extras} extra");
    assert!(
        unexplained.is_empty(),
        "{} of {extras} unexplained: {:#?}",
        unexplained.len(),
        &unexplained[..unexplained.len().min(8)]
    );
    assert_eq!(explained, extras);
}

/// GlobalsAA of a module asks no pointer solve: its tracked set is one use scan
/// of the module (`AnalyzeGlobals`), where it ran `alias::points_to` on every
/// body, five times over a d_faces compile (48.8 M inclusive, #3190).
#[test]
fn globals_aa_asks_no_pointer_solve() {
    let module = parsed(&format!(
        "{DOS}@g = internal global i16 0\n@h = internal global i16 0\ndefine i16 @f() {{\nb0:\n  store i16 7, ptr @g\n  %r = load i16, ptr @h\n  ret i16 %r\n}}\ndefine i16 @k() {{\nb0:\n  %r = load i16, ptr @g\n  ret i16 %r\n}}\n"
    ));
    let program = Program::new(vec![module], Rc::new(Neutral)).unwrap();
    let module = &program.modules[0];
    let mut modules = ModuleAnalyses::new(ProgramAnalyses::default().proxy(&program, 0));
    let (values, escapes) = (crate::alias::value_solves(), crate::alias::escape_solves());
    let found = modules.get::<GlobalsAA>(module);
    let found = Result::as_ref(&*found).unwrap();
    // Premise: the analysis ran and tracks both globals.
    assert_eq!(found.tracked_globals().len(), 2);
    assert_eq!(crate::alias::value_solves() - values, 0, "a body was solved for what it points to");
    assert_eq!(crate::alias::escape_solves() - escapes, 0, "a body was solved for what escapes");
}

/// `GlobalsAA` of the one-module program `text`.
fn globals_of(text: &str) -> crate::globalsaa::Globals {
    let module = parsed(text);
    let program = Program::new(vec![module], Rc::new(Neutral)).unwrap();
    let module = &program.modules[0];
    let mut modules = ModuleAnalyses::new(ProgramAnalyses::default().proxy(&program, 0));
    let found = modules.get::<GlobalsAA>(module);
    Result::as_ref(&*found).expect("GlobalsAA").clone()
}

/// GlobalsAA is held past the bodies ipa-cp adds after it was made. A copy
/// is no declaration: taken for one, its call wrote no tracked global and a
/// load of @g across it kept a stale value.
#[test]
fn a_function_added_after_globals_aa_was_made_reaches_every_tracked_global() {
    let found = globals_of(
        "@g = internal global i16 0

define void @f() {
b0:
  store i16 1, ptr @g
  ret void
}
",
    );
    let added = llrm_mir::context::GlobalId(2);
    let (read, written) = found.unsummarized(Some(added));
    assert_eq!((read.len(), written.len()), (1, 1), "a copy made since was taken for a declaration");
}

/// A loop summing `@a` that reads it by index, then the same loop after loop
/// strength reduction: through a pointer phi that steps by 2. `after` adds
/// `uses` to the second, in the loop (`ret` is its return value, `decl` what
/// it declares).
fn indexed_and_stepped(
    decl: &str,
    uses: &str,
    ret: (&str, &str),
) -> (crate::globalsaa::Globals, crate::globalsaa::Globals) {
    let head = format!(
        "{DOS}@a = internal global [128 x i8] zeroinitializer
@s = internal global [4 x i8] zeroinitializer
{decl}
"
    );
    let before = format!(
        "{head}define {ty} @f() {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i.next, %b1 ]
  %p = getelementptr inbounds [2 x i8], ptr @a, i16 %i
  %v = load i16, ptr %p
  store i16 %v, ptr @s
  %i.next = add nuw nsw i16 %i, 1
  %c = icmp ne i16 %i.next, 64
  br i1 %c, label %b1, label %b2

b2:
  ret {ty} {zero}
}}
",
        ty = ret.0,
        zero = if ret.0 == "ptr" { "null" } else { "0" },
    );
    let after = format!(
        "{head}define {ty} @f() {{
b0:
  br label %b1

b1:
  %p = phi ptr [ @a, %b0 ], [ %p.next, %b1 ]
  %i = phi i16 [ -256, %b0 ], [ %i.next, %b1 ]
  %v = load i16, ptr %p
  store i16 %v, ptr @s
  {uses}
  %i.next = add nsw i16 %i, 4
  %p.next = getelementptr i8, ptr %p, i16 2
  %c = icmp ne i16 %i.next, 0
  br i1 %c, label %b1, label %b2

b2:
  ret {ty} {value}
}}
",
        ty = ret.0,
        value = ret.1,
    );
    (globals_of(&before), globals_of(&after))
}

/// jpeg-shaped loops after `lsr` read a global through `phi ptr [ @a, ... ]`.
/// The pin's use scan calls that an escape (no `phi` case in
/// `AnalyzeUsesOfPointer`), so a held result that tracks `@a` was refused by
/// `LLRM_CHECK_STALE` as one that "no longer covers the module" with nothing
/// added but a pointer induction variable (lsr-stale-repro.bas, -m16 -O1).
#[test]
fn a_global_read_through_a_stepped_pointer_phi_is_still_covered() {
    let (before, after) = indexed_and_stepped("", "", ("i16", "0"));
    let a = GlobalId(0);
    assert!(before.tracked(a), "reached by name before");
    assert!(!after.tracked(a), "the pin's scan calls the phi an escape");
    assert!(before.covers(&after));
}

#[test]
fn a_stepped_pointer_phi_passed_to_a_call_is_not_covered() {
    let (before, after) = indexed_and_stepped("declare void @ext(ptr)", "call void @ext(ptr %p)", ("i16", "0"));
    assert!(!before.covers(&after));
}

#[test]
fn a_stepped_pointer_phi_stored_as_a_value_is_not_covered() {
    let (before, after) = indexed_and_stepped(
        "@keep = internal global [4 x i8] zeroinitializer",
        "store ptr %p, ptr @keep",
        ("i16", "0"),
    );
    assert!(!before.covers(&after));
}

#[test]
fn a_stepped_pointer_phi_that_is_returned_is_not_covered() {
    let (before, after) = indexed_and_stepped("", "", ("ptr", "%p"));
    assert!(!before.covers(&after));
}

/// Two globals reaching one pointer phi that is passed to a call: both
/// escape. The joins seen were one set for every candidate, so `@b` found the
/// phi already seen from `@a` and was judged not to escape (w-7's review). The
/// held result tracks `@b` only (`@a` is stored in `@hold`), so `@a`'s own
/// refusal cannot hide it.
#[test]
fn a_global_joined_into_a_phi_with_another_that_is_passed_to_a_call_is_not_covered() {
    let head = format!(
        "{DOS}@a = internal global [128 x i8] zeroinitializer
@b = internal global [128 x i8] zeroinitializer
@hold = internal global [2 x i8] zeroinitializer
declare void @ext(ptr)
"
    );
    let before = format!(
        "{head}define i16 @f(i16 %n) {{
b0:
  store ptr @a, ptr @hold
  %w = load i16, ptr @b
  ret i16 %w
}}
"
    );
    let after = format!(
        "{head}define i16 @f(i1 %c) {{
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi ptr [ @a, %b1 ], [ @b, %b2 ]
  call void @ext(ptr %p)
  ret i16 0
}}
"
    );
    let (before, after) = (globals_of(&before), globals_of(&after));
    assert!(!before.tracked(GlobalId(0)) && before.tracked(GlobalId(1)));
    assert!(!before.covers(&after));
}

/// `GlobalsAA` of the one-module program `text` on real mode.
fn globals_on_dos(text: &str) -> crate::globalsaa::Globals {
    let module = parsed(text);
    let program = Program::new(vec![module], Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    let module = &program.modules[0];
    let mut modules = ModuleAnalyses::new(ProgramAnalyses::default().proxy(&program, 0));
    let found = modules.get::<GlobalsAA>(module);
    Result::as_ref(&*found).expect("GlobalsAA").clone()
}

/// A body that makes a far pointer from `selector`, an `i16`, and reads a
/// byte through it, beside `@a`, an internal global nobody takes the address
/// of (`rest` follows).
fn forged_over_a(
    selector: &str,
    rest: &str,
) -> crate::globalsaa::Globals {
    globals_on_dos(&format!(
        "{DOS}@a = internal global [8 x i8] zeroinitializer
@w = internal global [2 x i8] zeroinitializer

define i16 @f(i16 %n) {{
b0:
  %s = inttoptr i16 {selector} to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %p = getelementptr i8, ptr addrspace(1) %far, i16 %n
  %v = load volatile i8, ptr addrspace(1) %p
  %w = load i16, ptr @a
  ret i16 %w
}}
{rest}"
    ))
}

/// far6.bas: z, a DIM SHARED whose address nobody took, was promoted to a
/// register (z's loads 4 -> 1) while `DEF SEG = VARSEG(w)` and a POKE at a
/// scanned offset wrote it: 21257 where BC printed 100. A selector made of an
/// integer that is no plain number may name the data segment, so no global is
/// reached by name only; a plain number names a segment no object occupies.
#[test]
fn a_selector_forged_from_a_computed_integer_leaves_no_global_tracked() {
    let a = GlobalId(0);
    assert!(forged_over_a("-18432", "").tracked(a), "premise: a numeric segment (&HB800) forges nothing");
    assert!(!forged_over_a("%n", "").tracked(a), "a selector computed from a parameter");
    let varseg = "ptrtoint (ptr addrspace(2) addrspacecast (ptr @w to ptr addrspace(2)) to i16)";
    assert!(!forged_over_a(varseg, "").tracked(a), "VARSEG(w): the data segment's own value");
}

/// The runtime's bodies come `available_externally` and are no forger of the
/// program's globals (`!llrm.named` is the same trust boundary).
#[test]
fn a_runtime_body_that_forges_a_selector_does_not_untrack_the_programs_globals() {
    let runtime = "
define available_externally i16 @rt(i16 %n) {
b0:
  %s = inttoptr i16 %n to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %s to ptr addrspace(1)
  %v = load volatile i8, ptr addrspace(1) %far
  %z = zext i8 %v to i16
  ret i16 %z
}
";
    assert!(forged_over_a("-18432", runtime).tracked(GlobalId(0)));
}
