//! `branch_fold` hoisting over the 6502's allocator target.

use llrm_6502::alloc::{Class, Mos};
use llrm_6502::reg;
use llrm_spike_mf::regalloc::Assignment;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Reg, branch_fold_hoisting};

fn op(
    name: &'static str,
    operands: Vec<Operand>,
) -> Inst {
    Inst::new(Kind::Op(name), operands)
}

fn copy(
    to: u32,
    from: u32,
) -> Inst {
    Inst::new(Kind::Copy, vec![Operand::Def(Reg::Virt(to)), Operand::Use(Reg::Virt(from))])
}

fn ret() -> Inst {
    let mut inst = op("RTS", vec![]);
    inst.terminator = true;
    inst.barrier = true;
    inst
}

/// Both ways of `cmp; (reads the flags); beq` begin with the same copy. The
/// copy into `x` is a `tax`, which sets N and Z: run above the branch it
/// replaced the compare's flags, and `beq` took the copy's. It stays below.
#[test]
fn a_copy_into_a_register_stays_below_the_branch_that_reads_the_compare() {
    let mut compare = op("CMP", vec![Operand::Use(Reg::Virt(0))]);
    compare.sets_flags = true;
    let mut set = op("SET", vec![Operand::Def(Reg::Virt(3))]);
    set.reads_flags = true;
    let mut branch = op("BEQ", vec![Operand::Block(2)]);
    branch.terminator = true;
    branch.reads_flags = true;
    let mut function = MFunction::new("f");
    let blocks = [vec![compare, set, branch], vec![copy(1, 0), ret()], vec![copy(2, 0), ret()]];
    for (id, insts) in blocks.into_iter().enumerate() {
        function.blocks.push(Block { id: id as u32, insts, phis: vec![] });
    }
    function.classes = vec![Class::ANY; 8];
    let assignment = Assignment([reg::A, reg::X, reg::X, reg::Y].to_vec());
    branch_fold_hoisting::<Mos<true>>(&mut function, &assignment);
    let first: Vec<_> = function.blocks[0].insts.iter().map(|inst| inst.kind).collect();
    assert_eq!(first, [Kind::Op("CMP"), Kind::Op("SET"), Kind::Op("BEQ")]);
}
