use crate::{parse, print, transforms};

/// `text` through the pipeline, printed.
fn optimized(text: &str) -> String {
    let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    transforms::optimized(&mut module).expect("optimizes");
    print::module(&module)
}

/// `text` through `passes` alone, printed.
fn through(passes: &[&str], text: &str) -> String {
    let mut module = parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"));
    transforms::optimized_with(&mut module, passes).expect("optimizes");
    print::module(&module)
}

/// Every Nib local was a stack cell loaded and stored around each use, so
/// the isel path ran 1.98 times the old path's instructions: a loop's
/// counter is a phi, and what it sums another.
#[test]
fn test_mem2reg_makes_a_loop_counter_a_phi() {
    let text = "define i16 @sum(i16 %n) {
b1:
  %i = alloca i16
  %total = alloca i16
  store i16 0, ptr %i
  store i16 0, ptr %total
  br label %b2

b2:
  %0 = load i16, ptr %i
  %1 = icmp slt i16 %0, %n
  br i1 %1, label %b3, label %b4

b3:
  %2 = load i16, ptr %total
  %3 = load i16, ptr %i
  %4 = add i16 %2, %3
  store i16 %4, ptr %total
  %5 = add i16 %3, 1
  store i16 %5, ptr %i
  br label %b2

b4:
  %6 = load i16, ptr %total
  ret i16 %6
}
";
    assert_eq!(
        optimized(text),
        "define i16 @sum(i16 %n) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %3, %b3 ]
  %1 = phi i16 [ 0, %b1 ], [ %4, %b3 ]
  %2 = icmp slt i16 %1, %n
  br i1 %2, label %b3, label %b4

b3:
  %3 = add i16 %0, %1
  %4 = add i16 %1, 1
  br label %b2

b4:
  ret i16 %0
}
"
    );
}

/// An alloca a GEP or a volatile store reaches stays memory.
#[test]
fn test_mem2reg_keeps_what_is_not_only_loaded_and_stored() {
    let text = "define i16 @f() {
b1:
  %a = alloca [2 x i16]
  %v = alloca i16
  store volatile i16 1, ptr %v
  %p = getelementptr i16, ptr %a, i16 1
  store i16 2, ptr %p
  %x = load i16, ptr %p
  %y = load i16, ptr %v
  %z = add i16 %x, %y
  ret i16 %z
}
";
    assert_eq!(through(&["mem2reg"], text), print::module(&parse::module(text).unwrap()));
}

/// Nib's frontend spells a condition `icmp`, `sext i1` to i8, `icmp ne 0`,
/// and scales an index by `mul 1`: three instructions where one decides.
#[test]
fn test_instcombine_takes_the_frontends_booleans_and_scales_apart() {
    let text = "define i16 @f(i16 %i, i16 %n) {
b1:
  %0 = icmp ult i16 %i, %n
  %1 = sext i1 %0 to i8
  %2 = icmp ne i8 %1, 0
  br i1 %2, label %b2, label %b3

b2:
  %3 = mul i16 %n, 1
  %4 = mul i16 %i, %3
  %5 = mul i16 %4, 4
  %6 = sub i16 %5, 3
  %7 = add i16 %6, 1
  %8 = add i16 2, 3
  %9 = add i16 %7, %8
  ret i16 %9

b3:
  ret i16 0
}
";
    assert_eq!(
        optimized(text),
        "define i16 @f(i16 %i, i16 %n) {
b1:
  %0 = icmp ult i16 %i, %n
  br i1 %0, label %b2, label %b3

b2:
  %1 = mul i16 %i, %n
  %2 = shl i16 %1, 2
  %3 = add i16 %2, 3
  ret i16 %3

b3:
  ret i16 0
}
"
    );
}

