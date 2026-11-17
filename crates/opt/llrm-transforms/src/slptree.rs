//! `SLPVectorizer`'s tree: a window of consecutive stores, and the loads,
//! arithmetic, casts, compares, selects and `llvm.fmuladd` calls that make
//! their values, become vector code when the cost model says the vectors are
//! cheaper.
//!
//! # How it works
//! `BoUpSLP::buildTree` (l.12052 `buildTreeRec`) starts from the stores' values
//! and goes up the operands, one entry for each bundle of one lane per store:
//! the bundle of loads of consecutive cells, of one binary operation, one cast,
//! one compare, one select, one `fneg`, one `llvm.fmuladd`, or of extracts of
//! the lanes of one vector (that vector is the bundle); a bundle that is none
//! of these (constants, one value in every lane, anything else) is a gather,
//! the vector built from its scalars. A bundle that repeats a lane is made once
//! over the distinct lanes and read through a shuffle (`slpgather`). The
//! operands of a commutative operation are put in the order that makes the
//! bundles alike (`VLOperands::reorder`, `getShallowScore` l.1119).
//! `getTreeCost` (l.19795) adds each entry's vector cost less its scalar ones
//! (`getEntryCost` l.16168) and, for each scalar the tree does not use, the
//! extract of its lane; the tree is taken below the `slp-threshold` of 0.
//! `vectorizeTree` makes each bundle's vector after its last scalar, and the
//! vector store at the last store.
//!
//! DEVIATE SLP1 (owner arm64, exit 2026-11-24): the tree has no reordered or
//! reversed loads, no alternate-opcode or gather-of-loads entries and
//! no operand look-ahead past the shallow score; the scale by the loop's trip
//! count is left out (it multiplies every entry alike, so the sign of the
//! total is the same). An external user placed before its bundle's last
//! scalar makes the tree not form (the pin's scheduler may order it). A scalar
//! the tree makes as a vector and a gather also reads is extracted for the
//! gather (`ExternalUses`), when its bundle's last scalar stands before the
//! gather's user's.
//!
//! DEVIATE SLP3 (owner arm64, exit 2026-11-24): `BlockScheduling`'s dependency
//! graph is not built: the loads must all stand before the first store, and
//! nothing else between the first load and the last store may write memory, nor
//! read it after the first store. A load or store between the window's stores
//! that `BasicAA`, scoped-noalias and TBAA say cannot touch their bytes may stay
//! (`StoreSeparation`); a call may not.
//!
//! # Runs
//! Under `slpvectorize`, on a window of stores whose values are not all
//! constants.
//!
//! # Used by
//! `slpvectorize`.

use llrm_analysis::memory::{MemRef, Unit as MemUnit};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::IdMap;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, FloatPredicate, IntPredicate, Opcode, Tail};
use llrm_mir::passes::Declared;
use llrm_mir::tti::{
    ArithOp, CastKind, CmpSel, CostKind, LaneOp, MemoryOp, Predicate, Tti, Ty, VectorInstrContext, VectorTy,
};
use llrm_mir::types::{FloatKind, Type, TypeId};
use std::collections::HashMap;

use crate::slpgather::{self, constant_of};

/// `MaxTreeRecursionDepth`.
const MAX_DEPTH: usize = 12;
/// `-slp-threshold`.
const COST_THRESHOLD: i64 = 0;
/// `-slp-phi-vectorization-budget`.
const PHI_BUDGET: usize = 1024;

/// `getShallowScore`'s scores (`LookAheadHeuristics`, l.1081).
const SCORE_CONSECUTIVE_LOADS: i32 = 40;
const SCORE_CONSTANTS: i32 = 15;
const SCORE_SAME_CONSTANTS: i32 = 17;
const SCORE_SAME_OPCODE: i32 = 20;
const SCORE_SPLAT: i32 = 10;

/// What a bundle is.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Store,
    Load,
    Binary(BinaryOp),
    FMulAdd,
    Cast(CastOp, TypeId),
    ICmp(IntPredicate),
    FCmp(FloatPredicate),
    Select,
    /// The phis of one block, an operand column for each incoming block.
    Phi,
    FNeg,
    /// `extractelement` of every lane of one vector, in order: the vector
    /// itself is the bundle.
    Extract(Operand),
    /// A vector built from its scalars.
    Gather,
}

/// One bundle: a scalar in each lane.
struct Entry {
    kind: Kind,
    /// The lane values (a store's is its stored value; `insts` has the store).
    lanes: Vec<Operand>,
    /// The instruction of each lane, where the bundle is vectorized.
    insts: Vec<InstId>,
    /// The entry of each operand, in the order the lanes are read.
    operands: Vec<usize>,
    /// The type of one lane (a compare's is `i1`).
    lane_ty: TypeId,
    /// A phi bundle's incoming blocks, one for each operand column.
    incoming: Vec<BlockId>,
    /// `ReuseShuffleIndices`: `lanes` are the distinct scalars and the entry's
    /// vector reads lane `reuse[i]` of them at `i`. Empty: none repeats.
    reuse: Vec<i32>,
}

impl Entry {
    /// `getVectorFactor`: the lanes the entry's value has, repeats counted.
    fn width(&self) -> usize {
        if self.reuse.is_empty() { self.lanes.len() } else { self.reuse.len() }
    }
}

/// A tree that is worth making, ready for [`emit`].
pub(crate) struct Tree {
    entries: Vec<Entry>,
    block: BlockId,
    /// The calls of the tree's blocks that touch memory; they keep their order.
    impure: std::collections::HashSet<InstId>,
}

struct Builder<'a, 'u> {
    tti: &'a dyn Tti,
    unit: &'a MemUnit<'u>,
    function: &'a Function,
    block: BlockId,
    entries: Vec<Entry>,
    /// The entry that makes each scalar instruction.
    made: IdMap<InstId, usize>,
    /// Where each instruction of the block stands.
    position: IdMap<InstId, usize>,
    failed: bool,
}

pub(crate) fn lane_ty(
    context: &Context,
    ty: TypeId,
) -> Option<Ty> {
    match context.types.get(ty) {
        Type::Int(bits @ (1 | 8 | 16 | 32 | 64)) => Some(Ty::Int(*bits)),
        Type::Float(FloatKind::Float) => Some(Ty::Float),
        Type::Float(FloatKind::Double) => Some(Ty::Double),
        _ => None,
    }
}

fn arith_of(op: BinaryOp) -> ArithOp {
    match op {
        BinaryOp::Add => ArithOp::Add,
        BinaryOp::Sub => ArithOp::Sub,
        BinaryOp::Mul => ArithOp::Mul,
        BinaryOp::UDiv => ArithOp::UDiv,
        BinaryOp::SDiv => ArithOp::SDiv,
        BinaryOp::URem => ArithOp::URem,
        BinaryOp::SRem => ArithOp::SRem,
        BinaryOp::Shl => ArithOp::Shl,
        BinaryOp::LShr => ArithOp::LShr,
        BinaryOp::AShr => ArithOp::AShr,
        BinaryOp::And => ArithOp::And,
        BinaryOp::Or => ArithOp::Or,
        BinaryOp::Xor => ArithOp::Xor,
        BinaryOp::FAdd => ArithOp::FAdd,
        BinaryOp::FSub => ArithOp::FSub,
        BinaryOp::FMul => ArithOp::FMul,
        BinaryOp::FDiv => ArithOp::FDiv,
        BinaryOp::FRem => ArithOp::FRem,
    }
}

fn cast_kind_of(op: CastOp) -> Option<CastKind> {
    Some(match op {
        CastOp::Trunc => CastKind::Trunc,
        CastOp::ZExt => CastKind::ZExt,
        CastOp::SExt => CastKind::SExt,
        CastOp::FPTrunc => CastKind::FPTrunc,
        CastOp::FPExt => CastKind::FPExt,
        CastOp::FPToUI => CastKind::FPToUI,
        CastOp::FPToSI => CastKind::FPToSI,
        CastOp::UIToFP => CastKind::UIToFP,
        CastOp::SIToFP => CastKind::SIToFP,
        _ => return None,
    })
}

/// `Instruction::isCommutative`.
fn commutative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor | BinaryOp::FAdd | BinaryOp::FMul
    )
}

impl Builder<'_, '_> {
    fn context(&self) -> &Context {
        self.unit.context
    }

    fn inst_of(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        let Operand::Value(id) = operand else { return None };
        match self.function.value(id).def {
            ValueDef::Instruction(inst) if self.function.parent(inst).is_some() => Some(inst),
            _ => None,
        }
    }

