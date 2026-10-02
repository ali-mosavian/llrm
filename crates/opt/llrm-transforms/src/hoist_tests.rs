//! Adapted from llrm-core's `optimize/transform_tests.rs` hoist tests, each
//! body now MIR text the interpreter runs before and after.
//!
//! Skipped: test_a_run_whose_flag_the_loop_still_reads_is_not_hoistable
//! (flags), test_a_definition_a_phi_carries_and_the_loop_rewrites_does_not_leave_it
//! and test_reparenting_a_hoisted_pointer_keeps_its_object_facts (variables
//! and their renaming; a value here is SSA).

use llrm_analysis::manager::Summaries;
use llrm_mir::module::Module;

use crate::testing::{parsed, printed, results};

/// `text` through `Hoist`, the module's summaries required, under the
/// verifier and preserved-analyses check.
fn hoisted(text: &str) -> (Module, Module) {
    let before = parsed(text);
    let mut after = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.require::<Summaries>();
    passes.add(super::Hoist);
    passes.run_module(&mut after, std::rc::Rc::new(llrm_mir::target::Neutral)).unwrap_or_else(|error| panic!("{error}\n{text}"));
    (before, after)
}

/// `text` hoisted, answering as before on `inputs`; its text.
fn checked(text: &str, inputs: &[Vec<i128>]) -> String {
    let (before, after) = hoisted(text);
    let inputs: Vec<&[i128]> = inputs.iter().map(Vec::as_slice).collect();
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{}", printed(&after));
    printed(&after)
}

/// The instructions of @f's block `name`, in `text`.
fn block(text: &str, name: &str) -> Vec<String> {
    block_of(text, "f", name)
}

fn block_of(text: &str, function: &str, name: &str) -> Vec<String> {
    let f = &text[text.find(&format!("define i16 @{function}(")).expect("the function")..];
    let start = f.find(&format!("\n{name}:\n")).unwrap_or_else(|| panic!("no {name} in\n{text}")) + name.len() + 3;
    f[start..].lines().take_while(|line| line.starts_with("  ")).map(|line| line.trim().to_owned()).collect()
}

/// `s += v` over `i < bound`, `v` computed by `body` in the loop; `pre`
/// goes before it and `head` into its header.
fn looped(declared: &str, parameters: &str, pre: &str, head: &str, bound: &str, body: &str) -> String {
    format!(
        "{declared}define i16 @f(i16 %n{parameters}) {{
b0:
{pre}  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
{head}  %c = icmp slt i16 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
{body}  %s1 = add i16 %s, %v
  %i1 = add i16 %i, 1
  br label %b1

b3:
  ret i16 %s
}}
"
    )
}

fn trips_and(values: &[i128]) -> Vec<Vec<i128>> {
    [0, 1, 5, -1].iter().flat_map(|&n| values.iter().map(move |&one| vec![n, one])).collect()
}

fn trips() -> Vec<Vec<i128>> {
    [0, 1, 5, -1].iter().map(|&n| vec![n]).collect()
}

const GLOBALS: &str = "@g = global i16 5\n@h = global i16 0\n\n";

#[test]
fn test_invariant_arithmetic_leaves_its_loop_in_order() {
    let text = looped("", ", i16 %a", "", "", "%n", "  %k = mul i16 %a, 3\n  %v = add i16 %k, 1\n");
    let done = checked(&text, &trips_and(&[0, 1, -1, 32767, -32768]));
    assert_eq!(block(&done, "b0"), ["%k = mul i16 %a, 3", "%v = add i16 %k, 1", "br label %b1"]);
    assert!(block(&done, "b1").contains(&"%c = icmp slt i16 %i, %n".to_owned()), "{done}");
}

/// A load and the multiply behind it leave together.
#[test]
fn test_an_operand_nothing_writes_down_may_leave_with_its_run() {
    let text = looped(GLOBALS, ", i16 %a", "", "", "%n", "  %l = load i16, ptr @g\n  %v = mul i16 %l, %a\n  store i16 %i, ptr @h\n");
    let done = checked(&text, &trips_and(&[0, 3, -1]));
    assert_eq!(block(&done, "b0"), ["%l = load i16, ptr @g", "%v = mul i16 %l, %a", "br label %b1"]);
}

#[test]
fn test_a_load_the_loop_may_write_stays() {
    // The same global, then a pointer that may be it.
    for (parameters, store) in [("", "store i16 %i, ptr @g"), (", ptr %p", "store i16 %i, ptr %p")] {
        let text = looped(GLOBALS, parameters, "", "", "%n", &format!("  %v = load i16, ptr @g\n  {store}\n"));
        let (before, after) = hoisted(&text);
        assert_eq!(printed(&after), printed(&before));
    }
}