/// A condition folded to a constant leaves a branch that goes one way, a
/// block nothing reaches, and a chain of blocks that only jump: isel then
/// jumped through each (T060's `value`, 71 executed instructions for 65).
#[test]
fn test_simplifycfg_folds_a_constant_branch_and_joins_the_chain() {
    let text = "define i16 @f(i16 %x) {
b1:
  br i1 true, label %b3, label %b2

b2:
  br label %b3

b3:
  %0 = phi i16 [ %x, %b1 ], [ 0, %b2 ]
  br label %b4

b4:
  %1 = add i16 %0, 1
  br label %b5

b5:
  ret i16 %1
}
";
    assert_eq!(
        optimized(text),
        "define i16 @f(i16 %x) {
b1:
  %0 = add i16 %x, 1
  ret i16 %0
}
"
    );
}

/// A block that only jumps is bypassed; the phi after it takes each of its
/// predecessors in its place.
#[test]
fn test_simplifycfg_bypasses_a_forwarding_block() {
    let text = "define i16 @f(i1 %c, i1 %d, i16 %x) {
b1:
  br i1 %c, label %b2, label %b3

b2:
  br i1 %d, label %b4, label %b5

b3:
  br label %b5

b4:
  br label %b5

b5:
  %0 = phi i16 [ 1, %b2 ], [ 2, %b3 ], [ 3, %b4 ]
  ret i16 %0
}
";
    assert_eq!(
        optimized(text),
        "define i16 @f(i1 %c, i1 %d, i16 %x) {
b1:
  br i1 %c, label %b2, label %b5

b2:
  br i1 %d, label %b4, label %b5

b4:
  br label %b5

b5:
  %0 = phi i16 [ 1, %b2 ], [ 2, %b1 ], [ 3, %b4 ]
  ret i16 %0
}
"
    );
}

/// T048 ran two more instructions per iteration of its first loop once
/// simplifycfg bypassed that loop's exit block: the second loop's phi
/// inputs were then made in the first loop's header. A loop's exit into a
/// block with phis stays.
#[test]
fn test_simplifycfg_keeps_a_loop_exit_before_phis() {
    let text = "define i16 @f(i16 %n) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %1, %b3 ]
  %c = icmp slt i16 %0, %n
  br i1 %c, label %b3, label %b4

b3:
  %1 = add i16 %0, 1
  br label %b2

b4:
  br label %b5

b5:
  %2 = phi i16 [ 0, %b4 ], [ %3, %b5 ]
  %3 = add i16 %2, 3
  %d = icmp slt i16 %3, 9
  br i1 %d, label %b5, label %b6

b6:
  ret i16 %3
}
";
    assert!(optimized(text).contains("b4:\n  br label %b5"), "{}", optimized(text));
}

/// nbody ran 201 more instructions once simplifycfg bypassed an inner
/// loop's preheader: its counter's start moved into the outer header. A
/// block leading into a loop's header stays.
#[test]
fn test_simplifycfg_keeps_a_loop_preheader() {
    let text = "define i16 @f(i16 %n, i1 %p) {
b1:
  br i1 %p, label %b5, label %b4

b5:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b5 ], [ %1, %b3 ]
  %1 = add i16 %0, 1
  %c = icmp slt i16 %1, %n
  br i1 %c, label %b3, label %b4

b3:
  br label %b2

b4:
  ret i16 %n
}
";
    assert!(optimized(text).contains("b5:\n  br label %b2"), "{}", optimized(text));
}

/// matmul8's inner loop loaded a view's dimension three times an iteration
/// and checked `k < dim` twice, once as the loop's condition: the check and
/// the reloads go, a load after a store reads the stored value, and one
/// after a call that may write stays.
#[test]
fn test_earlycse_reuses_loads_and_known_conditions() {
    let text = "declare void @panic()
declare void @write()

define i16 @f(ptr %v, i16 %k, ptr %out) {
b1:
  %0 = load i16, ptr %v
  %1 = icmp ult i16 %k, %0
  br i1 %1, label %b2, label %b4

b2:
  %2 = load i16, ptr %v
  %3 = icmp ult i16 %k, %2
  br i1 %3, label %b3, label %b5

b3:
  store i16 %k, ptr %out
  %4 = load i16, ptr %out
  call void @write()
  %5 = load i16, ptr %out
  %6 = add i16 %4, %5
  ret i16 %6

b4:
  ret i16 0

b5:
  call void @panic()
  unreachable
}
";
    assert_eq!(
        optimized(text),
        "declare void @panic()

declare void @write()

define i16 @f(ptr %v, i16 %k, ptr %out) {
b1:
  %0 = load i16, ptr %v
  %1 = icmp ult i16 %k, %0
  br i1 %1, label %b2, label %b4

b2:
  store i16 %k, ptr %out
  call void @write()
  %2 = load i16, ptr %out
  %3 = add i16 %k, %2
  ret i16 %3

b4:
  ret i16 0
}
"
    );
}

