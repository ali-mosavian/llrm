use llrm_analysis::scev::expr::{Scev, Ty};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::valuetracking::AssumptionCache;
use llrm_x86::tti::X86;

use crate::ivusers::{Env, IvUsers};
use crate::lsr_collect::Collection;
use crate::lsr_formula::{Formula, MemAccessTy};
use crate::lsr_use::{Fixup, Kind, Use, is_legal_use_formula};

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

/// `text`'s `@f` collected, then with its formulae generated as LSR does.
fn generated(
    text: &str,
    body: impl FnOnce(&Cx, &mut ScalarEvolution, &mut Collection, BlockId),
) {
    generated_for("core2", text, body)
}

/// [`generated`] for the x86 profile `cpu`.
fn generated_for(
    cpu: &str,
    text: &str,
    body: impl FnOnce(&Cx, &mut ScalarEvolution, &mut Collection, BlockId),
) {
    generated_marked(cpu, text, false, body)
}

/// [`generated_for`], with the exit tests marked to take the post-incremented
/// value (`OptimizeLoopTermCond`) where `post_inc`.
fn generated_marked(
    cpu: &str,
    text: &str,
    post_inc: bool,
    body: impl FnOnce(&Cx, &mut ScalarEvolution, &mut Collection, BlockId),
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
    let mut users = IvUsers::of(&mut se, &cx, header, &env);
    if post_inc {
        let conds: Vec<InstId> = users
            .uses
            .iter()
            .filter(|u| matches!(
                cx.function.instruction(u.user).opcode,
                llrm_mir::opcode::Opcode::ICmp(_)
            ))
            .map(|u| u.user)
            .collect();
        for cond in conds {
            users.transform_to_post_inc(cond, header);
        }
    }
    let latch_terminator = {
        let lp = loops.get_loop_for(header).expect("a loop");
        function.terminator(lp.get_loop_latch(function).expect("a latch")).expect("a terminator")
    };
    let tti = X86::of_cpu(cpu).unwrap();
    let safe = |_: &mut ScalarEvolution, _: Scev| true;
    let mut lsr = Collection::new(&cx, &tti, header, &safe, false, latch_terminator);
    lsr.collect_interesting_types_and_factors(&mut se, &users);
    lsr.collect_fixups_and_initial_formulae(&mut se, &users);
    lsr.collect_loop_invariant_fixups_and_formulae(&mut se);
    lsr.generate_all_reuse_formulae(&mut se);
    body(&cx, &mut se, &mut lsr, header);
}

/// `n - next == 0`, whose recurrence is `{(-1 + %n),+,-1}`: reassociating its
/// start gives `%n` and `{-1,+,-1}` (the recurrence the scaled register) and
/// `(-1 + %n)` and `{0,+,-1}`, as `CollectSubexprs` splits the add.
#[test]
fn the_exit_compare_is_reassociated_around_its_recurrence() {
    generated(STORE, |cx, se, lsr, _| {
        let cmp = lsr.uses.iter().find(|lu| lu.kind == Kind::ICmpZero).expect("the compare");
        let n = se.get_unknown(cx, Operand::Value(cx.function.parameters()[1]));
        let has = |se: &ScalarEvolution, f: &Formula, base: Scev, offset: i128| {
            let Some(scaled) = f.scaled_reg else { return false };
            f.base_regs == [base]
                && f.scale == 1
                && se.operands(scaled).len() == 2
                && se.as_constant(se.operands(scaled)[0].scev).map(|c| c.sext_value()) == Some(offset)
        };
        assert!(cmp.formulae.iter().any(|f| has(se, f, n, -1)), "{:?}", cmp.formulae);
        let minus_one_n = {
            let minus = se.get_constant_of_type(cx, Ty::Int(32), -1);
            se.get_add_expr(
                cx,
                vec![llrm_analysis::scev::expr::Use::of(minus), llrm_analysis::scev::expr::Use::of(n)],
                llrm_analysis::scev::Flags::default(),
                0,
            )
            .scev
        };
        assert!(cmp.formulae.iter().any(|f| has(se, f, minus_one_n, 0)), "{:?}", cmp.formulae);
    });
}

