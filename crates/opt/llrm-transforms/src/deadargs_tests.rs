use crate::deadargs::removed;
use crate::testing::{parsed, printed};

fn run(text: &str) -> String {
    let mut module = parsed(&format!("{}{text}", llrm_analysis::testing::DOS));
    removed(&mut module);
    printed(&module)
}

const CALLER: &str = "define i16 @top(i16 %x) {
b0:
  %r = call i16 @f(i16 %x, i16 12)
  ret i16 %r
}
";

/// A length every caller passed and the body folded was still pushed at each call: Nib's queens
/// pushed `12` to `safe` and `7` to `place` on every call.
#[test]
fn a_parameter_nothing_reads_is_not_passed() {
    let after = run(&format!(
        "define internal i16 @f(i16 %a, i16 %unused) {{
b0:
  %r = add i16 %a, 1
  ret i16 %r
}}
{CALLER}"
    ));
    assert!(after.contains("@f(i16 %a)") || after.contains("@f(i16 %0)"), "{after}");
    assert!(after.contains("call i16 @f(i16 %x)"), "{after}");
}

/// Passing it on to itself is no read.
#[test]
fn a_parameter_only_passed_on_to_the_recursive_call_is_not_passed() {
    let after = run(&format!(
        "define internal i16 @f(i16 %a, i16 %unused) {{
b0:
  %done = icmp sle i16 %a, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %a, 1
  %r = call i16 @f(i16 %m, i16 %unused)
  ret i16 %r
out:
  ret i16 %a
}}
{CALLER}"
    ));
    assert!(after.contains("call i16 @f(i16 %m)") && after.contains("call i16 @f(i16 %x)"), "{after}");
}

/// A caller outside the module passes it.
#[test]
fn an_exported_function_keeps_its_parameters() {
    let after = run(&format!(
        "define i16 @f(i16 %a, i16 %unused) {{
b0:
  ret i16 %a
}}
{CALLER}"
    ));
    assert!(after.contains("call i16 @f(i16 %x, i16 12)"), "{after}");
}

/// hanoi's three pegs only trade places in its calls and are never read: gcc's IPA-SRA and LLVM's DAE drop them
/// (`hanoi.isra.0(n)`), where a parameter passed only in its own place was all this knew: llrm pushed and held
/// four values where gcc holds one, hanoi 2.2 times gcc's clocks.
#[test]
fn parameters_passed_only_among_themselves_to_the_recursive_calls_are_not_passed() {
    let after = run("define internal i16 @f(i16 %n, i16 %a, i16 %b, i16 %c) {
b0:
  %done = icmp eq i16 %n, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %x = call i16 @f(i16 %m, i16 %a, i16 %c, i16 %b)
  %y = add i16 %x, 1
  %w = call i16 @f(i16 %m, i16 %c, i16 %b, i16 %a)
  %s = add i16 %y, %w
  ret i16 %s
out:
  ret i16 0
}
define i16 @top(i16 %x) {
b0:
  %r = call i16 @f(i16 %x, i16 1, i16 3, i16 2)
  ret i16 %r
}
");
    assert!(
        after.contains("call i16 @f(i16 %m)") && after.contains("call i16 @f(i16 %x)") && !after.contains("i16 %a"),
        "{after}"
    );
}

/// One parameter that is read keeps those it is passed to alive: `%a` is the loop's bound.
#[test]
fn a_parameter_the_body_reads_keeps_the_parameters_it_trades_with() {
    let after = run("define internal i16 @f(i16 %n, i16 %a, i16 %b) {
b0:
  %done = icmp eq i16 %n, %a
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %x = call i16 @f(i16 %m, i16 %b, i16 %a)
  ret i16 %x
out:
  ret i16 0
}
define i16 @top(i16 %x) {
b0:
  %r = call i16 @f(i16 %x, i16 1, i16 3)
  ret i16 %r
}
");
    assert!(after.contains("call i16 @f(i16 %m, i16 %b, i16 %a)"), "{after}");
}

/// A parameter handed to another function is read there: it is live, and so is the one it is traded with.
#[test]
fn a_parameter_passed_to_another_function_is_live_through_the_call() {
    let after = run("declare void @use(i16)
define internal i16 @f(i16 %n, i16 %a, i16 %b) {
b0:
  %done = icmp eq i16 %n, 0
  br i1 %done, label %out, label %again
again:
  call void @use(i16 %a)
  %m = sub i16 %n, 1
  %x = call i16 @f(i16 %m, i16 %b, i16 %a)
  ret i16 %x
out:
  ret i16 0
}
define i16 @top(i16 %x) {
b0:
  %r = call i16 @f(i16 %x, i16 1, i16 3)
  ret i16 %r
}
");
    assert!(after.contains("call i16 @f(i16 %m, i16 %b, i16 %a)"), "{after}");
}

/// Whoever takes the address of the function may call it with all its arguments: the signature stays.
#[test]
fn a_function_whose_address_is_taken_keeps_its_parameters() {
    let after = run("@hook = global ptr @f
define internal i16 @f(i16 %n, i16 %a, i16 %b) {
b0:
  %done = icmp eq i16 %n, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %x = call i16 @f(i16 %m, i16 %b, i16 %a)
  ret i16 %x
out:
  ret i16 0
}
define i16 @top(i16 %x) {
b0:
  %r = call i16 @f(i16 %x, i16 1, i16 3)
  ret i16 %r
}
");
    assert!(after.contains("call i16 @f(i16 %x, i16 1, i16 3)"), "{after}");
}

/// A call through a variadic type passes more than the parameters name: nothing is dropped.
#[test]
fn a_variadic_function_keeps_its_parameters() {
    let after = run("define internal i16 @f(i16 %n, i16 %a, ...) {
b0:
  %done = icmp eq i16 %n, 0
  br i1 %done, label %out, label %again
again:
  %m = sub i16 %n, 1
  %x = call i16 (i16, i16, ...) @f(i16 %m, i16 %a)
  ret i16 %x
out:
  ret i16 0
}
define i16 @top(i16 %x) {
b0:
  %r = call i16 (i16, i16, ...) @f(i16 %x, i16 1, i16 5)
  ret i16 %r
}
");
    assert!(after.contains("i16 %x, i16 1, i16 5)"), "{after}");
}
