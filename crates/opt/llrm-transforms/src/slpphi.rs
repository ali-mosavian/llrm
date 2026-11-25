//! `SLPVectorizer`'s phi seeds: the phis of a block that take alike values are
//! one vector phi when the tree of the values they take pays.
//!
//! # How it works
//! `vectorizeChainsInBlock` (l.35365) gathers the block's phis of a type the
//! vectors hold. Each is mapped to the values that reach it through chains of
//! phis (`PHIToOpcodes`) and they are sorted by type, size, and those values:
//! instructions first, by block and opcode, then constants, then the rest
//! (`PHICompare`). `tryToVectorizeSequence` (l.34805) cuts the sorted list
//! into runs of compatible phis (`AreCompatiblePHIs`, same type and alike
//! values), and gives each run, and the leftovers of runs too short to fill a
//! register, to `tryToVectorizeList` (l.30044): windows of the widest factor
//! down to the narrowest, a window whose tree costs less than the
//! `slp-threshold` of 0 being made. nbody's sixteen loop-carried doubles
//! become four `<2 x double>` phis.
//!
//! DEVIATE SLP6 (owner arm64, exit 2026-12-01): `slptree` states the trees of
//! phis it takes; the phis' windows are not reordered (`reorderTopToBottom`).
//!
//! # Runs
//! In `slpvectorize`, before the store chains (`vectorizeChainsInBlock`
//! looks at the phis first).
//!
//! # Used by
//! `slpvectorize`.
//!
//! # Example
//! Sixteen loop-carried doubles in a loop header become `<2 x double>` phis
//! when the trees of their values pay (the nbody test).
//!
//! ```text
//! %a = phi double [ 0.0, %entry ], [ %a1, %loop ]
//! %b = phi double [ 0.0, %entry ], [ %b1, %loop ]
//!         =>
//! %v = phi <2 x double> [ zeroinitializer, %entry ], [ %v1, %loop ]
//! ```

use std::cmp::Ordering;
use std::collections::HashSet;

use llrm_analysis::memory::Unit as MemUnit;
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::module::{BlockId, Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Opcode};
use llrm_mir::passes::{Declared, Outer};
use llrm_mir::tti::Tti;
use llrm_mir::types::{FloatKind, Type, TypeId};

/// `MaxPHINumOperands`.
const MAX_PHI_OPERANDS: usize = 128;

/// What `getSameOpcode` reads of an instruction: the same operation on the
/// same type, and for a call the same callee.
#[derive(Clone, PartialEq)]
struct Shape {
    ty: TypeId,
    opcode: Opcode,
    callee: Option<Operand>,
}

/// A value that reaches a phi, as `PHICompare` and `AreCompatiblePHIs` read it.
#[derive(Clone, PartialEq)]
enum Reaching {
    Undef,
    Constant,
    /// An argument.
    Other(u32),
    Instruction {
        /// Where the instruction's block stands in the layout (the order of
        /// the dominator tree's DFS numbers).
        block: usize,
        number: u32,
        shape: Shape,
    },
}

/// A phi and the values that reach it through chains of phis
/// (`PHIToOpcodes`).
struct Seed {
    phi: InstId,
    ty: TypeId,
    values: Vec<Reaching>,
}

