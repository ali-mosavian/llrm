//! `SLPVectorizer`'s store chains: consecutive stores of one scalar type
//! become vector stores when the cost model says the vector is cheaper.
//!
//! # How it works
//! `vectorizeStores` (l.29748): the stores of a block are grouped by the
//! object they address and the type they store, each group sorted by offset,
//! and cut into runs of adjacent cells. Over a run the vectorization factor
//! falls from the widest power of two the vector register and the run hold to
//! the target's `getStoreMinimumVF`; at each factor the run is tried in
//! windows of that many stores, a window that vectorizes is not tried again.
//! A window is a tree of the stores and the vector of their values; its cost
//! is the vector store less the scalar ones, the values' own (a vector of
//! constants costs nothing), and it vectorizes when that is below the
//! `slp-threshold` of 0 (`getTreeCost` l.19795, rejection l.19079). The
//! vector store stands at the last store of the window, with the address and
//! alignment of the first.
//!
//! A window whose values are not all constants is a tree of the loads,
//! arithmetic, casts, compares, selects and `llvm.fmuladd` that make them
//! (`slptree`); the first such window that pays is made and the function is
//! looked at again, so no plan names a scalar another has gone from.
//!
//! DEVIATE SLP1 (owner arm64, exit 2026-11-24): the narrower tree
//! `slptree` states.
//!
//! The phis of a block seed trees too (`vectorizeChainsInBlock` l.35538): they
//! are sorted by type, by the values that reach them (`PHICompare`), cut into
//! runs that are alike (`AreCompatiblePHIs`) and each run is tried in windows
//! (`tryToVectorizeSequence` l.34803, `tryToVectorizeList` l.30044). Before
//! the store chains, as the pin's block loop does. DEVIATE SLP5 (owner arm64,
//! exit 2026-11-27): two incoming binary operations are alike only when they
//! are one opcode (no alternate-opcode state); a block stands before another
//! in `PHICompare` by its place in the layout, not by the dominator tree's
//! DFS number; the element size is the phi's own type (`getVectorElementSize`
//! reads the loads that feed it).
//!
//! # Runs
//! At the pin's `slp-vectorizer` slot (`PassBuilderPipelines.cpp` l.1455).
//!
//! # Used by
//! `pipeline` (Late).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use llrm_analysis::memory::{MemRef, Unit as MemUnit};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::edit::Position;
use llrm_mir::module::{BlockId, Function, InstId, Operand};
use llrm_mir::opcode::{Flags, Opcode};
use llrm_mir::passes::{Analyses, Declared, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::tti::{CostKind, MemoryOp, Tti, Ty, VectorTy};
use llrm_mir::types::{FloatKind, Type, TypeId};

/// `-slp-threshold`: a tree is taken when its cost is below minus this.
const COST_THRESHOLD: i64 = 0;

/// One store of a chain.
#[derive(Clone)]
struct Seed {
    store: InstId,
    /// The store's place in its block.
    order: usize,
    disp: i64,
    align: Option<u64>,
    value: Operand,
}

/// The scalar `ty` as the cost hooks name it.
fn lane_of(
    context: &Context,
    ty: TypeId,
) -> Option<(Ty, u64)> {
    match context.types.get(ty) {
        Type::Int(bits @ (8 | 16 | 32 | 64)) => Some((Ty::Int(*bits), u64::from(*bits / 8))),
        Type::Float(FloatKind::Float) => Some((Ty::Float, 4)),
        Type::Float(FloatKind::Double) => Some((Ty::Double, 8)),
        _ => None,
    }
}

/// Stores grouped by the object and type they write, each group in offset
/// order with the runs of adjacent cells cut apart (`collectSeedInstructions`
/// l.30007 and the chain sort of `vectorizeStoreChains` l.36033).
fn chains(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
) -> Vec<(TypeId, Vec<Seed>)> {
    // One object, one variable index (the stores of a row of an image share
    // the column), one type.
    let mut groups: Vec<((Operand, Option<llrm_mir::module::ValueId>, i64), TypeId, Vec<Seed>)> = Vec::new();
    for (order, inst) in function.iter_block(block).enumerate() {
        let instruction = function.instruction(inst);
        let Opcode::Store { align, volatile: false } = instruction.opcode else { continue };
        let value = instruction.operands[0];
        let Some(ty) = function.operand_type(unit.context, value) else { continue };
        let Some((_, bytes)) = lane_of(unit.context, ty) else { continue };
        let reference = MemRef::at(unit, instruction.operands[1], bytes as u32);
        let Some(root) = reference.root.filter(|_| reference.segment.is_none()) else { continue };
        let key = (root, reference.base, reference.scale);
        let seed = Seed { store: inst, order, disp: reference.disp, align, value };
        match groups.iter_mut().find(|group| group.0 == key && group.1 == ty) {
            Some(group) => group.2.push(seed),
            None => groups.push((key, ty, vec![seed])),
        }
    }
    let mut runs = Vec::new();
    for (_, ty, mut seeds) in groups {
        let bytes = lane_of(unit.context, ty).expect("a lane type").1 as i64;
        seeds.sort_by_key(|seed| seed.disp);
        seeds.dedup_by_key(|seed| seed.disp);
        let mut run: Vec<Seed> = Vec::new();
        for seed in seeds {
            if run.last().is_some_and(|last| seed.disp != last.disp + bytes) {
                runs.push((ty, std::mem::take(&mut run)));
            }
            run.push(seed);
        }
        runs.push((ty, run));
    }
    runs
}

/// A window of `vf` stores is one vector store: its cost below the threshold.
fn worthwhile(
    tti: &dyn Tti,
    lane: Ty,
    align: u64,
    window: &[Seed],
    context: &Context,
) -> bool {
    let vf = window.len() as u32;
    let kind = CostKind::RecipThroughput;
    // `getEntryCost` of the store entry (l.17322): the vector store less the
    // scalar ones.
    let Some(vector) = tti.vector_memory_op_cost(MemoryOp::Store, VectorTy { lane, count: vf }, align, kind) else {
        return false;
    };
    let scalar = tti.memory_op_cost(MemoryOp::Store, lane, kind, true);
    if !vector.valid || !scalar.valid {
        return false;
    }
    // The values: a vector of constants is a constant (`getGatherCost`,
    // l.21526).
    let constants = window
        .iter()
        .all(
            |seed| matches!(
                seed.value,
                Operand::Constant(id) if is_scalar_constant(context, id)
            ),
        );
    constants && vector.value - scalar.value * i64::from(vf) < -COST_THRESHOLD
}

fn is_scalar_constant(
    context: &Context,
    id: llrm_mir::context::ConstantId,
) -> bool {
    matches!(
        context.get(id).kind,
        ConstantKind::Int(_) | ConstantKind::Float(_) | ConstantKind::Zero | ConstantKind::Null
    )
}

/// Nothing between the window's first and last store reads or writes the bytes
/// the window's own stores write (`BlockScheduling`, as `slptree`'s
/// `StoreSeparation`): the vector store may stand at the last.
fn movable(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    window: &[Seed],
) -> bool {
    let first = window.iter().map(|seed| seed.order).min().expect("a window");
    let last = window.iter().map(|seed| seed.order).max().expect("a window");
    let mut separation = crate::slptree::StoreSeparation::new(unit, window.iter().map(|seed| seed.store));
    function
        .iter_block(block)
        .enumerate()
        .skip(first)
        .take(last - first + 1)
        .all(
            |(order, inst)| {
                let own = window.iter().any(|seed| seed.order == order);
                own || !matches!(
                    function.instruction(inst).opcode,
                    Opcode::Load { .. }
                        | Opcode::Store { .. }
                        | Opcode::Call(_)
                        | Opcode::Invoke(_)
                        | Opcode::Atomic(_)
                        | Opcode::VaArg
                )
                    || separation.clear(function, inst)
            },
        )
}

/// The vector store in place of `window`'s stores.
fn vectorize(
    context: &mut Context,
    function: &mut Function,
    ty: TypeId,
    window: &[Seed],
) {
    let vf = window.len();
    let vector_ty = context.types.intern(Type::Vector { element: ty, count: vf as u32 });
    let first = &window[0];
    let last = window.iter().max_by_key(|seed| seed.order).expect("a window");
    let pointer = function.instruction(first.store).operands[1];
    let Operand::Constant(lane) = first.value else { unreachable!("a window of constants") };
    let kind = match context.get(lane).kind.clone() {
        ConstantKind::Int(0) | ConstantKind::Float(0) | ConstantKind::Zero => ConstantKind::Zero,
        _ if window.iter().all(|seed| seed.value == first.value) => ConstantKind::Aggregate(vec![lane; vf]),
        _ => ConstantKind::Aggregate(
            window
                .iter()
                .map(|seed| match seed.value {
                    Operand::Constant(one) => one,
                    _ => unreachable!("a window of constants"),
                })
                .collect(),
        ),
    };
    let value = Operand::Constant(context.constant(Constant { ty: vector_ty, kind }));
    let void = context.types.void();
    let store = function.create_instruction(
        Opcode::Store { align: first.align, volatile: false },
        void,
        vec![value, pointer],
        Flags::default(),
        None,
    );
    function.insert(store, Position::After(last.store)).expect("the stores' block");
    for seed in window {
        function.erase(seed.store).expect("a store");
    }
}

/// `-slp-min-reg-size`.
const MIN_VEC_REG_BITS: u64 = 128;
/// `MaxPHINumOperands`: a block's phis are read until one has more.
const MAX_PHI_OPERANDS: usize = 128;

/// LLVM's opcode numbers (`Instruction.def`), as `PHICompare` orders by them.
fn opcode_number(opcode: &Opcode) -> u32 {
    use llrm_mir::opcode::{BinaryOp as B, CastOp as C};
    match opcode {
        Opcode::Binary(op) => match op {
            B::Add => 13,
            B::FAdd => 14,
            B::Sub => 15,
            B::FSub => 16,
            B::Mul => 17,
            B::FMul => 18,
            B::UDiv => 19,
            B::SDiv => 20,
            B::FDiv => 21,
            B::URem => 22,
            B::SRem => 23,
            B::FRem => 24,
            B::Shl => 25,
            B::LShr => 26,
            B::AShr => 27,
            B::And => 28,
            B::Or => 29,
            B::Xor => 30,
        },
        Opcode::Alloca { .. } => 31,
        Opcode::Load { .. } => 32,
        Opcode::Store { .. } => 33,
        Opcode::GetElementPtr { .. } => 34,
        Opcode::Cast(op) => match op {
            C::Trunc => 38,
            C::ZExt => 39,
            C::SExt => 40,
            C::FPToUI => 41,
            C::FPToSI => 42,
            C::UIToFP => 43,
            C::SIToFP => 44,
            C::FPTrunc | C::FPRound => 45,
            C::FPExt => 46,
            C::PtrToInt => 47,
            C::IntToPtr => 48,
            C::BitCast => 49,
            C::AddrSpaceCast => 50,
        },
        Opcode::ICmp(_) => 53,
        Opcode::FCmp(_) => 54,
        Opcode::Phi => 55,
        Opcode::Call(_) => 56,
        Opcode::Select => 57,
        Opcode::VaArg => 60,
        Opcode::ExtractElement => 61,
        Opcode::InsertElement => 62,
        Opcode::ShuffleVector(_) => 63,
        Opcode::ExtractValue(_) => 64,
        Opcode::InsertValue(_) => 65,
        Opcode::LandingPad { .. } => 66,
        Opcode::Freeze => 67,
        _ => 1,
    }
}

/// `getNumberOfParts` of `count` lanes of `bits`: the registers the vector
/// splits into.
fn parts_of(
    register_bits: u64,
    bits: u64,
    count: u64,
) -> u64 {
    (count * bits).div_ceil(register_bits).max(1)
}

/// `getFloorFullVectorNumberOfElements`.
fn floor_full(
    register_bits: u64,
    bits: u64,
    size: u64,
) -> u64 {
    let parts = parts_of(register_bits, bits, size);
    let floor = if size == 0 { 0 } else { 1 << size.ilog2() };
    if parts >= size {
        return floor;
    }
    let per_register = size.div_ceil(parts).next_power_of_two();
    if per_register > size { floor } else { size / per_register * per_register }
}

/// `hasFullVectorsOrPowerOf2`.
fn full_or_power_of_two(
    register_bits: u64,
    bits: u64,
    size: u64,
) -> bool {
    if size <= 1 {
        return false;
    }
    if size.is_power_of_two() {
        return true;
    }
    let parts = parts_of(register_bits, bits, size);
    parts < size && (size / parts).is_power_of_two() && size % parts == 0
}

/// The phis of the blocks as `vectorizeChainsInBlock` seeds them.
struct PhiSeeds<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    declared: &'a mut Declared,
    function: &'a mut Function,
    outer: &'a Outer,
    tti: &'a dyn Tti,
    /// `PHIToOpcodes`: the values that reach each phi, other than phis.
    opcodes: HashMap<InstId, Vec<Operand>>,
    /// `isAnalyzedBundle`.
    analyzed: HashSet<Vec<InstId>>,
}

