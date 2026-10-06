//! `lsr` over loops the interpreter runs before and after, over trip
//! counts below zero, zero, one and more, on a target with a word address
//! form of three registers and a scaled dword one.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_analysis::testing::DOS;
use llrm_mir::module::Module;
use llrm_mir::passes::PassManager;
use llrm_mir::target::AddressForm;

use super::Lsr;
use crate::profit::OperationCosts;
use crate::testing::{Tuned, parsed, printed, results};

/// A 486's prices, six registers, two across a call, and its two address forms.
fn target() -> Tuned {
    let costs = OperationCosts { add: 1, multiply: 13, divide: 24, shift: 2, address: 1, load: 1, store: 1, memory_update: 3, extend: 3, prefix: 1, ..OperationCosts::default() };
    let word = AddressForm { partners: Some(2), bases: Some(1), indices: Some(2), ..AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("a form") };
    let dword = AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 1, costs.prefix, costs.extend, true, None).expect("a form");
    Tuned { costs, registers: 6, call_registers: 2, address_forms: vec![word, dword], ..Tuned::default() }
}

/// `text` in the DOS layout, and it through `Lsr`, printed.
fn reduced(text: &str) -> (Module, String) {
    reduced_for(text, target())
}

/// `reduced` on `machine`.
fn reduced_for(text: &str, machine: Tuned) -> (Module, String) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut after = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Lsr);
    manager.run_module(&mut after, Rc::new(machine)).unwrap();
    let text = printed(&after);
    (before, text)
}

/// `text` reduced, computing what it did for each of `inputs`.
fn same(text: &str, inputs: &[&[i128]]) -> String {
    let (before, printed) = reduced(text);
    let after = parsed(&printed);
    assert_eq!(results(&after, inputs), results(&before, inputs), "{printed}");
    printed
}

/// `n` and `k` for every loop below: no trip, one, several, and the
/// arrays' whole length.
const TRIPS: &[&[i128]] = &[&[-3, 5], &[0, 5], &[1, 5], &[2, -9], &[7, 3], &[32, 11]];

/// The counters and pointer recurrences `induction` finds in the loops
/// outside the fills: what the loops under test step.
fn counters(printed: &str) -> usize {
    let mut module = parsed(printed);
    let (layout, outer) = (llrm_analysis::testing::layout(&module), llrm_mir::passes::Outer::of(&module, None));
    let (context, function) = module.function_mut("f").expect("@f");
    let unit = llrm_analysis::memory::Unit::within(context, &layout, function, &outer);
    let filling = |loop_: &llrm_analysis::graph::loops::Loop| function.block(llrm_analysis::cfg::block(loop_.header)).name.as_deref().is_some_and(|name| name.starts_with("fill_"));
    unit.shape()
        .loops
        .iter()
        .filter(|loop_| !filling(loop_))
        .map(|loop_| llrm_analysis::induction::basics(&unit, loop_).len() + llrm_analysis::induction::pointers(&unit, loop_).len())
        .sum()
}

/// Lines of the blocks in `printed` whose instructions read `name`.
fn readers<'a>(printed: &'a str, name: &str) -> Vec<&'a str> {
    printed.lines().filter(|line| line.contains(&format!("{name},")) || line.ends_with(name)).collect()
}

/// `@name[j] = j * 7 - k` for `j` below 40, entered from `from`, leaving
/// to `next`: the arrays the loops read.
fn filled(name: &str, element: &str, bytes: u32, from: &str, next: &str) -> String {
    let narrowed = if element == "i32" { format!("add i32 %{name}.x, 0") } else { format!("trunc i32 %{name}.x to {element}") };
    format!(
        "  br label %fill_{name}_1

fill_{name}_1:
  %{name}.j = phi i16 [ 0, %{from} ], [ %{name}.j1, %fill_{name}_2 ]
  %{name}.more = icmp slt i16 %{name}.j, 40
  br i1 %{name}.more, label %fill_{name}_2, label %{next}

fill_{name}_2:
  %{name}.v = mul i16 %{name}.j, 7
  %{name}.w = sub i16 %{name}.v, %k
  %{name}.x = sext i16 %{name}.w to i32
  %{name}.t = {narrowed}
  %{name}.o = mul i16 %{name}.j, {bytes}
  %{name}.p = getelementptr i8, ptr @{name}, i16 %{name}.o
  store {element} %{name}.t, ptr %{name}.p
  %{name}.j1 = add i16 %{name}.j, 1
  br label %fill_{name}_1

{next}:
"
    )
}

/// A function of `n` and `k` that fills `arrays`, then runs `body` from
/// its block `start`.
fn program(arrays: &[(&str, &str, u32)], returns: &str, body: &str) -> String {
    let mut text = arrays.iter().map(|(name, element, _)| format!("@{name} = global [64 x {element}] zeroinitializer\n")).collect::<String>();
    text += &format!("\ndefine {returns} @f(i16 %n, i16 %k) {{\nentry:\n");
    let mut from = "entry".to_owned();
    for (index, (name, element, bytes)) in arrays.iter().enumerate() {
        let next = arrays.get(index + 1).map_or("start".to_owned(), |(after, _, _)| format!("fill_{after}_0"));
        text += &filled(name, element, *bytes, &from, &next);
        from = next;
    }
    text + body + "}\n"
}

