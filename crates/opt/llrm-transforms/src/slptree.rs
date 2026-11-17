//! `SLPVectorizer`'s tree: a window of consecutive stores, and the loads,
//! arithmetic, casts, compares, selects and `llvm.fmuladd` calls that make
//! their values, become vector code when the cost model says the vectors are
//! cheaper.
//!
//! # How it works
//! `BoUpSLP::buildTree` (l.12052 `buildTreeRec`) starts from the stores' values
//! and goes up the operands, one entry for each bundle of one lane per store:
//! the bundle of loads of consecutive cells, of one binary operation, one cast,
//! one compare, one select or one `llvm.fmuladd`; a bundle that is none of
//! these (constants, one value in every lane, anything else) is a gather, the
//! vector built from its scalars. The operands of a commutative operation are
//! put in the order that makes the bundles alike (`VLOperands::reorder`,
//! `getShallowScore` l.1119). `getTreeCost` (l.19795) adds each entry's vector
//! cost less its scalar ones (`getEntryCost` l.16168) and, for each scalar the
//! tree does not use, the extract of its lane; the tree is taken below the
//! `slp-threshold` of 0. `vectorizeTree` makes each bundle's vector after its
//! last scalar, and the vector store at the last store.
//!
//! DEVIATE SLP1 (owner arm64, exit 2026-11-24): the tree has no reordered or
//! reversed loads, no alternate-opcode, extract or gather-of-loads entries, no
//! lane reused within a bundle and no operand look-ahead past the shallow
//! score; the scale by the loop's trip count is left out (it multiplies every
//! entry alike, so the sign of the total is the same). A scalar the tree uses
//! as a gather lane, or an external user placed before its bundle's last
//! scalar, makes the tree not form (the pin's scheduler may order it).
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
    ArithOp, CastKind, CmpSel, CostKind, LaneOp, MemoryOp, Predicate, ShuffleKind, Tti, Ty, VectorInstrContext,
    VectorTy,
};
use llrm_mir::types::{FloatKind, Type, TypeId};

/// `MaxTreeRecursionDepth`.
const MAX_DEPTH: usize = 12;
/// `-slp-threshold`.
const COST_THRESHOLD: i64 = 0;

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
}

/// A tree that is worth making, ready for [`emit`].
pub(crate) struct Tree {
    entries: Vec<Entry>,
    block: BlockId,
}