impl PhiSeeds<'_> {
    fn bits_of(
        &self,
        phi: InstId,
    ) -> Option<u64> {
        let ty = self.function.instruction(phi).ty;
        lane_of(self.context, ty).map(|(_, bytes)| bytes * 8)
    }

    fn instruction_of(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(id) = operand else { return None };
        match self.function.value(id).def {
            llrm_mir::module::ValueDef::Instruction(inst) if self.function.parent(inst).is_some() => Some(inst),
            _ => None,
        }
    }

    fn undefined(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            operand,
            Operand::Constant(id) if matches!(self.context.get(id).kind, ConstantKind::Undef | ConstantKind::Poison)
        )
    }

    fn block_rank(
        &self,
        block: BlockId,
    ) -> usize {
        self.function.layout().iter().position(|one| *one == block).unwrap_or(usize::MAX)
    }

    /// The non-phi values that reach `phi` through phis.
    fn reaching(
        &self,
        phi: InstId,
    ) -> Vec<Operand> {
        let mut reached = Vec::new();
        let mut nodes = vec![phi];
        let mut visited = HashSet::new();
        while let Some(node) = nodes.pop() {
            if !visited.insert(node) {
                continue;
            }
            for pair in self.function.instruction(node).operands.chunks(2) {
                match self.instruction_of(pair[0]).filter(|one| self.function.instruction(*one).opcode == Opcode::Phi) {
                    Some(inner) => nodes.push(inner),
                    None => reached.push(pair[0]),
                }
            }
        }
        reached
    }

    /// `PHICompare`: whether `a` stands before `b`.
    fn compare(
        &self,
        a: InstId,
        b: InstId,
    ) -> Ordering {
        if a == b {
            return Ordering::Equal;
        }
        let (one, two) = (self.function.instruction(a), self.function.instruction(b));
        let rank = |ty| match self.context.types.get(ty) {
            Type::Float(FloatKind::Float) => (3, 32),
            Type::Float(_) => (4, 64),
            Type::Int(bits) => (12, u32::from(*bits)),
            _ => (99, 0),
        };
        // Types by id, then by size.
        let by_type = rank(one.ty).cmp(&rank(two.ty));
        if by_type != Ordering::Equal {
            return by_type;
        }
        let (first, second) = (&self.opcodes[&a], &self.opcodes[&b]);
        let by_count = first.len().cmp(&second.len());
        if by_count != Ordering::Equal {
            return by_count;
        }
        for (x, y) in first.iter().zip(second) {
            // Instructions come first, by block and then by opcode.
            match (self.instruction_of(*x), self.instruction_of(*y)) {
                (Some(i1), Some(i2)) => {
                    let (b1, b2) = (self.function.parent(i1).unwrap(), self.function.parent(i2).unwrap());
                    if b1 != b2 {
                        return self.block_rank(b1).cmp(&self.block_rank(b2));
                    }
                    let (n1, n2) =
                        (opcode_number(&self.function.instruction(i1).opcode), opcode_number(&self.function.instruction(i2).opcode));
                    if n1 != n2 {
                        return n1.cmp(&n2);
                    }
                    continue;
                }
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => {}
            }
            // Non-undef constants next.
            let constant = |operand| matches!(operand, Operand::Constant(_)) && !self.undefined(operand);
            match (constant(*x), constant(*y)) {
                (true, true) => continue,
                (true, false) => return Ordering::Less,
                (false, true) => return Ordering::Greater,
                _ => {}
            }
            // Non-constant non-instructions (arguments) before undefs.
            match (self.undefined(*x), self.undefined(*y)) {
                (false, false) => continue,
                (false, true) => return Ordering::Less,
                (true, false) => return Ordering::Greater,
                (true, true) => {}
            }
        }
        Ordering::Equal
    }

    /// `AreCompatiblePHIs`: `last` (the run's back) and `next` take one tree.
    fn compatible(
        &self,
        last: InstId,
        next: InstId,
    ) -> bool {
        if next == last {
            return true;
        }
        if self.function.instruction(next).ty != self.function.instruction(last).ty {
            return false;
        }
        let (first, second) = (&self.opcodes[&next], &self.opcodes[&last]);
        if first.len() != second.len() {
            return false;
        }
        for (x, y) in first.iter().zip(second) {
            if self.undefined(*x) || self.undefined(*y) {
                continue;
            }
            if let (Some(i1), Some(i2)) = (self.instruction_of(*x), self.instruction_of(*y)) {
                if self.function.is_erased(i1) || self.function.is_erased(i2) {
                    return false;
                }
                if self.function.parent(i1) != self.function.parent(i2) {
                    return false;
                }
                if self.function.instruction(i1).opcode == self.function.instruction(i2).opcode
                    && self.function.instruction(i1).ty == self.function.instruction(i2).ty
                {
                    continue;
                }
                return false;
            }
            if matches!(x, Operand::Constant(_)) && matches!(y, Operand::Constant(_)) {
                continue;
            }
            if std::mem::discriminant(x) != std::mem::discriminant(y)
                || self.instruction_of(*x).is_some() != self.instruction_of(*y).is_some()
            {
                return false;
            }
        }
        true
    }

    /// The tree of `ops`, if it pays.
    fn tree_of(
        &self,
        block: BlockId,
        ops: &[InstId],
    ) -> Option<crate::slptree::Tree> {
        let unit = MemUnit::within(self.context, self.layout, self.function, self.outer);
        let mut session = llrm_analysis::scev::analysis::ScevSession::of(self.context, self.function, self.outer);
        session.ask_within(
            self.context,
            self.layout,
            &self.outer.globals,
            &self.outer.metadata,
            self.function,
            |se, cx| {
                let scaler = crate::slptree::Scaler::new(se, cx);
                crate::slptree::try_phis(&unit, self.function, block, ops, self.tti, &scaler)
            },
        )
    }

    /// `tryToVectorizeList` of phis.
    fn try_list(
        &mut self,
        block: BlockId,
        list: &[InstId],
        max_vf_only: bool,
    ) -> bool {
        if list.len() < 2 {
            return false;
        }
        let Some(bits) = self.bits_of(list[0]) else { return false };
        let register = u64::from(self.tti.vector_register_bits());
        let min_vf = (MIN_VEC_REG_BITS / bits).max(2);
        let max_vf = floor_full(register, bits, list.len() as u64).max(min_vf);
        if max_vf < 2 {
            return false;
        }
        let mut changed = false;
        let (mut next, max) = (0usize, list.len());
        let mut vf = max_vf;
        while next + 1 < max && vf >= min_vf {
            if parts_of(register, bits, vf) != vf {
                let mut at = next;
                while at < max {
                    let actual = (max - at).min(vf as usize) as u64;
                    if !full_or_power_of_two(register, bits, actual) {
                        at += 1;
                        continue;
                    }
                    if max_vf_only && actual < max_vf {
                        break;
                    }
                    if (vf > min_vf && actual < vf) || (vf == min_vf && actual < 2) {
                        break;
                    }
                    let ops: Vec<InstId> = list[at..]
                        .iter()
                        .copied()
                        .filter(|phi| !self.function.is_erased(*phi))
                        .take(actual as usize)
                        .collect();
                    if ops.len() as u64 != actual {
                        break;
                    }
                    if self.analyzed.contains(&ops) {
                        at += 1;
                        continue;
                    }
                    match self.tree_of(block, &ops) {
                        Some(tree) => {
                            crate::slptree::emit(self.context, self.declared, self.function, &tree, None);
                            at += vf as usize - 1;
                            next = at + 1;
                            changed = true;
                        }
                        None => {
                            self.analyzed.insert(ops);
                        }
                    }
                    at += 1;
                }
            }
            vf = floor_full(register, bits, vf - 1);
            if vf < 2 {
                break;
            }
        }
        changed
    }

    /// `tryToVectorizeSequence` over `incoming`, sorted by `PHICompare`.
    fn sequence(
        &mut self,
        block: BlockId,
        mut incoming: Vec<InstId>,
    ) -> bool {
        incoming.sort_by(|a, b| self.compare(*a, *b));
        let mut changed = false;
        let mut candidates: Vec<InstId> = Vec::new();
        let live = |this: &Self, list: &[InstId]| -> Vec<InstId> {
            list.iter().copied().filter(|phi| !this.function.is_erased(*phi)).collect()
        };
        let mut at = 0;
        while at < incoming.len() {
            if self.function.is_erased(incoming[at]) || self.bits_of(incoming[at]).is_none() {
                at += 1;
                continue;
            }
            let start = at;
            let mut run: Vec<InstId> = Vec::new();
            while at < incoming.len()
                && (self.function.is_erased(incoming[at])
                    || run.last().is_none_or(|last| self.compatible(*last, incoming[at])))
            {
                if !self.function.is_erased(incoming[at]) {
                    run.push(incoming[at]);
                }
                at += 1;
            }
            let bits = self.bits_of(incoming[start]).expect("a lane type");
            if run.len() > 1 && self.try_list(block, &run, true) {
                changed = true;
                candidates = live(self, &run);
            } else if (run.len() as u64) < (u64::from(self.tti.vector_register_bits()) / bits).max(2)
                && (candidates.is_empty()
                    || self.function.instruction(candidates[0]).ty == self.function.instruction(incoming[start]).ty)
            {
                candidates.extend(live(self, &run));
            }
            // The last of its type: what is left is tried at any factor.
            let boundary = at >= incoming.len()
                || self.function.instruction(incoming[at]).ty != self.function.instruction(incoming[start]).ty;
            if candidates.len() > 1 && boundary {
                if self.try_list(block, &candidates, false) {
                    changed = true;
                } else {
                    let mut rest = 0;
                    while rest < candidates.len() {
                        if self.function.is_erased(candidates[rest]) {
                            rest += 1;
                            continue;
                        }
                        let from = rest;
                        let mut small: Vec<InstId> = Vec::new();
                        while rest < candidates.len()
                            && (self.function.is_erased(candidates[rest])
                                || self.compatible(candidates[rest], candidates[from]))
                        {
                            if !self.function.is_erased(candidates[rest]) {
                                small.push(candidates[rest]);
                            }
                            rest += 1;
                        }
                        if small.len() > 1 && self.try_list(block, &small, false) {
                            changed = true;
                        }
                    }
                }
                candidates.clear();
            }
        }
        changed
    }

    /// `vectorizeChainsInBlock`'s phi loop.
    fn block(
        &mut self,
        block: BlockId,
    ) -> bool {
        let mut changed = false;
        let mut visited: HashSet<InstId> = HashSet::new();
        loop {
            let mut incoming: Vec<InstId> = Vec::new();
            for inst in self.function.iter_block(block).collect::<Vec<_>>() {
                let phi = self.function.instruction(inst);
                if phi.opcode != Opcode::Phi || phi.operands.len() / 2 > MAX_PHI_OPERANDS {
                    break;
                }
                if !visited.contains(&inst) && self.bits_of(inst).is_some() {
                    incoming.push(inst);
                }
            }
            if incoming.len() <= 1 {
                break;
            }
            for phi in &incoming {
                if !self.opcodes.get(phi).is_some_and(|reached| !reached.is_empty()) {
                    let reached = self.reaching(*phi);
                    self.opcodes.insert(*phi, reached);
                }
            }
            let vectorized = self.sequence(block, incoming.clone());
            changed |= vectorized;
            if vectorized {
                self.opcodes.clear();
            }
            visited.extend(incoming);
            if !vectorized {
                break;
            }
        }
        changed
    }
}

