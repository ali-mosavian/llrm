//! A load of an incoming stack argument is made again where it is read, not
//! spilled.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// Four words in registers and three on the stack, read before and after a
/// call, were each loaded at entry and stored to a frame cell (10 stores and a
/// frame; `__cmpdf2` +70 B, -Os) where the pin loads the immutable fixed stack
/// object again at the use (`isLoadFromInvariantOrConstantMemory`).
#[test]
fn an_incoming_stack_argument_is_loaded_again_not_spilled() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
declare i16 @ext(i16)\n\
define i16 @f(i16 %a, i16 %b, i16 %c, i16 %d, i16 %e, i16 %g, i16 %h) {\n\
  %x1 = add i16 %a, %d\n  %x2 = xor i16 %b, %e\n  %x3 = or i16 %c, %g\n  %x4 = and i16 %x1, %h\n\
  %r = call i16 @ext(i16 %x4)\n  %y1 = add i16 %r, %x2\n  %y2 = add i16 %y1, %x3\n  %y3 = add i16 %y2, %d\n\
  %y4 = xor i16 %y3, %e\n  %y5 = add i16 %y4, %g\n  %y6 = add i16 %y5, %h\n  %y7 = add i16 %y6, %a\n\
  %y8 = add i16 %y7, %b\n  %y9 = add i16 %y8, %c\n  ret i16 %y9\n}\n";
    for level in [Level::O1, Level::O2, Level::Os] {
        let asm = text(ir, "f", 0, &Request::new(level)).expect("compiles");
        let stores = asm.lines().filter(|line| line.trim_start().starts_with("ld -") && line.contains("(ix),")).count();
        assert_eq!(stores, 0, "{level:?}: {stores} stores to the frame:\n{asm}");
    }
}