/// A precise volatile store orders only other volatile accesses; the
/// volatile access itself never moves.
#[test]
fn test_a_precise_volatile_access_does_not_block_disjoint_invariant_work() {
    let text = looped(GLOBALS, "", "", "", "%n", "  %v = load i16, ptr @g\n  store volatile i16 %i, ptr @h\n  %w = load volatile i16, ptr @h\n");
    let done = checked(&text, &trips());
    assert_eq!(block(&done, "b0"), ["%v = load i16, ptr @g", "br label %b1"]);
    assert!(block(&done, "b2").contains(&"%w = load volatile i16, ptr @h".to_owned()), "{done}");
}

/// A call keeps only the loads it may write; the old pass kept the whole
/// loop around any call.
#[test]
fn test_a_call_keeps_what_it_may_write() {
    let callees = "define void @writes() {\nb0:\n  store i16 9, ptr @g\n  ret void\n}\n\ndefine void @elsewhere() {\nb0:\n  store i16 9, ptr @h\n  ret void\n}\n\n";
    for (callee, moves) in [("writes", false), ("elsewhere", true)] {
        let body = format!("  %k = mul i16 %a, 3\n  call void @{callee}()\n  %l = load i16, ptr @g\n  %v = add i16 %k, %l\n");
        let text = looped(&format!("{GLOBALS}{callees}"), ", i16 %a", "", "", "%n", &body);
        let done = checked(&text, &trips_and(&[0, 2]));
        let expected: &[&str] = if moves { &["%k = mul i16 %a, 3", "%l = load i16, ptr @g", "%v = add i16 %k, %l", "br label %b1"] } else { &["%k = mul i16 %a, 3", "br label %b1"] };
        assert_eq!(block(&done, "b0"), expected, "{callee}");
    }
}

/// `v = arr[k]`, `arr` a local array of four: a load that may fault.
fn indexed(bound: &str) -> String {
    let pre = "  %arr = alloca [4 x i16]\n  store i16 10, ptr %arr\n";
    looped("", ", i16 %k", pre, "", bound, "  %q = getelementptr i16, ptr %arr, i16 %k\n  %v = load i16, ptr %q\n")
}

/// A load the loop might never run stays, where running it could fault.
#[test]
fn test_a_load_that_may_fault_stays_unless_a_trip_is_certain() {
    let done = checked(&indexed("%n"), &[vec![0, 100], vec![-1, 100], vec![3, 0]]);
    assert!(block(&done, "b2").contains(&"%v = load i16, ptr %q".to_owned()), "{done}");
    assert!(block(&done, "b0").contains(&"%q = getelementptr i16, ptr %arr, i16 %k".to_owned()), "{done}");
    // Four trips certainly run it.
    let done = checked(&indexed("4"), &[vec![0, 0]]);
    assert!(block(&done, "b0").contains(&"%v = load i16, ptr %q".to_owned()), "{done}");
}

/// The header runs whenever the loop is entered; a guard before it means
/// the loop may not be.
#[test]
fn test_a_header_load_is_certain_only_behind_an_unconditional_entry() {
    let text = |entry: &str, exit: &str| {
        format!(
            "define i16 @loop(ptr %p, i16 %n) {{
b0:
  {entry}

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %s = phi i16 [ 0, %b0 ], [ %s1, %b2 ]
  %v = load i16, ptr %p
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %s1 = add i16 %s, %v
  %i1 = add i16 %i, 1
  br label %b1

b3:
  {exit}
}}

define i16 @f(i16 %n) {{
b0:
  %a = alloca i16
  store i16 7, ptr %a
  %r = call i16 @loop(ptr %a, i16 %n)
  ret i16 %r
}}
"
        )
    };
    let entered = checked(&text("br label %b1", "ret i16 %s"), &trips());
    assert_eq!(block_of(&entered, "loop", "b0"), ["%v = load i16, ptr %p", "br label %b1"]);
    let guarded = text("%g = icmp sgt i16 %n, 0\n  br i1 %g, label %b1, label %b3", "%r = phi i16 [ 0, %b0 ], [ %s, %b1 ]\n  ret i16 %r");
    let (before, after) = hoisted(&guarded);
    assert_eq!(printed(&after), printed(&before));
}

#[test]
fn test_a_division_that_may_trap_stays() {
    for divisor in ["%d", "-1", "0"] {
        let text = looped("", ", i16 %a, i16 %d", "", "", "%n", &format!("  %v = sdiv i16 %a, {divisor}\n"));
        let (before, after) = hoisted(&text);
        assert_eq!(printed(&after), printed(&before), "{divisor}");
    }
    let text = looped("", ", i16 %a", "", "", "%n", "  %v = sdiv i16 %a, 7\n");
    let done = checked(&text, &trips_and(&[0, -32768, 32767]));
    assert_eq!(block(&done, "b0"), ["%v = sdiv i16 %a, 7", "br label %b1"]);
}

