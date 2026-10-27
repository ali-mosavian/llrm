//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what `opt -passes=sccp` makes of the same body (checked
//! with `tools/difftest`).

use llrm_mir::GlobalId;
use llrm_mir::module::{GlobalKind, Module};
use llrm_mir::passes::Outer;
use llrm_mir::valuelattice::ValueLatticeElement;
use llrm_mir::valuetracking::AssumptionCache;

use super::run_sccp;
use crate::predicateinfo::PredicateInfo;
use crate::sccpsolver::{ModuleEnv, ModuleResults, Results, Solver, is_constant};
use crate::testing::{parsed, printed, results};

/// @f after SCCP; whether anything changed.
fn sccped(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let layout = llrm_analysis::testing::layout(module);
    let id = module
        .globals
        .iter()
        .position(|one| one.name.as_deref() == Some("f"))
        .map(|at| llrm_mir::context::GlobalId(at as u32));
    let metadata = outer.metadata.clone();
    let (context, function) = module.function_mut("f").expect("@f");
    let dt = llrm_mir::domtree::DomTree::of(function);
    run_sccp(context, &layout, &outer, &metadata, id, &dt, function).anything
}

/// `text` after SCCP is `expected` and computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(sccped(&mut module), text != expected);
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// A branch on a compare of constants goes one way: the other block is gone,
/// and the compare with it.
#[test]
fn test_a_branch_on_a_constant_compare_goes_one_way() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp eq i32 1, 1
  br i1 %c, label %b1, label %b2

b1:
  ret i32 7

b2:
  ret i32 8
}
",
        "define i32 @f(i32 %x) {
b0:
  br label %b1

b1:
  ret i32 7
}
",
        &[&[0], &[5]],
    );
}

/// A phi whose feasible inputs agree is that constant, and so is what reads it.
#[test]
fn test_a_phi_of_equal_constants_and_its_user_fold() {
    check(
        "define i32 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i32 [ 5, %b1 ], [ 5, %b2 ]
  %r = add i32 %p, 1
  ret i32 %r
}
",
        "define i32 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  ret i32 6
}
",
        &[&[0], &[1]],
    );
}

/// A phi input from an edge that is never taken does not count: the loop
/// counter's `%i` cannot reach `%b3` with a value other than the constant.
#[test]
fn test_a_phi_input_from_an_infeasible_edge_is_ignored() {
    check(
        "define i32 @f(i32 %x) {
b0:
  br i1 false, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i32 [ %x, %b1 ], [ 9, %b2 ]
  ret i32 %p
}
",
        "define i32 @f(i32 %x) {
b0:
  br label %b2

b2:
  br label %b3

b3:
  ret i32 9
}
",
        &[&[0], &[3]],
    );
}

/// A sign extension of a value the range says is not negative is a zero
/// extension marked `nneg`, and keeps the name.
#[test]
fn test_a_sign_extension_of_a_non_negative_range_is_a_nneg_zero_extension() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %a = and i8 %x, 127
  %s = sext i8 %a to i32
  ret i32 %s
}
",
        "define range(i32 0, 128) i32 @f(i8 %x) {
b0:
  %a = and i8 %x, 127
  %s = zext nneg i8 %a to i32
  ret i32 %s
}
",
        &[&[0], &[200], &[127]],
    );
}

/// An add whose operand ranges cannot wrap gets `nuw nsw`.
#[test]
fn test_an_add_that_cannot_wrap_gets_the_flags() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  %a = add i32 %z, 1
  ret i32 %a
}
",
        "define range(i32 1, 257) i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  %a = add nuw nsw i32 %z, 1
  ret i32 %a
}
",
        &[&[0], &[255]],
    );
}

/// A switch on a value whose range holds two of its cases drops the third and
/// sends the default to a block of `unreachable`.
#[test]
fn test_a_switch_loses_the_cases_the_range_excludes() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %m = and i32 %x, 1
  switch i32 %m, label %b4 [
    i32 0, label %b1
    i32 1, label %b2
    i32 2, label %b3
  ]

