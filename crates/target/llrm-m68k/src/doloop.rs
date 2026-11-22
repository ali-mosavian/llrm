//! A counted one-block loop's `addq #1, %dn; bne` is `dbf %dn, loop`.
//!
//! # How it works
//! [`doloops`] runs on the final layout. A block that ends in a step of
//! a data register by one and a `bne` back to itself, whose register starts
//! at a constant on every way in (the step walks it to zero), that nothing
//! else of the loop touches and nothing after it reads, and whose exit is the
//! next block with no reader of the step's codes, ends in `dbf` instead. The
//! register then starts at the trip count less one: `dbf` decrements first
//! and leaves the loop at -1.
//!
//! `dbf` counts the low word only, so a loop of more than 65536 trips is not
//! made one; its
//! displacement is 16 bits, so a block of more than [`MOST_INSTS`]
//! instructions is not either. The codes `addq` set are not set: no reader.
//!
//! DEVIATE M68K-DOLOOP: see [`crate::dagisel::dbf`]. Mirrors
//! `doloop_optimize` of GCC's `loop-doloop.cc` (`doloop_valid_p`, the
//! iteration count a constant, the counter dead after the loop); LLVM's
//! `HardwareLoops` is the IR form of the same transform.

use llrm_spike_mf::regalloc::successors;
use llrm_spike_mf::{Inst, Kind, Operand};

use crate::dagisel::hooks::Cpu;
use crate::dagisel::{dbf, local};
use crate::mfunc::{MFunction, op_inst};
use crate::narrow_add::{read_after_with, root};
use crate::regalloc::Assignment;
use crate::{REGISTER_INFO, reg};

/// The most instructions of a loop block: each is at most 10 bytes, so the
/// displacement stays inside 16 bits.
pub const MOST_INSTS: usize = 3000;

/// The trip count a word step of `kind` walks from its start, and the start's
/// sign: `addq.w #1` from -n, `subq.w #1` from n.
fn unit_step(inst: &Inst) -> Option<i64> {
    let [Operand::Def(_), Operand::Use(_), Operand::Imm(1)] = &inst.operands[..] else { return None };
    match inst.kind {
        Kind::Op("ADDQ16di" | "ADDQ32di") => Some(1),
        Kind::Op("SUBQ16di" | "SUBQ32di") => Some(-1),
        _ => None,
    }
}

/// Whether the codes at the top of `block` are dead: read by none before one
/// sets them, or the block returns.
fn codes_dead_on_entry(insts: &[Inst]) -> bool {
    for inst in insts {
        if inst.reads_flags() && !inst.call {
            return false;
        }
        if inst.sets_flags() || inst.call {
            return true;
        }
    }
    insts.last().is_some_and(|last| last.kind == Kind::Op("RET"))
}

/// Whether the function's end reads `register`: only a result register does
/// (the saved ones are restored by the epilogue).
fn is_result(register: llrm_lir::registers::RegId) -> bool {
    [reg::D0, reg::D1].iter().any(|result| REGISTER_INFO.root(*result) == register)
}

/// Makes each sound counted loop a `dbf` loop; returns how many.
pub fn doloops(
    function: &mut MFunction,
    assignment: &Assignment,
    cpu: Cpu,
    returns_value: bool,
) -> usize {
    // priced per CPU by the manuals' tables: `dbf` taken against `addq.w` and a taken `bne` is 10 against 14 clocks
    // (68000, 68010), 6 against 8 (68020, 68030, but 4 more at the exit); the 68040 (3 against 3) and the 68060 (1
    // against 1) gain nothing
    if cpu > Cpu::M68030 {
        return 0;
    }
    let fewest = if cpu >= Cpu::M68020 { 3 } else { 1 };
    let Some(desc) = local::code_of(dbf::NAME).and_then(local::desc) else { return 0 };
    let following = successors(function);
    let mut predecessors = vec![Vec::new(); function.blocks.len()];
    for (from, targets) in following.iter().enumerate() {
        for &to in targets {
            predecessors[to].push(from);
        }
    }
    let mut made = 0;
    for block in 0..function.blocks.len() {
        let body = &function.blocks[block].insts;
        let [.., step, branch] = &body[..] else { continue };
        if body.len() > MOST_INSTS || branch.kind != Kind::Op("Bne8") {
            continue;
        }
        let [Operand::Block(target)] = branch.operands.iter().filter(|o| matches!(o, Operand::Block(_))).cloned().collect::<Vec<_>>()[..]
        else {
            continue;
        };
        if target != function.blocks[block].id || !following[block].contains(&block) {
            continue;
        }
        let Some(direction) = unit_step(step) else { continue };
        let [Operand::Def(def), Operand::Use(base), _] = &step.operands[..] else { continue };
        let counter = (def.clone(), base.clone());
        let register = root(assignment, def);
        if register != root(assignment, base) || !crate::flags::is_data(register) {
            continue;
        }
        // the exit is the next block, and the codes the step set are dead there
        let leaves: Vec<usize> = following[block].iter().copied().filter(|&next| next != block).collect();
        if leaves != [block + 1] || !codes_dead_on_entry(&function.blocks[block + 1].insts) {
            continue;
        }
        // nothing else of the loop touches the counter or calls
        let touches = |one: &Inst| {
            one.operands.iter().any(|operand| match operand {
                Operand::Use(reg) | Operand::Def(reg) => root(assignment, reg) == register,
                _ => false,
            })
        };
        let at = body.len() - 2;
        if body.iter().enumerate().any(|(other, one)| (touches(one) && other != at) || one.call) {
            continue;
        }
        // every way in starts it at a constant the step walks to zero
        let entries: Vec<usize> = predecessors[block].iter().copied().filter(|&p| p != block).collect();
        if entries.is_empty() {
            continue;
        }
        let mut starts = Vec::new();
        for &entry in &entries {
            let start = function.blocks[entry].insts.iter().enumerate().rev().find(|(_, one)| {
                one.call
                    || one.operands.iter().any(|operand| matches!(operand, Operand::Def(reg) if root(assignment, reg) == register))
            });
                let Some((index, Inst { kind: Kind::Op("MOV32ri" | "MOVI32ri"), operands, .. })) = start else { break };
            let [Operand::Def(_), Operand::Imm(k)] = operands[..] else { break };
            // -n for an `addq`, n for a `subq`: 1 to 65536 trips, the most a word counter runs
            if k == 0 || k.abs() > 0x10000 || k.abs() < fewest || (k > 0) != (direction < 0) {
                break;
            }
            starts.push((entry, index, k));
        }
        if starts.len() != entries.len() || read_after_with(function, assignment, &following, register, &leaves, returns_value && is_result(register), true) {
            continue;
        }
        for (entry, index, k) in starts {
            let trips = k.abs();
            let row = if (trips - 1) <= 127 { "MOVI32ri" } else { "MOV32ri" };
            let Some(move_desc) = local::code_of(row).and_then(local::desc) else { continue };
            let operands = vec![function.blocks[entry].insts[index].operands[0].clone(), Operand::Imm(trips - 1)];
            function.blocks[entry].insts[index] = op_inst(row, move_desc, operands);
        }
        let insts = &mut function.blocks[block].insts;
        insts.truncate(insts.len() - 2);
        insts.push(op_inst(
            dbf::NAME,
            desc,
            vec![Operand::Def(counter.0), Operand::Use(counter.1), Operand::Block(target)],
        ));
        made += 1;
    }
    made
}
