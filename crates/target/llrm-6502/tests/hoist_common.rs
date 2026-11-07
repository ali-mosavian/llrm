//! Both paths of a branch starting with the same instructions run them before it.

use llrm_6502::asm::Asm;
use llrm_6502::peephole::{branch_over_jump, hoist_common_successor_code};

fn run(code: &str) -> String {
    let mut asm = Asm::parse(code);
    hoist_common_successor_code(&mut asm);
    branch_over_jump(&mut asm);
    asm.text()
}

/// `digits`: `n = 1` was made in both successors of the loop guard, 7 bytes
/// (`lda #1; sta rc8` twice and a `jmp` over the other copy).
#[test]
fn a_literal_both_paths_store_is_stored_before_the_branch() {
    let before = "  cmp #10\n  lda rc3\n  sbc #0\n  bcs b20\nb23:\n  lda #1\n  sta rc8\n  jmp b3\nb20:\n  lda #1\n  sta rc8\n  inc rc8\nb3:\n  rts\n";
    let after = "  cmp #10\n  lda rc3\n  sbc #0\n  lda #1\n  sta rc8\n  bcc b3\nb20:\n  inc rc8\nb3:\n  rts\n";
    assert_eq!(run(before), after);
}

/// `lda` sets the zero flag a `beq` reads.
#[test]
fn an_instruction_that_writes_the_tested_flag_stays() {
    let code = "  cmp #10\n  beq b20\nb23:\n  lda #1\n  jmp b3\nb20:\n  lda #1\n  inc rc8\nb3:\n  rts\n";
    assert_eq!(run(code), code);
}

/// A second branch to the target reads it as it was.
#[test]
fn a_target_another_branch_reaches_keeps_its_code() {
    let code = "  cmp #10\n  bcs b20\nb23:\n  sta rc8\n  jmp b3\nb20:\n  sta rc8\n  inc rc8\n  bne b20\nb3:\n  rts\n";
    assert_eq!(run(code), code);
}

/// `lz4`'s literal copy: `ldy #0` at the top of the loop, run every byte, and
/// `dec rc4; lda rc4; bne`: 2 + 2 bytes and 2 + 3 clocks a byte.
#[test]
fn a_constant_the_loop_never_changes_is_loaded_before_it() {
    use llrm_6502::peephole::hoist_loop_constant;
    let mut asm = Asm::parse("  lda rc8\nl:\n  ldy #0\n  lda (rc2),y\n  sta (rc20),y\n  dex\n  bne l\n  rts\n");
    hoist_loop_constant(&mut asm);
    assert_eq!(asm.text(), "  lda rc8\n  ldy #0\nl:\n  lda (rc2),y\n  sta (rc20),y\n  dex\n  bne l\n  rts\n");
    let mut asm = Asm::parse("  sty rc4\nentry:\nl:\n  ldy #0\n  lda (rc2),y\n  dex\n  bne l\n  rts\n");
    hoist_loop_constant(&mut asm);
    assert_eq!(asm.text(), "  sty rc4\n  ldy #0\nentry:\nl:\n  lda (rc2),y\n  dex\n  bne l\n  rts\n", "a label nothing names is stepped over");
    let writes = "  lda rc8\nl:\n  ldy #0\n  lda (rc2),y\n  iny\n  bne l\n  rts\n";
    let mut asm = Asm::parse(writes);
    hoist_loop_constant(&mut asm);
    assert_eq!(asm.text(), writes, "the loop steps y");
    let other_entry = "  jmp l\n  lda rc8\nl:\n  ldy #0\n  lda (rc2),y\n  dex\n  bne l\n  rts\n";
    let mut asm = Asm::parse(other_entry);
    hoist_loop_constant(&mut asm);
    assert_eq!(asm.text(), other_entry, "another path enters the loop");
}

#[test]
fn a_reload_after_a_step_of_the_same_cell_is_dropped_when_a_is_dead() {
    use llrm_6502::peephole::drop_flag_reload;
    let mut asm = Asm::parse("l:\n  dec rc4\n  lda rc4\n  bne l\n  lda #0\n  rts\n");
    drop_flag_reload(&mut asm);
    assert_eq!(asm.text(), "l:\n  dec rc4\n  bne l\n  lda #0\n  rts\n");
    let live = "l:\n  dec rc4\n  lda rc4\n  bne l\n  sta rc5\n  rts\n";
    let mut asm = Asm::parse(live);
    drop_flag_reload(&mut asm);
    assert_eq!(asm.text(), live, "a is read after");
}