b1:
  ret i32 10

b2:
  ret i32 20

b3:
  ret i32 30

b4:
  ret i32 40
}
",
        "define range(i32 10, 21) i32 @f(i32 %x) {
b0:
  %m = and i32 %x, 1
  switch i32 %m, label %default.unreachable [
    i32 0, label %b1
    i32 1, label %b2
  ]

b1:
  ret i32 10

b2:
  ret i32 20

default.unreachable:
  unreachable
}
",
        &[&[0], &[3]],
    );
}

/// The function returns 3 or 5, so its return value is in `[3, 6)`.
#[test]
fn test_the_return_range_becomes_an_attribute() {
    check(
        "define i32 @f(i1 %c) {
b0:
  %s = select i1 %c, i32 3, i32 5
  ret i32 %s
}
",
        "define range(i32 3, 6) i32 @f(i1 %c) {
b0:
  %s = select i1 %c, i32 3, i32 5
  ret i32 %s
}
",
        &[&[0], &[1]],
    );
}

/// A mask that clears no bit the value can have is the value.
#[test]
fn test_a_mask_the_range_already_fits_is_removed() {
    check(
        "define i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  %m = and i32 %z, 255
  ret i32 %m
}
",
        "define range(i32 0, 256) i32 @f(i8 %x) {
b0:
  %z = zext i8 %x to i32
  ret i32 %z
}
",
        &[&[0], &[255]],
    );
}

/// `x u< 10` where `x` is known to be in `[0, 4)` is `true`: a branch on it
/// goes one way, through a block whose argument range comes from a `range`
/// attribute.
#[test]
fn test_an_argument_range_decides_a_compare() {
    check(
        "define i32 @f(i32 range(i32 0, 4) %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2

b1:
  ret i32 1

b2:
  ret i32 2
}
",
        "define i32 @f(i32 range(i32 0, 4) %x) {
b0:
  br label %b1

b1:
  ret i32 1
}
",
        &[&[0], &[3]],
    );
}

/// A compare a branch has already decided is a constant where the branch holds
/// (`PredicateInfo`'s copy of `%x` carries `[0, 10)` into `%b1`).
#[test]
fn test_a_branch_condition_decides_a_later_compare() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2

b1:
  %d = icmp ult i32 %x, 20
  br i1 %d, label %b3, label %b4

b3:
  ret i32 1

b4:
  ret i32 2

b2:
  ret i32 3
}
",
        "define range(i32 1, 4) i32 @f(i32 %x) {
b0:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b3:
  ret i32 1

b2:
  ret i32 3
}
",
        &[&[0], &[9], &[10], &[50]],
    );
}

/// Where `%x == 5` holds, `%x` is 5: what reads it there folds.
#[test]
fn test_an_equality_branch_makes_the_value_a_constant_where_it_holds() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  %r = add i32 %x, 1
  ret i32 %r

b2:
  ret i32 0
}
",
        "define range(i32 0, 7) i32 @f(i32 %x) {
b0:
  %c = icmp eq i32 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  ret i32 6

b2:
  ret i32 0
}
",
        &[&[5], &[6]],
    );
}

/// The rewrites read an instruction by `detached`: all of it but the metadata
/// strings, which a whole clone copied for every instruction they visited
/// (d_faces -O2: the clones were 5.5 of sccp's 16.6 Minstr).
#[test]
fn a_rewrite_reads_an_instruction_without_copying_its_metadata() {
    let mut module = parsed(
        "define i16 @f(ptr %p) {
b0:
  %v = load i16, ptr %p, !tbaa !1
  ret i16 %v
}

!0 = !{!\"root\"}
!1 = !{!0, !0, i64 0}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    let load = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| matches!(
            function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { .. }
        ))
        .expect("the load");
    assert!(!function.instruction(load).metadata.is_empty(), "the load carries !tbaa");
    let seen = super::detached(function, load);
    assert!(seen.metadata.is_empty());
    assert_eq!((seen.opcode, seen.ty, seen.operands, seen.flags, seen.result), {
        let one = function.instruction(load);
        (one.opcode.clone(), one.ty, one.operands.clone(), one.flags, one.result)
    });
}

