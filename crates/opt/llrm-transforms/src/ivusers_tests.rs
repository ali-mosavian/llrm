use std::cell::Cell;

use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::domtree::DomTree;
use llrm_mir::loopinfo::LoopInfo;
use llrm_mir::module::{BlockId, InstId};
use llrm_mir::opcode::Opcode;
use llrm_mir::valuetracking::AssumptionCache;

use crate::ivusers::{Env, IvStrideUse, IvUsers};

const ARRAY: &str = "define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %a
  %next = add i32 %i, 1
  %c = icmp slt i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  %r = add i32 %next, %v
  ret i32 %r
}
";

thread_local! {
    /// How many sets grew while the last `with_users` made its users.
    static GROWN: Cell<usize> = const { Cell::new(0) };
}

/// `text`'s `@f` with its IV users, the number of instructions asked about,
/// and the loop's header.
fn with_users(
    text: &str,
    body: impl FnOnce(&Cx, &mut ScalarEvolution, &IvUsers, BlockId, usize),
) {
    with_edited_users(text, false, |_| {}, body);
}

/// As `with_users`, with `edit` run on the parsed module first and the users
/// met in use-list order when `use_list_order`.
fn with_edited_users(
    text: &str,
    use_list_order: bool,
    edit: impl FnOnce(&mut llrm_mir::module::Module),
    body: impl FnOnce(&Cx, &mut ScalarEvolution, &IvUsers, BlockId, usize),
) {
    with_flagged_users(text, use_list_order, false, edit, body);
}

/// As `with_edited_users`, with `phi_operand_uses` (DEVIATE LSR-PHI) as asked.
fn with_flagged_users(
    text: &str,
    use_list_order: bool,
    phi_operand_uses: bool,
    edit: impl FnOnce(&mut llrm_mir::module::Module),
    body: impl FnOnce(&Cx, &mut ScalarEvolution, &IvUsers, BlockId, usize),
) {
    let mut module = llrm_mir::parse::module(text).expect("a module");
    edit(&mut module);
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
    let asked = Cell::new(0);
    let safe = |_: InstId| {
        asked.set(asked.get() + 1);
        true
    };
    let env = Env { safe_to_speculate: &safe, ephemeral: &|_| false, native_widths: &[], use_list_order, phi_operand_uses };
    let grown = llrm_mir::dense::idset_resizes::<InstId>();
    let users = IvUsers::of(&mut se, &cx, header, &env);
    GROWN.with(|n| n.set(llrm_mir::dense::idset_resizes::<InstId>() - grown));
    body(&cx, &mut se, &users, header, asked.get());
}

/// The user's opcode (its first word) and the operand's name.
fn named(
    cx: &Cx,
    u: &IvStrideUse,
) -> (String, String) {
    let operand = match u.operand {
        llrm_mir::module::Operand::Value(v) => cx.function.value(v).name.clone().unwrap_or_default(),
        _ => String::new(),
    };
    let opcode = format!("{:?}", cx.function.instruction(u.user).opcode);
    (opcode.split(|c: char| !c.is_alphanumeric()).next().unwrap_or_default().to_owned(), operand)
}

/// The load and the compare stay where they are and the gep is not a user: it
/// is reducible, so its load is. `%r = %next + %v` is reducible too (an add
/// with one interesting operand), so the `ret` is the user. The phi that takes
/// `%next` is already walked and is not recorded again.
#[test]
fn the_users_of_an_induction_variable_are_those_not_reducible_further() {
    with_users(ARRAY, |cx, _, users, _, _| {
        let found: Vec<_> = users.uses.iter().map(|u| named(cx, u)).collect();
        let has = |opcode: &str, operand: &str| {
            found.iter().any(|(o, v)| o == opcode && v.trim_start_matches('%') == operand)
        };
        assert!(has("Load", "a"), "{found:?}");
        assert!(has("Ret", "r"), "{found:?}");
        assert!(!found.iter().any(|(opcode, _)| opcode == "Phi"), "{found:?}");
        assert!(has("ICmp", "next"), "{found:?}");
        assert!(!found.iter().any(|(opcode, _)| opcode == "GetElementPtr"), "the gep is reduced further: {found:?}");
    });
}

