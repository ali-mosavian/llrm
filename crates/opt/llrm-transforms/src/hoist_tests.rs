//! Adapted from llrm-core's `optimize/transform_tests.rs` hoist tests, each
//! body now MIR text the interpreter runs before and after.
//!
//! Skipped: test_a_run_whose_flag_the_loop_still_reads_is_not_hoistable
//! (flags),
//! test_a_definition_a_phi_carries_and_the_loop_rewrites_does_not_leave_it and
//! test_reparenting_a_hoisted_pointer_keeps_its_object_facts (variables and
//! their renaming; a value here is SSA).

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
    passes.add(super::Hoist { size: false });
    passes
        .run_module(&mut after, std::rc::Rc::new(llrm_mir::target::Neutral))
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    (before, after)
}

/// `text` hoisted, answering as before on `inputs`; its text.
fn checked(
    text: &str,
    inputs: &[Vec<i128>],
) -> String {
    let (before, after) = hoisted(text);
    let inputs: Vec<&[i128]> = inputs.iter().map(Vec::as_slice).collect();
    assert_eq!(results(&after, &inputs), results(&before, &inputs), "{}", printed(&after));
    printed(&after)
}

/// The instructions of @f's block `name`, in `text`.
fn block(
    text: &str,
    name: &str,
) -> Vec<String> {
    block_of(text, "f", name)
}

fn block_of(
    text: &str,
    function: &str,
    name: &str,
) -> Vec<String> {
    let f = &text[text.find(&format!("define i16 @{function}(")).expect("the function")..];
    let start = f.find(&format!("\n{name}:\n")).unwrap_or_else(|| panic!("no {name} in\n{text}")) + name.len() + 3;
    f[start..].lines().take_while(|line| line.starts_with("  ")).map(|line| line.trim().to_owned()).collect()
}

/// `s += v` over `i < bound`, `v` computed by `body` in the loop; `pre`
/// goes before it and `head` into its header.
fn looped(
    declared: &str,
    parameters: &str,
    pre: &str,
    head: &str,
    bound: &str,
    body: &str,
) -> String {
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
    let text = looped(
        GLOBALS,
        ", i16 %a",
        "",
        "",
        "%n",
        "  %l = load i16, ptr @g\n  %v = mul i16 %l, %a\n  store i16 %i, ptr @h\n",
    );
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
    let text = looped(
        GLOBALS,
        "",
        "",
        "",
        "%n",
        "  %v = load i16, ptr @g\n  store volatile i16 %i, ptr @h\n  %w = load volatile i16, ptr @h\n",
    );
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
        let body =
            format!("  %k = mul i16 %a, 3\n  call void @{callee}()\n  %l = load i16, ptr @g\n  %v = add i16 %k, %l\n");
        let text = looped(&format!("{GLOBALS}{callees}"), ", i16 %a", "", "", "%n", &body);
        let done = checked(&text, &trips_and(&[0, 2]));
        let expected: &[&str] = if moves {
            &["%k = mul i16 %a, 3", "%l = load i16, ptr @g", "%v = add i16 %k, %l", "br label %b1"]
        } else {
            &["%k = mul i16 %a, 3", "br label %b1"]
        };
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
    let guarded = text(
        "%g = icmp sgt i16 %n, 0\n  br i1 %g, label %b1, label %b3",
        "%r = phi i16 [ 0, %b0 ], [ %s, %b1 ]\n  ret i16 %r",
    );
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
    assert!(
        body.iter().all(|line| !line.contains("load i16, ptr %d") && !line.contains("load ptr, ptr %data")),
        "{body:?}"
    );
}

/// The language says a load reads what is written once and never again: a store
/// the loop cannot rule out as that cell (any pointer) does not keep it in the
/// loop.
#[test]
fn test_a_load_the_language_says_is_invariant_leaves_past_a_store() {
    let with = |load: &str| {
        format!(
            "{}\n!0 = !{{}}\n",
            looped(GLOBALS, ", ptr %p", "", "", "%n", &format!("  %v = {load}\n  store i16 %i, ptr %p\n"))
        )
    };
    let (_, plain) = hoisted(&with("load i16, ptr @g"));
    assert!(
        block_of(&printed(&plain), "f", "b2").iter().any(|line| line.contains("load i16, ptr @g")),
        "a plain load stays"
    );
    let (_, stated) = hoisted(&with("load i16, ptr @g, !invariant.load !0"));
    let printed = printed(&stated);
    assert!(block_of(&printed, "f", "b2").iter().all(|line| !line.contains("load i16, ptr @g")), "{printed}");
    assert!(block_of(&printed, "f", "b0").iter().any(|line| line.contains("load i16, ptr @g")), "{printed}");
}