/// `for (i = 0; i < n; i++) s += (long)a[i] * b[i]`, as `examples/dot`.
const DOT: &str = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2.back ]
  %s = phi i32 [ 0, %start ], [ %s.next, %l2.back ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %o = mul nsw i16 %i, 2
  %pa = getelementptr inbounds i8, ptr @a, i16 %o
  %va = load i16, ptr %pa
  %wa = sext i16 %va to i32
  %pb = getelementptr inbounds i8, ptr @b, i16 %o
  %vb = load i16, ptr %pb
  %wb = sext i16 %vb to i32
  %m = mul nsw i32 %wa, %wb
  %s.next = add nsw i32 %s, %m
  %i.next = add nsw i16 %i, 1
  br label %l2.back

l2.back:
  br label %l1

l3:
  ret i32 %s
";

/// A dot product over two word arrays and a symbolic count keeps one
/// counter, counted to zero: no multiply or compare with `n` in the loop.
#[test]
fn test_a_dot_product_keeps_one_counter_counted_to_zero() {
    let text = program(&[("a", "i16", 2), ("b", "i16", 2)], "i32", DOT).replace("br label %l2.back\n\nl2.back:\n  br label %l1", "br label %l1").replace("%l2.back ]", "%l2 ]");
    let printed = same(&text, TRIPS);
    assert_eq!(counters(&printed), 1, "{printed}");
    assert!(printed.contains(", 0\n") && !printed.lines().any(|line| line.contains("icmp") && line.contains("%n") && !line.contains("sle") && !line.contains("sgt")), "{printed}");
}

/// The dot product as the pass finds it after the loop passes: a latch of
/// its own. The shape it is written for is present.
#[test]
fn test_the_dot_product_fixture_has_its_shape() {
    let text = program(&[("a", "i16", 2), ("b", "i16", 2)], "i32", DOT);
    assert!(text.contains("%o = mul nsw i16 %i, 2") && text.contains("icmp slt i16 %i, %n"), "{text}");
    let printed = same(&text, TRIPS);
    assert!(readers(&printed, "%n").iter().all(|line| !line.contains("icmp slt i16 %i")), "{printed}");
}

/// Arrays of bytes, words and dwords, one index: each realized from the
/// counters chosen, which are no more than the strides.
#[test]
fn test_arrays_of_three_element_sizes_share_counters() {
    let body = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i32 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %pc = getelementptr i8, ptr @c, i16 %i
  %vc = load i8, ptr %pc
  %wc = sext i8 %vc to i32
  %o2 = shl i16 %i, 1
  %pa = getelementptr i8, ptr @a, i16 %o2
  %va = load i16, ptr %pa
  %wa = sext i16 %va to i32
  %o4 = mul i16 %i, 4
  %pl = getelementptr i8, ptr @l, i16 %o4
  %wl = load i32, ptr %pl
  %t = add i32 %wc, %wa
  %u = add i32 %t, %wl
  %s.next = add i32 %s, %u
  %i.next = add i16 %i, 1
  br label %l1

l3:
  ret i32 %s
";
    let printed = same(&program(&[("c", "i8", 1), ("a", "i16", 2), ("l", "i32", 4)], "i32", body), TRIPS);
    assert!(counters(&printed) <= 3, "{printed}");
}

/// A counter stepping by two, and one counting down by one.
#[test]
fn test_steps_of_two_and_minus_one() {
    let up = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr @a, i16 %o
  %v = load i16, ptr %p
  %s.next = add i16 %s, %v
  %i.next = add nsw i16 %i, 2
  br label %l1

l3:
  %r = add i16 %s, %i
  ret i16 %r
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", up), TRIPS);
    // No symbolic count for a step of two: its exit keeps the counter.
    assert!(counters(&printed) <= 2, "{printed}");
    let down = "  %top = sub i16 %n, 1
  br label %l1

l1:
  %i = phi i16 [ %top, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp sge i16 %i, 0
  br i1 %more, label %l2, label %l3

l2:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr @a, i16 %o
  %v = load i16, ptr %p
  %w = mul i16 %v, %i
  %s.next = add i16 %s, %w
  %i.next = add nsw i16 %i, -1
  br label %l1

l3:
  ret i16 %s
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", down), &[&[-3, 5], &[0, 5], &[1, 5], &[2, -9], &[7, 3], &[33, 11]]);
    assert!(counters(&printed) <= 2, "{printed}");
}

/// Two loops read one array; the second from one place further.
#[test]
fn test_two_loops_share_an_array() {
    let body = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %m0

l2:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr @a, i16 %o
  %v = load i16, ptr %p
  %s.next = add i16 %s, %v
  %i.next = add i16 %i, 1
  br label %l1

m0:
  %s.out = phi i16 [ %s, %l1 ]
  br label %m1

m1:
  %j = phi i16 [ 1, %m0 ], [ %j.next, %m2 ]
  %t = phi i16 [ %s.out, %m0 ], [ %t.next, %m2 ]
  %again = icmp slt i16 %j, %n
  br i1 %again, label %m2, label %m3

m2:
  %q = mul i16 %j, 2
  %r = getelementptr i8, ptr @a, i16 %q
  %w = load i16, ptr %r
  %before = add i16 %q, -2
  %r0 = getelementptr i8, ptr @a, i16 %before
  %w0 = load i16, ptr %r0
  %d = sub i16 %w, %w0
  %t.next = add i16 %t, %d
  %j.next = add i16 %j, 1
  br label %m1

m3:
  ret i16 %t
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", body), TRIPS);
    assert!(counters(&printed) <= 2, "{printed}");
}

/// A dword index into a word address: the address wraps as the pointer's
/// index does.
#[test]
fn test_a_dword_index() {
    let body = "  %wide = sext i16 %n to i32
  br label %l1

l1:
  %i = phi i32 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i32 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i32 %i, %wide
  br i1 %more, label %l2, label %l3

l2:
  %o = mul i32 %i, 2
  %p = getelementptr i8, ptr @a, i32 %o
  %v = load i16, ptr %p
  %x = sext i16 %v to i32
  %y = mul i32 %x, %i
  %s.next = add i32 %s, %y
  %i.next = add i32 %i, 1
  br label %l1

l3:
  ret i32 %s
";
    same(&program(&[("a", "i16", 2)], "i32", body), TRIPS);
}

/// Walked by a pointer to an end pointer: the same count of counters as
/// the indexed loop.
#[test]
fn test_a_walking_pointer_keeps_one_counter() {
    let body = "  %bytes = mul i16 %n, 2
  %end = getelementptr i8, ptr @a, i16 %bytes
  %none = icmp sle i16 %n, 0
  br i1 %none, label %l3, label %l1

l1:
  %p = phi ptr [ @a, %start ], [ %p.next, %l1 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l1 ]
  %v = load i16, ptr %p
  %s.next = add i16 %s, %v
  %p.next = getelementptr i8, ptr %p, i16 2
  %more = icmp ne ptr %p.next, %end
  br i1 %more, label %l1, label %l2

l2:
  %s.out = phi i16 [ %s.next, %l1 ]
  br label %l3

l3:
  %r = phi i16 [ 0, %start ], [ %s.out, %l2 ]
  ret i16 %r
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", body), TRIPS);
    assert_eq!(counters(&printed), 1, "{printed}");
}

/// A multiply by an invariant leaves the loop, and a counter read after
/// the loop is its value as it left.
#[test]
fn test_an_invariant_multiple_and_the_value_after_the_loop() {
    let body = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %row = mul i16 %i, %k
  %s.next = add i16 %s, %row
  %i.next = add i16 %i, 1
  br label %l1

l3:
  %i.out = phi i16 [ %i, %l1 ]
  %r = mul i16 %s, %i.out
  ret i16 %r
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", body), TRIPS);
    let loop_ = &printed[printed.find("l2:").unwrap_or(0)..];
    assert!(!loop_.lines().take_while(|line| !line.starts_with("l3")).any(|line| line.contains("mul i16") && line.contains("%k")), "{printed}");
}


// Count-to-zero, the exit test replaced and the counters shared, as the
// passes `lsr` replaced were tested: each shape now `lsr`'s.

/// Nested loops of six trips, each with a counter and a scaled offset, the
/// inner body reached only where the counters differ.
fn nested(outer_stride: i64, inner_stride: i64, test: &str) -> String {
    format!(
        "@sum = global i16 0

define i16 @f(i16 %x) {{
b0:
  store i16 %x, ptr @sum
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %onext, %b6 ]
  %oo = phi i16 [ 0, %b0 ], [ %oonext, %b6 ]
  %ogo = icmp slt i16 %o, 6
  br i1 %ogo, label %b2, label %b9

b2:
  br label %b3

b3:
  %in = phi i16 [ 0, %b2 ], [ %innext, %b5 ]
  %io = phi i16 [ 0, %b2 ], [ %ionext, %b5 ]
  %igo = icmp slt i16 %in, 6
  br i1 %igo, label %b4, label %b6

b4:
  %same = icmp {test} i16 %in, %o
  br i1 %same, label %b5, label %b7

b7:
  %s = load i16, ptr @sum
  %t = add i16 %s, %io
  store i16 %t, ptr @sum
  br label %b5

b5:
  %innext = add i16 %in, 1
  %ionext = add i16 %io, {inner_stride}
  br label %b3

b6:
  %onext = add i16 %o, 1
  %oonext = add i16 %oo, {outer_stride}
  br label %b1

b9:
  %r = load i16, ptr @sum
  ret i16 %r
}}
"
    )
}

/// C nbody carried `other` beside `other*4` only for `other != body`.
#[test]
fn test_nested_counters_compared_for_equality_keep_their_values() {
    for (outer, inner, test) in [(4, 4, "eq"), (5, 4, "eq"), (4, 4, "slt"), (32768, 32768, "eq")] {
        same(&nested(outer, inner, test), &[&[0], &[3]]);
    }
}

/// Mandelbrot kept 16-bit `px` beside the 32-bit `cx`; an 8-bit coordinate
/// stepping by 64 repeats in four trips.
#[test]
fn test_a_wider_recurrence_beside_the_counter() {
    for (width, stride) in [(32, 24), (8, 64)] {
        let text = format!(
            "@seen = global i{width} 0

define i16 @f(i{width} %x) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %c = phi i{width} [ %x, %b0 ], [ %cnext, %b2 ]
  %go = icmp slt i16 %i, 4
  br i1 %go, label %b2, label %b9

b2:
  store i{width} %c, ptr @seen
  %inext = add i16 %i, 1
  %cnext = add i{width} %c, {stride}
  br label %b1

b9:
  %r = add i16 %i, 1
  ret i16 %r
}}
"
        );
        same(&text, &[&[0], &[-7], &[100]]);
    }
}

/// A dynamic counted loop with a second recurrence from `start`, read
/// plus 100 and folded into `%acc`.
fn symbolic(start: i64) -> String {
    format!(
        "define i16 @f(i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %c = phi i16 [ {start}, %b0 ], [ %cnext, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %b2 ]
  %go = icmp ult i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %off = add i16 %c, 100
  %twice = shl i16 %acc, 1
  %sum = xor i16 %twice, %off
  %inext = add i16 %i, 1
  %cnext = add i16 %c, 1
  br label %b1

b3:
  ret i16 %acc
}}
"
    )
}

const COUNTS: &[&[i128]] = &[&[0], &[1], &[7], &[300]];

/// A counter and a recurrence from `start`: the loop is counted to zero in
/// one block behind a guard where it runs no trip.
#[test]
fn test_a_symbolic_loop_counts_to_zero_behind_a_guard() {
    for start in [5, 0] {
        let printed = same(&symbolic(start), COUNTS);
        assert!(counters(&printed) <= 2, "{printed}");
        assert!(printed.lines().any(|line| line.contains("icmp ne i16") && line.ends_with(", 0")), "{printed}");
        let entry = printed.lines().skip_while(|line| !line.starts_with("b0:")).nth(1).into_iter().chain(printed.lines().skip_while(|line| !line.starts_with("b0:")).skip(1).take_while(|line| !line.is_empty())).collect::<Vec<_>>();
        assert!(entry.iter().any(|line| line.contains("br i1")), "a guard:\n{printed}");
    }
}

/// Unknown trips, and a recurrence read after the loop: its value where
/// the guard skipped the loop is its start.
#[test]
fn test_a_recurrence_read_after_a_guarded_loop_keeps_its_values() {
    same(&symbolic(0).replace("%off = add i16 %c, 100", "%off = add i16 %n, 100"), COUNTS);
    same(&symbolic(5).replace("  ret i16 %acc", "  %after = add i16 %acc, %c\n  ret i16 %after").replace("%off = add i16 %c, 100", "%off = add i16 %n, 100"), COUNTS);
}

/// A counter indexing memory, stored or not, masked or not, or less an
/// invariant, keeps what the loop computes.
#[test]
fn test_a_counter_indexing_memory() {
    let text = "@table = global [16 x i16] zeroinitializer

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %b2 ]
  %go = icmp ult i16 %i, 9
  br i1 %go, label %b2, label %b3

b2:
  %at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %i
  store i16 %i, ptr %at
  %got = load i16, ptr %at
  %sum = add i16 %acc, %got
  %inext = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}
