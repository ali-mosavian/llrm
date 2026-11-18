//! Two spills that the stack can hold only when the later one is stored after
//! the earlier one's reload become a `push` and a `pop` each.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `g(n - 1) + g(n - 2)`: the first result was spilled right after its call
/// and the argument `n` reloaded after that, so the two slots did not nest and
/// the result went to the frame (`ld iy, #2; add iy, sp; ld 0(iy), e; ld 1(iy),
/// d`, 46 B for fib against 31 B when both are pushes).
const IR: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
    declare i16 @g(i16)\n\
    define i16 @f(i16 %n) {\n  %a = sub i16 %n, 1\n  %x = call i16 @g(i16 %a)\n  %b = sub i16 %n, 2\n  %y = call i16 @g(i16 %b)\n  %s = add i16 %x, %y\n  ret i16 %s\n}\n";

#[test]
fn two_spills_that_nest_once_the_later_is_sunk_are_pushes() {
    let out = text(IR, "f", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(!out.contains("iy"), "a spill went through the frame:\n{out}");
}