/// Inner loops go first, so what leaves one leaves the next.
#[test]
fn test_invariant_work_leaves_every_loop_it_is_invariant_in() {
    let text = "define i16 @f(i16 %n, i16 %a) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b4 ]
  %s = phi i16 [ 0, %b0 ], [ %t1, %b4 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b5

b2:
  br label %b3

b3:
  %j = phi i16 [ 0, %b2 ], [ %j1, %b3 ]
  %t = phi i16 [ %s, %b2 ], [ %t1, %b3 ]
  %k = mul i16 %a, 3
  %t1 = add i16 %t, %k
  %j1 = add i16 %j, 1
  %d = icmp slt i16 %j1, 3
  br i1 %d, label %b3, label %b4

b4:
  %i1 = add i16 %i, 1
  br label %b1

b5:
  ret i16 %s
}
";
    let done = checked(text, &trips_and(&[0, 1, -7]));
    assert_eq!(block(&done, "b0"), ["%k = mul i16 %a, 3", "br label %b1"]);
}

/// `a(i)`, loaded under `if j <> i` in the inner of two counted loops,
/// leaves for the inner loop's preheader: its index's bounds there keep
/// every byte inside `@a`, so running it where the loop would not cannot
/// fault. nbody's
/// `posX(body)` was loaded again in every copy of the unrolled inner loop.
#[test]
fn a_load_its_index_bounds_keep_inside_its_object_leaves_the_loop() {
    let text = "@a = internal global [12 x i8] zeroinitializer

define i16 @f(i16 %x) {
b0:
  br label %outer

outer:
  %i = phi i16 [ 0, %b0 ], [ %in, %outerlatch ]
  %s = phi i16 [ 0, %b0 ], [ %t, %outerlatch ]
  %go = icmp slt i16 %i, 6
  br i1 %go, label %pre, label %done

pre:
  br label %inner

inner:
  %j = phi i16 [ 0, %pre ], [ %jn, %latch ]
  %t = phi i16 [ %s, %pre ], [ %u, %latch ]
  %more = icmp slt i16 %j, 6
  br i1 %more, label %body, label %outerlatch

body:
  %other = icmp ne i16 %j, %i
  br i1 %other, label %use, label %latch

use:
  %q = getelementptr inbounds i16, ptr @a, i16 %i
  %v = load i16, ptr %q
  %w = add i16 %t, %v
  br label %latch

latch:
  %u = phi i16 [ %w, %use ], [ %t, %body ]
  %jn = add i16 %j, 1
  br label %inner

outerlatch:
  %in = add i16 %i, 1
  br label %outer

done:
  ret i16 %s
}
";
    let done = checked(&format!("{}{text}", llrm_analysis::testing::DOS), &[vec![0]]);
    assert!(block(&done, "pre").iter().any(|one| one.contains("load")), "{done}");
}

/// A slice descriptor is a `noalias readonly` parameter and its elements are
/// written through a pointer loaded from it: the length and the data pointer
/// it holds are invariant whatever the stores write. Overlap with an
/// unknown pointer kept both in Nib's loops, and with them every bounds
/// check out of reach of the counting passes.
#[test]
fn test_a_load_from_a_readonly_noalias_parameter_leaves_past_a_store() {
    let text = "define i16 @f(i16 %n, ptr noalias readonly dereferenceable(16) %d) {
b0:
  %data = getelementptr i8, ptr %d, i16 2
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b2 ]
  %c = icmp slt i16 %i, %n
  br i1 %c, label %b2, label %b3

b2:
  %p = load ptr, ptr %data
  %q = getelementptr i8, ptr %p, i16 %i
  store i8 1, ptr %q
  %v = load i16, ptr %d
  %i1 = add i16 %i, %v
  br label %b1

b3:
  ret i16 %i
}
";
    let (_, after) = hoisted(text);
    let body = block_of(&printed(&after), "f", "b2");
    assert!(body.iter().all(|line| !line.contains("load i16, ptr %d") && !line.contains("load ptr, ptr %data")), "{body:?}");
}

/// The language says a load reads what is written once and never again: a store the loop
/// cannot rule out as that cell (any pointer) does not keep it in the loop.
#[test]
fn test_a_load_the_language_says_is_invariant_leaves_past_a_store() {
    let with = |load: &str| format!("{}\n!0 = !{{}}\n", looped(GLOBALS, ", ptr %p", "", "", "%n", &format!("  %v = {load}\n  store i16 %i, ptr %p\n")));
    let (_, plain) = hoisted(&with("load i16, ptr @g"));
    assert!(block_of(&printed(&plain), "f", "b2").iter().any(|line| line.contains("load i16, ptr @g")), "a plain load stays");
    let (_, stated) = hoisted(&with("load i16, ptr @g, !invariant.load !0"));
    let printed = printed(&stated);
    assert!(block_of(&printed, "f", "b2").iter().all(|line| !line.contains("load i16, ptr @g")), "{printed}");
    assert!(block_of(&printed, "f", "b0").iter().any(|line| line.contains("load i16, ptr @g")), "{printed}");
}