";
    same(text, &[&[0]]);
    let indexed = text.replace("  store i16 %i, ptr %at\n", "");
    same(&indexed, &[&[0]]);
    same(&indexed.replace("%at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %i", "%low = and i16 %i, 3\n  %at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %low"), &[&[0]]);
    let text = "@table = global [16 x i16] zeroinitializer

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b2 ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %b2 ]
  %go = icmp ult i16 %i, 9
  br i1 %go, label %b2, label %b3

b2:
  %k = sub i16 %i, -2
  %at = getelementptr inbounds [16 x i16], ptr @table, i16 0, i16 %k
  %got = load i16, ptr %at
  %sum = add i16 %acc, %got
  %inext = add i16 %i, 1
  br label %b1

b3:
  ret i16 %acc
}
";
    same(text, &[&[0]]);
}

/// Nothing is chosen twice: a loop `lsr` has counted is left alone, by it
/// and by `Rotate`.
#[test]
fn test_a_chosen_loop_is_settled() {
    let (_, once) = reduced(&symbolic(5));
    let (_, twice) = reduced(&once.replace(DOS, ""));
    assert_eq!(twice, once);
    let mut module = parsed(&once);
    assert_eq!(crate::testing::managed(&mut module, crate::rotate::Rotate), once);
}

/// A loop whose body is two blocks.
#[test]
fn test_a_body_of_two_blocks() {
    let text = "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %sum, %b3 ]
  %go = icmp slt i16 %i, 10
  br i1 %go, label %b2, label %b4

b2:
  %k = add i16 %i, 7
  br label %b3

b3:
  %sum = xor i16 %acc, %k
  %inext = add i16 %i, 1
  br label %b1

b4:
  %r = add i16 %acc, %i
  ret i16 %r
}
";
    same(text, &[&[0], &[5], &[-3]]);
}

/// A counter the loop stores beside a pointer it steps: the loop ends on
/// its own compare until the pointer's byte offset may count to zero.
fn pointer_beside_a_stored_counter() -> String {
    format!(
        "@a = internal global [8200 x i8] zeroinitializer

define i16 @f(i16 %x) {{
b0:
  br label %b1

b1:
  %v = phi i16 [ 1, %b0 ], [ %vnext, %b2 ]
  %p = phi ptr [ @a, %b0 ], [ %pnext, %b2 ]
  %go = icmp ne i16 %v, 81
  br i1 %go, label %b2, label %b3

b2:
  store i16 %v, ptr %p
  %pnext = getelementptr i8, ptr %p, i16 100
  %vnext = add i16 %v, 1
  br label %b1

b3:
  %at = getelementptr i8, ptr @a, i16 500
  %got = load i16, ptr %at
  ret i16 %got
}}
"
    )
}

/// A counter stored beside a pointer it steps, read after the loop, read
/// in it, or only after it: what the loop computes is kept.
#[test]
fn test_a_pointer_beside_a_stored_counter() {
    let text = pointer_beside_a_stored_counter();
    same(&text, &[&[0]]);
    same(&text.replace("  ret i16 %got", "  %r = add i16 %got, %v\n  ret i16 %r"), &[&[0]]);
    same(&text.replace("  store i16 %v, ptr %p", "  %w = add i16 %v, 7\n  store i16 %w, ptr %p").replace("  ret i16 %got", "  %r = add i16 %got, %v\n  ret i16 %r"), &[&[0]]);
    let printed = same(&text.replace("  store i16 %v, ptr %p", "  store i16 7, ptr %p").replace("  ret i16 %got", "  %r = add i16 %got, %v\n  ret i16 %r"), &[&[0]]);
    assert_eq!(counters(&printed), 1, "{printed}");
}

/// The whole pipeline over a counter stored beside a pointer settles, and
/// the loop keeps one counter where the pointer's offset counts to zero.
/// Strength and count-to-zero once cycled on it until the pipeline gave up.
#[test]
fn test_the_pipeline_settles_a_pointer_beside_a_stored_counter() {
    let mut module = parsed(&format!("{DOS}{}", pointer_beside_a_stored_counter()));
    let before = results(&module, &[&[0]]);
    llrm_mir::program::Program::lend(&mut module, Rc::new(llrm_x86_m16::Dos::default()), |program| crate::pipeline::applied(program, &crate::pipeline::Applied::default())).and_then(|done| done).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, &[&[0]]), before, "{after}");
}

/// A word counter indexing a dword array.
#[test]
fn test_a_word_counter_indexing_a_dword_array() {
    let text = "define i32 @f(i32 %n) {
entry:
  %a = alloca [16 x i32]
  br label %body
body:
  %i = phi i16 [ 0, %entry ], [ %j, %body ]
  %s = phi i32 [ 0, %entry ], [ %t, %body ]
  %m = mul nsw i16 %i, 4
  %p = getelementptr inbounds i8, ptr %a, i16 %m
  %x = zext i16 %i to i32
  %y = add i32 %x, %n
  store i32 %y, ptr %p
  %v = load i32, ptr %p
  %t = add i32 %s, %v
  %j = add i16 %i, 1
  %c = icmp ult i16 %j, 16
  br i1 %c, label %body, label %done
done:
  ret i32 %t
}
";
    same(text, &[&[5], &[-3]]);
}

/// `%acc` tripled `%n` times, the counter read by nothing else, and the
/// exit reading it through `leave` (`""` or an LCSSA phi).
fn dead_counter(test: &str, leave: &str, result: &str) -> String {
    format!(
        "define i16 @f(i16 %x, i16 %n) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %sum, %b2 ]
  %go = icmp {test} i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %sum = mul i16 %acc, 3
  %next = add i16 %i, 1
  br label %b1

b3:
{leave}  ret i16 {result}
}}
"
    )
}

const DEAD: &[&[i128]] = &[&[3, 0], &[3, 1], &[2, 7], &[-4, 300], &[5, 9]];

/// C floats retained `add/cmp/jb` in its ten-trip hot path; Clang tests the
/// count once before the loop, then ends every trip on the step. A dead
/// counter of unknown trips counts to zero in one block behind a guard,
/// and one leaving through an LCSSA phi leaves as its value.
#[test]
fn test_a_dead_counter_of_unknown_trips_counts_to_zero_behind_a_guard() {
    for test in ["slt", "ult", "ne"] {
        let printed = same(&dead_counter(test, "", "%acc"), DEAD);
        assert!(printed.lines().any(|line| line.contains("icmp ne i16") && line.ends_with(", 0")), "{test}:\n{printed}");
    }
    same(&dead_counter("slt", "  %e = phi i16 [ %i, %b1 ]\n  %r = add i16 %e, %acc\n", "%r"), DEAD);
    same(&format!("@g = global i16 0\n\n{}", dead_counter("slt", "", "%acc").replace("  %next = add", "  store i16 %i, ptr @g\n  %next = add")), DEAD);
}

/// TEXTFILL's `POKE o, ch + (o AND 15)` over `o` from 0 to 3998 by two:
/// the low bits of a counter counted to zero from -4000 are its own, so it
/// needs no add, and the loop keeps one counter. It kept two.
#[test]
fn test_a_masked_counter_counts_to_zero_without_an_add() {
    let text = "@screen = global [4000 x i8] zeroinitializer

define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %o.next, %b2 ]
  %go = icmp sle i16 %o, 3998
  br i1 %go, label %b2, label %b3

b2:
  %low = and i16 %o, 15
  %v = trunc i16 %low to i8
  %p = getelementptr i8, ptr @screen, i16 %o
  store i8 %v, ptr %p
  %o.next = add nsw i16 %o, 2
  br label %b1

b3:
  %q = getelementptr i8, ptr @screen, i16 34
  %r = load i8, ptr %q
  %w = zext i8 %r to i16
  ret i16 %w
}
";
    assert!(text.contains("and i16 %o, 15"));
    let printed = same(text, &[&[0]]);
    assert_eq!(counters(&printed), 1, "{printed}");
    assert!(printed.lines().any(|line| line.contains("icmp ne i16") && line.ends_with(", 0")), "{printed}");
}

/// TILE stored `Y% = 64` after its loop; the value read as a loop of known
/// count leaves is its start plus its steps, and needs no counter.
#[test]
fn test_a_value_read_after_a_known_count_is_a_constant() {
    let body = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, 20
  br i1 %more, label %l2, label %l3

l2:
  %o = mul i16 %i, 2
  %p = getelementptr i8, ptr @a, i16 %o
  %v = load i16, ptr %p
  %s.next = add i16 %s, %v
  %i.next = add i16 %i, 1
  br label %l1

l3:
  %i.out = phi i16 [ %i, %l1 ]
  %r = add i16 %s, %i.out
  ret i16 %r
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", body), TRIPS);
    assert_eq!(counters(&printed), 1, "{printed}");
    assert!(printed.contains("add i16 %s1, 20") || printed.contains(", 20\n") || printed.lines().any(|line| line.contains("%r = add") && line.contains("20")), "{printed}");
}

/// nbody's unrolled `IF other <> body`: an equality of the counter with a
/// constant takes the counter counted to zero, the constant moved by its bias.
#[test]
fn test_an_equality_takes_the_bias_into_its_constant() {
    let body = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l4 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l4 ]
  %more = icmp slt i16 %i, 6
  br i1 %more, label %l2, label %l5

