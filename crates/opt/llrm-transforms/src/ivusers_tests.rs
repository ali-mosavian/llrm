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
    let asked = Cell::new(0);
    let safe = |_: InstId| {
        asked.set(asked.get() + 1);
        true
    };
    let env = Env { safe_to_speculate: &safe, ephemeral: &|_| false };
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

/// Two loads written before the header read `%i` as a forward reference:
/// LLVM resolves them when `%i` is defined, in the order that makes the
/// earlier one first in the use list, so the earlier load is walked first.
/// IVUsers followed descending program order (IU1): the later load first, a
/// different tie-break in LSR's register choice (m64 matmul -Os +17 B).
#[test]
fn test_users_are_visited_in_use_list_order() {
    const FORWARD: &str = "define i32 @f(ptr %p, ptr %q, i32 %n) {
entry:
  br label %loop
one:
  %a1 = getelementptr i32, ptr %p, i32 %i
  %v1 = load i32, ptr %a1
  br label %two
two:
  %a2 = getelementptr i32, ptr %q, i32 %i
  %v2 = load i32, ptr %a2
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %two ]
  %next = add i32 %i, 1
  %c = icmp slt i32 %next, %n
  br i1 %c, label %one, label %exit
exit:
  ret i32 %next
}
";
    with_users(FORWARD, |cx, _, users, _, _| {
        let order: Vec<_> = users.uses.iter().map(|u| named(cx, u).1).collect();
        let first = order.iter().position(|o| o.trim_start_matches('%') == "a1").expect("the first load is a user");
        let second = order.iter().position(|o| o.trim_start_matches('%') == "a2").expect("the second load is a user");
        assert!(first < second, "the earlier forward reference is first in the use list: {order:?}");
    });
}