struct Builder<'a, 'u> {
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

fn constant_of(operand: Operand) -> bool {
    matches!(operand, Operand::Constant(_))
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
        // A scalar the tree makes as a vector, taken again as a gather lane,
        // would wait on its own bundle (DEVIATE SLP1).
        if lanes.iter().any(|lane| self.inst_of(*lane).is_some_and(|inst| self.made.contains_key(&inst))) {
            self.failed = true;
        }
        self.entries.push(Entry { kind: Kind::Gather, lanes, insts: Vec::new(), operands: Vec::new(), lane_ty });
        self.entries.len() - 1
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
        let mut insts = Vec::with_capacity(lanes.len());
        for lane in &lanes {
            match self.inst_of(*lane) {
                Some(inst) if !self.made.contains_key(&inst) && !insts.contains(&inst) => insts.push(inst),
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
        let entry = self.entries.len();
        self.entries.push(Entry { kind, lanes: lanes.clone(), insts: insts.clone(), operands: Vec::new(), lane_ty });
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
        Builder { unit, function, block, entries: Vec::new(), made: IdMap::new(), position, failed: false };
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
    let tree = Tree { entries: builder.entries, block };
    let made = builder.made;
    let position = builder.position;
    if !schedulable(unit, function, &tree, &made, &position) {
        llrm_support::debug!("slp", "tree: not schedulable");
        return None;
    }
    let costs = costs_of(unit, function, &tree, &made, align, tti, scaler)?;
    let cost = costs.total();
    llrm_support::debug!("slp", "tree of {} entries x{}: cost {cost}", tree.entries.len(), stores.len());
    // `getTreeCost`: the tree with its unprofitable subtrees gathered, if that costs less.
    if let Some((trimmed, trimmed_made)) = trim(unit, function, &tree, &costs, tti, scaler) {
        let trimmed_cost = costs_of(unit, function, &trimmed, &trimmed_made, align, tti, scaler)?.total();
        llrm_support::debug!("slp", "Trimmed tree cost = {trimmed_cost}, restored tree cost = {cost}");
        if trimmed_cost < cost && schedulable(unit, function, &trimmed, &trimmed_made, &position) {
            llrm_support::debug!("slp", "tree of {} entries x{}: cost {trimmed_cost}", trimmed.entries.len(), stores.len());
            return (trimmed_cost < -COST_THRESHOLD).then_some(trimmed);
        }
    }
    (cost < -COST_THRESHOLD).then_some(tree)
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

/// DEVIATE SLP3: the scalars of the tree can stand as vectors where the last of
/// each bundle is.
fn schedulable(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
    position: &IdMap<InstId, usize>,
) -> bool {
    let at = |inst: &InstId| position[inst];
    let root = &tree.entries[0];
    let seed = function.parent(root.insts[0]).expect("a placed store");
    let (first_store, last_store) =
        (root.insts.iter().map(at).min().expect("stores"), root.insts.iter().map(at).max().expect("stores"));
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
            return false;
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
            if block == seed && last > first_store {
                llrm_support::debug!("slp", "tree: a load stands after the first store");
                return false;
            }
            for (order, inst) in function.iter_block(block).enumerate() {
                let writes = match function.instruction(inst).opcode {
                    Opcode::Call(_) => !pure_call(inst),
                    Opcode::Store { .. } | Opcode::Invoke(_) | Opcode::Atomic(_) | Opcode::VaArg => true,
                    _ => false,
                };
                if order > first && order < last && !made.contains_key(&inst) && writes {
                    llrm_support::debug!("slp", "tree: a write stands among the loads");
                    return false;
                }
            }
        }
        // A user outside the tree stands after its bundle's last scalar.
        for inst in &entry.insts {
            let Some(result) = function.instruction(*inst).result else { continue };
            for used in function.users(result).iter() {
                if made.contains_key(&used.user) {
                    continue;
                }
                match function.parent(used.user) {
                    Some(user_block) if user_block == block => {
                        if position[&used.user] <= last || matches!(function.instruction(used.user).opcode, Opcode::Phi)
                        {
                            llrm_support::debug!("slp", "tree: a user stands before its bundle's last scalar");
                            return false;
                        }
                    }
                    Some(_) => {}
                    None => return false,
                }
            }
        }
    }
    true
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

/// Each entry's cost, and of the extracts of its lanes some user outside the
/// tree reads (`ExtractCosts`), by the block they stand in.
struct Costs {
    entry: Vec<i64>,
    extract: Vec<i64>,
}

impl Costs {
    fn total(&self) -> i64 {
        self.entry.iter().chain(&self.extract).sum()
    }
}

/// `getTreeCost`: the entries' vector costs less their scalars', and the
/// extracts of the lanes used outside. `None`: a price the target lacks.
fn costs_of(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
    store_align: u64,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Costs> {
    let kind = CostKind::RecipThroughput;
    let context = unit.context;
    let mut costs = Costs { entry: Vec::new(), extract: Vec::new() };
    for (at, entry) in tree.entries.iter().enumerate() {
        let Some(priced) = entry_cost(unit, function, tree, entry, store_align, tti) else {
            llrm_support::debug!("slp", "entry {:?} x{}: not priced", entry.kind, entry.lanes.len());
            return None;
        };
        costs.entry.push(priced * entry_scale(function, tree, at, scaler) as i64);
        let mut extracts = 0i64;
        // The lanes some user outside the tree reads are extracted.
        if !matches!(entry.kind, Kind::Gather | Kind::Store) {
            let lane = lane_ty_of(context, entry.lane_ty)?;
            let vector = VectorTy { lane, count: entry.lanes.len() as u32 };
            for (index, inst) in entry.insts.iter().enumerate() {
                let Some(result) = function.instruction(*inst).result else { continue };
                // The first user outside fixes the scale
                // (`ExtractCostCalculated`).
                if let Some(used) = function.users(result).iter().find(|used| !made.contains_key(&used.user)) {
                    let extract = tti.vector_instr_cost(
                        LaneOp::Extract,
                        vector,
                        Some(index as u32),
                        false,
                        VectorInstrContext::None,
                        kind,
                    )?;
                    let block = function.parent(used.user)?;
                    let scale = scaler.scale_at(function, block, &[Operand::Value(result)]);
                    extracts += extract.valid.then_some(extract.value)? * scale as i64;
                }
            }
        }
        costs.extract.push(extracts);
    }
    Some(costs)
}

/// What the trimming has made of an entry.
#[derive(Clone, Copy, PartialEq)]
enum Fate {
    Kept,
    /// Gathered, at this cost (`TransformedToGatherNodes`).
    Gathered(i64),
    /// Under a gathered entry (`DeletedNodes`).
    Deleted,
}

/// The entries only the subtree of `root` reads: its descendants, less any
/// that a user outside the subtree also reads.
fn subtree_of(
    tree: &Tree,
    root: usize,
) -> Vec<usize> {
    let mut inside = vec![false; tree.entries.len()];
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        for child in &tree.entries[at].operands {
            if !inside[*child] {
                inside[*child] = true;
                stack.push(*child);
            }
        }
    }
    inside[root] = false;
    loop {
        let mut dropped = false;
        for at in 0..inside.len() {
            let shared = tree
                .entries
                .iter()
                .enumerate()
                .any(|(user, one)| one.operands.contains(&at) && user != root && !inside[user]);
            if inside[at] && shared {
                inside[at] = false;
                dropped = true;
            }
        }
        if !dropped {
            break;
        }
    }
    (0..inside.len()).filter(|at| inside[*at]).collect()
}

/// `getTreeCost`, "Trimming unprofitable subtree": a subtree that costs more
/// than gathering the scalars of its root is replaced by that gather. The
/// tree so trimmed, and the entries it makes; `None`: nothing was trimmed.
/// There is no alternate-shuffle entry (`IsEqualCostAltShuffleToTrim`,
/// `PreferTrimmedTree`), no split, combined or gathered-load entry.
fn trim(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    costs: &Costs,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<(Tree, IdMap<InstId, usize>)> {
    let count = tree.entries.len();
    let subtrees: Vec<Vec<usize>> = (0..count).map(|at| subtree_of(tree, at)).collect();
    let cost_at = |at: usize| costs.entry[at] + costs.extract[at];
    // `SubtreeCosts`, the worklist's keys: by the cost, the later entry first.
    let mut worklist: Vec<(i64, usize)> =
        (0..count).map(|at| (cost_at(at) + subtrees[at].iter().map(|one| cost_at(*one)).sum::<i64>(), at)).collect();
    worklist.sort_by(|a, b| b.cmp(a));
    let mut fate = vec![Fate::Kept; count];
    let mut changed = false;
    let context = unit.context;
    for (key, at) in worklist {
        if key <= 0 {
            break;
        }
        let entry = &tree.entries[at];
        if entry.kind == Kind::Gather || at == 0 || fate[at] == Fate::Deleted {
            continue;
        }
        let size = entry.lanes.len() as i64;
        let priced = |one: usize| match fate[one] {
            Fate::Kept => cost_at(one),
            Fate::Gathered(cost) => cost,
            Fate::Deleted => 0,
        };
        let total = priced(at) + subtrees[at].iter().map(|one| priced(*one)).sum::<i64>();
        if total < size {
            continue;
        }
        // `getScalarizationOverhead`: an insert of each lane that is no constant.
        let lane = lane_ty_of(context, entry.lane_ty)?;
        let vector = VectorTy { lane, count: size as u32 };
        let mut gather = 0i64;
        for (index, value) in entry.lanes.iter().enumerate() {
            if !constant_of(*value) {
                let insert = tti.vector_instr_cost(
                    LaneOp::Insert,
                    vector,
                    Some(index as u32),
                    index == 0,
                    VectorInstrContext::None,
                    CostKind::RecipThroughput,
                )?;
                gather += insert.valid.then_some(insert.value)?;
            }
        }
        let gathered_elsewhere = |value: &Operand| {
            constant_of(*value)
                || tree.entries.iter().any(|one| one.kind == Kind::Gather && one.lanes.contains(value))
        };
        if entry.lanes.iter().all(gathered_elsewhere) {
            gather *= 2;
        }
        gather *= entry_scale(function, tree, at, scaler) as i64;
        // A size that is no power of two may have an inflated cost.
        let margin = i64::from(subtrees[at].iter().any(|one| !tree.entries[*one].lanes.len().is_power_of_two()));
        if total > gather + margin {
            llrm_support::debug!(
                "slp",
                "Trimming unprofitable subtree at node {at} with cost {total} and gather cost {gather}"
            );
            fate[at] = Fate::Gathered(gather);
            for one in &subtrees[at] {
                fate[*one] = Fate::Deleted;
            }
            changed = true;
        }
    }
    if !changed {
        return None;
    }
    let mut renamed = vec![usize::MAX; count];
    let mut entries = Vec::new();
    for (at, entry) in tree.entries.iter().enumerate() {
        if fate[at] == Fate::Deleted {
            continue;
        }
        renamed[at] = entries.len();
        let gathered = matches!(fate[at], Fate::Gathered(_));
        entries.push(Entry {
            kind: if gathered { Kind::Gather } else { entry.kind.clone() },
            lanes: entry.lanes.clone(),
            insts: if gathered { Vec::new() } else { entry.insts.clone() },
            operands: if gathered { Vec::new() } else { entry.operands.clone() },
            lane_ty: entry.lane_ty,
        });
    }
    for entry in &mut entries {
        for operand in &mut entry.operands {
            *operand = renamed[*operand];
        }
    }
    // A store of a gather is no tree.
    if entries.len() < 2 || entries[entries[0].operands[0]].kind == Kind::Gather {
        return None;
    }
    let mut made = IdMap::new();
    for entry_at in 0..entries.len() {
        for inst in &entries[entry_at].insts {
            made.insert(*inst, entry_at);
        }
    }
    Some((Tree { entries, block: tree.block }, made))
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
            Kind::Gather => gather_cost(context, tti, entry, vector, kind)?,
        };
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

/// `getGatherCost`: a vector of constants is free; one value in every lane is
/// one insert and a broadcast; any other lane is one insert.
fn gather_cost(
    context: &Context,
    tti: &dyn Tti,
    entry: &Entry,
    vector: VectorTy,
    kind: CostKind,
) -> Option<i64> {
    let _ = context;
    let valid = |cost: llrm_mir::tti::Cost| cost.valid.then_some(cost.value);
    if entry.lanes.iter().all(|lane| constant_of(*lane)) {
        return Some(0);
    }
    let insert = |index: u32| {
        valid(tti.vector_instr_cost(LaneOp::Insert, vector, Some(index), index == 0, VectorInstrContext::None, kind)?)
    };
    if entry.lanes.iter().all(|lane| *lane == entry.lanes[0]) {
        let shuffle =
            valid(tti.shuffle_cost(ShuffleKind::Broadcast, vector, &vec![0; vector.count as usize], kind)?)?;
        return Some(insert(0)? + shuffle);
    }
    let mut total = 0;
    for (index, lane) in entry.lanes.iter().enumerate() {
        if !constant_of(*lane) {
            total += insert(index as u32)?;
        }
    }
    Some(total)
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
    layout: &llrm_mir::datalayout::DataLayout,
    declared: &mut Declared,
    function: &mut Function,
    tree: &Tree,
    store_align: Option<u64>,
) {
    let position = positions(function);
    let last_of = |entry: &Entry| *entry.insts.iter().max_by_key(|inst| position[*inst]).expect("a bundle");
    let mut placer = Placer::default();
    // The vector of each entry, children first.
    let mut vectors: Vec<Option<Operand>> = vec![None; tree.entries.len()];
    let mut order: Vec<usize> = Vec::new();
    fn visit(
        tree: &Tree,
        at: usize,
        order: &mut Vec<usize>,
    ) {
        for child in &tree.entries[at].operands {
            if !order.contains(child) {
                visit(tree, *child, order);
            }
        }
        if !order.contains(&at) {
            order.push(at);
        }
    }
    visit(tree, 0, &mut order);
    for at in order {
        let entry = &tree.entries[at];
        let count = entry.lanes.len();
        let vector_ty = vector_of(context, entry.lane_ty, count);
        let operands: Vec<Operand> =
            entry.operands.iter().map(|child| vectors[*child].expect("an operand made first")).collect();
        let made = match &entry.kind {
            Kind::Gather => {
                // Built right before the bundle that reads it.
                let user = tree.entries.iter().position(|one| one.operands.contains(&at)).expect("a user");
                // A user's vector stands after its last scalar: the gather
                // stands there too, before the user, after every lane it reads.
                let anchor = last_of(&tree.entries[user]);
                Some(build_vector(context, function, &mut placer, anchor, entry, vector_ty))
            }
            Kind::Store => None,
            kind => {
                let lead = function.instruction(entry.insts[0]).clone();
                let mut flags = lead.flags;
                for inst in &entry.insts[1..] {
                    flags = flags.intersect(function.instruction(*inst).flags);
                }
                let opcode = match kind {
                    Kind::Load => Opcode::Load {
                        // `LoadInst::getAlign`: no `align` is the scalar's ABI
                        // alignment.
                        align: match lead.opcode {
                            Opcode::Load { align, .. } => align.or_else(|| Some(layout.align(&context.types, lead.ty))),
                            _ => None,
                        },
                        volatile: false,
                    },
                    Kind::Binary(op) => Opcode::Binary(*op),
                    Kind::Cast(op, _) => Opcode::Cast(*op),
                    Kind::ICmp(predicate) => Opcode::ICmp(*predicate),
                    Kind::FCmp(predicate) => Opcode::FCmp(*predicate),
                    Kind::Select => Opcode::Select,
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
                        vectors[at] = Some(result_of(function, made));
                        continue;
                    }
                    Kind::Store | Kind::Gather => unreachable!(),
                };
                let ops = if matches!(kind, Kind::Load) {
                    vec![function.instruction(entry.insts[0]).operands[0]]
                } else {
                    operands.clone()
                };
                let made = function.create_instruction(opcode, vector_ty, ops, flags, None);
                placer.after(function, last_of(entry), made);
                Some(result_of(function, made))
            }
        };
        vectors[at] = made;
    }
    // The vector store, at the last store.
    let root = &tree.entries[0];
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
    // The lanes some user outside the tree reads.
    let tree_insts: Vec<InstId> = tree.entries.iter().flat_map(|entry| entry.insts.iter().copied()).collect();
    let i32_ty = context.types.int(64);
    for (at, entry) in tree.entries.iter().enumerate() {
        if matches!(entry.kind, Kind::Gather | Kind::Store) {
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
            let index =
                Operand::Constant(context.constant(Constant { ty: i32_ty, kind: ConstantKind::Int(lane as u128) }));
            let Operand::Value(vector_value) = vector else { continue };
            let ValueDef::Instruction(defined) = function.value(vector_value).def else { continue };
            let extract = function.create_instruction(
                Opcode::ExtractElement,
                entry.lane_ty,
                vec![vector, index],
                Flags::default(),
                None,
            );
            function.insert(extract, Position::After(defined)).expect("the vector's block");
            let value = result_of(function, extract);
            for (user, operand) in outside {
                let mut operands = function.instruction(user).operands.clone();
                operands[operand as usize] = value;
                function.set_operands(user, operands);
            }
        }
    }
    // The scalars: each goes where nothing reads it; a user may stand in a
    // block after its operand's, so the rest are taken again.
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
    let _ = ValueId(0);
}

/// The vector of a gather: constants as a constant, one value as a splat, any
/// other as inserts.
fn build_vector(
    context: &mut Context,
    function: &mut Function,
    placer: &mut Placer,
    anchor: InstId,
    entry: &Entry,
    vector_ty: TypeId,
) -> Operand {
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
    let count = entry.lanes.len();
    let mut insert = |function: &mut Function, opcode: Opcode, operands: Vec<Operand>| {
        let made = function.create_instruction(opcode, vector_ty, operands, Flags::default(), None);
        placer.after(function, anchor, made);
        result_of(function, made)
    };
    let poison = Operand::Constant(context.constant(Constant { ty: vector_ty, kind: ConstantKind::Poison }));
    if entry.lanes.iter().all(|lane| *lane == entry.lanes[0]) {
        let zero = Operand::Constant(context.constant(Constant { ty: i32_ty, kind: ConstantKind::Int(0) }));
        let one = insert(function, Opcode::InsertElement, vec![poison, entry.lanes[0], zero]);
        return insert(function, Opcode::ShuffleVector(vec![0; count]), vec![one, poison]);
    }
    let mut vector = poison;
    for (lane, value) in entry.lanes.iter().enumerate() {
        let index = Operand::Constant(context.constant(Constant { ty: i32_ty, kind: ConstantKind::Int(lane as u128) }));
        vector = insert(function, Opcode::InsertElement, vec![vector, *value, index]);
    }
    vector
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