l2:
  %same = icmp ne i16 %i, 3
  br i1 %same, label %l3, label %l4

l3:
  %p = getelementptr i8, ptr @a, i16 %i
  %b = load i8, ptr %p
  %v = zext i8 %b to i16
  br label %l4

l4:
  %w = phi i16 [ %v, %l3 ], [ 0, %l2 ]
  %s.next = add i16 %s, %w
  %i.next = add i16 %i, 1
  br label %l1

l5:
  ret i16 %s
";
    let printed = same(&program(&[("a", "i8", 1)], "i16", body), TRIPS);
    assert_eq!(counters(&printed), 1, "{printed}");
    assert!(!printed.contains("icmp ne i16 %i, 3"), "{printed}");
}

/// MATRIX: an inner loop's old addresses, dead once it is rewritten, still
/// read the outer counter's multiple, and the outer loop kept a second
/// counter for them.
#[test]
fn test_what_an_inner_loop_leaves_dead_is_no_use_of_the_outer() {
    let body = "  br label %r1

r1:
  %r = phi i16 [ 0, %start ], [ %r.next, %r3 ]
  %rgo = icmp slt i16 %r, 4
  br i1 %rgo, label %c0, label %r4

c0:
  %row = mul i16 %r, 8
  br label %c1

c1:
  %c = phi i16 [ 0, %c0 ], [ %c.next, %c2 ]
  %cgo = icmp slt i16 %c, 8
  br i1 %cgo, label %c2, label %r3

c2:
  %at = add i16 %row, %c
  %o = mul i16 %at, 2
  %p = getelementptr i8, ptr @a, i16 %o
  %v = add i16 %r, %c
  store i16 %v, ptr %p
  %c.next = add i16 %c, 1
  br label %c1

r3:
  %r.next = add i16 %r, 1
  br label %r1

r4:
  %q = getelementptr i8, ptr @a, i16 22
  %got = load i16, ptr %q
  ret i16 %got
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", body), TRIPS);
    assert!(counters(&printed) <= 4, "{printed}");
}

/// A pointer walked beside an index that ends the loop: one counter, as
/// the indexed loop. The pointer was no recurrence the pass could share.
#[test]
fn test_a_pointer_walked_beside_an_index_shares_one_counter() {
    let body = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %p = phi ptr [ @a, %start ], [ %p.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, 30
  br i1 %more, label %l2, label %l3

l2:
  %v = load i16, ptr %p
  %s.next = add i16 %s, %v
  %p.next = getelementptr i8, ptr %p, i16 2
  %i.next = add i16 %i, 1
  br label %l1

l3:
  ret i16 %s
";
    let printed = same(&program(&[("a", "i16", 2)], "i16", body), TRIPS);
    assert_eq!(counters(&printed), 1, "{printed}");
}

/// `bench/mandel/mandel.c` as the pass meets it, over two rows: `cx`, the
/// column's `xOffset - 512 + 24 * px`, read across the inner loop.
const MANDEL: &str = "define i32 @f(i16 %0) {
b1:
  %1 = sext i16 %0 to i32
  br label %b2

b2:
  %2 = phi i16 [ -1, %b1 ], [ %12, %b6 ]
  %3 = phi i32 [ 0, %b1 ], [ %11, %b6 ]
  %4 = icmp slt i16 %2, 1
  br i1 %4, label %b4, label %b3

b3:
  %5 = phi i32 [ %3, %b2 ]
  ret i32 %5

b4:
  %6 = sext i16 %2 to i32
  %7 = mul nsw i32 %6, 24
  br label %b5

b5:
  %8 = phi i16 [ -16, %b4 ], [ %24, %b9 ]
  %9 = phi i32 [ %3, %b4 ], [ %23, %b9 ]
  %10 = icmp slt i16 %8, 16
  br i1 %10, label %b7, label %b6

b6:
  %11 = phi i32 [ %9, %b5 ]
  %12 = add nsw i16 %2, 1
  br label %b2

b7:
  %13 = sext i16 %8 to i32
  %14 = mul nsw i32 %13, 24
  %15 = sub nsw i32 %14, 128
  %16 = add nsw i32 %15, %1
  br label %b8

b8:
  %17 = phi i16 [ 0, %b7 ], [ %36, %b11 ]
  %18 = phi i32 [ 0, %b7 ], [ %33, %b11 ]
  %19 = phi i32 [ 0, %b7 ], [ %35, %b11 ]
  %20 = icmp slt i16 %17, 32
  br i1 %20, label %b10, label %b13

b9:
  %21 = phi i16 [ %37, %b12 ], [ %38, %b13 ]
  %22 = zext i16 %21 to i32
  %23 = add i32 %9, %22
  %24 = add nsw i16 %8, 1
  br label %b5

b10:
  %25 = mul nsw i32 %19, %19
  %26 = ashr i32 %25, 8
  %27 = mul nsw i32 %18, %18
  %28 = ashr i32 %27, 8
  %29 = add nsw i32 %26, %28
  %30 = icmp sgt i32 %29, 1024
  br i1 %30, label %b12, label %b11

b11:
  %31 = mul nsw i32 %19, %18
  %32 = ashr i32 %31, 7
  %33 = add nsw i32 %32, %7
  %34 = sub nsw i32 %26, %28
  %35 = add nsw i32 %34, %16
  %36 = add nsw i16 %17, 1
  br label %b8

b12:
  %37 = phi i16 [ %17, %b10 ]
  br label %b9

b13:
  %38 = phi i16 [ %17, %b8 ]
  br label %b9
}
";

/// A use rebuilt from a counter is a register while it lives: `cx` built
/// each column beside a constant counter held two across the inner loop,
/// and mandel ran 4% more instructions than one counter that is `cx`.
#[test]
fn test_a_value_read_across_an_inner_loop_is_its_own_counter() {
    let printed = same(MANDEL, &[&[0], &[5], &[-7]]);
    let mut block = "";
    let mut per_column = Vec::new();
    for line in printed.lines() {
        if let Some(label) = line.strip_suffix(':') {
            block = label;
        } else if block == "b7" && line.contains("%1") {
            per_column.push(line);
        }
    }
    assert!(per_column.is_empty(), "{printed}");
}

/// `tests/run/qb/addrm.bas`: a word array read at `i` and `i + 1`, and a
/// dword one stored `i` at `i`.
const ADDRM: &str = "  br label %l1

l1:
  %i = phi i16 [ 1, %start ], [ %i.next, %l2 ]
  %t = phi i16 [ 0, %start ], [ %t.next, %l2 ]
  %more = icmp sle i16 %i, 20
  br i1 %more, label %l2, label %l3

l2:
  %pa = getelementptr inbounds i16, ptr @a, i16 %i
  store i16 %i, ptr %pa
  %i1 = add i16 %i, 1
  %pa1 = getelementptr inbounds i16, ptr @a, i16 %i1
  %va = load i16, ptr %pa1
  %s = add i16 %i, %va
  %t.next = add i16 %t, %s
  %w = sext i16 %i to i32
  %pb = getelementptr inbounds i32, ptr @b, i16 %i
  store i32 %w, ptr %pb
  %i.next = add nsw i16 %i, 1
  br label %l1

l3:
  %q = getelementptr i8, ptr @b, i16 80
  %vb = load i32, ptr %q
  %nb = trunc i32 %vb to i16
  %r = add i16 %t, %nb
  ret i16 %r
";

/// An address after its counter's step reads the stepped value: addrm's
/// `B&[ecx*4]` read the counter beside its successor, and the two lived
/// in two registers, a copy a trip.
#[test]
fn test_an_address_after_the_step_reads_the_stepped_counter() {
    let printed = same(&program(&[("a", "i16", 2), ("b", "i32", 4)], "i16", ADDRM), TRIPS);
    let reads = |line: &str, name: &str| line.contains(&format!("{name},")) || line.ends_with(name);
    // Each new counter, what is computed from it, and whether it has stepped.
    let mut counters = printed
        .lines()
        .filter(|line| line.contains("%lsr.iv") && line.contains("= phi"))
        .map(|line| (vec![line.split_once(" = ").expect("a phi").0.trim().to_owned()], false))
        .collect::<Vec<_>>();
    assert!(!counters.is_empty(), "{printed}");
    for line in printed.lines().filter(|line| !line.contains("= phi")) {
        let defined = line.split_once(" = ").map(|(name, _)| name.trim().to_owned());
        for (from, stepped) in &mut counters {
            if from.iter().any(|one| reads(line, one)) {
                assert!(!*stepped, "a counter read after its step: {line}\n{printed}");
                if defined.as_deref().is_some_and(|name| name.starts_with("%lsr.iv.next")) {
                    *stepped = true;
                } else {
                    from.extend(defined.clone());
                }
            }
        }
    }
}

/// examples/ticker.nib's spin loop: `spins`, stepped in the body where the
/// loop continues either way, is printed when a tick has passed.
const TICKER: &str = "  br label %l1

l1:
  %s = phi i32 [ 0, %start ], [ %s1, %l4 ]
  %m = phi i16 [ 0, %start ], [ %m.next, %l4 ]
  %more = icmp ult i16 %m, 36
  br i1 %more, label %l2, label %l5