    fn intrinsic(
        &self,
        inst: InstId,
    ) -> Option<Intrinsic> {
        let callee = llrm_mir::memory::callee(self.context(), self.function, inst)?;
        self.unit.globals.get(callee.0 as usize)?.intrinsic()
    }

    fn gather(
        &mut self,
        lanes: Vec<Operand>,
        lane_ty: TypeId,
    ) -> usize {
        // `tryToFindDuplicates` for a gather: the distinct lanes and a shuffle
        // that repeats them, where that costs less than inserting every lane.
        let (unique, reuse) = slpgather::unique_lanes(&lanes);
        let (lanes, reuse) = match self.pack_repeats(&lanes, &unique, &reuse, lane_ty, true) {
            Some(true) => (unique, reuse),
            _ => (lanes, Vec::new()),
        };
        self.entries.push(Entry { kind: Kind::Gather, lanes, insts: Vec::new(), operands: Vec::new(), lane_ty, reuse, incoming: Vec::new() });
        self.entries.len() - 1
    }

    /// `EstimatePackPlusShuffleVsInserts` of the bundle `lanes`, which repeats
    /// a lane: whether to pack it. `None`: not priced.
    fn pack_repeats(
        &self,
        lanes: &[Operand],
        unique: &[Operand],
        reuse: &[i32],
        lane_ty: TypeId,
        gather_only: bool,
    ) -> Option<bool> {
        if reuse.is_empty() || !unique.len().is_power_of_two() || unique.len() < 2 {
            return Some(false);
        }
        let lane = lane_ty_of(self.context(), lane_ty)?;
        let instructions: Vec<InstId> = unique.iter().filter_map(|lane| self.inst_of(*lane)).collect();
        let same = instructions.len() == unique.len()
            && instructions.iter().all(|inst| {
                let (one, first) = (self.function.instruction(*inst), self.function.instruction(instructions[0]));
                std::mem::discriminant(&one.opcode) == std::mem::discriminant(&first.opcode) && one.ty == first.ty
            });
        let first = same.then(|| self.function.instruction(instructions[0]));
        let state = first.map(|first| matches!(first.opcode, Opcode::Load { .. }));
        // `doesNotNeedToSchedule`: every lane is used outside its block or has
        // no instruction operand.
        let home = same.then(|| self.function.parent(instructions[0])).flatten();
        let outside = |inst: InstId| {
            self.function
                .instruction(inst)
                .result
                .is_some_and(
                    |result| self.function.users(result).iter().all(|used| self.function.parent(used.user) != home),
                )
        };
        let scalar_operands = |inst: InstId| {
            self.function.instruction(inst).operands.iter().all(|operand| self.inst_of(*operand).is_none())
        };
        let require_scheduling = same
            && !(instructions.iter().all(|inst| outside(*inst))
                || instructions.iter().all(|inst| scalar_operands(*inst)));
        let arity = first.map_or(0, |first| match first.opcode {
            Opcode::Binary(_) | Opcode::ICmp(_) | Opcode::FCmp(_) => 2,
            Opcode::Select => 3,
            Opcode::Cast(_) | Opcode::Load { .. } | Opcode::FNeg => 1,
            Opcode::Call(_) => 3,
            _ => 0,
        });
        // `HasExtraReshuffle`: the columns, past the first, whose operands
        // repeat; the callee of a call is no constant (`isConstant` leaves out
        // globals), so its column always does.
        let columns_with_repeats = (0..arity)
            .filter(|column| {
                let mut seen: Vec<Operand> = Vec::new();
                lanes
                    .iter()
                    .filter_map(|lane| self.inst_of(*lane))
                    .any(
                        |inst| {
                            let operand = self.function.instruction(inst).operands[*column];
                            let repeats = !constant_of(operand) && seen.contains(&operand);
                            seen.push(operand);
                            repeats
                        },
                    )
            })
            .count()
            + usize::from(first.is_some_and(|first| matches!(first.opcode, Opcode::Call(_))));
        let half_unique_ok = first.is_some_and(|first| matches!(first.opcode, Opcode::Cast(_)));
        let root_operand = !gather_only && self.entries.len() == 1;
        let packing = slpgather::Packing {
            lanes,
            unique,
            reuse,
            state,
            gather_only,
            root_operand,
            require_scheduling,
            columns_with_repeats,
            lane,
            half_unique_ok,
        };
        let (pack, keep_originals) = slpgather::estimate_pack(self.tti, &packing)?;
        Some(pack && !keep_originals)
    }

    /// `getShallowScore` of `a` in the lane before `b`'s.
    fn shallow_score(
        &self,
        a: Operand,
        b: Operand,
    ) -> i32 {
        if a == b {
            return if constant_of(a) { SCORE_SAME_CONSTANTS } else { SCORE_SPLAT };
        }
        if constant_of(a) && constant_of(b) {
            return SCORE_CONSTANTS;
        }
        let (Some(first), Some(second)) = (self.inst_of(a), self.inst_of(b)) else { return 0 };
        let (one, two) = (self.function.instruction(first), self.function.instruction(second));
        if let (Opcode::Load { .. }, Opcode::Load { .. }) = (&one.opcode, &two.opcode) {
            let size = self.context().types.get(one.ty);
            let bytes = match size {
                Type::Int(bits) => i64::from(*bits / 8),
                Type::Float(FloatKind::Float) => 4,
                Type::Float(FloatKind::Double) => 8,
                _ => return 0,
            };
            let (x, y) = (
                MemRef::at(self.unit, one.operands[0], bytes as u32),
                MemRef::at(self.unit, two.operands[0], bytes as u32),
            );
            let same_object = x.root.is_some() && x.root == y.root && x.base == y.base && x.scale == y.scale;
            return if same_object && y.disp == x.disp + bytes { SCORE_CONSECUTIVE_LOADS } else { 0 };
        }
        if std::mem::discriminant(&one.opcode) == std::mem::discriminant(&two.opcode) && one.ty == two.ty {
            return SCORE_SAME_OPCODE;
        }
        0
    }

    /// The operand columns of a bundle, the commutative pair put in the order
    /// that matches lane 0 best (`VLOperands::reorder`, one pass).
    fn columns(
        &self,
        insts: &[InstId],
        arity: usize,
        swappable: bool,
        skip: usize,
    ) -> Vec<Vec<Operand>> {
        let mut columns: Vec<Vec<Operand>> = vec![Vec::new(); arity];
        for (lane, inst) in insts.iter().enumerate() {
            let operands = &self.function.instruction(*inst).operands;
            let mut taken: Vec<Operand> = operands[..arity].to_vec();
            if swappable && lane > 0 {
                let before = (columns[skip][lane - 1], columns[skip + 1][lane - 1]);
                let keep = self.shallow_score(before.0, taken[skip]) + self.shallow_score(before.1, taken[skip + 1]);
                let swap = self.shallow_score(before.0, taken[skip + 1]) + self.shallow_score(before.1, taken[skip]);
                if swap > keep {
                    taken.swap(skip, skip + 1);
                }
            }
            for (at, operand) in taken.into_iter().enumerate() {
                columns[at].push(operand);
            }
        }
        columns
    }

