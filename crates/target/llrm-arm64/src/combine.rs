//! arm64's side of `MachineCombiner`: `mul` + `add`/`sub` become `madd` and
//! `msub`. The pass is `llrm_codegen::combine`; this file is
//! `AArch64InstrInfo`'s `getMachineCombinerPatterns` (`getMaddPatterns`,
//! [L7801](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64InstrInfo.cpp#L7801)),
//! `genAlternativeCodeSequence` and `genFusedMultiply`/`genMaddR`
//! ([L8801](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64InstrInfo.cpp#L8801)),
//! and the Cortex-A510 latencies that `-mcpu=generic` schedules with
//! ([AArch64SchedA510.td](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Target/AArch64/AArch64SchedA510.td)).
//!
//! # How it works
//! A `mul` is `madd rd, rn, rm, zr`. An add or subtract of two registers one
//! of which is such a product, made in the same block and read by nothing
//! else (`canCombineWithMUL`), starts a pattern:
//!
//! ```text
//!   MULADD_OP1/OP2   t = mul a, b ; r = add t, c   =>  r = madd a, b, c
//!   MULSUB_OP2       t = mul a, b ; r = sub c, t   =>  r = msub a, b, c
//!   MULSUB_OP1       t = mul a, b ; r = sub t, c   =>  n = sub zr, c ; r = madd a, b, n
//!   MULADDWI/MULSUBWI  t = mul a, b ; r = add/sub t, #i  =>  n = mov ±i ; r = madd a, b, n
//! ```
//!
//! The shared pass takes a replacement only when it does not lengthen the
//! block's critical path with the latencies below.
//!
//! The Cortex-A510 numbers (WriteI 1, WriteISReg/IEReg 2, WriteIM32 3,
//! WriteIM64 5, WriteLD 2, WriteF/WriteFMul 4, WriteFDiv 13/22; `ReadIMA`
//! advances the accumulator of a multiply-add by 2). Every other instruction is
//! one cycle.
//!
//! DEVIATE MC-N (owner w15, exit: the pass moves before phi elimination, on
//! SSA): the pin runs on SSA, so a product's operands cannot change between
//! the `mul` and the `add`. The body here still has the several-definition
//! registers phis become; a pattern is not made when either operand of the
//! product, or the other operand of the sum, is defined between them.
//!
//! DEVIATE MC-R (owner w15, exit: `preservesResourceLen` over the A510 unit
//! tables): the resources test accepts every replacement; each fusion removes
//! an instruction.
//!
//! DEVIATE MC-P (owner w15, review 2026-11-07): the flag-setting forms and the
//! vector patterns are not made. The floating-point patterns need the
//! `contract` flag on the `fadd`/`fsub` (`isCombineInstrCandidateFP`), which
//! `MFunc` does not carry; `fmuladd` is already fused in selection.
//!
//! # Runs
//! After `prepass` (which sinks and drops dead flag settings) and before the
//! copies are ordered, at every -O level but -O0.

use std::collections::HashMap;

use llrm_codegen::combine::{self, CombinerIsa, Node, Replacement, Setting, Use};

use crate::expand_imm;
use crate::mc::inst::*;
use crate::mc::visit::Reg;
use crate::mfunc::{MFunc, Op, OpList};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    /// `add t, c` and `add c, t`.
    Add,
    /// `sub c, t`.
    SubFrom,
    /// `sub t, c`.
    SubOf,
    /// `add t, #imm` and `sub t, #imm`: the immediate, negated for a
    /// subtraction, is first put in a register.
    Immediate(u64),
}

#[derive(Clone, Copy, Debug)]
struct Pattern {
    kind: Kind,
    /// The index of the `mul` in the block.
    mul: usize,
    /// The register the product is combined with.
    other: Gpr,
    width: Width,
}

/// The number of definitions and reads of each virtual register in the
/// function: `MRI`'s `getUniqueVRegDef` and `hasOneNonDBGUse`.
struct Counts {
    defs: HashMap<Reg, usize>,
    uses: HashMap<Reg, usize>,
}

impl Counts {
    fn of(function: &MFunc) -> Self {
        let (mut defs, mut uses): (HashMap<Reg, usize>, HashMap<Reg, usize>) = Default::default();
        for op in function.blocks.iter().flat_map(|block| block.ops.iter()) {
            let (written, read) = op.defs_and_uses();
            for reg in written {
                *defs.entry(reg).or_default() += 1;
            }
            for reg in read {
                *uses.entry(reg).or_default() += 1;
            }
        }
        Self { defs, uses }
    }

