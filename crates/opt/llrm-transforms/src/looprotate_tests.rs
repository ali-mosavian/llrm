//! `rotate_loops` on loops the interpreter runs, before and after.

use llrm_mir::module::Module;
use llrm_x86::tti::X86;

use super::{Options, rotate_function, rotate_function_knowing};
use crate::testing::{parsed, printed, results};

/// The loops of @f rotated with the pass's own options (header limit 16,
/// header duplication on); whether any was.
fn rotated(module: &mut Module) -> bool {
    // The loop pass manager hands the pass loops in simplified, LCSSA form.
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options = Options {
        max_header_size: 16,
        priced_at_size: false,
        guard_must_fold: false,
        rotation_only: false,
        is_util_mode: false,
        prepare_for_lto: false,
        check_exit_count: false,
    };
    rotate_function(module, "f", &tti, options).expect("rotates")
}

const INPUTS: &[&[i128]] = &[&[1, 2], &[0, 5], &[-3, 7], &[100, -1], &[0, 0]];

/// Test in the header, a body in its own block: `for (i = 0; i < n; i++)`.
const COUNTED: &str = "define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %m = mul i16 %acc, 3
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = phi i16 [ %acc, %b1 ]
  ret i16 %r
}
";

#[test]
fn test_a_counted_loop_is_entered_behind_a_guard_and_computes_the_same() {
    let mut module = parsed(COUNTED);
    let before = results(&module, INPUTS);
    assert!(rotated(&mut module));
    let text = printed(&module);
    assert_eq!(results(&module, INPUTS), before, "{text}");
    // The entry ends in the guard, a copy of the test on the first values, and
    // the test is in the loop's block alone (the old header merged into the
    // latch).
    let entry = text.split("\n\n").next().unwrap();
    assert!(entry.contains("icmp slt i16 0, %n") && entry.contains("br i1 %0"), "{text}");
    assert_eq!(text.matches("icmp slt").count(), 2, "{text}");
}

/// What @f returns for each of `inputs`, or how it fails (a loop that does not
/// end for some input does not end after).
fn outcomes(
    module: &Module,
    inputs: &[&[i128]],
) -> Vec<Result<llrm_mir::interpret::Val, String>> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).unwrap();
    let widths: Vec<u32> = function
        .parameters()
        .iter()
        .map(|&one| module.context.types.int_bits(function.value(one).ty).expect("an integer"))
        .collect();
    inputs
        .iter()
        .map(|input| {
            let arguments = input
                .iter()
                .zip(&widths)
                .map(|(&value, &width)| llrm_mir::interpret::Val::Int {
                    bits: llrm_mir::context::mask(width) & value as u128,
                    width,
                })
                .collect();
            llrm_mir::interpret::run(module, "f", arguments, 20_000).map_err(|trap| format!("{trap:?}"))
        })
        .collect()
}

fn fixture_inputs(module: &Module) -> Option<Vec<Vec<i128>>> {
    let (_, _, function) = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f"))?;
    let widths: Option<Vec<u32>> =
        function.parameters().iter().map(|&one| module.context.types.int_bits(function.value(one).ty)).collect();
    let widths = widths?;
    if widths.iter().any(|&width| width == 1) {
        return None;
    }
    Some(
        [0i128, 1, 2, 5, 11, -3]
            .iter()
            .flat_map(|&x| [0i128, 1, 4, 9, 20].iter().map(move |&n| (x, n)))
            .map(|(x, n)| (0..widths.len()).map(|at| if at == 0 { x } else { n + at as i128 }).collect())
            .collect(),
    )
}

