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
