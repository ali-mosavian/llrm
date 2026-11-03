//! The z80 with `pinned_copies_unjoined` stated in its convention: the
//! allocator leaves the copies from the argument registers to itself, as the
//! pin does. A file of its own, so that the convention is stated before
//! anything parses it.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

fn unjoined() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let stated = llrm_z80::CALLING.replacen("[z80]\n", "[z80]\npinned_copies_unjoined = true\n", 1);
        llrm_z80::abi::use_calling(&stated).expect("stated before the first use");
    });
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
