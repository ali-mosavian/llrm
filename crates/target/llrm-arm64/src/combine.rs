//! arm64's side of `MachineCombiner`: `mul` + `add`/`sub` become `madd` and
//! `msub`, and a chain of one integer operation is rebuilt so the operand that
//! arrives last joins last. The pass is `llrm_codegen::combine`; this file is
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
//!   MULADDv_OP1/OP2    t = mul.v a, b ; r = add.v t, c  =>  r = c ; mla r, a, b
//!   MULSUBv_OP2        t = mul.v a, b ; r = sub.v c, t  =>  r = c ; mls r, a, b
//!   MULSUBv_OP1        t = mul.v a, b ; r = sub.v t, c  =>  r = neg c ; mla r, a, b
//! ```
//!
//! Where no such pattern starts, the generic ones of
//! `TargetInstrInfo::getMachineCombinerPatterns` do
//! (`isReassociationCandidate`, `reassociateOps`): the root is `add`, `and`,
//! `orr`, `eor` or `eon` of two registers
//! (`AArch64InstrInfo::isAssociativeAndCommutative`, no flags, no shift), one
//! operand is the result of the same operation that nothing else reads
//! (`prev`), and the pair is rebuilt:
//!
//! ```text
//!   REASSOC_AX_BY etc.   b = a op x ; c = b op y   =>   n = x op y ; c = a op n
//! ```
//!
//! Of `prev`'s two operands either may be `a`, and either may be the root's
//! `b` side: four patterns, tried in the pin's order. They must make the new
//! root shallower (`MustReduceDepth`); a chain of equal depths is rebuilt from
//! the third link on, a sum of six terms into an accumulator becomes a tree 4
//! deep rather than a chain of 6.
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
//! DEVIATE MC-A (owner w15, exit: the pass moves before phi elimination, on
//! SSA): `hasReassociableOperands` asks a unique definition for each operand;
//! here a register with several is a phi's result and counts as made in the
//! block. The instructions the pair becomes stand where the root does, so a
//! pattern is not made when either operand of `prev` is defined between the
//! two.
//!
//! DEVIATE MC-F (owner w15, exit: flags as a register of the trace): the trace
//! does not follow `NZCV`, as the pin's `MachineTraceMetrics` does, so a value
//! that waits for a compare (`csel`, `cset`) is as deep as one that does not;
//! the tree can differ from the pin's where depths there differ.
//!
//! DEVIATE MC-R (owner w15, exit: `preservesResourceLen` over the A510 unit
//! tables): the resources test accepts every replacement; each fusion removes
//! an instruction.
//!
//! DEVIATE MC-P (owner w15, review 2026-11-07): the flag-setting forms and the
//! integer vector patterns of the `_indexed` kind are not made. The
//! floating-point patterns need the `contract` flag on the `fadd`/`fsub`
//! (`isCombineInstrCandidateFP`), which `MFunc` does not carry; `fmuladd` is
//! already fused in selection.
//!
//! # Runs
//! After `prepass` (which sinks and drops dead flag settings) and before the
//! copies are ordered, at every -O level but -O0.

use llrm_codegen::combine::{self, CombinerIsa, Node, Objective, Replacement, Setting, Use};
use rustc_hash::FxHashMap as HashMap;

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
enum Pattern {
    Scalar(ScalarPattern),
    /// `mla`/`mls` of a vector product and the register it is combined with.
    Vector {
        kind: VectorKind,
        mul: usize,
        other: Vreg,
        lanes: Arrangement,
    },
    /// `REASSOC_*`: `prev` is the instruction before `root` whose result the
    /// root reads.
    Reassoc {
        shape: Reassoc,
        prev: usize,
    },
}

/// The four `MachineCombinerPattern::REASSOC_*`: which operand of `prev` is `A`
/// and which operand of the root `prev` is.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Reassoc {
    AxBy,
    XaBy,
    AxYb,
    XaYb,
}