/// `ret %next + %v` after the loop takes the loop's post-incremented value: the
/// use is normalized for the loop (its expression is not the value's), and
/// the users in the loop are not.
#[test]
fn a_user_after_the_loop_takes_the_post_incremented_value() {
    with_users(ARRAY, |cx, se, users, header, _| {
        let post: Vec<_> = users.uses.iter().filter(|u| !u.post_inc_loops.is_empty()).collect();
        assert_eq!(post.len(), 1, "{:?}", users.uses);
        let after = post[0];
        assert!(matches!(cx.function.instruction(after.user).opcode, Opcode::Ret));
        assert_eq!(after.post_inc_loops, [header]);
        assert_ne!(IvUsers::expr(se, cx, after), IvUsers::replacement_expr(se, cx, after));
        let stride = IvUsers::stride(se, cx, after, header).expect("a stride");
        assert_eq!(se.as_constant(stride).map(|one| one.sext_value()), Some(1));
    });
}

/// The stride of the load's address is the element size.
#[test]
fn the_stride_of_an_array_walk_is_the_element_size() {
    with_users(ARRAY, |cx, se, users, header, _| {
        let load = users
            .uses
            .iter()
            .find(|u| matches!(cx.function.instruction(u.user).opcode, Opcode::Load { .. }))
            .expect("the load");
        let stride = IvUsers::stride(se, cx, load, header).expect("a stride");
        assert_eq!(se.as_constant(stride).map(|one| one.sext_value()), Some(4));
    });
}

/// A loop with `n` loads of the walk asks about each instruction once: twice
/// the loads, twice the questions, not four times.
#[test]
fn the_walk_is_linear_in_the_users() {
    let looped = |n: usize| {
        let mut text = String::from(
            "define i32 @f(ptr %p, i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %next, %loop ]\n",
        );
        for k in 0..n {
            text += &format!("  %a{k} = getelementptr i32, ptr %p, i32 %i\n  %v{k} = load i32, ptr %a{k}\n");
        }
        text += "  %next = add i32 %i, 1\n  %c = icmp slt i32 %next, %n\n  br i1 %c, label %loop, label %exit\nexit:\n  ret i32 0\n}\n";
        text
    };
    let asked = |n| {
        let mut asked = 0;
        with_users(&looped(n), |_, _, users, _, count| {
            assert!(users.uses.len() >= n);
            asked = count;
        });
        asked
    };
    let (small, large) = (asked(100), asked(200));
    assert!(large <= 2 * small + 4, "{small} questions for 100 loads, {large} for 200");
}

/// IU1 walked the whole layout to place the blocks of a loop's users, for each
/// loop: memory_expansion -O2 paid 63 M instructions for 471 loops (`IdMap`
/// `from_iter`), quadratic in the function. The function makes the places once
/// for all its loops.
#[test]
fn test_the_block_places_are_made_once_for_all_the_loops_of_a_function() {
    let mut text = String::from("define i32 @f(ptr %p, i32 %n) {\nentry:\n  br label %l0\n");
    for k in 0..6 {
        let from = if k == 0 { "entry".to_string() } else { format!("x{}", k - 1) };
        let next = if k == 5 { "end".to_string() } else { format!("l{}", k + 1) };
        text += &format!(
            "l{k}:\n  %i{k} = phi i32 [ 0, %{from} ], [ %n{k}, %l{k} ]\n  %a{k} = getelementptr i32, ptr %p, i32 %i{k}\n  %v{k} = load i32, ptr %a{k}\n  %n{k} = add i32 %i{k}, 1\n  %c{k} = icmp slt i32 %n{k}, %n\n  br i1 %c{k}, label %l{k}, label %x{k}\nx{k}:\n  %r{k} = add i32 %n{k}, %v{k}\n  br label %{next}\n"
        );
    }
    text += "end:\n  ret i32 0\n}\n";
    let module = llrm_mir::parse::module(&text).expect("a module");
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
    let env = Env { safe_to_speculate: &|_| true, ephemeral: &|_| false, native_widths: &[], use_list_order: false, phi_operand_uses: false };
    let before = llrm_mir::module::block_places_built();
    let headers: Vec<_> = loops.top_level_loops().map(|one| one.get_header()).collect();
    assert_eq!(headers.len(), 6);
    for header in headers {
        IvUsers::of(&mut se, &cx, header, &env);
    }
    assert_eq!(llrm_mir::module::block_places_built() - before, 1);
}