/// LLVM's `Instruction::getOpcode()` numbering, which `PHICompare` orders by.
fn opcode_number(opcode: &Opcode) -> u32 {
    match opcode {
        Opcode::FNeg => 12,
        Opcode::Binary(op) => match op {
            BinaryOp::Add => 13,
            BinaryOp::FAdd => 14,
            BinaryOp::Sub => 15,
            BinaryOp::FSub => 16,
            BinaryOp::Mul => 17,
            BinaryOp::FMul => 18,
            BinaryOp::UDiv => 19,
            BinaryOp::SDiv => 20,
            BinaryOp::FDiv => 21,
            BinaryOp::URem => 22,
            BinaryOp::SRem => 23,
            BinaryOp::FRem => 24,
            BinaryOp::Shl => 25,
            BinaryOp::LShr => 26,
            BinaryOp::AShr => 27,
            BinaryOp::And => 28,
            BinaryOp::Or => 29,
            BinaryOp::Xor => 30,
        },
        Opcode::Load { .. } => 32,
        Opcode::Store { .. } => 33,
        Opcode::GetElementPtr { .. } => 34,
        Opcode::Cast(op) => match op {
            CastOp::Trunc => 38,
            CastOp::ZExt => 39,
            CastOp::SExt => 40,
            CastOp::FPToUI => 41,
            CastOp::FPToSI => 42,
            CastOp::UIToFP => 43,
            CastOp::SIToFP => 44,
            CastOp::FPTrunc => 45,
            CastOp::FPExt => 46,
            _ => 47,
        },
        Opcode::ICmp(_) => 53,
        Opcode::FCmp(_) => 54,
        Opcode::Phi => 55,
        Opcode::Call(_) => 56,
        Opcode::Select => 57,
        Opcode::ExtractElement => 61,
        Opcode::InsertElement => 62,
        Opcode::ShuffleVector(_) => 63,
        _ => 100,
    }
}

/// `AreCompatiblePHIs(VL, V1)`: `next` joins the run whose last is `last`.
fn compatible(
    last: Option<&Seed>,
    next: &Seed,
) -> bool {
    let Some(last) = last else { return true };
    if last.phi == next.phi {
        return true;
    }
    if last.ty != next.ty || last.values.len() != next.values.len() {
        return false;
    }
    for (a, b) in last.values.iter().zip(&next.values) {
        match (a, b) {
            (Reaching::Undef, _) | (_, Reaching::Undef) => {}
            (
                Reaching::Instruction { block: b1, shape: s1, .. },
                Reaching::Instruction { block: b2, shape: s2, .. },
            ) => {
                if b1 != b2 || s1 != s2 {
                    return false;
                }
            }
            (Reaching::Constant, Reaching::Constant) => {}
            (x, y) => {
                if x != y {
                    return false;
                }
            }
        }
    }
    true
}

fn rank(
    context: &Context,
    ty: TypeId,
) -> (u32, u32) {
    match context.types.get(ty) {
        Type::Float(FloatKind::Float) => (2, 32),
        Type::Float(FloatKind::Double) => (3, 64),
        Type::Int(bits) => (13, u32::from(*bits)),
        _ => (99, 0),
    }
}

/// `PHICompare`: whether `a` sorts before `b`.
fn less(
    context: &Context,
    a: &Seed,
    b: &Seed,
) -> bool {
    if a.phi == b.phi {
        return false;
    }
    if rank(context, a.ty) != rank(context, b.ty) {
        return rank(context, a.ty) < rank(context, b.ty);
    }
    if a.values.len() != b.values.len() {
        return a.values.len() < b.values.len();
    }
    for (x, y) in a.values.iter().zip(&b.values) {
        // Instructions come first.
        match (x, y) {
            (
                Reaching::Instruction { block: b1, number: o1, .. },
                Reaching::Instruction { block: b2, number: o2, .. },
            ) => {
                if b1 != b2 {
                    return b1 < b2;
                }
                if o1 == o2 {
                    continue;
                }
                return o1 < o2;
            }
            (Reaching::Instruction { .. }, _) => return true,
            (_, Reaching::Instruction { .. }) => return false,
            _ => {}
        }
        // Non-undef constants next.
        match (x, y) {
            (Reaching::Constant, Reaching::Constant) => continue,
            (Reaching::Constant, _) => return true,
            (_, Reaching::Constant) => return false,
            _ => {}
        }
        match (x, y) {
            (Reaching::Other(i), Reaching::Other(j)) => match i.cmp(j) {
                Ordering::Equal => continue,
                Ordering::Less => return true,
                Ordering::Greater => return false,
            },
            (Reaching::Other(_), _) => return true,
            (_, Reaching::Other(_)) => return false,
            // Undefs come last.
            _ => {}
        }
    }
    false
}

