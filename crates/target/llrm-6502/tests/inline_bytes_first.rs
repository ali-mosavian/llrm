//! A copy that grows the code and repays too few clocks is not made at
//! -O2/-O3 (3 clocks a byte, 6502-RATE).

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

const CALLEE_TWICE: &str = r#"
@a = global i32 7
@b = global i32 9
@r = global i32 0

define internal i32 @mix(i32 %x, i32 %y) noinline_not {
  %1 = mul i32 %x, %y
  %2 = add i32 %1, %x
  %3 = xor i32 %2, %y
  %4 = shl i32 %3, 3
  %5 = sub i32 %4, %x
  %6 = mul i32 %5, %y
  %7 = add i32 %6, %3
  %8 = xor i32 %7, %x
  %9 = lshr i32 %8, 5
  %10 = sub i32 %9, %y
  %11 = mul i32 %10, %7
  %12 = add i32 %11, %4
  ret i32 %12
}

define i32 @main() {
  %1 = load i32, ptr @a
  %2 = load i32, ptr @b
  %3 = call i32 @mix(i32 %1, i32 %2)
  %4 = call i32 @mix(i32 %2, i32 %3)
  store i32 %4, ptr @r
  ret i32 0
}
"#;

fn assembly(level: Level) -> String {
    let text =
        format!("target datalayout = \"{}\"\n{}", Mos.layout().datalayout, CALLEE_TWICE.replace(" noinline_not", ""));
    let mut request = Request::new(level);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap()
}

/// Two copies of a 12-operation i32 body cost more bytes than the two calls; at
/// -O2/-O3 the pin's threshold alone inlined them (`mix` gone, main larger).
#[test]
fn a_copy_that_grows_the_code_stays_a_call_at_speed() {
    for level in [Level::O2, Level::O3] {
        let text = assembly(level);
        assert!(text.contains("jsr mix"), "{level:?}: `mix` was inlined\n{text}");
    }
}
