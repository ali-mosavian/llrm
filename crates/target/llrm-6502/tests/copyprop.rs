//! Copy propagation across a store of the other register: `k_mulax3` kept
//! `sta rc4` and loaded it back after `stx rc3; lda rc3` (A:X live-in, 2026-11-09).

use llrm_6502::asm::Asm;
use llrm_6502::copyprop::{copy_propagation, enabled};
use llrm_6502::peephole_cells::drop_dead_stores;

fn run(text: &str) -> String {
    let mut asm = Asm::parse(text);
    copy_propagation(&mut asm);
    asm.text()
}

/// The copies that nothing reads go too; `results` are the cells the function returns.
fn run_and_drop(
    text: &str,
    results: &[&str],
) -> String {
    let mut asm = Asm::parse(text);
    asm.result_cells = results.iter().map(|one| one.to_string()).collect();
    copy_propagation(&mut asm);
    drop_dead_stores(&mut asm);
    asm.text()
}

/// `sta rc2; stx rc3; sta rc4; lda rc3; sta rc5; lda rc4` kept the mirror
/// `rc4` (2 B) and read `rc3` from memory when `x` held it (1 B).
#[test]
fn a_cell_that_mirrors_another_is_read_from_the_earlier_and_a_transfer_replaces_a_load() {
    assert_eq!(
        run_and_drop(
            "  sta rc2\n  stx rc3\n  sta rc4\n  lda rc3\n  sta rc5\n  lda rc4\n  asl a\n  rol rc5\n  clc\n  adc rc2\n  rts\n",
            &["rc2", "rc3", "rc5"]
        ),
        "  sta rc2\n  stx rc3\n  txa\n  sta rc5\n  lda rc2\n  asl a\n  rol rc5\n  clc\n  adc rc2\n  rts\n"
    );
}

/// The source is written between the copy and the read: the copy holds the old value.
#[test]
fn a_cell_is_not_read_from_a_source_that_changed() {
    let text = "  sta rc2\n  sta rc4\n  lda #1\n  sta rc2\n  lda rc4\n  rts\n";
    assert_eq!(run(text), text);
}

/// A load of the value `a` holds goes, unless a branch reads its flags.
#[test]
fn a_reload_of_the_value_a_holds_goes_when_the_flags_are_unread() {
    assert_eq!(run("  sta rc2\n  lda rc2\n  clc\n  adc rc3\n  rts\n"), "  sta rc2\n  clc\n  adc rc3\n  rts\n");
    let text = "  sta rc2\n  lda rc2\n  beq L1\n  rts\nL1:\n  rts\n";
    assert_eq!(run(text), text);
}

/// `lda rc1` fed only `tax`'s source and was dead itself: rewriting `ldx rc1` to
/// `tax` kept it alive, 65C02 `fill_nest` +2 B and +64 executed instructions.
#[test]
fn a_load_into_a_that_nothing_reads_does_not_make_a_transfer_out_of_an_index_load() {
    assert_eq!(
        run("  lda rc0\n  lda rc1\n  ldx rc1\n  lda rc0\n  clc\n  adc rc3\n  rts\n"),
        "  ldx rc1\n  lda rc0\n  clc\n  adc rc3\n  rts\n"
    );
}

/// The pass shipped behind `LLRM_6502_MCP=1` and nothing exercised it in the product:
/// it runs unless the variable is `0`.
#[test]
fn the_pass_runs_unless_the_kill_switch_is_zero() {
    use std::ffi::OsStr;
    assert!(enabled(None));
    assert!(enabled(Some(OsStr::new("1"))));
    assert!(!enabled(Some(OsStr::new("0"))));
}
