//! Copy propagation over a straight-line stretch of 6502 code: LLVM's
//! `MachineCopyPropagation`, on `a`, `x`, `y` and the zero-page and spill
//! cells.
//!
//! # How it works
//! Each unit (a register or a cell) holds a value number; a copy gives its
//! destination the number of its source. Walking a block forward, a read of a
//! unit reads another unit that holds the same value when that is cheaper or
//! lets the copy die: `lda c` is `txa` when `x` holds `c`, `lda t` is `lda s`
//! when `s` was the copy's source, and a load or transfer of a value the
//! register holds already goes. A copy that nothing reads afterwards is then
//! the job of `peephole_cells::drop_dead_stores`.
//!
//! ```text
//!   sta rc2; stx rc3; sta rc4;    sta rc2; stx rc3; sta rc4;
//!   lda rc3; sta rc5; lda rc4  => txa; sta rc5; lda rc2        (rc4 is now dead)
//! ```
//!
//! DEVIATE MCP1 (owner alloc-small, exit 2026-12-15): the pin asks the register
//! class whether a copy's source may replace its destination
//! (`isForwardableRegClassCopy`, MachineCopyPropagation.cpp#L706). The 6502 has
//! no classes: the question is which opcodes take which operand form (a memory
//! operand of an ALU instruction, `lda`, `ldx` and `ldy` is any cell; a
//! register never is), and a substitute cell must be zero page when the old
//! one is (`same_form`). Evidence: `tests/copyprop.rs`; exit when the allocator
//! states operand forms as classes.
//!
//! DEVIATE MCP2 (owner alloc-small, exit 2026-12-15): a copy sets the zero and
//! sign flags on the 6502. A transfer or load that is removed must leave the
//! flags unread (`peephole::flags_unread_after`); a rewrite to another opcode
//! that sets the same flags from the same value (`lda c` to `txa`) changes
//! none. The pin's copies are flag-free.
//!
//! DEVIATE MCP3 (owner alloc-small, exit 2026-12-15): a store through an index
//! or a pointer may write any cell, a call any register and any caller-saved
//! cell, a label or a raw line starts a new block, a jump or return ends one:
//! all end every tracked value (the pin has regmasks and walks one block).
//!
//! # Where it runs
//! After allocation, as the pin does twice: after the register rewrite
//! ([`addOptimizedRegAlloc`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1546))
//! and last in the late optimizations
//! ([`addMachineLateOptimization`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetPassConfig.cpp#L1575)).
//! Here it is one run in `peephole::run_with`, after the shape-specific
//! peepholes and before `drop_dead_stores`: earlier, it hid the copies those
//! peepholes match (`shift_seven` 2 tests, `hand_shrax7` +4 B). Dead writes go
//! first (`MaybeDeadCopies`), else a dead `lda` is kept alive as a transfer's
//! source. On by default; `LLRM_6502_MCP=0` is the kill switch ([`enabled`]).
//!
//! Hosting: `backend/machinecp.rs` is the generic translation of the same pass,
//! but only `flow::machine_with` runs it and the 6502 emits `Asm` without going
//! through that flow, so there is no allocated `LirBody` to run it on. Not
//! checked: its byte-lane tracker against A/X/Y, the zero-page cells and the
//! zero/sign flag rule (MCP2). At 6502-a's allocated-`LirBody` POC (2026-11-20,
//! window to about 11-27) check hosting there, then migrate or re-justify MCP1-3.
//!
//! # Not subsumed
//! With the pass on, skipping a special-case peephole (162 cells, 2026-11-09)
//! makes cells larger: `load_straight_into_a` +19 B (lz4c, hand_memmove),
//! `coalesce_copy` +8 B (hand_shrax7), `fold_spill_copy` +8 B (hand_strncpy),
//! `store_then_index_load` +3 B (hand_shrax3); `drop_reloads`,
//! `drop_cancelling_transfers` and `forward_cell_copy` are equal there but
//! +10..13 executed programs larger per run (82 programs, both CPUs, O2/Os).
//! None is deleted; each exits with its own row.
//!
//! # See also
//! LLVM [`MachineCopyPropagation`]
//! (https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/MachineCopyPropagation.cpp#L926)