    /// `buildTreeRec`.
    fn build(
        &mut self,
        lanes: Vec<Operand>,
        depth: usize,
    ) -> usize {
        let lane_ty = self.function.operand_type(self.context(), lanes[0]).expect("a typed lane");
        let first_lane = lanes[0];
        // Constants, and one value in every lane, are gathers (a constant
        // vector costs nothing, `getGatherCost`).
        if depth >= MAX_DEPTH
            || lanes.iter().any(|lane| !matches!(lane, Operand::Value(_) | Operand::Constant(_)))
            || lanes.iter().all(|lane| *lane == first_lane)
            || lanes.iter().all(|lane| constant_of(*lane))
        {
            return self.gather(lanes, lane_ty);
        }
        // The same bundle as one made: its vector is used again.
        if let Some(inst) = self.inst_of(first_lane)
            && let Some(&entry) = self.made.get(&inst)
            && self.entries[entry].lanes == lanes
        {
            return entry;
        }
        // A lane that repeats is made once and read through a shuffle.
        let (unique, reuse) = slpgather::unique_lanes(&lanes);
        let mut insts = Vec::with_capacity(unique.len());
        for lane in &unique {
            match self.inst_of(*lane) {
                Some(inst) if !self.made.contains_key(&inst) => insts.push(inst),
                _ => return self.gather(lanes, lane_ty),
            }
        }
        // A bundle stands in one block; the tree may span several.
        let home = self.function.parent(insts[0]);
        if insts.iter().any(|inst| self.function.parent(*inst) != home) {
            return self.gather(lanes, lane_ty);
        }
        let first = self.function.instruction(insts[0]).clone();
        let same = |one: &llrm_mir::module::Instruction, two: &llrm_mir::module::Instruction| {
            one.ty == two.ty && one.opcode == two.opcode
        };
        if lane_ty_of(self.context(), lane_ty).is_none() {
            return self.gather(lanes, lane_ty);
        }
        if first.opcode == Opcode::Phi {
            // `getSameOpcode`: a bundle of a phi and anything else is a gather.
            if insts.iter().any(|inst| self.function.instruction(*inst).opcode != Opcode::Phi) {
                return self.gather(lanes, lane_ty);
            }
            return self.build_phis(lanes, insts, lane_ty, depth);
        }
        let (kind, arity, swappable, skip) = match &first.opcode {
            Opcode::Load { volatile: false, .. } => (Kind::Load, 0, false, 0),
            Opcode::Binary(op) => (Kind::Binary(*op), 2, commutative(*op), 0),
            Opcode::Cast(op) => {
                let Some(source) = self.function.operand_type(self.context(), first.operands[0]) else {
                    return self.gather(lanes, lane_ty);
                };
                if cast_kind_of(*op).is_none() {
                    return self.gather(lanes, lane_ty);
                }
                (Kind::Cast(*op, source), 1, false, 0)
            }
            Opcode::ICmp(predicate) => (Kind::ICmp(*predicate), 2, false, 0),
            Opcode::FCmp(predicate) => (Kind::FCmp(*predicate), 2, false, 0),
            Opcode::Select => (Kind::Select, 3, false, 0),
            Opcode::FNeg => (Kind::FNeg, 1, false, 0),
            Opcode::ExtractElement => match self.extracted_vector(&insts, &reuse) {
                Some(source) => (Kind::Extract(source), 0, false, 0),
                None => return self.gather(lanes, lane_ty),
            },
            Opcode::Call(_) if self.intrinsic(insts[0]) == Some(Intrinsic::FMulAdd) => (Kind::FMulAdd, 3, true, 0),
            _ => return self.gather(lanes, lane_ty),
        };
        for inst in &insts[1..] {
            let other = self.function.instruction(*inst);
            let alike = match (&first.opcode, &other.opcode) {
                (Opcode::Load { volatile: false, .. }, Opcode::Load { volatile: false, .. }) => other.ty == first.ty,
                (Opcode::Cast(_), Opcode::Cast(_)) => {
                    same(&first, other)
                        && self.function.operand_type(self.context(), other.operands[0])
                            == self.function.operand_type(self.context(), first.operands[0])
                }
                (Opcode::Call(_), Opcode::Call(_)) => {
                    other.ty == first.ty && self.intrinsic(*inst) == Some(Intrinsic::FMulAdd)
                }
                _ => same(&first, other),
            };
            if !alike {
                return self.gather(lanes, lane_ty);
            }
        }
        // Compares: the operands of the bundle are one type.
        if matches!(kind, Kind::ICmp(_) | Kind::FCmp(_)) {
            let operand_ty = self.function.operand_type(self.context(), first.operands[0]);
            if insts.iter().any(|inst| {
                self.function.operand_type(self.context(), self.function.instruction(*inst).operands[0]) != operand_ty
            }) || operand_ty.and_then(|ty| lane_ty_of(self.context(), ty)).is_none()
            {
                return self.gather(lanes, lane_ty);
            }
        }
        if kind == Kind::Load && !self.consecutive(&insts) {
            return self.gather(lanes, lane_ty);
        }
        if !reuse.is_empty() && self.pack_repeats(&lanes, &unique, &reuse, lane_ty, false) != Some(true) {
            return self.gather(lanes, lane_ty);
        }
        let entry = self.entries.len();
        self.entries.push(Entry { kind, lanes: unique, insts: insts.clone(), operands: Vec::new(), lane_ty, reuse, incoming: Vec::new() });
        for inst in &insts {
            self.made.insert(*inst, entry);
        }
        if arity > 0 {
            let columns = self.columns(&insts, arity, swappable, skip);
            let children: Vec<usize> = columns.into_iter().map(|column| self.build(column, depth + 1)).collect();
            self.entries[entry].operands = children;
        }
        entry
    }

    /// The phis of a bundle read for each incoming block of the first
    /// (`PHIHandler::buildOperands`); `None` when the phis do not share one
    /// set of distinct incoming blocks.
    fn phi_columns(
        &self,
        insts: &[InstId],
    ) -> Option<(Vec<BlockId>, Vec<Vec<Operand>>)> {
        let arms = |inst: InstId| -> Vec<(Operand, BlockId)> {
            self.function
                .instruction(inst)
                .operands
                .chunks(2)
                .filter_map(
                    |pair| match pair[1] {
                        Operand::Block(block) => Some((pair[0], block)),
                        _ => None,
                    },
                )
                .collect()
        };
        let blocks: Vec<BlockId> = arms(insts[0]).iter().map(|arm| arm.1).collect();
        if blocks.iter().enumerate().any(|(at, block)| blocks[..at].contains(block)) {
            return None;
        }
        let mut columns: Vec<Vec<Operand>> = vec![Vec::with_capacity(insts.len()); blocks.len()];
        for inst in insts {
            let theirs = arms(*inst);
            if theirs.len() != blocks.len() {
                return None;
            }
            for (column, block) in columns.iter_mut().zip(&blocks) {
                column.push(theirs.iter().find(|arm| arm.1 == *block)?.0);
            }
        }
        Some((blocks, columns))
    }

    /// `buildTreeRec`'s `Instruction::PHI`: the entry is made before its
    /// operands, so a latch value that reads the phis again finds it.
    fn build_phis(
        &mut self,
        lanes: Vec<Operand>,
        insts: Vec<InstId>,
        lane_ty: TypeId,
        depth: usize,
    ) -> usize {
        let ty = self.function.instruction(insts[0]).ty;
        let columns = if insts.iter().all(|inst| self.function.instruction(*inst).ty == ty) {
            self.phi_columns(&insts)
        } else {
            None
        };
        // `-slp-phi-vectorization-budget`.
        let Some((incoming, columns)) = columns.filter(|(blocks, _)| insts.len() * blocks.len() <= PHI_BUDGET) else {
            return self.gather(lanes, lane_ty);
        };
        let entry = self.entries.len();
        self.entries.push(Entry {
            kind: Kind::Phi,
            lanes,
            insts: insts.clone(),
            operands: Vec::new(),
            lane_ty,
            incoming,
            reuse: Vec::new(),
        });
        for inst in &insts {
            self.made.insert(*inst, entry);
        }
        let children: Vec<usize> = columns.into_iter().map(|column| self.build(column, depth + 1)).collect();
        self.entries[entry].operands = children;
        entry
    }

    /// `canReuseExtract`: the `extractelement`s read lane `i` of one vector of
    /// their own count at `i`, in order (DEVIATE SLP5: no other order, no
    /// resize, no vector the tree makes); the vector.
    fn extracted_vector(
        &self,
        insts: &[InstId],
        reuse: &[i32],
    ) -> Option<Operand> {
        if !reuse.is_empty() {
            return None;
        }
        let source = self.function.instruction(insts[0]).operands[0];
        let Operand::Value(_) = source else { return None };
        let in_tree = self.inst_of(source).is_some_and(|inst| self.made.contains_key(&inst));
        let count = match self.context().types.get(self.function.operand_type(self.context(), source)?) {
            Type::Vector { count, .. } => *count as usize,
            _ => return None,
        };
        let in_order = insts
            .iter()
            .enumerate()
            .all(
                |(at, inst)| {
                    let operands = &self.function.instruction(*inst).operands;
                    operands[0] == source
                        && matches!(
                            operands[1],
                            Operand::Constant(index) if matches!(self.context().get(index).kind, ConstantKind::Int(lane) if lane == at as u128)
                        )
                },
            );
        (!in_tree && count == insts.len() && in_order).then_some(source)
    }

    /// The loads read ascending, adjacent cells of one object.
    fn consecutive(
        &self,
        insts: &[InstId],
    ) -> bool {
        let first = self.function.instruction(insts[0]);
        let bytes = match self.context().types.get(first.ty) {
            Type::Int(bits) => u32::from(*bits / 8),
            Type::Float(FloatKind::Float) => 4,
            Type::Float(FloatKind::Double) => 8,
            _ => return false,
        };
        let refs: Vec<MemRef> = insts
            .iter()
            .map(|inst| MemRef::at(self.unit, self.function.instruction(*inst).operands[0], bytes))
            .collect();
        let Some(root) = refs[0].root else { return false };
        refs.iter().all(|one| one.root == Some(root) && one.segment.is_none() && one.base == refs[0].base)
            && refs.windows(2).all(|pair| pair[1].disp == pair[0].disp + i64::from(bytes))
    }
}

