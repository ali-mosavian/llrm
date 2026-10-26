//! `X86FixupLEAs`. Translated from
//! [X86FixupLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L232)
//! (`runOnMachineFunction`), function by function, one PR each (plan:
//! `docs/working/lea-fixup-plan.md`).
//!
//! # How it works
//! The pass visits every LEA of every block in order. On i486 none of
//! `slowLEA`, `slow3OpsLEA` or `leaUsesAG` is set, so only `optTwoAddrLEA`
//! (L564) runs on each: [`opt_two_addr_lea`], its cases 1 and 2 and then
//! `optLEAALU` as case 3.
//!
//! `optLEAALU` (L500, with `searchALUInst` L405) is [`opt_lea_alu`]: `lea
//! t,[x+y]` whose only reader, within five instructions, is `add d,t` or `sub
//! d,t` with the flags dead, is `op d,x` and `op d,y`. The adjacent case was
//! the peephole group `lea_alu`.
//!
//! Done here: [`is_lea`] (L227), the block loop, [`previous_instr`] (L295),
//! [`uses_register`] (L270) and [`search_backwards`] (L308), which finds the
//! write of a register within five cycles before an instruction;
//! [`inefficient_lea_base`] (L331-347); [`alu_names`], the pin's opcode maps
//! `getADDrrFromLEA` to `getINCDECFromLEA` (L351-400), which on our
//! name-keyed instructions are one table; and [`check_reg_usage`] (L466).
//!
//! # Example
//! `a_write_beyond_five_cycles_is_not_found` in this file's tests: two loads
//! cost 8 cycles, so a write of the register before them is past
//! `INSTR_DISTANCE_THRESHOLD` and `search_backwards` finds none for the LEA
//! that reads it.
//!
//! # Runs
//! After `FixupBW`, optimizing levels only.

use std::sync::Arc;

use iced_x86::RflagsBits;
use llrm_lir::registers::{RegId, Regs};

use crate::backend::lanes::Lanes;
use crate::backend::liveness;
use crate::backend::peep::guards::{free_fields, lea_cell};
use crate::backend::peep::{field, make};
use crate::backend::peephole::{_lanes, DeadAfter, id};
use crate::backend::regthrash::_dead_after;
use crate::model::ir::{AddressRef, Imm, Loc, Operation, Reg, Semantics};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

/// `INSTR_DISTANCE_THRESHOLD`: how far back a write still matters, in cycles.
const INSTR_DISTANCE_THRESHOLD: u32 = 5;

/// The CPU inputs of `runOnMachineFunction`: `slow3OpsLEA`, `slowIncDec`, and
/// whether the function is optimised for size.
#[derive(Default)]
pub struct FixupLeas {
    pub slow_3ops: bool,
    pub slow_incdec: bool,
    pub size: bool,
}

impl LIRTransform for FixupLeas {
    fn class_name(&self) -> &'static str {
        "FixupLEAs"
    }

    fn name(&self) -> &str {
        "fixupleas"
    }

    fn transform(
        &mut self,
        body: LirBody,
    ) -> Result<LirBody, String> {
        let (regs, bits) = (body.regs(), body.bits);
        let exits = liveness::dead_at_exit(&body);
        let mut out = body.clone();
        for block in &mut out.blocks {
            let dead = _dead_after(regs, bits, block, exits[&block.at].clone());
            let mut current = block.clone();
            let mut at = 0;
            // `runOnMachineFunction`: every LEA of the block, in order.
            while at < current.insns.len() {
                if is_lea(&current.insns[at]) {
                    let opt_inc_dec = !self.slow_incdec || self.size;
                    let rewritten = opt_two_addr_lea(regs, &current, at, &dead, opt_inc_dec).or_else(|| {
                        self.slow_3ops
                            .then(|| process_instr_for_slow_3op_lea(regs, &current, at, &dead, opt_inc_dec))
                            .flatten()
                    });
                    if let Some(rewritten) = rewritten {
                        current = current.with_insns(rewritten);
                    }
                }
                at += 1;
            }
            *block = current;
        }
        Ok(out)
    }
}

/// What a replaced LEA may carry (`exact` in `peephole.peep`).
const EXACT: u32 = field::CLOBBERS
    | field::CLOBBERS_HIGH
    | field::REQUIRES
    | field::DELIVERS
    | field::SPREAD
    | field::GROUP
    | field::SYMBOL
    | field::FRAME_ADJUST
    | field::SPILL_RELOAD
    | field::SPILL_STORE;

const REWRITABLE: u32 = field::CLOBBERS | field::REQUIRES | field::DELIVERS | field::SPREAD | field::GROUP;

fn arithmetic_flags() -> Lanes {
    Lanes::flags(RflagsBits::OF | RflagsBits::SF | RflagsBits::ZF | RflagsBits::AF | RflagsBits::CF | RflagsBits::PF)
}

