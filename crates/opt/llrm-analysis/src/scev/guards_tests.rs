//! What the conditions that dominate a loop say, against values worked out by
//! hand: `applyLoopGuards` of a function argument (a `SCEVUnknown`, so
//! `createSCEV` is not read) and the entry-guard checks.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, Operand};
use llrm_mir::opcode::IntPredicate;
use llrm_mir::valuetracking::AssumptionCache;

use super::ap::Ap;
use super::expr::{Scev, Use};
use super::{Cx, ScalarEvolution};

/// A loop at `%h` entered under the condition `guard` (an `icmp` or a boolean
/// expression over `%n` and `%m`, i32 arguments).
fn loop_under(guard: &str) -> String {
    format!(
        "define void @f(i32 %n, i32 %m) {{
entry:
{guard}
  br i1 %g, label %h, label %exit

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %i2 = add i32 %i, 1
  %c = icmp ult i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret void
}}
"
    )
}

/// The test with a `ScalarEvolution`, the context, the loop's header and the
/// SCEVs of `%n` and `%m`.
fn with_guard(
    guard: &str,
    test: impl FnOnce(&mut ScalarEvolution, &Cx, BlockId, Scev, Scev),
) {
    let module = llrm_mir::parse::module(&loop_under(guard)).expect("a module");
    let (_, _, function) = module.functions().next().expect("a function");
    let layout = DataLayout::default();
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let header = *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let mut se = ScalarEvolution::new();
    let [n, m] = function.parameters()[..] else { panic!("two arguments") };
    let (n, m) = (se.get_scev(&cx, Operand::Value(n)), se.get_scev(&cx, Operand::Value(m)));
    test(&mut se, &cx, header, n, m);
}

fn constant(
    se: &mut ScalarEvolution,
    value: u128,
) -> Scev {
    se.get_constant(Ap::new(32, value))
}

/// `%n u< 10` rewrites `%n` to `umin(%n, 9)`.
#[test]
fn test_an_unsigned_less_than_guard_clamps_the_argument() {
    with_guard("  %g = icmp ult i32 %n, 10", |se, cx, header, n, _| {
        let want = {
            let nine = constant(se, 9);
            se.get_umin_expr(cx, vec![Use::of(n), Use::of(nine)], false)
        };
        assert_eq!(se.apply_loop_guards(cx, n, header), want);
        let range = se.get_unsigned_range(cx, want);
        assert_eq!(range.get_unsigned_max(), 9);
    });
}

/// `%n != 0` rewrites `%n` to `umax(%n, 1)`, and `%n s> 4` to `smax(%n, 5)`.
#[test]
fn test_a_nonzero_and_a_signed_greater_than_guard() {
    with_guard("  %g = icmp ne i32 %n, 0", |se, cx, header, n, _| {
        let one = constant(se, 1);
        let want = se.get_umax_expr(cx, vec![Use::of(n), Use::of(one)]);
        assert_eq!(se.apply_loop_guards(cx, n, header), want);
    });
    with_guard("  %g = icmp sgt i32 %n, 4", |se, cx, header, n, _| {
        let five = constant(se, 5);
        let want = se.get_smax_expr(cx, vec![Use::of(n), Use::of(five)]);
        assert_eq!(se.apply_loop_guards(cx, n, header), want);
    });
}

/// A guard on another value leaves `%n` alone.
#[test]
fn test_a_guard_on_another_argument_leaves_this_one() {
    with_guard("  %g = icmp ult i32 %m, 10", |se, cx, header, n, m| {
        assert_eq!(se.apply_loop_guards(cx, n, header), n);
        assert_ne!(se.apply_loop_guards(cx, m, header), m);
    });
}

/// `%n % 8 == 0` makes `%n` into `(%n /u 8) * 8`.
#[test]
fn test_a_divisibility_guard_rewrites_the_argument_as_a_multiple() {
    with_guard("  %r = urem i32 %n, 8\n  %g = icmp eq i32 %r, 0", |se, cx, header, n, _| {
        let eight = constant(se, 8);
        let quotient = se.get_udiv_expr(cx, Use::of(n), Use::of(eight));
        let want = se.get_mul_expr(cx, vec![Use::of(quotient), Use::of(eight)], Default::default(), 0).scev;
        assert_eq!(se.apply_loop_guards(cx, n, header), want);
    });
}