l2:
  %s1 = add i32 %s, 1
  %low = and i16 %m, 3
  %c = icmp eq i16 %low, 0
  br i1 %c, label %l3, label %l4

l3:
  call void @shown(i32 %s1, i16 %m)
  br label %l4

l4:
  %m.next = add i16 %m, 1
  br label %l1

l5:
  %q = load i32, ptr @b
  %r = trunc i32 %q to i16
  ret i16 %r
";

/// A counter's own step, read in the body, is its next value wherever the
/// step stands: ticker's `spins` was priced an add there and a register
/// besides, and was replaced by a counter from one, a store more.
#[test]
fn test_a_counter_read_at_its_own_step_is_kept() {
    let callee = "define void @shown(i32 %s, i16 %k) {
entry:
  %p = getelementptr i8, ptr @b, i16 0
  store i32 %s, ptr %p
  ret void
}
";
    let printed = same(&format!("{callee}{}", program(&[("b", "i32", 4)], "i16", TICKER)), TRIPS);
    assert!(!printed.lines().any(|line| line.contains("%lsr.iv") && line.contains("phi i32")), "{printed}");
}

/// Past the target's registers an end compared with is spilled for a load,
/// a counter for a memory update a trip: mandel's column kept a second
/// counter counted to zero in memory over comparing `cx` with its end,
/// where every register over the target's was priced as a counter.
#[test]
fn test_a_spilled_end_is_cheaper_than_a_second_counter() {
    let printed = same(MANDEL, &[&[0], &[5], &[-7]]);
    assert_eq!(counters(&printed), 3, "{printed}");
}

/// `s += a[3 * i + 8]` over words, `n` trips: a byte stride of six.
const STRIDE_SIX: &str = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %t = mul nsw i16 %i, 3
  %x = add nsw i16 %t, 8
  %p = getelementptr inbounds i16, ptr @a, i16 %x
  %v = load i16, ptr %p
  %s.next = add i16 %s, %v
  %i.next = add nsw i16 %i, 1
  br label %l1

l3:
  ret i16 %s
";

/// A multiply by a constant costs what the target makes of it: six times
/// `n`, a lea and an add, was priced as a multiply, and loop-corpus's
/// `a[3*i]` kept a second counter to zero beside its stride of six.
#[test]
fn test_a_stride_counted_to_zero_takes_its_start_at_the_target_s_multiply() {
    let text = program(&[("a", "i16", 2)], "i16", STRIDE_SIX);
    let before = parsed(&format!("{DOS}{text}"));
    let machine = Tuned { multiplies: std::collections::BTreeMap::from([(6, 3)]), ..target() };
    let (_, printed) = reduced_for(&text, machine);
    let trips: &[&[i128]] = &[&[-3, 5], &[0, 5], &[1, 5], &[2, -9], &[7, 3], &[18, 11]];
    assert_eq!(results(&parsed(&printed), trips), results(&before, trips), "{printed}");
    assert_eq!(counters(&printed), 1, "{printed}");
}

/// `while (a[i] != 0) i++` and the count `i` after it, over words: no
/// counted exit, so no bound on the trips but the access's own.
const WORDLEN: &str = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %p = getelementptr inbounds i16, ptr @a, i16 %i
  %v = load i16, ptr %p
  %go = icmp ne i16 %v, 0
  br i1 %go, label %l2, label %l3

l2:
  %i.next = add nsw i16 %i, 1
  br label %l1

l3:
  ret i16 %i
";

/// A count read after a loop of no counted exit is its address counter
/// halved, where an in-bounds access bounds the trips: `wordlen` kept `i`
/// beside the byte offset, two counters to loop-corpus's one.
#[test]
fn test_a_count_after_a_loop_of_no_counted_exit_shares_its_address_counter() {
    let printed = same(&program(&[("a", "i16", 2)], "i16", WORDLEN), &[&[0, 0], &[0, 7], &[0, 14], &[0, 21]]);
    assert_eq!(counters(&printed), 1, "{printed}");
    assert!(!printed.contains(", 0\n") || !printed.contains("sub i16 %lsr"), "a distance from a start of zero is the counter: {printed}");
}

/// Three word arrays summed to a symbolic `n`: a global is a displacement,
/// so one count-to-zero counter over `2n` held once serves every array.
/// Each array's own pointer was priced as a held register, twelve of them
/// in loop-corpus's conc12, and two counters were kept to avoid them.
#[test]
fn test_arrays_at_displacements_share_one_counter_and_one_base() {
    let body = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %t, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %pa = getelementptr inbounds i16, ptr @a, i16 %i
  %pb = getelementptr inbounds i16, ptr @b, i16 %i
  %pc = getelementptr inbounds i16, ptr @c, i16 %i
  %va = load i16, ptr %pa
  %vb = load i16, ptr %pb
  %vc = load i16, ptr %pc
  %u = add i16 %s, %va
  %w = add i16 %u, %vb
  %t = add i16 %w, %vc
  %i.next = add nsw i16 %i, 1
  br label %l1

l3:
  ret i16 %s
";
    let printed = same(&program(&[("a", "i16", 2), ("b", "i16", 2), ("c", "i16", 2)], "i16", body), &[&[-3, 5], &[0, 5], &[1, 5], &[7, 3], &[30, 11]]);
    assert_eq!(counters(&printed), 1, "{printed}");
}

/// Three local word arrays, filled, then `a[i + 8]` of each summed into a
/// long to a symbolic `n`: conc3's local arrays.
const FRAME_SUM: &str = "define i32 @f(i16 %n, i16 %k) {
start:
  %fa = alloca [600 x i8]
  %fb = alloca [600 x i8]
  %fc = alloca [600 x i8]
  br label %fill

fill:
  %g = phi i16 [ 0, %start ], [ %g.next, %fill ]
  %go = mul nsw i16 %g, 2
  %qa = getelementptr inbounds i8, ptr %fa, i16 %go
  store i16 %g, ptr %qa
  %qb = getelementptr inbounds i8, ptr %fb, i16 %go
  store i16 %g, ptr %qb
  %qc = getelementptr inbounds i8, ptr %fc, i16 %go
  store i16 %g, ptr %qc
  %g.next = add nsw i16 %g, 1
  %gc = icmp slt i16 %g.next, 60
  br i1 %gc, label %fill, label %l0

l0:
  br label %l1

l1:
  %i = phi i16 [ 0, %l0 ], [ %i.next, %l2 ]
  %s = phi i32 [ 0, %l0 ], [ %t, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %x = add nsw i16 %i, 8
  %o = mul nsw i16 %x, 2
  %pa = getelementptr inbounds i8, ptr %fa, i16 %o
  %va = load i16, ptr %pa
  %xa = sext i16 %va to i32
  %u = add nsw i32 %s, %xa
  %pb = getelementptr inbounds i8, ptr %fb, i16 %o
  %vb = load i16, ptr %pb
  %xb = sext i16 %vb to i32
  %w = add nsw i32 %u, %xb
  %pc = getelementptr inbounds i8, ptr %fc, i16 %o
  %vc = load i16, ptr %pc
  %xc = sext i16 %vc to i32
  %t = add nsw i32 %w, %xc
  %i.next = add nsw i16 %i, 1
  br label %l1

l3:
  %r = add nsw i32 %s, 1
  ret i32 %r
}
";

/// Frame arrays are not displacements: BP is their base and leaves one
/// register for what they add, so each array's `array + 2n` is a register
/// of its own, and priced so. Read as one shared base, conc3's three local
/// arrays kept one counter and spilled three pointers reloaded each trip.
#[test]
fn test_frame_arrays_keep_their_own_pointers() {
    let before = parsed(&format!("{DOS}{FRAME_SUM}"));
    let mut after = before.clone();
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Lsr);
    manager.run_module(&mut after, Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    let printed = printed(&after);
    let inputs: &[&[i128]] = &[&[-3, 5], &[0, 5], &[1, 5], &[7, 3], &[30, 11]];
    assert_eq!(results(&parsed(&printed), inputs), results(&before, inputs), "{printed}");
    // The fill's two, and the loop's two: an offset and a count to zero.
    assert_eq!(counters(&printed), 4, "{printed}");
}

/// `if (n > 0) do { a[i]++; s += b[i]; } while (++i < n)`: no runtime
/// guard, since the branch over the loop proves its first trip.
const GUARDED_DO: &str = "  %ok = icmp sgt i16 %n, 0
  br i1 %ok, label %pre, label %done

pre:
  br label %l1

l1:
  %i = phi i16 [ 0, %pre ], [ %i.next, %l1 ]
  %s = phi i16 [ 0, %pre ], [ %t, %l1 ]
  %o = mul nsw i16 %i, 2
  %pa = getelementptr inbounds i8, ptr @a, i16 %o
  %va = load i16, ptr %pa
  %wa = add i16 %va, 1
  store i16 %wa, ptr %pa
  %pb = getelementptr inbounds i8, ptr @b, i16 %o
  %vb = load i16, ptr %pb
  %t = add i16 %s, %vb
  %i.next = add nsw i16 %i, 1
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %l1, label %l3

l3:
  br label %done

done:
  %r = phi i16 [ 0, %start ], [ %t, %l3 ]
  ret i16 %r
";

