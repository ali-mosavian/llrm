//! Liveness is computed without allocating per operation or per iteration.

use llrm_arm64::mc::inst::*;
use llrm_arm64::mc::visit::Reg;
use llrm_arm64::mfunc::{MBlock, MFunc, Op, branch_to};
use llrm_arm64::prepass::{cfg, live_in};
use llrm_support::alloc_count::{Counting, counted};

#[global_allocator]
static COUNTING: Counting = Counting;

fn add(
    rd: u32,
    rn: u32,
    rm: u32,
) -> Op {
    Op::I(Inst::AddSubShifted {
        op: AddSub { subtract: false, set_flags: false },
        width: Width::X,
        rd: Gpr(rd),
        rn: Gpr(rn),
        rm: Gpr(rm),
        shift: Shift::Lsl,
        amount: 0,
    })
}

/// Eight blocks of 25 additions in a chain that loops back: 200 operations.
fn chain() -> MFunc {
    let mut function = MFunc::new("f");
    function.next_register = 1000;
    for block in 0..8u32 {
        let mut ops: Vec<Op> =
            (0..25).map(|n| add(100 + block * 25 + n, 100 + (block * 25 + n).saturating_sub(1), 99)).collect();
        ops.push(branch_to(if block == 7 { "b0".into() } else { format!("b{}", block + 1) }.as_str()));
        function.blocks.push(MBlock { label: format!("b{block}"), ops: ops.into() });
    }
    function
}

/// `live_in` collected the defs and uses of every operation into two vectors
/// and rebuilt a hash set per block per round: 904 operation walks cost 1.3 M
/// of queens' 5 M instructions in the prepass, and every round of `sinkfold`
/// paid it again. This body made over 600 allocations.
#[test]
fn liveness_of_a_loop_allocates_per_block_not_per_operation() {
    let function = chain();
    let graph = cfg(&function);
    let (live, made) = counted(|| live_in(&function, &graph));
    std::hint::black_box(live);
    assert!(made <= 64, "{made} allocations for 200 operations in 8 blocks");
}

/// `defs_and_uses` built two vectors for every operation looked at (58 call
/// sites; 0.6 M of queens' prepass after liveness stopped using it): an
/// ordinary operation allocated twice, and the answer is unchanged.
#[test]
fn the_registers_of_an_ordinary_operation_are_listed_without_allocating() {
    let op = add(100, 101, 102);
    let ((defs, uses), made) = counted(|| op.defs_and_uses());
    assert_eq!(
        (defs.to_vec(), uses.to_vec()),
        (vec![Reg::Gpr(Gpr(100))], vec![Reg::Gpr(Gpr(101)), Reg::Gpr(Gpr(102))])
    );
    assert_eq!(made, 0, "{made} allocations for one operation");
}
