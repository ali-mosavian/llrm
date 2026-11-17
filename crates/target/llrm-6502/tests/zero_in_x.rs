//! A byte stored to cells and also wanted in `x` is made in `x`, and a transfer
//! beside its reverse is one transfer.

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

fn at(
    level: Level,
    ir: &str,
) -> String {
    let text = format!("target datalayout = \"{}\"\n{ir}", Mos.layout().datalayout);
    let mut request = Request::new(level);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{level:?}: {why}"))
}

const TAIL_CALL: &str = "declare void @report(i32)

define void @f() noinline {
  tail call void @report(i32 1)
  ret void
}
";

const RETURNS_ZERO: &str = "declare void @report(i32)

define i16 @main() {
  tail call void @report(i32 12)
  tail call void @report(i32 0)
  ret i16 0
}
";

/// sibling_call_callees (75 B against llvm-mos 73): `f` was `lda #0; sta rc2;
/// sta rc3; tax; lda #1; jmp report` where llvm-mos makes the zero in `x`.
#[test]
fn the_zero_of_a_wide_argument_is_made_in_x() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = at(level, TAIL_CALL);
        assert!(text.contains("ldx #0\n  stx rc2\n  stx rc3\n  lda #1\n  jmp report"), "{level:?}\n{text}");
    }
}

/// The `main` that returns 0 ended `lda #0; tax; txa; rts`: the pair is one
/// transfer.
#[test]
fn a_transfer_beside_its_reverse_is_one_transfer() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = at(level, RETURNS_ZERO);
        assert!(!text.contains("tax\n  txa"), "{level:?}\n{text}");
    }
}

const ZERO_ARGUMENT: &str = "declare void @report(i32)

define void @main() {
  call void @report(i32 0)
  ret void
}
";

/// The first placement ran on parsed text, where no callee was known to take
/// `a`: `report(0)` lost its `lda #0` and strlen_idiom printed a wrong value.
#[test]
fn a_call_keeps_the_zero_it_takes_in_a() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = at(level, ZERO_ARGUMENT);
        assert!(text.contains("lda #0\n  sta rc2\n  sta rc3\n  tax\n  jmp report"), "{level:?}\n{text}");
    }
}