/// Every fixture of the oracle corpus that takes integers computes the same
/// before and after, and verifies after: the loops the oracle rotates like
/// `opt` rotate here to the same results.
#[test]
fn test_every_oracle_fixture_computes_the_same_after_rotation() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../tests/fixtures/rotate");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("the fixtures") {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        let mut module = parsed(&text);
        let Some(inputs) = fixture_inputs(&module) else { continue };
        let inputs: Vec<&[i128]> = inputs.iter().map(Vec::as_slice).collect();
        let before = outcomes(&module, &inputs);
        rotated(&mut module);
        let text = printed(&module);
        assert_eq!(outcomes(&module, &inputs), before, "{}\n{text}", path.display());
        checked += 1;
    }
    assert!(checked >= 10, "{checked} fixtures run");
}

/// `n` loops one after the other, each `for (i = 0; i < n; i++) acc += i`.
fn sequential_loops(n: usize) -> String {
    let mut text = String::from("define i16 @f(i16 %x, i16 %n) {\nb0:\n  br label %h0\n\n");
    for k in 0..n {
        let (this, next) = (format!("h{k}"), if k + 1 == n { "done".to_owned() } else { format!("h{}", k + 1) });
        let before = if k == 0 { "b0".to_owned() } else { format!("e{}", k - 1) };
        let acc_in = if k == 0 { "%x".to_owned() } else { format!("%r{}", k - 1) };
        text += &format!(
            "{this}:\n  %i{k} = phi i16 [ 0, %{before} ], [ %n{k}, %c{k} ]\n  %a{k} = phi i16 [ {acc_in}, %{before} ], [ %s{k}, %c{k} ]\n  %g{k} = icmp slt i16 %i{k}, %n\n  br i1 %g{k}, label %c{k}, label %e{k}\n\nc{k}:\n  %s{k} = add i16 %a{k}, %i{k}\n  %n{k} = add i16 %i{k}, 1\n  br label %{this}\n\ne{k}:\n  %r{k} = phi i16 [ %a{k}, %{this} ]\n  br label %{next}\n\n"
        );
    }
    text += &format!("done:\n  ret i16 %r{}\n}}\n", n - 1);
    text
}

/// Rotating `n` loops one after the other scans entries of the layout and of
/// use lists in proportion to `n`: a `LoopInfo` made again after each rotated
/// loop made it `n^2` (a count of entries looked over, not seconds: time fails
/// under load).
#[test]
fn test_rotating_loops_in_sequence_scans_in_proportion_to_their_number() {
    use llrm_mir::edit::layout_scanned;
    let scanned = |n: usize| {
        let mut module = parsed(&sequential_loops(n));
        let before = layout_scanned();
        assert!(rotated(&mut module));
        printed(&module);
        layout_scanned() - before
    };
    let (a, b, c) = (scanned(64), scanned(128), scanned(256));
    assert!(b * 100 <= a * 250 && c * 100 <= b * 250, "64: {a}, 128: {b}, 256: {c}");
}

/// A target with no cost model is priced by the generic profile, and the pass
/// names no target crate: a release build of the bins failed on `llrm_x86`
/// used outside the dev-dependencies (`cargo check --bins` is the test).
#[test]
fn test_the_slot_gets_its_cost_model_from_the_target() {
    let module = parsed("define i16 @f() {\nb:\n  ret i16 0\n}\n");
    let neutral = llrm_mir::passes::Outer::of(&module, None);
    // The generic profile: a phi is free, a branch costs one.
    let generic = super::target_tti(neutral.target());
    assert_eq!(generic.cf_instr_cost(true, llrm_mir::tti::CostKind::CodeSize), llrm_mir::tti::Cost::FREE);
    assert_eq!(generic.cf_instr_cost(false, llrm_mir::tti::CostKind::CodeSize), llrm_mir::tti::Cost::BASIC);
}

/// A callee made after the pass manager's snapshot of the globals (a clone an
/// earlier pass of the run created) indexed past it: `looprotate.rs:860`
/// "index out of bounds" on jpeg_single and jpeg_fixed at -Os with rotate on.
#[test]
fn test_a_callee_made_after_the_globals_snapshot_does_not_panic_the_rotation() {
    let mut module = parsed(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %v = call i16 @late(i16 %i)
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %i
}

declare i16 @late(i16)
",
    );
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options = Options {
        max_header_size: 16,
        priced_at_size: false,
        guard_must_fold: false,
        rotation_only: false,
        is_util_mode: false,
        prepare_for_lto: false,
        check_exit_count: false,
    };
    let known = module.functions().position(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f") + 1;
    assert!(rotate_function_knowing(&mut module, "f", &tti, options, known).expect("rotates"), "{}", printed(&module));
}

