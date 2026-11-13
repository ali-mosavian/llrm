//! A bool made by a select that is returned: the return is copied into the
//! arm that falls into it (the pin's late `TailDuplicate`), not joined through
//! `y`.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

/// `(x & m) != 0` returned: the join `ldy #0 .. ldy #1; tya; rts` made
/// `bit_test` 1 B and 6 clk longer than `lda #1; rts` / `txa; rts`
/// (2026-11-11).
#[test]
fn a_returned_bool_select_returns_in_each_arm_not_through_y() {
    for level in [Level::O1, Level::Os] {
        let ir = format!(
            "target datalayout = \"{}\"
define i16 @f(i16 %x, i16 %m) {{
  %a = and i16 %x, %m
  %c = icmp ne i16 %a, 0
  %r = zext i1 %c to i16
  ret i16 %r
}}",
            datalayout()
        );
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let asm = text[text.find("f:").unwrap()..].split(".segment").next().unwrap().to_owned();
        assert!(!asm.contains("tya") && !asm.contains("ldy"), "{level:?}: joined through y\n{asm}");
        let bytes = estimate(Cpu::Mos6502, &asm).unwrap_or_else(|e| panic!("{e}\n{asm}")).bytes;
        assert!(bytes <= 25, "{level:?}: {bytes} B\n{asm}");
    }
}