/// The function of the test below: two loops inside one, each with three
/// invariant loads past six registers.
fn two_inner_loops() -> String {
    let loads = |names: [&str; 3]| {
        names.map(|name| format!("  %{name} = load i16, ptr @{name}\n  %w{name} = shl i16 %{name}, 1\n")).concat()
    };
    let inner = |at: &str, names: [&str; 3], next: &str| {
        format!(
            "{at}:\n  %i{at} = phi i16 [ 0, %{pre} ], [ %n{at}, %{at} ]\n  %s{at} = phi i16 [ %t, %{pre} ], [ %r{at}, %{at} ]\n{loads}  %u{at} = add i16 %w{a}, %w{b}\n  %v{at} = add i16 %u{at}, %w{c}\n  %r{at} = add i16 %s{at}, %v{at}\n  %n{at} = add i16 %i{at}, 1\n  %k{at} = icmp slt i16 %n{at}, %n\n  br i1 %k{at}, label %{at}, label %{next}\n\n",
            pre = if at == "b3" { "b2" } else { "b4" },
            loads = loads(names),
            a = names[0],
            b = names[1],
            c = names[2],
        )
    };
    let text = format!(
        "{globals}define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %j = phi i16 [ 0, %b0 ], [ %j1, %b6 ]
  %t = phi i16 [ 0, %b0 ], [ %sb5, %b6 ]
  %c = icmp slt i16 %j, %n
  br i1 %c, label %b2, label %b9

b2:
  br label %b3

{first}b4:
  br label %b5

{second}b6:
  %j1 = add i16 %j, 1
  br label %b1

b9:
  ret i16 %t
}}
",
        globals = ["g1", "g2", "g3", "g4", "g5", "g6"].map(|name| format!("@{name} = global i16 0\n")).concat(),
        first = inner("b3", ["g1", "g2", "g3"], "b4"),
        second = inner("b5", ["g4", "g5", "g6"], "b6"),
    );
    text
}

/// Two inner loops each read three globals nothing writes and double them.
/// Hoisted out of the outer loop too, six values live across both inner loops,
/// more than the nine registers hold with the loop's own counters (PLASMABLOBS
/// -Os +73 B, #529). At -Os the ones past the registers stay in their inner
/// preheader.
#[test]
fn test_invariants_past_the_registers_stay_in_the_inner_preheader() {
    let text = two_inner_loops();
    let before = parsed(&text);
    let mut after = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.require::<Summaries>();
    passes.add(super::Hoist { size: true });
    passes
        .run_module(&mut after, std::rc::Rc::new(crate::testing::Tuned { registers: 6, ..Default::default() }))
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    let printed = printed(&after);
    let hoisted = block(&printed, "b0").iter().filter(|line| line.contains("load")).count();
    assert!(hoisted < 6, "{hoisted} of 6 loads left both inner loops\n{printed}");
}

/// A loop that calls and reads six fields of its argument: each `getelementptr`
/// of a constant is folded into its load, a displacement from the argument, but
/// counted as a value live across the calls it was forecast spilled, and left
/// in the loop (QCport part.c +56 B at -Os, #529).
#[test]
fn test_a_displacement_the_loads_fold_is_not_pruned_for_the_registers() {
    let fields = (1..=6).map(|at| format!("  %p{at} = getelementptr i8, ptr %p, i16 {}\n  %v{at} = load i16, ptr %p{at}\n  %s{at} = add i16 %s{}, %v{at}\n", at * 2, at - 1)).collect::<String>();
    let text = format!(
        "declare void @g()
define i16 @f(ptr %p, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %s0 = phi i16 [ 0, %b0 ], [ %s6, %b1 ]
  call void @g()
{fields}  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  ret i16 %s6
}}
"
    );
    let before = parsed(&text);
    let mut after = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.require::<Summaries>();
    passes.add(super::Hoist { size: true });
    passes
        .run_module(
            &mut after,
            std::rc::Rc::new(crate::testing::Tuned { registers: 3, call_registers: 1, ..Default::default() }),
        )
        .unwrap_or_else(|error| panic!("{error}\n{text}"));
    let printed = printed(&after);
    let hoisted = block(&printed, "b0").iter().filter(|line| line.contains("getelementptr")).count();
    assert_eq!(hoisted, 6, "{printed}");
}

/// A loop's invariant `double` load, hoisted: a value on a stack machine is
/// held across the loop and released after it. x86-m32's -Os grew 4 B per float
/// hoisted (nbody: two `fld` and two `fstp st(0)` out of an inner loop
/// of 1.5 trips) because the price counted a move and no release.
fn float_hoist(
    size: bool,
    release: i64,
) -> usize {
    let text = "@g = global double 1.0

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %i1, %b1 ]
  %s = phi double [ 0.0, %b0 ], [ %s1, %b1 ]
  %v = load double, ptr @g
  %s1 = fadd double %s, %v
  %i1 = add i16 %i, 1
  %c = icmp slt i16 %i1, %n
  br i1 %c, label %b1, label %b2

b2:
  %r = fptosi double %s1 to i16
  ret i16 %r
}
";
    let before = parsed(text);
    let mut after = before.clone();
    let mut passes = llrm_mir::passes::PassManager::default();
    (passes.verify_each, passes.verify_invalidation) = (true, true);
    passes.require::<Summaries>();
    passes.add(super::Hoist { size });
    let prices = crate::profit::OperationCosts { float_release: release, ..Default::default() };
    let machine = crate::testing::Tuned { registers: 6, costs: prices.clone(), sizes: prices, ..Default::default() };
    passes.run_module(&mut after, std::rc::Rc::new(machine)).unwrap_or_else(|error| panic!("{error}\n{text}"));
    block(&printed(&after), "b0").iter().filter(|line| line.contains("load")).count()
}

