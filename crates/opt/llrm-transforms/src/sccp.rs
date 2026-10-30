//! Sparse conditional constant propagation. A translation of LLVM's `SCCP.cpp`
//! (`runSCCP`) at d1106deb71cc with the rewrites of `SCCPSolver.cpp` it calls:
//! solve for constants over the executable part of the function, replace each
//! value with its constant, empty the blocks that never run, and cut the
//! edges that were never taken.
//!
//! # How it works
//! [`crate::sccpsolver::Solver`] marks the entry block executable, gives each
//! argument the state its attributes say, and runs to a fixed point; what is
//! left unknown is made overdefined and the solve resumes until nothing
//! changes. Then, per block: an unreachable block is emptied (a phi stays; the
//! rest becomes `unreachable`) and deleted last; a reachable one has each
//! value replaced by its constant, a signed operation on non-negative values
//! made unsigned, wrap flags added that the ranges prove, and a few
//! range-based simplifications. A branch or switch with an edge that was never
//! feasible loses it. The function's return range is added to its `range`
//! attribute.
//!
//! The pass is on LLVM's pipeline as `ipsccp` then `sccp`; here it is the
//! function-level one at `sccp@792`.
//!
//! ```text
//!  b0: %c = icmp eq i32 1, 1      b0: br label %b1
//!      br i1 %c, b1, b2           b1: ret i32 7
//!  b1: ret i32 7                  (b2 deleted)
//!  b2: ret i32 8
//! ```
//!
//! DEVIATEs: those of [`crate::sccpsolver`].
//!
//! # Example
//! See `sccp_tests.rs`.
//!
//! # See also
//! - Uses: [`crate::sccpsolver`], [`crate::local::change_to_unreachable`]

use llrm_mir::constantrange::{ConstantRange, NO_SIGNED_WRAP, NO_UNSIGNED_WRAP, PreferredRangeType};
use llrm_mir::context::GlobalId;
use llrm_mir::context::{Constant, ConstantId, ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::facts::{Fact, Facts};
use llrm_mir::module::{BlockId, Function, InstId, Instruction, Linkage, MetadataNode, Operand, ValueId};
use llrm_mir::opcode::{Attribute, BinaryOp, CastOp, Flags, IntPredicate, Opcode, Tail};
use llrm_mir::passes::{Analyses, FunctionPass, Outer, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuelattice::{LatticeConstant, ValueLatticeElement};
use llrm_mir::valuetracking::pattern::{self, IntrinsicId, add_like, binary, call_arguments};
use llrm_mir::valuetracking::{AssumptionCache, Query};

use crate::local;
use crate::predicateinfo::PredicateInfo;
use crate::sccpsolver::{Env, Results, Solver};

pub struct Sccp;

impl FunctionPass for Sccp {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "sccp"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("sccp")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let outer = analyses.outer().clone();
        let dt = analyses.get::<llrm_mir::passes::Dominators>(&*unit.context, unit.layout, &*unit.function);
        let changed = run_sccp(unit.context, unit.layout, &outer, unit.metadata, unit.id, &dt, unit.function);
        match (changed.anything, changed.control_flow) {
            (false, _) => PreservedAnalyses::all(),
            (true, false) => PreservedAnalyses::none()
                .preserve::<llrm_mir::passes::Dominators>()
                .preserve::<llrm_mir::passes::Loops>(),
            (true, true) => PreservedAnalyses::none(),
        }
    }
}

/// What the run changed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Changed {
    pub anything: bool,
    /// A block went or an edge was cut.
    pub control_flow: bool,
}

/// `GlobalValue::hasExactDefinition` of the function, and not `naked`
/// (`canTrackReturnsInterprocedurally`).
fn can_track_returns_interprocedurally(
    outer: &Outer,
    function: &Function,
    id: Option<GlobalId>,
) -> bool {
    let Some(id) = id else { return false };
    can_track_returns(outer.globals[id.0 as usize].linkage, function)
}

/// `canTrackReturnsInterprocedurally` of a function of linkage `linkage`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueLatticeUtils.cpp#L24>
pub(crate) fn can_track_returns(
    linkage: Linkage,
    function: &Function,
) -> bool {
    let has_flag = |name: &str| function.attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name));
    has_exact_definition(linkage, function) && !has_flag("naked")
}