/// The values that reach `phi` through chains of phis, as
/// `vectorizeChainsInBlock` finds them.
fn reaching(
    context: &Context,
    function: &Function,
    phi: InstId,
) -> Vec<Reaching> {
    let mut values = Vec::new();
    let mut nodes = vec![phi];
    let mut visited: IdSet<InstId> = IdSet::new();
    while let Some(node) = nodes.pop() {
        if !visited.insert(node) {
            continue;
        }
        for pair in function.instruction(node).operands.chunks(2) {
            let value = pair[0];
            match value {
                Operand::Value(id) => match function.value(id).def {
                    ValueDef::Instruction(def) if function.instruction(def).opcode == Opcode::Phi => nodes.push(def),
                    ValueDef::Instruction(def) => {
                        let data = function.instruction(def);
                        let block = function.parent(def);
                        values.push(Reaching::Instruction {
                            block: function.layout().iter().position(|one| Some(*one) == block).unwrap_or(usize::MAX),
                            number: opcode_number(&data.opcode),
                            shape: Shape {
                                ty: data.ty,
                                opcode: data.opcode.clone(),
                                callee: matches!(data.opcode, Opcode::Call(_))
                                    .then(|| data.operands.last().copied())
                                    .flatten(),
                            },
                        });
                    }
                    ValueDef::Argument(_) => values.push(Reaching::Other(id.0)),
                },
                Operand::Constant(id) => {
                    values.push(if matches!(
                        context.get(id).kind,
                        ConstantKind::Undef | ConstantKind::Poison
                    ) {
                        Reaching::Undef
                    } else {
                        Reaching::Constant
                    })
                }
                _ => values.push(Reaching::Other(u32::MAX)),
            }
        }
    }
    values
}

/// The phis of `block` whose type the vectors hold, not yet looked at.
fn incoming(
    context: &Context,
    function: &Function,
    block: BlockId,
    visited: &IdSet<InstId>,
) -> Vec<Seed> {
    let mut seeds = Vec::new();
    for inst in function.iter_block(block) {
        let data = function.instruction(inst);
        if data.opcode != Opcode::Phi || data.operands.len() / 2 > MAX_PHI_OPERANDS {
            break;
        }
        if visited.contains(&inst) || bits_of(context, data.ty) < 8 {
            continue;
        }
        seeds.push(Seed { phi: inst, ty: data.ty, values: reaching(context, function, inst) });
    }
    seeds
}

fn bits_of(
    context: &Context,
    ty: TypeId,
) -> u32 {
    match context.types.get(ty) {
        Type::Float(FloatKind::Float) => 32,
        Type::Float(FloatKind::Double) => 64,
        Type::Int(bits @ (8 | 16 | 32 | 64)) => u32::from(*bits),
        _ => 0,
    }
}

/// What a window is made one vector of.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Roots {
    Phis,
    Inserts,
    /// `vectorizeSeeds`: instructions that are not roots of any other kind,
    /// each read by one user (`StandaloneSeeds`).
    Seeds,
    /// `vectorizeNonVectorizableInsts`: the value operands of the stores of a
    /// block, bundled by opcode and type.
    Operands,
}

/// What a run of `try_list` needs: what the function is vectorized with.
pub(crate) struct Session<'a> {
    pub(crate) context: &'a mut Context,
    layout: &'a DataLayout,
    declared: &'a mut Declared,
    outer: &'a Outer,
    tti: &'a dyn Tti,
    block: BlockId,
    roots: Roots,
    /// `isAnalyzedBundle`: the windows that did not pay.
    analyzed: HashSet<Vec<InstId>>,
    /// `positions` of the function and the instructions of the block that stand
    /// anywhere their operands do, kept until a tree is emitted:
    /// `BlockScheduling` keeps its block's data across the bundles it
    /// tries.
    positions: Option<crate::slptree::BlockFacts>,
    /// The expressions the attempts ask, made once and dropped when a tree is
    /// emitted (a failed attempt leaves the function as it was).
    scev: Option<llrm_analysis::scev::analysis::ScevSession>,
}

