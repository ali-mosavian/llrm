//! Trip counts over expressions built by hand: `createSCEV` (S3) does not
//! yet turn a loop's phis into add recurrences, so the loop-level counts wait
//! for it. Here: the chain evaluation and the range solves that
//! `howFarToZero` and `howManyLessThans` are built on, against values worked
//! out by hand.

use llrm_mir::constantrange::ConstantRange;
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::BlockId;
use llrm_mir::valuetracking::AssumptionCache;

use super::ap::Ap;
use super::expr::{Scev, Use};
use super::{Cx, Flags, ScalarEvolution};

const LOOP: &str = "define void @f(i32 %n) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret void
}
";

/// A `ScalarEvolution` over `LOOP`, and the loop's header.
fn with_loop(test: impl FnOnce(&mut ScalarEvolution, &Cx, BlockId)) {
    let module = llrm_mir::parse::module(LOOP).expect("a module");
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
    test(&mut ScalarEvolution::new(), &cx, header);
}

fn constant(
    se: &mut ScalarEvolution,
    width: u32,
    value: u128,
) -> Scev {
    se.get_constant(Ap::new(width, value))
}

fn chain(
    se: &mut ScalarEvolution,
    cx: &Cx,
    header: BlockId,
    terms: &[u128],
) -> Scev {
    let operands = terms.iter().map(|&one| Use::of(constant(se, 32, one))).collect();
    se.get_add_rec_expr(cx, operands, header, Flags::default()).scev
}

/// `{A,+,B,+,C}` at iteration `n` is `A + B*n + C*n*(n-1)/2`.
#[test]
fn test_a_chain_of_recurrences_is_evaluated_by_binomial_coefficients() {
    with_loop(|se, cx, header| {
        let add_rec = chain(se, cx, header, &[1, 2, 3]);
        for (n, want) in [(0u128, 1u128), (1, 3), (5, 1 + 2 * 5 + 3 * 10), (100, 1 + 200 + 3 * 4950)] {
            let it = constant(se, 32, n);
            let got = se.evaluate_at_iteration(cx, add_rec, it);
            assert_eq!(se.as_constant(got), Some(Ap::new(32, want)), "at iteration {n}");
        }
    });
}

/// `{0,+,A}` stays in `[lo, hi)` until `x * A` leaves it.
#[test]
fn test_the_iterations_of_an_affine_recurrence_in_a_range_are_counted() {
    with_loop(|se, cx, header| {
        let up = chain(se, cx, header, &[0, 3]);
        // 0, 3, 6, 9 are below 10; 12 is not: 4 iterations.
        let range = ConstantRange::make_exact_icmp_region(llrm_mir::opcode::IntPredicate::Ult, 32, 10);
        let got = se.get_num_iterations_in_range(cx, up, &range);
        assert_eq!(se.as_constant(got), Some(Ap::new(32, 4)));
        // A range that does not hold zero is left on the first iteration.
        let away = ConstantRange::make_exact_icmp_region(llrm_mir::opcode::IntPredicate::Ugt, 32, 10);
        let got = se.get_num_iterations_in_range(cx, up, &away);
        assert_eq!(se.as_constant(got), Some(Ap::new(32, 0)));
        // A full range never leaves.
        let full = ConstantRange::make_exact_icmp_region(llrm_mir::opcode::IntPredicate::Uge, 32, 0);
        assert_eq!(se.get_num_iterations_in_range(cx, up, &full), se.get_could_not_compute());
    });
}

/// `A * X = B (mod 2^32)` for a power-of-two `A`, through `howFarToZero`
/// of the constant-start recurrence `{start,+,step}`: the guards are seams
/// (`applyLoopGuards`, S5), so only the constant cases that reach none.
#[test]
fn test_a_constant_exit_value_is_zero_trips_or_infinite() {
    with_loop(|se, cx, header| {
        let zero = constant(se, 32, 0);
        let limit = se.how_far_to_zero(cx, zero, header, true, false);
        assert_eq!(limit.exact_not_taken, zero, "already zero: the backedge is taken no times");
        let five = constant(se, 32, 5);
        let limit = se.how_far_to_zero(cx, five, header, true, false);
        assert_eq!(limit.exact_not_taken, se.get_could_not_compute(), "a constant other than zero loops forever");
    });
}

/// `howFarToZero` took the guards of the loop once for the whole query, as the
/// pin does (`LoopGuards Guards = LoopGuards::collect(L, *this)`), and applied
/// them to each expression. Ours collected them again for each of the five
/// expressions: 226 collections over d_faces -O2's LSR, 9 M of its 40 M
/// instructions, where the pin's whole `howFarToZero` costs 0.6 M.
#[test]
fn test_how_far_to_zero_collects_the_loop_guards_once() {
    with_loop(|se, cx, header| {
        let v = chain(se, cx, header, &[7, 3]);
        let before = super::guards::collections();
        let limit = se.how_far_to_zero(cx, v, header, true, false);
        assert_ne!(limit.exact_not_taken, se.get_could_not_compute(), "the query reaches the guards");
        assert_eq!(super::guards::collections() - before, 1);
    });
}

/// `{-49,+,1,+,2}` is `n*n - 49`: the pinned `print<scalar-evolution>` takes
/// the backedge 7 times (ten quadratic loops of `tools/scev/counts.py` agree).
/// It was CouldNotCompute (D-T2). `n*n - 50` has no root: still unknown.
#[test]
fn test_a_quadratic_recurrence_is_zero_at_its_root() {
    with_loop(|se, cx, header| {
        let root = chain(se, cx, header, &[(1u128 << 32) - 49, 1, 2]);
        let limit = se.how_far_to_zero(cx, root, header, true, false);
        assert_eq!(se.as_constant(limit.exact_not_taken), Some(Ap::new(32, 7)));
        let none = chain(se, cx, header, &[(1u128 << 32) - 50, 1, 2]);
        let limit = se.how_far_to_zero(cx, none, header, true, false);
        assert_eq!(limit.exact_not_taken, se.get_could_not_compute());
    });
}

/// `{0,+,1,+,2}` is `n*n`: 0, 1, 4, ..., 49 are below 50, 64 is not.
#[test]
fn test_the_iterations_of_a_quadratic_recurrence_in_a_range_are_counted() {
    with_loop(|se, cx, header| {
        let squares = chain(se, cx, header, &[0, 1, 2]);
        let range = ConstantRange::make_exact_icmp_region(llrm_mir::opcode::IntPredicate::Ult, 32, 50);
        let got = se.get_num_iterations_in_range(cx, squares, &range);
        assert_eq!(se.as_constant(got), Some(Ap::new(32, 8)));
    });
}