/// IU1 ordered every instruction of the function to sort the two users of a
/// value: d_faces -O2 paid 1.9 M instructions for 34 loops
/// (`IdMap::from_iter`). The users alone are ordered, however much else the
/// function holds.
#[test]
fn test_sorting_the_users_orders_no_more_than_the_users() {
    let mut text = String::from("define i32 @f(ptr %p, i32 %n) {\nentry:\n  %z0 = add i32 %n, 1\n");
    for k in 1..300 {
        text += &format!("  %z{k} = add i32 %z{}, {k}\n", k - 1);
    }
    text += "  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %next, %loop ]\n  %a = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %a\n  %next = add i32 %i, 1\n  %c = icmp slt i32 %next, %n\n  br i1 %c, label %loop, label %exit\nexit:\n  %r = add i32 %next, %v\n  ret i32 %r\n}\n";
    with_users(&text, |_, _, users, _, _| {
        assert!(users.ordered_instructions() <= 8, "{} instructions ordered", users.ordered_instructions());
    });
}

/// A carried set that an edit made stale is told from a current one. The
/// uses of the loop with its exit test on the counter and on the stepped
/// counter differ in the compare's operand.
#[test]
fn a_set_carried_past_an_edit_that_changed_a_use_differs_from_a_fresh_one() {
    let mut carried = Vec::new();
    with_users(ARRAY, |_, _, users, _, _| carried = users.uses.clone());
    let mut again = Vec::new();
    with_users(ARRAY, |_, _, users, _, _| again = users.uses.clone());
    assert_eq!(crate::ivusers::difference(&carried, &again), None);
    let edited = ARRAY.replace("icmp slt i32 %next, %n", "icmp slt i32 %i, %n");
    let mut stale = Vec::new();
    with_users(&edited, |_, _, users, _, _| stale = users.uses.clone());
    let found = crate::ivusers::difference(&carried, &stale).expect("a difference");
    assert!(found.contains("carried"), "{found}");
    carried.pop();
    assert!(crate::ivusers::difference(&carried, &again).is_some());
}

/// Each instruction asked about made a set of its result's users that grew to
/// the highest id (727 instructions apiece, 18 M on memory_expansion); the
/// users of one IV grow no set but the processed one, made with room.
#[test]
fn the_users_of_an_induction_variable_grow_no_set() {
    with_users(ARRAY, |_, _, users, _, _| assert!(!users.uses.is_empty()));
    assert_eq!(GROWN.with(Cell::get), 0);
}

/// The users of `text`'s loop on a target whose layout has no 16-bit integer
/// and declares 16 native (the 6502): the opcodes of the users found.
fn users_on_a_16_bit_native_target(text: &str) -> Vec<String> {
    let module = llrm_mir::parse::module(text).expect("a module");
    let (_, _, function) = module.functions().next().expect("a function");
    let layout = DataLayout::parse("e-p:16:8-i16:8-i32:8-i64:8-a:8-n8").unwrap();
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
    let env = Env { safe_to_speculate: &|_: InstId| true, ephemeral: &|_| false, native_widths: &[16], use_list_order: false, phi_operand_uses: false };
    let users = IvUsers::of(&mut se, &cx, header, &env);
    users.uses.iter().map(|u| format!("{:?}", function.instruction(u.user).opcode)).collect()
}

/// DEVIATE LU19. `arr[zext i8 i]` was rewritten into a 16-bit pointer walk
/// (`adc (rc2),y; iny; bne; inc rc3`) where `adc arr,x` was one instruction:
/// six llrm-6502 tests went red on release/next (2026-11-11). A width the
/// target declares native is for induction variables of that width; the same
/// loop over a 16-bit counter is still reduced.
#[test]
fn a_native_width_is_not_for_a_narrow_counter_an_extension_widens() {
    let walk = |phi: &str, index: &str, bound: &str| {
        format!(
            "@arr = global [256 x i8] zeroinitializer\n\
             define i8 @f(i8 %n) {{\nentry:\n  br label %loop\nloop:\n  \
             %i = phi {phi} [ 0, %entry ], [ %next, %loop ]\n  {index}\n  \
             %a = getelementptr i8, ptr @arr, i16 %z\n  %v = load i8, ptr %a\n  \
             %next = add nuw {phi} %i, 1\n  {bound}\n  \
             br i1 %c, label %loop, label %exit\nexit:\n  ret i8 %v\n}}\n"
        )
    };
    let widened = walk("i8", "%z = zext i8 %i to i16", "%w = zext i8 %next to i16\n  %c = icmp ne i16 %w, 200");
    let native = walk("i16", "%z = add i16 %i, 0", "%c = icmp ne i16 %next, 200");
    let widened = users_on_a_16_bit_native_target(&widened);
    assert!(
        !widened.is_empty() && widened.iter().all(|op| op.starts_with("Cast")),
        "the walk stops at the extensions: {widened:?}"
    );
    let native = users_on_a_16_bit_native_target(&native);
    assert!(
        native.iter().any(|op| !op.starts_with("Cast")),
        "a 16-bit counter reaches the gep and the compare: {native:?}"
    );
}

