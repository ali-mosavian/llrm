//! `site_price`: the tabulated inline costs are the lowering's, and each long shift or multiply at -O1 takes
//! the form its own price names.
use llrm_z80::compile::{lines, text};
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::price_emitted;
use llrm_z80::site_price::{INLINE_MUL, INLINE_SHIFT};

fn cost(body: &str) -> (u32, u64) {
    let ir = format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\ndefine i32 @f(i32 %x) {{\n{body}}}\n");
    let request = Request::new(Level::O2);
    let lines = lines(&ir, "f", 0, &request).expect("compiles");
    price_emitted(&lines, true)
}

const RET: (u32, u64) = (1, 10);

/// The inline shift and multiply costs `site_price` prices against the calls: a table that has drifted from the
/// lowering prices a site by a form it no longer takes.
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

/// call_keeps -O1 chose a form for the whole function (every long shift inline, or the routine for all the heavy
/// ones by a fixed threshold, on a 1.6% margin of the function's price): `x << 3` (32 B, 148 T inline against a
/// 5 B call) stayed 27 B too big while `x << 9` (42 B, 167 T against 426 T) was the routine's. Each site is
/// priced alone: `<< 3` is the call, `<< 9` stays inline, in one function.
#[test]
fn at_o1_each_long_shift_takes_the_form_its_own_price_names() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"
define i32 @f(i32 %x, i32 %y, i32 %z, i32 %w) {
  %a = shl i32 %x, 3
  %b = shl i32 %y, 9
  %c = mul i32 %z, 1000
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