/// A load from a `noalias readonly` parameter reads what nobody writes
/// while the function runs, so a call between two loads leaves the first
/// current.
#[test]
fn test_earlycse_reuses_a_load_no_write_reaches() {
    let text = "declare void @write()

define i16 @g(ptr noalias readonly %v, ptr %w) {
b1:
  %0 = load i16, ptr %v
  %1 = load i16, ptr %w
  call void @write()
  %2 = load i16, ptr %v
  %3 = load i16, ptr %w
  %4 = add i16 %0, %2
  %5 = add i16 %1, %3
  %6 = add i16 %4, %5
  ret i16 %6
}
";
    let out = through(&["earlycse"], text);
    assert_eq!(out.matches("load i16, ptr %v").count(), 1, "{out}");
    assert_eq!(out.matches("load i16, ptr %w").count(), 2, "{out}");
}

/// matmul8 reloaded each view descriptor's shape and data pointer in its
/// innermost loop, 30024 instructions to the old path's 15344: a load that
/// may run anywhere, of memory the loop cannot write, leaves the loop. One
/// through a plain pointer, in a loop that stores, stays.
#[test]
fn test_licm_hoists_an_invariant_load_from_a_conditional_block() {
    let text = "define void @f(ptr noalias readonly dereferenceable(4) %v, ptr %w, i16 %n, ptr %out) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %7, %b4 ]
  %1 = icmp slt i16 %0, %n
  br i1 %1, label %b3, label %b5

b3:
  %2 = getelementptr i8, ptr %v, i16 2
  %3 = load i16, ptr %2
  %4 = load i16, ptr %w
  %5 = add i16 %3, %4
  %6 = getelementptr i16, ptr %out, i16 %0
  store i16 %5, ptr %6
  br label %b4

b4:
  %7 = add i16 %0, 1
  br label %b2

b5:
  ret void
}
";
    assert_eq!(
        through(&["licm"], text),
        "define void @f(ptr noalias readonly dereferenceable(4) %v, ptr %w, i16 %n, ptr %out) {
b1:
  %0 = getelementptr i8, ptr %v, i16 2
  %1 = load i16, ptr %0
  br label %b2

b2:
  %2 = phi i16 [ 0, %b1 ], [ %7, %b4 ]
  %3 = icmp slt i16 %2, %n
  br i1 %3, label %b3, label %b5

b3:
  %4 = load i16, ptr %w
  %5 = add i16 %1, %4
  %6 = getelementptr i16, ptr %out, i16 %2
  store i16 %5, ptr %6
  br label %b4

b4:
  %7 = add i16 %2, 1
  br label %b2

b5:
  ret void
}
"
    );
}

/// A fill's `a[i, j]` was `(i * 8 + j) * 4` each iteration, 768
/// instructions to the old path's 69 in nested_fill: the address becomes a
/// byte offset stepping by 4, and `i * 8` leaves the inner loop.
#[test]
fn test_loop_reduce_steps_an_address_by_its_stride() {
    let text = "define void @fill() {
b1:
  %0 = alloca [256 x i8]
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %9, %b6 ]
  %2 = icmp slt i16 %1, 8
  br i1 %2, label %b3, label %b7

b3:
  %3 = shl i16 %1, 3
  br label %b4

b4:
  %4 = phi i16 [ 0, %b3 ], [ %8, %b5 ]
  %5 = icmp slt i16 %4, 8
  br i1 %5, label %b5, label %b6

b5:
  %6 = add i16 %3, %4
  %7 = getelementptr inbounds i32, ptr %0, i16 %6
  store i32 0, ptr %7
  %8 = add i16 %4, 1
  br label %b4