/// The block that splits a loop's exit edge stands right before the exit
/// (`ExitSplit->moveBefore(Exit)`, LoopRotationUtils.cpp:788), not after the
/// loop block the edge leaves.
#[test]
fn test_the_block_that_splits_a_loop_exit_stands_before_the_exit() {
    let mut module = parsed(
        "define i16 @f(i16 %n, i1 %c) {
entry:
  br label %oh

oh:
  %i = phi i16 [ 0, %entry ], [ %in, %ol ]
  %og = icmp slt i16 %i, %n
  br i1 %og, label %ih, label %exit

ih:
  %j = phi i16 [ 0, %oh ], [ %jn, %ib ]
  %jg = icmp slt i16 %j, %n
  br i1 %jg, label %ib, label %exit

ib:
  %jn = add i16 %j, 1
  br i1 %c, label %ih, label %ol

ol:
  %in = add i16 %i, 1
  br label %oh

exit:
  ret i16 %n
}
",
    );
    assert!(rotated(&mut module));
    let function = module.function_mut("f").expect("@f").1;
    let order: Vec<String> =
        function.layout().iter().map(|&one| function.block(one).name.clone().unwrap_or("new".into())).collect();
    assert_eq!(
        order,
        ["entry", "new", "new", "ih", "ib", "ol", "new", "new", "new", "exit"],
        "{}",
        llrm_mir::print::module(&module)
    );
}

/// quicksort's tail-recursion loop holds a loop that starts at the outer
/// test's own operands. LLVM rotates the inner loop first, so the copy of its
/// test sits behind the outer test and folds to true; outside in, the copy
/// stayed as a fourth `icmp slt`, which `decide@809` cleaned up and the pin
/// never builds (+48 B on quicksort once that pass is gone).
#[test]
fn test_an_inner_loop_is_rotated_before_the_loop_around_it() {
    let mut module = parsed(
        "define void @f(i32 %lo, i32 %hi, ptr %a) {
entry:
  br label %outer

outer:
  %l = phi i32 [ %lo, %entry ], [ %next, %olatch ]
  %c = icmp slt i32 %l, %hi
  br i1 %c, label %pre, label %exit

pre:
  br label %inner

inner:
  %j = phi i32 [ %l, %pre ], [ %j2, %ibody ]
  %c2 = icmp slt i32 %j, %hi
  br i1 %c2, label %ibody, label %olatch

ibody:
  %p = getelementptr i32, ptr %a, i32 %j
  store i32 %j, ptr %p
  %j2 = add i32 %j, 1
  br label %inner

olatch:
  %next = add i32 %j, 1
  br label %outer

exit:
  ret void
}
",
    );
    assert!(rotated(&mut module));
    let out = printed(&module);
    assert_eq!(out.matches("icmp slt").count(), 3, "{out}");
}

/// observed.c's `mix` at -O2 -g lost the debug record of its parameter `n`,
/// which stands before the preheader's branch into the loop: the rotation
/// erased that branch and its records with it, and gdb read `n` as optimised
/// out where it is in a register throughout.
#[test]
fn test_the_debug_records_before_the_preheaders_branch_survive_rotation() {
    let mut module = parsed(
        "define i16 @f(i16 %x, i16 %n) {
b0:
  #dbg_value(i16 %n, !1)
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %s, %b2 ]
  %go = icmp slt i16 %i, 4
  br i1 %go, label %b2, label %b3

b2:
  %m = mul i16 %i, %n
  %s = add i16 %m, %acc
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = add i16 %acc, %n
  ret i16 %r
}

