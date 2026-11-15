//! A sum of a byte and a constant that `a` already holds, re-associated.
//!
//! # How it works
//! A forward walk tracks what `a` holds as `source + k` (a register or cell
//! plus a constant, through `lda`, `txa`, `tya` and `adc #n`) and what the
//! carry is (known after `clc`, `sec`, and the fall-through of `bcs`/`bcc`
//! after a compare). Where `[clc;] load source; adc #c2` follows with `a` still
//! `source + k`, the load goes: `adc #(c2 - k)` adds to what `a` holds. The
//! carry the new add leaves is not the old one, so it is taken only where the
//! carry is dead after. A `clc` (`sec`) where the carry is known already goes.
//!
//! ```text
//!   clc; txa; adc #159; cmp #26; bcs L      clc; txa; adc #159; cmp #26; bcs L
//!   clc; txa; adc #224; tax            =>   adc #65; tax
//! ```
//!
//! Pass: LLVM's `DAGCombiner::visitADD` and `Reassociate` fold `(x + c1) + c2`
//! of one value. DEVIATE REASSOC (owner 6502, exit 2026-11-24): here the two
//! sums are separate nodes (the range test's `x - 97` and the fold's `x - 32`),
//! so the machine text is where one register holds both. Retired by:
//! translating `DAGCombiner::visitADD` / `Reassociate` at the DAG level, which
//! reuses the sibling `(add x, c1)` node.
//!
//! # Runs
//! In [`crate::peephole`]'s suffix, after copy propagation.

use llrm_support::hash::HashSet;

use crate::asm::{Asm, Inst, Line, Op, Operand, Registers};
use crate::peephole::carry_dead_from;