impl Reassoc {
    /// Whether `prev` is the root's second operand.
    fn commuted(self) -> bool {
        matches!(self, Reassoc::AxYb | Reassoc::XaYb)
    }

    /// Whether `A` is `prev`'s second operand.
    fn a_second(self) -> bool {
        matches!(self, Reassoc::XaBy | Reassoc::XaYb)
    }

    /// `mustSwapOperands`: the root's and the new inner instruction's operand
    /// orders.
    fn swaps(self) -> (bool, bool) {
        match self {
            Reassoc::AxBy => (false, false),
            Reassoc::XaBy => (true, false),
            Reassoc::AxYb | Reassoc::XaYb => (true, true),
        }
    }
}

/// `AArch64InstrInfo::isAssociativeAndCommutative` for the scalar integer
/// forms: `ADDrr`, `ANDrr`, `ORRrr`, `EORrr` and `EONrr`, never the
/// flag-setting ones.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Assoc {
    Add,
    And,
    Orr,
    Eor,
    Eon,
}

/// A reassociable instruction: `rd = rn op rm` with no shift.
#[derive(Clone, Copy, Debug)]
struct Associative {
    op: Assoc,
    width: Width,
    rd: Gpr,
    rn: Gpr,
    rm: Gpr,
}

impl Associative {
    fn of(op: &Op) -> Option<Self> {
        let Op::I(inst) = op else { return None };
        match *inst {
            Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width,
                rd,
                rn,
                rm,
                shift: Shift::Lsl,
                amount: 0,
            } => Some(Self { op: Assoc::Add, width, rd, rn, rm }),
            Inst::LogicShifted { op: logic, invert, width, rd, rn, rm, shift: Shift::Lsl, amount: 0 } => {
                let op = match (logic, invert) {
                    (Logic::And, false) => Assoc::And,
                    (Logic::Orr, false) => Assoc::Orr,
                    (Logic::Eor, false) => Assoc::Eor,
                    (Logic::Eor, true) => Assoc::Eon,
                    _ => return None,
                };
                Some(Self { op, width, rd, rn, rm })
            }
            _ => None,
        }
    }

    /// The same operation with other registers.
    fn with(
        self,
        rd: Gpr,
        rn: Gpr,
        rm: Gpr,
    ) -> Op {
        let (shift, amount) = (Shift::Lsl, 0);
        let logic = |op, invert| Inst::LogicShifted { op, invert, width: self.width, rd, rn, rm, shift, amount };
        Op::I(match self.op {
            Assoc::Add => Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width: self.width,
                rd,
                rn,
                rm,
                shift,
                amount,
            },
            Assoc::And => logic(Logic::And, false),
            Assoc::Orr => logic(Logic::Orr, false),
            Assoc::Eor => logic(Logic::Eor, false),
            Assoc::Eon => logic(Logic::Eor, true),
        })
    }

    fn same_operation(
        self,
        other: Self,
    ) -> bool {
        self.op == other.op && self.width == other.width
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VectorKind {
    /// `MULADDv_OP1/OP2`: `mla`.
    Add,
    /// `MULSUBv_OP2`, `sub c, t`: `mls`.
    SubFrom,
    /// `MULSUBv_OP1`, `sub t, c`: `neg` of `c`, then `mla`.
    SubOf,
}

impl Pattern {
    /// The instruction before the root that the replacement takes away: the
    /// `mul`, or `prev`.
    fn earlier(self) -> usize {
        match self {
            Pattern::Scalar(pattern) => pattern.mul,
            Pattern::Vector { mul, .. } => mul,
            Pattern::Reassoc { prev, .. } => prev,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarPattern {
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

    fn single_vector(
        &self,
        reg: Vreg,
    ) -> bool {
        let key = Reg::Fpr(reg);
        reg.is_virtual() && self.defs.get(&key) == Some(&1) && self.uses.get(&key) == Some(&1)
    }
}

/// Where each register is defined in a block, ascending: `MRI.getUniqueVRegDef`
/// and the walks between two instructions answered by search, not by a scan
/// back through the block per query (a block of N instructions took N
/// scans of up to N operations; one fcmp-heavy file spent half its compile in
/// them).
#[derive(Default)]
struct Definitions(HashMap<Reg, Vec<usize>>);

impl Definitions {
    fn of(ops: &OpList) -> Self {
        let mut found: HashMap<Reg, Vec<usize>> = HashMap::default();
        for (at, op) in ops.iter().enumerate() {
            for reg in op.defs_and_uses().0 {
                let places = found.entry(reg).or_default();
                if places.last() != Some(&at) {
                    places.push(at);
                }
            }
        }
        Self(found)
    }

    /// The last definition of `reg` before `at`.
    fn before(
        &self,
        reg: Reg,
        at: usize,
    ) -> Option<usize> {
        let places = self.0.get(&reg)?;
        places[..places.partition_point(|place| *place < at)].last().copied()
    }

    /// Whether `reg` is defined strictly between `from` and `to`.
    fn between(
        &self,
        reg: Reg,
        from: usize,
        to: usize,
    ) -> bool {
        self.0
            .get(&reg)
            .is_some_and(
                |places| {
                    let first = places.partition_point(|place| *place <= from);
                    places.get(first).is_some_and(|place| *place < to)
                },
            )
    }
}

struct Block<'a> {
    definitions: Definitions,
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
            // A510 ASIMD: `(ADD|SUB|NEG)v` 3, `MULv` (`WriteVq`)
            // and `ML[AS]v` 4.
            Inst::VectorBinary { op: VectorOp::Add | VectorOp::Sub, .. } => 3,
            Inst::VectorIntUnary { op: VectorIntUnary::Neg, .. } => 3,
            Inst::VectorBinary { op: VectorOp::Mul, .. } | Inst::VectorMulAcc { .. } => 4,
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
        self.definitions.between(Reg::Gpr(reg), from, to)
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
        let Some(mul) = self.definitions.before(Reg::Gpr(rn), root) else {
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
        vec![Pattern::Scalar(ScalarPattern { kind: Kind::Immediate(value), mul, other: Gpr::R31, width })]
    }
}

impl Block<'_> {
    /// `MULADDv_OP1/OP2` and `MULSUBv_OP2` of `getMaddPatterns`: the add or
    /// subtract of a lane-wise product read nowhere else.
    fn vector_patterns(
        &self,
        root: usize,
        subtract: bool,
        lanes: Arrangement,
        rn: Vreg,
        rm: Vreg,
    ) -> Vec<Pattern> {
        let candidates: &[(Vreg, Vreg, VectorKind)] = if subtract {
            &[(rm, rn, VectorKind::SubFrom), (rn, rm, VectorKind::SubOf)]
        } else {
            &[(rn, rm, VectorKind::Add), (rm, rn, VectorKind::Add)]
        };
        let mut found = vec![];
        for &(product, other, kind) in candidates {
            if !product.is_virtual() || !other.is_virtual() {
                continue;
            }
            let Some(mul) = self.definitions.before(Reg::Fpr(product), root) else {
                continue;
            };
            let Op::I(Inst::VectorBinary { op: VectorOp::Mul, lanes: l, rd, rn: a, rm: b }) = self.ops[mul] else {
                continue;
            };
            if l != lanes || rd != product || !self.counts.single_vector(product) {
                continue;
            }
            let stable =
                [a, b].iter().all(|reg| reg.is_virtual() && !self.definitions.between(Reg::Fpr(*reg), mul, root));
            if stable {
                found.push(Pattern::Vector { kind, mul, other, lanes });
            }
        }
        found
    }

    /// `genFusedMultiply` of `MLAv`/`MLSv`: the accumulator is a copy of the
    /// other operand, which the instruction then changes.
    fn generate_vector(
        &mut self,
        root: usize,
        pattern: Pattern,
    ) -> Option<Replacement> {
        let Pattern::Vector { kind, mul, other, lanes } = pattern else { return None };
        let Op::I(Inst::VectorBinary { rd, .. }) = self.ops[root] else { return None };
        let Op::I(Inst::VectorBinary { rn: a, rm: b, .. }) = self.ops[mul] else { return None };
        let accumulate = match kind {
            VectorKind::SubOf => Op::I(Inst::VectorIntUnary { op: VectorIntUnary::Neg, lanes, rd, rn: other }),
            _ => Op::Copy { dst: Reg::Fpr(rd), src: Reg::Fpr(other) },
        };
        let op = if kind == VectorKind::SubFrom { MulAcc::Sub } else { MulAcc::Add };
        let made = vec![accumulate, Op::I(Inst::VectorMulAcc { op, lanes, rd, rn: a, rm: b })];
        let inserted = made.iter().map(node_of).collect();
        self.pending = made;
        Some(Replacement { inserted, deleted: vec![mul, root] })
    }
}

impl Block<'_> {
    /// The index of the last instruction before `at` that defines `reg`.
    fn definition_before(
        &self,
        at: usize,
        reg: Gpr,
    ) -> Option<usize> {
        self.definitions.before(Reg::Gpr(reg), at)
    }

    /// `hasReassociableOperands`: both operands virtual, at least one made in
    /// the block. The pin asks for a unique definition; a register with
    /// several is the result of a phi (the pin's `PHI` of the block), whose
    /// value is the one at the block's entry, and the rule below keeps
    /// either operand from changing between the two instructions.
    fn reassociable_operands(
        &self,
        at: usize,
        rn: Gpr,
        rm: Gpr,
    ) -> bool {
        let phi = |reg: Gpr| self.counts.defs.get(&Reg::Gpr(reg)).is_some_and(|defs| *defs > 1);
        rn.is_virtual()
            && rm.is_virtual()
            && rn != rm
            && [rn, rm].iter().any(|reg| self.definition_before(at, *reg).is_some() || phi(*reg))
    }

    /// `getMachineCombinerPatterns` through `isReassociationCandidate`: the
    /// root is associative, its operands are reassociable, and one of them
    /// is the result of an instruction of the same operation (`prev`) that
    /// nothing else reads and whose own operands are reassociable.
    fn reassoc_patterns(
        &self,
        root: usize,
    ) -> Vec<Pattern> {
        let Some(this) = Associative::of(&self.ops[root]) else { return vec![] };
        if !self.reassociable_operands(root, this.rn, this.rm) {
            return vec![];
        }
        let sibling = |reg: Gpr| {
            let at = self.definition_before(root, reg)?;
            Associative::of(&self.ops[at]).filter(|found| found.same_operation(this)).map(|found| (at, found))
        };
        let (first, second) = (sibling(this.rn), sibling(this.rm));
        let commuted = first.is_none() && second.is_some();
        let Some((prev, found)) = (if commuted { second } else { first }) else { return vec![] };
        if !self.counts.single(found.rd) || !self.reassociable_operands(prev, found.rn, found.rm) {
            return vec![];
        }
        // The new instructions stand where the root does: neither `A` nor `X`
        // may change in between.
        if [found.rn, found.rm].iter().any(|reg| self.defined_between(prev, root, *reg)) {
            return vec![];
        }
        let shapes = if commuted { [Reassoc::AxYb, Reassoc::XaYb] } else { [Reassoc::AxBy, Reassoc::XaBy] };
        shapes.into_iter().map(|shape| Pattern::Reassoc { shape, prev }).collect()
    }

    /// `reassociateOps`: `B = A op X; C = B op Y` becomes `N = X op Y; C = A op
    /// N`.
    fn generate_reassoc(
        &mut self,
        root: usize,
        shape: Reassoc,
        prev: usize,
    ) -> Option<Replacement> {
        let (root_op, prev_op) = (Associative::of(&self.ops[root])?, Associative::of(&self.ops[prev])?);
        let (a, x) = if shape.a_second() { (prev_op.rm, prev_op.rn) } else { (prev_op.rn, prev_op.rm) };
        let y = if shape.commuted() { root_op.rn } else { root_op.rm };
        let (swap_root, swap_prev) = shape.swaps();
        let new = self.fresh();
        let (first, second) = if swap_prev { (y, x) } else { (x, y) };
        let (left, right) = if swap_root { (new, a) } else { (a, new) };
        let made = vec![prev_op.with(new, first, second), root_op.with(root_op.rd, left, right)];
        let inserted = made.iter().map(node_of).collect();
        self.pending = made;
        Some(Replacement { inserted, deleted: vec![prev, root] })
    }
}

impl Block<'_> {
    /// `getMaddPatterns`: ADDWrr/ADDXrr, SUBWrr/SUBXrr.
    fn madd_patterns(
        &self,
        root: usize,
    ) -> Vec<Pattern> {
        if let Op::I(Inst::VectorBinary { op: op @ (VectorOp::Add | VectorOp::Sub), lanes, rn, rm, .. }) =
            self.ops[root]
        {
            return self.vector_patterns(root, op == VectorOp::Sub, lanes, rn, rm);
        }
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
            let Some(mul) = self.definitions.before(Reg::Gpr(product), root) else {
                continue;
            };
            let Some((a, b)) = self.product_at(mul, product, width) else { continue };
            // The zero register is the same value everywhere.
            let readable = |reg: Gpr| reg.is_virtual() || reg == Gpr::R31;
            // Only the product's operands are read later than the `mul` read
            // them; `other` is read by the root where it stands.
            let stable =
                readable(other) && [a, b].iter().all(|reg| readable(*reg) && !self.defined_between(mul, root, *reg));
            if stable {
                found.push(Pattern::Scalar(ScalarPattern { kind, mul, other, width }));
            }
        }
        found
    }
}