fn lane_ty_of(
    context: &Context,
    ty: TypeId,
) -> Option<Ty> {
    lane_ty(context, ty)
}

/// Whether `lanes`' instructions (stores, in a window) read as one vector
/// store of a tree that pays: the tree, if it does.
#[allow(clippy::too_many_arguments)]
pub(crate) fn try_window(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    stores: &[InstId],
    store_ty: TypeId,
    align: u64,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Tree> {
    let position = positions(function);
    let values: Vec<Operand> = stores.iter().map(|store| function.instruction(*store).operands[0]).collect();
    let mut builder =
        Builder { tti, unit, function, block, entries: Vec::new(), made: IdMap::new(), position, failed: false };
    // The root: the stores.
    builder
        .entries
        .push(
            Entry {
                kind: Kind::Store,
                lanes: values.clone(),
                insts: stores.to_vec(),
                operands: Vec::new(),
                lane_ty: store_ty,
                incoming: Vec::new(),
                reuse: Vec::new(),
            },
        );
    for store in stores {
        builder.made.insert(*store, 0);
    }
    let child = builder.build(values, 1);
    builder.entries[0].operands = vec![child];
    if builder.failed || builder.entries.len() < 2 || builder.entries[child].kind == Kind::Gather {
        llrm_support::debug!(
            "slp",
            "tree x{}: {} entries, root {:?}{}",
            stores.len(),
            builder.entries.len(),
            builder.entries[child].kind,
            if builder.failed { ", a lane used as a gather" } else { "" }
        );
        return None;
    }
    let mut tree = Tree { entries: builder.entries, block, impure: Default::default() };
    let made = builder.made;
    let position = builder.position;
    let Some(impure) = schedulable(unit, function, &tree, &made, &position) else {
        llrm_support::debug!("slp", "tree: not schedulable");
        return None;
    };
    tree.impure = impure;
    let cost = cost_of(unit, function, &tree, &made, align, tti, scaler)?;
    llrm_support::debug!("slp", "tree of {} entries x{}: cost {cost}", tree.entries.len(), stores.len());
    (cost < -COST_THRESHOLD).then_some(tree)
}

/// `tryToVectorizeList` on the instructions `roots` of one block (phis, or the
/// operand pair of a binary operation): their tree, if it pays. The root is
/// their bundle; nothing is stored.
pub(crate) fn try_bundle(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    phis: &[InstId],
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Tree> {
    let position = positions(function);
    let lanes: Vec<Operand> = phis.iter().map(|phi| result_of(function, *phi)).collect();
    let mut builder =
        Builder { tti, unit, function, block, entries: Vec::new(), made: IdMap::new(), position, failed: false };
    builder.build(lanes, 0);
    if builder.failed || builder.entries.first().is_none_or(|root| root.kind == Kind::Gather) {
        llrm_support::debug!("slp", "bundle tree x{}: no bundle{}", phis.len(), if builder.failed { ", a lane used as a gather" } else { "" });
        return None;
    }
    let mut tree = Tree { entries: builder.entries, block, impure: Default::default() };
    let made = builder.made;
    let position = builder.position;
    if tiny_without_stores(&tree) {
        llrm_support::debug!("slp", "bundle tree x{}: tiny", phis.len());
        return None;
    }
    let Some(impure) = schedulable(unit, function, &tree, &made, &position) else {
        llrm_support::debug!("slp", "bundle tree: not schedulable");
        return None;
    };
    tree.impure = impure;
    let cost = cost_of(unit, function, &tree, &made, 1, tti, scaler)?;
    llrm_support::debug!("slp", "bundle tree of {} entries x{}: cost {cost}", tree.entries.len(), phis.len());
    (cost < -COST_THRESHOLD).then_some(tree)
}

/// `isTreeTinyAndNotFullyVectorizable`, the parts a tree without a store root
/// meets: three entries whose two others are small gathers, one vector entry
/// among gathers that only repack, phis and gathers alone.
fn tiny_without_stores(tree: &Tree) -> bool {
    let gather = |entry: &Entry| entry.kind == Kind::Gather;
    let constant_lanes = |entry: &Entry| entry.lanes.iter().all(|lane| constant_of(*lane));
    let splat = |entry: &Entry| entry.lanes.iter().all(|lane| *lane == entry.lanes[0]);
    let rest = &tree.entries[1..];
    // `-slp-min-tree-size` 3: the tree of a root and two gathers.
    if tree.entries.len() == 3
        && rest.iter().all(|entry| gather(entry) && entry.lanes.len() <= 4 && !constant_lanes(entry))
    {
        return true;
    }
    let vector = tree.entries.iter().filter(|entry| !gather(entry)).count();
    if tree.entries.len() > 1
        && vector == 1
        && rest.iter().all(|entry| gather(entry) && !splat(entry) && !constant_lanes(entry))
    {
        return true;
    }
    // "Only PHI nodes and gathers": the phis cost nothing.
    tree.entries.iter().all(|entry| gather(entry) || entry.kind == Kind::Phi)
}

/// The memory dependence `BlockScheduling` finds between a window's stores and
/// an instruction standing among them (`calculateDependencies`, `BatchAA`):
/// the vector store stands at the last of the stores, so a load or store that
/// cannot alias any of them may stay where it is.
pub(crate) struct StoreSeparation<'a> {
    unit: &'a MemUnit<'a>,
    stores: Vec<llrm_analysis::aa::MemoryLocation>,
    aa: llrm_analysis::aggregate::AggregateAA<'a>,
}

impl<'a> StoreSeparation<'a> {
    pub(crate) fn new(
        unit: &'a MemUnit<'a>,
        stores: impl Iterator<Item = InstId>,
    ) -> Self {
        let mut aa = llrm_analysis::aggregate::AggregateAA::new();
        aa.add_aa_result(llrm_analysis::basicaa::BasicAA::new(unit));
        aa.add_aa_result(llrm_analysis::scopednoaliasaa::ScopedNoAliasAA::new(unit));
        aa.add_aa_result(llrm_analysis::typebasedaa::TypeBasedAA::new(unit));
        let stores = stores.filter_map(|store| llrm_analysis::aa::MemoryLocation::get(unit, store)).collect();
        Self { unit, stores, aa }
    }

    /// Whether `inst` is a plain load or store that touches none of the stores'
    /// bytes. A call, a volatile or atomic access, and a store whose place is
    /// unknown are dependences.
    pub(crate) fn clear(
        &mut self,
        function: &Function,
        inst: InstId,
    ) -> bool {
        let plain = match function.instruction(inst).opcode {
            Opcode::Load { volatile, .. } | Opcode::Store { volatile, .. } => !volatile,
            _ => false,
        };
        let Some(here) = llrm_analysis::aa::MemoryLocation::get(self.unit, inst).filter(|_| plain) else {
            return false;
        };
        let mut batch = llrm_analysis::aa::BatchAAResults::new(&mut self.aa);
        self.stores.iter().all(|store| batch.is_no_alias(&here, store))
    }
}

/// The scalars the tree makes as a vector that a gather of the tree also
/// reads: each is extracted for it.
fn gathered_scalars(
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
) -> std::collections::HashSet<InstId> {
    let mut found = std::collections::HashSet::new();
    for entry in tree.entries.iter().filter(|entry| entry.kind == Kind::Gather) {
        for lane in &entry.lanes {
            let Operand::Value(id) = lane else { continue };
            if let ValueDef::Instruction(def) = function.value(*id).def
                && made.contains_key(&def)
            {
                found.insert(def);
            }
        }
    }
    found
}

/// The blocks that hold the lanes of the tree's bundles (a phi's are the
/// block's top).
fn tree_blocks(
    function: &Function,
    tree: &Tree,
) -> Vec<BlockId> {
    let mut blocks: Vec<BlockId> = Vec::new();
    for entry in tree.entries.iter().filter(|entry| entry.kind != Kind::Gather) {
        for block in entry.insts.iter().filter_map(|inst| function.parent(*inst)).chain(entry.incoming.iter().copied()) {
            if !blocks.contains(&block) {
                blocks.push(block);
            }
        }
    }
    blocks
}

/// Whether the instruction keeps its place among those that touch memory.
fn orders_memory(
    function: &Function,
    inst: InstId,
    impure: &std::collections::HashSet<InstId>,
) -> bool {
    match function.instruction(inst).opcode {
        Opcode::Call(_) => impure.contains(&inst),
        Opcode::Store { .. } | Opcode::Invoke(_) | Opcode::Atomic(_) | Opcode::VaArg | Opcode::Load { .. } => true,
        _ => false,
    }
}

/// `BlockScheduling`'s question for a block: the instructions of `list`, with
/// `node` naming the unit each stands in, can be put in an order where every
/// unit stands after what it reads and the ones that touch memory keep theirs
/// (`key` places a unit among them). The order, least `key` first.
fn schedule_units(
    function: &Function,
    list: &[InstId],
    nodes: usize,
    node: &dyn Fn(usize) -> usize,
    key: &dyn Fn(usize) -> usize,
    touches: &dyn Fn(InstId) -> bool,
) -> Option<Vec<usize>> {
    let index: HashMap<InstId, usize> = list.iter().enumerate().map(|(at, inst)| (*inst, at)).collect();
    let mut after: Vec<Vec<usize>> = vec![Vec::new(); nodes];
    let mut waits = vec![0usize; nodes];
    let mut link = |from: usize, to: usize, after: &mut Vec<Vec<usize>>| {
        if !after[from].contains(&to) {
            after[from].push(to);
            waits[to] += 1;
        }
    };
    for (at, inst) in list.iter().enumerate() {
        for operand in &function.instruction(*inst).operands {
            let Operand::Value(id) = operand else { continue };
            let llrm_mir::module::ValueDef::Instruction(def) = function.value(*id).def else { continue };
            let Some(from) = index.get(&def).map(|from| node(*from)) else { continue };
            let to = node(at);
            if from == to {
                // A lane that reads another lane of its own bundle.
                return None;
            }
            link(from, to, &mut after);
        }
    }
    let mut memory: Vec<(usize, usize)> =
        list.iter().enumerate().filter(|(_, inst)| touches(**inst)).map(|(at, _)| (key(at), node(at))).collect();
    memory.sort();
    memory.dedup();
    for pair in memory.windows(2) {
        if pair[0].1 != pair[1].1 {
            link(pair[0].1, pair[1].1, &mut after);
        }
    }
    let mut used = vec![false; nodes];
    let mut keys = vec![usize::MAX; nodes];
    for at in 0..list.len() {
        let one = node(at);
        used[one] = true;
        keys[one] = keys[one].min(at);
    }
    let mut ready: std::collections::BinaryHeap<std::cmp::Reverse<(usize, usize)>> = (0..nodes)
        .filter(|one| used[*one] && waits[*one] == 0)
        .map(|one| std::cmp::Reverse((keys[one], one)))
        .collect();
    let mut order = Vec::new();
    while let Some(std::cmp::Reverse((_, one))) = ready.pop() {
        order.push(one);
        for next in std::mem::take(&mut after[one]) {
            waits[next] -= 1;
            if waits[next] == 0 {
                ready.push(std::cmp::Reverse((keys[next], next)));
            }
        }
    }
    (order.len() == used.iter().filter(|one| **one).count()).then_some(order)
}

/// Whether the scalars of the tree can stand as its vectors: in each block, with
/// the lanes of a bundle one unit, no unit waits on itself.
fn contracted_order_exists(
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
    position: &IdMap<InstId, usize>,
    touches: &dyn Fn(InstId) -> bool,
) -> bool {
    let _ = position;
    for block in tree_blocks(function, tree) {
        let terminator = function.terminator(block);
        let list: Vec<InstId> = function
            .iter_block(block)
            .filter(|inst| function.instruction(*inst).opcode != Opcode::Phi && Some(*inst) != terminator)
            .collect();
        let unit_of = |at: usize| match made.get(&list[at]) {
            Some(entry) if tree.entries[*entry].kind != Kind::Phi => list.len() + *entry,
            _ => at,
        };
        // A bundle stands where its last lane does.
        let mut last: HashMap<usize, usize> = HashMap::new();
        for at in 0..list.len() {
            last.insert(unit_of(at), at);
        }
        let key = |at: usize| last[&unit_of(at)];
        if schedule_units(function, &list, list.len() + tree.entries.len(), &unit_of, &key, touches).is_none() {
            return false;
        }
    }
    true
}

/// After the vectors stand: the instructions of each block in an order where
/// each follows what it reads, as near the layout as that allows
/// (`BlockScheduling::scheduleBlock`).
fn reschedule(
    function: &mut Function,
    blocks: &[BlockId],
    impure: &std::collections::HashSet<InstId>,
) {
    for block in blocks {
        let Some(terminator) = function.terminator(*block) else { continue };
        let list: Vec<InstId> = function
            .iter_block(*block)
            .filter(|inst| function.instruction(*inst).opcode != Opcode::Phi && *inst != terminator)
            .collect();
        let touches = |inst: InstId| orders_memory(function, inst, impure);
        let Some(order) = schedule_units(function, &list, list.len(), &|at| at, &|at| at, &touches) else {
            llrm_support::debug!("slp", "reschedule: the block has no order");
            continue;
        };
        if order.iter().enumerate().all(|(at, one)| at == *one) {
            continue;
        }
        for one in order {
            function.move_to(list[one], Position::Before(terminator)).expect("a placed instruction");
        }
    }
}

/// DEVIATE SLP3: the scalars of the tree can stand as vectors where the last of
/// each bundle is.
fn schedulable(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
    position: &IdMap<InstId, usize>,
) -> Option<std::collections::HashSet<InstId>> {
    let at = |inst: &InstId| position[inst];
    let root = &tree.entries[0];
    let seed = function.parent(root.insts[0]).expect("a placed root");
    // The stores of a store root: the vector store stands at the last.
    let stores = (root.kind == Kind::Store)
        .then(|| (root.insts.iter().map(at).min().expect("stores"), root.insts.iter().map(at).max().expect("stores")));
    // A call of an intrinsic that computes each lane from its arguments
    // (`isTriviallyVectorizable`) touches no memory.
    let pure_call = |inst: InstId| {
        llrm_mir::memory::callee(unit.context, function, inst)
            .and_then(|callee| unit.globals.get(callee.0 as usize)?.intrinsic())
            .is_some_and(llrm_analysis::vectorutils::is_trivially_vectorizable)
    };
    let touches = |inst: InstId| match function.instruction(inst).opcode {
        Opcode::Call(_) => !pure_call(inst),
        Opcode::Store { .. } | Opcode::Invoke(_) | Opcode::Atomic(_) | Opcode::VaArg | Opcode::Load { .. } => true,
        _ => false,
    };
    if let Some((first_store, last_store)) = stores {
        // The stores move to the last: nothing else between them touches their bytes.
        let mut separation = StoreSeparation::new(unit, root.insts.iter().copied());
        for (order, inst) in function.iter_block(seed).enumerate() {
            if order > first_store
                && order < last_store
                && !made.contains_key(&inst)
                && touches(inst)
                && !separation.clear(function, inst)
            {
                llrm_support::debug!("slp", "tree: {:?} stands between the stores", function.instruction(inst).opcode);
                return None;
            }
        }
    }
    for entry in &tree.entries {
        if matches!(entry.kind, Kind::Gather | Kind::Store) {
            continue;
        }
        let block = function.parent(entry.insts[0]).expect("a placed bundle");
        let (first, last) =
            (entry.insts.iter().map(at).min().expect("a bundle"), entry.insts.iter().map(at).max().expect("a bundle"));
        if entry.kind == Kind::Load {
            // Loads of the seed block stand before its first store; the loads
            // move down to the last, past nothing that writes.
            if let Some((first_store, _)) = stores
                && block == seed
                && last > first_store
            {
                llrm_support::debug!("slp", "tree: a load stands after the first store");
                return None;
            }
            for (order, inst) in function.iter_block(block).enumerate() {
                let writes = match function.instruction(inst).opcode {
                    Opcode::Call(_) => !pure_call(inst),
                    Opcode::Store { .. } | Opcode::Invoke(_) | Opcode::Atomic(_) | Opcode::VaArg => true,
                    _ => false,
                };
                if order > first && order < last && !made.contains_key(&inst) && writes {
                    llrm_support::debug!("slp", "tree: a write stands among the loads");
                    return None;
                }
            }
        }
    }
    if !contracted_order_exists(function, tree, made, position, &touches) {
        llrm_support::debug!("slp", "tree: the bundles wait on one another");
        return None;
    }
    let impure = tree_blocks(function, tree)
        .into_iter()
        .flat_map(|block| function.iter_block(block).collect::<Vec<_>>())
        .filter(|inst| matches!(function.instruction(*inst).opcode, Opcode::Call(_)) && !pure_call(*inst))
        .collect();
    Some(impure)
}

/// `getScaleToLoopIterations`: how often an entry's cost is paid
/// (`-slp-cost-loop-trip-count` 2). DEVIATE SLP4 (owner arm64, exit
/// 2026-11-24): `getLoopEstimatedTripCount` (profile weights) is not read; a
/// loop without a constant trip count counts 2.
pub(crate) struct Scaler<'a> {
    se: std::cell::RefCell<&'a mut ScalarEvolution>,
    cx: &'a Cx<'a>,
}

