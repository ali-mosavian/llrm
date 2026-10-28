//! `SCEVExpander` over expressions built by hand, as `createSCEV` (S3) does
//! not yet make them from the IR: what each node expands to, printed.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::Operand;
use llrm_mir::opcode::Opcode;
use llrm_mir::passes::Declared;
use llrm_mir::valuetracking::AssumptionCache;

use super::ap::Ap;
use super::expander::{Env, Expander};
use super::expr::{Scev, Ty, Use};
use super::{Cx, Flags, ScalarEvolution};

const BODY: &str = "define i32 @f(i32 %a, i32 %b, i64 %w) {
entry:
  ret i32 0
}
";

/// `build`'s SCEV expanded before the `ret` of `BODY`, and the body printed.
fn expanded(build: impl FnOnce(&mut ScalarEvolution, &Cx, &[Operand]) -> Scev) -> String {
    expanded_in(BODY, build)
}

/// [`expanded`] over `body`; `build` gets the three parameters, then the
/// result of each instruction that has one.
fn expanded_in(
    body: &str,
    build: impl FnOnce(&mut ScalarEvolution, &Cx, &[Operand]) -> Scev,
) -> String {
    let module = llrm_mir::parse::module(body).expect("a module");
    let (_, _, original) = module.functions().next().expect("a function");
    let mut function = original.clone();
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let mut se = ScalarEvolution::new();
    let mut arguments: Vec<Operand> = (0..3).map(|at| Operand::Value(function.parameters()[at])).collect();
    let entry = function.entry().expect("an entry");
    arguments.extend(function.iter_block(entry).filter_map(|one| function.instruction(one).result.map(Operand::Value)));
    let s = build(&mut se, &env.cx(&context, &function), &arguments);
    let ret = function.terminator(function.entry().expect("an entry")).expect("a ret");
    let mut expander = Expander::new("t");
    expander.set_insert_point(ret);
    let mut declared = Declared::of(&module);
    let result = expander.expand_code_for(&env, &mut context, &mut declared, &mut function, &mut se, s, None);
    assert!(matches!(function.instruction(ret).opcode, Opcode::Ret));
    let _ = result;
    llrm_mir::print::body(&context, &function)
}

fn unknown(
    se: &mut ScalarEvolution,
    cx: &Cx,
    v: Operand,
) -> Use {
    Use::of(se.get_unknown(cx, v))
}

fn constant(
    se: &mut ScalarEvolution,
    width: u32,
    value: u128,
) -> Use {
    Use::of(se.get_constant(Ap::new(width, value)))
}

/// The body's instructions, one per line, without the `ret`.
fn lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|l| l.starts_with('%') && l.contains(" = ")).collect()
}

#[test]
fn test_an_add_of_a_scaled_unknown_is_a_shift_and_an_add() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        let four = constant(se, 32, 4);
        let scaled = se.get_mul_expr(cx, vec![four, b], Flags::default(), 0);
        se.get_add_expr(cx, vec![a, scaled], Flags::default(), 0).scev
    });
    assert_eq!(lines(&text), ["%0 = shl i32 %b, 2", "%1 = add i32 %a, %0"]);
}

/// `a + (-1 * b)` is a subtract, not a negate and an add.
#[test]
fn test_a_negated_unknown_is_subtracted() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        let minus_two = constant(se, 32, u32::MAX as u128 - 1);
        let scaled = se.get_mul_expr(cx, vec![minus_two, b], Flags::default(), 0);
        se.get_add_expr(cx, vec![a, scaled], Flags::default(), 0).scev
    });
    assert_eq!(lines(&text), ["%0 = shl i32 %b, 1", "%1 = sub i32 %a, %0"]);
}

#[test]
fn test_a_udiv_by_a_power_of_two_is_a_shift_right() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let eight = constant(se, 32, 8);
        se.get_udiv_expr(cx, a, eight)
    });
    assert_eq!(lines(&text), ["%0 = lshr i32 %a, 3"]);
}

#[test]
fn test_a_max_of_unknowns_is_the_intrinsic() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        se.get_umax_expr(cx, vec![a, b])
    });
    assert_eq!(lines(&text), ["%0 = call i32 @1(i32 %b, i32 %a)"]);
}