use std::collections::HashMap;
use std::ffi::OsStr;

use crate::asm::{Asm, Inst, Line, Op, Operand};
use crate::peephole::flags_unread_after;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Unit {
    A,
    X,
    Y,
    Cell(String),
}

/// The value number of each unit, and the step at which it got it.
#[derive(Default)]
struct Tracker {
    next: u32,
    held: HashMap<Unit, (u32, u32)>,
}

impl Tracker {
    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next
    }

    fn value(
        &mut self,
        unit: &Unit,
    ) -> u32 {
        if let Some((value, _)) = self.held.get(unit) {
            return *value;
        }
        let value = self.fresh();
        let at = self.fresh();
        self.held.insert(unit.clone(), (value, at));
        value
    }

    /// `unit` now holds `value`, written just now.
    fn set(
        &mut self,
        unit: Unit,
        value: u32,
    ) {
        let at = self.fresh();
        self.held.insert(unit, (value, at));
    }

    /// `unit` holds something nobody else does.
    fn kill(
        &mut self,
        unit: Unit,
    ) {
        let value = self.fresh();
        self.set(unit, value);
    }

    fn forget_cells(&mut self) {
        self.held.retain(|unit, _| !matches!(unit, Unit::Cell(_)));
    }

    fn forget_all(&mut self) {
        self.held.clear();
    }

    /// The cell, other than `not`, that holds `value` and got it before `not`
    /// did, and takes the same operand form.
    fn earlier_cell(
        &self,
        value: u32,
        not: &str,
    ) -> Option<String> {
        let born = self.held.get(&Unit::Cell(not.to_owned()))?.1;
        self.held
            .iter()
            .filter_map(|(unit, (v, at))| match unit {
                Unit::Cell(name) if *v == value && name != not && *at < born && same_form(name, not) => {
                    Some((*at, name.clone()))
                }
                _ => None,
            })
            .min()
            .map(|(_, name)| name)
    }

    fn register_holding(
        &self,
        value: u32,
        among: &[Unit],
    ) -> Option<Unit> {
        among.iter().find(|unit| self.held.get(unit).is_some_and(|(v, _)| *v == value)).cloned()
    }
}

/// Both zero-page registers, or both not: the same operand form and size.
fn same_form(
    a: &str,
    b: &str,
) -> bool {
    Operand::Direct(a.to_owned()).zero_page() == Operand::Direct(b.to_owned()).zero_page()
}

fn cell_of(operand: &Operand) -> Option<String> {
    match operand {
        Operand::Direct(name) if operand.is_cell() => Some(name.clone()),
        _ => None,
    }
}

/// The instructions that read their memory operand and write nothing to it.
fn reads_cell(op: Op) -> bool {
    use Op::*;
    matches!(op, Lda | Ldx | Ldy | Adc | Sbc | And | Ora | Eor | Cmp | Cpx | Cpy | Bit)
}

/// Whether the pass runs, given the value of `LLRM_6502_MCP`: on unless it is `0`.
pub fn enabled(setting: Option<&OsStr>) -> bool {
    setting.is_none_or(|value| value != "0")
}