/// `scmp`'s second `ldy #0` sat after a label nothing names (`scmp_b4:`, kept
/// by the emitter), and the walk forgot every register there: 2 bytes and 2
/// clocks a byte.
#[test]
fn a_label_nothing_names_keeps_what_is_known() {
    use llrm_6502::peephole::drop_known_loads;
    let mut asm = Asm::parse("f:\n  ldy #0\n  inc rc2\nq:\n  ldy #0\n  lda (rc2),y\n  rts\n");
    drop_known_loads(&mut asm);
    assert_eq!(asm.text(), "f:\n  ldy #0\n  inc rc2\nq:\n  lda (rc2),y\n  rts\n");
}

/// `fletcher`'s `if (a >= 255) a -= 255`: the sum is in `a` and copied to `x`
/// for the compare; the select stored it back through `txa; jmp` (11 bytes)
/// where `cmp #255; bne l; lda #0; l:` keeps it (6): the function was 64 bytes.
#[test]
fn a_select_of_what_a_compare_left_in_a_overwrites_it_in_place() {
    use llrm_6502::dagisel::hooks::Cpu;
    use llrm_6502::driver::{Level, Request, assembly};
    let text = assembly(include_str!("data/fletcher.ll"), &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("fletcher:").unwrap();
    let body = text[from..].split(".segment").next().unwrap();
    let bytes = llrm_6502::timing::estimate(Cpu::Mos6502, body).unwrap_or_else(|e| panic!("{e}\n{body}")).bytes;
    assert!(bytes <= 56, "{bytes} B\n{body}");
}

/// `strnlen`'s `lda #0; sta rc6; lda #0; sta rc7` ahead of a loop label: the
/// scan for a reader of the flags stopped at the label, so the second load
/// stayed (2 bytes, 2 clocks).
#[test]
fn a_load_before_a_label_is_dropped_when_the_flags_are_set_again_after_it() {
    use llrm_6502::peephole::drop_known_loads;
    let mut asm = Asm::parse("f:\n  lda #0\n  sta rc6\n  lda #0\n  sta rc7\nl:\n  clc\n  lda rc2\n  bne l\n  rts\n");
    drop_known_loads(&mut asm);
    assert_eq!(asm.text(), "f:\n  lda #0\n  sta rc6\n  sta rc7\nl:\n  clc\n  lda rc2\n  bne l\n  rts\n");
    let read = "f:\n  lda #0\n  sta rc6\n  lda #0\n  sta rc7\nl:\n  bne l\n  rts\n";
    let mut asm = Asm::parse(read);
    drop_known_loads(&mut asm);
    assert_eq!(asm.text(), read, "a branch after the label reads the flags the load set");
}

/// `lz4`'s entry: `lda rc16; sta rc20; lda rc17; sta rc21` led both successors of
/// `lda rc6; ora rc7; bne` (the loads write the flag the branch reads, so the copy
/// stayed twice: 8 B). It runs before the loads that make the flag.
#[test]
fn a_copy_both_paths_start_with_goes_before_the_flag_setup() {
    let before = "  lda rc6\n  ora rc7\n  bne b83\nb85:\n  lda rc16\n  sta rc20\n  lda rc17\n  sta rc21\n  lda rc20\n  rts\nb83:\n  lda rc16\n  sta rc20\n  lda rc17\n  sta rc21\nb2:\n  ldy #0\n  lda (rc2),y\n  rts\n";
    let after = "  lda rc16\n  sta rc20\n  lda rc17\n  sta rc21\n  lda rc6\n  ora rc7\n  bne b83\nb85:\n  lda rc20\n  rts\nb83:\nb2:\n  ldy #0\n  lda (rc2),y\n  rts\n";
    assert_eq!(run(before), after);
}

/// The copy cannot pass a setup that reads a cell it stores, and `a` or the flags
/// must not be read after it.
#[test]
fn a_copy_stays_when_the_setup_reads_what_it_stores_or_a_successor_reads_a_or_the_flags() {
    let reads_stored = "  lda rc6\n  ora rc20\n  bne b83\nb85:\n  lda rc16\n  sta rc20\n  lda rc20\n  rts\nb83:\n  lda rc16\n  sta rc20\n  lda rc20\n  rts\n";
    assert_eq!(run(reads_stored), reads_stored);
    let reads_a = "  lda rc6\n  ora rc7\n  bne b83\nb85:\n  lda rc16\n  sta rc20\n  sta rc21\n  rts\nb83:\n  lda rc16\n  sta rc20\n  lda #1\n  rts\n";
    assert_eq!(run(reads_a), reads_a);
    let reads_flags = "  lda rc6\n  ora rc7\n  bne b83\nb85:\n  lda rc16\n  sta rc20\n  beq b85\n  rts\nb83:\n  lda rc16\n  sta rc20\n  lda #1\n  rts\n";
    assert_eq!(run(reads_flags), reads_flags);
}