/// A loop tested after its trips, its entry guarded, counts to zero with
/// one counter: conc2's `do ... while`, two counters and a pointer
/// reloaded from the frame each trip.
#[test]
fn test_a_guarded_do_while_keeps_one_counter() {
    let printed = same(&program(&[("a", "i16", 2), ("b", "i16", 2)], "i16", GUARDED_DO), &[&[-3, 5], &[0, 5], &[1, 5], &[7, 3], &[30, 11]]);
    assert_eq!(counters(&printed), 1, "{printed}");
}

/// The guarded do-while's way back split to a block that only jumps:
/// the test in the body, the step before it.
#[test]
fn test_a_guarded_do_while_behind_a_forwarding_latch_keeps_one_counter() {
    let text = GUARDED_DO
        .replace("br i1 %c, label %l1, label %l3", "br i1 %c, label %back, label %l3")
        .replace("l3:\n  br label %done", "back:\n  br label %l1\n\nl3:\n  br label %done")
        .replace("[ %i.next, %l1 ]", "[ %i.next, %back ]")
        .replace("[ %t, %l1 ]", "[ %t, %back ]");
    let printed = same(&program(&[("a", "i16", 2), ("b", "i16", 2)], "i16", &text), &[&[-3, 5], &[0, 5], &[1, 5], &[7, 3], &[30, 11]]);
    assert_eq!(counters(&printed), 1, "{printed}");
}

/// A far pointer walked a word a trip beside a counter that only tests
/// the exit: the counter is the pointer's distance.
const FAR_SUM: &str = "define i16 @f(ptr addrspace(1) %q, i16 %n) {
entry:
  br label %l1

l1:
  %i = phi i16 [ 0, %entry ], [ %i.next, %l2 ]
  %p = phi ptr addrspace(1) [ %q, %entry ], [ %p.next, %l2 ]
  %s = phi i16 [ 0, %entry ], [ %t, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %v = load i16, ptr addrspace(1) %p
  %t = add i16 %s, %v
  %p.next = getelementptr i8, ptr addrspace(1) %p, i16 2
  %i.next = add nsw i16 %i, 1
  br label %l1

l3:
  ret i16 %s
}
";

/// Basic's five far arrays read at `lo + 8 + i`, as the descriptors give
/// them: three word arrays and two dword, summed to a symbolic bound.
const FAR_ARRAYS: &str = "define i32 @f(ptr dereferenceable(18) nocapture readonly %0, ptr dereferenceable(18) nocapture readonly %1, ptr dereferenceable(18) nocapture readonly %2, ptr dereferenceable(18) nocapture readonly %3, ptr dereferenceable(18) nocapture readonly %4, ptr dereferenceable(2) nocapture readonly %5, ptr dereferenceable(2) nocapture readonly %6, ptr dereferenceable(2) nocapture readonly %7) addrspace(1) memory(read, inaccessiblemem: none) {
b1:
  %8 = load i16, ptr %5
  %9 = sub i16 %8, 1
  %10 = getelementptr i8, ptr %0, i16 2
  %11 = load i16, ptr %10
  %12 = getelementptr i8, ptr %0, i16 10
  %13 = load i16, ptr %12
  %14 = inttoptr i16 %11 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr i8, ptr %1, i16 2
  %17 = load i16, ptr %16
  %18 = getelementptr i8, ptr %1, i16 10
  %19 = load i16, ptr %18
  %20 = inttoptr i16 %17 to ptr addrspace(2)
  %21 = addrspacecast ptr addrspace(2) %20 to ptr addrspace(1)
  %22 = getelementptr i8, ptr %2, i16 2
  %23 = load i16, ptr %22
  %24 = getelementptr i8, ptr %2, i16 10
  %25 = load i16, ptr %24
  %26 = inttoptr i16 %23 to ptr addrspace(2)
  %27 = addrspacecast ptr addrspace(2) %26 to ptr addrspace(1)
  %28 = getelementptr i8, ptr %3, i16 2
  %29 = load i16, ptr %28
  %30 = getelementptr i8, ptr %3, i16 10
  %31 = load i16, ptr %30
  %32 = inttoptr i16 %29 to ptr addrspace(2)
  %33 = addrspacecast ptr addrspace(2) %32 to ptr addrspace(1)
  %34 = getelementptr i8, ptr %4, i16 2
  %35 = load i16, ptr %34
  %36 = getelementptr i8, ptr %4, i16 10
  %37 = load i16, ptr %36
  %38 = inttoptr i16 %35 to ptr addrspace(2)
  %39 = addrspacecast ptr addrspace(2) %38 to ptr addrspace(1)
  br label %b2

b2:
  %40 = phi i16 [ 0, %b1 ], [ %69, %b5 ]
  %41 = phi i32 [ 0, %b1 ], [ %68, %b5 ]
  %42 = icmp sle i16 %40, %9
  br i1 %42, label %b5, label %b6

b5:
  %43 = add i16 %40, 8
  %44 = mul i16 %43, 2
  %45 = add i16 %13, %44
  %46 = getelementptr i8, ptr addrspace(1) %15, i16 %45
  %47 = load i16, ptr addrspace(1) %46
  %48 = sext i16 %47 to i32
  %49 = add i32 %41, %48
  %50 = mul i16 %43, 4
  %51 = add i16 %19, %50
  %52 = getelementptr i8, ptr addrspace(1) %21, i16 %51
  %53 = load i32, ptr addrspace(1) %52
  %54 = add i32 %49, %53
  %55 = add i16 %25, %44
  %56 = getelementptr i8, ptr addrspace(1) %27, i16 %55
  %57 = load i16, ptr addrspace(1) %56
  %58 = sext i16 %57 to i32
  %59 = add i32 %54, %58
  %60 = add i16 %31, %50
  %61 = getelementptr i8, ptr addrspace(1) %33, i16 %60
  %62 = load i32, ptr addrspace(1) %61
  %63 = add i32 %59, %62
  %64 = add i16 %37, %44
  %65 = getelementptr i8, ptr addrspace(1) %39, i16 %64
  %66 = load i16, ptr addrspace(1) %65
  %67 = sext i16 %66 to i32
  %68 = add i32 %63, %67
  %69 = add nsw i16 %40, 1
  br label %b2

b6:
  %70 = phi i32 [ %41, %b2 ]
  %71 = add i32 %70, 1
  ret i32 %71
}
";

/// A far pointer walked beside the counter was tested for the exit with
/// `icmp ne ptr addrspace(1)`, which the selector has no form for: 147
/// loop-corpus cases (Basic and C) failed to build.
#[test]
fn test_a_far_pointer_is_never_compared_for_the_exit() {
    let printed = on_core(FAR_ARRAYS);
    assert!(!printed.lines().any(|line| line.contains("icmp") && line.contains("ptr addrspace(1)")), "{printed}");
}

/// `text` through `Lsr` on a Core: an address-size prefix stalls three clocks.
fn on_core(text: &str) -> String {
    let costs = llrm_x86_m16::target::costs("Core");
    let machine = llrm_x86_m16::Dos { address_forms: llrm_x86_m16::target::address_forms(&costs, 3), costs, ..llrm_x86_m16::Dos::default() };
    let mut module = parsed(&format!("{DOS}{text}"));
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Lsr);
    manager.run_module(&mut module, Rc::new(machine)).unwrap();
    printed(&module)
}

/// A decreasing loop over six arrays at symbolic strides, as a fuzz case
/// (`rnd98_0367`): `199 - i` read the stepped value of a new counter before
/// the step that defines it, and the pass's output failed verification.
const DOWN_WITH_STRIDES: &str = "@a1 = global [8060 x i8] zeroinitializer\n@a2 = global [1612 x i8] zeroinitializer\n\ndeclare i16 @_lcopy(ptr addrspace(1), ptr addrspace(1), i16) addrspace(1)\n\ndefine i32 @f(ptr addrspace(1) nocapture writeonly %0, ptr addrspace(1) nocapture readonly %1, ptr addrspace(1) nocapture readonly %2, ptr addrspace(1) nocapture readonly %3, ptr nocapture readonly %4, ptr addrspace(1) nocapture readonly %5, ptr addrspace(1) nocapture readonly %6, i16 %7, i16 %8, i16 %9) addrspace(1) {
b1:
  %10 = alloca [8060 x i8]
  %11 = addrspacecast ptr @a1 to ptr addrspace(1)
  %12 = addrspacecast ptr %10 to ptr addrspace(1)
  %13 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %12, ptr addrspace(1) %11, i16 8060)
  br label %b2

b2:
  %14 = phi i16 [ 0, %b1 ], [ %83, %b6 ]
  %15 = icmp slt i16 %14, 3
  br i1 %15, label %b4, label %b3

b3:
  ret i32 1

b4:
  %16 = mul nsw i16 %14, %9
  br label %b5

