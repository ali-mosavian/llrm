//! ALLOCA-ALIGN-2 (owner 6502, exit 2026-11-15): a local the C ABI aligns to 2
//! sits at an odd address when the frame above it is odd, and the mid-end folds
//! `ptr & 1` to 0 from the alloca's `align 2`. On sim65 `use` sees bit 0 = 1
//! while `probe` returns 0. Known to fail until frames and argument areas are
//! kept even; run with `--ignored` to see the evidence.

mod common;

use llrm_6502::driver::{Level, Request, assembly};

const PROBE: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"

declare void @use(ptr)

define i8 @probe() {
entry:
  %x = alloca i16, align 2
  call void @use(ptr %x)
  %i = ptrtoint ptr %x to i16
  %m = and i16 %i, 1
  %r = trunc i16 %m to i8
  ret i8 %r
}
";

#[test]
#[ignore = "ALLOCA-ALIGN-2: an align-2 local is odd on an odd frame; the folded bit is 0"]
fn an_align_2_local_on_an_odd_frame_has_the_address_bit_the_code_returns() {
    let compiled = assembly(PROBE, &Request::new(Level::O2)).unwrap();
    let functions: String = compiled.lines().filter(|line| !line.contains(" = $")).map(|line| format!("{line}\n")).collect();
    // `g` has a one-byte frame; `use` keeps the low bit of the pointer it is given.
    let extra = format!(
        "{functions}
g:
  lda rc0
  bne g1
  dec rc1
g1:
  dec rc0
  jsr probe
  tax
  inc rc0
  bne g2
  inc rc1
g2:
  txa
  rts
use:
  lda rc2
  and #1
  sta seen
  rts
.segment \"BSS\"
seen: .res 1
.segment \"CODE\"
"
    );
    let main = "  lda #0\n  sta rc0\n  lda #$F0\n  sta rc1\n  jsr g\n  cmp seen\n  beq same\n  lda #1\n  rts\nsame:\n  lda #0\n";
    assert_eq!(common::run_with("alloca-align-two", main, &extra), 0);
}
