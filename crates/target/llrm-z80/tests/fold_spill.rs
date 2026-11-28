//! A spilled read is taken from its frame cell
//! (`TargetInstrInfo::foldMemoryOperand`).

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `until_bit`'s mask kept in `push`/`pop` (`pop hl; push hl` every trip to
/// read it), a reload into a pair for each `and`, and a flag spilled the same
/// way: 68 instructions in the loop's function. The byte and word ops read
/// their spilled operand as `and a, d(ix)`, so the pair is never loaded: 62
/// (shift_until_idioms -O2 -6k T).
#[test]
fn a_spilled_operand_of_a_loop_and_is_read_from_its_cell() {
    let asm =
        text(include_str!("kernels/ir/until_bit.ll"), "until_bit", 0, &Request::new(Level::O2)).expect("compiles");
    let count = asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count();
    assert!(!asm.contains("pop hl\n\tpush hl") && count <= 63, "{count} instructions:\n{asm}");
}

/// `shift_count_sum` spilled its `off` pair and reloaded both bytes each trip
/// for an add that reads only the low one: 63 instructions. The low byte is
/// read from its half of the cell (`ld d, -8(ix)`), the high load is gone: 62
/// (shift_until_idioms `offset_count` -O2).
#[test]
fn the_low_half_of_a_spilled_pair_is_read_from_its_cell() {
    let asm = text(include_str!("kernels/ir/shift_count_sum.ll"), "g", 0, &Request::new(Level::O2)).expect("compiles");
    let count = asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count();
    assert!(count <= 62, "{count} instructions:\n{asm}");
}

/// wide_shift_by_one -Os `main` has no locals of its own, so a spilled word
/// that an `or` read was loaded into a pair first (`ld c, -20(ix); ld b,
/// -19(ix); ... or a, l`) where a frame existed but no local held it: 374
/// instructions against 372, 10 B over the program. The fold is made wherever
/// the function is cheaper for it, priced against the slots as `push`/`pop`.
#[test]
fn a_spilled_word_is_folded_in_a_function_with_a_frame_of_its_spills() {
    let asm =
        text(include_str!("kernels/ir/wide_shift_by_one.ll"), "main", 0, &Request::new(Level::Os)).expect("compiles");
    let count = asm.lines().filter(|line| line.starts_with('\t') && !line.trim_start().starts_with('.')).count();
    assert!(count <= 372, "{count} instructions:\n{asm}");
}
