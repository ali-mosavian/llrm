//! A lane moved into a pair that is live is moved.

mod common;

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8";

const LANE_POINTER: &str = "define void @lanes(ptr %a, i16 %j, i16 %v) {
  %o = shl i16 %j, 1
  %b = getelementptr i8, ptr %a, i16 %o
  %p0 = getelementptr i8, ptr %b, i16 112
  %v0 = add i16 %v, 117
  store i16 %v0, ptr %p0, align 1
  %p1 = getelementptr i8, ptr %b, i16 768
  %v1 = add i16 %v, 124
  store i16 %v1, ptr %p1, align 1
  ret void
}";

/// The pointer `b + 768` keeps `b`'s low byte: `UNPACK` into the low lane of
/// the new pair. The lane move was dropped as dead (liveness knew the pair,
/// not its bytes), so the store went to `a`'s low byte and `b`'s high + 3:
/// 47 wrong values in a 24x24 table. a = buf, j = 1, v = 1: buf[770] = 125.
#[test]
fn a_lane_moved_into_a_live_pair_is_moved() {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{LANE_POINTER}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, "lanes", Cpu::Mos6502).expect("selects");
    let code = function_text(&module, "lanes", &selection).expect("emits");
    let data = ".segment \"DATA\"\nbuf: .res 1024\n.segment \"CODE\"\n";
    let setup = "  lda #1\n  sta rc4\n  lda #0\n  sta rc5\n  lda #<buf\n  sta rc2\n  lda #>buf\n  sta rc3\n  \
                 ldx #0\n  lda #1\n  jsr lanes\n  lda buf+770\n";
    assert_eq!(common::run_with("lane-pointer", setup, &format!("{code}{data}")), 125);
}