impl CombinerIsa for Block<'_> {
    type Pattern = Pattern;

    fn nodes(&self) -> Vec<Node> {
        self.ops.iter().map(node_of).collect()
    }

    /// `AArch64InstrInfo::getMachineCombinerPatterns`: the madd patterns, and
    /// where there are none the generic reassociation ones.
    fn patterns(
        &self,
        root: usize,
    ) -> Vec<Pattern> {
        let found = self.madd_patterns(root);
        if found.is_empty() { self.reassoc_patterns(root) } else { found }
    }

    /// `MachineCombinerImpl::getCombinerObjective`: a reassociation must make
    /// the root shallower.
    fn objective(
        &self,
        pattern: Pattern,
    ) -> Objective {
        match pattern {
            Pattern::Reassoc { .. } => Objective::MustReduceDepth,
            _ => Objective::Default,
        }
    }

    /// `genAlternativeCodeSequence`.
    fn generate(
        &mut self,
        root: usize,
        pattern: Pattern,
    ) -> Option<Replacement> {
        let pattern = match pattern {
            Pattern::Scalar(pattern) => pattern,
            Pattern::Reassoc { shape, prev } => return self.generate_reassoc(root, shape, prev),
            Pattern::Vector { .. } => return self.generate_vector(root, pattern),
        };
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
        self.ops.remove(pattern.earlier());
        for (index, op) in std::mem::take(&mut self.pending).into_iter().enumerate() {
            self.ops.insert_at(root - 1 + index, op, loc);
        }
        self.definitions = Definitions::of(self.ops);
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
        let mut view = Block {
            definitions: Definitions::of(&block.ops),
            ops: &mut block.ops,
            next_register,
            counts: &counts,
            pending: vec![],
        };
        combine::combined(&mut view, Setting { optimize_for_size, in_loop: false });
    }
}

#[cfg(test)]
#[path = "combine_tests.rs"]
mod tests;
