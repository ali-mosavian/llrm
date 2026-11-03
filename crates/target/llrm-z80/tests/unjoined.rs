//! The z80's default convention leaves the copies from the argument registers
//! to the allocator (`pinned_copies_unjoined`), as the pin does.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

fn unjoined() {
    assert!(llrm_z80::abi::z80().pinned_copies_unjoined);
}

/// The placement of a copy into the entrance that lacks it ran before branch
/// folding, when the entrance was still a block of its own: bsearch8 -O2 got
/// `jr nz` / `ld a, d` / `jr` for a bare `jr z` and grew by 2 B, the copy and
/// its jump left in a block that folding would have emptied.
#[test]
fn a_copy_is_placed_after_branch_folding_has_emptied_the_blocks() {
    unjoined();
    let ir = include_str!("kernels/ir/bsearch8.ll");
    let out = text(ir, "bsearch8", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    let instructions = out.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(instructions <= 37, "{instructions}: {out}");
}

/// Stated, the convention changes what the allocator does: crc16 -O2 keeps its
/// pointer where it arrives instead of joining the copy, so its code differs
/// from the unstated convention's.
#[test]
fn the_convention_states_whether_the_argument_copies_are_joined() {
    unjoined();
    assert!(llrm_z80::abi::z80().pinned_copies_unjoined);
    assert!(llrm_z80::abi::use_calling(llrm_z80::CALLING).is_err());
}

/// With the argument copy left in place, digits copied the dividend `hl` to
/// `de` at entry for the first compare, and the loop head copied it back: the
/// compare reads `hl` where the copy is made, the loop head's copy moves to the
/// back edge (an `ex de, hl` there), and the entry copy has no reader left.
/// With the guard and latch tests merged the loop is entered at its test, so
/// the entry swap is an `ex de, hl` too, and the test is emitted once.
#[test]
fn a_word_compared_after_its_copy_reads_the_copys_source() {
    unjoined();
    let ir = include_str!("kernels/ir/digits.ll");
    let out = text(ir, "digits", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("ld e, l") && out.matches("sbc a, #0").count() == 1, "{out}");
}

/// With the argument copy left in place, crc16 copied `a` to `b` to store it
/// (`ld b, a` / `ld -1(ix), b`) after the prologue's `push af`: 4 B over the
/// hook-free code (64 B against 60). A store of a register reads the copy's
/// source, and the prologue pushes it.
#[test]
fn a_spill_of_a_copy_reads_the_copys_source() {
    unjoined();
    let ir = include_str!("kernels/ir/crc16.ll");
    let out = text(ir, "crc16", 0, &Request::new(Level::O2)).unwrap_or_else(|why| panic!("{why}"));
    assert!(!out.contains("(ix), b"), "{out}");
    let lines: Vec<&str> = out.lines().map(str::trim).collect();
    assert!(lines.windows(2).any(|pair| pair == ["push af", "push hl"]), "{out}");
}

/// Joined, the copies pinned an argument's pair to its last use: crc16 -O2 took
/// 64 B where 60 are enough, a `long long` could not be allocated. The default
/// convention states the other.
#[test]
fn the_default_convention_leaves_the_argument_copies_unjoined() {
    assert!(llrm_z80::abi::z80().pinned_copies_unjoined);
    assert!(llrm_z80::CALLING.contains("pinned_copies_unjoined = true"));
}