/// Solves `text` across its functions as `runIPSCCP` sets the solver up: the
/// return of each in `returns` is tracked, the arguments of each in
/// `arguments`; any other function is called from outside. `drive` solves.
fn module_solved(
    text: &str,
    returns: &[&str],
    arguments: &[&str],
    drive: impl FnOnce(&mut Solver),
) -> (Module, ModuleResults) {
    let mut module = parsed(text);
    let layout = llrm_analysis::testing::layout(&module);
    let declared = module.declarations();
    let mut predicates = Vec::new();
    for (at, global) in module.globals.iter_mut().enumerate() {
        let GlobalKind::Function(function) = &mut global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let dt = llrm_mir::domtree::DomTree::of(function);
        let ac = AssumptionCache::of(&module.context, &declared, &module.metadata, function);
        let info = PredicateInfo::new(&module.context, &declared, &module.metadata, function, &dt, &ac);
        predicates.push((GlobalId(at as u32), info));
    }
    let results = {
        let functions = predicates
            .iter()
            .map(|(id, info)| (*id, module.globals[id.0 as usize].function().expect("a function"), info))
            .collect();
        let env = ModuleEnv {
            context: &mut module.context,
            layout: &layout,
            globals: &module.globals,
            metadata: &module.metadata,
        };
        let mut solver = Solver::module(env, functions).expect("a defined function");
        for (id, _) in &predicates {
            let global = &module.globals[id.0 as usize];
            let name = global.name.as_deref().expect("a named function");
            if returns.contains(&name) {
                solver.add_tracked_function(*id);
            }
            if arguments.contains(&name) {
                solver.add_argument_tracked_function(*id);
                continue;
            }
            let function = global.function().expect("a function");
            solver.mark_block_executable_in(*id, function.entry().expect("a body"));
            for index in 0..function.parameters().len() {
                solver.track_value_of_argument_in(*id, index as u32);
            }
        }
        drive(&mut solver);
        solver.into_module_results()
    };
    (module, results)
}

/// What the solve found of `@function`.
fn solved_of<'r>(
    module: &Module,
    results: &'r ModuleResults,
    function: &str,
) -> &'r Results {
    let id = module.named(function).expect("the function");
    &results.functions.iter().find(|(one, _)| *one == Some(id)).expect("solved").1
}

/// The solved state of `%value` in `@function`.
fn lattice_of(
    module: &Module,
    results: &ModuleResults,
    function: &str,
    value: &str,
) -> ValueLatticeElement {
    let body = module.global(module.named(function).expect("the function")).function().expect("a function");
    solved_of(module, results, function).lattice_value(llrm_analysis::testing::value(body, value))
}

fn solve_all(solver: &mut Solver) {
    solver.solve_while_resolved_undefs_in();
}

/// A tracked callee's argument is the join of what its callers pass: 1 and 3
/// make [1, 4), and so does the return each call reads.
#[test]
fn a_tracked_argument_is_the_join_of_the_actuals() {
    let (module, results) = module_solved(
        "define internal i32 @g(i32 %a) {
b0:
  ret i32 %a
}

define i32 @f() {
b0:
  %x = call i32 @g(i32 1)
  %y = call i32 @g(i32 3)
  %s = add i32 %x, %y
  ret i32 %s
}
",
        &["g"],
        &["g"],
        solve_all,
    );
    for (function, value) in [("g", "a"), ("f", "x"), ("f", "y")] {
        let lv = lattice_of(&module, &results, function, value);
        assert!(lv.is_constant_range(false), "%{value}: {lv:?}");
        let range = lv.get_constant_range(false);
        assert_eq!((range.lower, range.upper), (1, 4), "%{value}: {lv:?}");
    }
}