/// The phis of each block as seeds; whether any tree was made.
fn run_phis(
    context: &mut Context,
    layout: &DataLayout,
    declared: &mut Declared,
    function: &mut Function,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let mut changed = false;
    let blocks = function.layout().to_vec();
    let mut seeds = PhiSeeds {
        context,
        layout,
        declared,
        function,
        outer,
        tti,
        opcodes: HashMap::new(),
        analyzed: HashSet::new(),
    };
    for block in blocks {
        changed |= seeds.block(block);
    }
    changed
}

/// Store chains to vector stores in `function`; whether any was made.
fn run_function(
    context: &mut Context,
    layout: &DataLayout,
    declared: &mut Declared,
    function: &mut Function,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let phis = run_phis(context, layout, declared, function, outer, tti);
    let mut plan: Vec<(TypeId, Vec<Seed>)> = Vec::new();
    {
        let unit = MemUnit::within(context, layout, function, outer);
        for block in function.layout().to_vec() {
            for (ty, run) in chains(&unit, function, block) {
                let (lane, bytes) = lane_of(unit.context, ty).expect("a lane type");
                let widest = (u64::from(tti.vector_register_bits()) / (bytes * 8)).max(1);
                let mut vf = (widest.min(run.len() as u64)).next_power_of_two();
                if vf > widest.min(run.len() as u64) {
                    vf /= 2;
                }
                let floor = tti.store_minimum_vf(2, lane, lane).max(2);
                let mut taken = vec![false; run.len()];
                while vf >= u64::from(floor).max(2) {
                    let mut at = 0;
                    while at + vf as usize <= run.len() {
                        let window = &run[at..at + vf as usize];
                        let free = !taken[at..at + vf as usize].iter().any(|&one| one);
                        let align = window[0].align.unwrap_or(bytes);
                        if free
                            && worthwhile(tti, lane, align, window, unit.context)
                            && movable(&unit, function, block, window)
                        {
                            taken[at..at + vf as usize].fill(true);
                            plan.push((ty, window.to_vec()));
                            at += vf as usize;
                        } else {
                            at += 1;
                        }
                    }
                    vf /= 2;
                }
            }
        }
    }
    for (ty, window) in &plan {
        vectorize(context, function, *ty, window);
    }
    let mut changed = phis || !plan.is_empty();
    // One tree at a time, the function read again after each.
    while let Some((tree, align)) = next_tree(context, layout, function, outer, tti) {
        crate::slptree::emit(context, declared, function, &tree, align);
        changed = true;
    }
    changed
}