const LOOP_TRIP_COUNT: u64 = 2;

impl<'a> Scaler<'a> {
    pub(crate) fn new(
        se: &'a mut ScalarEvolution,
        cx: &'a Cx<'a>,
    ) -> Self {
        Self { se: std::cell::RefCell::new(se), cx }
    }

    /// `getLoopTripCount`.
    fn trip_count(
        &self,
        function: &Function,
        one: &Loop<'_>,
    ) -> u64 {
        let counted = u64::from(self.se.borrow_mut().get_small_constant_trip_count(self.cx, one.get_header()));
        match counted {
            0 => LOOP_TRIP_COUNT,
            count if one.get_exiting_block(function).is_none() => count.min(LOOP_TRIP_COUNT),
            count => count,
        }
    }

    /// `getLoopNestScale`: the trip counts of the loop and those around it.
    fn nest_scale(
        &self,
        function: &Function,
        innermost: Option<Loop<'_>>,
    ) -> u64 {
        let mut scale = 1u64;
        let mut at = innermost;
        while let Some(one) = at {
            scale = scale.saturating_mul(self.trip_count(function, &one).max(1));
            at = one.get_parent_loop();
        }
        scale
    }

    /// The scale of what `scalars` make at `block`: the innermost loop around
    /// it in which one of them varies (`findInnermostNonInvariantLoop`).
    fn scale_at(
        &self,
        function: &Function,
        block: BlockId,
        scalars: &[Operand],
    ) -> u64 {
        let mut at = self.cx.loops.get_loop_for(block);
        while let Some(one) = at {
            if scalars.iter().any(|scalar| !one.is_loop_invariant(function, *scalar)) {
                break;
            }
            at = one.get_parent_loop();
        }
        self.nest_scale(function, at)
    }
}