impl<'a> Session<'a> {
    pub(crate) fn new(
        context: &'a mut Context,
        layout: &'a DataLayout,
        declared: &'a mut Declared,
        outer: &'a Outer,
        tti: &'a dyn Tti,
        block: BlockId,
        roots: Roots,
    ) -> Self {
        Self {
            context,
            layout,
            declared,
            outer,
            tti,
            block,
            roots,
            analyzed: HashSet::new(),
            positions: None,
            scev: None,
        }
    }

    pub(crate) fn deleted_inst(
        function: &Function,
        inst: InstId,
    ) -> bool {
        Self::deleted(function, inst)
    }

    /// The type of one lane of a root: a phi's, the element an insert sets.
    fn lane_type(
        &self,
        function: &Function,
        root: InstId,
    ) -> Option<TypeId> {
        match self.roots {
            Roots::Phis | Roots::Seeds | Roots::Operands => Some(function.instruction(root).ty),
            Roots::Inserts => function.operand_type(self.context, function.instruction(root).operands[1]),
        }
    }

    fn deleted(
        function: &Function,
        inst: InstId,
    ) -> bool {
        function.is_erased(inst) || function.parent(inst).is_none()
    }

    /// Whether `inst` reads the result of any of `taken`.
    fn reads_any(
        function: &Function,
        inst: InstId,
        taken: &[InstId],
    ) -> bool {
        function
            .instruction(inst)
            .operands
            .iter()
            .any(
                |operand| match operand {
                    Operand::Value(value) => matches!(
                        function.value(*value).def,
                        ValueDef::Instruction(def) if taken.contains(&def)
                    ),
                    _ => false,
                },
            )
    }

    /// `Instruction::isIdenticalTo`: the same operation on the same operands.
    fn identical(
        function: &Function,
        a: InstId,
        b: InstId,
    ) -> bool {
        let (a, b) = (function.instruction(a), function.instruction(b));
        a.opcode == b.opcode && a.ty == b.ty && a.operands == b.operands && a.flags == b.flags
    }

    /// The tree of `window`, if it pays, made: whether it was.
    fn attempt(
        &mut self,
        function: &mut Function,
        window: &[InstId],
    ) -> bool {
        let mut facts = self.positions.take();
        let mut session = match self.scev.take() {
            Some(session) => session,
            None => llrm_analysis::scev::analysis::ScevSession::of(self.context, function, self.outer),
        };
        let tree = {
            let unit = MemUnit::within(self.context, self.layout, function, self.outer);
            let facts_now = &*facts.get_or_insert_with(|| crate::slptree::BlockFacts::of(&unit, function, self.block));
            session.ask_within(
                self.context,
                self.layout,
                &self.outer.globals,
                &self.outer.metadata,
                function,
                |se, cx| {
                    let scaler = crate::slptree::Scaler::new(se, cx);
                    match self.roots {
                        Roots::Phis => {
                            crate::slptree::try_phis(&unit, function, self.block, window, facts_now, self.tti, &scaler)
                        }
                        Roots::Inserts => crate::slptree::try_inserts(
                            &unit, function, self.block, window, facts_now, self.tti, &scaler,
                        ),
                        Roots::Seeds | Roots::Operands => {
                            crate::slptree::try_seeds(&unit, function, self.block, window, facts_now, self.tti, &scaler)
                        }
                    }
                },
            )
        };
        match tree {
            Some(tree) => {
                crate::slptree::emit(self.context, self.layout, self.declared, function, &tree, None);
                true
            }
            None => {
                self.positions = facts;
                self.scev = Some(session);
                false
            }
        }
    }