#[test]
fn test_a_sequential_min_freezes_what_it_may_skip() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        se.get_sequential_min_max_expr(cx, super::expr::Type::SequentialUMin, vec![a, b])
    });
    // The last operand is frozen; the first (`i == 0`) is not.
    assert_eq!(lines(&text), ["%0 = freeze i32 %b", "%1 = call i32 @1(i32 %0, i32 %a)"]);
}

/// A truncate of a zero-extend reads back the narrow value.
#[test]
fn test_casts_expand_to_casts() {
    let text = expanded(|se, cx, args| {
        let w = unknown(se, cx, args[2]);
        se.get_truncate_expr(cx, w, Ty::Int(32), 0)
    });
    assert_eq!(lines(&text), ["%0 = trunc i64 %w to i32"]);
}

/// Two expansions of one binop at one point are one instruction:
/// `InsertBinop`'s scan of the six before the insert point.
#[test]
fn test_an_equal_binop_just_before_the_insert_point_is_reused() {
    let text = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        let sum = se.get_add_expr(cx, vec![a, b], Flags::default(), 0);
        se.get_add_expr(cx, vec![sum, sum], Flags::default(), 0).scev
    });
    assert_eq!(lines(&text).len(), 2, "{text}");
}

/// `InsertBinop` builds through `InstSimplifyFolder`: `(%a + -1) + 1` is `%a`.
/// LSR's compare on a post-increment value plus the step it was normalized
/// by came out as `add %a.next, 1` beside the phi it equals, and x_insertion
/// compiled worse.
#[test]
fn test_a_sum_that_cancels_an_earlier_add_is_the_operand_it_started_from() {
    let body = "define i32 @f(i32 %a, i32 %b, i64 %w) {
entry:
  %x = add nsw i32 %a, -1
  ret i32 0
}
";
    let text = expanded_in(body, |se, cx, args| {
        let x = unknown(se, cx, args[3]);
        let one = constant(se, 32, 1);
        se.get_add_expr(cx, vec![x, one], Flags::default(), 0).scev
    });
    assert_eq!(lines(&text).len(), 1, "{text}");
}

/// `isSafeToExpand` was `todo!`: a udiv by an unknown divisor may divide by
/// zero, so it is not safe; by a nonzero constant it is.
#[test]
fn test_a_udiv_is_safe_to_expand_only_by_a_divisor_known_nonzero() {
    let mut safe = Vec::new();
    for divisor_is_constant in [false, true] {
        expanded(|se, cx, args| {
            let a = unknown(se, cx, args[0]);
            let d = if divisor_is_constant { constant(se, 32, 3) } else { unknown(se, cx, args[1]) };
            let s = se.get_udiv_expr(cx, a, d);
            safe.push(Expander::new("t").is_safe_to_expand(cx, se, s));
            s
        });
    }
    assert_eq!(safe, [false, true]);
}

const LOOP: &str = "define i32 @f(i32 %a, i32 %b, i32 %n) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret i32 0
}
";

/// `build`'s SCEV (given the loop header and the function's arguments)
/// expanded before `%c` of `LOOP`, in canonical mode or not, and the body
/// printed.
fn expanded_in_loop(
    canonical: bool,
    build: impl FnOnce(&mut ScalarEvolution, &Cx, llrm_mir::module::BlockId, &[Operand]) -> Scev,
) -> String {
    expanded_in_loop_with(canonical, false, build)
}

/// [`expanded_in_loop`], in post-increment mode for the loop if `post_inc`.
fn expanded_in_loop_with(
    canonical: bool,
    post_inc: bool,
    build: impl FnOnce(&mut ScalarEvolution, &Cx, llrm_mir::module::BlockId, &[Operand]) -> Scev,
) -> String {
    let module = llrm_mir::parse::module(LOOP).expect("a module");
    let (_, _, original) = module.functions().next().expect("a function");
    let mut function = original.clone();
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let header = *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let mut se = ScalarEvolution::new();
    let arguments: Vec<Operand> = (0..3).map(|at| Operand::Value(function.parameters()[at])).collect();
    let s = build(&mut se, &env.cx(&context, &function), header, &arguments);
    let at = function
        .iter_block(header)
        .find(|&one| matches!(function.instruction(one).opcode, Opcode::ICmp(_)))
        .expect("the compare");
    let mut expander = Expander::new("t");
    expander.set_canonical_mode(canonical);
    if post_inc {
        expander.set_post_inc(header);
    }
    expander.set_insert_point(at);
    let mut declared = Declared::of(&module);
    expander.expand_code_for(&env, &mut context, &mut declared, &mut function, &mut se, s, None);
    llrm_mir::print::body(&context, &function)
}