/// `GlobalValue::hasExactDefinition`.
pub(crate) fn has_exact_definition(
    linkage: Linkage,
    function: &Function,
) -> bool {
    let has_flag = |name: &str| function.attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name));
    let may_be_derefined = match linkage {
        Linkage::WeakOdr | Linkage::LinkOnceOdr | Linkage::AvailableExternally => true,
        _ => {
            let interposable = has_flag("noipa")
                || matches!(
                    linkage,
                    Linkage::Weak | Linkage::LinkOnce | Linkage::Common | Linkage::ExternWeak
                );
            interposable || has_flag("nobuiltin")
        }
    };
    !function.is_declaration() && !may_be_derefined
}

/// `runSCCP`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Transforms/Scalar/SCCP.cpp#L54>
pub fn run_sccp(
    context: &mut Context,
    layout: &DataLayout,
    outer: &Outer,
    metadata: &[MetadataNode],
    id: Option<GlobalId>,
    dt: &DomTree,
    function: &mut Function,
) -> Changed {
    if function.is_declaration() {
        return Changed::default();
    }
    let tracks_returns = can_track_returns_interprocedurally(outer, function, id);
    // `addPredicateInfo`: copies of the values a branch, switch or assume
    // tells something about, for the code it holds for.
    let assumptions = AssumptionCache::of(&*context, &outer.globals, metadata, function);
    let predicates = PredicateInfo::new(&*context, &outer.globals, metadata, function, dt, &assumptions);
    // Solve for constants.
    let results = {
        let env = Env { context: &mut *context, layout, outer, metadata, id, tracks_returns, predicates: &predicates };
        let mut solver = Solver::new(env, &*function);
        loop {
            solver.solve();
            if !solver.resolved_undefs_in() {
                break;
            }
        }
        solver.into_results()
    };

    let mut changed = Changed::default();
    let mut rewrite = Rewrite::new(&*context, layout, outer, metadata, results);
    // If we decided that there are basic blocks that are dead in this
    // function, delete their contents now. Note that we cannot actually delete
    // the blocks yet, as the edges into them are still to be cut.
    let mut blocks_to_erase = Vec::new();
    for block in function.layout().to_vec() {
        if !rewrite.results.is_block_executable(block) {
            blocks_to_erase.push(block);
            changed.anything = true;
            continue;
        }
        changed.anything |= rewrite.simplify_insts_in_block(function, block);
    }
    // Remove unreachable blocks and non-feasible edges.
    for &dead in &blocks_to_erase {
        let first = function
            .iter_block(dead)
            .find(|&one| function.instruction(one).opcode != Opcode::Phi)
            .expect("a block has a terminator");
        local::change_to_unreachable(rewrite.context, function, first);
    }
    let mut new_unreachable: Option<BlockId> = None;
    for block in function.layout().to_vec() {
        let cut = rewrite.remove_non_feasible_edges(function, block, &mut new_unreachable);
        changed.anything |= cut;
        changed.control_flow |= cut;
    }
    for &dead in &blocks_to_erase {
        rewrite.delete_block(function, dead);
    }
    changed.control_flow |= !blocks_to_erase.is_empty();
    remove_ssa_copies(function, &predicates);
    changed.anything |= rewrite.infer_return_attributes(function);
    changed
}

/// `removeSSACopies`: each copy `PredicateInfo` made goes, its uses reading
/// the value again.
pub(crate) fn remove_ssa_copies(
    function: &mut Function,
    predicates: &PredicateInfo,
) {
    let mut copies: Vec<InstId> = predicates.copies().collect();
    copies.sort_by_key(|one| one.0);
    for copy in copies {
        if function.is_erased(copy) {
            continue;
        }
        let instruction = function.instruction(copy);
        let (result, source) = (instruction.result.expect("a copy has a value"), instruction.operands[0]);
        function.replace_all_uses_with(result, source);
        function.erase(copy).expect("its uses were replaced");
    }
}