/// The first window of stores, by `vectorizeStores`' order, whose values make a
/// tree that pays.
fn next_tree(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    outer: &Outer,
    tti: &dyn Tti,
) -> Option<(crate::slptree::Tree, Option<u64>)> {
    let unit = MemUnit::within(context, layout, function, outer);
    let mut session = llrm_analysis::scev::analysis::ScevSession::of(context, function, outer);
    session.ask_within(context, layout, &outer.globals, &outer.metadata, function, |se, cx| {
        let scaler = crate::slptree::Scaler::new(se, cx);
        for block in function.layout().to_vec() {
            for (ty, run) in chains(&unit, function, block) {
                let (lane, bytes) = lane_of(unit.context, ty).expect("a lane type");
                let widest = (u64::from(tti.vector_register_bits()) / (bytes * 8)).max(1);
                let mut vf = (widest.min(run.len() as u64)).next_power_of_two();
                if vf > widest.min(run.len() as u64) {
                    vf /= 2;
                }
                let floor = tti.store_minimum_vf(2, lane, lane).max(2);
                while vf >= u64::from(floor).max(2) {
                    for window in run.windows(vf as usize) {
                        if window.iter().all(|seed| matches!(seed.value, Operand::Constant(_))) {
                            continue;
                        }
                        let stores: Vec<InstId> = window.iter().map(|seed| seed.store).collect();
                        let align = window[0].align;
                        if let Some(tree) = crate::slptree::try_window(
                            &unit,
                            function,
                            block,
                            &stores,
                            ty,
                            align.unwrap_or(bytes),
                            tti,
                            &scaler,
                        ) {
                            return Some((tree, align));
                        }
                    }
                    vf /= 2;
                }
            }
        }
        None
    })
}

/// Store chains to vector stores, on a target with vector registers.
pub struct SlpVectorize;

impl FunctionPass for SlpVectorize {
    fn name(&self) -> &'static str {
        "slpvectorize"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("slp-vectorizer")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let chains = outer
            .target()
            .tti()
            .is_some_and(
                |tti| tti.slp_priced() && tti.vector_register_bits() > 0 && outer.target().has_vector_mir() && {
                    let stores = run_function(unit.context, unit.layout, unit.declared, unit.function, &outer, &*tti);
                    let reductions = crate::slpreduction::run_function(
                        unit.context,
                        unit.layout,
                        unit.function,
                        unit.declared,
                        &outer,
                        &*tti,
                    );
                    stores || reductions
                },
            );
        // The pin's one pass also makes the wide compare of an `and` chain; it
        // runs wherever it did before this pass held the store chains.
        let compares = crate::slpcompares::SlpCompares.run(unit, analyses);
        if chains { PreservedAnalyses::none() } else { compares }
    }
}

#[cfg(test)]
#[path = "slpvectorize_tests.rs"]
mod tests;
