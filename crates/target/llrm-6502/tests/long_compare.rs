//! A compare of two 64-bit values is one borrow chain over the eight bytes.

use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::select::function_text;

const DATALAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8";

fn text(
    function: &str,
    body: &str,
) -> String {
    let ir = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&ir).expect("parses");
    let selection = select_function(&module, function, Cpu::Mos6502).expect("selects");
    function_text(&module, function, &selection).expect("emits")
}

/// `n >= d` of a division loop was the compare of the high words, the compare
/// of the low words, and their xors or-ed to pick between them: 16 `sbc` and
/// 8 `eor`/`ora` for the 8 `sbc` of the chain (and one to make the bit) (the divide runs 4516 B and
/// 8786 clk against llvm-mos's 2291 and 3970).
#[test]
fn an_unsigned_compare_of_longs_is_one_chain() {
    let body = "define i1 @ge(i64 %n, i64 %d) {\n  %c = icmp uge i64 %n, %d\n  ret i1 %c\n}";
    let t = text("ge", body);
    assert_eq!(t.matches("sbc").count(), 9, "{t}"); // 8 of the chain, 1 to make the bit
    assert_eq!(t.matches("ora").count(), 0, "{t}");
}

/// The signed order flips the top byte's sign and keeps the chain.
#[test]
fn a_signed_compare_of_longs_is_one_chain() {
    let body = "define i1 @lt(i64 %n, i64 %d) {\n  %c = icmp slt i64 %n, %d\n  ret i1 %c\n}";
    let t = text("lt", body);
    assert_eq!(t.matches("sbc").count(), 9, "{t}");
}

/// The chain's bit was saved in a register at each word (`lda #0; rol a; tax`
/// ... `txa; lsr a`) with only loads and stores between, which keep the
/// carry: 5 instructions a word.
#[test]
fn the_borrow_stays_in_the_carry_across_loads() {
    let body = "define i1 @ge(i64 %n, i64 %d) {\n  %c = icmp uge i64 %n, %d\n  ret i1 %c\n}";
    let t = text("ge", body);
    assert_eq!(t.matches("rol a").count(), 0, "{t}");
}