/// `searchALUInst`: the `add d,t` or `sub d,t` after `lea t,..` that reads `t`
/// last, within five instructions, with the flags dead after it.
pub fn search_alu_inst(
    regs: Regs,
    block: &LirBlock,
    lea: usize,
    dead: &DeadAfter,
) -> Option<usize> {
    let Some(what) = block.insns[lea].what.as_ref() else { return None };
    let [Loc::Reg(t)] = what.dests.as_slice() else { return None };
    let mine = _lanes(regs, t.register);
    let names = alu_names();
    let mut distance = 1;
    for (cur, one) in block.insns.iter().enumerate().skip(lea + 1) {
        let what = one.what.as_ref()?;
        if matches!(what.op, Operation::Call | Operation::Escape) || distance > INSTR_DISTANCE_THRESHOLD {
            return None;
        }
        let overlaps = |loc: &Loc| match loc {
            Loc::Reg(reg) => !_lanes(regs, reg.register).is_disjoint(&mine),
            Loc::Mem(mem) => [mem.through, mem.index_through]
                .into_iter()
                .any(|r| r != RegId::None && !_lanes(regs, r).is_disjoint(&mine)),
            Loc::Address(address) => [address.through, address.index_through]
                .into_iter()
                .any(|r| r != RegId::None && !_lanes(regs, r).is_disjoint(&mine)),
            _ => false,
        };
        if what.dests.iter().any(|dest| matches!(dest, Loc::Reg(_)) && overlaps(dest)) {
            return None;
        }
        if what.dests.iter().chain(&what.sources).any(overlaps) {
            let name = what.name.as_deref()?;
            let ([Loc::Reg(d)], [Loc::Reg(first), Loc::Reg(second)]) = (what.dests.as_slice(), what.sources.as_slice())
            else {
                return None;
            };
            if (name != names.add && name != names.sub) || first != d || second != t || d.width != t.width {
                return None;
            }
            let after = dead.get(&id(one))?;
            // `isKill` on the read, and `registerDefIsDead(EFLAGS)`.
            return (mine.is_subset(after) && arithmetic_flags().is_subset(after)).then_some(cur);
        }
        distance += 1;
    }
    None
}

