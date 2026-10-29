//! A value loaded from an incoming argument slot and read once or twice (a copy
//! chain to each reader), by instructions that have a memory form, is read from the
//! slot by those instructions: `move.l (12,%sp), %a1 ... cmp.l %a1, %d1` is `cmp.l (12,%sp),
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
const LOAD_BYTES: i64 = 4;
const FOLD_BYTES: i64 = 2;
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
    /// The displacement from the slot's start: the bytes before the part the instruction reads.
    offset: i64,
}

fn fold_of(name: &str) -> Option<Fold> {
    // `OPWWxy`: the operation, its width, the destination's mode and the source's. A source in
    // a register (`r`, or `d` for a data register) has its memory form with `p` there.
    let stem = name.strip_suffix('r').or_else(|| name.strip_suffix('d'))?;
    let digits: String = stem.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    let bits: i64 = digits.parse().ok()?;
    let tail = stem.trim_start_matches(|c: char| !c.is_ascii_digit()).trim_start_matches(|c: char| c.is_ascii_digit());
    if !matches!(bits, 8 | 16 | 32) || tail.len() != 1 {
        return None;
    }
    let &(_, reg_text, reg_operands) = crate::dagisel::asm::asm(name)?;
    let &(memory_name, _, memory_operands) = crate::dagisel::asm::asm(&format!("{stem}p"))?;
    let slot = memory_operands.iter().find(|one| one.method.starts_with("printARID") && one.method.ends_with("Mem"))?;
    // the operand the register form prints first
    let first = reg_text.find('$')? + 1;
    let word: String = reg_text[first..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
    let source = reg_operands.iter().find(|one| one.name == word)?;
    // the slot holds the whole 32-bit argument, big-endian: a narrower read is its last bytes
    Some(Fold { memory: memory_name, source: usize::from(source.at), at: usize::from(slot.at), offset: (32 - bits) / 8 })
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
    // a copy's destination with a second definition (a select's or a phi's) is not the loaded value
    let mut defined: HashMap<u32, usize> = HashMap::new();
    for block in &function.blocks {
        for inst in &block.insts {
            for operand in &inst.operands {
                if let Operand::Def(Reg::Virt(v)) = operand {
                    *defined.entry(*v).or_default() += 1;
                }
            }
        }
    }
    let mut remove: Vec<(usize, usize)> = Vec::new();
    let mut rewrite: Vec<((usize, usize), Inst)> = Vec::new();
    for (b, block) in function.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            let Some((dst, slot)) = argument_load(inst) else { continue };
            // Each reader of the value is the instruction that has the memory form or a
            // chain of single-reader copies to one. The load goes only when every reader
            // folds: a reader left behind would keep the register.
            let mut chain = vec![(b, i)];
            let mut users: Vec<(usize, usize)> = Vec::new();
            let mut folded: Vec<((usize, usize), Inst)> = Vec::new();
            let mut all = true;
            let mut starts: Vec<(u32, (usize, usize))> = Vec::new();
            for &(rb, ri) in readers.get(&dst).map(Vec::as_slice).unwrap_or(&[]) {
                starts.push((dst, (rb, ri)));
            }
            if starts.is_empty() {
                continue;
            }
            for (mut value, mut at) in starts {
                // follow single-reader copies to the one instruction that reads the value
                let user = loop {
                    let reader = &function.blocks[at.0].insts[at.1];
                    if let (Kind::Copy, [Operand::Def(Reg::Virt(next)), Operand::Use(_)]) =
                        (reader.kind, &reader.operands[..])
                    {
                        if defined.get(next) != Some(&1) {
                            break None;
                        }
                        chain.push(at);
                        value = *next;
                        let Some([next_at]) = readers.get(&value).map(Vec::as_slice) else { break None };
                        at = *next_at;
                    } else {
                        break Some(at);
                    }
                };
                let Some((ub, ui)) = user else {
                    all = false;
                    break;
                };
                let reader = &function.blocks[ub].insts[ui];
                let Kind::Op(name) = reader.kind else {
                    all = false;
                    break;
                };
                let Some(fold) = fold_of(name) else {
                    all = false;
                    break;
                };
                // the replaced operand is the loaded value, and not one the result
                // is tied to
                let position =
                    reader.operands.iter().position(|o| matches!(o, Operand::Use(Reg::Virt(v)) if *v == value));
                let Some(position) = position else {
                    all = false;
                    break;
                };
                if position != fold.source
                    || reader.ties.iter().any(|&(_, used)| used == position)
                    || users.contains(&(ub, ui))
                    || (cfg.cycle_depth(ub) > cfg.cycle_depth(b) && !in_loops)
                {
                    all = false;
                    break;
                }
                let mut operands = reader.operands.clone();
                operands.remove(position);
                let at = fold.at.min(operands.len());
                operands.insert(at, Operand::Frame(slot));
                operands.insert(at, Operand::Imm(fold.offset));
                let mut memory = Inst { kind: Kind::Op(fold.memory), operands, ..reader.clone() };
                memory.may_load = true;
                debug_assert!(reader.ties.iter().all(|&(def, used)| def < at && used < at));
                users.push((ub, ui));
                folded.push(((ub, ui), memory));
            }
            // The load is 4 bytes and each reader grows 2 in memory form (a `cmp`, or a `move`
            // to the outgoing area): two readers tie in bytes and free the register, three lose.
            if !all || users.len() as i64 * FOLD_BYTES > LOAD_BYTES {
                continue;
            }
            remove.extend(chain);
            rewrite.extend(folded);
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
