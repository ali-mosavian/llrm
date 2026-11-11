//! `site_price`: the tabulated inline costs are the lowering's, and each long
//! shift or multiply at -O1 takes the form its own price names.
use llrm_z80::compile::{lines, text};
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::Line;
use llrm_z80::expand::price_emitted;
use llrm_z80::site_price::{CALL_MINUS_INLINE_SHIFT, INLINE_MUL, INLINE_SHIFT};

fn cost(body: &str) -> (u32, u64) {
    let ir = format!(
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\ndefine i32 @f(i32 %x) {{\n{body}}}\n"
    );
    let request = Request::new(Level::O2);
    let lines = lines(&ir, "f", 0, &request).expect("compiles");
    price_emitted(&lines, true)
}

/// The call form's cost less the inline form's, or `NEVER` where the call form
/// does not compile (a shift by one is `add x, x` before the call is
/// considered).
fn delta(
    op: &str,
    n: u32,
) -> (i64, i64) {
    match (at_o1(op, n, true), at_o1(op, n, false)) {
        (Some(c), Some(i)) => ((i64::from(c.0) - i64::from(i.0)) / 2, (c.1 as i64 - i.1 as i64) / 2),
        _ => NEVER,
    }
}

const NEVER: (i64, i64) = (0, 1_000_000);

/// Two functions of one long shift each (the routine is not the sole use)
/// beside one with two long multiplies by constants (so the routine is linked),
/// in the form `call` (or inline), priced as the sweep prices it.
fn at_o1(
    op: &str,
    n: u32,
    call: bool,
) -> Option<(u32, u64)> {
    let ir = format!(
        "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
define i32 @f(i32 %x) {{
  %a = {op} i32 %x, {n}
  ret i32 %a
}}
define i32 @h(i32 %x) {{
  %a = {op} i32 %x, {n}
  ret i32 %a
}}
define i32 @g(i32 %z, i32 %w) {{
  %c = mul i32 %z, 777
  %d = mul i32 %w, 999
  %e = add i32 %c, %d
  ret i32 %e
}}
"
    );
    let dir = std::env::temp_dir().join(format!("site_price_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temp dir");
    let input = dir.join(format!("{op}{n}.ll"));
    let output = dir.join(format!("{op}{n}.s"));
    std::fs::write(&input, ir).expect("writes");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc"))
        .args(["-S", "-O1", "-o"])
        .arg(&output)
        .arg(&input)
        .env("Z80_SITE_FORM", format!("{}:{n}", if call { "call" } else { "inline" }))
        .output()
        .expect("runs llc");
    if !out.status.success() {
        return None;
    }
    let lines: Vec<Line> = std::fs::read_to_string(&output)
        .expect("the assembly")
        .lines()
        .filter(|line| !line.trim_start().starts_with('.'))
        .map(|line| match line.strip_suffix(':').or_else(|| line.strip_suffix("::")) {
            Some(label) if !line.starts_with('\t') => Line::Label(label.to_owned()),
            _ => Line::Inst(line.trim().to_owned()),
        })
        .collect();
    Some(price_emitted(&lines, true))
}

#[test]
#[ignore]
fn dump_call_minus_inline() {
    for op in ["shl", "lshr", "ashr"] {
        let row: Vec<String> = (1..32).map(|n| format!("{:?}", delta(op, n))).collect();
        println!("{op}: [{}]", row.join(", "));
    }
}

#[test]
fn the_tabulated_call_minus_inline_costs_are_what_the_lowering_emits() {
    for (row, op) in ["shl", "lshr", "ashr"].into_iter().enumerate() {
        for n in 1..32u32 {
            assert_eq!(delta(op, n), CALL_MINUS_INLINE_SHIFT[row][n as usize - 1], "{op} {n}");
        }
    }
}

const RET: (u32, u64) = (1, 10);

/// The inline shift and multiply costs `site_price` prices against the calls: a
/// table that has drifted from the lowering prices a site by a form it no
/// longer takes.
#[test]
fn the_tabulated_inline_costs_are_what_the_lowering_emits() {
    for (row, op) in ["shl", "lshr", "ashr"].into_iter().enumerate() {
        for n in 1..32usize {
            let c = cost(&format!("%r = {op} i32 %x, {n}\nret i32 %r\n"));
            assert_eq!((c.0 - RET.0, c.1 - RET.1), INLINE_SHIFT[row][n - 1], "{op} {n}");
        }
    }
    for &(c, want) in INLINE_MUL {
        let m = cost(&format!("%r = mul i32 %x, {c}\nret i32 %r\n"));
        assert_eq!((m.0 - RET.0, u64::from(m.1) - RET.1 as u64), (want.0, want.1), "mul {c}");
    }
}

/// call_keeps -O1 chose a form for the whole function (every long shift inline,
/// or the routine for all the heavy ones by a fixed threshold, on a 1.6% margin
/// of the function's price): `x << 3` (32 B, 148 T inline against a
/// 5 B call) stayed 27 B too big while `x << 9` (42 B, 167 T against 426 T) was
/// the routine's. Each site is priced alone: `<< 3` is the call, `<< 9` stays
/// inline, in one function.
#[test]
fn at_o1_each_long_shift_takes_the_form_its_own_price_names() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
define i32 @f(i32 %x, i32 %y, i32 %z, i32 %w) {
  %a = shl i32 %x, 3
  %b = shl i32 %y, 9
  %c = mul i32 %z, 777
  %d = mul i32 %w, 3
  %e = add i32 %a, %b
  %f = add i32 %e, %c
  %g = add i32 %f, %d
  ret i32 %g
}
";
    let text = text(ir, "f", 0, &Request::new(Level::O1)).expect("compiles");
    assert_eq!(text.matches("call __ashlsi3").count(), 1, "one shift is the routine's:\n{text}");
    assert_eq!(text.matches("call __mulsi3").count(), 1, "z * 1000 is the routine's, w * 3 is not:\n{text}");
}