#[test]
fn test_the_entry_guard_proves_what_the_branch_says() {
    with_guard("  %g = icmp ult i32 %n, 10", |se, cx, header, n, _| {
        let (ten, nine, five) = (constant(se, 10), constant(se, 9), constant(se, 5));
        assert!(se.is_loop_entry_guarded_by_cond_pred(cx, header, IntPredicate::Ult, n, ten));
        assert!(se.is_loop_entry_guarded_by_cond_pred(cx, header, IntPredicate::Ule, n, nine));
        assert!(!se.is_loop_entry_guarded_by_cond_pred(cx, header, IntPredicate::Ult, n, five));
    });
    with_guard("  %g = icmp ult i32 %m, 10", |se, cx, header, n, _| {
        let ten = constant(se, 10);
        assert!(!se.is_loop_entry_guarded_by_cond_pred(cx, header, IntPredicate::Ult, n, ten));
    });
}

/// A loop whose header starts with `fact` (an assume or a guard of `%g`), and
/// the answer to whether `%n u< 10` is guarded at the header's entry.
fn guarded_at_entry_by_header_start(fact: &str) -> bool {
    let text = format!(
        "declare void @llvm.assume(i1)
declare void @llvm.experimental.guard(i1, ...)

define void @f(i32 %n, i1 %go) {{
entry:
  %g = icmp ult i32 %n, 10
  br label %h

h:
  {fact}
  br i1 %go, label %h, label %exit

exit:
  ret void
}}
"
    );
    let module = llrm_mir::parse::module(&text).expect("a module");
    let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("@f");
    let layout = DataLayout::default();
    let tree = DomTree::of(function);
    let loops = LoopInfo::analyze(function, || &tree);
    let assumptions = AssumptionCache::new(&module, function);
    let cx = Cx {
        context: &module.context,
        globals: &module.globals,
        metadata: &module.metadata,
        function,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let header = *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let mut se = ScalarEvolution::new();
    let n = se.get_scev(&cx, Operand::Value(function.parameters()[0]));
    let ten = constant(&mut se, 10);
    se.is_basic_block_entry_guarded_by_cond(&cx, header, IntPredicate::Ult, n, ten)
}

/// An `llvm.assume` that opens the header does not hold on the way into the
/// header (`DT.dominates(Inst, BB)` is false inside `BB`): `%n u< 10` is not
/// guarded there. A check that counted the first instruction as dominating
/// its own block answered true, and a loop was entered under a fact it had
/// not yet assumed.
#[test]
fn test_an_assume_that_opens_the_header_is_not_a_guard_of_its_entry() {
    assert!(!guarded_at_entry_by_header_start("call void @llvm.assume(i1 %g)"));
}

/// The same for a guard.
#[test]
fn test_a_guard_that_opens_the_header_is_not_a_guard_of_its_entry() {
    assert!(!guarded_at_entry_by_header_start("call void (i1, ...) @llvm.experimental.guard(i1 %g)"));
}

/// `has_guards` read every instruction of the function for a call to
/// `llvm.experimental.guard`, declared or not: 0.23 M Ir on d_faces -O2. The
/// pin asks the module for the declaration.
#[test]
fn test_a_module_without_the_guard_declaration_reads_no_body_for_guards() {
    let scans = |declaration: &str, call: &str| {
        let text = format!(
            "{declaration}
define void @f(i1 %g) {{
entry:
  {call}
  ret void
}}
"
        );
        let module = llrm_mir::parse::module(&text).expect("a module");
        let (_, _, function) = module.functions().find(|(_, _, function)| !function.is_declaration()).expect("@f");
        let layout = DataLayout::default();
        let tree = DomTree::of(function);
        let loops = LoopInfo::analyze(function, || &tree);
        let assumptions = AssumptionCache::new(&module, function);
        let cx = Cx {
            context: &module.context,
            globals: &module.globals,
            metadata: &module.metadata,
            function,
            layout: &layout,
            loops: &loops,
            tree: &tree,
            assumptions: &assumptions,
        };
        let before = super::implied::guard_scans();
        let has = ScalarEvolution::new().has_guards(&cx);
        (has, super::implied::guard_scans() - before)
    };
    assert_eq!(scans("declare void @h()", "call void @h()"), (false, 0));
    let declared = "declare void @llvm.experimental.guard(i1, ...)";
    assert_eq!(scans(declared, "call void (i1, ...) @llvm.experimental.guard(i1 %g)"), (true, 1));
}