fn add_rec(
    se: &mut ScalarEvolution,
    cx: &Cx,
    header: llrm_mir::module::BlockId,
    ops: Vec<Scev>,
) -> Scev {
    se.get_add_rec_expr(cx, ops.into_iter().map(Use::of).collect(), header, Flags::default()).scev
}

/// `{0,+,1}` is the loop's own canonical phi: nothing is added.
#[test]
fn test_the_unit_recurrence_is_the_existing_canonical_iv() {
    let text = expanded_in_loop(true, |se, cx, h, _| {
        let (zero, one) = (constant(se, 32, 0).scev, constant(se, 32, 1).scev);
        add_rec(se, cx, h, vec![zero, one])
    });
    assert_eq!(
        lines(&text),
        ["%i = phi i32 [ 0, %entry ], [ %i2, %h ]", "%i2 = add i32 %i, 1", "%c = icmp slt i32 %i2, %n"]
    );
}

/// `{%a,+,4}` in canonical mode is `%a + 4 * %i`: no new phi.
#[test]
fn test_an_affine_recurrence_is_the_start_plus_the_scaled_canonical_iv() {
    let text = expanded_in_loop(true, |se, cx, h, args| {
        let a = unknown(se, cx, args[0]).scev;
        let four = constant(se, 32, 4).scev;
        add_rec(se, cx, h, vec![a, four])
    });
    assert_eq!(
        lines(&text),
        [
            "%i = phi i32 [ 0, %entry ], [ %i2, %h ]",
            "%0 = shl i32 %i, 2",
            "%1 = add i32 %a, %0",
            "%i2 = add i32 %i, 1",
            "%c = icmp slt i32 %i2, %n"
        ]
    );
}

/// Not canonical, `{%a,+,%b}` gets its own phi, started in the preheader.
#[test]
fn test_a_recurrence_written_literally_has_its_own_phi() {
    let text = expanded_in_loop(false, |se, cx, h, args| {
        let a = unknown(se, cx, args[0]).scev;
        let b = unknown(se, cx, args[1]).scev;
        add_rec(se, cx, h, vec![a, b])
    });
    assert_eq!(
        lines(&text),
        [
            "%0 = phi i32 [ %1, %h ], [ %a, %entry ]",
            "%i = phi i32 [ 0, %entry ], [ %i2, %h ]",
            "%i2 = add i32 %i, 1",
            "%c = icmp slt i32 %i2, %n",
            "%1 = add i32 %0, %b"
        ]
    );
}

/// `{1,+,1}` in post-inc mode, expanded after `%i2 = %i + 1`, is that
/// increment: the phi is found, and so is its increment. A duplicate
/// `add %i, 1` was made when the insert point was hoisted above `%i2` to the
/// header's first instruction (`expand` omitted `!PostIncLoops.count(L)`),
/// where `%i2` does not dominate it.
#[test]
fn test_a_post_increment_recurrence_reuses_the_latch_increment() {
    let text = expanded_in_loop_with(false, true, |se, cx, h, _| {
        let one = constant(se, 32, 1).scev;
        add_rec(se, cx, h, vec![one, one])
    });
    assert_eq!(
        lines(&text),
        ["%i = phi i32 [ 0, %entry ], [ %i2, %h ]", "%i2 = add i32 %i, 1", "%c = icmp slt i32 %i2, %n"]
    );
}

/// In post-inc mode `{%a,+,%b}` is the value after the increment: the phi
/// starts at the normalized `%a - %b` and the increment does not dominate the
/// compare, so one is added there: before the compare, not hoisted to the
/// header (`expand` does not hoist for a loop it expands post-incremented).
#[test]
fn test_a_post_increment_recurrence_starts_one_step_back() {
    let text = expanded_in_loop_with(false, true, |se, cx, h, args| {
        let a = unknown(se, cx, args[0]).scev;
        let b = unknown(se, cx, args[1]).scev;
        add_rec(se, cx, h, vec![a, b])
    });
    assert_eq!(
        lines(&text),
        [
            "%0 = sub i32 %a, %b",
            "%1 = phi i32 [ %3, %h ], [ %0, %entry ]",
            "%i = phi i32 [ 0, %entry ], [ %i2, %h ]",
            "%i2 = add i32 %i, 1",
            "%2 = add i32 %1, %b",
            "%c = icmp slt i32 %i2, %n",
            "%3 = add i32 %1, %b"
        ]
    );
}

