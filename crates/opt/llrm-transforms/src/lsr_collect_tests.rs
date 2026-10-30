use std::cell::Cell;

use llrm_analysis::scev::expr::Scev;
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId};
use llrm_mir::valuetracking::AssumptionCache;
use llrm_x86::tti::X86;

use crate::ivusers::{Env, IvUsers};
use crate::lsr_collect::*;
use crate::lsr_formula::Immediate;
use crate::lsr_use::Kind;

const STORE: &str = "define void @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a = getelementptr i32, ptr %p, i32 %i
  store i32 0, ptr %a
  %next = add i32 %i, 1
  %c = icmp eq i32 %next, %n
  br i1 %c, label %exit, label %loop
exit:
  ret void
}
";

/// `text`'s `@f`, collected as LSR does before it generates formulae; `body`
/// gets the collection, the analyses and how often `isSafeToExpand` was asked.
fn collected(
    text: &str,
    body: impl FnOnce(&Cx, &mut ScalarEvolution, &Collection, BlockId, usize),
) {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().next().expect("a function");
    let layout = DataLayout::parse("e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32").unwrap();
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
    let mut se = ScalarEvolution::new();
    let header = loops.top_level_loops().next().expect("a loop").get_header();
    let env = Env { safe_to_speculate: &|_: InstId| true, ephemeral: &|_| false };
    let users = IvUsers::of(&mut se, &cx, header, &env);
    let latch_terminator = {
        let lp = loops.get_loop_for(header).expect("a loop");
        function.terminator(lp.get_loop_latch(function).expect("a latch")).expect("a terminator")
    };
    let tti = X86::of_cpu("core2").unwrap();
    let asked = Cell::new(0);
    let safe = |_: &mut ScalarEvolution, _: Scev| {
        asked.set(asked.get() + 1);
        true
    };
    let mut lsr = Collection::new(&cx, &tti, header, &safe, false, latch_terminator);
    lsr.collect_interesting_types_and_factors(&mut se, &users);
    lsr.collect_fixups_and_initial_formulae(&mut se, &users);
    lsr.collect_loop_invariant_fixups_and_formulae(&mut se);
    body(&cx, &mut se, &lsr, header, asked.get());
}

/// The store of `p[i]` is an address use of `{%p,+,4}` and the exit compare
/// `next == n` is `n - next == 0`: a compare use of `{(-1 + %n),+,-1}` (an
/// immediate does not fold into a compare with a scaled register, so it stays
/// in the expression). Each has its one fixup and one formula, the loop's
/// recurrence whole.
#[test]
fn an_array_store_and_the_exit_compare_are_two_uses() {
    collected(STORE, |_, _, lsr, _, _| {
        let kinds: Vec<Kind> = lsr.uses.iter().map(|lu| lu.kind).collect();
        assert_eq!(kinds.len(), 2, "{kinds:?}");
        assert!(kinds.contains(&Kind::Address) && kinds.contains(&Kind::ICmpZero), "{kinds:?}");
        for lu in &lsr.uses {
            assert_eq!(lu.fixups.len(), 1);
            assert_eq!(lu.formulae.len(), 1);
            assert_eq!((lu.min_offset, lu.max_offset), (Immediate::ZERO, Immediate::ZERO));
            assert_eq!(lu.formulae[0].base_regs.len(), 1);
        }
        assert!(lsr.uses.iter().all(|lu| lu.all_fixups_unconditional && !lu.all_fixups_outside_loop));
    });
}

/// 4 and 1 are the loop's strides: 4 is the factor between them, and the
/// equality compare makes -4 and -1 interesting too. All uses are of one type,
/// so none is for truncation.
#[test]
fn factors_are_the_ratios_of_strides_and_a_compare_adds_their_negations() {
    collected(STORE, |_, _, lsr, _, _| {
        let mut factors: Vec<i64> = lsr.factors.iter().copied().collect();
        factors.sort_unstable();
        assert_eq!(factors, [-4, -1, 4]);
        assert!(lsr.types.is_empty());
    });
}

/// Both registers are counted for their uses, and the baseline cost is the two
/// first formulae together: two recurrences, two registers, and their setup.
#[test]
fn the_baseline_cost_is_the_first_formulae_together() {
    collected(STORE, |_, _, lsr, _, _| {
        assert_eq!(lsr.reg_uses.registers().len(), 2);
        let c = lsr.baseline_cost.c;
        assert_eq!((c.num_regs, c.add_rec_cost, c.insns), (2, 2, 2), "{c:?}");
        assert!(!lsr.baseline_cost.is_loser());
    });
}

