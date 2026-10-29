//! A value loaded from an incoming argument slot and read once, by an
//! instruction that has a memory form, is read from the slot by that
//! instruction: `move.l (12,%sp), %a1 ... cmp.l %a1, %d1` is `cmp.l (12,%sp),
//! %d1`. The slot is never written, so the read may happen where the value is
//! used, and the register and the load go.
//!
//! Outside a loop that is smaller and no slower (a memory `cmp` costs less than
//! a load and a register `cmp`). In a loop it costs clocks on every trip, so it
//! is done only where size is wanted ([`Tuning`]): `-Os`, or a function marked
//! `optsize` or `minsize` (the pin's `shouldOptForSize`).
//!
//! DEVIATE from the pin: M68k has no `foldMemoryOperandImpl`, and the pin's
//! selection folds a load only into a user in its own block. Reason: bytes and
//! a register on the 68000, for no extra instruction. Exit: never; a target
//! choice, off with [`crate::compile::Options::overlay`].

use std::collections::HashMap;

use llrm_spike_mf::cfg::Cfg;
use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::mfunc::MFunction;

/// The m68k entry of the size-versus-clocks table: how many clocks a byte
/// saved may cost on each trip of a loop where size is wanted. 0 is size
/// only, the pin's `-Os` meaning. A target with no entry takes the default,
/// which never folds into a loop.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    pub clocks_per_byte: u32,
}

pub const M68K: Tuning = Tuning { clocks_per_byte: 0 };

impl Tuning {
    /// Whether `bytes` saved are worth `clocks` lost on every trip.
    pub fn allows(
        &self,
        bytes: i64,
        clocks: i64,
    ) -> bool {
        self.clocks_per_byte == 0 || clocks <= bytes * i64::from(self.clocks_per_byte)
    }
}

/// `move.l (d,%sp), %rN` is 4 bytes and 16 clocks; the memory form of a `cmp`
/// or `add` is 2 bytes and about 12 clocks longer than the register form.
const BYTES_SAVED: i64 = 2;
const CLOCKS_LOST: i64 = 12;

/// The memory form of the register form `name`, and how to build its operands
/// from the register form's: the operand printed first is the one replaced by
/// the slot's displacement and base.
struct Fold {
    memory: &'static str,
    /// Operand number of the replaced register in the register form.
    source: usize,
    /// Where the displacement and base go in the memory form.
    at: usize,
}

fn fold_of(name: &str) -> Option<Fold> {
    let stem = name.strip_suffix('r')?;
    let &(_, reg_text, reg_operands) = crate::dagisel::asm::asm(name)?;
    let &(memory_name, _, memory_operands) = crate::dagisel::asm::asm(&format!("{stem}p"))?;
    let slot = memory_operands.iter().find(|one| one.method.starts_with("printARID") && one.method.ends_with("Mem"))?;
    // the operand the register form prints first
    let first = reg_text.find('$')? + 1;
    let word: String = reg_text[first..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    let source = reg_operands.iter().find(|one| one.name == word)?;
    Some(Fold { memory: memory_name, source: usize::from(source.at), at: usize::from(slot.at) })
}

/// A load of an incoming argument: `MOV32rp dst, 0, Frame(-n)`.
fn argument_load(inst: &Inst) -> Option<(u32, i32)> {
    match (inst.kind, &inst.operands[..]) {
        (Kind::Op("MOV32rp"), [Operand::Def(Reg::Virt(dst)), Operand::Imm(0), Operand::Frame(slot)]) if *slot < 0 => {
            Some((*dst, *slot))
        }
        _ => None,
    }
}

pub fn fold_argument_loads(
    function: &mut MFunction,
    in_loops: bool,
    tuning: Tuning,
) {
    let cfg = Cfg::new(function);
    let in_loops = in_loops && tuning.allows(BYTES_SAVED, CLOCKS_LOST);
    // every reader of each register
    let mut readers: HashMap<u32, Vec<(usize, usize)>> = HashMap::new();
    for (b, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            for operand in &inst.operands {
                if let Operand::Use(Reg::Virt(v)) = operand {
                    readers.entry(*v).or_default().push((b, i));
                }
            }
        }
    }
    let mut remove: Vec<(usize, usize)> = Vec::new();
    let mut rewrite: Vec<((usize, usize), Inst)> = Vec::new();
    for (b, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            let Some((dst, slot)) = argument_load(inst) else { continue };
            // follow single-reader copies to the one instruction that reads the
            // value
            let mut chain = vec![(b, i)];
            let mut value = dst;
            let user = loop {
                let Some([(ub, ui)]) = readers.get(&value).map(Vec::as_slice) else { break None };
                let reader = &function.blocks[*ub].insts[*ui];
                if let (Kind::Copy, [Operand::Def(Reg::Virt(next)), Operand::Use(_)]) =
                    (reader.kind, &reader.operands[..])
                {
                    chain.push((*ub, *ui));
                    value = *next;
                } else {
                    break Some((*ub, *ui));
                }
            };
            let Some((ub, ui)) = user else { continue };
            let reader = &function.blocks[ub].insts[ui];
            let Kind::Op(name) = reader.kind else { continue };
            let Some(fold) = fold_of(name) else { continue };
            // the replaced operand is the loaded value, and not one the result
            // is tied to
            let position = reader.operands.iter().position(|o| matches!(o, Operand::Use(Reg::Virt(v)) if *v == value));
            let Some(position) = position else { continue };
            if position != fold.source || reader.ties.iter().any(|&(_, used)| used == position) {
                continue;
            }
            if cfg.cycle_depth(ub) > cfg.cycle_depth(b) && !in_loops {
                continue;
            }
            let mut operands = reader.operands.clone();
            operands.remove(position);
            let at = fold.at.min(operands.len());
            operands.insert(at, Operand::Frame(slot));
            operands.insert(at, Operand::Imm(0));
            let mut folded = Inst { kind: Kind::Op(fold.memory), operands, ..reader.clone() };
            folded.may_load = true;
            debug_assert!(reader.ties.iter().all(|&(def, used)| def < at && used < at));
            remove.extend(chain);
            rewrite.push(((ub, ui), folded));
        }
    }
    for ((b, i), inst) in rewrite {
        function.blocks[b].insts[i] = inst;
    }
    remove.sort_unstable();
    remove.dedup();
    for (b, i) in remove.into_iter().rev() {
        function.blocks[b].insts.remove(i);
    }
}
