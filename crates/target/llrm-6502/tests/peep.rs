//! The clean-up of a function's text: what it removes, what it keeps, and the two ways it first
//! got a program wrong.

use llrm_6502::peep::optimize;

fn opt(body: &str) -> String {
    optimize(body, &[]).expect("the pass models every instruction")
}

fn lines(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).collect()
}

/// `sta c; lda c` reloads what A holds, and `c` is read nowhere else: both go.
#[test]
fn a_reload_of_a_stored_value_and_its_dead_store_go() {
    let out = opt("f:\n  lda rc2\n  clc\n  adc rc3\n  sta v_f_1\n  lda v_f_1\n  rts\n");
    assert_eq!(lines(&out), ["f:", "lda rc2", "clc", "adc rc3", "rts"]);
}

/// Two cells that are never live together take zero-page registers, so no cell is left in memory.
#[test]
fn cells_take_zero_page_registers() {
    let out = opt("f:\n  lda rc2\n  sta v_f_1\n  lda #5\n  sta v_f_2\n  lda v_f_1\n  clc\n  adc v_f_2\n  rts\n");
    assert!(!out.contains("v_f_"), "{out}");
}

/// A cell that is read after a call cannot be in a caller-saved register: the callee may clobber it.
#[test]
fn a_cell_live_across_a_call_stays_in_memory() {
    let out = opt("f:\n  lda #7\n  sta v_f_1\n  jsr g\n  lda v_f_1\n  rts\n");
    assert!(out.contains("v_f_1"), "{out}");
}

/// `lda (rc6),y` reads Y. The first version left it out of the instruction's reads, took the first
/// `ldy #0` for dead (a second `ldy` follows) and read byte 1 twice (`bump` gave 12, not 10).
#[test]
fn an_indexed_indirect_load_keeps_its_ldy() {
    let out = opt("f:\n  ldy #0\n  lda (rc6),y\n  sta rc2\n  ldy #1\n  lda (rc6),y\n  sta rc3\n  rts\n");
    assert_eq!(lines(&out), ["f:", "ldy #0", "lda (rc6),y", "sta rc2", "ldy #1", "lda (rc6),y", "sta rc3", "rts"]);
}

/// The argument stored into `rc2` before a call is kept although `rc2` holds that value already:
/// the call reads it, and once the store was gone the allocator gave `rc2` to another cell (`pass32`
/// returned 0).
#[test]
fn a_call_argument_store_is_kept() {
    let out = opt("f:\n  lda rc2\n  sta v_f_1\n  lda v_f_1\n  sta rc2\n  jsr g\n  rts\n");
    let call = lines(&out).iter().position(|l| *l == "jsr g").unwrap();
    assert!(lines(&out)[..call].contains(&"sta rc2"), "{out}");
}