/// `getScaleToLoopIterations(TE)`: a bundle's cost by the block it stands in; a
/// gather by the block of the bundle that reads it.
fn entry_scale(
    function: &Function,
    tree: &Tree,
    at: usize,
    scaler: &Scaler,
) -> u64 {
    let entry = &tree.entries[at];
    match entry.kind {
        Kind::Gather => {
            // The bundle that reads it, past gathers.
            let mut user = tree.entries.iter().position(|one| one.operands.contains(&at));
            while let Some(found) = user {
                if tree.entries[found].kind != Kind::Gather {
                    let block = function.parent(tree.entries[found].insts[0]).expect("a placed bundle");
                    return scaler.scale_at(function, block, &entry.lanes);
                }
                user = tree.entries.iter().position(|one| one.operands.contains(&found));
            }
            1
        }
        Kind::Store => {
            let block = function.parent(entry.insts[0]).expect("a placed store");
            scaler.scale_at(function, block, &[])
        }
        _ => {
            let block = function.parent(entry.insts[0]).expect("a placed bundle");
            scaler.scale_at(function, block, &entry.lanes)
        }
    }
}

/// `getTreeCost`: the entries' vector costs less their scalars', and the
/// extracts of the lanes used outside. `None`: a price the target lacks.
fn cost_of(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
    store_align: u64,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<i64> {
    let kind = CostKind::RecipThroughput;
    let context = unit.context;
    let mut total = 0i64;
    let gathered = gathered_scalars(function, tree, made);
    for (at, entry) in tree.entries.iter().enumerate() {
        let Some(priced) = entry_cost(unit, function, tree, entry, store_align, tti) else {
            llrm_support::debug!("slp", "entry {:?} x{}: not priced", entry.kind, entry.lanes.len());
            return None;
        };
        total += priced * entry_scale(function, tree, at, scaler) as i64;
        // The lanes some user outside the tree reads are extracted.
        if !matches!(entry.kind, Kind::Gather | Kind::Store) {
            let lane = lane_ty_of(context, entry.lane_ty)?;
            let vector = VectorTy { lane, count: entry.width() as u32 };
            for (index, inst) in entry.insts.iter().enumerate() {
                let Some(result) = function.instruction(*inst).result else { continue };
                // The first user outside fixes the scale
                // (`ExtractCostCalculated`); a gather that reads the scalar
                // is one.
                let outside = function.users(result).iter().find(|used| !made.contains_key(&used.user));
                let site = outside
                    .map(|used| function.parent(used.user))
                    .or_else(|| gathered.contains(inst).then(|| function.parent(*inst)));
                if let Some(site) = site {
                    // DEVIATE SLP5: the extract of an extract (the pin keeps
                    // the scalar when it costs less) is not
                    // made.
                    if matches!(entry.kind, Kind::Extract(_)) {
                        return None;
                    }
                    // `findLaneForValue`: the first place the lane is read.
                    let at = entry.reuse.iter().position(|&lane| lane == index as i32).unwrap_or(index);
                    let extract = tti.vector_instr_cost(
                        LaneOp::Extract,
                        vector,
                        Some(at as u32),
                        false,
                        VectorInstrContext::None,
                        kind,
                    )?;
                    let block = site?;
                    let scale = scaler.scale_at(function, block, &[Operand::Value(result)]);
                    total += extract.valid.then_some(extract.value)? * scale as i64;
                }
            }
        }
    }
    Some(total)
}

/// `getEntryCost`: the vector of `entry` less its scalars.
fn entry_cost(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    entry: &Entry,
    store_align: u64,
    tti: &dyn Tti,
) -> Option<i64> {
    let kind = CostKind::RecipThroughput;
    let context = unit.context;
    {
        let lane = lane_ty_of(context, entry.lane_ty)?;
        let count = entry.lanes.len() as u32;
        let vector = VectorTy { lane, count };
        let n = i64::from(count);
        let valid = |cost: llrm_mir::tti::Cost| cost.valid.then_some(cost.value);
        let entry_cost = match &entry.kind {
            Kind::Store => {
                let wide = valid(tti.vector_memory_op_cost(MemoryOp::Store, vector, store_align, kind)?)?;
                wide - n * valid(tti.memory_op_cost(MemoryOp::Store, lane, kind, false))?
            }
            Kind::Load => {
                let align = match function.instruction(entry.insts[0]).opcode {
                    Opcode::Load { align, .. } => align.unwrap_or(1),
                    _ => 1,
                };
                let wide = valid(tti.vector_memory_op_cost(MemoryOp::Load, vector, align, kind)?)?;
                wide - n * valid(tti.memory_op_cost(MemoryOp::Load, lane, kind, false))?
            }
            Kind::Binary(op) => {
                let wide = valid(tti.vector_arithmetic_cost(arith_of(*op), vector, kind)?)?;
                wide - n * valid(tti.arithmetic_cost(
                    arith_of(*op),
                    lane,
                    kind,
                    Default::default(),
                    Default::default(),
                ))?
            }
            Kind::FMulAdd => {
                let wide = valid(tti.vector_arithmetic_cost(ArithOp::FMul, vector, kind)?)?;
                wide - n * valid(tti.arithmetic_cost(
                    ArithOp::FMul,
                    lane,
                    kind,
                    Default::default(),
                    Default::default(),
                ))?
            }
            Kind::Cast(op, source) => {
                let cast = cast_kind_of(*op)?;
                let from = lane_ty_of(context, *source)?;
                let wide = valid(tti.vector_cast_cost(cast, vector, VectorTy { lane: from, count }, kind)?)?;
                wide - n * valid(tti.cast_cost(cast, lane, from, kind))?
            }
            Kind::ICmp(_) | Kind::FCmp(_) => {
                let (op, predicate) = match entry.kind {
                    Kind::ICmp(predicate) => (CmpSel::ICmp, Predicate::Int(predicate)),
                    Kind::FCmp(predicate) => (CmpSel::FCmp, Predicate::Float(predicate)),
                    _ => unreachable!(),
                };
                // A compare's lane type is its operands'.
                let operand = function.operand_type(context, function.instruction(entry.insts[0]).operands[0])?;
                let operand = lane_ty_of(context, operand)?;
                let wide =
                    valid(tti.vector_cmp_sel_cost(op, VectorTy { lane: operand, count }, Some(predicate), kind)?)?;
                wide - n * valid(tti.cmp_sel_cost_for(op, operand, kind, Some(predicate)))?
            }
            Kind::Select => {
                let predicate = predicate_of(function, tree, entry);
                let wide = valid(tti.vector_cmp_sel_cost(CmpSel::Select, vector, predicate, kind)?)?;
                wide - n * valid(tti.cmp_sel_cost(CmpSel::Select, lane, kind))?
            }
            // The phis cost nothing, scalar or vector (`getEntryCost`, l.16449).
            Kind::Phi => 0,
            Kind::FNeg => {
                let wide = valid(tti.vector_arithmetic_cost(ArithOp::FNeg, vector, kind)?)?;
                wide - n * valid(tti.arithmetic_cost(
                    ArithOp::FNeg,
                    lane,
                    kind,
                    Default::default(),
                    Default::default(),
                ))?
            }
            // The vector is the source's: what the scalar extracts cost is
            // saved.
            Kind::Extract(_) => {
                let mut extracts = 0;
                for index in 0..count {
                    extracts += valid(tti.vector_instr_cost(
                        LaneOp::Extract,
                        vector,
                        Some(index),
                        false,
                        VectorInstrContext::None,
                        kind,
                    )?)?;
                }
                -extracts
            }
            Kind::Gather => slpgather::gather_cost(tti, lane, &entry.lanes, &entry.reuse)?,
        };
        // The lanes that repeat are read through a shuffle of the bundle.
        let reuse_cost = match entry.kind {
            Kind::Gather | Kind::Store => 0,
            _ => slpgather::shuffle_cost(tti, VectorTy { lane, count: entry.width() as u32 }, &entry.reuse)?,
        };
        let entry_cost = entry_cost + reuse_cost;
        llrm_support::debug!("slp", "entry {:?} x{}: {}", entry.kind, count, entry_cost);
        Some(entry_cost)
    }
}

/// The predicate of the compare that feeds the lanes of a select.
fn predicate_of(
    function: &Function,
    tree: &Tree,
    entry: &Entry,
) -> Option<Predicate> {
    let condition = &tree.entries[entry.operands[0]];
    match condition.kind {
        Kind::ICmp(predicate) => Some(Predicate::Int(predicate)),
        Kind::FCmp(predicate) => Some(Predicate::Float(predicate)),
        _ => {
            let _ = function;
            None
        }
    }
}

/// Where a made instruction goes: right after an instruction, or right before
/// a block's terminator.
#[derive(Clone, Copy)]
enum Anchor {
    After(InstId),
    Before(InstId),
}

/// Where the next vector made for an anchor goes: after the last one made for
/// it, so a gather stands before the bundle that reads it.
#[derive(Default)]
struct Placer {
    cursor: IdMap<InstId, InstId>,
}

impl Placer {
    fn after(
        &mut self,
        function: &mut Function,
        anchor: InstId,
        made: InstId,
    ) {
        let at = self.cursor.get(&anchor).copied().unwrap_or(anchor);
        function.insert(made, Position::After(at)).expect("the anchor's block");
        self.cursor.insert(anchor, made);
    }

    fn place(
        &mut self,
        function: &mut Function,
        anchor: Anchor,
        made: InstId,
    ) {
        match anchor {
            Anchor::After(anchor) => self.after(function, anchor, made),
            Anchor::Before(terminator) => {
                function.insert(made, Position::Before(terminator)).expect("the terminator's block");
            }
        }
    }
}

/// Where each instruction stands in its block.
fn positions(function: &Function) -> IdMap<InstId, usize> {
    let mut places = IdMap::new();
    for block in function.layout().to_vec() {
        for (at, inst) in function.iter_block(block).enumerate() {
            places.insert(inst, at);
        }
    }
    places
}

fn vector_of(
    context: &mut Context,
    element: TypeId,
    count: usize,
) -> TypeId {
    context.types.intern(Type::Vector { element, count: count as u32 })
}

fn result_of(
    function: &Function,
    inst: InstId,
) -> Operand {
    Operand::Value(function.instruction(inst).result.expect("a value"))
}

/// `vectorizeTree`: the tree as vectors, its scalars gone.
pub(crate) fn emit(
    context: &mut Context,
    declared: &mut Declared,
    function: &mut Function,
    tree: &Tree,
    store_align: Option<u64>,
) {
    let position = positions(function);
    let blocks = tree_blocks(function, tree);
    let last_of = |entry: &Entry| *entry.insts.iter().max_by_key(|inst| position[*inst]).expect("a bundle");
    let mut placer = Placer::default();
    // The vector of each entry, children first; a phi's before them, since a
    // latch value reads it, and its incoming values once they exist.
    let mut vectors: Vec<Option<Operand>> = vec![None; tree.entries.len()];
    let mut order: Vec<(usize, bool)> = Vec::new();
    fn visit(
        tree: &Tree,
        at: usize,
        seen: &mut [bool],
        order: &mut Vec<(usize, bool)>,
    ) {
        if std::mem::replace(&mut seen[at], true) {
            return;
        }
        let phi = tree.entries[at].kind == Kind::Phi;
        if phi {
            order.push((at, false));
        }
        for child in &tree.entries[at].operands {
            visit(tree, *child, seen, order);
        }
        order.push((at, phi));
    }
    visit(tree, 0, &mut vec![false; tree.entries.len()], &mut order);
    for (at, fill) in order {
        let entry = &tree.entries[at];
        let count = entry.lanes.len();
        let vector_ty = vector_of(context, entry.lane_ty, count);
        if fill {
            // The phi's incoming values, now that its operands are made.
            let phi = defined_by(function, vectors[at].expect("the vector phi"));
            let mut arms = Vec::with_capacity(entry.incoming.len() * 2);
            for (block, child) in entry.incoming.iter().zip(&entry.operands) {
                arms.push(vectors[*child].expect("an operand made first"));
                arms.push(Operand::Block(*block));
            }
            function.set_operands(phi, arms);
            continue;
        }
        if entry.kind == Kind::Phi {
            // After the block's phis, as the pin's builder stands at its first
            // non-phi.
            let phi = function.create_instruction(Opcode::Phi, vector_ty, Vec::new(), Flags::default(), None);
            placer.after(function, last_phi_of(function, entry.insts[0]), phi);
            vectors[at] = Some(result_of(function, phi));
            continue;
        }
        let operands: Vec<Operand> =
            entry.operands.iter().map(|child| vectors[*child].expect("an operand made first")).collect();
        let made = match &entry.kind {
            Kind::Extract(source) => Some(reused(context, function, &mut placer, last_of(entry), entry, *source)),
            Kind::Gather => {
                // Built right before the bundle that reads it.
                let user = tree.entries.iter().position(|one| one.operands.contains(&at)).expect("a user");
                let user = &tree.entries[user];
                let anchor = if user.kind == Kind::Phi {
                    // A phi reads it at the end of the incoming block.
                    let column = user.operands.iter().position(|one| *one == at).expect("a column");
                    let block = user.incoming[column];
                    Anchor::Before(function.terminator(block).expect("a terminated block"))
                } else {
                    // A user's vector stands after its last scalar: the gather
                    // stands there too, before the user, after every lane it reads.
                    Anchor::After(last_of(user))
                };
                Some(build_vector(context, function, &mut placer, anchor, entry))
            }
            Kind::Store | Kind::Phi => None,
            kind => {
                let lead = function.instruction(entry.insts[0]).clone();
                let mut flags = lead.flags;
                for inst in &entry.insts[1..] {
                    flags = flags.intersect(function.instruction(*inst).flags);
                }
                let opcode = match kind {
                    Kind::Load => Opcode::Load {
                        align: match lead.opcode {
                            Opcode::Load { align, .. } => align,
                            _ => None,
                        },
                        volatile: false,
                    },
                    Kind::Binary(op) => Opcode::Binary(*op),
                    Kind::Cast(op, _) => Opcode::Cast(*op),
                    Kind::ICmp(predicate) => Opcode::ICmp(*predicate),
                    Kind::FCmp(predicate) => Opcode::FCmp(*predicate),
                    Kind::Select => Opcode::Select,
                    Kind::FNeg => Opcode::FNeg,
                    Kind::FMulAdd => {
                        let function_type = context
                            .types
                            .intern(
                                Type::Function {
                                    returns: vector_ty,
                                    parameters: vec![vector_ty; 3],
                                    variadic: false,
                                },
                            );
                        let name = format!("llvm.fmuladd.{}", llrm_mir::intrinsics::mangle(&context.types, vector_ty));
                        let callee = declared.declare(&name, function_type);
                        let pointer = context.types.ptr(0);
                        let mut with_callee = operands.clone();
                        with_callee.push(Operand::Constant(
                            context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }),
                        ));
                        let made = function.create_instruction(
                            Opcode::Call(Box::new(CallInfo {
                                function_type,
                                calling_convention: 0,
                                return_attrs: Vec::new(),
                                argument_attrs: vec![Vec::new(); 3],
                                attrs: Vec::new(),
                                tail: Tail::default(),
                            })),
                            vector_ty,
                            with_callee,
                            flags,
                            None,
                        );
                        placer.after(function, last_of(entry), made);
                        let value = result_of(function, made);
                        vectors[at] = Some(reused(context, function, &mut placer, last_of(entry), entry, value));
                        continue;
                    }
                    Kind::Store | Kind::Gather | Kind::Extract(_) | Kind::Phi => unreachable!(),
                };
                let ops = if matches!(kind, Kind::Load) {
                    vec![function.instruction(entry.insts[0]).operands[0]]
                } else {
                    operands.clone()
                };
                let made = function.create_instruction(opcode, vector_ty, ops, flags, None);
                placer.after(function, last_of(entry), made);
                let value = result_of(function, made);
                Some(reused(context, function, &mut placer, last_of(entry), entry, value))
            }
        };
        vectors[at] = made;
    }
    // The vector store, at the last store.
    let root = &tree.entries[0];
    if root.kind == Kind::Store {
        let last = last_of(root);
        let vector = vectors[root.operands[0]].expect("the stored vector");
        let pointer = function.instruction(root.insts[0]).operands[1];
        let void = context.types.void();
        let store = function.create_instruction(
            Opcode::Store { align: store_align, volatile: false },
            void,
            vec![vector, pointer],
            Flags::default(),
            None,
        );
        placer.after(function, last, store);
    }
    // The lanes some user outside the tree reads.
    let tree_insts: Vec<InstId> = tree.entries.iter().flat_map(|entry| entry.insts.iter().copied()).collect();
    let i32_ty = context.types.int(64);
    for (at, entry) in tree.entries.iter().enumerate() {
        if matches!(entry.kind, Kind::Gather | Kind::Store | Kind::Extract(_)) {
            continue;
        }
        let vector = vectors[at].expect("a vector");
        for (lane, inst) in entry.insts.iter().enumerate() {
            let Some(result) = function.instruction(*inst).result else { continue };
            let outside: Vec<_> = function
                .users(result)
                .iter()
                .filter(|used| !tree_insts.contains(&used.user))
                .map(|used| (used.user, used.index))
                .collect();
            if outside.is_empty() {
                continue;
            }
            // `findLaneForValue`: the first place the lane is read.
            let place = entry.reuse.iter().position(|&one| one == lane as i32).unwrap_or(lane);
            let index =
                Operand::Constant(context.constant(Constant { ty: i32_ty, kind: ConstantKind::Int(place as u128) }));
            let Operand::Value(vector_value) = vector else { continue };
            let ValueDef::Instruction(defined) = function.value(vector_value).def else { continue };
            let extract = function.create_instruction(
                Opcode::ExtractElement,
                entry.lane_ty,
                vec![vector, index],
                Flags::default(),
                None,
            );
            if entry.kind == Kind::Phi {
                // Phis stay grouped: the extract follows the last of them.
                placer.after(function, last_phi_of(function, entry.insts[0]), extract);
            } else {
                function.insert(extract, Position::After(defined)).expect("the vector's block");
            }
            let value = result_of(function, extract);
            for (user, operand) in outside {
                let mut operands = function.instruction(user).operands.clone();
                operands[operand as usize] = value;
                function.set_operands(user, operands);
            }
        }
    }
    // The scalars: each goes where nothing reads it; a user may stand in a
    // block after its operand's, so the rest are taken again. What is left
    // reads only itself (a phi and its latch value): all go at once.
    let mut doomed: Vec<InstId> = tree_insts;
    loop {
        let before = doomed.len();
        doomed.retain(|inst| {
            let used =
                function.instruction(*inst).result.is_some_and(|result| function.users(result).iter().next().is_some());
            if !used {
                function.erase(*inst).expect("a tree scalar");
            }
            used
        });
        if doomed.is_empty() || doomed.len() == before {
            break;
        }
    }
    if !doomed.is_empty() {
        function.erase_all(&doomed).expect("scalars only the tree reads");
    }
    reschedule(function, &blocks, &tree.impure);
    let _ = ValueId(0);
}

