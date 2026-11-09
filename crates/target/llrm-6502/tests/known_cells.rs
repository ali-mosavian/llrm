//! A register that holds the byte of a cell, across labels: `xs8` reloaded
//! `rc2` at its loop head and at its exit (`lda rc2` after `sta rc2`, 2 B and
//! 3 clocks each, a trip) though `a` held it on every edge (2026-11-09).

mod common;

use common::pipeline::datalayout;
use llrm_6502::asm::Asm;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::peephole::drop_known_loads;

fn known(text: &str) -> String {
    let mut asm = Asm::parse(text);
    drop_known_loads(&mut asm);
    asm.text()
}

/// (`f:` is the function's entry: the first label is never a branch target.)
/// Every edge into `L` has `a` equal to `rc2` (the fall-through and the back
/// edge both end in `sta rc2`): the load at `L` goes.
#[test]
fn a_load_of_the_cell_a_holds_on_every_edge_goes() {
    let text = "f:\n  sta rc2\nL:\n  lda rc2\n  asl a\n  sta rc2\n  dex\n  bne L\n  rts\n";
    assert_eq!(known(text), "f:\n  sta rc2\nL:\n  asl a\n  sta rc2\n  dex\n  bne L\n  rts\n");
}

/// The back edge arrives with `a` changed (`asl a` after the store): the
/// edges disagree and the load stays.
#[test]
fn a_load_stays_when_one_edge_changed_the_register() {
    let text = "f:\n  sta rc2\nL:\n  lda rc2\n  sta rc2\n  asl a\n  dex\n  bne L\n  rts\n";
    assert_eq!(known(text), text);
}

/// Any other write of the cell, between the store and the load, makes the
/// register stale: a read-modify-write, an indexed or indirect store, a call.
/// A store of another register makes that register the holder: `a` is stale.
#[test]
fn a_write_of_the_cell_forgets_what_a_holds() {
    let text = "  sta rc2\n  stx rc2\n  lda rc2\n  rts\n";
    assert_eq!(known(text), text);
    for write in ["  inc rc2\n", "  sta rc2,x\n", "  sta (rc4),y\n", "  jsr f\n"] {
        let text = format!("  sta rc2\n{write}  lda rc2\n  rts\n");
        assert_eq!(known(&text), text, "{write:?}");
    }
}

/// `x` holds the cell as well: `ldx rc2` of what `x` holds goes. A load into
/// another register stays, not a transfer: `sta rc7 / ldx rc7` as `tax` hid the
/// copy from the folds after this pass, which make `lda rc1 / sta rc7 / ldx rc7
/// ... txa` one `lda rc1` (`fill_nest` O2: +12 B and +384 instructions).
#[test]
fn a_load_of_what_another_register_holds_stays() {
    assert_eq!(known("  stx rc2\n  ldx rc2\n  rts\n"), "  stx rc2\n  rts\n");
    for text in ["  stx rc2\n  lda rc2\n  rts\n", "  sta rc7\n  ldx rc7\n  rts\n"] {
        assert_eq!(known(text), text);
    }
}

/// A branch that reads the flags the load sets keeps it.
#[test]
fn a_load_whose_flags_are_read_stays() {
    let text = "  sta rc2\n  lda rc2\n  beq L\n  rts\nL:\n  rts\n";
    assert_eq!(known(text), text);
}

/// A store must not turn "`a` holds 0" into "`a` holds the cell": the next
/// `lda #0` went undropped, +2 B and +2 clocks per store of a zero run in 80 of
/// 86 corpus programs (2026-11-09).
#[test]
fn a_store_keeps_the_immediate_the_register_holds() {
    let text = "  lda #0\n  sta rc3\n  lda #0\n  sta rc4\n  rts\n";
    assert_eq!(known(text), "  lda #0\n  sta rc3\n  sta rc4\n  rts\n");
}

/// `xs8`: the byte is in `a` on every edge, so the loop reloads nothing, and
/// the answer is the same.
#[test]
fn a_byte_kept_in_a_cell_round_a_loop_is_not_reloaded() {
    let ir = format!(
        "target datalayout = \"{}\"\n\
         define i8 @xs8(i8 %x, i8 %n) {{\nentry:\n  %z = icmp eq i8 %n, 0\n  br i1 %z, label %out, label %l\n\
         l:\n  %i = phi i8 [%n, %entry], [%i1, %l]\n  %v = phi i8 [%x, %entry], [%v3, %l]\n  %a = shl i8 %v, 3\n  \
         %b = xor i8 %v, %a\n  %c = lshr i8 %b, 5\n  %d = xor i8 %b, %c\n  %e = shl i8 %d, 1\n  %v3 = xor i8 %d, %e\n  \
         %i1 = sub i8 %i, 1\n  %cc = icmp ne i8 %i1, 0\n  br i1 %cc, label %l, label %out\n\
         out:\n  %r = phi i8 [%x, %entry], [%v3, %l]\n  ret i8 %r\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("xs8:").unwrap();
    let body: Vec<&str> = text[from..].split(".segment").next().unwrap().lines().map(str::trim).collect();
    let reloads = body.iter().filter(|l| l.starts_with("lda rc")).count();
    assert_eq!(reloads, 0, "{body:?}");
}