/// `optTwoAddrLEA`: the block's instructions with the LEA at `at` made an
/// `add`, `inc` or `dec` (cases 1 and 2) or, for case 3, `optLEAALU`'s two
/// steps; `None` where the pin leaves it. No segment, scale above 1 or symbol
/// displacement, and no flag read after it. `opt_inc_dec` is `OptIncDec`.
pub fn opt_two_addr_lea(
    regs: Regs,
    block: &LirBlock,
    at: usize,
    dead: &DeadAfter,
    opt_inc_dec: bool,
) -> Option<Vec<Arc<Insn>>> {
    let lea = &block.insns[at];
    let what = lea.what.as_ref()?;
    let ([Loc::Reg(d)], [source]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    let (symbol, base, index, scale, offset) = lea_cell(source)?;
    if symbol || scale > 1 || !arithmetic_flags().is_subset(dead.get(&id(lea))?) || !free_fields(lea, EXACT) {
        return None;
    }
    let made = |op: Operation, name: &str, sources: Vec<Loc>| {
        let mut what = what.clone();
        (what.op, what.name, what.sources) = (op, Some(name.into()), sources);
        let mut out = block.insns.iter().cloned().collect::<Vec<_>>();
        out[at] = Arc::new(Insn { what: Some(what), ..(**lea).clone() });
        Some(out)
    };
    let reg = |register: RegId| Loc::Reg(Reg { register, width: d.width });
    let names = alu_names();
    // Case 1: `lea r1,[r1+r2]` or `lea r1,[r2+r1]` is `add r1,r2`.
    if base != RegId::None && index != RegId::None && offset == 0 && (d.register == base || d.register == index) {
        let other = if d.register == base { index } else { base };
        return made(Operation::Binary, names.add, vec![Loc::Reg(*d), reg(other)]);
    }
    // Case 2: `lea r,[r+disp]` is `inc`, `dec` or `add r,disp`.
    if base == d.register && index == RegId::None {
        return match offset {
            1 | -1 if opt_inc_dec => {
                made(Operation::Unary, if offset == 1 { names.inc } else { names.dec }, vec![Loc::Reg(*d)])
            }
            _ => made(
                Operation::Binary,
                names.add,
                vec![Loc::Reg(*d), Loc::Imm(Imm { value: offset, width: d.width, address: None })],
            ),
        };
    }
    // Case 3.
    if base != RegId::None && index != RegId::None && offset == 0 {
        return opt_lea_alu(regs, block, at, dead);
    }
    None
}

/// `processInstrForSlow3OpLEA` (L752), for `slow-3ops-lea` CPUs (`x86-64`): a
/// three-operand LEA, or one with an inefficient base, as one or two
/// instructions; `None` where the pin leaves it. Deviations: no symbol
/// displacement (the pin adds it as an immediate), and the flags are dead by
/// the block's exact liveness where the pin scans four instructions
/// (`computeRegisterLiveness(.., 4)`).
pub fn process_instr_for_slow_3op_lea(
    regs: Regs,
    block: &LirBlock,
    at: usize,
    dead: &DeadAfter,
    opt_inc_dec: bool,
) -> Option<Vec<Arc<Insn>>> {
    let lea = &block.insns[at];
    let what = lea.what.as_ref()?;
    let ([Loc::Reg(d)], [source]) = (what.dests.as_slice(), what.sources.as_slice()) else { return None };
    let (symbol, base, index, scale, offset) = lea_cell(source)?;
    let three_ops = base != RegId::None && index != RegId::None && offset != 0;
    if symbol
        || !(three_ops || inefficient_lea_base(regs, base, index))
        || !arithmetic_flags().is_subset(dead.get(&id(lea))?)
        || !free_fields(lea, EXACT)
    {
        return None;
    }
    // LEA64_32r: 64-bit terms, a 32-bit result, added as the terms' low halves.
    let wide = |register: RegId| register != RegId::None && regs.bytes(register) == Some(8);
    let narrow = d.width == 4 && (wide(base) || wide(index));
    let low = |register: RegId| if narrow { regs.view(regs.root(register), 32).unwrap_or(register) } else { register };
    let term = |register: RegId| Loc::Reg(Reg { register: low(register), width: d.width });
    let same = |a: RegId, b: RegId| a != RegId::None && regs.root(a) == regs.root(b);
    let (is_scale1, ineff_base, ineff_index) =
        (scale == 1, inefficient_lea_reg(regs, base), inefficient_lea_reg(regs, index));
    let (dest_is_base, dest_is_index) = (same(d.register, base), same(d.register, index));
    let base_or_index_is_dst = dest_is_base || dest_is_index;
    if ineff_base && dest_is_base && !is_scale1 {
        return None;
    }
    let names = alu_names();
    let rebuilt =
        |what_for: &dyn Fn(&Semantics) -> Semantics| Arc::new(Insn { what: Some(what_for(what)), ..(**lea).clone() });
    let address = |through: RegId, index_through: RegId, scale: i64, offset: i64| {
        Loc::Address(AddressRef { through, index_through, scale, offset, ..AddressRef::new(None) })
    };
    let lea_of = |through: RegId, index_through: RegId, scale: i64, offset: i64| {
        rebuilt(&|what: &Semantics| Semantics {
            sources: vec![address(through, index_through, scale, offset)],
            ..what.clone()
        })
    };
    let alu = |op: Operation, name: &'static str, sources: Vec<Loc>| {
        rebuilt(&|what: &Semantics| Semantics {
            op,
            name: Some(name.into()),
            dests: vec![Loc::Reg(*d)],
            sources: sources.clone(),
            ..what.clone()
        })
    };
    // The add of the displacement the split LEA no longer carries.
    let with_offset = |first: Arc<Insn>| {
        let mut out = vec![first];
        if offset != 0 {
            out.push(match offset {
                1 | -1 if opt_inc_dec => {
                    alu(Operation::Unary, if offset == 1 { names.inc } else { names.dec }, vec![Loc::Reg(*d)])
                }
                // ADD of +128 needs a 32-bit immediate, SUB of -128 fits the
                // sign-extended byte.
                128 => alu(
                    Operation::Binary,
                    names.sub,
                    vec![Loc::Reg(*d), Loc::Imm(Imm { value: -128, width: d.width, address: None })],
                ),
                _ => alu(
                    Operation::Binary,
                    names.add,
                    vec![Loc::Reg(*d), Loc::Imm(Imm { value: offset, width: d.width, address: None })],
                ),
            });
        }
        out
    };
    let steps: Vec<Arc<Insn>> =
        if is_scale1 && same(base, index) && (offset != 0 || (ineff_base && !base_or_index_is_dst)) {
            // `lea D(%base,%base)` is `lea D(,%base,2)`.
            vec![lea_of(RegId::None, index, 2, offset)]
        } else if is_scale1 && base_or_index_is_dst {
            // `lea (%base,%index),%base` is `add %index,%base`.
            let other = if dest_is_base { index } else { base };
            with_offset(alu(Operation::Binary, names.add, vec![Loc::Reg(*d), term(other)]))
        } else if !ineff_base || (!ineff_index && is_scale1) {
            // An inefficient base trades places with the index; otherwise the
            // offset is split off.
            let (through, index_through) = if ineff_base { (index, base) } else { (base, index) };
            with_offset(lea_of(through, index_through, scale, 0))
        } else {
            // The rest has an inefficient base that is not the destination.
            if narrow {
                return None;
            }
            if is_scale1 && offset == 0 {
                // `lea (%base,%index),%dst` is `mov %base,%dst; add
                // %index,%dst`.
                vec![
                    rebuilt(&|what: &Semantics| Semantics {
                        op: Operation::Move,
                        name: Some("mov".into()),
                        dests: vec![Loc::Reg(*d)],
                        sources: vec![Loc::Reg(Reg { register: base, width: d.width })],
                        ..what.clone()
                    }),
                    alu(Operation::Binary, names.add, vec![Loc::Reg(*d), term(index)]),
                ]
            } else {
                // `lea off(%base,%index,scale),%dst` is `lea
                // off(,%index,scale),%dst; add %base,%dst`.
                vec![
                    lea_of(RegId::None, index, scale, offset),
                    alu(Operation::Binary, names.add, vec![Loc::Reg(*d), term(base)]),
                ]
            }
        };
    let mut out = Vec::with_capacity(block.insns.len() + 1);
    for (position, one) in block.insns.iter().enumerate() {
        if position == at {
            out.extend(steps.iter().cloned());
        } else {
            out.push(Arc::clone(one));
        }
    }
    Some(out)
}