/// The instruction that defines `vector`.
fn defined_by(
    function: &Function,
    vector: Operand,
) -> InstId {
    let Operand::Value(id) = vector else { unreachable!("a made vector") };
    let ValueDef::Instruction(inst) = function.value(id).def else { unreachable!("a made vector") };
    inst
}

/// The last phi of the block of `inst`, a phi.
fn last_phi_of(
    function: &Function,
    inst: InstId,
) -> InstId {
    let block = function.parent(inst).expect("a placed phi");
    function
        .iter_block(block)
        .take_while(|one| function.instruction(*one).opcode == Opcode::Phi)
        .last()
        .expect("a phi")
}

/// The entry's vector read through its reuse mask (`FinalShuffle`).
fn reused(
    context: &mut Context,
    function: &mut Function,
    placer: &mut Placer,
    anchor: InstId,
    entry: &Entry,
    vector: Operand,
) -> Operand {
    if entry.reuse.is_empty() || (slpgather::identity(&entry.reuse) && entry.reuse.len() == entry.lanes.len()) {
        return vector;
    }
    let from = vector_of(context, entry.lane_ty, entry.lanes.len());
    let to = vector_of(context, entry.lane_ty, entry.width());
    let poison = Operand::Constant(context.constant(Constant { ty: from, kind: ConstantKind::Poison }));
    let made = function.create_instruction(
        Opcode::ShuffleVector(entry.reuse.clone()),
        to,
        vec![vector, poison],
        Flags::default(),
        None,
    );
    placer.after(function, anchor, made);
    result_of(function, made)
}

