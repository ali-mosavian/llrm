//! `has_fp` found the body's block for each way that reached
//! one by a search of all the blocks: N^2 steps in `assemble`'s own time,
//! x2.60 per doubling on the `branches` scaling row (#2260).

use std::sync::Arc;

use llrm_codegen::backend::framefree::{LOOKED, has_fp};
use llrm_codegen::model::lir::{Block, Insn, LirBlock, LirBody};
use llrm_codegen::support::hash::IndexMap;
use llrm_target::Target;

/// A chain of `blocks` blocks, each a `nop` jumping to the next.
fn chain(blocks: i64) -> LirBody {
    let nop = llrm_codegen::model::ir::Semantics::new(llrm_codegen::model::ir::Operation::Nothing);
    let all: Vec<Block> = (0..blocks)
        .map(|k| {
            let at = k * 0x10;
            let insns = vec![Arc::new(Insn::new(at, Some((at, at + 1)), Some(nop.clone()), vec![], vec![]))];
            let succ = if k == blocks - 1 { vec![] } else { vec![(k + 1) * 0x10] };
            Block::from(LirBlock { succ: succ.into(), ..LirBlock::new(at, insns) })
        })
        .collect();
    LirBody {
        bits: 32,
        registers: Some(llrm_codegen::backend::registerinfo::Regs(llrm_x86_m32::M32.registers())),
        ..LirBody::new("chain", 0, all, IndexMap::default(), IndexMap::default())
    }
}

#[test]
fn test_each_block_is_looked_up_once_not_searched_for_by_every_way_into_it() {
    let registers = llrm_x86_m32::M32.frame_registers();
    let looked = |blocks: i64| {
        let body = chain(blocks);
        let before = LOOKED.with(std::cell::Cell::get);
        let kept = has_fp(&body, &registers, &Default::default(), false, false, false);
        assert!(!kept, "premise: a chain of nops needs no frame register");
        LOOKED.with(std::cell::Cell::get) - before
    };
    let (small, big) = (looked(500), looked(1000));
    assert!(big <= 2 * small + 4, "{small} blocks looked over for 500 blocks, {big} for 1000");
}