b6:
  %9 = add i16 %1, 1
  br label %b2

b7:
  ret void
}
";
    assert_eq!(
        through(&["loop-reduce", "instcombine"], text),
        "define void @fill() {
b1:
  %0 = alloca [256 x i8]
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %11, %b6 ]
  %2 = icmp slt i16 %1, 8
  br i1 %2, label %b3, label %b7

b3:
  %3 = shl i16 %1, 3
  %4 = shl i16 %3, 2
  br label %b4

b4:
  %5 = phi i16 [ %4, %b3 ], [ %10, %b5 ]
  %6 = phi i16 [ 0, %b3 ], [ %9, %b5 ]
  %7 = icmp slt i16 %6, 8
  br i1 %7, label %b5, label %b6

b5:
  %8 = getelementptr inbounds i8, ptr %0, i16 %5
  store i32 0, ptr %8
  %9 = add i16 %6, 1
  %10 = add i16 %5, 4
  br label %b4

b6:
  %11 = add i16 %1, 1
  br label %b2

b7:
  ret void
}
"
    );
}

/// floats.compare's `a[at]` shares `at - 1` with the counter: a recurrence
/// of its own would save one shift for one more live value, so it stays.
#[test]
fn test_loop_reduce_keeps_what_would_add_a_live_value() {
    let text = "define i8 @compare(ptr %a, ptr %b) {
b1:
  br label %b2

b2:
  %at = phi i16 [ 72, %b1 ], [ %next, %b4 ]
  %more = icmp ne i16 %at, 0
  br i1 %more, label %b3, label %b6

b3:
  %next = add i16 %at, -1
  %offset = shl i16 %next, 1
  %p = getelementptr i8, ptr %a, i16 %offset
  %x = load i16, ptr %p
  %q = getelementptr i8, ptr %b, i16 %offset
  %y = load i16, ptr %q
  %differ = icmp ne i16 %x, %y
  br i1 %differ, label %b5, label %b4

b4:
  br label %b2

b5:
  ret i8 1

b6:
  ret i8 0
}
";
    let same = through(&[], text);
    assert_eq!(through(&["loop-reduce"], text), same);
}

/// runtime.nib's `scratch_at` was a far call per digit of every printed
/// number, 40460 instructions to the old path's 36460 in nbody: a small
/// callee's body replaces the call, its two returns a phi.
#[test]
fn test_inline_copies_a_small_callee_into_its_caller() {
    let text = "define internal i16 @magnitude(i16 %v) {
b1:
  %0 = icmp slt i16 %v, 0
  br i1 %0, label %b2, label %b3

b2:
  %1 = sub i16 0, %v
  ret i16 %1

b3:
  ret i16 %v
}

define i16 @f(i16 %n) {
b1:
  %0 = call i16 @magnitude(i16 %n)
  %1 = add i16 %0, 1
  ret i16 %1
}
";
    assert_eq!(
        through(&["inline"], text),
        "define internal i16 @magnitude(i16 %v) {
b1:
  %0 = icmp slt i16 %v, 0
  br i1 %0, label %b2, label %b3

b2:
  %1 = sub i16 0, %v
  ret i16 %1

b3:
  ret i16 %v
}

define i16 @f(i16 %n) {
b1:
  br label %0

0:
  %1 = icmp slt i16 %n, 0
  br i1 %1, label %2, label %4

2:
  %3 = sub i16 0, %n
  br label %5

4:
  br label %5

5:
  %6 = phi i16 [ %3, %2 ], [ %n, %4 ]
  %7 = add i16 %6, 1
  ret i16 %7
}
"
    );
}

/// A call within a cycle of calls stays a call.
#[test]
fn test_inline_keeps_a_recursive_call() {
    let text = "define internal i16 @down(i16 %n) {
b1:
  %0 = icmp eq i16 %n, 0
  br i1 %0, label %b2, label %b3

b2:
  ret i16 0

b3:
  %1 = sub i16 %n, 1
  %2 = call i16 @down(i16 %1)
  ret i16 %2
}
";
    assert_eq!(through(&["inline"], text), through(&[], text));
}