/// `normalizeForPostIncUse` then `denormalizeForPostIncUse` is the identity.
#[test]
fn test_normalizing_for_post_inc_is_inverted_by_denormalizing() {
    expanded_in_loop(false, |se, cx, h, args| {
        let a = unknown(se, cx, args[0]).scev;
        let b = unknown(se, cx, args[1]).scev;
        let chain = add_rec(se, cx, h, vec![a, b, a]);
        let normalized = se.normalize_for_post_inc_use(cx, chain, &[h], true).expect("invertible");
        assert_ne!(normalized, chain);
        assert_eq!(se.denormalize_for_post_inc_use(cx, normalized, &[h]), chain);
        chain
    });
}

/// `hoistIVInc` moves the increment above a use that reads the next value.
#[test]
fn test_an_iv_increment_is_hoisted_above_the_use_that_needs_it() {
    let text = "define i32 @f(i32 %a, i32 %n) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %u = add i32 %i2, %a
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret i32 0
}
";
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, original) = module.functions().next().expect("a function");
    let mut function = original.clone();
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let header = *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let named = |function: &llrm_mir::module::Function, name: &str| {
        function
            .iter_block(header)
            .find(|&one| {
                function.instruction(one).result.is_some_and(|v| function.value(v).name.as_deref() == Some(name))
            })
            .expect("named")
    };
    let (inc, user) = (named(&function, "i2"), named(&function, "u"));
    let mut se = ScalarEvolution::new();
    let mut expander = Expander::new("t");
    assert!(expander.hoist_iv_inc(&env, &mut context, &mut function, &mut se, inc, user, false));
    let after = llrm_mir::print::body(&context, &function);
    let at = |needle: &str| after.find(needle).expect(needle);
    assert!(at("%i2 = add i32 %i, 1") < at("%u = add i32 %i2, %a"), "{after}");
}

/// The increments of a wide and a narrow phi agree on the operation.
#[test]
fn test_two_increments_of_one_operation_can_share_flags() {
    let text = "define void @f(i32 %n) {
entry:
  br label %h

h:
  %i = phi i32 [ 0, %entry ], [ %i2, %h ]
  %j = phi i32 [ 0, %entry ], [ %j2, %h ]
  %i2 = add nuw i32 %i, 1
  %j2 = sub i32 %j, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %exit

exit:
  ret void
}
";
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().next().expect("a function");
    let find = |name: &str| {
        function
            .layout()
            .iter()
            .flat_map(|&b| function.iter_block(b).collect::<Vec<_>>())
            .find(|&one| {
                function.instruction(one).result.is_some_and(|v| function.value(v).name.as_deref() == Some(name))
            })
            .expect("named")
    };
    let (i, j, i2, j2) = (find("i"), find("j"), find("i2"), find("j2"));
    assert!(Expander::can_reuse_flags_from_original_iv_inc(function, i, i, i2, i2));
    assert!(!Expander::can_reuse_flags_from_original_iv_inc(function, i, j, i2, j2));
}

/// `getAllInsertedInstructions` names what an expansion made, in order, and not
/// the phi it reused.
#[test]
fn test_the_inserted_instructions_are_what_the_expansion_made() {
    let module = llrm_mir::parse::module(BODY).expect("a module");
    let (_, _, original) = module.functions().next().expect("a function");
    let mut function = original.clone();
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let mut se = ScalarEvolution::new();
    let args: Vec<Operand> = (0..3).map(|at| Operand::Value(function.parameters()[at])).collect();
    let s = {
        let cx = env.cx(&context, &function);
        let a = unknown(&mut se, &cx, args[0]);
        let b = unknown(&mut se, &cx, args[1]);
        let four = constant(&mut se, 32, 4);
        let scaled = se.get_mul_expr(&cx, vec![four, b], Flags::default(), 0);
        se.get_add_expr(&cx, vec![a, scaled], Flags::default(), 0).scev
    };
    let ret = function.terminator(function.entry().expect("an entry")).expect("a ret");
    let mut expander = Expander::new("t");
    expander.set_insert_point(ret);
    let mut declared = Declared::of(&module);
    expander.expand_code_for(&env, &mut context, &mut declared, &mut function, &mut se, s, None);
    let made = expander.inserted_instructions();
    assert_eq!(made.len(), 2);
    assert!(made.iter().all(|one| expander.is_inserted_instruction(one)));
}