/// `function`'s `inst` as the rewrites read it: without the metadata, whose
/// strings the solver's rewrites never look at and a whole-instruction clone
/// copies for every instruction they visit.
fn detached(
    function: &Function,
    inst: InstId,
) -> Instruction {
    let one = function.instruction(inst);
    Instruction {
        opcode: one.opcode.clone(),
        ty: one.ty,
        operands: one.operands.clone(),
        flags: one.flags,
        result: one.result,
        metadata: Vec::new(),
        link: (0, 0),
    }
}

/// The rewrites, over the solved function.
pub(crate) struct Rewrite<'a> {
    context: &'a Context,
    layout: &'a DataLayout,
    outer: &'a Outer,
    metadata: &'a [MetadataNode],
    pub(crate) results: Results,
    /// `InsertedValues`: values made by the rewrites, unknown to the solver.
    inserted: IdSet<ValueId>,
    /// `addToMustPreserveReturnsInFunctions`: the callees of the `musttail`
    /// calls whose result was not replaced.
    pub(crate) preserved: Vec<GlobalId>,
}

impl<'a> Rewrite<'a> {
    pub(crate) fn new(
        context: &'a Context,
        layout: &'a DataLayout,
        outer: &'a Outer,
        metadata: &'a [MetadataNode],
        results: Results,
    ) -> Self {
        Self { context, layout, outer, metadata, results, inserted: IdSet::new(), preserved: Vec::new() }
    }