!1 = !{!\"f\", !\"n\", !2, i64 0, i64 1, i64 0}
!2 = !{!\"int\"}
",
    );
    assert!(rotated(&mut module));
    let text = printed(&module);
    assert!(text.contains("#dbg_value(i16 %n"), "{text}");
}

/// -Os cut the header limit to 4 on every target; the byte sweep that showed it
/// to be smaller was m16 only (a target states its own limit). A target with
/// none keeps the pin's 16.
#[test]
fn test_the_size_header_limit_is_the_targets_own() {
    use llrm_mir::target::Neutral;
    assert_eq!(super::header_limit(true, true, &Neutral, &llrm_mir::tti::Basic), 16);
    assert_eq!(super::header_limit(true, true, &llrm_x86_m16::Dos::default(), &llrm_mir::tti::Basic), 4);
    assert_eq!(super::header_limit(true, false, &llrm_x86_m16::Dos::default(), &llrm_mir::tti::Basic), 16);
    assert_eq!(super::header_limit(false, true, &llrm_x86_m16::Dos::default(), &llrm_mir::tti::Basic), 0);
}

/// The -Os limit of 4 followed `segment_registers() > 0`, so any target with
/// segment registers copied at most four header instructions; the sweep
/// behind 4 was m16's, which now states it itself.
#[test]
fn test_the_size_header_limit_comes_from_the_target_not_its_segments() {
    let segmented = crate::testing::Tuned { segments: 2, ..Default::default() };
    assert_eq!(super::header_limit(true, true, &segmented, &llrm_mir::tti::Basic), 16);
}

/// `rotated` as a size build on arm64, where a header is copied only when it
/// is no bigger than the branches rotation removes.
fn rotated_priced(module: &mut Module) -> bool {
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(module).expect("closes");
    let tti = llrm_arm64::tti::AArch64::of_cpu("generic").expect("a cpu");
    let options = Options {
        max_header_size: 16,
        priced_at_size: true,
        guard_must_fold: false,
        rotation_only: false,
        is_util_mode: false,
        prepare_for_lto: false,
        check_exit_count: false,
    };
    rotate_function(module, "f", &tti, options).expect("rotates")
}

/// -Os arm64: a header of only the exit test is rotated, a header that also
/// computes something is not (copying it grew jpeg_fixed, jpeg_single, nbody
/// and recchop by 404 B in all).
#[test]
fn test_a_size_build_copies_a_header_only_when_it_is_no_bigger_than_the_removed_branches() {
    let mut test_only = parsed(COUNTED);
    assert!(rotated_priced(&mut test_only), "{}", printed(&test_only));
    let mut computing = parsed(
        &COUNTED.replace("  %go = icmp slt i16 %i, %n\n", "  %t = add i16 %i, %acc\n  %go = icmp slt i16 %t, %n\n"),
    );
    assert!(!rotated_priced(&mut computing), "{}", printed(&computing));
}

/// Header exit `i < n` counts the trips; the latch exits on a load, which
/// does not. No header phi dies by rotating, so only the late slot's
/// `CheckExitCount` finds it profitable (PassBuilderPipelines.cpp#L1621):
/// the rotated loop's trip count is computable for unrolling.
const LATCH_UNCOUNTED: &str = "define i16 @f(i16 %n, ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %v = load i16, ptr %p
  %next = add i16 %i, 1
  %stop = icmp eq i16 %v, 0
  br i1 %stop, label %b3, label %b1

b3:
  %r = phi i16 [ %i, %b1 ], [ %next, %b2 ]
  ret i16 %r
}
";

fn rotated_checking(check_exit_count: bool) -> bool {
    let mut module = parsed(LATCH_UNCOUNTED);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options = Options {
        max_header_size: 16,
        priced_at_size: false,
        guard_must_fold: false,
        rotation_only: false,
        is_util_mode: false,
        prepare_for_lto: false,
        check_exit_count,
    };
    rotate_function(&mut module, "f", &tti, options).expect("rotates")
}