/// `%i2` of `LOOP` expanded before the `ret` after the loop, and the body
/// printed.
fn loop_value_expanded_after_the_loop() -> String {
    let module = llrm_mir::parse::module(LOOP).expect("a module");
    let (_, _, original) = module.functions().next().expect("a function");
    let mut function = original.clone();
    let mut context = module.context.clone();
    let layout = DataLayout::default();
    let tree = DomTree::of(&function);
    let loops = LoopInfo::analyze(&function, || &tree);
    let assumptions = AssumptionCache::new(&module, &function);
    let env = Env {
        globals: &module.globals,
        metadata: &module.metadata,
        layout: &layout,
        loops: &loops,
        tree: &tree,
        assumptions: &assumptions,
    };
    let mut se = ScalarEvolution::new();
    let header = *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("h")).expect("%h");
    let exit =
        *function.layout().iter().find(|&&one| function.block(one).name.as_deref() == Some("exit")).expect("%exit");
    let i2 = function.iter_block(header).nth(1).expect("%i2");
    let s = unknown(
        &mut se,
        &env.cx(&context, &function),
        Operand::Value(function.instruction(i2).result.expect("a value")),
    );
    let ret = function.terminator(exit).expect("a ret");
    let mut expander = Expander::new("t");
    expander.set_insert_point(ret);
    let mut declared = Declared::of(&module);
    expander.expand_code_for(&env, &mut context, &mut declared, &mut function, &mut se, s.scev, None);
    llrm_mir::print::body(&context, &function)
}

/// A loop's value expanded after the loop panicked on `todo!("S6: ...")` in
/// `fixupLCSSAFormFor`; it is now read through an exit phi (planets, m32).
#[test]
fn test_a_loop_value_read_after_the_loop_is_read_through_an_exit_phi() {
    let text = loop_value_expanded_after_the_loop();
    assert!(lines(&text).contains(&"%i2.lcssa = phi i32 [ %i2, %h ]"), "{text}");
    assert!(!text.contains("tmp.lcssa"), "{text}");
}

/// An expression that exists is found with the operands borrowed: asking for
/// `a + b` again copies no operand list (it built a `Vec` per ask, the
/// largest single frame of the key hashing: IndVars on d_faces 22.25 M -> 20.89
/// M Ir).
#[test]
fn test_an_expression_that_exists_is_found_without_copying_its_operands() {
    let _ = expanded(|se, cx, args| {
        let a = unknown(se, cx, args[0]);
        let b = unknown(se, cx, args[1]);
        let first = se.get_add_expr(cx, vec![a, b], Flags::default(), 0).scev;
        let owned = llrm_mir::counts::get(llrm_mir::counts::Event::ScevKeysOwned);
        let again = se.get_add_expr(cx, vec![a, b], Flags::default(), 0).scev;
        assert_eq!(first, again);
        assert_eq!(llrm_mir::counts::get(llrm_mir::counts::Event::ScevKeysOwned), owned);
        first
    });
}

/// `scev_nodes` counts expressions made from values. A constant asked for again
/// was counted each time: the unbounded climb of
/// `isBasicBlockEntryGuardedByCond` over 128 guarded loops read 46077 of 50000
/// "nodes" as constants and failed the `functions` event slope (17272 ->
/// 51992).
#[test]
fn test_asking_for_a_constant_again_makes_no_node() {
    let _ = expanded(|se, cx, _| {
        let entry = cx.function.entry().expect("an entry");
        let ret = cx.function.terminator(entry).expect("a ret");
        let zero = cx.function.instruction(ret).operands[0];
        assert!(matches!(zero, Operand::Constant(_)), "premise: the ret returns a constant");
        let before = llrm_mir::counts::get(llrm_mir::counts::Event::ScevNodes);
        let first = se.get_scev(cx, zero);
        for _ in 0..10 {
            assert_eq!(se.get_scev(cx, zero), first);
        }
        assert_eq!(llrm_mir::counts::get(llrm_mir::counts::Event::ScevNodes), before, "a constant is not a node");
        first
    });
}