#[test]
fn test_a_float_load_is_not_hoisted_where_its_release_costs_the_code_more() {
    // Moved out of the loop its price is the same: only the release after the
    // loop differs.
    assert_eq!(float_hoist(true, 0), 1, "unpriced, it is hoisted");
    assert_eq!(float_hoist(true, 2), 0, "released after the loop at 2 bytes, it stays");
}

/// At speed the loop's trips pay for the release: 10 loads saved against one
/// release of 2.
#[test]
fn test_a_float_load_is_hoisted_where_the_loop_pays_for_its_release() {
    assert_eq!(float_hoist(false, 2), 1);
}

/// A chain of loads each reading through the one before: one more of them is
/// ready each round of `_invariant_run`, and a load that was not ready was
/// asked again, round after round, whether any write in the loop reaches it
/// (x_life, d_alias: hoist's alias queries `memoryssa::spares` ->
/// `regions::overlapping`, 1.3 of its 3.4 points). A load is asked once.
#[test]
fn test_a_load_that_waits_for_the_one_before_is_asked_whether_the_loop_writes_it_once() {
    let globals = "@a = global ptr @b\n@b = global ptr @c\n@c = global ptr @d\n@d = global ptr @e\n@e = global i16 7\n@w = global i16 0\n\n";
    let text = looped(
        globals,
        "",
        "",
        "",
        "%n",
        "  store i16 %i, ptr @w\n  %p1 = load ptr, ptr @a\n  %p2 = load ptr, ptr %p1\n  %p3 = load ptr, ptr %p2\n  %p4 = load ptr, ptr %p3\n  %v = load i16, ptr %p4\n",
    );
    let asked = crate::transform::undisturbed_asked();
    let out = checked(&text, &trips());
    let asks = crate::transform::undisturbed_asked() - asked;
    assert!(out.matches("load").count() >= 5);
    assert!(asks <= 5, "{asks} asks for 5 loads");
}

/// A motion's price is its work and one spill forecast; hoist found the
/// forecast twice (once for the price, again for the spilled set), 41% of its
/// time on a 16-deep loop nest.
#[test]
fn test_a_motion_is_priced_by_one_forecast() {
    let text = two_inner_loops();
    let mut after = parsed(&text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.require::<Summaries>();
    passes.add(super::Hoist { size: true });
    crate::profit::PRICED.with(|count| count.set((0, 0)));
    passes
        .run_module(&mut after, std::rc::Rc::new(crate::testing::Tuned { registers: 6, ..Default::default() }))
        .unwrap();
    let (priced, forecast) = crate::profit::PRICED.with(std::cell::Cell::get);
    assert!(priced > 0, "nothing was priced");
    assert_eq!(forecast, priced, "a price and its forecast were found separately");
}

/// Each loop whose run was priced priced the function as it stood again for
/// `kept`, though the loop before had just priced it moved (branches(512) at
/// -O2: 64 loops, 128 whole-function prices). The price of a function a motion
/// was accepted for is the next motion's `kept`.
#[test]
fn test_a_function_a_motion_was_priced_for_is_not_priced_again_for_the_next() {
    let text = two_inner_loops();
    let mut after = parsed(&text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.require::<Summaries>();
    passes.add(super::Hoist { size: true });
    crate::profit::PRICED.with(|count| count.set((0, 0)));
    passes
        .run_module(&mut after, std::rc::Rc::new(crate::testing::Tuned { registers: 6, ..Default::default() }))
        .unwrap();
    let (priced, _) = crate::profit::PRICED.with(std::cell::Cell::get);
    assert!(priced > 0, "nothing was priced");
    // Seven before: each of the three motions priced `kept` and its move.
    assert!(priced <= 5, "{priced} prices of the function for three motions");
}

/// The traffic of every cell was found for each motion to price the values it
/// makes cross the loop, which most motions have none of that the model
/// spills (4.5% of branches(512) at -O2): the forecast finds it once, and
/// hoist does only when one is asked.
#[test]
fn test_a_motion_that_makes_no_spilled_value_cross_finds_no_traffic_of_its_own() {
    let text = two_inner_loops();
    let mut after = parsed(&text);
    let mut passes = llrm_mir::passes::PassManager::default();
    passes.require::<Summaries>();
    passes.add(super::Hoist { size: true });
    crate::profit::PRICED.with(|count| count.set((0, 0)));
    crate::spill::TRAFFIC.with(|count| count.set(0));
    passes
        .run_module(&mut after, std::rc::Rc::new(crate::testing::Tuned { registers: 16, ..Default::default() }))
        .unwrap();
    let (priced, forecast) = crate::profit::PRICED.with(std::cell::Cell::get);
    let found = crate::spill::TRAFFIC.with(std::cell::Cell::get);
    assert!(priced > 0, "nothing was priced");
    assert_eq!(found, forecast, "{found} traffics for {forecast} forecasts");
}