/// `{%p,+,4}` is `%p + {0,+,4}` (the start of the loop's recurrence is split
/// off), and with the factor 4 between the strides, `%p + 4*{0,+,1}`. The
/// compare's `n - next` scaled by -1 is `{1,+,1}`-based, a register the store
/// can share at a distance of one element: `%p - 4 + 4*{1,+,1}`. That is
/// LSR's result for a loop of an array walk and its exit test: one induction
/// variable for both.
#[test]
fn the_store_can_share_the_compares_induction_variable() {
    generated(STORE, |_, _, lsr, _| {
        let cmp_idx = lsr.uses.iter().position(|lu| lu.kind == Kind::ICmpZero).expect("the compare");
        let address = lsr.uses.iter().find(|lu| lu.kind == Kind::Address).expect("the store");
        let shared = address
            .formulae
            .iter()
            .find(|f| {
                f.scale == 4
                    && f.base_offset.0 == -4
                    && f.scaled_reg.is_some_and(|reg| lsr.uses[cmp_idx].regs.contains(&reg))
            })
            .unwrap_or_else(|| panic!("{:?}", address.formulae));
        assert_eq!(shared.base_regs.len(), 1);
        // And the first formula is still there.
        assert!(address.formulae.iter().any(|f| f.scale == 0 && f.base_regs.len() == 1));
    });
}

/// Whatever was generated is legal for its use, canonical, and one per set of
/// registers.
#[test]
fn every_generated_formula_is_legal_canonical_and_distinct() {
    generated(STORE, |_, se, lsr, header| {
        let tti = X86::of_cpu("core2").unwrap();
        for lu in &lsr.uses {
            let mut keys = llrm_support::hash::HashSet::default();
            for f in &lu.formulae {
                assert!(f.is_canonical(se, header), "{f:?}");
                assert!(is_legal_use_formula(&tti, lu.min_offset, lu.max_offset, lu.kind, lu.access_ty, f), "{f:?}");
                let mut key = f.base_regs.clone();
                key.extend(f.scaled_reg);
                key.sort_unstable();
                assert!(keys.insert(key), "two formulae with one set of registers: {:?}", lu.formulae);
            }
        }
    });
}

/// Twice the pointers walked, twice the formulae offered: the generators do
/// work per use, bounded by its formulae.
#[test]
fn generation_is_linear_in_the_uses() {
    let looped = |n: usize| {
        let mut text = String::from("define void @f(");
        for k in 0..n {
            text += &format!("ptr %p{k}, ");
        }
        text += "i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %next, %loop ]\n";
        for k in 0..n {
            text += &format!("  %a{k} = getelementptr i32, ptr %p{k}, i32 %i\n  store i32 0, ptr %a{k}\n");
        }
        text += "  %next = add i32 %i, 1\n  %c = icmp eq i32 %next, %n\n  br i1 %c, label %exit, label %loop\nexit:\n  ret void\n}\n";
        text
    };
    let mut attempts = Vec::new();
    for n in [20usize, 40] {
        generated(&looped(n), |_, _, lsr, _| {
            assert_eq!(lsr.uses.len(), n + 1);
            attempts.push(lsr.insert_attempts);
        });
    }
    assert!(attempts[1] <= 2 * attempts[0] + 8, "{attempts:?}");
}

/// Four uses of twenty formulae each are 160000 combinations, past the limit of
/// 65535: the register the most uses share wins, and each use keeps only the
/// formulae that have it.
#[test]
fn too_many_formulae_are_narrowed_to_the_shared_register() {
    generated(STORE, |cx, se, lsr, _| {
        lsr.uses.clear();
        lsr.reg_uses.clear();
        let registers: Vec<Scev> = (1..=20).map(|v| se.get_constant_of_type(cx, Ty::Int(32), v)).collect();
        for lu_idx in 0..4 {
            let mut lu = Use::new(Kind::Basic, MemAccessTy::unknown(0));
            lu.push_fixup(Fixup::new(InstId(0), Operand::Value(cx.function.parameters()[0])));
            lsr.uses.push(lu);
            for &reg in &registers {
                let f = Formula { base_regs: vec![reg], has_base_reg: true, ..Formula::default() };
                assert!(lsr.insert_formula(se, lu_idx, &f));
            }
        }
        assert!(lsr.estimate_search_space_complexity() >= 65535);
        lsr.narrow_search_space_using_heuristics(se);
        assert!(lsr.estimate_search_space_complexity() < 65535);
        // Every use is left with the one register they all share.
        let first = lsr.uses[0].formulae.clone();
        assert!(
            lsr.uses.iter().all(|lu| lu.formulae.len() == first.len() && lu.formulae == first),
            "{:?}",
            lsr.uses.iter().map(|lu| lu.formulae.len()).collect::<Vec<_>>()
        );
    });
}

