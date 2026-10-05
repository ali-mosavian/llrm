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
    let after = run(&format!("define internal i16 @f(i16 %a, i16 %unused) {{
b0:
  %r = add i16 %a, 1
  ret i16 %r
}}
{CALLER}"));
    assert!(after.contains("@f(i16 %a)") || after.contains("@f(i16 %0)"), "{after}");
    assert!(after.contains("call i16 @f(i16 %x)"), "{after}");
}

/// Passing it on to itself is no read.
#[test]
fn a_parameter_only_passed_on_to_the_recursive_call_is_not_passed() {
    let after = run(&format!("define internal i16 @f(i16 %a, i16 %unused) {{
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
{CALLER}"));
    assert!(after.contains("call i16 @f(i16 %m)") && after.contains("call i16 @f(i16 %x)"), "{after}");
}

/// A caller outside the module passes it.
#[test]
fn an_exported_function_keeps_its_parameters() {
    let after = run(&format!("define i16 @f(i16 %a, i16 %unused) {{
b0:
  ret i16 %a
}}
{CALLER}"));
    assert!(after.contains("call i16 @f(i16 %x, i16 12)"), "{after}");
}