    fn single(
        &self,
        reg: Gpr,
    ) -> bool {
        let key = Reg::Gpr(reg);
        reg.is_virtual() && self.defs.get(&key) == Some(&1) && self.uses.get(&key) == Some(&1)
    }
}

struct Block<'a> {
    ops: &'a mut OpList,
    next_register: &'a mut u32,
    counts: &'a Counts,
    /// The operations `generate` made, for `apply`.
    pending: Vec<Op>,
}

fn id(reg: Reg) -> combine::Reg {
    match reg {
        Reg::Gpr(Gpr(n)) => n * 2,
        Reg::Fpr(Vreg(n)) => n * 2 + 1,
    }
}

/// The latency of a result of `op` (Cortex-A510).
fn latency(op: &Op) -> u32 {
    match op {
        Op::Copy { .. } => 0,
        Op::I(inst) => match inst {
            Inst::ThreeSource { width: Width::W, .. } => 3,
            Inst::ThreeSource { width: Width::X, .. } => 5,
            Inst::AddSubShifted { shift, amount, .. } if !(*shift == Shift::Lsl && *amount == 0) => 2,
            Inst::AddSubExtended { .. } => 2,
            Inst::Load { .. } => 2,
            Inst::FpBinary { op: FpBinary::Div, size: FpSize::D, .. } => 22,
            Inst::FpBinary { op: FpBinary::Div, .. } => 13,
            Inst::FpBinary { .. } | Inst::FpFused { .. } => 4,
            _ => 1,
        },
        _ => 1,
    }
}

fn node_of(op: &Op) -> Node {
    let (defs, uses) = op.defs_and_uses();
    let defs: Vec<_> = defs.into_iter().filter(|reg| reg.is_virtual()).map(id).collect();
    let accumulator = match op {
        Op::I(Inst::ThreeSource {
            op: Mul3::Madd | Mul3::Msub | Mul3::Smaddl | Mul3::Smsubl | Mul3::Umaddl | Mul3::Umsubl,
            ra,
            rn,
            rm,
            ..
        }) if ra != rn && ra != rm => Some(Reg::Gpr(*ra)),
        _ => None,
    };
    let uses = uses
        .into_iter()
        .filter(|reg| reg.is_virtual())
        .map(|reg| Use { reg: id(reg), advance: if Some(reg) == accumulator { 2 } else { 0 } })
        .collect();
    Node { defs, uses, latency: latency(op), transient: matches!(op, Op::Copy { .. }) }
}

impl Block<'_> {
    /// `canCombineWithMUL`: the instruction at `at` is `mul rd, rn, rm`, rd
    /// being defined and read once in the function.
    fn product_at(
        &self,
        at: usize,
        reg: Gpr,
        width: Width,
    ) -> Option<(Gpr, Gpr)> {
        let Op::I(Inst::ThreeSource { op: Mul3::Madd, width: w, rd, rn, rm, ra }) = self.ops[at] else { return None };
        (w == width && rd == reg && ra == Gpr::R31 && self.counts.single(reg)).then_some((rn, rm))
    }

    fn defined_between(
        &self,
        from: usize,
        to: usize,
        reg: Gpr,
    ) -> bool {
        self.ops[from + 1..to].iter().any(|op| op.defs_and_uses().0.contains(&Reg::Gpr(reg)))
    }

    fn fresh(&mut self) -> Gpr {
        let number = *self.next_register;
        *self.next_register += 1;
        Gpr(number)
    }
}

impl Block<'_> {
    /// `ADDWri`/`ADDXri`/`SUBWri`/`SUBXri` (`MULADDWI_OP1` and the others):
    /// only when the immediate takes one instruction to make.
    fn immediate_pattern(
        &self,
        root: usize,
        op: AddSub,
        width: Width,
        rn: Gpr,
        imm: u64,
    ) -> Vec<Pattern> {
        if op.set_flags || !rn.is_virtual() {
            return vec![];
        }
        let Some(mul) = self.ops[..root].iter().rposition(|op| op.defs_and_uses().0.contains(&Reg::Gpr(rn))) else {
            return vec![];
        };
        let Some((a, b)) = self.product_at(mul, rn, width) else { return vec![] };
        if [a, b].iter().any(|reg| !(reg.is_virtual() || *reg == Gpr::R31) || self.defined_between(mul, root, *reg)) {
            return vec![];
        }
        let bits = if width == Width::W { 32 } else { 64 };
        let value = if op.subtract { imm.wrapping_neg() } else { imm };
        let value = if bits == 32 { u64::from(value as u32) } else { value };
        if expand_imm::mov_imm(value, bits, Gpr::R31).len() != 1 {
            return vec![];
        }
        vec![Pattern { kind: Kind::Immediate(value), mul, other: Gpr::R31, width }]
    }
}