b5:
  %17 = phi i16 [ 199, %b4 ], [ %81, %b5 ]
  %18 = mul nsw i16 %17, %9
  %19 = add nsw i16 %18, %16
  %20 = add nsw i16 %19, 10
  %21 = mul nsw i16 %20, 10
  %22 = getelementptr inbounds i8, ptr %10, i16 %21
  %23 = getelementptr inbounds i8, ptr %22, i16 0
  %24 = load i16, ptr %23
  %25 = sext i16 %24 to i32
  %26 = mul nsw i16 3, %17
  %27 = add nsw i16 %26, %16
  %28 = add nsw i16 %27, 10
  %29 = mul nsw i16 %28, 2
  %30 = getelementptr inbounds i8, ptr @a2, i16 %29
  %31 = load i16, ptr %30
  %32 = sext i16 %31 to i32
  %33 = add nsw i32 %25, %32
  %34 = add nsw i16 %17, %16
  %35 = add nsw i16 %34, 13
  %36 = mul nsw i16 %35, 2
  %37 = getelementptr inbounds i8, ptr addrspace(1) %1, i16 %36
  %38 = load i16, ptr addrspace(1) %37
  %39 = sext i16 %38 to i32
  %40 = add nsw i32 %33, %39
  %41 = add nsw i16 %17, %7
  %42 = add nsw i16 %41, %16
  %43 = add nsw i16 %42, 15
  %44 = mul nsw i16 %43, 4
  %45 = getelementptr inbounds i8, ptr addrspace(1) %2, i16 %44
  %46 = load i32, ptr addrspace(1) %45
  %47 = add nsw i32 %40, %46
  %48 = add nsw i16 %27, 15
  %49 = mul nsw i16 %48, 2
  %50 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 %49
  %51 = load i16, ptr addrspace(1) %50
  %52 = sext i16 %51 to i32
  %53 = add nsw i32 %47, %52
  %54 = add nsw i16 %17, %8
  %55 = add nsw i16 %54, %16
  %56 = add nsw i16 %55, 11
  %57 = mul nsw i16 %56, 4
  %58 = getelementptr inbounds i8, ptr %4, i16 %57
  %59 = load i32, ptr %58
  %60 = add nsw i32 %53, %59
  %61 = sub nsw i16 199, %17
  %62 = add nsw i16 %61, %16
  %63 = add nsw i16 %62, 13
  %64 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 %63
  %65 = load i8, ptr addrspace(1) %64
  %66 = zext i8 %65 to i32
  %67 = add nsw i32 %60, %66
  %68 = add nsw i16 %55, 13
  %69 = mul nsw i16 %68, 2
  %70 = getelementptr inbounds i8, ptr addrspace(1) %6, i16 %69
  %71 = load i16, ptr addrspace(1) %70
  %72 = sext i16 %71 to i32
  %73 = add nsw i32 %67, %72
  %74 = srem i32 %73, 97
  %75 = trunc i32 %74 to i16
  %76 = add nsw i16 %17, -6
  %77 = add nsw i16 %76, %16
  %78 = add nsw i16 %77, 15
  %79 = mul nsw i16 %78, 2
  %80 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %79
  store i16 %75, ptr addrspace(1) %80
  %81 = add nsw i16 %17, -1
  %82 = icmp sge i16 %81, 0
  br i1 %82, label %b5, label %b6

b6:
  %83 = add nsw i16 %14, 1
  br label %b2
}
";

/// `text` through `Lsr` on a P5, as the rich route prices it.
fn on_p5(text: &str) -> String {
    let costs = llrm_x86_m16::target::costs("P5");
    let machine = llrm_x86_m16::Dos { address_forms: llrm_x86_m16::target::address_forms(&costs, 0), costs, ..llrm_x86_m16::Dos::default() };
    let mut module = parsed(&format!("{DOS}{text}"));
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Lsr);
    manager.run_module(&mut module, Rc::new(machine)).unwrap();
    printed(&module)
}

/// Verification after the pass is the assertion: a use of a stepped value
/// is dominated by the step.
#[test]
fn test_a_step_stays_ahead_of_the_readers_of_its_value() {
    let printed = on_p5(DOWN_WITH_STRIDES);
    assert!(printed.contains("lsr.iv.next"), "{printed}");
}

/// `s += a[k * i + 8]` over words, `n` trips: a symbolic byte stride of `2k`.
const STRIDE_SYMBOLIC: &str = "  br label %l1

l1:
  %i = phi i16 [ 0, %start ], [ %i.next, %l2 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l2 ]
  %more = icmp slt i16 %i, %n
  br i1 %more, label %l2, label %l3

l2:
  %t = mul nsw i16 %i, %k
  %x = add nsw i16 %t, 8
  %p = getelementptr inbounds i16, ptr @a, i16 %x
  %v = load i16, ptr %p
  %s.next = add i16 %s, %v
  %i.next = add nsw i16 %i, 1
  br label %l1

l3:
  ret i16 %s
";

/// `i += 3` over `a[k * i + 8]`: the product steps by `3k`, which no single
/// value carries.
#[test]
fn test_a_product_of_a_counter_stepping_by_three_is_walked_too() {
    let text = program(&[("a", "i16", 2)], "i16", STRIDE_SYMBOLIC).replace("%i.next = add nsw i16 %i, 1", "%i.next = add nsw i16 %i, 3");
    let printed = same(&text, &[&[0, 2], &[1, 2], &[5, 3], &[9, 2], &[5, -1]]);
    let body = printed.split("\nl2:").nth(1).and_then(|rest| rest.split("\nl3:").next()).unwrap_or_default();
    assert!(!body.contains(" mul ") && !body.contains("%x") && body.contains("i16 16"), "{printed}");
}

/// A product by an invariant, plus a constant, scaled by the element size, is
/// an address recurrence stepping by a symbolic `2k`: walked as one, with the
/// constant a displacement. The product was no recurrence a `gep` could be
/// derived from, so loop-corpus's `a[i*m + 8]` kept an element index and an
/// `add` the target scaled with a `lea` each trip, where main walked a pointer.
#[test]
fn test_an_address_off_a_product_by_an_invariant_is_walked_by_it() {
    let text = program(&[("a", "i16", 2)], "i16", STRIDE_SYMBOLIC);
    let printed = same(&text, &[&[0, 2], &[1, 2], &[5, 3], &[9, 4], &[5, -1]]);
    let body = printed.split("\nl2:").nth(1).and_then(|rest| rest.split("\nl3:").next()).unwrap_or_default();
    assert!(!body.contains("add nsw"), "{printed}");
    assert!(body.contains("i16 16"), "{printed}");
}

/// A far array of a runtime segment, offset zero, filled and summed over a
/// constant 20 trips: `segld`'s inner loop.
const FAR_SEGMENT_LOOP: &str = "define i16 @f(i16 %sel) {
entry:
  %seg = inttoptr i16 %sel to ptr addrspace(2)
  %far = addrspacecast ptr addrspace(2) %seg to ptr addrspace(1)
  br label %l1

l1:
  %i = phi i16 [ 1, %entry ], [ %i.next, %l1 ]
  %t = phi i16 [ 0, %entry ], [ %t.next, %l1 ]
  %p = getelementptr inbounds i16, ptr addrspace(1) %far, i16 %i
  store i16 %i, ptr addrspace(1) %p
  %t.next = add i16 %t, %i
  %i.next = add nsw i16 %i, 1
  %more = icmp sle i16 %i.next, 20
  br i1 %more, label %l1, label %l3

l3:
  ret i16 %t.next
}
";

/// A far view of a segment is held in a segment register, not a general one:
/// priced as a register it made walking the far pointer cheaper than counting
/// an offset to zero, and `segld`'s loop kept a compare against 21 beside it
/// (+105 instructions, and 1.2 million reloads in the QB demo's PLASMA).
#[test]
fn test_a_far_view_of_a_segment_takes_no_general_register() {
    let printed = on_p5(FAR_SEGMENT_LOOP);
    assert!(!printed.contains(", 21"), "{printed}");
}

/// Each address of two registers is a base and an index, and a register is
/// one or the other: with one base register and two index registers, a base
/// shared by two streams is fine and two bases are not, as `partners` (how
/// many bases one index pairs with) took `fs:[si+ax]` to be.
#[test]
fn test_addresses_need_a_base_register_and_an_index_register() {
    use super::Reg;
    let pair = |one: usize, other: usize| (Reg::Iv(one), Reg::Iv(other));
    let registers = |pairs: &[(Reg, Reg)]| pairs.iter().flat_map(|(one, other)| [one.clone(), other.clone()]).collect::<std::collections::BTreeSet<_>>();
    // `[bx+si]` and `[bx+di]`: one base, two indices.
    let shared = [pair(0, 1), pair(0, 2)];
    assert_eq!(super::_misplaced(&registers(&shared), &shared.iter().cloned().collect(), 1, 2), 0);
    // `[bx+si]` and `[ax+di]`: two bases, and there is one.
    let apart = [pair(0, 1), pair(2, 3)];
    assert_eq!(super::_misplaced(&registers(&apart), &apart.iter().cloned().collect(), 1, 2), 1);
}

/// A counter already counted to zero whose value is summed: one recurrence,
/// tested for zero, read as itself.
const ZEROED: &str = "  br label %l1

l1:
  %i = phi i16 [ -8, %start ], [ %i.next, %l1 ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l1 ]
  %s.next = add i16 %s, %i
  %i.next = add nsw i16 %i, 1
  %done = icmp ne i16 %i.next, 0
  br i1 %done, label %l1, label %l3

l3:
  ret i16 %s.next
";

