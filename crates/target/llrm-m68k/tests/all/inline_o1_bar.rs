//! M68K-INLINE-O1: the -O1 inline bar is 100 (`Tuning::inline_threshold_o1`),
//! the pin's 225 stays at -O2. Before it, QB at 68020 -O1 was +23,414 B over
//! gcc with 87 modules over; a function inlined at both levels cost the
//! difference in bytes.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// `@leaf` of `ops` distinct operations, called twice from `@f`.
fn module(ops: usize) -> String {
    let mut body = String::new();
    let mut last = "%a".to_owned();
    for op in 0..ops {
        let name = format!("%t{op}");
        let how = ["add", "xor", "sub", "or"][op % 4];
        body += &format!("  {name} = {how} i32 {last}, {}\n", 3 + 2 * op);
        last = name;
    }
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
define i32 @leaf(i32 %a) {{
{body}  ret i32 {last}
}}
define i32 @f(i32 %x, i32 %y) {{
  %p = call i32 @leaf(i32 %x)
  %q = call i32 @leaf(i32 %y)
  %r = add i32 %p, %q
  ret i32 %r
}}
"
    )
}

fn calls_leaf(
    ops: usize,
    level: Level,
) -> bool {
    let out = assembly(&module(ops), cpu_of("68020").unwrap(), level).expect("compiles");
    let body = out.split("\nf:").nth(1).unwrap_or(&out);
    body.contains("leaf")
}

#[test]
fn a_callee_between_the_o1_and_o2_bars_is_inlined_at_o2_only() {
    let ops = 50;
    assert!(!calls_leaf(ops, Level::O2), "premise: -O2 inlines it");
    assert!(calls_leaf(ops, Level::O1), "-O1 inlined a callee over its bar");
}

/// 68000 -O1 was +1,298 B over gcc at the bar of 112 (68020 -978), under it at
/// 100: a callee of pin cost between the two is a call at -O1.
#[test]
fn a_callee_between_100_and_112_is_a_call_at_o1() {
    assert!(!calls_leaf(40, Level::O2), "premise: -O2 inlines it");
    assert!(calls_leaf(40, Level::O1), "-O1 inlined a callee over its bar of 100");
}