/// `optLEAALU`: the block's instructions with the LEA at `lea` and its add or
/// sub made two steps on the destination, or `None` where the pin leaves them.
/// `dead` is the liveness of the block as it was.
pub fn opt_lea_alu(
    regs: Regs,
    block: &LirBlock,
    lea: usize,
    dead: &DeadAfter,
) -> Option<Vec<Arc<Insn>>> {
    let alu = search_alu_inst(regs, block, lea, dead)?;
    let (lea_insn, alu_insn) = (&block.insns[lea], &block.insns[alu]);
    // The pin's LEA32r: two registers as wide as the add, scale 1, no
    // displacement, symbol or segment.
    let [Loc::Reg(t)] = lea_insn.what.as_ref()?.dests.as_slice() else { return None };
    let [source] = lea_insn.what.as_ref()?.sources.as_slice() else { return None };
    let (symbol, base, index, scale, offset) = lea_cell(source)?;
    let alu_what = alu_insn.what.as_ref()?;
    let [Loc::Reg(d)] = alu_what.dests.as_slice() else { return None };
    let wide = |register: RegId| regs.bytes(register) == Some(i64::from(d.width));
    if symbol
        || !wide(base)
        || !wide(index)
        || scale != 1
        || offset != 0
        || base == RegId::None
        || index == RegId::None
        || base == t.register
        || index == t.register
        || (base == d.register && index == d.register)
        || !free_fields(lea_insn, REWRITABLE)
        || !free_fields(alu_insn, REWRITABLE)
        || !arithmetic_flags().is_subset(dead.get(&id(lea_insn))?)
    {
        return None;
    }
    let found = check_reg_usage(regs, block, lea, alu, dead);
    let at_lea = found.base_index_def;
    if at_lea && found.alu_dest_ref {
        return None;
    }
    // `swap(BaseReg, IndexReg)` when the destination is the index.
    let (first, second) = if index == d.register { (index, base) } else { (base, index) };
    let step = |term: RegId| {
        let mut what = alu_what.clone();
        what.sources = vec![Loc::Reg(*d), Loc::Reg(Reg { register: term, width: d.width })];
        Arc::new(Insn {
            what: Some(what),
            uses: make::joined_uses(regs, lea_insn, alu_insn),
            widths: make::joined_widths(regs, lea_insn, alu_insn),
            ..(**alu_insn).clone()
        })
    };
    let steps = [step(first), step(second)];
    let mut out = Vec::with_capacity(block.insns.len() + 1);
    for (at, one) in block.insns.iter().enumerate() {
        if at == lea {
            if at_lea {
                out.extend(steps.iter().cloned());
            }
        } else if at == alu {
            if !at_lea {
                out.extend(steps.iter().cloned());
            }
        } else {
            out.push(Arc::clone(one));
        }
    }
    Some(out)
}

/// `isLEA`: `LEA32r`.
pub fn is_lea(one: &Insn) -> bool {
    one.what.as_ref().is_some_and(|what| what.op == Operation::Address && what.name.as_deref() == Some("lea"))
}

/// `isInefficientLEAReg`: EBP and R13 in any width, by their roots.
pub fn inefficient_lea_reg(
    regs: Regs,
    register: RegId,
) -> bool {
    register != RegId::None && [RegId::EBP, RegId::R13].iter().any(|bad| regs.root(register) == regs.root(*bad))
}

/// `hasInefficientLEABaseReg`: an inefficient base with an index register.
pub fn inefficient_lea_base(
    regs: Regs,
    base: RegId,
    index: RegId,
) -> bool {
    inefficient_lea_reg(regs, base) && index != RegId::None
}

/// The mnemonics the opcode maps give for a LEA: `ADDrr`, `SUBrr`, `ADDri`,
/// `SUBri`, `INC`, `DEC`. `ADD32ri` and `ADD64ri32` differ only in the
/// pin's immediate width, which our `add` takes from its operand.
pub const fn alu_names() -> AluNames {
    AluNames { add: "add", sub: "sub", inc: "inc", dec: "dec" }
}

pub struct AluNames {
    pub add: &'static str,
    pub sub: &'static str,
    pub inc: &'static str,
    pub dec: &'static str,
}

/// The base and index registers of a LEA's address (`RegId::None` where
/// absent).
pub fn lea_base_index(one: &Insn) -> Option<(RegId, RegId)> {
    match one.what.as_ref()?.sources.as_slice() {
        [Loc::Address(address)] => Some((address.through, address.index_through)),
        [Loc::Mem(cell)] => Some((cell.through, cell.index_through)),
        _ => None,
    }
}

/// What `checkRegUsage` found between a LEA and the ALU instruction after it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RegUsageCheck {
    /// A def of the base or index register.
    pub base_index_def: bool,
    /// The ALU destination is named.
    pub alu_dest_ref: bool,
    /// The instruction that reads the base, and ends it.
    pub killed_base: Option<usize>,
    /// The instruction that reads the index, and ends it.
    pub killed_index: Option<usize>,
}