/// `opt -passes=loop-reduce` (LLVM 20, i386) on this loop gives a pointer
/// advancing by 4 for the store and a counter from `%n` down to zero for the
/// exit test: each use keeps its first formula, one register each, where
/// sharing one induction variable would take a scaled index and a base.
#[test]
fn the_solver_keeps_the_pointer_walk_and_counts_the_exit_down() {
    generated_for("i486", STORE, |_, se, lsr, _| {
        let solution = lsr.solve(se);
        assert_eq!(solution.len(), 2);
        for (lu, &f_idx) in lsr.uses.iter().zip(&solution) {
            let f = &lu.formulae[f_idx];
            assert_eq!((f.base_regs.len(), f.scaled_reg, f.scale), (1, None, 0), "{:?} {f:?}", lu.kind);
            let reg = f.base_regs[0];
            let step = se.as_constant(se.operands(reg)[1].scev).map(|c| c.sext_value());
            match lu.kind {
                Kind::Address => assert_eq!(step, Some(4)),
                Kind::ICmpZero => {
                    assert_eq!(step, Some(-1));
                    assert!(f.counts_down_to_zero(se));
                }
                other => panic!("{other:?}"),
            }
        }
    });
}

/// Of two uses that could each take either of two constants as their register,
/// the solver picks the one both take: one register where two would be.
#[test]
fn uses_that_can_share_a_register_do() {
    generated(STORE, |cx, se, lsr, _| {
        lsr.uses.clear();
        lsr.reg_uses.clear();
        let reg = |se: &mut ScalarEvolution, v| se.get_constant_of_type(cx, Ty::Int(32), v);
        let (r1, r2, r3) = (reg(se, 1), reg(se, 2), reg(se, 3));
        for regs in [[r1, r2], [r2, r3]] {
            let lu_idx = lsr.uses.len();
            let mut lu = Use::new(Kind::Basic, MemAccessTy::unknown(0));
            lu.push_fixup(Fixup::new(InstId(0), Operand::Value(cx.function.parameters()[0])));
            lsr.uses.push(lu);
            for reg in regs {
                let f = Formula { base_regs: vec![reg], has_base_reg: true, ..Formula::default() };
                assert!(lsr.insert_formula(se, lu_idx, &f));
            }
        }
        let solution = lsr.solve(se);
        let chosen: Vec<Scev> = lsr.uses.iter().zip(&solution).map(|(lu, &f)| lu.formulae[f].base_regs[0]).collect();
        assert_eq!(chosen, [r2, r2]);
    });
}

/// Work grows with the uses as the solver prunes it: the formulae offered and
/// the formulae rated by the filter, the narrowing and the solver, at 8, 16 and
/// 32 pointers walked together, each at most about double when the uses double.
#[test]
fn the_whole_search_is_linear_in_the_uses() {
    let looped = |n: usize| {
        let mut text = String::from("define void @f(");
        for k in 0..n {
            text += &format!("ptr %p{k}, ");
        }
        text += "i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %next, %loop ]\n";
        for k in 0..n {
            text += &format!("  %a{k} = getelementptr i32, ptr %p{k}, i32 %i\n  store i32 0, ptr %a{k}\n");
        }
        text += "  %next = add i32 %i, 1\n  %c = icmp eq i32 %next, %n\n  br i1 %c, label %exit, label %loop\nexit:\n  ret void\n}\n";
        text
    };
    let mut work = Vec::new();
    for n in [8usize, 16, 32] {
        generated_for("i486", &looped(n), |_, se, lsr, _| {
            lsr.filter_out_undesirable_dedicated_registers(se);
            lsr.narrow_search_space_using_heuristics(se);
            let solution = lsr.solve(se);
            assert_eq!(solution.len(), n + 1);
            work.push((lsr.insert_attempts, lsr.rated.get()));
        });
    }
    for pair in work.windows(2) {
        assert!(pair[1].0 <= 2 * pair[0].0 + 8 && pair[1].1 <= 2 * pair[0].1 + 8, "{work:?}");
    }
}