/// Forward copy propagation of `asm`.
pub fn copy_propagation(asm: &mut Asm) {
    // The pin deletes a copy nothing reads as it walks (`MaybeDeadCopies`): a dead
    // load into `a` must not be kept alive as the source of a transfer.
    crate::peephole::drop_dead_writes(asm);
    let mut out: Vec<Line> = Vec::with_capacity(asm.lines.len());
    let mut track = Tracker::default();
    let lines = std::mem::take(&mut asm.lines);
    for (at, line) in lines.iter().enumerate() {
        let Line::Inst(inst) = line else {
            track.forget_all();
            out.push(line.clone());
            continue;
        };
        let mut inst = inst.clone();
        // what the rest of the block (from `lines`, as it stands) does with the flags
        let flags_dead = || flags_unread_after(&lines, at + 1);
        match (inst.op, cell_of(&inst.operand)) {
            (Op::Lda, Some(c)) => {
                let v = track.value(&Unit::Cell(c.clone()));
                if track.value(&Unit::A) == v {
                    if flags_dead() {
                        continue;
                    }
                } else if let Some(r) = track.register_holding(v, &[Unit::X, Unit::Y]) {
                    inst = Inst::new(if r == Unit::X { Op::Txa } else { Op::Tya }, Operand::None);
                } else if let Some(s) = track.earlier_cell(v, &c) {
                    inst.operand = Operand::Direct(s);
                }
                track.set(Unit::A, v);
            }
            (Op::Ldx | Op::Ldy, Some(c)) => {
                let (this, other_load) = if inst.op == Op::Ldx { (Unit::X, Op::Tax) } else { (Unit::Y, Op::Tay) };
                let v = track.value(&Unit::Cell(c.clone()));
                if track.value(&this) == v {
                    if flags_dead() {
                        continue;
                    }
                } else if track.value(&Unit::A) == v {
                    inst = Inst::new(other_load, Operand::None);
                } else if let Some(s) = track.earlier_cell(v, &c) {
                    inst.operand = Operand::Direct(s);
                }
                track.set(this, v);
            }
            (Op::Sta | Op::Stx | Op::Sty, Some(c)) => {
                let src = match inst.op {
                    Op::Sta => Unit::A,
                    Op::Stx => Unit::X,
                    _ => Unit::Y,
                };
                let v = track.value(&src);
                if track.value(&Unit::Cell(c.clone())) == v {
                    continue;
                }
                track.set(Unit::Cell(c), v);
            }
            (op, Some(c)) if reads_cell(op) => {
                // adc, sbc, and, ora, eor, cmp, cpx, cpy, bit: read another cell holding the value
                let v = track.value(&Unit::Cell(c.clone()));
                if let Some(s) = track.earlier_cell(v, &c) {
                    inst.operand = Operand::Direct(s);
                }
                match op {
                    Op::Adc | Op::Sbc | Op::And | Op::Ora | Op::Eor => track.kill(Unit::A),
                    _ => {}
                }
            }
            (Op::Tax | Op::Tay | Op::Txa | Op::Tya, _) => {
                let (from, to) = match inst.op {
                    Op::Tax => (Unit::A, Unit::X),
                    Op::Tay => (Unit::A, Unit::Y),
                    Op::Txa => (Unit::X, Unit::A),
                    _ => (Unit::Y, Unit::A),
                };
                let v = track.value(&from);
                if track.value(&to) == v && flags_dead() {
                    continue;
                }
                track.set(to, v);
            }
            _ => step(&mut track, &inst),
        }
        out.push(Line::Inst(inst));
    }
    asm.lines = out;
}

/// What an instruction that is not a copy does to the tracked values.
fn step(
    track: &mut Tracker,
    inst: &Inst,
) {
    use Op::*;
    match inst.op {
        Clc | Sec | Cmp | Cpx | Cpy | Bit | Nop | Pha | Phx | Phy | Php | Cld | Sed | Clv => {}
        Lda | Adc | Sbc | And | Ora | Eor | Pla => track.kill(Unit::A),
        Ldx | Plx | Inx | Dex | Tsx => track.kill(Unit::X),
        Ldy | Ply | Iny | Dey => track.kill(Unit::Y),
        Asl | Lsr | Rol | Ror | Inc | Dec => match &inst.operand {
            Operand::Acc | Operand::None => track.kill(Unit::A),
            operand => match cell_of(operand) {
                Some(c) => track.kill(Unit::Cell(c)),
                None => track.forget_cells(),
            },
        },
        Sta | Stx | Sty | Stz => match cell_of(&inst.operand) {
            Some(c) => track.kill(Unit::Cell(c)),
            // a global named directly is no cell; anything indexed or through a pointer may be
            None if matches!(inst.operand, Operand::Direct(_)) => {}
            None => track.forget_cells(),
        },
        // a branch falls through with what it had
        op if op.is_branch() => {}
        _ => track.forget_all(),
    }
}