#[derive(Clone, PartialEq, Debug)]
enum Source {
    X,
    Y,
    Cell(String),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Carry {
    Unknown,
    Known(bool),
    /// Set by a compare: a branch on it settles it on the fall-through.
    Compare,
}

/// What `a` holds, as `source + k`, and the carry.
#[derive(Clone, PartialEq, Debug)]
struct State {
    a: Option<(Source, u8)>,
    carry: Carry,
}

const NOTHING: State = State { a: None, carry: Carry::Unknown };

fn immediate(inst: &Inst) -> Option<u8> {
    let Operand::Imm(text) = &inst.operand else { return None };
    let value = match text.strip_prefix('$') {
        Some(hex) => i64::from_str_radix(hex, 16).ok()?,
        None => text.parse::<i64>().ok()?,
    };
    (-256..256).contains(&value).then_some(value as u8)
}

/// What `inst` loads into `a` and leaves unchanged: a register or a cell.
fn loaded(inst: &Inst) -> Option<Source> {
    match (inst.op, &inst.operand) {
        (Op::Txa, _) => Some(Source::X),
        (Op::Tya, _) => Some(Source::Y),
        (Op::Lda, operand) if operand.is_cell() => {
            let Operand::Direct(name) = operand else { return None };
            Some(Source::Cell(name.clone()))
        }
        _ => None,
    }
}

fn writes_source(
    inst: &Inst,
    source: &Source,
) -> bool {
    match source {
        Source::X => crate::peephole::writes_register(inst, Registers::X),
        Source::Y => crate::peephole::writes_register(inst, Registers::Y),
        Source::Cell(name) => {
            let memory = matches!(
                inst.op,
                Op::Sta | Op::Stx | Op::Sty | Op::Stz | Op::Inc | Op::Dec | Op::Asl | Op::Lsr | Op::Rol | Op::Ror
            ) && !matches!(inst.operand, Operand::Acc | Operand::None);
            // an indexed or indirect store may reach any cell
            memory && !matches!(&inst.operand, Operand::Direct(n) if n != name)
        }
    }
}

/// The state after `inst`, from the state before it. An opcode not listed ends
/// what is known.
fn step(
    state: &State,
    inst: &Inst,
) -> State {
    let mut next = state.clone();
    let a_written = crate::peephole::writes_register(inst, Registers::A);
    match inst.op {
        Op::Clc => next.carry = Carry::Known(false),
        Op::Sec => next.carry = Carry::Known(true),
        Op::Cmp | Op::Cpx | Op::Cpy => next.carry = Carry::Compare,
        Op::Bcc | Op::Bcs => {
            if state.carry == Carry::Compare {
                next.carry = Carry::Known(inst.op == Op::Bcc);
            }
        }
        Op::Beq | Op::Bne | Op::Bmi | Op::Bpl | Op::Sta | Op::Stx | Op::Sty | Op::Stz | Op::Tax | Op::Tay | Op::Bit => {
        }
        Op::Lda
        | Op::Txa
        | Op::Tya
        | Op::Ldx
        | Op::Ldy
        | Op::Inx
        | Op::Iny
        | Op::Dex
        | Op::Dey
        | Op::And
        | Op::Ora
        | Op::Eor
        | Op::Pha
        | Op::Pla
        | Op::Phx
        | Op::Phy
        | Op::Plx
        | Op::Ply
        | Op::Nop => {}
        Op::Inc | Op::Dec if !matches!(inst.operand, Operand::Acc | Operand::None) => {}
        Op::Adc | Op::Sbc | Op::Asl | Op::Lsr | Op::Rol | Op::Ror | Op::Inc | Op::Dec => next.carry = Carry::Unknown,
        _ => return NOTHING,
    }
    if matches!(inst.op, Op::Adc) {
        next.a = match (&state.a, immediate(inst), state.carry) {
            (Some((source, k)), Some(n), Carry::Known(c)) => {
                Some((source.clone(), k.wrapping_add(n).wrapping_add(c as u8)))
            }
            _ => None,
        };
    } else if let Some(source) = loaded(inst) {
        next.a = Some((source, 0));
    } else if a_written {
        next.a = None;
    }
    if let Some((source, _)) = &next.a
        && writes_source(inst, source)
    {
        next.a = None;
    }
    next
}

/// `[clc|sec;] load; adc #c2` at the end of `out`, where `a` held
/// `source + k` before the pair: what to emit in its place.
fn rewrite(
    out: &[Line],
    before: &[State],
    adc: &Inst,
    carry_after_pair: Carry,
) -> Option<(usize, Vec<Inst>)> {
    let n = out.len();
    let inst_at = |at: usize| match out.get(at) {
        Some(Line::Inst(inst)) => Some(inst),
        _ => None,
    };
    let flag = |inst: &Inst| matches!(inst.op, Op::Clc | Op::Sec);
    let last = inst_at(n.checked_sub(1)?)?;
    let start = match inst_at(n.checked_sub(2)?) {
        Some(prev) if (loaded(last).is_some() && flag(prev)) || (flag(last) && loaded(prev).is_some()) => n - 2,
        _ if loaded(last).is_some() => n - 1,
        _ => return None,
    };
    let source = loaded(if flag(last) { inst_at(n - 2)? } else { last })?;
    let (held, k) = before[start].a.clone()?;
    if held != source {
        return None;
    }
    let Carry::Known(carry_in) = carry_after_pair else { return None };
    let c2 = immediate(adc)?;
    // the old sum was `source + c2 + carry_in`; `a` holds `source + k`
    let delta = c2.wrapping_sub(k).wrapping_add(carry_in as u8);
    let mut emit = Vec::new();
    let new_carry = match before[start].carry {
        Carry::Known(c) => c,
        _ => {
            emit.push(Inst { op: Op::Clc, operand: Operand::None, place: adc.place });
            false
        }
    };
    emit.push(Inst {
        op: Op::Adc,
        operand: Operand::Imm(delta.wrapping_sub(new_carry as u8).to_string()),
        place: adc.place,
    });
    Some((start, emit))
}

/// See the module docs.
pub fn reassociate_adds(asm: &mut Asm) {
    // a signed-overflow flag read after the add would see the new operands
    let reads_overflow =
        asm.lines.iter().any(|line| matches!(line, Line::Inst(Inst { op: Op::Bvc | Op::Bvs | Op::Php, .. })));
    if reads_overflow {
        return;
    }
    let lines = std::mem::take(&mut asm.lines);
    // a label only the line before falls into keeps what is known; a raw line
    // may name any
    let raw = lines.iter().any(|line| matches!(line, Line::Raw(_)));
    let targets: HashSet<&str> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Inst(inst) => inst.operand.name(),
            _ => None,
        })
        .collect();
    let mut out: Vec<Line> = Vec::with_capacity(lines.len());
    // the state before each line of `out`
    let mut before: Vec<State> = Vec::with_capacity(lines.len());
    let mut state = NOTHING;
    for (at, line) in lines.iter().enumerate() {
        match line {
            Line::Label(label) => {
                if raw || targets.contains(label.as_str()) {
                    state = NOTHING;
                }
                before.push(state.clone());
                out.push(line.clone());
            }
            Line::Raw(_) => {
                state = NOTHING;
                before.push(state.clone());
                out.push(line.clone());
            }
            Line::Inst(inst) => {
                // a carry the branch before settled needs no `clc`/`sec`
                if (inst.op == Op::Clc && state.carry == Carry::Known(false))
                    || (inst.op == Op::Sec && state.carry == Carry::Known(true))
                {
                    continue;
                }
                if inst.op == Op::Adc
                    && let Some((start, emit)) = rewrite(&out, &before, inst, state.carry)
                    && carry_dead_from(&lines, at + 1, &mut Default::default())
                {
                    let restart = before[start].clone();
                    out.truncate(start);
                    before.truncate(start);
                    state = restart;
                    for inst in emit {
                        before.push(state.clone());
                        state = step(&state, &inst);
                        out.push(Line::Inst(inst));
                    }
                    continue;
                }
                before.push(state.clone());
                state = step(&state, inst);
                out.push(line.clone());
            }
        }
    }
    asm.lines = out;
}