/// Two address computations on one induction variable, the earlier one's use
/// of it made last (an edit re-adds a use at the head of the list). Descending
/// program order meets the later computation first; the pin walks the use
/// list, newest first. wasm32 memcpy_runtime -O2 started the variable at 0
/// where the pin's started at 3, and memory_expansion was 1307 B over the pin.
#[test]
fn test_the_users_are_met_in_use_list_order_when_the_target_asks() {
    let text = "define i32 @f(ptr %p, ptr %q, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %a = getelementptr i32, ptr %p, i32 %i
  %b = getelementptr i32, ptr %q, i32 %i
  %v = load i32, ptr %a
  %w = load i32, ptr %b
  %next = add i32 %i, 1
  %c = icmp slt i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  %r = add i32 %v, %w
  ret i32 %r
}
";
    let edit = |module: &mut llrm_mir::module::Module| {
        let (_, function) = module.function_mut("f").expect("@f");
        // The loop is the second block; its first gep is `%a`.
        let a = function
            .iter_block(function.layout()[1])
            .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::GetElementPtr { .. }))
            .expect("the gep of %a");
        let index = function.instruction(a).operands[1];
        function.set_operand(a, 1, index);
    };
    let first = |use_list_order| {
        let mut found = Vec::new();
        with_edited_users(text, use_list_order, edit, |cx, _, users, _, _| {
            found = users.uses.iter().map(|u| named(cx, u)).collect();
        });
        found
    };
    assert_eq!(first(true).first().map(|(_, v)| v.as_str()), Some("a"), "the newest use is met first");
    assert_eq!(first(false).first().map(|(_, v)| v.as_str()), Some("next"), "the default is program order");
}

/// DEVIATE LSR-PHI. x_cordic m32 -O3 merges `i*300+21472` and `i*300-81472` in
/// one phi: the pin's "do not recurse into a phi twice" keeps the first
/// operand as a use and drops the second, which stays a multiply chain (100
/// trips of 5 instructions that gcc does not run). Where the target asks, each
/// incoming value of the phi is a use.
#[test]
fn a_phi_of_two_affine_values_has_a_use_per_operand_where_the_target_asks() {
    let text = "define i32 @f(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %latch ]
  %m = mul nuw nsw i32 %i, 300
  %c = icmp ult i32 %i, 100
  br i1 %c, label %a, label %b
a:
  %pa = add nuw nsw i32 %m, 21472
  br label %latch
b:
  %pb = add nsw i32 %m, -81472
  br label %latch
latch:
  %p = phi i32 [ %pa, %a ], [ %pb, %b ]
  store volatile i32 %p, ptr null
  %next = add nuw nsw i32 %i, 1
  %e = icmp eq i32 %next, %n
  br i1 %e, label %exit, label %loop
exit:
  ret i32 0
}
";
    let phi_operands = |asked: bool| {
        let mut found = Vec::new();
        with_flagged_users(text, false, asked, |_| {}, |cx, _, users, _, _| {
            found = users.uses.iter().map(|u| named(cx, u)).filter(|(opcode, _)| opcode == "Phi").collect();
        });
        found.sort();
        found
    };
    assert_eq!(phi_operands(false).len(), 1, "the pin keeps one operand: {:?}", phi_operands(false));
    let both = phi_operands(true);
    assert_eq!(both.iter().map(|(_, v)| v.trim_start_matches('%')).collect::<Vec<_>>(), ["pa", "pb"], "{both:?}");
}