    /// `tryToVectorizeList`.
    pub(crate) fn try_list(
        &mut self,
        function: &mut Function,
        list: &[InstId],
        max_vf_only: bool,
    ) -> bool {
        if list.len() < 2 {
            return false;
        }
        let Some(ty) = self.lane_type(function, list[0]) else { return false };
        let bits = bits_of(self.context, ty) as usize;
        if bits == 0 || list.iter().any(|root| self.lane_type(function, *root) != Some(ty)) {
            return false;
        }
        let register = self.tti.vector_register_bits() as usize;
        let min_vf = ((self.tti.min_vector_register_bits() as usize) / bits).max(2);
        let opcode = match self.roots {
            Roots::Phis => Opcode::Phi,
            Roots::Inserts => Opcode::InsertElement,
            Roots::Seeds | Roots::Operands => function.instruction(list[0]).opcode.clone(),
        };
        // `getMaximumVF` caps a list at what the target says; where it says
        // nothing, the list as long as its vectors fill whole registers (a
        // `<4 x i64>` is two): the phis and inserts keep to one register.
        let unbounded = self.roots == Roots::Operands;
        let widest = match self.tti.maximum_vf(bits as u32, opcode) {
            0 if unbounded => usize::MAX / 2,
            0 => register / bits,
            limit => limit as usize,
        };
        let floor_pow2 = |n: usize| if n == 0 { 0 } else { 1usize << (usize::BITS - 1 - n.leading_zeros()) };
        let mut max_vf = floor_pow2(list.len()).max(min_vf).min(widest);
        // Standalone seeds only need one register worth of lanes.
        let standalone = self.roots == Roots::Seeds;
        if standalone {
            max_vf = max_vf.min((register / bits).max(min_vf));
        }
        if max_vf < 2 {
            return false;
        }
        let mut changed = false;
        let (mut next, max_inst) = (0usize, list.len());
        let mut vf = max_vf;
        while next + 1 < max_inst && vf >= min_vf {
            // No actual vectorization where the vector is as many registers
            // as it has lanes (`getNumberOfParts == VF`); the phis and inserts
            // also stay within one register.
            let parts = (vf * bits).div_ceil(register.max(1));
            if (unbounded && parts == vf) || (!unbounded && vf * bits > register) {
                vf = floor_pow2(vf - 1);
                continue;
            }
            let mut at = next;
            while at < max_inst {
                let actual = (max_inst - at).min(vf);
                if !actual.is_power_of_two() {
                    at += 1;
                    continue;
                }
                if max_vf_only && actual < max_vf {
                    break;
                }
                if (vf > min_vf && actual < vf) || (vf == min_vf && actual < 2) {
                    break;
                }
                let mut window: Vec<InstId> = Vec::with_capacity(actual);
                let mut dependent_skipped = false;
                for root in list[at..].iter().copied().filter(|root| !Self::deleted(function, *root)) {
                    // The lanes of a standalone seed cannot depend on each
                    // other: the seed fed by one already taken waits for the
                    // next window.
                    if standalone && Self::reads_any(function, root, &window) {
                        dependent_skipped = true;
                        continue;
                    }
                    window.push(root);
                    if window.len() == actual {
                        break;
                    }
                }
                if window.len() != actual {
                    if dependent_skipped {
                        at += 1;
                        continue;
                    }
                    break;
                }
                // Identical lanes compute one value: the vector only copies it.
                let identical =
                    standalone && window[1..].iter().all(|lane| Self::identical(function, window[0], *lane));
                if self.analyzed.contains(&window) || identical {
                    if standalone {
                        at += actual - 1;
                    }
                    at += 1;
                    continue;
                }
                if self.attempt(function, &window) {
                    at += vf - 1;
                    next = at + 1;
                    changed = true;
                } else {
                    self.analyzed.insert(window);
                    if standalone {
                        at += actual - 1;
                    }
                }
                at += 1;
            }
            vf = floor_pow2(vf - 1);
            if vf == 0 {
                break;
            }
        }
        changed
    }