/// `checkRegUsage`: the instructions strictly between `lea` and `alu`. A read
/// is a kill where no lane of the register is live after the instruction
/// (`isKill`).
pub fn check_reg_usage(
    regs: Regs,
    block: &LirBlock,
    lea: usize,
    alu: usize,
    dead: &DeadAfter,
) -> RegUsageCheck {
    let mut found = RegUsageCheck::default();
    let (base, index) = lea_base_index(&block.insns[lea]).unwrap_or((RegId::None, RegId::None));
    let alu_dest = block
        .insns[alu]
        .what
        .as_ref()
        .and_then(
            |what| match what.dests.as_slice() {
                [Loc::Reg(reg)] => Some(reg.register),
                _ => None,
            },
        );
    let overlaps =
        |a: RegId, b: RegId| a != RegId::None && b != RegId::None && !_lanes(regs, a).is_disjoint(&_lanes(regs, b));
    for at in lea + 1..alu {
        let one = &block.insns[at];
        let Some(what) = one.what.as_ref() else { continue };
        let mut operands: Vec<(RegId, bool)> = Vec::new();
        for dest in &what.dests {
            if let Loc::Reg(reg) = dest {
                operands.push((reg.register, true));
            }
        }
        for loc in what.dests.iter().chain(&what.sources) {
            match loc {
                Loc::Mem(mem) => operands.extend([(mem.through, false), (mem.index_through, false)]),
                Loc::Address(address) => operands.extend([(address.through, false), (address.index_through, false)]),
                _ => {}
            }
        }
        for source in &what.sources {
            if let Loc::Reg(reg) = source {
                operands.push((reg.register, false));
            }
        }
        for (register, is_def) in operands {
            if alu_dest.is_some_and(|d| overlaps(register, d)) {
                found.alu_dest_ref = true;
            }
            let ends = || dead.get(&id(one)).is_some_and(|lanes| _lanes(regs, register).is_subset(lanes));
            for (mine, killed) in [(base, &mut found.killed_base), (index, &mut found.killed_index)] {
                if overlaps(register, mine) {
                    if is_def {
                        found.base_index_def = true;
                    } else if ends() {
                        *killed = Some(at);
                    }
                }
            }
        }
    }
    found
}

/// `RegUsageState`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegUsage {
    NotUsed,
    Read,
    Write,
}

/// `usesRegister`: whether `one` writes, else reads, any byte of `register`.
/// A write wins over a read, as the pin's operand loop returns at the first
/// def.
pub fn uses_register(
    regs: Regs,
    register: RegId,
    one: &Insn,
) -> RegUsage {
    let Some(what) = one.what.as_ref() else { return RegUsage::NotUsed };
    let mine = _lanes(regs, register);
    let touches = |loc: &Loc| match loc {
        Loc::Reg(reg) => !_lanes(regs, reg.register).is_disjoint(&mine),
        Loc::Mem(mem) => [mem.through, mem.index_through]
            .into_iter()
            .any(|r| r != RegId::None && !_lanes(regs, r).is_disjoint(&mine)),
        _ => false,
    };
    if what.dests.iter().any(|dest| matches!(dest, Loc::Reg(_)) && touches(dest)) {
        return RegUsage::Write;
    }
    if what.dests.iter().chain(&what.sources).any(touches) { RegUsage::Read } else { RegUsage::NotUsed }
}

/// `getPreviousInstr`: the index before `at`, wrapping to the last instruction
/// when the block branches to itself.
pub fn previous_instr(
    block: &LirBlock,
    at: usize,
) -> Option<usize> {
    match at {
        0 if block.succ.contains(&block.at) && !block.insns.is_empty() => Some(block.insns.len() - 1),
        0 => None,
        _ => Some(at - 1),
    }
}

/// `TSM.computeInstrLatency` on the default model: a load takes 4, a
/// bookkeeping instruction 0, anything else 1.
fn instr_latency(one: &Insn) -> u32 {
    let Some(what) = one.what.as_ref() else { return 0 };
    if matches!(
        what.op,
        Operation::Nothing | Operation::Restore | Operation::Data
    ) {
        0
    } else if what.sources.iter().any(|source| matches!(source, Loc::Mem(_))) {
        4
    } else {
        1
    }
}

