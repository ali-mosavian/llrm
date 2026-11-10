//! The allocator's view of an operation is made without formatting it.

use llrm_arm64::lir_view::{content, name_of};
use llrm_arm64::mc::inst::*;
use llrm_arm64::mfunc::Op;
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

fn add() -> Op {
    Op::I(Inst::AddSubShifted {
        op: AddSub { subtract: false, set_flags: false },
        width: Width::X,
        rd: Gpr(100),
        rn: Gpr(101),
        rm: Gpr(102),
        shift: Shift::Lsl,
        amount: 0,
    })
}

/// `name_of` formatted the instruction with `{:?}` and cut the variant out of
/// the text, and `content` hashed that text: 3.4 K instructions and four
/// allocations per operation, 0.8 M of queens' 58 M, twice per function.
#[test]
fn the_name_and_content_of_an_operation_allocate_nothing_once_seen() {
    let op = add();
    let Op::I(inst) = &op else { unreachable!() };
    assert_eq!(name_of(inst), "AddSubShifted", "the premise: the variant's name");
    let (_, made) = counted(|| {
        for _ in 0..50 {
            std::hint::black_box((name_of(inst), content(&op)));
        }
    });
    assert_eq!(made, 0, "{made} allocations for 50 operations");
}

/// `keyed` (the key of `cse` and of the hoist's value groups) was the debug
/// text of the operation, and flagcse's key was the same: 0.5 M of queens'
/// prepass and a string per operation. An operation is its own key.
#[test]
fn the_key_of_an_operation_allocates_nothing() {
    let op = add();
    let (key, made) = counted(|| llrm_arm64::prepass::keyed(&op));
    assert_eq!(made, 0, "{made} allocations for one key");
    assert_eq!(key, llrm_arm64::prepass::keyed(&op), "the premise: equal operations are one key");
    let Op::I(Inst::AddSubShifted { rd, .. }) = &op else { unreachable!() };
    let mut other = op.clone();
    if let Op::I(Inst::AddSubShifted { rd: slot, .. }) = &mut other {
        *slot = Gpr(rd.0 + 9);
    }
    assert_eq!(key, llrm_arm64::prepass::keyed(&other), "the destination is blanked: it does not make the key differ");
}
