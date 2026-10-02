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

/// `body` over `double %x`, through instcombine, as one function's text.
fn floated(body: &str) -> String {
    through(&["instcombine"], &format!("define double @f(double %x) {{\nb1:\n{body}\n}}\n"))
}

/// What the language lets a floating operation lose, each rule only under its flag: a division by a
/// constant a multiply by its reciprocal (`arcp`), two constants of a chain one (`reassoc`), a
/// zero's sign unobserved (`nsz`), no NaN or infinity (`nnan`, `ninf`). Without the flag the
/// instruction stays: `x / 4.0` differs from `x * 0.25` only for a reciprocal that rounds, `x * 0.0`
/// is NaN for a NaN, `x + 0.0` is +0.0 for -0.0.
#[test]
fn test_floating_flags_license_the_folds_and_their_absence_keeps_the_operation() {
    let stays = |body: &str| {
        let text = floated(body);
        assert!(text.contains(body.lines().next().unwrap().trim()), "{text}");
    };
    // arcp
    assert!(floated("  %r = fdiv arcp double %x, 3.0\n  ret double %r").contains("fmul arcp double %x, 0x3FD5555555555555"), "{}", floated("  %r = fdiv arcp double %x, 3.0\n  ret double %r"));
    stays("  %r = fdiv double %x, 3.0\n  ret double %r");
    // reassoc, both operations
    let chain = |first: &str, second: &str| format!("  %a = fadd {first} double %x, 1.0\n  %r = fadd {second} double %a, 2.0\n  ret double %r");
    assert!(floated(&chain("reassoc", "reassoc")).contains("fadd reassoc double %x, 3.0"), "{}", floated(&chain("reassoc", "reassoc")));
    assert!(floated(&chain("reassoc", "")).contains("fadd double %a, 2.0"), "{}", floated(&chain("reassoc", "")));
    assert!(floated(&chain("", "reassoc")).contains("fadd reassoc double %a, 2.0"), "{}", floated(&chain("", "reassoc")));
    // nsz
    assert_eq!(floated("  %r = fadd nsz double %x, 0.0\n  ret double %r"), "define double @f(double %x) {\nb1:\n  ret double %x\n}\n");
    stays("  %r = fadd double %x, 0.0\n  ret double %r");
    assert!(floated("  %r = fadd double %x, -0.0\n  ret double %r").contains("ret double %x"));
    // nnan and nsz make x * 0 a zero; each alone does not
    assert!(floated("  %r = fmul nnan nsz double %x, 0.0\n  ret double %r").contains("ret double 0.0"), "{}", floated("  %r = fmul nnan nsz double %x, 0.0\n  ret double %r"));
    stays("  %r = fmul nnan double %x, 0.0\n  ret double %r");
    stays("  %r = fmul nsz double %x, 0.0\n  ret double %r");
    // nnan and ninf make x - x a zero and x / x a one
    assert!(floated("  %r = fsub nnan ninf double %x, %x\n  ret double %r").contains("ret double 0.0"));
    assert!(floated("  %r = fdiv nnan ninf double %x, %x\n  ret double %r").contains("ret double 1.0"));
    stays("  %r = fsub nnan double %x, %x\n  ret double %r");
    // the multiply by one needs no flag
    assert!(floated("  %r = fmul double 1.0, %x\n  ret double %r").contains("ret double %x"));
}

fn spin(attrs: &str, load: &str) -> String {
    format!(
        "define i16 @spin(ptr %p) {attrs} {{
b0:
  br label %b1

b1:
  %v = {load} i16, ptr %p
  %more = icmp ne i16 %v, 0
  br i1 %more, label %b1, label %b2

b2:
  ret i16 0
}}
"
    )
}

/// A loop no counter bounds ended only where the language says it must: C11
/// lets a loop that does nothing observable be assumed to end, and
/// `mustprogress` is that promise. `willreturn` waited for a counted bound
/// and so was never inferred for `while (*p)`; an observable loop (a volatile
/// load) may legally run forever and stays unmarked.
#[test]
fn test_function_attrs_takes_mustprogress_for_a_loop_it_cannot_count() {
    let willreturn = |text: &str| through(&["function-attrs"], text).lines().find(|line| line.starts_with("define")).is_some_and(|line| line.contains("willreturn"));
    assert!(willreturn(&spin("mustprogress", "load")));
    assert!(!willreturn(&spin("", "load")), "no promise: an uncounted loop may not end");
    assert!(!willreturn(&spin("mustprogress", "load volatile")), "an observable loop may run forever");
}

/// C11 6.8.5p6 lets only a loop whose controlling expression is not constant be
/// assumed to end, so clang marks each such loop (`llvm.loop.mustprogress`), never
/// the function: `for (;;) {}` hangs. A function whose every uncounted loop is marked
/// is `willreturn`; one with a `for (;;)` among them is not, however the rest are marked.
#[test]
fn test_willreturn_needs_every_uncounted_loop_marked_and_for_forever_is_never_marked() {
    let willreturn = |text: &str| through(&["function-attrs"], text).lines().find(|line| line.starts_with("define")).is_some_and(|line| line.contains("willreturn"));
    let function = |second: &str, marks: &str| {
        format!(
            "define void @f(ptr %p) {{
b0:
  br label %first

first:
  %v = load i16, ptr %p
  %more = icmp ne i16 %v, 0
  br i1 %more, label %first, label %next, !llvm.loop !0

next:
  br label %second

second:
{second}
done:
  ret void
}}

!0 = distinct !{{!0, !1}}
!1 = !{{!\"llvm.loop.mustprogress\"}}
{marks}"
        )
    };
    let counted = "  %w = load i16, ptr %p\n  %again = icmp ne i16 %w, 0\n  br i1 %again, label %second, label %done, !llvm.loop !2\n";
    let forever = "  %w = load i16, ptr %p\n  br label %second, !llvm.loop !2\n";
    let marked = "!2 = distinct !{!2, !1}\n";
    let unmarked = "!2 = distinct !{!2}\n";
    assert!(willreturn(&function(counted, marked)), "both loops marked");
    assert!(!willreturn(&function(counted, unmarked)), "the second is not");
    assert!(!willreturn(&function(forever, marked)), "an unconditional loop is no loop a language promises ends, even if some pass marked it");
    assert!(!willreturn(&function(forever, unmarked)));
}