    fn query<'f>(
        &'f self,
        function: &'f Function,
    ) -> Query<'f> {
        Query {
            context: self.context,
            globals: &self.outer.globals,
            metadata: self.metadata,
            function,
            layout: self.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    /// `getRange(Op, Solver, InsertedValues)`.
    fn get_range(
        &self,
        function: &Function,
        op: Operand,
    ) -> ConstantRange {
        let ty = function.operand_type(self.context, op).expect("a value or constant");
        let width = self.context.types.int_bits(ty).unwrap_or(1);
        match op {
            Operand::Constant(id) => match LatticeConstant::of(self.context, id) {
                LatticeConstant::Int { width, bits } => ConstantRange::single(width, bits),
                _ => ConstantRange::full(width),
            },
            Operand::Value(value) if self.inserted.contains(&value) => ConstantRange::full(width),
            Operand::Value(value) => self.results.lattice_value(value).as_constant_range(width, false),
            Operand::Block(_) => unreachable!("a block has no range"),
        }
    }

    /// `SCCPSolver::tryToReplaceWithConstant` of an instruction.
    fn try_to_replace_with_constant(
        &mut self,
        function: &mut Function,
        inst: InstId,
    ) -> bool {
        let Some(result) = function.instruction(inst).result else { return false };
        self.try_to_replace_value_with_constant(function, result, Some(inst))
    }

    /// `SCCPSolver::tryToReplaceWithConstant` of `result`: the value of
    /// `inst`, or an argument.
    pub(crate) fn try_to_replace_value_with_constant(
        &mut self,
        function: &mut Function,
        result: ValueId,
        inst: Option<InstId>,
    ) -> bool {
        let Some(constant) = self.results.constant_or_null(self.context, function, result) else { return false };
        // Replacing `musttail` instructions with constant breaks `musttail`
        // invariant unless the call itself can be removed.
        if let Some(inst) = inst
            && let Opcode::Call(info) | Opcode::Invoke(info) = &function.instruction(inst).opcode
            && info.tail == Tail::MustTail
            && !local::would_instruction_be_trivially_dead(
                self.context,
                self.outer.callees(),
                &self.outer.globals,
                function,
                inst,
            )
        {
            // Don't zap returns of the callee.
            if let Some(callee) = llrm_mir::memory::callee(self.context, function, inst) {
                self.preserved.push(callee);
            }
            return false;
        }
        // For pointer constants derived from PredicateInfo, the constant may
        // have different provenance. Take this into account during constant
        // pointer propagation.
        let ty = function.value(result).ty;
        if matches!(self.context.types.get(ty), Type::Pointer(_))
            && self.results.lattice_value(result).may_have_different_provenance()
        {
            let uses: Vec<_> = function.users(result).iter().map(|one| *one).collect();
            let mut changed = false;
            for at in uses {
                if local::can_replace_pointers_in_use_if_equal(
                    self.context,
                    self.layout,
                    &self.outer.globals,
                    self.outer.sizes(),
                    function,
                    at,
                    Operand::Constant(constant),
                ) {
                    function.set_operand(at.user, at.index as usize, Operand::Constant(constant));
                    changed = true;
                }
            }
            return changed;
        }
        // Replaces all of the uses of a variable with uses of the constant.
        function.replace_value(result, Operand::Constant(constant));
        true
    }

    /// `simplifyInstsInBlock`.
    pub(crate) fn simplify_insts_in_block(
        &mut self,
        function: &mut Function,
        block: BlockId,
    ) -> bool {
        let mut made_changes = false;
        let insts: Vec<InstId> = function.iter_block(block).collect();
        for inst in insts {
            if function.is_erased(inst) || function.instruction(inst).result.is_none() {
                continue;
            }
            if self.try_to_replace_with_constant(function, inst) {
                if local::is_instruction_trivially_dead(
                    self.context,
                    self.outer.callees(),
                    &self.outer.globals,
                    function,
                    inst,
                ) {
                    function.erase(inst).expect("a dead instruction has no users");
                }
                made_changes = true;
            } else if self.replace_signed_inst(function, inst) || self.refine_instruction(function, inst) {
                made_changes = true;
            } else if let Some(simpler) = self.simplify_instruction(function, inst) {
                let result = function.instruction(inst).result.expect("an instruction with a value");
                function.replace_value(result, simpler);
                function.erase(inst).expect("its uses were replaced");
                made_changes = true;
            }
        }
        made_changes
    }

    /// `replaceSignedInst`: try to replace signed instructions with their
    /// unsigned equivalent.
    fn replace_signed_inst(
        &mut self,
        function: &mut Function,
        inst: InstId,
    ) -> bool {
        let instruction = detached(function, inst);
        // Determine if a signed value is known to be >= 0.
        let is_non_negative = |this: &Self, v: Operand| this.get_range(function, v).is_all_non_negative();
        let (opcode, operands, flags) = match &instruction.opcode {
            // If the source value is not negative, this is a zext/uitofp.
            Opcode::Cast(op @ (CastOp::SExt | CastOp::SIToFP)) => {
                if !is_non_negative(self, instruction.operands[0]) {
                    return false;
                }
                let to = if *op == CastOp::SExt { CastOp::ZExt } else { CastOp::UIToFP };
                (Opcode::Cast(to), instruction.operands.clone(), Flags::NNEG)
            }
            // If the shifted value is not negative, this is a logical shift
            // right.
            Opcode::Binary(BinaryOp::AShr) => {
                if !is_non_negative(self, instruction.operands[0]) {
                    return false;
                }
                (Opcode::Binary(BinaryOp::LShr), instruction.operands.clone(), exact_only(instruction.flags))
            }
            // If both operands are not negative, this is the same as
            // udiv/urem.
            Opcode::Binary(op @ (BinaryOp::SDiv | BinaryOp::SRem)) => {
                if !is_non_negative(self, instruction.operands[0]) || !is_non_negative(self, instruction.operands[1]) {
                    return false;
                }
                let (to, flags) = if *op == BinaryOp::SDiv {
                    (BinaryOp::UDiv, exact_only(instruction.flags))
                } else {
                    (BinaryOp::URem, Flags::default())
                };
                (Opcode::Binary(to), instruction.operands.clone(), flags)
            }
            _ => return false,
        };
        // Wire up the new instruction and update state.
        let old = instruction.result.expect("an instruction with a value");
        let new = function.create_instruction(opcode, instruction.ty, operands, flags, None);
        function.insert(new, Position::Before(inst)).expect("a placed instruction");
        let new_value = function.instruction(new).result.expect("an instruction with a value");
        function.take_name(old, new_value);
        self.inserted.insert(new_value);
        function.replace_all_uses_with(old, Operand::Value(new_value));
        self.results.remove_lattice_value(old);
        function.erase(inst).expect("its uses were replaced");
        true
    }

    /// `refineInstruction`: try to use `inst`'s value range to infer flags.
    fn refine_instruction(
        &self,
        function: &mut Function,
        inst: InstId,
    ) -> bool {
        let instruction = detached(function, inst);
        let mut changed = false;
        let mut flags = instruction.flags;
        match &instruction.opcode {
            Opcode::Binary(op @ (BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)) => {
                if flags.contains(Flags::NSW) && flags.contains(Flags::NUW) {
                    return false;
                }
                let range_a = self.get_range(function, instruction.operands[0]);
                let range_b = self.get_range(function, instruction.operands[1]);
                if !flags.contains(Flags::NUW) {
                    let region = ConstantRange::make_guaranteed_no_wrap_region(*op, &range_b, NO_UNSIGNED_WRAP);
                    if region.contains_range(&range_a) {
                        flags.insert(Flags::NUW);
                        changed = true;
                    }
                }
                if !flags.contains(Flags::NSW) {
                    let region = ConstantRange::make_guaranteed_no_wrap_region(*op, &range_b, NO_SIGNED_WRAP);
                    if region.contains_range(&range_a) {
                        flags.insert(Flags::NSW);
                        changed = true;
                    }
                }
            }
            Opcode::Cast(CastOp::ZExt | CastOp::UIToFP) if !flags.contains(Flags::NNEG) => {
                if self.get_range(function, instruction.operands[0]).is_all_non_negative() {
                    flags.insert(Flags::NNEG);
                    changed = true;
                }
            }
            Opcode::Cast(CastOp::Trunc) => {
                if flags.contains(Flags::NSW) && flags.contains(Flags::NUW) {
                    return false;
                }
                let range = self.get_range(function, instruction.operands[0]);
                let dest = self.context.types.int_bits(instruction.ty).expect("an integer trunc");
                if !flags.contains(Flags::NUW) && range.get_active_bits() <= dest {
                    flags.insert(Flags::NUW);
                    changed = true;
                }
                if !flags.contains(Flags::NSW) && range.get_min_signed_bits() <= dest {
                    flags.insert(Flags::NSW);
                    changed = true;
                }
            }
            Opcode::GetElementPtr { .. } => {
                let no_unsigned_signed_wrap = flags.contains(Flags::INBOUNDS) || flags.contains(Flags::NUSW);
                if flags.contains(Flags::NUW) || !no_unsigned_signed_wrap {
                    return false;
                }
                if instruction.operands[1..].iter().all(|&index| self.get_range(function, index).is_all_non_negative())
                {
                    flags.insert(Flags::NUW);
                    changed = true;
                }
            }
            _ => {}
        }
        if changed {
            function.set_flags(inst, flags);
        }
        changed
    }

    /// `simplifyInstruction`: try to use `inst`'s value range to simplify it.
    fn simplify_instruction(
        &mut self,
        function: &mut Function,
        inst: InstId,
    ) -> Option<Operand> {
        let instruction = detached(function, inst);
        // Remove masking operations.
        if instruction.opcode == Opcode::Binary(BinaryOp::And)
            && let Some((mask, width)) = self.query(function).int_constant(instruction.operands[1])
            && is_low_bit_mask(mask, width)
        {
            let x = instruction.operands[0];
            if self.get_range(function, x).get_unsigned_max() <= mask {
                return Some(x);
            }
        }
        // Check if we can simplify [us]cmp(X, Y) to X - Y.
        if let Some(id @ (IntrinsicId::Scmp | IntrinsicId::Ucmp)) = pattern::call_intrinsic(&self.query(function), inst)
        {
            return self.simplify_cmp_intrinsic(function, inst, id == IntrinsicId::Scmp);
        }
        // Relax range checks.
        if let Opcode::ICmp(pred) = instruction.opcode {
            return self.relax_range_check(function, inst, pred);
        }
        None
    }

    /// The `CmpIntrinsic` arm of `simplifyInstruction`.
    fn simplify_cmp_intrinsic(
        &mut self,
        function: &mut Function,
        inst: InstId,
        signed: bool,
    ) -> Option<Operand> {
        let instruction = detached(function, inst);
        let args = call_arguments(&instruction).to_vec();
        let (lhs, rhs) = (args[0], args[1]);
        let ty = function.operand_type(self.context, lhs).expect("a value");
        let bit_width = self.context.types.int_bits(ty).expect("an integer operand");
        // Bail out on 1-bit comparisons.
        if bit_width == 1 {
            return None;
        }
        let l_range = self.get_range(function, lhs);
        if l_range.is_size_larger_than(3) {
            return None;
        }
        let r_range = self.get_range(function, rhs);
        if r_range.is_size_larger_than(3) {
            return None;
        }
        let one = ConstantRange::single(bit_width, 1);
        let rhs_lower = r_range.sub(&one);
        let rhs_upper = r_range.add(&one);
        let pred = if signed { IntPredicate::Sle } else { IntPredicate::Ule };
        if !rhs_lower.icmp(pred, &l_range) || !l_range.icmp(pred, &rhs_upper) {
            return None;
        }
        // IRBuilder<NoFolder>: the instruction is built as written.
        let name = instruction.result.and_then(|result| function.value(result).name.clone());
        let flags = if signed { Flags::NSW } else { Flags::default() };
        let mut sub =
            self.insert_before(function, inst, Opcode::Binary(BinaryOp::Sub), ty, vec![lhs, rhs], flags, name);
        if function.operand_type(self.context, sub) != Some(instruction.ty) {
            // CreateSExtOrTrunc
            let dest = self.context.types.int_bits(instruction.ty).expect("an integer result");
            let op = if dest > bit_width { CastOp::SExt } else { CastOp::Trunc };
            sub =
                self.insert_before(function, inst, Opcode::Cast(op), instruction.ty, vec![sub], Flags::default(), None);
        }
        Some(sub)
    }

    /// The `ICmpInst` arm of `simplifyInstruction`.
    fn relax_range_check(
        &mut self,
        function: &mut Function,
        inst: InstId,
        pred: IntPredicate,
    ) -> Option<Operand> {
        let instruction = detached(function, inst);
        let (rhs_bits, width) = self.query(function).int_constant(instruction.operands[1])?;
        let lhs = instruction.operands[0];
        let lhs_value = match lhs {
            Operand::Value(value) => value,
            _ => return None,
        };
        let one_use = function.users(lhs_value).only().is_some();
        let q = self.query(function);
        let mut matched: Option<(Operand, ConstantRange)> = None;
        if one_use && let Some((x, offset)) = add_like(&q, lhs, Flags::default()) {
            if let Some((offset, _)) = q.int_constant(offset) {
                matched = Some((x, ConstantRange::make_exact_icmp_region(pred, width, rhs_bits).subtract(offset)));
            }
        }
        // Match icmp eq/ne X & NegPow2, C
        if matched.is_none()
            && matches!(pred, IntPredicate::Eq | IntPredicate::Ne)
            && one_use
            && let Some((_, and, x, mask)) = binary(&q, lhs, BinaryOp::And)
            && let Some((mask, _)) = q.int_constant(mask)
            && is_negated_power2(mask, width)
            && count_trailing_zeros(rhs_bits, width) >= count_trailing_zeros(mask, width)
        {
            let _ = and;
            let upper = rhs_bits.wrapping_sub(mask) & mask_of(width);
            let cr = ConstantRange::new(width, rhs_bits, upper);
            matched = Some((x, if pred == IntPredicate::Eq { cr } else { cr.inverse() }));
        }
        let (x, cr) = matched?;
        let l_range = self.get_range(function, x);
        // Early exit if we know nothing about X.
        if l_range.is_full_set() {
            return None;
        }
        let x_ty = function.operand_type(self.context, x).expect("a value");
        // We are allowed to refine the comparison to either true or false for
        // out of range inputs. Here we refine the comparison to false, and
        // check if we can narrow the range check to a simpler test.
        for new_cr in [cr.exact_intersect_with(&l_range), cr.exact_union_with(&l_range.inverse())] {
            // Check if we can represent NewCR as an icmp predicate.
            if let Some((new_pred, new_rhs)) = new_cr.and_then(|new_cr| new_cr.get_equivalent_icmp()) {
                let i1 = self.context.types.int(1);
                let constant = Operand::Constant(self.context.int(x_ty, new_rhs as i128));
                return Some(self.insert_before(
                    function,
                    inst,
                    Opcode::ICmp(new_pred),
                    i1,
                    vec![x, constant],
                    Flags::default(),
                    None,
                ));
            }
        }
        None
    }

    /// Builds an instruction before `at` as `IRBuilder<NoFolder>` does, and
    /// records its value in `InsertedValues`.
    fn insert_before(
        &mut self,
        function: &mut Function,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
        name: Option<String>,
    ) -> Operand {
        let new = function.create_instruction(opcode, ty, operands, flags, name.as_deref());
        function.insert(new, Position::Before(at)).expect("a placed instruction");
        let value = function.instruction(new).result.expect("an instruction with a value");
        self.inserted.insert(value);
        Operand::Value(value)
    }

    /// `SCCPSolver::removeNonFeasibleEdges`.
    pub(crate) fn remove_non_feasible_edges(
        &self,
        function: &mut Function,
        block: BlockId,
        new_unreachable: &mut Option<BlockId>,
    ) -> bool {
        let Some(term) = function.terminator(block) else { return false };
        let operands = function.instruction(term).operands.clone();
        let successors: Vec<BlockId> =
            operands.iter().filter_map(|one| if let Operand::Block(to) = one { Some(*to) } else { None }).collect();
        let mut feasible_successors: Vec<BlockId> = Vec::new();
        let mut has_non_feasible_edges = false;
        for &succ in &successors {
            if self.results.is_edge_feasible(block, succ) {
                if !feasible_successors.contains(&succ) {
                    feasible_successors.push(succ);
                }
            } else {
                has_non_feasible_edges = true;
            }
        }
        // All edges feasible, nothing to do.
        if !has_non_feasible_edges {
            return false;
        }
        // SCCP can only determine non-feasible edges for br and switch.
        debug_assert!(
            matches!(function.instruction(term).opcode, Opcode::Br | Opcode::Switch),
            "Terminator must be a br or switch"
        );
        let void = function.instruction(term).ty;
        match feasible_successors.len() {
            0 => {
                // Branch on undef/poison, replace with unreachable.
                for &succ in &successors {
                    function.predecessor_removed(self.context, succ, block, false);
                }
                function.erase(term).expect("a terminator has no users");
                let unreachable =
                    function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
                function.insert(unreachable, Position::End(block)).expect("a placed block");
            }
            1 => {
                // Replace with an unconditional branch to the only feasible
                // successor.
                let only = feasible_successors[0];
                let mut have_seen_only_feasible_successor = false;
                for &succ in &successors {
                    if succ == only && !have_seen_only_feasible_successor {
                        // Don't remove the edge to the only feasible successor
                        // the first time we see it. We still do need to remove
                        // any multi-edges to it though.
                        have_seen_only_feasible_successor = true;
                        continue;
                    }
                    function.predecessor_removed(self.context, succ, block, false);
                }
                let branch =
                    function.create_instruction(Opcode::Br, void, vec![Operand::Block(only)], Flags::default(), None);
                function.erase(term).expect("a terminator has no users");
                function.insert(branch, Position::End(block)).expect("a placed block");
            }
            _ => {
                let Operand::Block(default_dest) = operands[1] else { unreachable!("a default destination") };
                let mut kept = vec![operands[0], operands[1]];
                // If the default destination is unfeasible it will never be
                // taken. Replace it with a new block with a single
                // Unreachable instruction.
                if !feasible_successors.contains(&default_dest) {
                    let unreachable_block = *new_unreachable.get_or_insert_with(|| {
                        let created = function.create_block(Some("default.unreachable"));
                        let at = function
                            .layout()
                            .iter()
                            .position(|&one| one == default_dest)
                            .expect("the default destination is placed");
                        let after = at.checked_sub(1).map(|previous| function.layout()[previous]);
                        function.insert_block(created, after).expect("a new block");
                        let unreachable =
                            function.create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
                        function.insert(unreachable, Position::End(created)).expect("a placed block");
                        created
                    });
                    function.predecessor_removed(self.context, default_dest, block, false);
                    kept[1] = Operand::Block(unreachable_block);
                }
                for pair in operands[2..].chunks(2) {
                    let Operand::Block(successor) = pair[1] else { unreachable!("a case successor") };
                    if feasible_successors.contains(&successor) {
                        kept.extend_from_slice(pair);
                        continue;
                    }
                    function.predecessor_removed(self.context, successor, block, false);
                }
                function.set_operands(term, kept);
            }
        }
        true
    }

    /// `DomTreeUpdater::deleteBB` of a dead block: the block and what is left
    /// of it (its phis and the `unreachable`) go.
    pub(crate) fn delete_block(
        &self,
        function: &mut Function,
        block: BlockId,
    ) {
        let insts: Vec<InstId> = function.iter_block(block).collect();
        for dead in insts.into_iter().rev() {
            if let Some(result) = function.instruction(dead).result
                && !function.users(result).is_empty()
            {
                let ty = function.instruction(dead).ty;
                let poison: ConstantId = self.context.constant(Constant { ty, kind: ConstantKind::Poison });
                function.replace_all_uses_with(result, Operand::Constant(poison));
            }
            function.erase(dead).expect("its uses were replaced");
        }
        function.erase_block(block).expect("nothing names a dead block");
    }

    /// `inferReturnAttributes`.
    pub(crate) fn infer_return_attributes(
        &self,
        function: &mut Function,
    ) -> bool {
        let Some(value) = self.results.tracked_return().cloned() else { return false };
        let Type::Function { returns, .. } = self.context.types.get(function.ty).clone() else { return false };
        infer_attribute(self.context, &mut function.return_attrs, returns, &value)
    }
}