/// Rotating a loop whose latch exit count it makes computable was never done:
/// the late `loop-rotate<check-exit-count>` slot was absent (x_prefix-shaped
/// loops stayed unrotated for unroll and LSR).
#[test]
fn test_check_exit_count_rotates_a_loop_whose_latch_exit_count_it_makes_computable() {
    assert!(!rotated_checking(false), "premise: no header phi dies, the early slot leaves it");
    assert!(rotated_checking(true));
}

/// The late slot made a `ScevSession` (dominators, loops, assumptions) for each
/// loop it asked about: memory_expansion -O2 paid 413 M instructions for 217 of
/// them, quadratic in the function. A function whose loops it leaves makes one.
#[test]
fn test_check_exit_count_asks_with_one_session_for_all_the_loops_it_leaves() {
    let mut text = String::from("define i16 @f(ptr %p) {\nb0:\n  br label %h0\n");
    for k in 0..5 {
        let from = if k == 0 { "b0".to_string() } else { format!("e{}", k - 1) };
        let next = if k == 4 { "end".to_string() } else { format!("h{}", k + 1) };
        text += &format!(
            "h{k}:\n  %i{k} = phi i16 [ 0, %{from} ], [ %n{k}, %l{k} ]\n  %v{k} = load i16, ptr %p\n  %g{k} = icmp eq i16 %v{k}, 0\n  br i1 %g{k}, label %e{k}, label %l{k}\nl{k}:\n  %w{k} = load i16, ptr %p\n  %n{k} = add i16 %i{k}, 1\n  %s{k} = icmp eq i16 %w{k}, 1\n  br i1 %s{k}, label %e{k}, label %h{k}\ne{k}:\n  %r{k} = phi i16 [ %i{k}, %h{k} ], [ %n{k}, %l{k} ]\n  br label %{next}\n"
        );
    }
    text += "end:\n  ret i16 0\n}\n";
    let mut module = parsed(&text);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options = Options {
        max_header_size: 16,
        priced_at_size: false,
        guard_must_fold: false,
        rotation_only: false,
        is_util_mode: false,
        prepare_for_lto: false,
        check_exit_count: true,
    };
    let before = llrm_analysis::scev::analysis::sessions_made();
    rotate_function(&mut module, "f", &tti, options).expect("rotates");
    assert_eq!(llrm_analysis::scev::analysis::sessions_made() - before, 1);
}

/// A header declaring a scope for its store and `!noalias` it on its load kept
/// one scope in the guard's copy and in the loop: the copy's store and the
/// loop's load were called apart by `ScopedNoAliasAA`, though they are two runs
/// of the code and `%p` and `%q` may be one address
/// (`cloneAndAdaptNoAliasScopes` in LoopRotationUtils).
#[test]
fn test_the_header_copy_and_the_loop_do_not_share_a_declared_scope() {
    use llrm_analysis::aa::{AliasResult, BatchAAResults, MemoryLocation};
    use llrm_analysis::aggregate::AggregateAA;
    use llrm_analysis::memory::Unit;
    use llrm_analysis::scopednoaliasaa::ScopedNoAliasAA;

    let mut module = parsed(
        "declare void @llvm.experimental.noalias.scope.decl()

define void @f(ptr %p, ptr %q, i32 %n) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b2 ]
  call void @llvm.experimental.noalias.scope.decl(), !noalias.scope.decl !1
  store i32 1, ptr %p, !alias.scope !1
  %v = load i32, ptr %q, !noalias !1
  %go = icmp slt i32 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  store i32 %v, ptr %q
  %next = add i32 %i, 1
  br label %b1

b3:
  ret void
}