/// Stores at `p[i+k]` share one use: the immediates come out of the
/// expression, the fixups keep their offsets, and the use's range spans them.
/// One expression is asked about once, however many fixups.
#[test]
fn stores_at_constant_offsets_share_one_use_and_its_range_grows() {
    let looped = |n: usize| {
        let mut text = String::from(
            "define void @f(ptr %p, i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %next, %loop ]\n",
        );
        for k in 0..n {
            text += &format!(
                "  %j{k} = add i32 %i, {k}\n  %a{k} = getelementptr i32, ptr %p, i32 %j{k}\n  store i32 0, ptr %a{k}\n"
            );
        }
        text += "  %next = add i32 %i, 1\n  %c = icmp eq i32 %next, %n\n  br i1 %c, label %exit, label %loop\nexit:\n  ret void\n}\n";
        text
    };
    let mut asked_for = Vec::new();
    for n in [50usize, 100] {
        collected(&looped(n), |_, _, lsr, _, asked| {
            let address: Vec<_> = lsr.uses.iter().filter(|lu| lu.kind == Kind::Address).collect();
            assert_eq!(address.len(), 1, "one use for the stores");
            assert_eq!(address[0].fixups.len(), n);
            assert_eq!((address[0].min_offset, address[0].max_offset), (Immediate(0), Immediate(4 * (n as i64 - 1))));
            asked_for.push(asked);
        });
    }
    assert_eq!(asked_for[0], asked_for[1], "twice the stores, the same questions: {asked_for:?}");
}

/// `isAlwaysFoldable` folds an immediate into an address and not into a plain
/// use; `getExactSDiv` divides the strides exactly.
#[test]
fn an_immediate_folds_into_an_address_and_not_into_a_basic_use() {
    let tti = X86::of_cpu("core2").unwrap();
    assert!(is_always_foldable(&tti, Kind::Address, 0, false, Immediate(8), true));
    assert!(!is_always_foldable(&tti, Kind::Basic, 0, false, Immediate(8), true));
    assert!(is_always_foldable(&tti, Kind::Basic, 0, false, Immediate::ZERO, true));
}

#[test]
fn an_exact_division_has_no_remainder() {
    collected(STORE, |cx, se, _, _, _| {
        use llrm_analysis::scev::expr::Ty;
        let c = |se: &mut ScalarEvolution, v| se.get_constant_of_type(cx, Ty::Int(32), v);
        let (twelve, four, five) = (c(se, 12), c(se, 4), c(se, 5));
        let three = get_exact_sdiv(se, cx, twelve, four, false).expect("12 /s 4");
        assert_eq!(se.as_constant(three).map(|one| one.sext_value()), Some(3));
        assert_eq!(get_exact_sdiv(se, cx, twelve, five, false), None);
        let same = get_exact_sdiv(se, cx, five, five, false).expect("x /s x");
        assert_eq!(se.as_constant(same).map(|one| one.sext_value()), Some(1));
    });
}

/// The immediate comes out of the sum and out of the start of a recurrence; a
/// recurrence with none is as it was.
#[test]
fn an_immediate_is_taken_out_of_a_sum_and_a_recurrence_start() {
    collected(STORE, |cx, se, lsr, _, _| {
        use llrm_analysis::scev::Flags;
        use llrm_analysis::scev::expr::{Ty, Use as ScevUse};
        let n = se.get_unknown(cx, llrm_mir::module::Operand::Value(cx.function.parameters()[1]));
        let seven = se.get_constant_of_type(cx, Ty::Int(32), 7);
        let sum = se.get_add_expr(cx, vec![ScevUse::of(seven), ScevUse::of(n)], Flags::default(), 0).scev;
        let (offset, rest) = extract_immediate(se, cx, sum);
        assert_eq!((offset, rest), (Immediate(7), n));
        let ar = se.get_add_rec_expr(cx, vec![ScevUse::of(sum), ScevUse::of(seven)], lsr.l, Flags::default()).scev;
        let (offset, rest) = extract_immediate(se, cx, ar);
        assert_eq!(offset, Immediate(7));
        assert_eq!(se.operands(rest)[0].scev, n);
        let (none, same) = extract_immediate(se, cx, n);
        assert_eq!((none, same), (Immediate::ZERO, n));
    });
}
