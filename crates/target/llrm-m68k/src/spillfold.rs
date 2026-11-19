//! A spilled value an instruction reads is read from its frame slot: `add.b
//! (3,%sp), %d1` for `move.l (0,%sp), %d0; add.b %d0, %d1` (2 B and 4 clk
//! less).
//!
//! # How it works
//! The allocator asks [`fold`] (`Target::fold_spill`, the shared
//! `InlineSpiller::foldMemoryOperand`) for the form of an instruction that
//! reads one spilled register from memory: the register-source rows of `add`,
//! `sub`, `and`, `or` and `cmp` take their `p` twin, a compare of a spilled
//! register with an immediate (`cmpi`) and a `tst` take the memory row. A spill
//! slot holds the whole 32-bit register, so a byte or word reads the low bytes
//! of it: the displacement is `4 - width/8` (big-endian).
//!
//! DEVIATE SPILL-FOLD-M68K: the pin's `M68kInstrInfo` has no
//! `foldMemoryOperandImpl`, so its spiller reloads and then uses. Reason: best
//! of gcc and clang is the goal; both read the slot in place. Owner: m68k.
//! Exit: none.
//!
//! A spilled `cmp` source folds only on the source side: the destination of
//! `cmp` must be a data register.

use llrm_spike_mf::{Inst, Kind, Operand, Reg};

use crate::dagisel::local;
use crate::mfunc::op_inst;

thread_local! {
    static ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `run` with the fold on or off for the allocator it calls
/// (`Options::spill_fold`; `LLRM_NO_SPILL_FOLD` turns it off).
pub fn scoped<R>(
    on: bool,
    run: impl FnOnce() -> R,
) -> R {
    let before = ON.with(|one| one.replace(on));
    let result = run();
    ON.with(|one| one.set(before));
    result
}

/// Whether `function` has an instruction that reads a spill slot in place,
/// rather than by the plain reload.
pub fn has_folded_reads(function: &crate::mfunc::MFunction) -> bool {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .any(
            |inst| inst.may_load
                && !matches!(inst.kind, Kind::Op("MOVM32mp_P"))
                && inst.operands.iter().any(|operand| {
                    matches!(
                        operand,
                        Operand::Frame(slot) if !function.fixed.contains_key(slot)
                    )
                }),
        )
}

/// The memory row `name` becomes when the register at operand `at` is read from
/// memory, and the operand list of that row before the slot is placed.
fn twin(
    name: &str,
    at: usize,
) -> Option<(String, bool)> {
    let (stem, tail) = name.split_at(name.find(|c: char| c.is_ascii_digit())?);
    let bits = tail.trim_end_matches(char::is_alphabetic);
    let shape = &tail[bits.len()..];
    if !matches!(bits, "8" | "16" | "32") {
        return None;
    }
    match (stem, shape) {
        // the register source of a two-address row
        ("ADD" | "SUB" | "AND" | "OR", "dd" | "dr") | ("CMP", "dd" | "dr") => {
            let source = usize::from(local::text(name)?.2.first()?.at);
            (at == source).then(|| (format!("{stem}{bits}dp"), true))
        }
        // `cmpi` of the register and `tst` of it: the register is the memory
        // operand
        ("CMP", "di") | ("TST", "d") => {
            let memory = if shape == "di" { "pi" } else { "p" };
            Some((format!("{stem}{bits}{memory}"), false))
        }
        _ => None,
    }
}

/// `inst` with its read of `register`, its only mention, taken from frame
/// `slot`; `None` where there is no such form.
pub fn fold(
    inst: &Inst,
    register: Reg,
    slot: i32,
) -> Option<Inst> {
    let Kind::Op(name) = inst.kind else { return None };
    if !ON.with(std::cell::Cell::get) {
        return None;
    }
    let mentioned = |operand: &Operand| matches!(operand, Operand::Def(r) | Operand::Use(r) if *r == register);
    if inst.operands.iter().filter(|operand| mentioned(operand)).count() != 1 {
        return None;
    }
    let at = inst.operands.iter().position(|operand| matches!(operand, Operand::Use(r) if *r == register))?;
    let (row, source_side) = twin(name, at)?;
    let bits: i64 = row
        .trim_start_matches(|c: char| c.is_ascii_alphabetic())
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let displacement = 4 - bits / 8;
    let code = local::code_of(&row)?;
    let row = local::name_of(u32::from(code))?;
    let mut operands = inst.operands.clone();
    if source_side {
        // `add.b (d,%sp), %d1`: the memory operand sits where the twin's asm
        // names it
        let source = usize::from(local::text(inst_name(inst)?)?.2.first()?.at);
        operands.remove(source);
        let memory = usize::from(local::text(row)?.2.first()?.at);
        operands.splice(memory..memory, [Operand::Imm(displacement), Operand::Frame(slot)]);
    } else {
        operands.splice(at..=at, [Operand::Imm(displacement), Operand::Frame(slot)]);
    }
    let mut folded = op_inst(row, local::desc(code)?, operands);
    folded.origin = inst.origin;
    folded.may_load = true;
    Some(folded)
}

fn inst_name(inst: &Inst) -> Option<&'static str> {
    match inst.kind {
        Kind::Op(name) => Some(name),
        _ => None,
    }
}