/// `inferAttribute`: what the lattice value `value` of a return or an
/// argument of type `ty` states, as a `range` or `nonnull` in `attrs`;
/// whether `attrs` changed.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SCCPSolver.cpp#L522>
pub(crate) fn infer_attribute(
    context: &Context,
    attrs: &mut Vec<Attribute>,
    ty: TypeId,
    value: &ValueLatticeElement,
) -> bool {
    // If there is a known constant range for the value, add range attribute.
    if value.is_constant_range(true) && !value.get_constant_range(true).is_single_element() {
        // Do not add range attribute if the value may include undef.
        if value.is_constant_range_including_undef() {
            return false;
        }
        // Take the intersection of the existing attribute and the inferred
        // range.
        let mut range = *value.get_constant_range(false);
        let old = attrs
            .iter()
            .find_map(
                |one| match one {
                    Attribute::Range { ty, lower, upper } => {
                        Some(ConstantRange::new(context.types.int_bits(*ty).expect("an integer range"), *lower, *upper))
                    }
                    _ => None,
                },
            );
        if let Some(old) = old {
            range = range.intersect_with(&old, PreferredRangeType::Smallest);
        }
        attrs.retain(|one| !matches!(one, Attribute::Range { .. }));
        attrs.push(Attribute::Range { ty, lower: range.lower, upper: range.upper });
        return true;
    }
    // Infer nonnull attribute.
    if value.is_not_constant()
        && matches!(context.types.get(ty), Type::Pointer(_))
        && value.get_not_constant().is_null_value()
        && !Facts::of(attrs).non_null()
    {
        attrs.push(Fact::NonNull.attribute().expect("nonnull is an attribute"));
        return true;
    }
    false
}

/// `setIsExact(I.isExact())` onto a fresh instruction.
fn exact_only(flags: Flags) -> Flags {
    if flags.contains(Flags::EXACT) { Flags::EXACT } else { Flags::default() }
}

fn mask_of(width: u32) -> u128 {
    if width >= 128 { u128::MAX } else { (1_u128 << width) - 1 }
}

/// `m_LowBitMask`: the bits below some bit, at least one.
fn is_low_bit_mask(
    value: u128,
    width: u32,
) -> bool {
    value != 0 && value & mask_of(width) == value && (value.wrapping_add(1) & value) == 0
}

/// `m_NegatedPower2`: the negation of a power of two.
fn is_negated_power2(
    value: u128,
    width: u32,
) -> bool {
    value != 0 && (0_u128.wrapping_sub(value) & mask_of(width)).is_power_of_two()
}

/// `APInt::countr_zero`.
fn count_trailing_zeros(
    value: u128,
    width: u32,
) -> u32 {
    if value & mask_of(width) == 0 { width } else { value.trailing_zeros() }
}

#[cfg(test)]
#[path = "sccp_tests.rs"]
mod tests;
