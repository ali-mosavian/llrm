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
//! total is the same). A scalar the tree uses
//! as a gather lane, or an external user placed before its bundle's last
//! scalar, makes the tree not form (the pin's scheduler may order it).
//!
//! DEVIATE SLP3 (owner arm64, exit 2026-11-24): `BlockScheduling`'s dependency
//! graph is not built: a load that stands after one of the window's stores must
//! be one that `BasicAA`, scoped-noalias and TBAA say cannot read its bytes,
//! and nothing else between the first load and the last store may write memory,
//! nor read it after the first store. A load or store between the window's
//! stores that `BasicAA`, scoped-noalias and TBAA say cannot touch their bytes
//! may stay (`StoreSeparation`); a call may not.
//!
//! # Runs
//! Under `slpvectorize`, on a window of stores whose values are not all
//! constants.
//!
//! # Used by
//! `slpvectorize`.
//!
//! # Example
//! Four `fmuladd`s of one scalar and four consecutive loads, plus a constant,
//! stored to four consecutive floats, become one vector chain.
//!
//! ```text
//! 4 x (load c[i]; fmuladd a, c[i], 1.0; fadd 128.5; store out[i])
//!         =>
//! load <4 x float>; fmuladd.v4f32 (splat a, v, splat 1.0)
//! fadd <4 x float>; store <4 x float>
//! ```

use llrm_analysis::memory::{MemRef, Unit as MemUnit};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::context::{Constant, ConstantKind, Context};
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::loopinfo::Loop;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, FloatPredicate, IntPredicate, Opcode, Tail};
use llrm_mir::passes::Declared;
use llrm_mir::tti::{
    ArithOp, CastKind, CmpSel, CostKind, LaneOp, MemoryOp, OperandInfo, Predicate, Tti, Ty, VectorInstrContext,
    VectorTy,
};
use llrm_mir::types::{FloatKind, Type, TypeId};
use llrm_support::hash::HashMap;

use crate::slpgather::{self, constant_of};

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
    FNeg,
    /// `extractelement` of every lane of one vector, in order: the vector
    /// itself is the bundle.
    Extract(Operand),
    /// The phis of one block, one vector phi: its operands are the incoming
    /// values of every phi, one entry for each incoming block.
    Phi,
    /// The inserts of a chain of `insertelement`s: the root of a tree whose
    /// bundle is the scalars they insert.
    Insert,
    /// The operations of a horizontal reduction: the root of a tree whose
    /// bundle is the reduced values (`HorizontalReduction`, `buildTree(VL,
    /// IgnoreList)`). Its `insts` are the chain's links, which are not
    /// vectorized and whose uses of the values are not external ones.
    Reduction,
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
    /// `ReuseShuffleIndices`: `lanes` are the distinct scalars and the entry's
    /// vector reads lane `reuse[i]` of them at `i`. Empty: none repeats.
    reuse: Vec<i32>,
    /// A phi bundle's incoming blocks, in the order of its operands.
    blocks: Vec<BlockId>,
    /// An insert root: where each scalar goes.
    insert: Option<crate::slpinsert::Inserts>,
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
    /// A reduction tree's chain.
    pub(crate) reduction: Option<ReductionInfo>,
    block: BlockId,
    /// The calls of the block that are pure computations (`llvm.fmuladd`).
    pure_calls: IdSet<InstId>,
    /// The vectors stand where the block's order and what they read allow
    /// (`BlockScheduling`): a root with scalar users that stand before its
    /// last lane.
    reschedule: bool,
}

#[derive(Clone, Copy)]
enum Mode {
    Load,
    Opcode,
    Constant,
    Splat,
}

struct Builder<'a, 'u, 's> {
    scaler: &'a Scaler<'s>,
    tti: &'a dyn Tti,
    unit: &'a MemUnit<'u>,
    function: &'a Function,
    block: BlockId,
    entries: Vec<Entry>,
    /// The entry that makes each scalar instruction.
    made: IdMap<InstId, usize>,
    /// Where each instruction of the block stands.
    position: std::borrow::Cow<'a, IdMap<InstId, usize>>,
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

