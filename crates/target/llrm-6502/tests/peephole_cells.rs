//! Dead cell stores at a return, and a word shift that ends in `a`.

use llrm_6502::asm::Asm;
use llrm_6502::peephole_cells::{chain_ends_in_a, drop_dead_stores, store_then_index_load};

fn asm(
    text: &str,
    results: &[&str],
) -> Asm {
    let mut asm = Asm::parse(text);
    asm.result_cells = results.iter().map(|c| (*c).to_owned()).collect();
    asm
}

/// The `sta rc2` after a shift run left `a` as the result: 2 bytes.
#[test]
fn a_store_to_a_cell_the_return_does_not_read_is_dropped() {
    let mut a = asm("  sta rc2\n  ldx rc3\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  ldx rc3\n  rts\n");
}

/// `rc2` is a long result's byte; `rc8` is the caller's; neither is dead.
#[test]
fn a_store_to_a_returned_or_callee_saved_cell_stays() {
    for (text, results) in [("  sta rc2\n  rts\n", &["rc2"][..]), ("  sta rc8\n  rts\n", &[])] {
        let mut a = asm(text, results);
        drop_dead_stores(&mut a);
        assert_eq!(a.text(), text);
    }
}

/// stricmp's `select i16 -1, 1` left `lda #255; sta rc4; sta rc5` (6 B) before
/// a `bcs` over the loads of the other arm: the scan stopped at the branch,
/// though both paths reach an `rts` that does not read the cells.
#[test]
fn a_store_before_a_branch_is_dropped_when_no_path_reads_the_cell() {
    let mut a = asm("  sta rc4\n  cmp rc3\n  bcs L\n  ldy #255\nL:\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  cmp rc3\n  bcs L\n  ldy #255\nL:\n  rts\n");
}

/// One path reads `rc4`: the store stays.
#[test]
fn a_store_before_a_branch_stays_when_one_path_reads_the_cell() {
    let text = "  sta rc4\n  cmp rc3\n  bcs L\n  lda rc4\n  rts\nL:\n  rts\n";
    let mut a = asm(text, &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), text);
}

/// A loop that never reads `rc4` before the return, and one that does.
#[test]
fn a_store_before_a_loop_is_dropped_only_when_the_loop_never_reads_it() {
    let dead = "  sta rc4\nL:\n  dex\n  bne L\n  rts\n";
    let mut a = asm(dead, &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "L:\n  dex\n  bne L\n  rts\n");
    let read = "  sta rc4\nL:\n  lda rc4\n  dex\n  bne L\n  rts\n";
    let mut a = asm(read, &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), read);
}

/// `float_conversions_loop` (+#8590) kept `ldy #0; sty rc10` after a call:
/// nothing read `rc10` before the epilogue's restore, and a call keeps the
/// callee-saved cells, so the store is dead (4 B with its load).
#[test]
fn a_store_to_a_callee_saved_cell_is_dropped_when_only_the_restore_follows() {
    let mut a = asm("  sty rc10\n  jsr f\n  jsr g\n  pla\n  sta rc10\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  jsr f\n  jsr g\n  pla\n  sta rc10\n  rts\n");
}

/// A read after the call, a return without the restore, and a caller-saved cell
/// across a call (the callee may read it as an argument) all keep the store.
#[test]
fn a_store_across_a_call_stays_when_a_read_or_the_return_follows() {
    for text in [
        "  sty rc10\n  jsr f\n  lda rc10\n  rts\n",
        "  sty rc10\n  jsr f\n  rts\n",
        "  sta rc2\n  jsr f\n  pla\n  rts\n",
        // a call through a pointer: the `rts` jumps to the address pushed, and
        // the callee reads rc2
        "  sta rc2\n  lda t\n  pha\n  lda t+1\n  pha\n  rts\n",
    ] {
        let mut a = asm(text, &[]);
        drop_dead_stores(&mut a);
        assert_eq!(a.text(), text);
    }
}

/// The 65C02 stores a zero with `stz`: `float_conversions_loop` kept a `stz
/// rc10` after the call that only the restore followed (65C02-FLOATCONV-8590),
/// where the 6502's `ldy #0; sty rc10` was dropped. Every store op is a store.
#[test]
fn a_dead_stz_goes_like_a_dead_sta() {
    let mut a = asm("  stz rc10\n  jsr f\n  pla\n  sta rc10\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  jsr f\n  pla\n  sta rc10\n  rts\n");
    let mut a = asm("  stz rc2\n  jsr f\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  stz rc2\n  jsr f\n  rts\n");
}

/// `sta rc3; lda rc2; ldx rc3` went through memory to reach `x`.
#[test]
fn a_byte_stored_and_loaded_into_x_is_transferred() {
    let mut a = asm("  sta rc3\n  lda rc2\n  ldx rc3\n  rts\n", &[]);
    store_then_index_load(&mut a);
    assert_eq!(a.text(), "  tax\n  lda rc2\n  rts\n");
}

#[test]
fn a_shift_run_whose_low_byte_is_loaded_next_shifts_in_a() {
    let mut a = asm("  lsr rc3\n  ror rc2\n  lsr rc3\n  ror rc2\n  lda rc2\n  ldx rc3\n  rts\n", &[]);
    chain_ends_in_a(&mut a);
    assert_eq!(a.text(), "  lda rc2\n  lsr rc3\n  ror a\n  lsr rc3\n  ror a\n  ldx rc3\n  rts\n");
}

/// A call clobbers the scratch cells and reads none of them: `sta rc16; ...;
/// jsr f` kept the store (rep_count -O1 inlined into main: two dead splat
/// copies a trip, +1,703 instructions, 2026-11-12).
#[test]
fn a_store_to_a_scratch_cell_before_a_call_is_dropped() {
    let mut a = asm("  sta rc16\n  lda rc3\n  jsr f\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  lda rc3\n  jsr f\n  rts\n");
}

/// `rc6` is an argument cell: the call reads it.
#[test]
fn a_store_to_an_argument_cell_before_a_call_stays() {
    let text = "  sta rc6\n  jsr f\n  rts\n";
    let mut a = asm(text, &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), text);
}

/// `__mulsi3` works in `__sa`/`__sb` and names no `rcN`: a constant's high
/// bytes stored to `rc2`/`rc3` before it were kept as arguments (mul_constants
/// -Os: 249 lines, 478 B of dead stores, 2026-11-12). `__udivhi3` reads `rc4`:
/// that store stays.
#[test]
fn a_store_before_a_routine_that_names_no_cell_is_dropped() {
    let mut a = asm("  sta rc3\n  sta __sb\n  jsr __mulsi3\n  rts\n", &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), "  sta __sb\n  jsr __mulsi3\n  rts\n");
    let text = "  sta rc4\n  jsr __udivhi3\n  rts\n";
    let mut a = asm(text, &[]);
    drop_dead_stores(&mut a);
    assert_eq!(a.text(), text);
}