/// A callee nothing calls keeps its argument unknown and runs no block, while
/// the one that is called gets the actual.
#[test]
fn a_callee_never_called_stays_unknown() {
    let (module, results) = module_solved(
        "define internal i32 @h(i32 %a) {
b0:
  ret i32 %a
}

define internal i32 @g(i32 %a) {
b0:
  ret i32 %a
}

define i32 @f() {
b0:
  %x = call i32 @g(i32 2)
  ret i32 %x
}
",
        &["g", "h"],
        &["g", "h"],
        solve_all,
    );
    let h = solved_of(&module, &results, "h");
    let entry = module.global(module.named("h").expect("@h")).function().and_then(|one| one.entry()).expect("a body");
    assert!(lattice_of(&module, &results, "h", "a").is_unknown());
    assert!(!h.is_block_executable(entry));
    assert!(h.tracked_return().is_some_and(ValueLatticeElement::is_unknown));
    assert!(is_constant(&lattice_of(&module, &results, "g", "a")));
}

/// A constant a tracked callee returns is the call's result in the caller;
/// an untracked callee's result is overdefined.
#[test]
fn a_tracked_return_reaches_the_call_and_an_untracked_one_does_not() {
    let (module, results) = module_solved(
        "define internal i32 @g() {
b0:
  ret i32 7
}

define internal i32 @u() {
b0:
  ret i32 7
}

define i32 @f() {
b0:
  %x = call i32 @g()
  %y = call i32 @u()
  %s = add i32 %x, %y
  ret i32 %s
}
",
        &["g"],
        &["g", "u"],
        solve_all,
    );
    let x = lattice_of(&module, &results, "f", "x");
    assert!(is_constant(&x), "{x:?}");
    assert_eq!(x.get_constant_range(true).get_single_element(), Some(7), "{x:?}");
    assert!(lattice_of(&module, &results, "f", "y").is_overdefined());
}

/// A callee that calls itself reaches a fixed point: the base case's 5 is
/// what every call returns, and the recursion's argument widens and stops.
#[test]
fn a_recursive_callee_reaches_a_fixed_point() {
    let (module, results) = module_solved(
        "define internal i32 @r(i32 %n) {
b0:
  %c = icmp eq i32 %n, 0
  br i1 %c, label %b1, label %b2

b1:
  ret i32 5

b2:
  %m = sub i32 %n, 1
  %v = call i32 @r(i32 %m)
  ret i32 %v
}

define i32 @f() {
b0:
  %x = call i32 @r(i32 10)
  ret i32 %x
}
",
        &["r"],
        &["r"],
        solve_all,
    );
    let x = lattice_of(&module, &results, "f", "x");
    assert!(is_constant(&x), "{x:?}");
    assert_eq!(x.get_constant_range(true).get_single_element(), Some(5), "{x:?}");
    assert!(!lattice_of(&module, &results, "r", "n").is_unknown());
}

/// `resolvedUndefsIn` over the module: an add of undef in a callee, unknown
/// after the solve, becomes overdefined; the call to a tracked function that
/// never returns stays unknown.
#[test]
fn resolved_undefs_in_reaches_every_function() {
    let text = "define internal i32 @g(i32 %a) {
b0:
  %u = add i32 %a, undef
  ret i32 %u
}

define internal i32 @k() {
b0:
  unreachable
}

define i32 @f() {
b0:
  %x = call i32 @g(i32 1)
  %y = call i32 @k()
  ret i32 %x
}
";
    let probe = parsed(text);
    let g = probe.named("g").expect("@g");
    let u = llrm_analysis::testing::value(probe.global(g).function().expect("@g"), "u");
    let mut steps = Vec::new();
    let (module, results) = module_solved(text, &["g", "k"], &["g", "k"], |solver| {
        solver.solve();
        steps.push(solver.lattice_value_in(g, u).is_unknown());
        steps.push(solver.resolved_undefs_in());
        solver.solve_while_resolved_undefs_in();
    });
    assert_eq!(steps, [true, true]);
    assert!(lattice_of(&module, &results, "g", "u").is_overdefined());
    assert!(lattice_of(&module, &results, "f", "y").is_unknown());
}
