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
        "define i16 @f(i16 %i, i16 %n) memory(none) willreturn norecurse {
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

/// A call into a recursion stays a call: bench_agg's call of a recursive
/// `take` was inlined, the copy's own call of `take` inlined again, and
/// the inliner never stopped.
#[test]
fn test_inline_keeps_a_call_into_a_recursion() {
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

define i16 @f(i16 %n) {
b1:
  %0 = call i16 @down(i16 %n)
  ret i16 %0
}
";
    assert_eq!(through(&["inline"], text), through(&[], text));
}

/// priced_unroll's `main` called `value(3)` for nothing: `value` touches
/// only its own stack and counts every loop to its bound, so it is
/// `memory(none) willreturn`, and the call goes. One that calls what may
/// not return stays.
#[test]
fn test_function_attrs_lets_an_unused_call_go() {
    let text = "declare void @fail()

define internal i16 @quiet(i16 %n) {
b1:
  %0 = alloca i16
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %2, %b3 ]
  %c = icmp slt i16 %1, 8
  br i1 %c, label %b3, label %b4

b3:
  store i16 %1, ptr %0
  %2 = add i16 %1, 1
  br label %b2

b4:
  %3 = load i16, ptr %0
  ret i16 %3
}

define internal i16 @loud(i16 %n) {
b1:
  call void @fail()
  ret i16 %n
}

define i16 @main() {
b1:
  %0 = call i16 @quiet(i16 3)
  %1 = call i16 @loud(i16 3)
  ret i16 0
}
";
    let out = through(&["function-attrs", "instcombine"], text);
    assert!(out.contains("define internal i16 @quiet(i16 %n) memory(none) willreturn norecurse {"), "{out}");
    assert!(!out.contains("call i16 @quiet"), "{out}");
    assert!(out.contains("call i16 @loud"), "{out}");
}

/// Once its loads read what was stored, a slot is only written: it and
/// every store and memset into it go.
#[test]
fn test_instcombine_removes_a_slot_only_written() {
    let text = "declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) memory(argmem: write)

define i16 @f() {
b1:
  %0 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 8, i1 false)
  %1 = getelementptr inbounds i8, ptr %0, i16 2
  store i16 7, ptr %1
  ret i16 7
}
";
    let out = through(&["instcombine"], text);
    assert!(out.contains("define i16 @f() {\nb1:\n  ret i16 7\n}"), "{out}");
}

/// Which functions of `text` `function-attrs` marks `norecurse`.
fn norecurse(text: &str) -> Vec<String> {
    let out = through(&["function-attrs"], text);
    let mut found = Vec::new();
    for line in out.lines().filter(|line| line.starts_with("define")) {
        if line.contains("norecurse") {
            let name = line.split('@').nth(1).and_then(|rest| rest.split('(').next()).expect("a name");
            found.push(name.to_owned());
        }
    }
    found
}

/// A function that nothing can enter while it runs says so: a leaf, one that
/// calls a `nocallback` declaration or an intrinsic. Not one that calls
/// itself, its mutual caller, an unbounded pointer, a declaration that may
/// call back, or anything reaching those. Inline kept its own `recursive` set
/// and the rest of the pipeline had none.
#[test]
fn test_function_attrs_infers_norecurse_where_nothing_can_reenter() {
    let text = "declare void @quiet() nocallback
declare void @loud()
declare i16 @llvm.smax.i16(i16, i16)

define void @leaf() {
b0:
  ret void
}

define void @calls_quiet() {
b0:
  call void @quiet()
  ret void
}

define void @calls_loud() {
b0:
  call void @loud()
  ret void
}

define i16 @calls_intrinsic(i16 %c) {
b0:
  %m = call i16 @llvm.smax.i16(i16 %c, i16 0)
  ret i16 %m
}

define void @self() {
b0:
  call void @self()
  ret void
}

define void @ping() {
b0:
  call void @pong()
  ret void
}

define void @pong() {
b0:
  call void @ping()
  ret void
}

define void @pointer(ptr %p) {
b0:
  call void %p()
  ret void
}

define void @listed(ptr %p) {
b0:
  call void %p(), !callees !0
  ret void
}

define void @through() {
b0:
  call void @pointer(ptr null)
  ret void
}

!0 = !{ptr @leaf}
";
    let mut found = norecurse(text);
    found.sort();
    assert_eq!(found, ["calls_intrinsic", "calls_quiet", "leaf", "listed"]);
}
