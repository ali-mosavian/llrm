//! `X86FixupLEAs`. Translated from
//! [X86FixupLEAs.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86FixupLEAs.cpp#L232)
//! (`runOnMachineFunction`), function by function, one PR each (plan:
//! `docs/working/lea-fixup-plan.md`).
//!
//! # How it works
//! The pass visits every LEA of every block in order. On i486 none of
//! `slowLEA`, `slow3OpsLEA` or `leaUsesAG` is set, so only `optTwoAddrLEA`
//! (L564) runs on each; until it moves here (row 100, PR 4) the rules in
//! `peephole.peep` (`lea_fixed`, and `lea_alu`'s case 1) do its work.
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
use crate::model::ir::{Loc, Operation, Reg};
use crate::model::lir::{Insn, LirBlock, LirBody};
use crate::model::passes::LIRTransform;

/// `INSTR_DISTANCE_THRESHOLD`: how far back a write still matters, in cycles.
const INSTR_DISTANCE_THRESHOLD: u32 = 5;

pub struct FixupLeas;

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
                if is_lea(&current.insns[at])
                    && let Some(rewritten) = opt_lea_alu(regs, &current, at, &dead)
                {
                    current = current.with_insns(rewritten);
                }
                at += 1;
            }
            *block = current;
        }
        Ok(out)
    }
}

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

/// `isInefficientLEAReg` with `hasInefficientLEABaseReg`: a base of EBP with an
/// index register (`R13` has no counterpart before x86-64).
pub fn inefficient_lea_base(
    base: RegId,
    index: RegId,
) -> bool {
    base == RegId::EBP && index != RegId::None
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
        assert_eq!(FixupLeas.transform(b.clone()).unwrap(), b);
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

    #[test]
    fn an_ebp_base_with_an_index_is_inefficient() {
        assert!(inefficient_lea_base(RegId::EBP, RegId::ECX));
        assert!(!inefficient_lea_base(RegId::EBP, RegId::None));
        assert!(!inefficient_lea_base(RegId::EAX, RegId::ECX));
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
}