!0 = distinct !{!0, !\"domain\"}
!2 = distinct !{!2, !0, !\"scope\"}
!1 = !{!2}
",
    );
    assert!(rotated(&mut module), "{}", printed(&module));
    printed(&module); // it verifies
    let layout = llrm_mir::datalayout::DataLayout::default();
    let function = module.functions().find(|(_, global, _)| global.name.as_deref() == Some("f")).expect("@f").2;
    let unit = Unit::of(&module, &layout, function);
    let is_constant_store = |inst| {
        let op = function.instruction(inst);
        matches!(op.opcode, llrm_mir::opcode::Opcode::Store { .. })
            && matches!(op.operands[0], llrm_mir::module::Operand::Constant(_))
    };
    let stores: Vec<MemoryLocation> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| is_constant_store(inst))
        .filter_map(|inst| MemoryLocation::get(&unit, inst))
        .collect();
    let loads: Vec<MemoryLocation> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            llrm_mir::opcode::Opcode::Load { .. }
        ))
        .filter_map(|inst| MemoryLocation::get(&unit, inst))
        .collect();
    // The guard's copy first, then the loop's.
    assert_eq!((stores.len(), loads.len()), (2, 2), "{}", printed(&module));
    let mut chain = AggregateAA::new();
    chain.add_aa_result(ScopedNoAliasAA::new(&unit));
    let mut aa = BatchAAResults::new(&mut chain);
    assert_eq!(aa.alias(&stores[0], &loads[0]), AliasResult::NoAlias, "premise: the copy's own pair");
    assert_eq!(aa.alias(&stores[1], &loads[1]), AliasResult::NoAlias, "premise: the loop's own pair");
    assert_eq!(aa.alias(&stores[0], &loads[1]), AliasResult::MayAlias, "{}", printed(&module));
}

/// `rotated` as a size build where an unfolded guard is refused (RS1).
fn rotated_guard_must_fold(text: &str) -> bool {
    let mut module = parsed(text);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let options = Options {
        max_header_size: 16,
        priced_at_size: false,
        guard_must_fold: true,
        rotation_only: false,
        is_util_mode: false,
        prepare_for_lto: false,
        check_exit_count: false,
    };
    rotate_function(&mut module, "f", &tti, options).expect("rotates")
}

/// A loop that stops on a loaded value: its two-instruction header (compare
/// and branch) copied to the preheader is an unfolded guard, and no trip
/// count follows. Rotating it grew jpeg_fixed Os by 118 B over 20-odd loops.
const UNCOUNTED: &str = "define i16 @f(ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %v = load i16, ptr %p
  %go = icmp ne i16 %v, 0
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  store i16 %next, ptr %p
  br label %b1

b3:
  ret i16 %i
}
";

/// -Os (RS1): a loop whose exit test is neither folded by the preheader's
/// values nor counted is left alone; the same loop is rotated without the
/// rule.
#[test]
fn test_a_size_build_leaves_an_uncounted_loop_whose_guard_does_not_fold() {
    assert!(!rotated_guard_must_fold(UNCOUNTED));
    assert!(rotated(&mut parsed(UNCOUNTED)));
}

/// -Os (RS1): `i < n` counts its trips, so it is rotated (queens' column
/// loop is +18 B unrotated).
#[test]
fn test_a_size_build_rotates_a_counted_loop() {
    assert!(rotated_guard_must_fold(COUNTED));
}

/// -Os (RS1): a guard the preheader's values decide (`0 < 8`) is removed by
/// the rotation, so it is rotated.
#[test]
fn test_a_size_build_rotates_a_loop_whose_guard_folds() {
    assert!(rotated_guard_must_fold(&COUNTED.replace("%n)", "%y)").replace("i16 %i, %n", "i16 %i, 8")));
}

/// m32 -Os copied a header of up to 16 instructions: x_hash's probe loop test
/// was repeated before the loop (+29 B on 280). The target's tuning states 4.
#[test]
fn test_the_size_header_limit_of_m32_is_four() {
    let m32 = crate::indvars::tests::m32();
    let tti = m32.tti().unwrap();
    assert_eq!(super::header_limit(true, true, &*m32, &*tti), 4);
    assert_eq!(super::header_limit(true, false, &*m32, &*tti), 16);
}