/// `searchBackwards`: the nearest earlier instruction in the block that
/// writes `register`, within [`INSTR_DISTANCE_THRESHOLD`] cycles and no
/// call or inline asm between.
pub fn search_backwards(
    regs: Regs,
    register: RegId,
    block: &LirBlock,
    at: usize,
) -> Option<usize> {
    let mut distance = 1;
    let mut cur = previous_instr(block, at);
    while let Some(index) = cur.filter(|index| *index != at) {
        let one = &block.insns[index];
        if one.what.as_ref().is_some_and(|what| matches!(what.op, Operation::Call | Operation::Escape)) {
            break;
        }
        if distance > INSTR_DISTANCE_THRESHOLD {
            break;
        }
        if uses_register(regs, register, one) == RegUsage::Write {
            return Some(index);
        }
        distance += instr_latency(one);
        cur = previous_instr(block, index);
    }
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use llrm_lir::registers::{RegId, Regs};

    use super::*;
    use crate::model::ir::{Mem, Reg, Semantics};

    fn regs() -> Regs {
        Regs(&llrm_x86_m32::REGISTER_INFO)
    }

    fn reg(
        register: RegId,
        width: u32,
    ) -> Loc {
        Loc::Reg(Reg { register, width })
    }

    fn insn(
        at: i64,
        op: Operation,
        name: &str,
        dests: Vec<Loc>,
        sources: Vec<Loc>,
    ) -> Arc<Insn> {
        let what = Semantics { name: Some(name.into()), dests, sources, ..Semantics::new(op) };
        Arc::new(Insn::new(at, Some((at, at)), Some(what), vec![], vec![]))
    }

    fn mov(
        at: i64,
        dest: RegId,
        source: RegId,
    ) -> Arc<Insn> {
        insn(at, Operation::Move, "mov", vec![reg(dest, 4)], vec![reg(source, 4)])
    }

    fn load(
        at: i64,
        dest: RegId,
    ) -> Arc<Insn> {
        let cell = Loc::Mem(Mem { through: RegId::ESI, ..Mem::new(None, 4) });
        insn(at, Operation::Move, "mov", vec![reg(dest, 4)], vec![cell])
    }

    fn lea(at: i64) -> Arc<Insn> {
        insn(at, Operation::Address, "lea", vec![reg(RegId::EAX, 4)], vec![reg(RegId::ECX, 4)])
    }

    fn block(
        insns: Vec<Arc<Insn>>,
        succ: Vec<i64>,
    ) -> LirBlock {
        LirBlock { succ: succ.into(), ..LirBlock::new(0, insns) }
    }

    #[test]
    fn a_write_to_a_byte_of_a_register_is_a_write_of_the_register() {
        let al = insn(0, Operation::Move, "mov", vec![reg(RegId::AL, 1)], vec![reg(RegId::CL, 1)]);
        assert_eq!(uses_register(regs(), RegId::EAX, &al), RegUsage::Write);
        assert_eq!(uses_register(regs(), RegId::ECX, &al), RegUsage::Read);
        assert_eq!(uses_register(regs(), RegId::EDX, &al), RegUsage::NotUsed);
    }

    #[test]
    fn an_address_register_is_read() {
        assert_eq!(uses_register(regs(), RegId::ESI, &load(0, RegId::EAX)), RegUsage::Read);
    }

    #[test]
    fn the_nearest_write_within_the_window_is_found() {
        let b = block(vec![mov(0, RegId::ECX, RegId::EDX), mov(1, RegId::ECX, RegId::EBX), lea(2)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 2), Some(1));
        assert_eq!(search_backwards(regs(), RegId::EDX, &b, 2), None);
    }

    /// Two loads cost 8 cycles: the write before them is past the threshold.
    #[test]
    fn a_write_beyond_five_cycles_is_not_found() {
        let b = block(vec![mov(0, RegId::ECX, RegId::EDX), load(1, RegId::EBX), load(2, RegId::EDI), lea(3)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 3), None);
        let near = block(vec![mov(0, RegId::ECX, RegId::EDX), load(1, RegId::EBX), lea(2)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &near, 2), Some(0));
    }

    #[test]
    fn a_call_ends_the_search() {
        let call = insn(1, Operation::Call, "call", vec![], vec![]);
        let b = block(vec![mov(0, RegId::ECX, RegId::EDX), call, lea(2)], vec![]);
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 2), None);
    }

    /// A block that branches to itself continues from its last instruction;
    /// the search stops on reaching the start again.
    #[test]
    fn a_self_loop_wraps_to_the_last_instruction() {
        let b = block(vec![lea(0), mov(1, RegId::ECX, RegId::EDX)], vec![0]);
        assert_eq!(previous_instr(&b, 0), Some(1));
        assert_eq!(search_backwards(regs(), RegId::ECX, &b, 0), Some(1));
        let straight = block(vec![lea(0), mov(1, RegId::ECX, RegId::EDX)], vec![]);
        assert_eq!(previous_instr(&straight, 0), None);
        assert_eq!(search_backwards(regs(), RegId::ECX, &straight, 0), None);
    }

    #[test]
    fn only_the_lea_is_a_lea() {
        assert!(is_lea(&lea(0)));
        assert!(!is_lea(&mov(0, RegId::EAX, RegId::ECX)));
    }

    #[test]
    fn the_pass_leaves_a_body_alone() {
        let b = LirBody::new("f", 0, vec![block(vec![lea(0)], vec![]).into()], Default::default(), Default::default());
        assert_eq!(FixupLeas::default().transform(b.clone()).unwrap(), b);
    }

    fn lea_of(
        at: i64,
        dest: RegId,
        base: RegId,
        index: RegId,
    ) -> Arc<Insn> {
        let address = crate::model::ir::AddressRef {
            through: base,
            index_through: index,
            ..crate::model::ir::AddressRef::new(None)
        };
        insn(at, Operation::Address, "lea", vec![reg(dest, 4)], vec![Loc::Address(address)])
    }

    fn add(
        at: i64,
        dest: RegId,
        source: RegId,
    ) -> Arc<Insn> {
        insn(at, Operation::Binary, "add", vec![reg(dest, 4)], vec![reg(dest, 4), reg(source, 4)])
    }

    fn dead_after(
        block: &LirBlock,
        dead: &[(usize, &[RegId])],
    ) -> DeadAfter {
        dead.iter()
            .map(|(at, registers)| (id(&block.insns[*at]), registers.iter().flat_map(|r| _lanes(regs(), *r)).collect()))
            .collect()
    }

    /// `lea eax,[eax+ecx]` is `add eax,ecx` only where no flag is live after
    /// it: `add` sets all six, `lea` none, and a `jb` after it read the carry
    /// of something else.
    #[test]
    fn a_lea_becomes_an_add_only_where_every_flag_is_dead() {
        let b = block(vec![lea_of(0, RegId::EAX, RegId::EAX, RegId::ECX)], vec![]);
        let key = id(&b.insns[0]);
        let flags = arithmetic_flags();
        let dead: DeadAfter = [(key, flags)].into_iter().collect();
        let made = opt_two_addr_lea(regs(), &b, 0, &dead, true).expect("flags dead: an add");
        assert_eq!(made[0].what.as_ref().unwrap().name.as_deref(), Some("add"));
        for live in [RflagsBits::CF, RflagsBits::ZF] {
            let kept = flags.minus(&Lanes::flags(live));
            let dead: DeadAfter = [(key, kept)].into_iter().collect();
            assert!(opt_two_addr_lea(regs(), &b, 0, &dead, true).is_none(), "flag {live:#x} live");
        }
    }

    #[test]
    fn an_ebp_base_with_an_index_is_inefficient() {
        assert!(inefficient_lea_base(regs(), RegId::EBP, RegId::ECX));
        assert!(!inefficient_lea_base(regs(), RegId::EBP, RegId::None));
        assert!(!inefficient_lea_base(regs(), RegId::EAX, RegId::ECX));
    }

    #[test]
    fn the_opcode_maps_name_add_sub_inc_dec() {
        let names = alu_names();
        assert_eq!([names.add, names.sub, names.inc, names.dec], ["add", "sub", "inc", "dec"]);
    }

    /// `lea eax,[ebx+ecx]`; `mov edx,ebx` (last use of ebx); `mov ecx,5`
    /// (defines the index); `add esi,eax`.
    #[test]
    fn check_reg_usage_finds_a_killed_base_and_a_defined_index() {
        let b = block(
            vec![
                lea_of(0, RegId::EAX, RegId::EBX, RegId::ECX),
                mov(1, RegId::EDX, RegId::EBX),
                insn(
                    2,
                    Operation::Move,
                    "mov",
                    vec![reg(RegId::ECX, 4)],
                    vec![Loc::Imm(crate::model::ir::Imm { value: 5, width: 4, address: None })],
                ),
                add(3, RegId::ESI, RegId::EAX),
            ],
            vec![],
        );
        let dead = dead_after(&b, &[(1, &[RegId::EBX])]);
        let found = check_reg_usage(regs(), &b, 0, 3, &dead);
        assert_eq!(
            found,
            RegUsageCheck { base_index_def: true, alu_dest_ref: false, killed_base: Some(1), killed_index: None }
        );
    }

    /// A lea whose source is a `Mem` cell (as allocation leaves it) has a base
    /// and index: a def of the base between it and the add was missed, the adds
    /// were put after `mov edx, ecx` and read the new edx (call_keeps m16
    /// printed 905759280 for 686444454).
    #[test]
    fn check_reg_usage_sees_the_base_of_a_lea_with_a_cell_source() {
        let cell = Loc::Mem(Mem { through: RegId::EDX, index_through: RegId::ECX, scale: 1, ..Mem::new(None, 4) });
        let b = block(
            vec![
                insn(0, Operation::Address, "lea", vec![reg(RegId::EBX, 4)], vec![cell]),
                mov(1, RegId::EDX, RegId::ECX),
                add(2, RegId::ESI, RegId::EBX),
            ],
            vec![],
        );
        let found = check_reg_usage(regs(), &b, 0, 2, &dead_after(&b, &[]));
        assert!(found.base_index_def, "{found:?}");
    }

    #[test]
    fn check_reg_usage_sees_a_reference_to_the_alu_destination() {
        let b = block(
            vec![
                lea_of(0, RegId::EAX, RegId::EBX, RegId::ECX),
                mov(1, RegId::EDX, RegId::ESI),
                add(2, RegId::ESI, RegId::EAX),
            ],
            vec![],
        );
        let found = check_reg_usage(regs(), &b, 0, 2, &DeadAfter::default());
        assert!(found.alu_dest_ref && !found.base_index_def && found.killed_base.is_none());
    }

    /// The register file and a body of one block at x86-64.
    fn regs64() -> Regs {
        Regs(&llrm_x86_m64::REGISTER_INFO)
    }

    fn lea64(
        dest: (RegId, u32),
        through: RegId,
        index_through: RegId,
        scale: i64,
        offset: i64,
    ) -> Arc<Insn> {
        let address = crate::model::ir::AddressRef {
            through,
            index_through,
            scale,
            offset,
            ..crate::model::ir::AddressRef::new(None)
        };
        insn(0, Operation::Address, "lea", vec![reg(dest.0, dest.1)], vec![Loc::Address(address)])
    }

    /// The pass on one LEA at `-march=x86-64`: each result instruction as `name
    /// dest, sources`.
    fn split(lea: Arc<Insn>) -> Vec<String> {
        // A compare that writes every flag, and a return that reads only rax:
        // the rest is dead after the LEA.
        let compare = insn(
            1,
            Operation::Compare,
            "cmp",
            vec![],
            vec![reg(RegId::RDI, 8), Loc::Imm(Imm { value: 8, width: 8, address: None })],
        );
        let ret = Arc::new(Insn {
            requires: vec![(crate::model::ir::Held { value: 7, width: 8 }, RegId::RAX)],
            reads_complete: true,
            ..Insn::new(2, Some((2, 2)), Some(Semantics::new(Operation::Return)), vec![], vec![7])
        });
        let mut body = LirBody::new(
            "f",
            0,
            vec![block(vec![lea, compare, ret], vec![]).into()],
            Default::default(),
            Default::default(),
        );
        body.registers = Some(regs64());
        body.bits = 64;
        let done = FixupLeas { slow_3ops: true, ..FixupLeas::default() }.transform(body).expect("the pass cannot fail");
        let all = &done.blocks[0].insns;
        all[..all.len() - 2]
            .iter()
            .map(|one| {
                let what = one.what.as_ref().unwrap();
                let show = |loc: &Loc| match loc {
                    Loc::Reg(r) => regs64().name(r.register).unwrap_or("?").to_owned(),
                    Loc::Imm(i) => i.value.to_string(),
                    Loc::Address(a) => format!(
                        "[{}+{}*{}{:+}]",
                        regs64().name(a.through).unwrap_or("-"),
                        regs64().name(a.index_through).unwrap_or("-"),
                        a.scale,
                        a.offset
                    ),
                    _ => "?".into(),
                };
                format!(
                    "{} {}, {}",
                    what.name.as_deref().unwrap(),
                    what.dests.iter().map(show).collect::<Vec<_>>().join(","),
                    what.sources.iter().map(show).collect::<Vec<_>>().join(",")
                )
            })
            .collect()
    }

    /// The pinned clang at `-target x86_64-linux-gnu` (generic tuning) gives,
    /// for `a + b*4 + 7`, `lea (%rdi,%rsi,4),%rax; add $7,%rax`: a
    /// three-operand LEA takes three cycles. Without the case it stayed one
    /// LEA.
    #[test]
    fn a_three_operand_lea_is_a_lea_and_an_add_at_x86_64() {
        let (rax, rdi, rsi) = ((RegId::RAX, 8), RegId::RDI, RegId::RSI);
        assert_eq!(split(lea64(rax, rdi, rsi, 4, 7)), ["lea rax, [rdi+rsi*4+0]", "add rax, rax,7"]);
        assert_eq!(split(lea64(rax, rdi, rsi, 8, -3)), ["lea rax, [rdi+rsi*8+0]", "add rax, rax,-3"]);
        assert_eq!(split(lea64(rax, rdi, rsi, 1, 1)), ["lea rax, [rdi+rsi*1+0]", "inc rax, rax"]);
        assert_eq!(split(lea64(rax, rdi, rsi, 1, -1)), ["lea rax, [rdi+rsi*1+0]", "dec rax, rax"]);
        // ADD of +128 needs a 32-bit immediate, SUB of -128 does not.
        assert_eq!(split(lea64(rax, rdi, rsi, 1, 128)), ["lea rax, [rdi+rsi*1+0]", "sub rax, rax,-128"]);
    }

    /// `a + a + 5` is `lea 5(,%rdi,2),%rax`: one instruction, no base.
    #[test]
    fn a_lea_of_one_register_twice_is_a_scaled_index() {
        assert_eq!(split(lea64((RegId::RAX, 8), RegId::RDI, RegId::RDI, 1, 5)), ["lea rax, [-+rdi*2+5]"]);
    }

    /// `lea (%rax,%rsi,1),%rax` is `add %rsi,%rax`, then the displacement.
    #[test]
    fn a_three_operand_lea_that_writes_its_base_is_adds() {
        let (rax, rsi) = ((RegId::RAX, 8), RegId::RSI);
        assert_eq!(split(lea64(rax, RegId::RAX, rsi, 1, 7)), ["add rax, rax,rsi", "add rax, rax,7"]);
        assert_eq!(split(lea64(rax, rsi, RegId::RAX, 1, 7)), ["add rax, rax,rsi", "add rax, rax,7"]);
    }

    /// An RBP or R13 base takes a displacement byte: `lea (%rbp,%rsi),%rax` is
    /// `mov %rbp,%rax; add %rsi,%rax`, and with a scale `lea (,%rsi,4),%rax;
    /// add %rbp,%rax`.
    #[test]
    fn an_inefficient_base_is_moved_or_swapped() {
        let rax = (RegId::RAX, 8);
        assert_eq!(split(lea64(rax, RegId::RBP, RegId::RSI, 1, 0)), ["lea rax, [rsi+rbp*1+0]"]);
        assert_eq!(split(lea64(rax, RegId::R13, RegId::RBP, 1, 0)), ["mov rax, r13", "add rax, rax,rbp"]);
        assert_eq!(split(lea64(rax, RegId::R13, RegId::RBP, 4, 0)), ["lea rax, [-+rbp*4+0]", "add rax, rax,r13"]);
    }

    /// A two-operand LEA, or one with no flag free, is the pin's to leave.
    #[test]
    fn a_two_operand_lea_is_left_alone_at_x86_64() {
        assert_eq!(split(lea64((RegId::RAX, 8), RegId::RDI, RegId::RSI, 4, 0)), ["lea rax, [rdi+rsi*4+0]"]);
    }
}