impl CombinerIsa for Block<'_> {
    type Pattern = Pattern;

    fn nodes(&self) -> Vec<Node> {
        self.ops.iter().map(node_of).collect()
    }

    /// `getMaddPatterns`: ADDWrr/ADDXrr, SUBWrr/SUBXrr.
    fn patterns(
        &self,
        root: usize,
    ) -> Vec<Pattern> {
        if let Op::I(Inst::AddSubImm { op, width, rn, imm12, shift12, .. }) = self.ops[root] {
            return self.immediate_pattern(root, op, width, rn, u64::from(imm12) << if shift12 { 12 } else { 0 });
        }
        let Op::I(Inst::AddSubShifted { op, width, rd, rn, rm, shift: Shift::Lsl, amount: 0 }) = self.ops[root] else {
            return vec![];
        };
        if op.set_flags {
            return vec![];
        }
        let _ = rd;
        let candidates: &[(Gpr, Gpr, Kind)] = if op.subtract {
            &[(rm, rn, Kind::SubFrom), (rn, rm, Kind::SubOf)]
        } else {
            &[(rn, rm, Kind::Add), (rm, rn, Kind::Add)]
        };
        let mut found = vec![];
        for &(product, other, kind) in candidates {
            if !product.is_virtual() {
                continue;
            }
            let Some(mul) = self.ops[..root].iter().rposition(|op| op.defs_and_uses().0.contains(&Reg::Gpr(product)))
            else {
                continue;
            };
            let Some((a, b)) = self.product_at(mul, product, width) else { continue };
            // The zero register is the same value everywhere.
            let readable = |reg: Gpr| reg.is_virtual() || reg == Gpr::R31;
            let stable = [a, b, other].iter().all(|reg| readable(*reg) && !self.defined_between(mul, root, *reg));
            if stable {
                found.push(Pattern { kind, mul, other, width });
            }
        }
        found
    }

    /// `genAlternativeCodeSequence`.
    fn generate(
        &mut self,
        root: usize,
        pattern: Pattern,
    ) -> Option<Replacement> {
        let (Op::I(Inst::AddSubShifted { rd, .. }) | Op::I(Inst::AddSubImm { rd, .. })) = self.ops[root] else {
            return None;
        };
        let Op::I(Inst::ThreeSource { rn: a, rm: b, .. }) = self.ops[pattern.mul] else { return None };
        let mut made = vec![];
        let (fused, addend) = match pattern.kind {
            Kind::Add => (Mul3::Madd, pattern.other),
            Kind::SubFrom => (Mul3::Msub, pattern.other),
            Kind::Immediate(value) => {
                let constant = self.fresh();
                let bits = if pattern.width == Width::W { 32 } else { 64 };
                made.push(Op::Imm { dst: constant, bits, value });
                (Mul3::Madd, constant)
            }
            Kind::SubOf => {
                let negated = self.fresh();
                made.push(Op::I(Inst::AddSubShifted {
                    op: AddSub { subtract: true, set_flags: false },
                    width: pattern.width,
                    rd: negated,
                    rn: Gpr::R31,
                    rm: pattern.other,
                    shift: Shift::Lsl,
                    amount: 0,
                }));
                (Mul3::Madd, negated)
            }
        };
        made.push(Op::I(Inst::ThreeSource { op: fused, width: pattern.width, rd, rn: a, rm: b, ra: addend }));
        let inserted = made.iter().map(node_of).collect();
        self.pending = made;
        Some(Replacement { inserted, deleted: vec![pattern.mul, root] })
    }

    fn apply(
        &mut self,
        root: usize,
        pattern: Pattern,
        _replacement: &Replacement,
    ) {
        let (_, loc) = self.ops.remove(root);
        self.ops.remove(pattern.mul);
        for (index, op) in std::mem::take(&mut self.pending).into_iter().enumerate() {
            self.ops.insert_at(root - 1 + index, op, loc);
        }
    }
}

/// Fuse the products of `function` where that does not lengthen a block's
/// critical path. `optimize_for_size`: at -Os and -Oz.
pub fn run(
    function: &mut MFunc,
    optimize_for_size: bool,
) {
    let counts = Counts::of(function);
    let MFunc { blocks, next_register, .. } = function;
    for block in blocks.iter_mut() {
        let mut view = Block { ops: &mut block.ops, next_register, counts: &counts, pending: vec![] };
        combine::combined(&mut view, Setting { optimize_for_size, in_loop: false });
    }
}
