//! The flag verifier (`flagcheck`): a flag a compare sets must survive to its reader.

use llrm_6502::asm::Line;
use llrm_6502::flagcheck::{Step, check_block};

fn text(source: &str) -> Vec<Line> {
    source.lines().map(Line::parse).collect()
}

fn check(parts: &[(bool, bool, &str)]) -> Result<(), String> {
    let texts: Vec<Vec<Line>> = parts.iter().map(|(_, _, source)| text(source)).collect();
    let steps: Vec<Step> =
        parts.iter().zip(&texts).map(|((sets, reads, _), text)| Step { sets: *sets, reads: *reads, text }).collect();
    check_block(&steps)
}

/// div_constants_short at -Os: `cmp #11; clc; adc #1; bcs` read the step's carry. The instruction
/// that made the step was not marked as setting flags, so nothing said the compare's carry was gone.
#[test]
fn a_step_that_clears_the_carry_between_a_compare_and_its_branch_is_an_error() {
    let err = check(&[(true, false, "  cmp #11"), (false, false, "  clc\n  adc #1"), (false, true, "  bcs far")])
        .expect_err("the carry of the compare is cleared by the step");
    assert!(err.contains("reads C") && err.contains("instruction 1"), "{err}");
}

/// The 6502's other counter steps leave the carry: `inx`, `inc` of a cell, the 65C02's `inc a`.
#[test]
fn a_step_that_leaves_the_carry_is_no_error() {
    for step in ["  inx", "  inc rc3", "  inc a", "  ldx rc2"] {
        check(&[(true, false, "  cmp #11"), (false, false, step), (false, true, "  bcs far")])
            .unwrap_or_else(|why| panic!("{step}: {why}"));
    }
}

/// A load between a compare and a branch on zero is a new zero flag, not the compare's.
#[test]
fn a_load_between_a_compare_and_a_zero_branch_is_an_error() {
    check(&[(true, false, "  cmp #11"), (false, false, "  lda rc2"), (false, true, "  beq far")])
        .expect_err("the load wrote Z");
    // the carry branch does not read what a load writes
    check(&[(true, false, "  cmp #11"), (false, false, "  lda rc2"), (false, true, "  bcs far")]).unwrap();
}

/// A multi-byte add's carry reaches the next byte's `adc` through a store and a load.
#[test]
fn an_adc_chain_through_stores_and_loads_is_no_error() {
    check(&[
        (true, false, "  clc\n  adc rc2"),
        (false, false, "  sta rc4\n  lda rc3"),
        (true, true, "  adc rc5"),
    ])
    .unwrap();
}

/// A compare of a byte with zero after the load that made it expands to nothing: the branch reads the load's
/// zero and sign (`__fixsfdi`, flagged by the first version of the check).
#[test]
fn a_zero_compare_that_expanded_to_nothing_reads_the_loads_flags() {
    check(&[(false, false, "  lda rc7"), (true, false, ""), (false, true, "  bmi far")]).unwrap();
}

/// `plx` and `ply` set Z and N like `pla`; a table without them let a pull clobber a compare's zero flag unreported.
#[test]
fn a_pull_into_an_index_register_between_a_compare_and_a_zero_branch_is_an_error() {
    for pull in ["  pla", "  plx", "  ply"] {
        check(&[(true, false, "  cmp #11"), (false, false, pull), (false, true, "  beq far")])
            .expect_err("the pull wrote Z");
    }
}
