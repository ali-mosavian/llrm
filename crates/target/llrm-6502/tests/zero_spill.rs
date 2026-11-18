//! A whole program's static cells were BSS: three-byte, four-clock operands
//! while 87 bytes of zero page were free (recdeep -O1: 38 absolute accesses a
//! trip where 24 were needed once the argument cells took the registers). The
//! cells the loops touch most are zero-page equates.

mod common;

use llrm_6502::zerospill::static_cells_to_zero_page;

const TEXT: &str = ".segment \"CODE\"\nf:\n  lda s_f_1\n  sta s_f_1\n  lda s_f_1\n  sta s_f_1\n  lda s_f_1\nloop:\n  lda s_f_0\n  sta s_f_0\n  dex\n  bne loop\n  rts\n.segment \"BSS\"\ns_f_0: .res 1\ns_f_1: .res 1\n.segment \"CODE\"\n";

#[test]
fn a_cell_in_a_loop_is_a_zero_page_equate_ahead_of_one_touched_more_often_outside() {
    let text = static_cells_to_zero_page(TEXT, 0xA9, 1);
    assert!(text.starts_with("s_f_0 = $A9\n"), "{text}");
    assert!(!text.contains("s_f_0: .res"), "{text}");
    assert!(text.contains("s_f_1: .res 1"), "{text}");
}

#[test]
fn every_cell_that_fits_is_taken_in_order_of_weight() {
    let text = static_cells_to_zero_page(TEXT, 0xA9, 2);
    assert!(text.starts_with("s_f_0 = $A9\ns_f_1 = $AA\n"), "{text}");
    assert!(!text.contains(".res"), "{text}");
}

/// A program with more cells than zero page left still assembles: the rest
/// stay where they were.
#[test]
fn a_budget_of_nothing_changes_nothing() {
    assert_eq!(static_cells_to_zero_page(TEXT, 0xA9, 0), TEXT);
}

/// Zero page wraps at 256, and the address of an equate is a zero-page address:
/// an indexed cell or one whose address is taken stays BSS.
#[test]
fn an_indexed_cell_or_one_whose_address_is_taken_stays_bss() {
    let indexed = TEXT.replace("lda s_f_0\n", "lda s_f_0,y\n");
    let text = static_cells_to_zero_page(&indexed, 0xA9, 2);
    assert!(text.contains("s_f_0: .res 1") && !text.contains("s_f_0 = "), "{text}");
    let taken = TEXT.replace("lda s_f_0\n", "lda #<(s_f_0)\n");
    let text = static_cells_to_zero_page(&taken, 0xA9, 2);
    assert!(text.contains("s_f_0: .res 1") && !text.contains("s_f_0 = "), "{text}");
}

/// Names that are not `s_` cells are the static locals: arrays code indexes.
#[test]
fn a_static_local_stays_bss() {
    let text = ".segment \"CODE\"\nf:\n  lda datag\n  rts\n.segment \"BSS\"\ndatag: .res 4\n";
    assert_eq!(static_cells_to_zero_page(text, 0xA9, 8), text);
}

/// Four longs held across two calls spill to `s_g_` cells; a whole program
/// takes them into zero page on a platform with room, a separate object keeps
/// BSS.
const SPILLS: &str = "@v = global [4 x i32] zeroinitializer\n\
    declare void @f(i32)\n\
    define internal i32 @g(i32 %a, i32 %b, i32 %c, i32 %d) noinline norecurse nounwind {\n\
      call void @f(i32 %a)\n  call void @f(i32 %b)\n\
      %x = xor i32 %a, %b\n  %y = xor i32 %c, %d\n  %z = add i32 %x, %y\n  ret i32 %z\n}\n\
    define i32 @main() {\n  %p0 = load volatile i32, ptr @v\n  %q = getelementptr i32, ptr @v, i16 1\n\
      %p1 = load volatile i32, ptr %q\n  %r = getelementptr i32, ptr @v, i16 2\n  %p2 = load volatile i32, ptr %r\n\
      %t = getelementptr i32, ptr @v, i16 3\n  %p3 = load volatile i32, ptr %t\n\
      %x = call i32 @g(i32 %p0, i32 %p1, i32 %p2, i32 %p3)\n  ret i32 %x\n}\n";

#[test]
fn a_whole_program_spills_to_zero_page_and_a_separate_object_to_bss() {
    use llrm_6502::driver::{Level, Request, assembly};
    let ir = format!("target datalayout = \"{}\"\n{SPILLS}", common::pipeline::datalayout());
    let mut whole = Request::new(Level::O2);
    whole.passes.push("-fwhole-program".to_owned());
    let text = assembly(&ir, &whole).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("s_g_") || text.contains("s_main_"), "no spill cell at all:\n{text}");
    assert!(text.lines().any(|l| l.starts_with("s_") && l.contains(" = $")), "{text}");
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.lines().any(|l| l.starts_with("s_") && l.contains(" = $")), "{text}");
}
