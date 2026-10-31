//! `WebAssemblyFixBrTableDefaults`: a `br_table` comes with a dummy default
//! (its first case) and a guard block above it that range-checks the index.
//! Where the guard's check is the plain `i32.gt_u`, the guard's target becomes
//! the `br_table`'s default and the table is spliced into the guard's block.
//!
//! DEVIATE W9: `fixBrTableIndex` (the `i64` index of wasm64) is not translated;
//! a `BR_TABLE_I64` is left alone. Reason: wasm64 does not reach it. Exit:
//! wasm64.
//!
//! # See also
//! `WebAssemblyFixBrTableDefaults.cpp` (`fixBrTableDefault` L87), pin
//! d1106deb71cc.

use crate::branch::{analyze, remove};
use crate::dagisel::generated::ids::opcode as op;
use crate::mir::{Function, Inst, Operand, PHI};

/// Whether anything changed.
pub fn fix_br_table_defaults(f: &mut Function) -> bool {
    let n = f.blocks.len();
    // The opcode that defines each register: the guard's check is found through
    // it.
    let mut def_opcode = vec![None; f.num_vregs()];
    for inst in f.blocks.iter().flatten() {
        for reg in inst.defs() {
            def_opcode[reg as usize].get_or_insert(inst.opcode);
        }
    }
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (from, to) in f.succs.iter().enumerate() {
        for &block in to {
            preds[block].push(from);
        }
    }
    let mut removed = vec![false; n];
    for table in 0..n {
        let Some(last) = f.blocks[table].last() else { continue };
        if last.opcode != op::BR_TABLE_I32 || removed[table] {
            continue;
        }
        // `assert(MBB->pred_size() == 1)`: a table with another way in is left
        // as it is.
        let [header] = preds[table][..] else { continue };
        let Some(guard) = analyze(&f.blocks[header]) else { continue };
        if let Some(taken) = guard.taken.filter(|&taken| taken != table) {
            let Some((_, check)) = guard.cond else { continue };
            if def_opcode[check as usize] != Some(op::GT_U_I32) {
                continue;
            }
            let default = f.blocks[table].last_mut().and_then(|inst| inst.operands.last_mut()).expect("a default");
            *default = Operand::Block(taken as u32);
        }
        remove(&mut f.blocks[header]);
        let moved = std::mem::take(&mut f.blocks[table]);
        f.blocks[header].extend(moved);
        // The header skips the table's block: its successors are the table's,
        // and the PHIs that named the table's block name the header.
        let table_succs = std::mem::take(&mut f.succs[table]);
        f.succs[header].retain(|&to| to != table && !table_succs.contains(&to));
        for &to in &table_succs {
            if !f.succs[header].contains(&to) {
                f.succs[header].push(to);
            }
            preds[to].retain(|&from| from != table);
            if !preds[to].contains(&header) {
                preds[to].push(header);
            }
            for inst in f.blocks[to].iter_mut().take_while(|inst| inst.opcode == PHI) {
                rename_pred(inst, table, header);
            }
        }
        removed[table] = true;
    }
    if !removed.contains(&true) {
        return false;
    }
    let kept: Vec<usize> = (0..n).filter(|&block| !removed[block]).collect();
    f.apply_order(&kept);
    true
}

fn rename_pred(
    phi: &mut Inst,
    from: usize,
    to: usize,
) {
    for operand in &mut phi.operands {
        if *operand == Operand::Block(from as u32) {
            *operand = Operand::Block(to as u32);
        }
    }
}