impl Builder<'_, '_, '_> {
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
        // `tryToFindDuplicates` for a gather: the distinct lanes and a shuffle
        // that repeats them, where that costs less than inserting every lane.
        let (unique, reuse) = slpgather::unique_lanes(&lanes);
        let (lanes, reuse) = match self.pack_repeats(&lanes, &unique, &reuse, lane_ty, true) {
            Some(true) => (unique, reuse),
            _ => (lanes, Vec::new()),
        };
        self.entries.push(Entry {
            kind: Kind::Gather,
            lanes,
            insts: Vec::new(),
            operands: Vec::new(),
            lane_ty,
            reuse,
            blocks: Vec::new(),
            insert: None,
        });
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
            kind: cost_kind(self.function),
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

    /// Whether `value` is loop invariant in the loop of the bundle's block.
    fn invariant(
        &self,
        value: Operand,
    ) -> bool {
        self.scaler.loop_invariant(self.function, self.block, value)
    }

    /// `getSameOpcode({a, b})` of two instructions.
    fn same_opcode(
        &self,
        a: Operand,
        b: Operand,
    ) -> bool {
        let (Some(one), Some(two)) = (self.inst_of(a), self.inst_of(b)) else { return false };
        let (one, two) = (self.function.instruction(one), self.function.instruction(two));
        one.opcode == two.opcode && one.ty == two.ty
    }

    /// `VLOperands::reorder` of a commutative pair: `pairs[lane]` are the two
    /// operands of each lane; returns them in the order each column best
    /// matches the lane before it.
    ///
    /// DEVIATE SLP9 (owner arm64, exit 2026-12-01): the look-ahead score is
    /// `getShallowScore` (depth 1, not 2), the operands' alternate opcodes are
    /// not followed, and `Failed` is never a mode.
    fn reorder(
        &self,
        pairs: &[[Operand; 2]],
    ) -> Vec<[Operand; 2]> {
        let lanes = pairs.len();
        let mut data = pairs.to_vec();
        let mut used = vec![[false; 2]; lanes];
        let first = self.best_lane_to_start(&data);
        let mut modes = [Mode::Splat; 2];
        for (index, mode) in modes.iter_mut().enumerate() {
            let operand = data[first][index];
            *mode = if let Some(inst) = self.inst_of(operand) {
                if self.should_broadcast(&data, &mut used, operand, index, first)
                    || !self.can_be_vectorized(&data, &used, inst, first)
                {
                    Mode::Splat
                } else if matches!(self.function.instruction(inst).opcode, Opcode::Load { .. }) {
                    Mode::Load
                } else {
                    Mode::Opcode
                }
            } else if constant_of(operand) {
                Mode::Constant
            } else {
                Mode::Splat
            };
        }
        used.iter_mut().for_each(|flags| *flags = [false; 2]);
        // A perfect or shuffled diamond needs no reordering.
        let unique: Vec<Operand> = {
            let mut seen = Vec::new();
            for pair in &data {
                if !seen.contains(&pair[0]) {
                    seen.push(pair[0]);
                }
            }
            seen
        };
        if data.iter().all(|pair| unique.contains(&pair[1])) && unique.len() != 2 {
            return data;
        }
        let mut scores: HashMap<(usize, usize), i32> = HashMap::default();
        for _pass in 0..2 {
            let mut failed = false;
            used.iter_mut().for_each(|flags| *flags = [false; 2]);
            for distance in 1..lanes {
                for direction in [1i64, -1] {
                    let lane = first as i64 + direction * distance as i64;
                    if lane < 0 || lane >= lanes as i64 {
                        continue;
                    }
                    let (lane, last) = (lane as usize, (lane - direction) as usize);
                    for index in 0..2 {
                        match self.best_operand(&data, &mut used, &mut scores, &modes, index, lane, last) {
                            Some(best) => {
                                data[lane].swap(index, best);
                                used[lane].swap(index, best);
                            }
                            None => failed = true,
                        }
                    }
                }
            }
            if !failed {
                break;
            }
        }
        data
    }

    /// `getBestLaneToStartReordering`: the lane whose operands have the
    /// fewest to match, the most common shape of operands breaking a tie.
    fn best_lane_to_start(
        &self,
        data: &[[Operand; 2]],
    ) -> usize {
        // Both operands of a commutative pair share one APO: every lane has
        // the same count of operands free to move.
        let mut same_count = usize::MAX;
        // (hash, votes, lane), in the order they were made.
        let mut votes: Vec<(u64, usize, usize)> = Vec::new();
        for lane in (0..data.len()).rev() {
            let mut majority: Option<InstId> = None;
            let mut count = 0usize;
            let mut hash = 0u64;
            for (index, operand) in data[lane].iter().enumerate() {
                if let Some(inst) = self.inst_of(*operand) {
                    let same = majority.is_some_and(|one| {
                        self.same_opcode(
                            Operand::Value(self.function.instruction(one).result.expect("a value")),
                            *operand,
                        ) && self.function.parent(one) == self.function.parent(inst)
                    });
                    if !same {
                        if count == 0 {
                            count = 1;
                            majority = Some(inst);
                        } else {
                            count -= 1;
                        }
                    } else {
                        count += 1;
                    }
                }
                let id = match self.inst_of(*operand) {
                    Some(inst) => {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        format!("{:?}", self.function.instruction(inst).opcode).hash(&mut hasher);
                        hasher.finish() | 4
                    }
                    None if constant_of(*operand) => 1,
                    None => 2,
                };
                hash = hash.wrapping_mul(31).wrapping_add((index as u64 + 1).wrapping_mul(id.wrapping_add(1)));
            }
            if count < same_count {
                same_count = count;
                votes.clear();
                votes.push((hash, 1, lane));
            } else if count == same_count {
                match votes.iter_mut().find(|vote| vote.0 == hash) {
                    Some(vote) => vote.1 += 1,
                    None => votes.push((hash, 1, lane)),
                }
            }
        }
        let mut best = (usize::MAX, 0);
        for (_, count, lane) in votes.iter().rev() {
            if *count < best.0 {
                best = (*count, *lane);
            }
        }
        best.1
    }

    /// `shouldBroadcast`: enough lanes read `operand`, or a constant or a loop
    /// invariant value that can stand where it does not, to fill a vector.
    fn should_broadcast(
        &self,
        data: &[[Operand; 2]],
        used: &mut [[bool; 2]],
        operand: Operand,
        index: usize,
        lane: usize,
    ) -> bool {
        let lanes = data.len();
        if matches!(
            self.inst_of(operand).map(|inst| &self.function.instruction(inst).opcode),
            Some(Opcode::Load { .. })
        ) && lanes == 2
        {
            return false;
        }
        let _ = index;
        let invariant = self.invariant(operand);
        let mut count = 0;
        let found = (0..lanes).filter(|other| *other != lane).all(|other| {
            for slot in 0..2 {
                if used[other][slot] {
                    continue;
                }
                let value = data[other][slot];
                let constant_here = constant_of(data[lane][slot]);
                let candidate = value == operand
                    || (!constant_here
                        && ((lanes > 2 && constant_of(value))
                            || (lanes == 2
                                && !self.same_opcode(operand, data[other][(slot + 1) % 2])
                                && constant_of(value))))
                    || (invariant && !constant_of(value) && !self.same_opcode(operand, value) && self.invariant(value));
                if candidate {
                    used[other][slot] = value == operand;
                    if value == operand {
                        count += 1;
                    }
                    return true;
                }
            }
            false
        });
        used.iter_mut().for_each(|flags| *flags = [false; 2]);
        found && (lanes == 2 || count > 1)
    }

    /// `canBeVectorized`: some other lane has an operand `inst` can share a
    /// vector with.
    fn can_be_vectorized(
        &self,
        data: &[[Operand; 2]],
        used: &[[bool; 2]],
        inst: InstId,
        lane: usize,
    ) -> bool {
        let operand = Operand::Value(self.function.instruction(inst).result.expect("a value"));
        (0..data.len())
            .filter(|other| *other != lane)
            .any(
                |other| (0..2).any(|slot| {
                    let value = data[other][slot];
                    used[other][slot]
                        || self.invariant(value)
                        || (self.same_opcode(operand, value)
                            && self
                                .inst_of(value)
                                .is_some_and(|there| self.function.parent(there) == self.function.parent(inst)))
                }),
            )
    }

    /// `getBestOperand`: which operand of `lane` goes to column `index`, to
    /// match the one `last` has there.
    #[allow(clippy::too_many_arguments)]
    fn best_operand(
        &self,
        data: &[[Operand; 2]],
        used: &mut [[bool; 2]],
        scores: &mut HashMap<(usize, usize), i32>,
        modes: &[Mode; 2],
        index: usize,
        lane: usize,
        last: usize,
    ) -> Option<usize> {
        let previous = data[last][index];
        let mode = modes[index];
        let mut best: Option<usize> = None;
        let mut best_score = *scores.entry((index, lane)).or_insert(0);
        let mut is_used = matches!(mode, Mode::Splat | Mode::Constant | Mode::Load);
        for slot in 0..2 {
            if used[lane][slot] {
                continue;
            }
            let operand = data[lane][slot];
            match mode {
                Mode::Load | Mode::Opcode => {
                    let (left, right) = if lane > last { (previous, operand) } else { (operand, previous) };
                    let score = self.shallow_score(left, right);
                    if score > best_score || (score > 0 && score == best_score && slot == index) {
                        best = Some(slot);
                        best_score = score;
                        scores.insert((index, lane), score);
                    }
                }
                Mode::Constant => {
                    if constant_of(operand) || (best_score == 0 && self.invariant(operand)) {
                        best = Some(slot);
                        if constant_of(operand) {
                            best_score = SCORE_CONSTANTS;
                            scores.insert((index, lane), SCORE_CONSTANTS);
                        } else {
                            is_used = false;
                        }
                    }
                }
                Mode::Splat => {
                    if operand == previous || (best_score == 0 && constant_of(operand)) {
                        is_used = operand == previous;
                        if operand == previous {
                            best_score = SCORE_SPLAT;
                            scores.insert((index, lane), SCORE_SPLAT);
                        }
                        best = Some(slot);
                    }
                }
            }
        }
        if let Some(slot) = best {
            used[lane][slot] = is_used;
        }
        best
    }

    /// The operand columns of a bundle, the commutative pair put in the order
    /// `VLOperands::reorder` gives.
    fn columns(
        &self,
        insts: &[InstId],
        arity: usize,
        swappable: bool,
        skip: usize,
    ) -> Vec<Vec<Operand>> {
        let mut columns: Vec<Vec<Operand>> = vec![Vec::new(); arity];
        let pairs: Option<Vec<[Operand; 2]>> = swappable.then(|| {
            let pairs: Vec<[Operand; 2]> = insts
                .iter()
                .map(|inst| {
                    let operands = &self.function.instruction(*inst).operands;
                    [operands[skip], operands[skip + 1]]
                })
                .collect();
            self.reorder(&pairs)
        });
        for (lane, inst) in insts.iter().enumerate() {
            let operands = &self.function.instruction(*inst).operands;
            for at in 0..arity {
                let operand = match (&pairs, at.checked_sub(skip)) {
                    (Some(pairs), Some(slot @ (0 | 1))) => pairs[lane][slot],
                    _ => operands[at],
                };
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
        let (kind, arity, swappable, skip) = match &first.opcode {
            Opcode::Load { volatile: false, .. } => (Kind::Load, 0, false, 0),
            Opcode::Binary(op) => (Kind::Binary(*op), 2, commutative(*op), 0),
            Opcode::Phi => (Kind::Phi, 0, false, 0),
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
        // `buildTreeRec`, `Instruction::PHI`: the phis come from the same
        // edges; each edge's values are an operand.
        let phi_columns = if kind == Kind::Phi {
            match self.phi_columns(&insts) {
                Some(found) => Some(found),
                None => return self.gather(lanes, lane_ty),
            }
        } else {
            None
        };
        if !reuse.is_empty() && self.pack_repeats(&lanes, &unique, &reuse, lane_ty, false) != Some(true) {
            return self.gather(lanes, lane_ty);
        }
        let entry = self.entries.len();
        self.entries.push(Entry {
            kind,
            lanes: unique,
            insts: insts.clone(),
            operands: Vec::new(),
            lane_ty,
            reuse,
            blocks: Vec::new(),
            insert: None,
        });
        for inst in &insts {
            self.made.insert(*inst, entry);
        }
        if let Some((blocks, columns)) = phi_columns {
            self.entries[entry].blocks = blocks;
            let children: Vec<usize> = columns.into_iter().map(|column| self.build(column, depth + 1)).collect();
            self.entries[entry].operands = children;
        } else if arity > 0 {
            let columns = self.columns(&insts, arity, swappable, skip);
            let children: Vec<usize> = columns.into_iter().map(|column| self.build(column, depth + 1)).collect();
            self.entries[entry].operands = children;
        }
        entry
    }

    /// The incoming blocks of the first phi and, for each, the value every
    /// phi takes from it; none where the phis differ in their edges.
    fn phi_columns(
        &self,
        phis: &[InstId],
    ) -> Option<(Vec<BlockId>, Vec<Vec<Operand>>)> {
        let arms = |phi: InstId| -> Vec<(Operand, BlockId)> {
            self.function
                .instruction(phi)
                .operands
                .chunks(2)
                .filter_map(|pair| match pair {
                    [value, Operand::Block(from)] => Some((*value, *from)),
                    _ => None,
                })
                .collect()
        };
        let first = arms(phis[0]);
        let blocks: Vec<BlockId> = first.iter().map(|arm| arm.1).collect();
        // One value for each incoming block.
        let mut distinct = blocks.clone();
        distinct.sort_by_key(|block| block.0);
        distinct.dedup();
        if blocks.is_empty() || distinct.len() != blocks.len() {
            return None;
        }
        let mut columns: Vec<Vec<Operand>> = vec![Vec::new(); blocks.len()];
        for phi in phis {
            let taken = arms(*phi);
            if taken.len() != blocks.len() {
                return None;
            }
            for (at, block) in blocks.iter().enumerate() {
                columns[at].push(taken.iter().find(|arm| arm.1 == *block)?.0);
            }
        }
        Some((blocks, columns))
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

/// The width in bits of an integer lane type.
fn lane_bits(
    context: &Context,
    ty: TypeId,
) -> u32 {
    match context.types.get(ty) {
        Type::Int(bits) => u32::from(*bits),
        _ => 64,
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
    position: &IdMap<InstId, usize>,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Tree> {
    let values: Vec<Operand> = stores.iter().map(|store| function.instruction(*store).operands[0]).collect();
    let mut builder = Builder {
        scaler,
        tti,
        unit,
        function,
        block,
        entries: Vec::new(),
        made: IdMap::new(),
        position: std::borrow::Cow::Borrowed(position),
        failed: false,
    };
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
                reuse: Vec::new(),
                blocks: Vec::new(),
                insert: None,
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
    let mut entries = builder.entries;
    share_gathers(&mut entries);
    let tree = Tree { entries, block, pure_calls: Default::default(), reschedule: false, reduction: None };
    let made = builder.made;
    let builder_position = builder.position;
    if !schedulable(unit, function, &tree, &made, &builder_position) {
        llrm_support::debug!("slp", "tree: not schedulable");
        return None;
    }
    let (tree, cost) = trim_non_profitable(unit, function, tree, align, tti, scaler)?;
    llrm_support::debug!("slp", "tree of {} entries x{}: cost {cost}", tree.entries.len(), stores.len());
    (cost < -COST_THRESHOLD).then_some(tree)
}

/// `tryToVectorizeList` for phis: the tree whose root is the vector phi of
/// `phis`, if it pays.
pub(crate) fn try_phis(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    phis: &[InstId],
    facts: &BlockFacts,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Tree> {
    let values: Vec<Operand> = phis.iter().map(|phi| result_of(function, *phi)).collect();
    let mut builder = Builder {
        scaler,
        tti,
        unit,
        function,
        block,
        entries: Vec::new(),
        made: IdMap::new(),
        position: std::borrow::Cow::Borrowed(&facts.position),
        failed: false,
    };
    let root = builder.build(values, 0);
    if builder.failed || root != 0 || builder.entries[0].kind != Kind::Phi {
        llrm_support::debug!(
            "slp",
            "phi tree x{}: no tree{}",
            phis.len(),
            if builder.failed { " (a lane gathered)" } else { "" }
        );
        return None;
    }
    let mut entries = builder.entries;
    share_gathers(&mut entries);
    let tree = Tree { entries, block, pure_calls: facts.pure_calls.clone(), reschedule: false, reduction: None };
    let made = builder.made;
    if tiny_and_not_fully_vectorizable(&tree) {
        llrm_support::debug!("slp", "phi tree x{}: tiny", phis.len());
        return None;
    }
    if !schedulable_phis(function, &tree) {
        llrm_support::debug!("slp", "phi tree x{}: not schedulable", phis.len());
        return None;
    }
    let cost = cost_of(unit, function, &tree, &made, 0, tti, scaler)?;
    llrm_support::debug!("slp", "phi tree of {} entries x{}: cost {cost}", tree.entries.len(), phis.len());
    (cost < -COST_THRESHOLD).then_some(tree)
}

/// The instructions of `block` that compute from their operands alone and may
/// stand anywhere those do: the calls of a trivially vectorizable intrinsic,
/// and the divisions whose divisor is a constant that cannot trap
/// (`isSafeToSpeculativelyExecute`).
pub(crate) fn pure_instructions(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
) -> IdSet<InstId> {
    let context = unit.context;
    function
        .iter_block(block)
        .filter(|inst| {
            let instruction = function.instruction(*inst);
            match instruction.opcode {
                Opcode::Binary(op @ (BinaryOp::UDiv | BinaryOp::URem | BinaryOp::SDiv | BinaryOp::SRem)) => {
                    let signed = matches!(op, BinaryOp::SDiv | BinaryOp::SRem);
                    let Operand::Constant(divisor) = instruction.operands[1] else { return false };
                    let Some(bits) = context.types.int_bits(instruction.ty) else { return false };
                    match context.get(divisor).kind {
                        ConstantKind::Int(value) => {
                            let ones = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
                            value & ones != 0 && !(signed && value & ones == ones)
                        }
                        _ => false,
                    }
                }
                _ => llrm_mir::memory::callee(unit.context, function, *inst)
                    .and_then(|callee| unit.globals.get(callee.0 as usize)?.intrinsic())
                    .is_some_and(llrm_analysis::vectorutils::is_trivially_vectorizable),
            }
        })
        .collect()
}

/// `tryToVectorizeList` for the standalone seeds of `vectorizeSeeds`: the tree
/// whose root is the bundle of `seeds` themselves (their users stay scalar and
/// read the lanes), if it pays.
pub(crate) fn try_seeds(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    seeds: &[InstId],
    facts: &BlockFacts,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Tree> {
    let values: Vec<Operand> = seeds.iter().map(|seed| result_of(function, *seed)).collect();
    let mut builder = Builder {
        scaler,
        tti,
        unit,
        function,
        block,
        entries: Vec::new(),
        made: IdMap::new(),
        position: std::borrow::Cow::Borrowed(&facts.position),
        failed: false,
    };
    let root = builder.build(values, 0);
    if builder.failed || root != 0 || builder.entries[0].kind == Kind::Gather {
        llrm_support::debug!("slp", "seed tree x{}: no tree", seeds.len());
        return None;
    }
    let mut entries = builder.entries;
    share_gathers(&mut entries);
    let tree = Tree { entries, block, pure_calls: facts.pure_calls.clone(), reschedule: true, reduction: None };
    let made = builder.made;
    if tiny_and_not_fully_vectorizable(&tree) {
        llrm_support::debug!("slp", "seed tree x{}: tiny", seeds.len());
        return None;
    }
    if !schedulable_phis(function, &tree) {
        llrm_support::debug!("slp", "seed tree x{}: not schedulable", seeds.len());
        return None;
    }
    let cost = cost_of(unit, function, &tree, &made, 0, tti, scaler)?;
    llrm_support::debug!("slp", "seed tree of {} entries x{}: cost {cost}", tree.entries.len(), seeds.len());
    (cost < -COST_THRESHOLD).then_some(tree)
}

/// `tryToVectorizeList` for the inserts of a buildvector chain: the tree whose
/// root is the inserts (ordered by the element each sets), if it pays.
pub(crate) fn try_inserts(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    inserts: &[InstId],
    facts: &BlockFacts,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Tree> {
    let context = unit.context;
    if !crate::slpinsert::connected(function, inserts) {
        return None;
    }
    let indices: Option<Vec<u32>> =
        inserts.iter().map(|insert| crate::slpinsert::element_index(context, function, *insert)).collect();
    let indices = indices?;
    let first = crate::slpinsert::first_insert(function, inserts);
    let info = crate::slpinsert::Inserts {
        indices,
        elts: crate::slpinsert::elements_of(context, function.instruction(inserts[0]).ty)?,
        base: function.instruction(first).operands[0],
    };
    let values: Vec<Operand> = inserts.iter().map(|insert| function.instruction(*insert).operands[1]).collect();
    let lane_ty = function.operand_type(context, values[0])?;
    let mut builder = Builder {
        scaler,
        tti,
        unit,
        function,
        block,
        entries: Vec::new(),
        made: IdMap::new(),
        position: std::borrow::Cow::Borrowed(&facts.position),
        failed: false,
    };
    builder
        .entries
        .push(
            Entry {
                kind: Kind::Insert,
                lanes: values.clone(),
                insts: inserts.to_vec(),
                operands: Vec::new(),
                lane_ty,
                reuse: Vec::new(),
                blocks: Vec::new(),
                insert: Some(info),
            },
        );
    for insert in inserts {
        builder.made.insert(*insert, 0);
    }
    let child = builder.build(values, 1);
    builder.entries[0].operands = vec![child];
    if builder.failed || builder.entries.len() < 2 {
        llrm_support::debug!("slp", "insert tree x{}: no tree", inserts.len());
        return None;
    }
    let mut entries = builder.entries;
    share_gathers(&mut entries);
    let tree = Tree { entries, block, pure_calls: facts.pure_calls.clone(), reschedule: false, reduction: None };
    let made = builder.made;
    if tiny_and_not_fully_vectorizable(&tree) {
        llrm_support::debug!("slp", "insert tree x{}: tiny", inserts.len());
        return None;
    }
    if !schedulable_phis(function, &tree) {
        llrm_support::debug!("slp", "insert tree x{}: not schedulable", inserts.len());
        return None;
    }
    let (tree, cost) = trim_non_profitable(unit, function, tree, 0, tti, scaler)?;
    llrm_support::debug!("slp", "insert tree of {} entries x{}: cost {cost}", tree.entries.len(), inserts.len());
    (cost < -COST_THRESHOLD).then_some(tree)
}

/// `isGatherShuffledEntry`, the whole-vector case: a gather of the scalars a
/// bundle of the tree has, in its order, is that bundle's vector (the bundle
/// may have been made after the gather: the tree is walked depth first, and
/// stops at `MAX_DEPTH`). DEVIATE SLP8 (owner arm64, exit 2026-12-01): a
/// bundle in another order, or only some of whose scalars the gather reads,
/// stays a gather.
fn share_gathers(entries: &mut Vec<Entry>) {
    let mut redirect: Vec<usize> = (0..entries.len()).collect();
    for at in 0..entries.len() {
        let gather = &entries[at];
        if gather.kind != Kind::Gather
            || !gather.reuse.is_empty()
            || gather.lanes.iter().all(|lane| constant_of(*lane))
            || gather.lanes.iter().all(|lane| *lane == gather.lanes[0])
        {
            continue;
        }
        let same = entries
            .iter()
            .position(
                |one| !matches!(
                    one.kind,
                    Kind::Gather | Kind::Store | Kind::Insert | Kind::Reduction
                )
                    && one.reuse.is_empty()
                    && one.lanes == gather.lanes,
            );
        if let Some(found) = same {
            redirect[at] = found;
        }
    }
    if redirect.iter().enumerate().all(|(at, to)| at == *to) {
        return;
    }
    // The gathers that are shared go; the rest are numbered again.
    let mut renumber: Vec<usize> = vec![0; entries.len()];
    let mut next = 0;
    for at in 0..entries.len() {
        if redirect[at] == at {
            renumber[at] = next;
            next += 1;
        }
    }
    for at in 0..entries.len() {
        if redirect[at] != at {
            renumber[at] = renumber[redirect[at]];
        }
    }
    let mut at = 0;
    entries.retain(|_| {
        let keep = redirect[at] == at;
        at += 1;
        keep
    });
    for entry in entries.iter_mut() {
        for operand in &mut entry.operands {
            *operand = renumber[*operand];
        }
    }
}

/// `isTreeTinyAndNotFullyVectorizable`, the rules for a tree of the shapes this
/// pass makes: a tree of phis and gathers only, one vectorized entry fed by
/// gathers that only repack scalars, and three entries of which two are
/// gathers of a few scalars.
fn tiny_and_not_fully_vectorizable(tree: &Tree) -> bool {
    tiny_entries(&tree.entries)
}

/// [`tiny_and_not_fully_vectorizable`] over the bundles of a tree (a reduction
/// tree's without its chain).
fn tiny_entries(entries: &[Entry]) -> bool {
    let gather = |entry: &Entry| entry.kind == Kind::Gather;
    let splat = |entry: &Entry| entry.lanes.iter().all(|lane| *lane == entry.lanes[0]);
    let constant = |entry: &Entry| entry.lanes.iter().all(|lane| constant_of(*lane));
    let splat_or_constant = |entry: &Entry| splat(entry) || constant(entry);
    // PHI nodes and gathers only: the vector phi costs nothing, the gathers
    // something.
    if entries.iter().all(|entry| gather(entry) || entry.kind == Kind::Phi) {
        return true;
    }
    // No need to vectorize inserts of gathered values.
    if entries[0].kind == Kind::Insert
        && entries.len() == 2
        && gather(&entries[1])
        && (entries[1].width() <= 2 || !(splat(&entries[1]) || constant(&entries[1])))
    {
        return true;
    }
    // The single vectorized node, fed by buildvectors that repack scalars.
    let vectorized = entries.iter().filter(|entry| !gather(entry)).count();
    if vectorized == 1 && entries.iter().all(|entry| !gather(entry) || !splat_or_constant(entry)) {
        return true;
    }
    // Three entries, the last two gathers of up to four scalars.
    entries.len() == 3
        && entries[1..]
            .iter()
            .all(|entry| gather(entry) && entry.width() <= 4 && !entry.lanes.iter().all(|lane| constant_of(*lane)))
}

/// A horizontal reduction's chain (`HorizontalReduction`): the operation, its
/// links (`ReductionOps`, the root first) and the values left out of the vector
/// reduction, combined with its result as `op.rdx`.
#[derive(Clone)]
pub(crate) struct ReductionInfo {
    pub(crate) op: BinaryOp,
    pub(crate) links: Vec<InstId>,
    pub(crate) leftovers: Vec<Operand>,
}

/// `tryToReduce`'s tree for the window `values` of a chain: `buildTree(VL,
/// IgnoreList)`, then `calculateTreeCostAndTrimNonProfitable`, then the
/// reduction's own cost (`getReductionCost`); the tree if the total is below
/// the `slp-threshold`.
pub(crate) fn try_reduction(
    unit: &MemUnit,
    function: &Function,
    block: BlockId,
    info: &ReductionInfo,
    values: &[Operand],
    facts: &BlockFacts,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Tree> {
    let root = info.links[0];
    let mut builder = Builder {
        scaler,
        tti,
        unit,
        function,
        block,
        entries: Vec::new(),
        made: IdMap::new(),
        position: std::borrow::Cow::Borrowed(&facts.position),
        failed: false,
    };
    // The root: the chain, whose uses of the values are not external ones.
    builder
        .entries
        .push(
            Entry {
                kind: Kind::Reduction,
                lanes: values.to_vec(),
                insts: info.links.clone(),
                operands: Vec::new(),
                lane_ty: function.instruction(root).ty,
                reuse: Vec::new(),
                blocks: Vec::new(),
                insert: None,
            },
        );
    for link in &info.links {
        builder.made.insert(*link, 0);
    }
    let child = builder.build(values.to_vec(), 1);
    builder.entries[0].operands = vec![child];
    if builder.failed || builder.entries.len() < 2 || builder.entries[child].kind == Kind::Gather {
        llrm_support::debug!(
            "slp",
            "reduction tree x{}: {} entries{}",
            values.len(),
            builder.entries.len(),
            if builder.failed { ", a lane used as a gather" } else { "" }
        );
        return None;
    }
    let mut entries = builder.entries;
    share_gathers(&mut entries);
    if tiny_entries(&entries[1..]) {
        llrm_support::debug!("slp", "reduction tree x{}: tiny", values.len());
        return None;
    }
    let tree =
        Tree { entries, block, pure_calls: facts.divides.clone(), reschedule: false, reduction: Some(info.clone()) };
    let (tree, total) = trim_non_profitable(unit, function, tree, 0, tti, scaler)?;
    // Scheduling is of the tree that stays: the trimmed subtrees are scalars.
    if !schedulable_phis(function, &tree) {
        llrm_support::debug!("slp", "reduction tree x{}: not schedulable", values.len());
        return None;
    }
    llrm_support::debug!("slp", "reduction tree of {} entries x{}: cost {total}", tree.entries.len() - 1, values.len());
    (total < -COST_THRESHOLD).then_some(tree)
}

/// `tree` with the chain's leftovers set (the window is known only once the
/// tree is made).
pub(crate) fn with_reduction(
    mut tree: Tree,
    info: ReductionInfo,
) -> Tree {
    tree.reduction = Some(info);
    tree
}

/// The entries that make up the tree, by the instructions they stand for.
fn made_of(tree: &Tree) -> IdMap<InstId, usize> {
    let mut made = IdMap::new();
    for (at, entry) in tree.entries.iter().enumerate() {
        if entry.kind != Kind::Gather {
            for inst in &entry.insts {
                made.insert(*inst, at);
            }
        }
    }
    made
}

/// `calculateTreeCostAndTrimNonProfitable`: the subtree under an entry whose
/// cost, with the extracts of its lanes, is more than the buildvector of the
/// entry's own scalars is dropped for that gather (DEVIATE SLP-RDX2: no
/// splat subtrees, gathered loads, `MinBWs` or combined nodes). The trimmed
/// tree is kept when it costs less than the restored one, or the same when a
/// subtree cost as much as its gather. The tree and its cost, the reduction's
/// included.
///
/// DEVIATE SLP-TRIM-PHI (owner w11, exit 2026-11-29): the pin trims every
/// tree; phi trees are not trimmed here. Ours are 39 entries where the pin's
/// nbody trees are ~90 (a deeper tree, DEVIATE SLP-RDX2 and the gathers it
/// leaves), and trimming the smaller tree vectorizes a different first group:
/// nbody_phis gives 1 vector phi pair, the pin's 4.
fn trim_non_profitable(
    unit: &MemUnit,
    function: &Function,
    tree: Tree,
    store_align: u64,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<(Tree, i64)> {
    let made = made_of(&tree);
    let costs = entry_costs(unit, function, &tree, &made, store_align, tti, scaler)?;
    let n = tree.entries.len();
    // The entries under each entry, itself not counted.
    let under = |at: usize| -> Vec<usize> {
        let mut seen = vec![false; n];
        let mut stack: Vec<usize> = tree.entries[at].operands.clone();
        let mut found = Vec::new();
        while let Some(next) = stack.pop() {
            if !std::mem::replace(&mut seen[next], true) {
                found.push(next);
                stack.extend(tree.entries[next].operands.iter().copied());
            }
        }
        found
    };
    let subtrees: Vec<Vec<usize>> = (0..n).map(under).collect();
    let own = |at: usize| costs[at].0 + costs[at].1;
    let total = |at: usize| own(at) + subtrees[at].iter().map(|child| own(*child)).sum::<i64>();
    // `calculateTreeCostAndTrimNonProfitable`'s early exit: nothing to trim
    // when the tree already pays.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|at| std::cmp::Reverse((total(*at), *at)));
    let context = unit.context;
    let mut gather_of: Vec<Option<i64>> = vec![None; n];
    let mut deleted = vec![false; n];
    let mut prefer_trimmed = false;
    for at in order {
        if total(at) <= 0 {
            break;
        }
        let entry = &tree.entries[at];
        // The root of the pin's tree (its entry 0) is never trimmed: for a
        // reduction it is the bundle of the reduced values, here under ours.
        if entry.kind == Kind::Gather
            || at == 0
            || (tree.reduction.is_some() && at == tree.entries[0].operands[0])
            || deleted[at]
            || gather_of[at].is_some()
        {
            continue;
        }
        // The costs of the subtrees already turned into gathers stand in for
        // their own.
        let mut subtree_total = total(at);
        for child in &subtrees[at] {
            if let Some(gather) = gather_of[*child] {
                subtree_total -= total(*child);
                subtree_total += gather;
            }
        }
        if subtree_total < 0 || subtree_total < entry.lanes.len() as i64 {
            continue;
        }
        let lane = lane_ty_of(context, entry.lane_ty)?;
        let mut gather = slpgather::gather_cost(tti, cost_kind(function), lane, &entry.lanes, &entry.reuse)?;
        let scale = entry_scale(function, &tree, at, scaler) as i64;
        gather *= scale;
        // Non-power-of-2 entries may have inflated costs: a margin of 1.
        let margin =
            i64::from(subtrees[at].iter().chain([&at]).any(|one| !tree.entries[*one].lanes.len().is_power_of_two()));
        if subtree_total > gather + margin {
            prefer_trimmed |= subtree_total == gather;
            llrm_support::debug!("slp", "trim #{at}: subtree {subtree_total} against its gather {gather}");
            gather_of[at] = Some(gather);
            for child in &subtrees[at] {
                deleted[*child] = true;
                gather_of[*child] = None;
            }
        }
    }
    let trimmed_cost: i64 = (0..n)
        .filter(|at| !deleted[*at])
        .map(|at| match gather_of[at] {
            Some(gather) => gather,
            None => own(at),
        })
        .sum();
    let restored_cost: i64 = (0..n).map(own).sum();
    let reverted = gather_of.iter().all(Option::is_none)
        || trimmed_cost > restored_cost
        || (!prefer_trimmed && trimmed_cost == restored_cost);
    if reverted {
        return Some((tree, restored_cost));
    }
    // The trimmed tree: the gathers in place of their subtrees, the rest
    // renumbered.
    let mut entries = tree.entries;
    for (at, gather) in gather_of.iter().enumerate() {
        if gather.is_some() {
            let entry = &mut entries[at];
            entry.kind = Kind::Gather;
            entry.insts = Vec::new();
            entry.operands = Vec::new();
        }
    }
    let kept: Vec<usize> = (0..n).filter(|at| !deleted[*at]).collect();
    let renumber: Vec<usize> = {
        let mut next = 0;
        (0..n)
            .map(|at| {
                let now = next;
                if !deleted[at] {
                    next += 1;
                }
                now
            })
            .collect()
    };
    let mut slots: Vec<Option<Entry>> = entries.into_iter().map(Some).collect();
    let mut entries: Vec<Entry> = kept.iter().map(|at| slots[*at].take().expect("an entry")).collect();
    for entry in &mut entries {
        for operand in &mut entry.operands {
            *operand = renumber[*operand];
        }
    }
    let tree = Tree {
        entries,
        block: tree.block,
        pure_calls: tree.pure_calls,
        reschedule: tree.reschedule,
        reduction: tree.reduction,
    };
    let made = made_of(&tree);
    let total = cost_of(unit, function, &tree, &made, store_align, tti, scaler)?;
    Some((tree, total))
}

/// DEVIATE SLP6 (owner arm64, exit 2026-12-01): the scalars of a tree of phis
/// stand as vectors where the last of each bundle is (`BlockScheduling` would
/// move them, and any scalar between): the block is one, no bundle stores or
/// calls, a bundle of loads has no write among them (no alias analysis), and no
/// lane depends on another of its bundle.
fn schedulable_phis(
    function: &Function,
    tree: &Tree,
) -> bool {
    let home = tree.block;
    for entry in &tree.entries {
        if matches!(entry.kind, Kind::Gather) {
            continue;
        }
        if !matches!(
            entry.kind,
            Kind::Phi
                | Kind::Insert
                | Kind::Reduction
                | Kind::Load
                | Kind::Binary(_)
                | Kind::FMulAdd
                | Kind::Cast(..)
                | Kind::ICmp(_)
                | Kind::FCmp(_)
                | Kind::Select
                | Kind::FNeg
                | Kind::Extract(_)
        ) {
            llrm_support::debug!("slp", "phi tree: a {:?} bundle", entry.kind);
            return false;
        }
        if entry.insts.iter().any(|inst| function.parent(*inst) != Some(home)) {
            llrm_support::debug!("slp", "phi tree: a bundle outside the loop's block");
            return false;
        }
        if entry.kind == Kind::Phi {
            // The loop is one block: its edges are its own and one other.
            if !entry.blocks.contains(&home) || entry.blocks.len() != 2 {
                llrm_support::debug!("slp", "phi tree: the loop is not one block");
                return false;
            }
            continue;
        }
    }
    // The bundles are one instruction each: they must not wait on each other.
    let mut bundles: Vec<(Vec<InstId>, Vec<InstId>)> = Vec::new();
    for entry in &tree.entries {
        if matches!(
            entry.kind,
            Kind::Gather | Kind::Phi | Kind::Insert | Kind::Reduction
        ) {
            continue;
        }
        let gathered = entry
            .operands
            .iter()
            .filter(|child| tree.entries[**child].kind == Kind::Gather)
            .flat_map(|child| tree.entries[*child].lanes.iter())
            .filter_map(|lane| match lane {
                Operand::Value(value) => match function.value(*value).def {
                    ValueDef::Instruction(def) => Some(def),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        bundles.push((entry.insts.clone(), gathered));
    }
    if !crate::slpschedule::acyclic(function, home, &bundles, &tree.pure_calls) {
        llrm_support::debug!("slp", "phi tree: the bundles wait on each other");
        return false;
    }
    true
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
    // The stores move to the last: nothing else between them touches their
    // bytes.
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
            // The loads move to the first, before every store of the window: a
            // load that stands after a store it may read the bytes of would
            // read the old ones (`BlockScheduling`'s memory dependence,
            // `BatchAA`).
            if block == seed && last > first_store {
                for load in entry.insts.iter().filter(|load| at(load) > first_store) {
                    let mut earlier =
                        StoreSeparation::new(unit, root.insts.iter().copied().filter(|store| at(store) < at(load)));
                    if !earlier.clear(function, *load) {
                        llrm_support::debug!("slp", "tree: a load stands after a store it may read");
                        return false;
                    }
                }
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

    /// Whether `value` is loop invariant in the innermost loop of `block`.
    fn loop_invariant(
        &self,
        function: &Function,
        block: BlockId,
        value: Operand,
    ) -> bool {
        self.cx.loops.get_loop_for(block).is_some_and(|one| one.is_loop_invariant(function, value))
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
        Kind::Store | Kind::Reduction => {
            let block = function.parent(entry.insts[0]).expect("a placed store");
            scaler.scale_at(function, block, &[])
        }
        // `findInnermostNonInvariantLoop` over the inserts themselves: they
        // stand in the loop.
        Kind::Insert => {
            let block = function.parent(entry.insts[0]).expect("a placed insert");
            let inserts: Vec<Operand> = entry.insts.iter().map(|insert| result_of(function, *insert)).collect();
            scaler.scale_at(function, block, &inserts)
        }
        _ => {
            let block = function.parent(entry.insts[0]).expect("a placed bundle");
            scaler.scale_at(function, block, &entry.lanes)
        }
    }
}

/// `getTreeCost`: the entries' vector costs less their scalars', and the
/// extracts of the lanes used outside. `None`: a price the target lacks.
/// `getSLPCostKind`: code size at `optsize`, else reciprocal throughput.
pub(crate) fn cost_kind(function: &Function) -> CostKind {
    if function.optimizes_for_size() { CostKind::CodeSize } else { CostKind::RecipThroughput }
}

fn cost_of(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
    store_align: u64,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<i64> {
    let costs = entry_costs(unit, function, tree, made, store_align, tti, scaler)?;
    Some(costs.iter().map(|(priced, extracts)| priced + extracts).sum())
}

/// Each entry's cost (the vector less the scalars, by the entry's scale) and
/// the extracts of its lanes some user outside the tree reads.
fn entry_costs(
    unit: &MemUnit,
    function: &Function,
    tree: &Tree,
    made: &IdMap<InstId, usize>,
    store_align: u64,
    tti: &dyn Tti,
    scaler: &Scaler,
) -> Option<Vec<(i64, i64)>> {
    let kind = cost_kind(function);
    let context = unit.context;
    let mut costs: Vec<(i64, i64)> = Vec::with_capacity(tree.entries.len());
    for (at, entry) in tree.entries.iter().enumerate() {
        let Some(priced) = entry_cost(unit, function, tree, entry, store_align, tti) else {
            llrm_support::debug!("slp", "entry {:?} x{}: not priced", entry.kind, entry.lanes.len());
            return None;
        };
        let scale = entry_scale(function, tree, at, scaler) as i64;
        llrm_support::debug!(
            "slp",
            "  #{at} {:?} x{} w{} reuse{:?} ops{:?}: {priced} * {scale}",
            entry.kind,
            entry.lanes.len(),
            entry.width(),
            entry.reuse,
            entry.operands
        );
        let mut here = (priced * scale, 0i64);
        // The lanes some user outside the tree reads are extracted.
        if !matches!(
            entry.kind,
            Kind::Gather | Kind::Store | Kind::Insert | Kind::Reduction
        ) {
            let lane = lane_ty_of(context, entry.lane_ty)?;
            let vector = VectorTy { lane, count: entry.width() as u32 };
            for (index, inst) in entry.insts.iter().enumerate() {
                let Some(result) = function.instruction(*inst).result else { continue };
                // The first user outside fixes the scale
                // (`ExtractCostCalculated`).
                if let Some(used) = function.users(result).iter().find(|used| !made.contains_key(&used.user)) {
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
                    let block = function.parent(used.user)?;
                    let scale = scaler.scale_at(function, block, &[Operand::Value(result)]);
                    here.1 += extract.valid.then_some(extract.value)? * scale as i64;
                }
            }
        }
        costs.push(here);
    }
    Some(costs)
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
    let kind = cost_kind(function);
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
                // `getEntryCost`'s `Op2Info`: the second operands of the lanes
                // as constants, and as powers of two where every one is.
                let second = |inst: InstId| {
                    let operand = function.instruction(inst).operands[1];
                    let Operand::Constant(constant) = operand else { return None };
                    let ConstantKind::Int(value) = context.get(constant).kind else { return None };
                    let bits = lane_bits(context, entry.lane_ty);
                    Some(llrm_mir::tti::OperandInfo::constant(value as i128, bits))
                };
                let lanes: Option<Vec<_>> = entry.insts.iter().map(|inst| second(*inst)).collect();
                let vector_second = match lanes {
                    Some(lanes) => llrm_mir::tti::OperandInfo {
                        constant: true,
                        power_of_two: lanes.iter().all(|one| one.power_of_two),
                        negated_power_of_two: lanes.iter().all(|one| one.negated_power_of_two),
                        non_uniform: lanes.iter().any(|one| one.value != lanes[0].value),
                        value: None,
                    },
                    None => llrm_mir::tti::OperandInfo::ANY,
                };
                let scalar_second = second(entry.insts[0]).unwrap_or(llrm_mir::tti::OperandInfo::ANY);
                let wide = valid(tti.vector_arithmetic_cost_op2(arith_of(*op), vector, vector_second, kind)?)?;
                wide - n * valid(tti.arithmetic_cost(arith_of(*op), lane, kind, Default::default(), scalar_second))?
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
            Kind::Gather => slpgather::gather_cost(tti, kind, lane, &entry.lanes, &entry.reuse)?,
            // `getEntryCost`, `Instruction::PHI`: the vector phi costs what the
            // scalar ones did.
            Kind::Phi => 0,
            Kind::Insert => crate::slpinsert::cost(context, function, tti, &entry.insts, entry.insert.as_ref()?)?,
            Kind::Reduction => crate::slpreduction::tree_reduction_cost(
                tti,
                kind,
                tree.reduction.as_ref()?.op,
                lane,
                entry.lanes.len(),
            )?,
        };
        // The lanes that repeat are read through a shuffle of the bundle.
        let reuse_cost = match entry.kind {
            Kind::Gather | Kind::Store | Kind::Insert | Kind::Reduction => 0,
            _ => slpgather::shuffle_cost(tti, kind, VectorTy { lane, count: entry.width() as u32 }, &entry.reuse)?,
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

    fn put(
        &mut self,
        function: &mut Function,
        place: Where,
        made: InstId,
    ) {
        match place {
            Where::After(anchor) => self.after(function, anchor, made),
            // Before the block's terminator: the ones made earlier stand
            // before the ones made later.
            Where::End(terminator) => function.insert(made, Position::Before(terminator)).expect("a block's end"),
        }
    }
}

/// Where a gather is built: after an instruction, or at the end of a block.
#[derive(Clone, Copy)]
enum Where {
    After(InstId),
    End(InstId),
}

/// The last phi of `block`: where the instructions that read the vector phis
/// begin.
fn last_phi(
    function: &Function,
    block: BlockId,
) -> InstId {
    function
        .iter_block(block)
        .take_while(|inst| function.instruction(*inst).opcode == Opcode::Phi)
        .last()
        .expect("a block with a phi")
}

/// What the attempts at one block read of a function that does not change
/// between them: where each instruction stands, the instructions that may stand
/// anywhere their operands do, and the divides among them. `BlockScheduling`
/// keeps a block's data across the bundles it tries; an attempt that made no
/// tree leaves the function as it was, so these stand until one is emitted.
pub(crate) struct BlockFacts {
    pub(crate) position: IdMap<InstId, usize>,
    pub(crate) pure_calls: IdSet<InstId>,
    pub(crate) divides: IdSet<InstId>,
}

impl BlockFacts {
    pub(crate) fn of(
        unit: &MemUnit,
        function: &Function,
        block: BlockId,
    ) -> Self {
        Self {
            position: positions(function),
            pure_calls: pure_instructions(unit, function, block),
            divides: crate::slpschedule::safe_divides(unit.context, function, block),
        }
    }
}

/// Where each instruction stands in its block.
pub(crate) fn positions(function: &Function) -> IdMap<InstId, usize> {
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
    let mut phis: Vec<(usize, InstId)> = Vec::new();
    let mut order: Vec<usize> = Vec::new();
    fn visit(
        tree: &Tree,
        at: usize,
        order: &mut Vec<usize>,
    ) {
        // A vector phi is made before its operands: they read it in turn.
        if tree.entries[at].kind == Kind::Phi {
            if order.contains(&at) {
                return;
            }
            order.push(at);
        }
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
        // A vector phi's operands are made after it.
        let operands: Vec<Operand> = if entry.kind == Kind::Phi {
            Vec::new()
        } else {
            entry.operands.iter().map(|child| vectors[*child].expect("an operand made first")).collect()
        };
        let made = match &entry.kind {
            Kind::Extract(source) => Some(reused(context, function, &mut placer, last_of(entry), entry, *source)),
            Kind::Gather => {
                // Built right before the bundle that reads it.
                let user = tree.entries.iter().position(|one| one.operands.contains(&at)).expect("a user");
                let place = if tree.entries[user].kind == Kind::Phi {
                    // A phi's operand is made at the end of its edge's block.
                    let edge = tree.entries[user].operands.iter().position(|one| *one == at).expect("an operand");
                    let block = tree.entries[user].blocks[edge];
                    Where::End(function.terminator(block).expect("a terminated block"))
                } else {
                    // A user's vector stands after its last scalar: the gather
                    // stands there too, before the user, after every lane it
                    // reads.
                    Where::After(last_of(&tree.entries[user]))
                };
                Some(build_vector(context, function, &mut placer, place, entry))
            }
            Kind::Phi => {
                let home = function.parent(entry.insts[0]).expect("a placed phi");
                let first = function.iter_block(home).next().expect("a block");
                let phi = function.create_instruction(Opcode::Phi, vector_ty, Vec::new(), Flags::default(), None);
                function.insert(phi, Position::Before(first)).expect("the phis' block");
                phis.push((at, phi));
                let value = result_of(function, phi);
                Some(reused(context, function, &mut placer, last_phi(function, home), entry, value))
            }
            Kind::Store | Kind::Insert | Kind::Reduction => None,
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
                    Kind::Store | Kind::Insert | Kind::Reduction | Kind::Gather | Kind::Extract(_) | Kind::Phi => {
                        unreachable!()
                    }
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
    // The vector phis' operands, now that the vectors are made.
    for (at, phi) in &phis {
        let entry = &tree.entries[*at];
        let mut arms = Vec::new();
        for (child, block) in entry.operands.iter().zip(&entry.blocks) {
            arms.push(vectors[*child].expect("an operand made"));
            arms.push(Operand::Block(*block));
        }
        function.set_operands(*phi, arms);
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
    // The reduction: the root vector reduced where the chain's root stood.
    if root.kind == Kind::Reduction {
        let info = tree.reduction.as_ref().expect("a reduction root");
        let vector = vectors[root.operands[0]].expect("the reduced vector");
        let width = tree.entries[root.operands[0]].width();
        crate::slpreduction::emit_tree_reduction(context, declared, function, info, vector, width);
    }
    // The chain's vector: the bundle's vector shuffled into what the chain's
    // first insert inserts into, read where the chain's last insert was.
    if root.kind == Kind::Insert {
        let info = root.insert.as_ref().expect("an insert root");
        let last = crate::slpinsert::last_insert(function, &root.insts);
        let child = vectors[root.operands[0]].expect("the inserted vector");
        let non_poisonous = !constant_of(child)
            && llrm_mir::valuetracking::guarantee::is_guaranteed_not_to_be_poison(
                &llrm_mir::valuetracking::Query::bare(context, function, layout),
                child,
                0,
            );
        let lane_ty = root.lane_ty;
        let mut shuffle = |context: &mut Context, function: &mut Function, mask: Vec<i32>, a: Operand, b: Operand| {
            let ty = vector_of(context, lane_ty, mask.len());
            let made = function.create_instruction(Opcode::ShuffleVector(mask), ty, vec![a, b], Flags::default(), None);
            function.insert(made, Position::Before(last)).expect("the chain's block");
            result_of(function, made)
        };
        let value =
            crate::slpinsert::placed_vector(context, function, non_poisonous, &root.insts, info, child, &mut shuffle);
        let result = function.instruction(last).result.expect("a value");
        let users: Vec<(InstId, u32)> = function.users(result).iter().map(|used| (used.user, used.index)).collect();
        for (user, at) in users {
            let mut operands = function.instruction(user).operands.clone();
            operands[at as usize] = value;
            function.set_operands(user, operands);
        }
    }
    // The lanes some user outside the tree reads.
    let tree_insts: Vec<InstId> = tree.entries.iter().flat_map(|entry| entry.insts.iter().copied()).collect();
    let i32_ty = context.types.int(64);
    for (at, entry) in tree.entries.iter().enumerate() {
        if matches!(
            entry.kind,
            Kind::Gather | Kind::Store | Kind::Insert | Kind::Reduction | Kind::Extract(_)
        ) {
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
            // Among phis nothing else may stand: after the last of them.
            let anchor = match function.instruction(defined).opcode {
                Opcode::Phi => last_phi(function, function.parent(defined).expect("a placed phi")),
                _ => defined,
            };
            function.insert(extract, Position::After(anchor)).expect("the vector's block");
            let value = result_of(function, extract);
            for (user, operand) in outside {
                let mut operands = function.instruction(user).operands.clone();
                operands[operand as usize] = value;
                function.set_operands(user, operands);
            }
        }
    }
    // The scalars of a tree of phis read each other around the loop: the phis
    // let go first, then the rest go last to first, the phis at the end.
    if tree.entries[0].kind == Kind::Phi {
        let mut scalars: Vec<InstId> = tree_insts;
        scalars.sort_by_key(|inst| std::cmp::Reverse(position[inst]));
        let phi_insts: Vec<InstId> =
            scalars.iter().copied().filter(|inst| function.instruction(*inst).opcode == Opcode::Phi).collect();
        for inst in &phi_insts {
            let ty = function.instruction(*inst).ty;
            let poison = Operand::Constant(context.constant(Constant { ty, kind: ConstantKind::Poison }));
            let arms: Vec<Operand> = function
                .instruction(*inst)
                .operands
                .iter()
                .map(|operand| if matches!(operand, Operand::Block(_)) { *operand } else { poison })
                .collect();
            function.set_operands(*inst, arms);
        }
        let (phis, others): (Vec<InstId>, Vec<InstId>) =
            scalars.into_iter().partition(|inst| function.instruction(*inst).opcode == Opcode::Phi);
        for inst in others.into_iter().chain(phis) {
            function.erase(inst).expect("a tree scalar");
        }
        reschedule(function, tree, &position);
        return;
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
    // The vectors stand where the block's order and what they read allow.
    if tree.entries[0].kind == Kind::Insert || tree.reschedule {
        reschedule(function, tree, &position);
    }
    let _ = ValueId(0);
}

/// The block's instructions in the order its bundles' vectors need
/// (`BlockScheduling`).
fn reschedule(
    function: &mut Function,
    tree: &Tree,
    position: &IdMap<InstId, usize>,
) {
    let mut pure_calls = tree.pure_calls.clone();
    let home = tree.block;
    pure_calls.extend(
        function
            .iter_block(home)
            .filter(
                |inst| matches!(function.instruction(*inst).opcode, Opcode::Call(_)) && !position.contains_key(inst),
            ),
    );
    // A vector division is made only of scalar ones `pure_calls` has: it
    // cannot trap either, so it is not a write.
    pure_calls.extend(function.iter_block(home).filter(|inst| {
        !position.contains_key(inst)
            && matches!(
                function.instruction(*inst).opcode,
                Opcode::Binary(
                    llrm_mir::opcode::BinaryOp::UDiv
                        | llrm_mir::opcode::BinaryOp::SDiv
                        | llrm_mir::opcode::BinaryOp::URem
                        | llrm_mir::opcode::BinaryOp::SRem
                )
            )
    }));
    let original: IdMap<InstId, usize> =
        function.iter_block(home).filter_map(|inst| position.get(&inst).map(|at| (inst, *at))).collect();
    let scheduled = crate::slpschedule::schedule(function, home, &pure_calls, &original);
    debug_assert!(scheduled, "a checked tree is schedulable");
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
    place: Where,
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
        placer.put(function, place, made);
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