/// The vector of a gather: constants as a constant, any other as inserts of
/// the scalars `TryPackScalars` leaves and the shuffle that reads them (a value
/// in every lane is one insert and a broadcast).
fn build_vector(
    context: &mut Context,
    function: &mut Function,
    placer: &mut Placer,
    anchor: Anchor,
    entry: &Entry,
) -> Operand {
    let width = entry.width();
    let vector_ty = vector_of(context, entry.lane_ty, width);
    if entry.lanes.iter().all(|lane| constant_of(*lane)) {
        let lanes: Vec<_> = entry
            .lanes
            .iter()
            .map(|lane| match lane {
                Operand::Constant(id) => *id,
                _ => unreachable!(),
            })
            .collect();
        let kind = if lanes.iter().all(|id| lanes[0] == *id) && is_zero(context, lanes[0]) {
            ConstantKind::Zero
        } else {
            ConstantKind::Aggregate(lanes)
        };
        return Operand::Constant(context.constant(Constant { ty: vector_ty, kind }));
    }
    let i32_ty = context.types.int(64);
    let mut insert = |function: &mut Function, opcode: Opcode, operands: Vec<Operand>| {
        let made = function.create_instruction(opcode, vector_ty, operands, Flags::default(), None);
        placer.place(function, anchor, made);
        result_of(function, made)
    };
    let poison = Operand::Constant(context.constant(Constant { ty: vector_ty, kind: ConstantKind::Poison }));
    let (packed, mask) = slpgather::gather_plan(&entry.lanes, &entry.reuse, width);
    let mut vector = poison;
    for (lane, value) in packed.iter().enumerate() {
        let Some(value) = value else { continue };
        let index = Operand::Constant(context.constant(Constant { ty: i32_ty, kind: ConstantKind::Int(lane as u128) }));
        vector = insert(function, Opcode::InsertElement, vec![vector, *value, index]);
    }
    if slpgather::identity(&mask) {
        return vector;
    }
    insert(function, Opcode::ShuffleVector(mask), vec![vector, poison])
}

fn is_zero(
    context: &Context,
    id: llrm_mir::context::ConstantId,
) -> bool {
    matches!(
        context.get(id).kind,
        ConstantKind::Int(0) | ConstantKind::Float(0) | ConstantKind::Zero | ConstantKind::Null
    )
}

#[cfg(test)]
#[path = "slptree_tests.rs"]
mod tests;
