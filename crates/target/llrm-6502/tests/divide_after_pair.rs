//! The remainder of a divide-and-remainder pair (rc6:rc7) was read after a
//! later divide had reused those cells (div_constants_short, 2026-11-15: wrong
//! at every level). The sign pair's runtime scanned as naming only rc2..rc7, so
//! the allocator kept the remainder across the call; the divides keep the
//! convention clobber set.

mod common;

use llrm_6502::driver::{Level, Request, assembly};

fn program(
    x: i16,
    y: i16,
    a: i16,
    b: i16,
) -> String {
    format!(
        "target datalayout = \"{}\"\n\
         @gx = global i16 {x}\n@gy = global i16 {y}\n@ga = global i16 {a}\n@gb = global i16 {b}\n\
         define i16 @f() {{\n\
         \x20 %x = load volatile i16, ptr @gx\n  %y = load volatile i16, ptr @gy\n\
         \x20 %a = load volatile i16, ptr @ga\n  %b = load volatile i16, ptr @gb\n\
         \x20 %q = sdiv i16 %x, %y\n  %r = srem i16 %x, %y\n\
         \x20 %p = sdiv i16 %a, %b\n  %m = srem i16 %a, %b\n\
         \x20 %s = add i16 %p, %r\n  %t = add i16 %s, %q\n  %u = xor i16 %t, %m\n  ret i16 %u\n}}\n",
        common::pipeline::datalayout()
    )
}

#[test]
fn a_remainder_of_a_pair_survives_the_divides_after_it() {
    for level in [Level::O1, Level::O2, Level::Os] {
        for (x, y, a, b) in [(-4829, 37, 1234, -7), (4829, -37, -31000, 100), (30000, 7, 999, 3)] {
            let compiled = assembly(&program(x, y, a, b), &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
            let text: String =
                compiled.lines().filter(|line| !line.contains(" = $")).map(|line| format!("{line}\n")).collect();
            let expected = (x / y + x % y + a / b) ^ (a % b);
            let main = "  jsr __r_f\n  stx rc2\n  eor rc2\n";
            let want = ((expected as u16 & 0xff) ^ (expected as u16 >> 8)) as i32;
            let got = common::run_with("divide_after_pair", main, &text);
            assert_eq!(got, want, "{level:?} {x} {y} {a} {b}:\n{text}");
        }
    }
}