/// Asks the manager what it holds, and says so in `held`.
struct Holds(std::rc::Rc<std::cell::Cell<(bool, bool)>>);

impl llrm_mir::passes::FunctionPass for Holds {
    fn name(&self) -> &'static str {
        "holds"
    }

    fn run(
        &mut self,
        _: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        self.0.set((
            analyses.cached::<llrm_mir::passes::Dominators>().is_some(),
            analyses.cached::<llrm_mir::passes::Loops>().is_some(),
        ));
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// Compile time: a rotation returned `none()`, so the dominator tree and the
/// loops were made again for the next pass (64 of 65 rotations on memexp -O2
/// were followed by a rebuild). The rotation edits them and keeps them.
#[test]
fn test_a_rotation_keeps_the_dominator_tree_and_the_loops() {
    let mut module = parsed(COUNTED);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let held = std::rc::Rc::new(std::cell::Cell::new((false, false)));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(super::LoopRotate { duplicate_headers: true, size: false, check_exit_count: false });
    manager.add(Holds(held.clone()));
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    assert!(printed(&module).contains(".lr.ph") || printed(&module) != printed(&parsed(COUNTED)), "the loop was not rotated");
    assert_eq!(held.get(), (true, true), "dominators, loops held after the rotation");
}

/// Makes the memory graph, so the rotation after it has one to update.
struct Warm;

impl llrm_mir::passes::FunctionPass for Warm {
    fn name(&self) -> &'static str {
        "warm"
    }

    fn run(
        &mut self,
        unit: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        let _ = analyses.get::<llrm_analysis::manager::MemorySsa>(&*unit.context, unit.layout, &*unit.function);
        llrm_mir::passes::PreservedAnalyses::all()
    }
}

/// A loop that loads and stores in its header, behind a store: the clones
/// in the preheader read what that store wrote.
const MEMORY: &str = "@ga = global i16 0
@gb = global i16 0

define i16 @f(i16 %x, i16 %n) {
b0:
  store i16 %x, ptr @ga
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %v = load i16, ptr @ga
  store i16 %v, ptr @gb
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %w = load i16, ptr @gb
  store i16 %w, ptr @ga
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = load i16, ptr @gb
  ret i16 %r
}
";

/// Compile time: LICM built the memory graph again after every rotation (64
/// of 65 on memexp -O2). The rotation updates it as the pin's
/// `MemorySSAUpdater` calls do, and it says what a fresh graph says (checked
/// in debug builds by `rotate_loops`).
#[test]
fn test_a_rotation_keeps_the_memory_graph() {
    let mut module = parsed(MEMORY);
    let function = module.function_mut("f").expect("@f").1;
    crate::loopsimplify::simplified(function);
    crate::testing::closed(&mut module).expect("closes");
    let before = printed(&module);
    let held = std::rc::Rc::new(std::cell::Cell::new((false, false)));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(Warm);
    manager.add(super::LoopRotate { duplicate_headers: true, size: false, check_exit_count: false });
    manager.add(HoldsMemory(held.clone()));
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap();
    assert_ne!(printed(&module), before, "the loop was not rotated");
    assert!(held.get().0, "the memory graph is held after the rotation");
}

struct HoldsMemory(std::rc::Rc<std::cell::Cell<(bool, bool)>>);

impl llrm_mir::passes::FunctionPass for HoldsMemory {
    fn name(&self) -> &'static str {
        "holds-memory"
    }

    fn run(
        &mut self,
        _: &mut llrm_mir::passes::Unit,
        analyses: &mut llrm_mir::passes::Analyses,
    ) -> llrm_mir::passes::PreservedAnalyses {
        self.0.set((analyses.cached::<llrm_analysis::manager::MemorySsa>().is_some(), false));
        llrm_mir::passes::PreservedAnalyses::all()
    }
}