/// The loop as it stands is a candidate, priced as every other: where it is
/// the cheapest the pass emits nothing, as LLVM's LSR leaves a loop it cannot
/// improve.
#[test]
fn test_a_loop_the_pass_cannot_improve_is_left_as_it_is() {
    let text = program(&[("a", "i16", 2)], "i16", ZEROED);
    let (before, printed) = reduced(&text);
    let loop_of = |text: &str| text.split("\nl1:").nth(1).and_then(|rest| rest.split("\n\n").next()).unwrap_or_default().to_owned();
    assert_eq!(loop_of(&printed), loop_of(&crate::testing::printed(&before)), "{printed}");
}

/// `a[i]` over a huge pointer, as C's `__huge` lowers it: the counter beside
/// a pointer that each trip advances with a carry into its selector.
const HUGE_WALK: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32\"

define i16 @f(ptr addrspace(3) %a, i16 %n) {
start:
  br label %l

l:
  %i = phi i16 [ 0, %start ], [ %i.next, %l ]
  %s = phi i16 [ 0, %start ], [ %s.next, %l ]
  %p = phi ptr addrspace(3) [ %a, %start ], [ %p.next, %l ]
  %v = load i16, ptr addrspace(3) %p
  %s.next = add i16 %s, %v
  %p.next = getelementptr inbounds i16, ptr addrspace(3) %p, i16 1
  %i.next = add nsw i16 %i, 1
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %l, label %d

d:
  ret i16 %s.next
}
";

/// Every advance of a huge pointer pays the carry into its selector, so
/// an integer offset and a `getelementptr` per use saves nothing over the
/// pointer that steps once. With the carry dear, the pass swapped the
/// pointer for `%lsr.iv` and kept the carry in the loop all the same.
#[test]
fn test_a_huge_pointer_walk_is_not_swapped_for_an_offset_that_carries_too() {
    let machine = Tuned { costs: OperationCosts { carry: 30, ..target().costs }, ..target() };
    let mut after = parsed(HUGE_WALK);
    let mut manager = PassManager::default();
    manager.verify_each = true;
    manager.add(Lsr);
    manager.run_module(&mut after, Rc::new(machine)).unwrap();
    let printed = printed(&after);
    assert!(!printed.contains("lsr.iv"), "{printed}");
}

/// A huge array indexed by a counter, as each frontend states one.
const HUGE_INDEXED: &str = "@g = addrspace(1) global [120000 x i8] zeroinitializer

define i16 @f() {
start:
  %b = addrspacecast ptr addrspace(1) @g to ptr addrspace(3)
  br label %l

l:
  %i = phi i32 [ 0, %start ], [ %i.next, %l ]
  %o = mul i32 %i, 4
  %p = getelementptr inbounds i8, ptr addrspace(3) %b, i32 %o
  store i32 %i, ptr addrspace(3) %p
  %i.next = add nsw i32 %i, 1
  %c = icmp slt i32 %i.next, 30000
  br i1 %c, label %l, label %d

d:
  ret i16 0
}
";

/// A huge pointer stepped by a constant carries by a mask; built from a
/// counter it pays the whole carry each use. Priced alike, bench/huge kept
/// the counter and carried twice an element in C, BASIC and Nib.
#[test]
fn test_a_huge_array_indexed_by_a_counter_is_walked_by_a_huge_pointer() {
    let machine = Tuned { costs: OperationCosts { carry: 11, carry_step: 4, ..target().costs }, ..target() };
    let (_, printed) = reduced_for(HUGE_INDEXED, machine);
    assert!(printed.contains("phi ptr addrspace(3)"), "{printed}");
}

/// Two arrays walked by pointers beside their counter, where a copy loop
/// reads one and writes the other.
const POINTER_WALK: &str = "@a = global [64 x i16] zeroinitializer
@b = global [64 x i16] zeroinitializer

define i16 @f(i16 %n) {
start:
  br label %l

l:
  %i = phi i16 [ 0, %start ], [ %i.next, %l ]
  %p = phi ptr [ @a, %start ], [ %p.next, %l ]
  %q = phi ptr [ @b, %start ], [ %q.next, %l ]
  %v = load i16, ptr %p
  %w = add i16 %v, 1
  store i16 %w, ptr %q
  %p.next = getelementptr inbounds i16, ptr %p, i16 1
  %q.next = getelementptr inbounds i16, ptr %q, i16 7
  %i.next = add nsw i16 %i, 1
  %c = icmp slt i16 %i.next, %n
  br i1 %c, label %l, label %d

d:
  ret i16 %i.next
}
";

/// A pointer steps with an add, not an address: pricing its step as `lea`
/// (#183) made a dear `lea` swap every pointer walk for integer offsets,
/// one more register in the loop (x_tripdata_usescale7 in C: the bound spilled).
#[test]
fn test_a_pointer_steps_at_the_price_of_an_add_whatever_an_address_costs() {
    let run = |address| {
        let machine = Tuned { costs: OperationCosts { address, ..target().costs }, ..target() };
        let mut after = parsed(&format!("{DOS}{POINTER_WALK}"));
        let mut manager = PassManager::default();
        manager.add(Lsr);
        manager.run_module(&mut after, Rc::new(machine)).unwrap();
        printed(&after)
    };
    assert_eq!(run(3), run(1));
}

/// Prices followed the loop's trips only, and a loop with no counter fitting a
/// wider use took any plan: a use behind a branch (the sieve's `sum += i32(i)`,
/// on the primes) weighed as much as one on every trip, so lsr carried a
/// 32-bit counter, an `inc` a trip, for a `movzx` on one trip in six
/// (sieve.nib -O2: 12111 -> 12951 instructions, #386).
#[test]
fn test_a_use_behind_a_branch_is_not_priced_as_one_on_every_trip() {
    let text = "define i32 @f(ptr %a) {
b1:
  br label %b6

b6:
  %i = phi i16 [ 2, %b1 ], [ %next, %b8 ]
  %sum = phi i32 [ 0, %b1 ], [ %sum.out, %b8 ]
  %count = phi i16 [ 0, %b1 ], [ %count.out, %b8 ]
  %more = icmp ult i16 %i, 1024
  br i1 %more, label %b7, label %b9

b7:
  %p = getelementptr inbounds i8, ptr %a, i16 %i
  %v = load i8, ptr %p
  %composite = icmp ne i8 %v, 0
  br i1 %composite, label %b8, label %b14

b8:
  %sum.out = phi i32 [ %sum, %b7 ], [ %added, %b14 ], [ %added, %b20 ]
  %count.out = phi i16 [ %count, %b7 ], [ %counted, %b14 ], [ %counted, %b20 ]
  %next = add nuw i16 %i, 1
  br label %b6

b9:
  %total = zext i16 %count to i32
  %high = shl i32 %total, 16
  %mixed = xor i32 %high, %sum
  ret i32 %mixed

b14:
  %counted = add i16 %count, 1
  %wide = zext i16 %i to i32
  %added = add i32 %sum, %wide
  %small = icmp ule i16 %i, 31
  br i1 %small, label %b15, label %b8

b15:
  %square = mul i16 %i, %i
  br label %b18

b18:
  %at = phi i16 [ %square, %b15 ], [ %after, %b19 ]
  %inside = icmp ult i16 %at, 1024
  br i1 %inside, label %b19, label %b20

b19:
  %q = getelementptr inbounds i8, ptr %a, i16 %at
  store i8 1, ptr %q
  %after = add i16 %at, %i
  br label %b18

b20:
  br label %b8
}
";
    let (_, after) = reduced(text);
    let wide = |text: &str| text.lines().filter(|line| line.contains("phi i32")).count();
    assert_eq!(wide(&after), wide(text), "{after}");
}

/// A counted loop that reads `i + k` on the side of a branch only: a value the counter makes in one add.
fn offset_read() -> String {
    "define i16 @f(i16 %n, i16 %k, i16 %m) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %inext, %b5 ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %b5 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b6

b2:
  %odd = and i16 %acc, 1
  %c = icmp ne i16 %odd, 0
  br i1 %c, label %b3, label %b4

b3:
  %v = add i16 %i, %k
  %t = xor i16 %v, %acc
  br label %b5

b4:
  %u = shl i16 %acc, 1
  br label %b5

b5:
  %sum = phi i16 [ %t, %b3 ], [ %u, %b4 ]
  %inext = add i16 %i, 1
  br label %b1

b6:
  ret i16 %acc
}
"
    .to_owned()
}


/// A value made from the counter in one arm, `i + k`, costs an add where an add makes it in place
/// and `mov; add` on a two-address target whose forms have no `lea` of any register (`[bx+si]`
/// only): the price omitted the copy (#705), so the arm's use and its half of a trip never paid for
/// a counter of its own. A target whose forms take any register makes it in one `lea`.
#[test]
fn test_a_value_made_from_the_counter_is_priced_with_its_copy() {
    let ivs = |two_address: bool, flat: bool| {
        let mut machine = target();
        machine.two_address = two_address;
        if !flat {
            machine.address_forms.truncate(1);
        }
        let (_, printed) = reduced_for(&offset_read(), machine);
        printed.lines().filter(|line| line.contains("= phi") && line.contains("%lsr.iv")).count()
    };
    assert_eq!(ivs(false, false), 1, "a one-address target adds in place");
    assert_eq!(ivs(true, false), 2, "`mov; add` is dearer than the step of a counter of its own");
    assert_eq!(ivs(true, true), 1, "`lea` makes it in one");
}
