//! A jump table of 16-bit offsets: `add.l %d0, %d0; move.w (table,%pc,%d0.l),
//! %d0; jmp (table,%pc,%d0.w)` with the table directly after the `jmp`, in
//! place of the pin's `lea (table,%pc), %a0; move.l (0,%a0,%d0*4), %a0; jmp
//! (%a0)` over 32-bit entries.
//!
//! # How it works
//! [`word_offset_tables`] runs after selection, before allocation. A block
//! that ends in the pin's table jump (`JMP32j`) over `lea table`, one load
//! and the shift of the index by 2, each read once and defined once, is
//! rewritten: the shift becomes the add of the index to itself, the load the
//! word load, the jump [`crate::dagisel::wordjump`]. The table is then 2 bytes
//! an entry, and the dispatch 2 bytes shorter before the 68020 (10 bytes
//! either way from it). The table is placed after the jump by
//! [`crate::compile`]; a function too big for 16-bit offsets keeps the pin's.
//!
//! DEVIATE M68K-WORD-TABLE: gcc's `casesi` (`m68k.md`) makes this form; the
//! pin's M68k keeps 32-bit entries (`EK_BlockAddress`). Owner x87-b. Exit:
//! 2026-11-24.

use llrm_spike_mf::{Kind, Operand, Reg};

use crate::dagisel::{local, wordjump};
use crate::mfunc::{Class, MFunction, op_inst};

/// A function of more instructions keeps 32-bit entries: 10 bytes an
/// instruction at most, so under the 32 KiB a signed offset reaches.
const MOST_INSTRUCTIONS: usize = 2500;

pub fn word_offset_tables(function: &mut MFunction) {
    if function.blocks.iter().map(|block| block.insts.len()).sum::<usize>() > MOST_INSTRUCTIONS {
        return;
    }
    let mut uses = vec![0u32; function.classes.len()];
    let mut defs = vec![0u32; function.classes.len()];
    for block in &function.blocks {
        for phi in &block.phis {
            if let Reg::Virt(v) = phi.result {
                defs[v as usize] += 1;
            }
            for (_, incoming) in &phi.incoming {
                if let Reg::Virt(v) = incoming {
                    uses[*v as usize] += 1;
                }
            }
        }
        for operand in block.insts.iter().flat_map(|inst| inst.operands.iter()) {
            match operand {
                Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
                Operand::Use(Reg::Virt(v)) => uses[*v as usize] += 1,
                _ => {}
            }
        }
    }
    let once = |v: u32| uses[v as usize] == 1 && defs[v as usize] == 1;
    for b in 0..function.blocks.len() {
        let insts = &function.blocks[b].insts;
        let Some(jump) = insts.len().checked_sub(1) else { continue };
        let (Kind::Op("JMP32j"), [Operand::Use(Reg::Virt(target)), Operand::Symbol(table, 0), blocks @ ..]) =
            (insts[jump].kind, &insts[jump].operands[..])
        else {
            continue;
        };
        let (target, table, blocks) = (*target, table.clone(), blocks.to_vec());
        let find = |def: u32| {
            insts[..jump].iter().position(|inst| inst.operands.first() == Some(&Operand::Def(Reg::Virt(def))))
        };
        let Some(load) = find(target).filter(|_| once(target)) else { continue };
        let [Operand::Def(_), Operand::Imm(0), Operand::Use(Reg::Virt(base)), Operand::Use(Reg::Virt(index))] =
            insts[load].operands[..]
        else {
            continue;
        };
        let (Kind::Op("MOV32rf"), true, true) = (insts[load].kind, once(base), once(index)) else { continue };
        let (Some(lea), Some(shift)) = (find(base), find(index)) else { continue };
        let (Kind::Op("LEA32q"), [_, Operand::Symbol(name, 0)]) = (insts[lea].kind, &insts[lea].operands[..]) else {
            continue;
        };
        let (Kind::Op("SHL32di"), [_, Operand::Use(source), Operand::Imm(2)]) =
            (insts[shift].kind, &insts[shift].operands[..])
        else {
            continue;
        };
        if *name != table {
            continue;
        }
        let source = *source;
        let (Some(add), Some(word), Some(jmp)) = (
            local::code_of("ADD32dd").and_then(local::desc),
            local::code_of("MOV16dk").and_then(local::desc),
            local::code_of(wordjump::NAME).and_then(local::desc),
        ) else {
            continue;
        };
        let doubled = function.classes.len() as u32;
        function.classes.push(Class::Dr32);
        let loaded = function.classes.len() as u32;
        function.classes.push(Class::Dr16);
        let insts = &mut function.blocks[b].insts;
        insts[shift] =
            op_inst("ADD32dd", add, vec![Operand::Def(Reg::Virt(doubled)), Operand::Use(source), Operand::Use(source)]);
        insts[load] = op_inst(
            "MOV16dk",
            word,
            vec![Operand::Def(Reg::Virt(loaded)), Operand::Symbol(table.clone(), 0), Operand::Use(Reg::Virt(doubled))],
        );
        let mut operands = vec![Operand::Symbol(table, 0), Operand::Use(Reg::Virt(loaded))];
        operands.extend(blocks);
        insts[jump] = op_inst(wordjump::NAME, jmp, operands);
        insts.remove(lea);
    }
}