    /// `tryToVectorizeSequence` over the sorted phis of the block.
    fn sequence(
        &mut self,
        function: &mut Function,
        seeds: &[Seed],
        max_vf_only: bool,
    ) -> bool {
        let mut changed = false;
        let mut candidates: Vec<usize> = Vec::new();
        let mut at = 0;
        while at < seeds.len() {
            let first = &seeds[at];
            if Self::deleted(function, first.phi) {
                at += 1;
                continue;
            }
            let mut run: Vec<usize> = Vec::new();
            let mut same = at;
            while same < seeds.len()
                && (Self::deleted(function, seeds[same].phi)
                    || compatible(run.last().map(|last| &seeds[*last]), &seeds[same]))
            {
                if !Self::deleted(function, seeds[same].phi) {
                    run.push(same);
                }
                same += 1;
            }
            let list: Vec<InstId> = run.iter().map(|seed| seeds[*seed].phi).collect();
            if list.len() > 1 && self.try_list(function, &list, true) {
                changed = true;
                candidates.retain(|seed| !Self::deleted(function, seeds[*seed].phi));
            } else {
                let bits = (bits_of(self.context, first.ty) as usize).max(1);
                let min_elements = (self.tti.vector_register_bits() as usize / bits).max(2);
                let same_type = candidates.first().is_none_or(|front| seeds[*front].ty == first.ty);
                if list.len() < min_elements && same_type {
                    candidates.extend(run.iter().copied().filter(|seed| !Self::deleted(function, seeds[*seed].phi)));
                }
            }
            // The final attempt on what several short runs left.
            let type_ends = same == seeds.len() || seeds[same].ty != first.ty;
            if candidates.len() > 1 && type_ends {
                let pooled: Vec<InstId> = candidates.iter().map(|seed| seeds[*seed].phi).collect();
                if self.try_list(function, &pooled, false) {
                    changed = true;
                } else if max_vf_only {
                    let live: Vec<usize> =
                        candidates.iter().copied().filter(|seed| !Self::deleted(function, seeds[*seed].phi)).collect();
                    let mut start = 0;
                    while start < live.len() {
                        let mut end = start;
                        while end < live.len() && compatible(Some(&seeds[live[end]]), &seeds[live[start]]) {
                            end += 1;
                        }
                        let small: Vec<InstId> = live[start..end].iter().map(|seed| seeds[*seed].phi).collect();
                        if small.len() > 1 && self.try_list(function, &small, false) {
                            changed = true;
                        }
                        start = end.max(start + 1);
                    }
                }
                candidates.clear();
            }
            at = same.max(at + 1);
        }
        changed
    }
}

/// The phis of `function` that are one vector phi, made; whether any was.
pub(crate) fn run_function(
    context: &mut Context,
    layout: &DataLayout,
    declared: &mut Declared,
    function: &mut Function,
    outer: &Outer,
    tti: &dyn Tti,
) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        let mut visited: IdSet<InstId> = IdSet::new();
        let mut session = Session::new(context, layout, declared, outer, tti, block, Roots::Phis);
        loop {
            let mut seeds = incoming(session.context, function, block, &visited);
            if seeds.len() <= 1 {
                break;
            }
            seeds.sort_by(|a, b| {
                if less(session.context, a, b) {
                    Ordering::Less
                } else if less(session.context, b, a) {
                    Ordering::Greater
                } else {
                    Ordering::Equal
                }
            });
            let vectorized = session.sequence(function, &seeds, true);
            changed |= vectorized;
            visited.extend(seeds.iter().map(|seed| seed.phi));
            if !vectorized {
                break;
            }
        }
    }
    changed
}
