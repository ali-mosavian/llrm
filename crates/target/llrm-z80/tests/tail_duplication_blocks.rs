//! A returning tail copied into a predecessor that branches elsewhere first stays a block of its own.

use llrm_z80::compile::tail_duplicate_returns;
use llrm_z80::mfunc::{Block, Inst, Kind, MFunction, Operand};

fn terminator(
    name: &'static str,
    target: Option<u32>,
    barrier: bool,
) -> Inst {
    let mut inst = Inst::new(Kind::Op(name), target.map(|id| vec![Operand::Block(id)]).unwrap_or_default());
    inst.terminator = true;
    inst.barrier = barrier;
    inst
}

/// `jp nz, 2; jp 1` with block 1 a call and `ret` (`sibling_call_callees`' `g`): the copy went after the `jp nz` in
/// the same block, a branch in the middle that branch folding cannot see (its guard fires on three tests).
#[test]
fn a_returning_tail_copied_after_a_conditional_branch_is_a_block_of_its_own() {
    let call = Inst::new(Kind::Op("CALL"), vec![Operand::Symbol("h".to_owned(), 0)]);
    let mut f = MFunction::new("g");
    f.blocks = vec![
        Block { id: 0, insts: vec![terminator("JPNZ", Some(2), false), terminator("JP", Some(1), true)], phis: Vec::new() },
        Block { id: 1, insts: vec![call, terminator("RET", None, true)], phis: Vec::new() },
        Block { id: 2, insts: vec![terminator("RET", None, true)], phis: Vec::new() },
    ];
    tail_duplicate_returns(&mut f);
    for block in &f.blocks {
        let first = block.insts.iter().position(|inst| inst.terminator);
        let after = first.is_some_and(|at| block.insts[at..].iter().any(|inst| !inst.terminator));
        assert!(!after, "block {} has an instruction after a branch: {:?}", block.id, block.insts);
    }
    let kinds: Vec<Vec<Kind>> = f.blocks.iter().map(|block| block.insts.iter().map(|inst| inst.kind).collect()).collect();
    assert!(kinds.iter().any(|block| block == &[Kind::Op("CALL"), Kind::Op("RET")]), "the copy is gone: {kinds:?}");
}
