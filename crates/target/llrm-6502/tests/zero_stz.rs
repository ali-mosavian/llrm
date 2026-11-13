//! A zero stored through `a` on the 65C02: `fletcher` at -Os spent `lda #0; sta
//! rc6` (3 B) and `ldy #0; tya; sta rc6` (4 B) where `stz rc6` is 2 B, 53 B
//! against SDCC's 52 (2026-11-09).

mod common;

use common::pipeline::datalayout;
use llrm_6502::asm::Asm;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::peephole::zero_cells_with_stz;

fn stz(
    text: &str,
    cmos: bool,
) -> String {
    let mut asm = Asm::parse(text);
    asm.cmos = cmos;
    zero_cells_with_stz(&mut asm);
    asm.text()
}

/// `a` is overwritten after the store: the zero goes to the cell on its own,
/// one `stz` per cell of the run.
#[test]
fn a_zero_stored_and_not_read_again_is_stz() {
    assert_eq!(stz("  lda #0\n  sta rc6\n  lda rc7\n  rts\n", true), "  stz rc6\n  lda rc7\n  rts\n");
    assert_eq!(
        stz("  lda #0\n  sta rc3\n  lda #0\n  sta rc4\n  sta rc5\n  lda rc7\n  rts\n", true),
        "  stz rc3\n  stz rc4\n  stz rc5\n  lda rc7\n  rts\n"
    );
}

/// A `bra` leaves the block as a `jmp` does: the flags `lda #0` set are not
/// read after it (`fletcher`'s empty-input path kept `lda #0; sta rc6; bra`).
#[test]
fn a_zero_stored_before_a_branch_always_is_stz() {
    let text = "  lda #0\n  sta rc6\n  bra L\nL:\n  lda rc5\n  rts\n";
    assert_eq!(stz(text, true), "  stz rc6\n  bra L\nL:\n  lda rc5\n  rts\n");
}

/// `a` is read after the store (the function returns it, or a branch reads the
/// flags `lda #0` set): the load stays. So it does on the 6502, which has no
/// `stz`.
#[test]
fn a_zero_that_is_read_again_or_has_no_stz_stays() {
    for text in ["  lda #0\n  sta rc6\n  rts\n", "  lda #0\n  sta rc6\n  beq L\nL:\n  rts\n"] {
        assert_eq!(stz(text, true), text);
    }
    let text = "  lda #0\n  sta rc6\n  lda rc7\n  rts\n";
    assert_eq!(stz(text, false), text);
}

/// `fletcher` end to end: no zero goes through `a` into a cell.
#[test]
fn fletcher_at_os_stores_its_zeros_with_stz() {
    let ir = format!(
        "target datalayout = \"{}\"\n\
         define i16 @f(ptr %p, i8 %n) {{\nentry:\n  %z = icmp eq i8 %n, 0\n  br i1 %z, label %out, label %l\n\
         l:\n  %i = phi i8 [0, %entry], [%i1, %l]\n  %a = phi i8 [0, %entry], [%a2, %l]\n  \
         %b = phi i8 [0, %entry], [%b2, %l]\n  %q = getelementptr i8, ptr %p, i8 %i\n  %v = load i8, ptr %q\n  \
         %s = add i8 %a, %v\n  %c = icmp eq i8 %s, -1\n  %a2 = select i1 %c, i8 0, i8 %s\n  %t = add i8 %b, %a2\n  \
         %d = icmp eq i8 %t, -1\n  %b2 = select i1 %d, i8 0, i8 %t\n  %i1 = add i8 %i, 1\n  \
         %cc = icmp ne i8 %i1, %n\n  br i1 %cc, label %l, label %out\n\
         out:\n  %ra = phi i8 [0, %entry], [%a2, %l]\n  %rb = phi i8 [0, %entry], [%b2, %l]\n  \
         %x = zext i8 %ra to i16\n  %y = zext i8 %rb to i16\n  %h = shl i16 %y, 8\n  %r = or i16 %h, %x\n  ret i16 %r\n}}\n",
        datalayout()
    );
    let mut request = Request::new(Level::Os);
    request.cpu = Cpu::W65c02;
    let text = assembly(&ir, &request).unwrap_or_else(|e| panic!("{e}"));
    let body: Vec<&str> = text.split(".segment").next().unwrap().lines().map(str::trim).collect();
    let through_a = body.windows(2).filter(|w| w[0] == "lda #0" && w[1].starts_with("sta rc")).count();
    assert_eq!(through_a, 0, "{body:?}");
}
