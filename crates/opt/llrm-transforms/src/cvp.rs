//! Correlated value propagation: a value is replaced, narrowed or given a flag
//! when the branches and ranges around it prove the fact, as LLVM's
//! `correlated-propagation`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `CorrelatedValuePropagation.cpp`, one
//! Rust function per C++ function in file order, each linking its line. The
//! pass walks the blocks reachable from the entry in depth-first preorder and
//! asks [`llrm_mir::lazyvalueinfo::LazyValueInfo`] (the translation of
//! `LazyValueInfo.cpp`) for the constant, range or predicate of an operand at
//! its use. `runImpl` visits each instruction once, then the terminator, and
//! gives the function a `range` return attribute when every `ret` is bounded.
//! The analysis is kept across edits and told of every instruction erased
//! (`forget_value`), as LLVM's value handles do, so a function of N
//! instructions costs O(N) solves (`steps` counts them; the slope test in
//! `cvp_tests.rs` doubles a function and checks the count does not more than
//! double).
//!
//! Beside the pass, the C++ it calls that MIR does not hold is translated
//! here with its permalink: `BasicBlock::removePredecessor`,
//! `PHINode::removeIncomingValue`, `PHINode::hasConstantValue`, the `switch`
//! arm of `ConstantFoldTerminator`, `SwitchInst::removeCase`,
//! `SwitchInstProfUpdateWrapper`, `DomTree::dominates(Instruction,
//! BasicBlock)` and `GlobalValue::hasExactDefinition`.
//!
//! DEVIATE, permanent (MIR or Rust lacks the construct):
//! 1. No vectors: MIR types vectors but has no lane operation and nothing
//!    produces one, so each `process*` function takes a scalar integer only
//!    (#2201).
//! 2. No `undef`; `poison` stands for it where LVI hands back an `UndefValue`
//!    (#2202).
//! 3. No operand bundles: the `deopt` loop of `processCallSite` (lines 694-713)
//!    has nothing to read (#2203).
//! 4. `DomTreeUpdater` is the dominator tree brought up to date from the
//!    function's change log before its next query (`Analyses::get`), the lazy
//!    strategy's effect; `ConstantFoldTerminator` is its `switch` arm alone
//!    (the terminator `processSwitch` passes is a `switch`), with no
//!    `DeleteDeadConditions` (it is false there).
//! 5. `CreateSelectWithUnknownProfile` adds `!prof` only when the function has
//!    profile data; MIR has none. `STATISTIC` counters and `takeName` are not
//!    kept; debug locations do not exist.
//! 6. `Function::hasExactDefinition` ignores `getSemanticInterposition` and
//!    `isDSOLocal` (MIR has no module flags or visibility); a function the pass
//!    is run on without its id in the module (`Unit::id` is `None`) has no
//!    `range` attribute inferred.
//! 7. `BasicBlockEdge` dominance of an `invoke` result counts the predecessors
//!    of the normal destination once each (`Function::predecessors` lists a
//!    block once); two edges from one `invoke` to one block cannot occur.
//!
//! OWED (MIR can represent it; each has an issue and its exit):
//! 8. `fcmp`: LVI's `getPredicateAt` takes an `ICmpInst` predicate and holds no
//!    floating-point fact, so `processCmp` leaves an `fcmp` alone; only a
//!    compare of two constants, which `instsimplify` folds, differs. Exit:
//!    floating-point constant folding (#2246).
//! 9. `simplifyInstruction(P, SQ)` of `processPHI` is
//!    [`llrm_mir::simplify::simplify_instruction`], MIR's integer subset of
//!    `InstructionSimplify.cpp` without the dominator tree or the assumptions
//!    the C++ query holds. Exit: `simplify_instruction_in` once #2245 is in
//!    main.
//! 10. The `umin`/`umax` made by `processMinMaxIntrinsic` is declared through
//!     `Declared`, which the module has after the pass; LVI is given the same
//!     declaration beside the module's (`function_declaration`) while the run
//!     lasts, unless an earlier pass of the pipeline already declared a name,
//!     when LVI answers the full range through the call: #2347.
//! 11. MIR gives no `speculatable` to the saturating intrinsics and no
//!     `range(-1,2)` to `scmp`/`ucmp` (LVI note 13): eight functions of LLVM's
//!     tests differ. Exit: #2142.
//! 12. Not linear: `Function::insert(Before)` and `erase` scan the block, so
//!     one block with many conversions is quadratic (20,000 `sext`: 0.28 s, 4x
//!     per doubling). The pass's own work is O(n) (slope test on LVI solves).
//!     Exit: #2348.
//!
//! # Example
//! In the block entered when `%x u< 10`, `udiv i32 %x, 16` is `0`: the range
//! `[0, 10)` is below the divisor.
//!
//! # Runs
//! Alone, through `llrm-pass cvp`; not in the pipeline. `tools/difftest`
//! compares it with `opt -passes=correlated-propagation`.
//!
//! # Keeps
//! Dominators and loops where no `switch` changed an edge.
//!
//! # See also
//! [`llrm_mir::lazyvalueinfo`], [`llrm_mir::constantrange`].

use std::rc::Rc;

use llrm_mir::build::function_declaration;
use llrm_mir::constantrange::{ConstantRange, NO_SIGNED_WRAP, NO_UNSIGNED_WRAP, PreferredRangeType};
use llrm_mir::constfold;
use llrm_mir::context::{Constant, ConstantId, ConstantKind, mask, signed};
use llrm_mir::dense::IdMap;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::lazyvalueinfo::{Env, LazyValueInfo};
use llrm_mir::module::{
    BlockId, GlobalKind, GlobalValue, InstId, Linkage, MetadataId, MetadataNode, MetadataOperand, Operand, Use,
};
use llrm_mir::opcode::{Attribute, BinaryOp, CallInfo, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::simplify::simplify_instruction;
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuelattice::LatticeConstant;
use llrm_mir::valuetracking::guarantee::{is_guaranteed_not_to_be_poison, is_guaranteed_not_to_be_undef};
use llrm_mir::valuetracking::pattern::{self, IntrinsicId};
use llrm_mir::valuetracking::{AssumptionCache, Query};

pub struct Cvp;

#[cfg(test)]
thread_local! {
    /// The `solve` iterations of LVI, summed over the runs of this thread,
    /// for the slope test.
    pub(crate) static SOLVED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl FunctionPass for Cvp {
    fn name(&self) -> &'static str {
        "cvp"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("correlated-propagation")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if llrm_support::env_flag!("LLRM_NO_CVP") {
            return PreservedAnalyses::all();
        }
        let globals = Rc::clone(&analyses.outer().globals);
        let assumptions = AssumptionCache::of(unit.context, &globals, unit.metadata, unit.function);

        let shared = analyses.get::<llrm_mir::lazyvalueinfo::LviCache>(unit.context, unit.layout, unit.function);
        let mut pass = State { unit, analyses, globals, assumptions, lvi: shared.take(), cfg_changed: false };
        #[cfg(test)]
        let before = pass.lvi.get_impl().map_or(0, |one| one.steps.solve);
        let changed = pass.run_impl();
        #[cfg(test)]
        SOLVED.with(|solved| solved.set(solved.get() + pass.lvi.get_impl().map_or(0, |one| one.steps.solve) - before));
        shared.put(std::mem::take(&mut pass.lvi));

        match (changed, pass.cfg_changed) {
            (false, _) => PreservedAnalyses::all(),
            (true, false) => PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>(),
            (true, true) => PreservedAnalyses::none(),
        }
    }
}

/// What `runImpl` holds: the pass's unit and analyses, the module's
/// declarations (they name the callees), the assumptions and LVI.
struct State<'a, 'u> {
    unit: &'a mut Unit<'u>,
    analyses: &'a mut Analyses,
    globals: Rc<Vec<GlobalValue>>,
    assumptions: AssumptionCache,
    lvi: LazyValueInfo,
    /// A `switch` edge went: the dominator tree is brought up to date before
    /// its next query and the pass does not claim to keep it.
    cfg_changed: bool,
}

/// `Domain`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L763>
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Domain {
    NonNegative,
    NonPositive,
    Unknown,
}

/// `NoWrapFlags`.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1176>
#[derive(Clone, Copy, Default)]
struct NoWrapFlags {
    nsw: bool,
    nuw: bool,
}

/// `SwitchInstProfUpdateWrapper`: the branch weights of a `switch`, kept in
/// step with the cases removed and written back once.
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Instructions.h#L3640>
struct ProfWrapper {
    switch: InstId,
    weights: Option<Vec<u32>>,
    changed: bool,
}

// ---- glue: what the C++ gets from `Value`, `Instruction` and `IRBuilder` ----

impl State<'_, '_> {
    fn query(&self) -> Query<'_> {
        Query {
            context: &*self.unit.context,
            globals: &self.globals,
            metadata: self.unit.metadata,
            function: &*self.unit.function,
            layout: self.unit.layout,
            dt: None,
            assumptions: None,
            dom_conditions: None,
            cond_context: None,
            cxt: None,
        }
    }

    fn op(
        &self,
        inst: InstId,
        index: usize,
    ) -> Operand {
        self.unit.function.instruction(inst).operands[index]
    }

    fn result(
        &self,
        inst: InstId,
    ) -> llrm_mir::module::ValueId {
        self.unit.function.instruction(inst).result.expect("a value")
    }

    fn parent(
        &self,
        inst: InstId,
    ) -> BlockId {
        self.unit.function.parent(inst).expect("a placed instruction")
    }

    fn ty(
        &self,
        operand: Operand,
    ) -> TypeId {
        self.unit.function.operand_type(self.unit.context, operand).expect("a value")
    }

    /// `getScalarSizeInBits` of an integer operand, 0 for anything else.
    fn width(
        &self,
        operand: Operand,
    ) -> u32 {
        match self.unit.context.types.get(self.ty(operand)) {
            Type::Int(bits) => *bits,
            _ => 0,
        }
    }

    fn is_pointer(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            self.unit.context.types.get(self.ty(operand)),
            Type::Pointer(_)
        )
    }

    fn name_of(
        &self,
        inst: InstId,
    ) -> String {
        self.unit
            .function
            .instruction(inst)
            .result
            .and_then(|value| self.unit.function.value(value).name.clone())
            .unwrap_or_default()
    }

    /// The `ConstantInt` `operand` is: its bits and width.
    fn int_constant(
        &self,
        operand: Operand,
    ) -> Option<(u128, u32)> {
        self.query().int_constant(operand)
    }

    fn int(
        &mut self,
        ty: TypeId,
        value: i128,
    ) -> Operand {
        Operand::Constant(self.unit.context.int(ty, value))
    }

    fn bool_constant(
        &mut self,
        value: bool,
    ) -> Operand {
        let ty = self.unit.context.types.int(1);
        self.int(ty, i128::from(value))
    }

    fn null_of(
        &mut self,
        ty: TypeId,
    ) -> Operand {
        let kind =
            if self.unit.context.types.int_bits(ty).is_some() { ConstantKind::Int(0) } else { ConstantKind::Null };
        Operand::Constant(self.unit.context.constant(Constant { ty, kind }))
    }

    /// The `Constant *` LVI answers, as a constant of the context.
    fn constant_of(
        &mut self,
        lattice: LatticeConstant,
        value: Operand,
    ) -> ConstantId {
        match lattice {
            LatticeConstant::Int { width, bits } => {
                let ty = self.unit.context.types.int(width);
                self.unit.context.constant(Constant { ty, kind: ConstantKind::Int(bits) })
            }
            LatticeConstant::Other { ty, kind } => self.unit.context.constant(Constant { ty, kind }),
            LatticeConstant::Undef => {
                let ty = self.ty(value);
                self.unit.context.constant(Constant { ty, kind: ConstantKind::Undef })
            }
        }
    }

    fn erase(
        &mut self,
        inst: InstId,
    ) {
        // The deleted value handle of LVI.
        if let Some(result) = self.unit.function.instruction(inst).result {
            self.lvi.forget_value(Operand::Value(result));
        }
        self.unit.function.erase(inst).expect("an instruction nothing uses");
    }

    /// `replaceAllUsesWith(with)` then `eraseFromParent()`.
    fn replace_and_erase(
        &mut self,
        inst: InstId,
        with: Operand,
    ) {
        let result = self.result(inst);
        self.unit.function.replace_value(result, with);
        self.erase(inst);
    }

    /// An instruction made before `at`, as `IRBuilder` and
    /// `BinaryOperator::Create(.., InsertBefore)` do.
    fn emit(
        &mut self,
        at: InstId,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
        name: &str,
    ) -> Operand {
        let name = Some(name).filter(|one| !one.is_empty());
        let inst = self.unit.function.create_instruction(opcode, ty, operands, flags, name);
        self.unit.function.insert(inst, Position::Before(at)).expect("a placed instruction");
        Operand::Value(self.unit.function.instruction(inst).result.expect("a value"))
    }

    /// `IRBuilder::CreateBinOp`: two constants fold (`ConstantFolder`).
    fn create_bin_op(
        &mut self,
        at: InstId,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        flags: Flags,
        name: &str,
    ) -> Operand {
        if flags == Flags::default()
            && let (Operand::Constant(x), Operand::Constant(y)) = (lhs, rhs)
            && let Some(folded) = constfold::binary(self.unit.context, op, x, y)
        {
            return Operand::Constant(folded);
        }
        let ty = self.ty(lhs);
        self.emit(at, Opcode::Binary(op), ty, vec![lhs, rhs], flags, name)
    }

    /// `BinaryOperator::Create`: no folding.
    fn binary_operator(
        &mut self,
        at: InstId,
        op: BinaryOp,
        lhs: Operand,
        rhs: Operand,
        flags: Flags,
        name: &str,
    ) -> Operand {
        let ty = self.ty(lhs);
        self.emit(at, Opcode::Binary(op), ty, vec![lhs, rhs], flags, name)
    }

    /// `BinaryOperator::CreateNeg`: `sub 0, V`.
    fn binary_operator_neg(
        &mut self,
        at: InstId,
        value: Operand,
        name: &str,
    ) -> Operand {
        let zero = self.null_of(self.ty(value));
        self.binary_operator(at, BinaryOp::Sub, zero, value, Flags::default(), name)
    }

    /// `IRBuilder::CreateCast` (`CreateTruncOrBitCast` between two widths,
    /// `CreateSExt`, `CreateZExt`): a constant folds.
    fn create_cast(
        &mut self,
        at: InstId,
        op: CastOp,
        value: Operand,
        to: TypeId,
        name: &str,
    ) -> Operand {
        if let Operand::Constant(c) = value
            && let Some(folded) = constfold::cast(self.unit.context, op, c, to)
        {
            return Operand::Constant(folded);
        }
        self.emit(at, Opcode::Cast(op), to, vec![value], Flags::default(), name)
    }

    fn create_icmp(
        &mut self,
        at: InstId,
        predicate: IntPredicate,
        lhs: Operand,
        rhs: Operand,
        name: &str,
    ) -> Operand {
        let i1 = self.unit.context.types.int(1);
        if let (Operand::Constant(x), Operand::Constant(y)) = (lhs, rhs)
            && let Some(folded) = constfold::compare(self.unit.context, predicate, x, y)
        {
            return Operand::Constant(folded);
        }
        self.emit(at, Opcode::ICmp(predicate), i1, vec![lhs, rhs], Flags::default(), name)
    }

    /// The dominator tree: up to date with every `switch` edge removed so far.
    fn dominator_tree(&mut self) -> Rc<DomTree> {
        let function = &*self.unit.function;
        self.analyses.invalidate_if_edited(function, &PreservedAnalyses::none().preserve::<Dominators>());
        self.analyses.get::<Dominators>(&*self.unit.context, self.unit.layout, function)
    }

    // ---- LVI, asked of the function as it is now ----

    fn lvi_constant(
        &mut self,
        v: Operand,
        at: InstId,
    ) -> Option<ConstantId> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        let found = self.lvi.get_constant(&mut env, v, at);
        found.map(|lattice| self.constant_of(lattice, v))
    }

    fn lvi_range(
        &mut self,
        v: Operand,
        at: InstId,
        undef_allowed: bool,
    ) -> ConstantRange {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        self.lvi.get_constant_range(&mut env, v, at, undef_allowed)
    }

    fn lvi_range_at_use(
        &mut self,
        user: InstId,
        index: u32,
        undef_allowed: bool,
    ) -> ConstantRange {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        self.lvi.get_constant_range_at_use(&mut env, Use { user, index }, undef_allowed)
    }

    fn lvi_constant_on_edge(
        &mut self,
        v: Operand,
        from: BlockId,
        to: BlockId,
        cxt: InstId,
    ) -> Option<ConstantId> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        let found = self.lvi.get_constant_on_edge(&mut env, v, from, to, Some(cxt));
        found.map(|lattice| self.constant_of(lattice, v))
    }

    fn lvi_predicate_on_edge(
        &mut self,
        pred: IntPredicate,
        v: Operand,
        c: ConstantId,
        from: BlockId,
        to: BlockId,
        cxt: InstId,
    ) -> Option<bool> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        self.lvi.get_predicate_on_edge(&mut env, pred, v, c, from, to, Some(cxt))
    }

    fn lvi_predicate_at(
        &mut self,
        pred: IntPredicate,
        v: Operand,
        c: ConstantId,
        at: InstId,
        use_block_value: bool,
    ) -> Option<bool> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        self.lvi.get_predicate_at(&mut env, pred, v, c, at, use_block_value)
    }

    fn lvi_predicate_at_values(
        &mut self,
        pred: IntPredicate,
        lhs: Operand,
        rhs: Operand,
        at: InstId,
        use_block_value: bool,
    ) -> Option<bool> {
        let mut env = Env::new(
            &mut *self.unit.context,
            &self.globals,
            self.unit.metadata,
            &*self.unit.function,
            self.unit.layout,
            &self.assumptions,
        );
        self.lvi.get_predicate_at_values(&mut env, pred, lhs, rhs, at, use_block_value)
    }

    // ---- the intrinsics of IntrinsicInst.h ----

    /// `IntrinsicInst::getIntrinsicID` of a call (not an invoke).
    fn intrinsic_id(
        &self,
        inst: InstId,
    ) -> Option<IntrinsicId> {
        pattern::call_intrinsic(&self.query(), inst)
    }

    /// `BinaryOpIntrinsic`: `getBinaryOp`, `isSigned`, and whether it is a
    /// `WithOverflowInst` (else a `SaturatingInst`).
    fn binary_op_intrinsic(id: IntrinsicId) -> Option<(BinaryOp, bool, bool)> {
        use IntrinsicId::*;
        Some(match id {
            UaddWithOverflow => (BinaryOp::Add, false, true),
            SaddWithOverflow => (BinaryOp::Add, true, true),
            UsubWithOverflow => (BinaryOp::Sub, false, true),
            SsubWithOverflow => (BinaryOp::Sub, true, true),
            UmulWithOverflow => (BinaryOp::Mul, false, true),
            SmulWithOverflow => (BinaryOp::Mul, true, true),
            UaddSat => (BinaryOp::Add, false, false),
            SaddSat => (BinaryOp::Add, true, false),
            UsubSat => (BinaryOp::Sub, false, false),
            SsubSat => (BinaryOp::Sub, true, false),
            _ => return None,
        })
    }

    /// `MinMaxIntrinsic::getPredicate`.
    fn min_max_predicate(id: IntrinsicId) -> Option<IntPredicate> {
        Some(match id {
            IntrinsicId::Umin => IntPredicate::Ult,
            IntrinsicId::Umax => IntPredicate::Ugt,
            IntrinsicId::Smin => IntPredicate::Slt,
            IntrinsicId::Smax => IntPredicate::Sgt,
            _ => return None,
        })
    }

    /// `CallBase::paramHasAttr(ArgNo, Kind)` of a flag attribute: the call's,
    /// then the callee's.
    fn param_has_attr(
        &self,
        call: InstId,
        arg: usize,
        name: &str,
    ) -> bool {
        let q = self.query();
        let instruction = self.unit.function.instruction(call);
        let has = |attrs: &Vec<Attribute>| attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name));
        pattern::call_info(instruction).is_some_and(|info| info.argument_attrs.get(arg).is_some_and(has))
            || pattern::called_function(&q, instruction)
                .is_some_and(|function| function.parameter_attrs.get(arg).is_some_and(has))
    }
}

// ---- CorrelatedValuePropagation.cpp ----

impl State<'_, '_> {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L97>
    fn get_constant_at(
        &mut self,
        v: Operand,
        at: InstId,
    ) -> Option<ConstantId> {
        if let Some(c) = self.lvi_constant(v, at) {
            return Some(c);
        }

        // TODO: The following really should be sunk inside LVI's core
        // algorithm, or at least the outer shims around such.
        let (_, instruction) = self.query().instruction_of(v)?;
        let Opcode::ICmp(pred) = instruction.opcode else { return None };
        let (op0, op1) = (instruction.operands[0], instruction.operands[1]);
        let Operand::Constant(op1) = op1 else { return None };

        let res = self.lvi_predicate_at(pred, op0, op1, at, false)?;
        let i1 = self.unit.context.types.int(1);
        Some(self.unit.context.int(i1, i128::from(res)))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L116>
    fn process_select(
        &mut self,
        s: InstId,
    ) -> bool {
        let condition = self.op(s, 0);
        if matches!(
            self.unit.context.types.get(self.ty(Operand::Value(self.result(s)))),
            Type::Vector { .. }
        )
            || matches!(condition, Operand::Constant(_))
        {
            return false;
        }

        let mut changed = false;
        let result = self.result(s);
        for u in self.unit.function.users(result).snapshot() {
            let user = u.user;
            let c = if self.unit.function.instruction(user).opcode == Opcode::Phi {
                let incoming = self.op(user, u.index as usize + 1);
                let Operand::Block(incoming) = incoming else { unreachable!("a phi names its blocks") };
                let to = self.parent(user);
                self.lvi_constant_on_edge(condition, incoming, to, user)
            } else {
                self.get_constant_at(condition, user)
            };

            let Some((bits, 1)) = c.and_then(|c| self.int_constant(Operand::Constant(c))) else { continue };

            let replacement = if bits == 1 { self.op(s, 1) } else { self.op(s, 2) };
            self.unit.function.set_operand(user, u.index as usize, replacement);
            changed = true;
        }

        if changed && self.unit.function.users(result).is_empty() {
            self.erase(s);
        }

        changed
    }

    /// Try to simplify a phi with constant incoming values that match the edge
    /// values of a non-constant value on all other edges:
    /// bb0:
    ///   %isnull = icmp eq i8* %x, null
    ///   br i1 %isnull, label %bb2, label %bb1
    /// bb1:
    ///   br label %bb2
    /// bb2:
    ///   %r = phi i8* [ %x, %bb1 ], [ null, %bb0 ]
    /// -->
    ///   %r = %x
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L156>
    fn simplify_common_value_phi(
        &mut self,
        p: InstId,
    ) -> bool {
        // Collect incoming constants and initialize possible common value.
        let incoming: Vec<(Operand, Operand)> =
            self.unit.function.instruction(p).operands.chunks(2).map(|pair| (pair[0], pair[1])).collect();
        let mut incoming_constants: Vec<(ConstantId, usize)> = Vec::new();
        let mut common_value: Option<Operand> = None;
        for (i, &(value, _)) in incoming.iter().enumerate() {
            if let Operand::Constant(c) = value {
                incoming_constants.push((c, i));
            } else if common_value.is_none() {
                // The potential common value is initialized to the first
                // non-constant.
                common_value = Some(value);
            } else if Some(value) != common_value {
                // There can be only one non-constant common value.
                return false;
            }
        }

        let Some(common_value) = common_value else { return false };
        if incoming_constants.is_empty() {
            return false;
        }

        // The common value must be valid in all incoming blocks.
        let to_bb = self.parent(p);
        let dt = self.dominator_tree();
        if let Some((common_inst, _)) = self.query().instruction_of(common_value)
            && !self.dominates_block(&dt, common_inst, to_bb)
        {
            return false;
        }

        // We have a phi with exactly 1 variable incoming value and 1 or more
        // constant incoming values. See if all constant incoming values
        // can be mapped back to the same incoming variable value.
        for &(c, index) in &incoming_constants {
            let Operand::Block(incoming_bb) = incoming[index].1 else { unreachable!("a phi names its blocks") };
            let on_edge = self.lvi_constant_on_edge(common_value, incoming_bb, to_bb, p);
            let same = on_edge.is_some_and(|found| {
                LatticeConstant::of(self.unit.context, found) == LatticeConstant::of(self.unit.context, c)
            });
            if !same {
                return false;
            }
        }

        // LVI only guarantees that the value matches a certain constant if the
        // value is not poison. Make sure we don't replace a
        // well-defined value with poison. This is usually satisfied due
        // to a prior branch on the value.
        let q = Query { dt: Some(&dt), cxt: Some(p), ..self.query() };
        if !is_guaranteed_not_to_be_poison(&q, common_value, 0) {
            return false;
        }

        // All constant incoming values map to the same variable along the
        // incoming edges of the phi. The phi is unnecessary.
        self.replace_and_erase(p, common_value);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L207>
    fn get_value_on_edge(
        &mut self,
        incoming: Operand,
        from: BlockId,
        to: BlockId,
        cxt: InstId,
    ) -> Option<Operand> {
        if let Some(c) = self.lvi_constant_on_edge(incoming, from, to, cxt) {
            return Some(Operand::Constant(c));
        }

        // Look if the incoming value is a select with a scalar condition for
        // which LVI can tells us the value. In that case replace the
        // incoming value with the appropriate value of the select. This
        // often allows us to remove the select later.
        let (si, select) = self.query().instruction_of(incoming)?;
        if select.opcode != Opcode::Select {
            return None;
        }
        let (condition, true_value, false_value) = (select.operands[0], select.operands[1], select.operands[2]);

        // Once LVI learns to handle vector types, we could also add support
        // for vector type constants that are not all zeroes or all ones.
        if !matches!(
            self.unit.context.types.get(self.ty(condition)),
            Type::Vector { .. }
        )
            && let Some(c) = self.lvi_constant_on_edge(condition, from, to, cxt)
            && let Some((bits, width)) = self.int_constant(Operand::Constant(c))
        {
            if bits == mask(width) {
                return Some(true_value);
            }
            if bits == 0 {
                return Some(false_value);
            }
        }

        // Look if the select has a constant but LVI tells us that the incoming
        // value can never be that constant. In that case replace the incoming
        // value with the other value of the select. This often allows us to
        // remove the select later.

        // The "false" case
        let select_value = Operand::Value(self.result(si));
        if let Operand::Constant(c) = false_value
            && self.lvi_predicate_on_edge(IntPredicate::Eq, select_value, c, from, to, cxt) == Some(false)
        {
            return Some(true_value);
        }

        // The "true" case,
        // similar to the select "false" case, but try the select "true" value
        if let Operand::Constant(c) = true_value
            && self.lvi_predicate_on_edge(IntPredicate::Eq, select_value, c, from, to, cxt) == Some(false)
        {
            return Some(false_value);
        }

        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L256>
    fn process_phi(
        &mut self,
        p: InstId,
    ) -> bool {
        let mut changed = false;

        let bb = self.parent(p);
        let count = self.unit.function.instruction(p).operands.len() / 2;
        for i in 0..count {
            let incoming = self.op(p, 2 * i);
            if matches!(incoming, Operand::Constant(_)) {
                continue;
            }

            let Operand::Block(incoming_bb) = self.op(p, 2 * i + 1) else { unreachable!("a phi names its blocks") };
            if let Some(v) = self.get_value_on_edge(incoming, incoming_bb, bb, p) {
                self.unit.function.set_operand(p, 2 * i, v);
                changed = true;
            }
        }

        if let Some(v) = simplify_instruction(self.unit.context, self.unit.layout, self.unit.function, p) {
            self.replace_and_erase(p, v);
            changed = true;
        }

        if !changed {
            changed = self.simplify_common_value_phi(p);
        }

        changed
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L287>
    fn process_icmp(
        &mut self,
        cmp: InstId,
    ) -> bool {
        let Opcode::ICmp(pred) = self.unit.function.instruction(cmp).opcode else { unreachable!("an icmp") };
        // Only for signed relational comparisons of integers.
        if self.width(self.op(cmp, 0)) == 0 {
            return false;
        }

        let is_signed = matches!(
            pred,
            IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
        );
        let is_unsigned = matches!(
            pred,
            IntPredicate::Ugt | IntPredicate::Uge | IntPredicate::Ult | IntPredicate::Ule
        );
        let same_sign = self.unit.function.instruction(cmp).flags.contains(Flags::SAMESIGN);
        if !is_signed && (!is_unsigned || same_sign) {
            return false;
        }

        let mut changed = false;

        let cr1 = self.lvi_range_at_use(cmp, 0, false);
        let cr2 = self.lvi_range_at_use(cmp, 1, false);

        if is_signed {
            let Some(unsigned_pred) = ConstantRange::get_equivalent_pred_with_flipped_signedness(pred, &cr1, &cr2)
            else {
                return false;
            };

            self.unit.function.set_icmp_predicate(cmp, unsigned_pred);
            changed = true;
        }

        if ConstantRange::are_insensitive_to_signedness_of_icmp_predicate(&cr1, &cr2) {
            let mut flags = self.unit.function.instruction(cmp).flags;
            flags.insert(Flags::SAMESIGN);
            self.unit.function.set_flags(cmp, flags);
            changed = true;
        }

        changed
    }

    /// See if LazyValueInfo's ability to exploit edge conditions or range
    /// information is sufficient to prove this comparison. Even for local
    /// conditions, this can sometimes prove conditions instcombine can't by
    /// exploiting range information.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L327>
    fn constant_fold_cmp(
        &mut self,
        cmp: InstId,
    ) -> bool {
        let Opcode::ICmp(pred) = self.unit.function.instruction(cmp).opcode else { unreachable!("an icmp") };
        let (op0, op1) = (self.op(cmp, 0), self.op(cmp, 1));
        let Some(res) = self.lvi_predicate_at_values(pred, op0, op1, cmp, true) else { return false };
        let res = self.bool_constant(res);

        // `replaceUsesWithIf`: every use but an `llvm.assume`'s.
        let result = self.result(cmp);
        let mut changed = false;
        for u in self.unit.function.users(result).snapshot() {
            if self.intrinsic_id(u.user) != Some(IntrinsicId::Assume) {
                self.unit.function.set_operand(u.user, u.index as usize, res);
                changed = true;
            }
        }
        if self.unit.function.users(result).is_empty() {
            self.erase(cmp);
            changed = true;
        }

        changed
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L348>
    fn process_cmp(
        &mut self,
        cmp: InstId,
    ) -> bool {
        if self.constant_fold_cmp(cmp) {
            return true;
        }

        if self.process_icmp(cmp) {
            return true;
        }

        false
    }

    /// Simplify a switch instruction by removing cases which can never fire. If
    /// the uselessness of a case could be determined locally then constant
    /// propagation would already have figured it out. Instead, walk the
    /// predecessors and statically evaluate cases based on information
    /// available on that edge. Cases that cannot fire no matter what the
    /// incoming edge can safely be removed. If a case fires on every
    /// incoming edge then the entire switch can be removed and replaced
    /// with a branch to the case destination.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L366>
    fn process_switch(
        &mut self,
        i: InstId,
    ) -> bool {
        let mut cond = self.op(i, 0);
        let bb = self.parent(i);

        // Analyse each switch case in turn.
        let mut changed = false;
        let mut successors_count = IdMap::<BlockId, i32>::new();
        for succ in self.switch_edges(i) {
            *successors_count.get_or_insert_with(succ, || 0) += 1;
        }

        {
            // Scope for SwitchInstProfUpdateWrapper. It must not live during
            // ConstantFoldTerminator() as the underlying SwitchInst can be
            // changed.
            let mut si = self.prof_wrapper(i);
            let cr = self.lvi_range_at_use(i, 0, false);
            let mut reachable_case_count: u64 = 0;

            let mut case_index = 0;
            while case_index < self.num_cases(i) {
                let case = self.case_value(i, case_index);
                let case_bits = self.int_constant(Operand::Constant(case)).expect("an integer case").0;
                let mut predicate: Option<bool> = None;
                if !cr.contains(case_bits) {
                    predicate = Some(false);
                } else if cr.is_single_element() && cr.get_single_element() == Some(case_bits) {
                    predicate = Some(true);
                }
                if predicate.is_none() {
                    // Handle missing cases, e.g., the range has a hole.
                    predicate = self.lvi_predicate_at(IntPredicate::Eq, cond, case, i, true);
                }

                if predicate == Some(false) {
                    // This case never fires - remove it.
                    let succ = self.case_successor(i, case_index);
                    self.remove_predecessor(succ, bb, false);
                    self.prof_remove_case(&mut si, case_index);

                    // The condition can be modified by removePredecessor's PHI
                    // simplification logic.
                    cond = self.op(i, 0);

                    changed = true;
                    let count = successors_count.get_or_insert_with(succ, || 0);
                    *count -= 1;
                    if *count == 0 {
                        self.cfg_changed = true;
                    }
                    continue;
                }
                if predicate == Some(true) {
                    // This case always fires.  Arrange for the switch to be
                    // turned into an unconditional branch
                    // by replacing the switch condition with the case
                    // value.
                    self.unit.function.set_operand(i, 0, Operand::Constant(case));
                    changed = true;
                    break;
                }

                // Increment the case iterator since we didn't delete it.
                case_index += 1;
                reachable_case_count += 1;
            }

            // The default dest is unreachable if all cases are covered.
            if !self.default_dest_unreachable(i) && !cr.is_size_larger_than(reachable_case_count) {
                let default_dest = self.default_dest(i);
                let new_unreachable_bb = self.unit.function.create_block(Some("default.unreachable"));
                let layout = self.unit.function.layout();
                let at = layout.iter().position(|one| *one == default_dest).expect("a placed block");
                let after = at.checked_sub(1).map(|before| layout[before]);
                self.unit.function.insert_block(new_unreachable_bb, after).expect("a new block");
                let void = self.unit.context.types.void();
                let ui = self
                    .unit
                    .function
                    .create_instruction(Opcode::Unreachable, void, Vec::new(), Flags::default(), None);
                self.unit.function.insert(ui, Position::End(new_unreachable_bb)).expect("a new block");

                self.remove_predecessor(default_dest, bb, false);
                self.unit.function.set_operand(i, 1, Operand::Block(new_unreachable_bb));

                self.cfg_changed = true;
                changed = true;
            }
            self.prof_wrapper_drop(si);
        }

        if changed {
            // If the switch has been simplified to the point where it can be
            // replaced by a branch then do so now.
            self.constant_fold_terminator(bb);
            if self.cfg_changed {
                let function = &*self.unit.function;
                self.analyses.invalidate(function, &PreservedAnalyses::none());
            }
        }
        changed
    }

    // See if we can prove that the given binary op intrinsic will not overflow.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L467>
    fn will_not_overflow(
        &mut self,
        bo: InstId,
        op: BinaryOp,
        is_signed: bool,
    ) -> bool {
        let l_range = self.lvi_range_at_use(bo, 0, false);
        let r_range = self.lvi_range_at_use(bo, 1, false);
        let no_wrap_kind = if is_signed { NO_SIGNED_WRAP } else { NO_UNSIGNED_WRAP };
        let nw_region = ConstantRange::make_guaranteed_no_wrap_region(op, &r_range, no_wrap_kind);
        nw_region.contains_range(&l_range)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L477>
    fn set_deduced_overflowing_flags(
        &mut self,
        v: Operand,
        new_nsw: bool,
        new_nuw: bool,
    ) {
        let Some((inst, _)) = self.query().instruction_of(v) else { return };
        let mut flags = self.unit.function.instruction(inst).flags;
        if new_nsw {
            flags.insert(Flags::NSW);
        }
        if new_nuw {
            flags.insert(Flags::NUW);
        }
        if new_nsw || new_nuw {
            self.unit.function.set_flags(inst, flags);
        }
    }

    // See if @llvm.abs argument is alays positive/negative, and simplify.
    // Notably, INT_MIN can belong to either range, regardless of the NSW,
    // because it is negation-invariant.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L529>
    fn process_abs_intrinsic(
        &mut self,
        ii: InstId,
    ) -> bool {
        let x = self.op(ii, 0);
        let is_int_min_poison = self.int_constant(self.op(ii, 1)).is_some_and(|(bits, _)| bits == 1);
        let width = self.width(x);
        let int_min = 1u128 << (width - 1);
        let range = self.lvi_range_at_use(ii, 0, is_int_min_poison);

        // Is X in [0, IntMin]?  NOTE: INT_MIN is fine!
        if range.icmp(IntPredicate::Ule, &ConstantRange::single(width, int_min)) {
            self.replace_and_erase(ii, x);
            return true;
        }

        // Is X in [IntMin, 0]?  NOTE: INT_MIN is fine!
        if signed(range.get_signed_max(), width) <= 0 {
            let flags = if is_int_min_poison { Flags::NSW } else { Flags::default() };
            let name = self.name_of(ii);
            let zero = self.null_of(self.ty(x));
            let neg_x = self.create_bin_op(ii, BinaryOp::Sub, zero, x, flags, &name);
            self.replace_and_erase(ii, neg_x);

            // See if we can infer some no-wrap flags.
            if let Some((bo, _)) = self.query().instruction_of(neg_x) {
                self.process_bin_op(bo);
            }

            return true;
        }

        // Argument's range crosses zero.
        // Can we at least tell that the argument is never INT_MIN?
        if !is_int_min_poison && !range.contains(int_min) {
            let t = self.bool_constant(true);
            self.unit.function.set_operand(ii, 1, t);
            return true;
        }
        false
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L571>
    fn process_cmp_intrinsic(
        &mut self,
        ci: InstId,
        is_signed: bool,
    ) -> bool {
        let lhs_cr = self.lvi_range_at_use(ci, 0, false);
        let rhs_cr = self.lvi_range_at_use(ci, 1, false);
        let (gt, lt) =
            if is_signed { (IntPredicate::Sgt, IntPredicate::Slt) } else { (IntPredicate::Ugt, IntPredicate::Ult) };
        let ty = self.unit.function.instruction(ci).ty;

        if lhs_cr.icmp(gt, &rhs_cr) {
            let one = self.int(ty, 1);
            self.replace_and_erase(ci, one);
            return true;
        }
        if lhs_cr.icmp(lt, &rhs_cr) {
            let minus_one = self.int(ty, -1);
            self.replace_and_erase(ci, minus_one);
            return true;
        }
        if lhs_cr.icmp(IntPredicate::Eq, &rhs_cr) {
            let zero = self.int(ty, 0);
            self.replace_and_erase(ci, zero);
            return true;
        }

        false
    }

    // See if this min/max intrinsic always picks it's one specific operand.
    // If not, check whether we can canonicalize signed minmax into unsigned
    // version
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L601>
    fn process_min_max_intrinsic(
        &mut self,
        mm: InstId,
        id: IntrinsicId,
    ) -> bool {
        // `CmpInst::getNonStrictPredicate`.
        let pred = match Self::min_max_predicate(id).expect("a min or max") {
            IntPredicate::Ult => IntPredicate::Ule,
            IntPredicate::Ugt => IntPredicate::Uge,
            IntPredicate::Slt => IntPredicate::Sle,
            IntPredicate::Sgt => IntPredicate::Sge,
            other => other,
        };
        let lhs_cr = self.lvi_range_at_use(mm, 0, false);
        let rhs_cr = self.lvi_range_at_use(mm, 1, false);
        let (lhs, rhs) = (self.op(mm, 0), self.op(mm, 1));
        if lhs_cr.icmp(pred, &rhs_cr) {
            self.replace_and_erase(mm, lhs);
            return true;
        }
        if rhs_cr.icmp(pred, &lhs_cr) {
            self.replace_and_erase(mm, rhs);
            return true;
        }

        let is_signed = matches!(id, IntrinsicId::Smin | IntrinsicId::Smax);
        if is_signed && ConstantRange::are_insensitive_to_signedness_of_icmp_predicate(&lhs_cr, &rhs_cr) {
            let unsigned = if id == IntrinsicId::Smin { IntrinsicId::Umin } else { IntrinsicId::Umax };
            let replacement = self.create_binary_intrinsic(mm, unsigned, lhs, rhs);
            self.replace_and_erase(mm, replacement);
            return true;
        }

        false
    }

    // Rewrite this with.overflow intrinsic as non-overflowing.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L637>
    fn process_overflow_intrinsic(
        &mut self,
        wo: InstId,
        opcode: BinaryOp,
        is_signed: bool,
    ) -> bool {
        let nsw = is_signed;
        let nuw = !is_signed;

        let (lhs, rhs) = (self.op(wo, 0), self.op(wo, 1));
        let name = self.name_of(wo);
        let new_op = self.create_bin_op(wo, opcode, lhs, rhs, Flags::default(), &name);
        self.set_deduced_overflowing_flags(new_op, nsw, nuw);

        let st = self.unit.function.instruction(wo).ty;
        let Type::Struct { fields, .. } = self.unit.context.types.get(st).clone() else { unreachable!("a pair") };
        let poison = self.unit.context.constant(Constant { ty: fields[0], kind: ConstantKind::Poison });
        let false_value = self.unit.context.int(fields[1], 0);
        // `ConstantStruct::get`, then `CreateInsertValue(Struct, NewOp, 0)`.
        let new_i = match new_op {
            Operand::Constant(c) => {
                let folded = Constant { ty: st, kind: ConstantKind::Aggregate(vec![c, false_value]) };
                Operand::Constant(self.unit.context.constant(folded))
            }
            _ => {
                let struct_ = Constant { ty: st, kind: ConstantKind::Aggregate(vec![poison, false_value]) };
                let struct_ = Operand::Constant(self.unit.context.constant(struct_));
                self.emit(wo, Opcode::InsertValue(vec![0]), st, vec![struct_, new_op], Flags::default(), "")
            }
        };
        self.replace_and_erase(wo, new_i);

        // See if we can infer the other no-wrap too.
        if let Some((bo, _)) = self.query().instruction_of(new_op) {
            self.process_bin_op(bo);
        }

        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L663>
    fn process_saturating_inst(
        &mut self,
        si: InstId,
        opcode: BinaryOp,
        is_signed: bool,
    ) -> bool {
        let nsw = is_signed;
        let nuw = !is_signed;
        let (lhs, rhs) = (self.op(si, 0), self.op(si, 1));
        let name = self.name_of(si);
        let bin_op = self.binary_operator(si, opcode, lhs, rhs, Flags::default(), &name);
        self.set_deduced_overflowing_flags(bin_op, nsw, nuw);

        self.replace_and_erase(si, bin_op);

        // See if we can infer the other no-wrap too.
        let (bin_op, _) = self.query().instruction_of(bin_op).expect("an instruction");
        self.process_bin_op(bin_op);

        true
    }

    /// Infer nonnull attributes for the arguments at the specified callsite.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L683>
    fn process_call_site(
        &mut self,
        cb: InstId,
    ) -> bool {
        let id = self.intrinsic_id(cb);

        if id == Some(IntrinsicId::Abs) && self.scalar_int_args(cb, 1) {
            return self.process_abs_intrinsic(cb);
        }

        if let Some(id @ (IntrinsicId::Scmp | IntrinsicId::Ucmp)) = id
            && self.scalar_int_args(cb, 2)
        {
            return self.process_cmp_intrinsic(cb, id == IntrinsicId::Scmp);
        }

        if let Some(id) = id
            && Self::min_max_predicate(id).is_some()
            && self.scalar_int_args(cb, 2)
        {
            return self.process_min_max_intrinsic(cb, id);
        }

        if let Some(id) = id
            && let Some((op, is_signed, with_overflow)) = Self::binary_op_intrinsic(id)
            && self.scalar_int_args(cb, 2)
        {
            if with_overflow {
                if self.will_not_overflow(cb, op, is_signed) {
                    return self.process_overflow_intrinsic(cb, op, is_signed);
                }
            } else if self.will_not_overflow(cb, op, is_signed) {
                return self.process_saturating_inst(cb, op, is_signed);
            }
        }

        let changed = false;

        let mut arg_nos: Vec<usize> = Vec::new();
        let args = pattern::call_arguments(self.unit.function.instruction(cb)).to_vec();

        for (arg_no, &v) in args.iter().enumerate() {
            // Try to mark pointer typed parameters as non-null.  We skip the
            // relatively expensive analysis for constants which are obviously
            // either null or non-null to start with.
            if self.is_pointer(v) && !self.param_has_attr(cb, arg_no, "nonnull") && !matches!(v, Operand::Constant(_)) {
                let ty = self.ty(v);
                let null = self.unit.context.constant(Constant { ty, kind: ConstantKind::Null });
                if self.lvi_predicate_at(IntPredicate::Eq, v, null, cb, false) == Some(false) {
                    arg_nos.push(arg_no);
                }
            }
        }

        if arg_nos.is_empty() {
            return changed;
        }

        for arg_no in arg_nos {
            self.add_nonnull(cb, arg_no);
        }

        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L765>
    fn get_domain(cr: &ConstantRange) -> Domain {
        if cr.is_all_non_negative() {
            return Domain::NonNegative;
        }
        if cr.icmp(IntPredicate::Sle, &ConstantRange::single(cr.width, 0)) {
            return Domain::NonPositive;
        }
        Domain::Unknown
    }

    /// Try to shrink a sdiv/srem's width down to the smallest power of two
    /// that's sufficient to contain its operands.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L775>
    fn narrow_sdiv_or_srem(
        &mut self,
        instr: InstId,
        lcr: &ConstantRange,
        rcr: &ConstantRange,
    ) -> bool {
        let Opcode::Binary(opcode) = self.unit.function.instruction(instr).opcode else { unreachable!("a division") };

        // Find the smallest power of two bitwidth that's sufficient to hold
        // Instr's operands.
        let orig_width = self.width(Operand::Value(self.result(instr)));

        // What is the smallest bit width that can accommodate the entire value
        // ranges of both of the operands?
        let mut min_signed_bits = lcr.get_min_signed_bits().max(rcr.get_min_signed_bits());

        // sdiv/srem is UB if divisor is -1 and divident is INT_MIN, so unless
        // we can prove that such a combination is impossible, we need
        // to bump the bitwidth.
        let sext_min = mask(orig_width) & !mask(min_signed_bits - 1);
        if rcr.contains(mask(orig_width)) && lcr.contains(sext_min) {
            min_signed_bits += 1;
        }

        // Don't shrink below 8 bits wide.
        let new_width = min_signed_bits.next_power_of_two().max(8);

        // NewWidth might be greater than OrigWidth if OrigWidth is not a power
        // of two.
        if new_width >= orig_width {
            return false;
        }

        let trunc_ty = self.unit.context.types.int(new_width);
        let ty = self.unit.function.instruction(instr).ty;
        let name = self.name_of(instr);
        let (op0, op1) = (self.op(instr, 0), self.op(instr, 1));
        let lhs = self.create_cast(instr, CastOp::Trunc, op0, trunc_ty, &format!("{name}.lhs.trunc"));
        let rhs = self.create_cast(instr, CastOp::Trunc, op1, trunc_ty, &format!("{name}.rhs.trunc"));
        let bo = self.create_bin_op(instr, opcode, lhs, rhs, Flags::default(), &name);
        let sext = self.create_cast(instr, CastOp::SExt, bo, ty, &format!("{name}.sext"));
        if let Some((bin_op, _)) = self.query().instruction_of(bo)
            && opcode == BinaryOp::SDiv
        {
            let exact = self.unit.function.instruction(instr).flags.intersect(Flags::EXACT);
            let mut flags = self.unit.function.instruction(bin_op).flags;
            flags.insert(exact);
            self.unit.function.set_flags(bin_op, flags);
        }

        self.replace_and_erase(instr, sext);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L821>
    fn expand_udiv_or_urem(
        &mut self,
        instr: InstId,
        xcr: &ConstantRange,
        ycr: &ConstantRange,
    ) -> bool {
        let ty = self.unit.function.instruction(instr).ty;
        let Opcode::Binary(opcode) = self.unit.function.instruction(instr).opcode else { unreachable!("a division") };
        let is_rem = opcode == BinaryOp::URem;

        let x = self.op(instr, 0);
        let y = self.op(instr, 1);

        // X u/ Y -> 0  iff X u< Y
        // X u% Y -> X  iff X u< Y
        if xcr.icmp(IntPredicate::Ult, ycr) {
            let with = if is_rem { x } else { self.null_of(ty) };
            self.replace_and_erase(instr, with);
            return true;
        }

        // Given
        //   R  = X u% Y
        // We can represent the modulo operation as a loop/self-recursion:
        //   urem_rec(X, Y):
        //     Z = X - Y
        //     if X u< Y
        //       ret X
        //     else
        //       ret urem_rec(Z, Y)
        // which isn't better, but if we only need a single iteration
        // to compute the answer, this becomes quite good:
        //   R  = X < Y ? X : X - Y    iff X u< 2*Y (w/ unsigned saturation)
        // Now, we do not care about all full multiples of Y in X, they do not
        // change the answer, thus we could rewrite the expression as:
        //   X* = X - (Y * |_ X / Y _|)
        //   R  = X* % Y
        // so we don't need the *first* iteration to return, we just need to
        // know *which* iteration will always return, so we could also rewrite
        // it as:   X* = X - (Y * |_ X / Y _|)
        //   R  = X* % Y                 iff X* u< 2*Y (w/ unsigned saturation)
        // but that does not seem profitable here.

        // Even if we don't know X's range, the divisor may be so large, X can't
        // ever be 2x larger than that. I.e. if divisor is always
        // negative.
        if !xcr.icmp(IntPredicate::Ult, &ycr.uadd_sat(ycr)) && !ycr.is_all_negative() {
            return false;
        }

        let name = self.name_of(instr);
        let expanded_op = if xcr.icmp(IntPredicate::Uge, ycr) {
            // If X is between Y and 2*Y the result is known.
            if is_rem { self.create_bin_op(instr, BinaryOp::Sub, x, y, Flags::NUW, "") } else { self.int(ty, 1) }
        } else if is_rem {
            // NOTE: this transformation introduces two uses of X,
            //       but it may be undef so we must freeze it first.
            let mut frozen_x = x;
            if !self.guaranteed_not_undef(x) {
                frozen_x = self.emit(instr, Opcode::Freeze, self.ty(x), vec![x], Flags::default(), &self.frozen(x));
            }
            let mut frozen_y = y;
            if !self.guaranteed_not_undef(y) {
                frozen_y = self.emit(instr, Opcode::Freeze, self.ty(y), vec![y], Flags::default(), &self.frozen(y));
            }
            let adj_x =
                self.create_bin_op(instr, BinaryOp::Sub, frozen_x, frozen_y, Flags::NUW, &format!("{name}.urem"));
            let cmp = self.create_icmp(instr, IntPredicate::Ult, frozen_x, frozen_y, &format!("{name}.cmp"));
            let selected = self.ty(frozen_x);
            self.create_select(instr, cmp, frozen_x, adj_x, selected)
        } else {
            let cmp = self.create_icmp(instr, IntPredicate::Uge, x, y, &format!("{name}.cmp"));
            self.create_cast(instr, CastOp::ZExt, cmp, ty, &format!("{name}.udiv"))
        };
        self.replace_and_erase(instr, expanded_op);
        true
    }

    /// Try to shrink a udiv/urem's width down to the smallest power of two
    /// that's sufficient to contain its operands.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L903>
    fn narrow_udiv_or_urem(
        &mut self,
        instr: InstId,
        xcr: &ConstantRange,
        ycr: &ConstantRange,
    ) -> bool {
        let Opcode::Binary(opcode) = self.unit.function.instruction(instr).opcode else { unreachable!("a division") };

        // Find the smallest power of two bitwidth that's sufficient to hold
        // Instr's operands.

        // What is the smallest bit width that can accommodate the entire value
        // ranges of both of the operands?
        let max_active_bits = xcr.get_active_bits().max(ycr.get_active_bits());
        // Don't shrink below 8 bits wide.
        let new_width = max_active_bits.next_power_of_two().max(8);

        // NewWidth might be greater than OrigWidth if OrigWidth is not a power
        // of two.
        if new_width >= self.width(Operand::Value(self.result(instr))) {
            return false;
        }

        let trunc_ty = self.unit.context.types.int(new_width);
        let ty = self.unit.function.instruction(instr).ty;
        let name = self.name_of(instr);
        let (op0, op1) = (self.op(instr, 0), self.op(instr, 1));
        let lhs = self.create_cast(instr, CastOp::Trunc, op0, trunc_ty, &format!("{name}.lhs.trunc"));
        let rhs = self.create_cast(instr, CastOp::Trunc, op1, trunc_ty, &format!("{name}.rhs.trunc"));
        let bo = self.create_bin_op(instr, opcode, lhs, rhs, Flags::default(), &name);
        let zext = self.create_cast(instr, CastOp::ZExt, bo, ty, &format!("{name}.zext"));
        if let Some((bin_op, _)) = self.query().instruction_of(bo)
            && opcode == BinaryOp::UDiv
        {
            let exact = self.unit.function.instruction(instr).flags.intersect(Flags::EXACT);
            let mut flags = self.unit.function.instruction(bin_op).flags;
            flags.insert(exact);
            self.unit.function.set_flags(bin_op, flags);
        }

        self.replace_and_erase(instr, zext);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L940>
    fn process_udiv_or_urem(
        &mut self,
        instr: InstId,
    ) -> bool {
        let xcr = self.lvi_range_at_use(instr, 0, false);
        // Allow undef for RHS, as we can assume it is division by zero UB.
        let ycr = self.lvi_range_at_use(instr, 1, true);
        if self.expand_udiv_or_urem(instr, &xcr, &ycr) {
            return true;
        }

        self.narrow_udiv_or_urem(instr, &xcr, &ycr)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L954>
    fn process_srem(
        &mut self,
        sdi: InstId,
        lcr: &ConstantRange,
        rcr: &ConstantRange,
    ) -> bool {
        if lcr.abs(false).icmp(IntPredicate::Ult, &rcr.abs(false)) {
            let x = self.op(sdi, 0);
            self.replace_and_erase(sdi, x);
            return true;
        }

        let mut ops = [(self.op(sdi, 0), Self::get_domain(lcr)), (self.op(sdi, 1), Self::get_domain(rcr))];
        if ops[0].1 == Domain::Unknown || ops[1].1 == Domain::Unknown {
            return false;
        }

        // We know domains of both of the operands!

        // We need operands to be non-negative, so negate each one that isn't.
        for op in &mut ops {
            if op.1 == Domain::NonNegative {
                continue;
            }
            let name = self.value_name(op.0);
            op.0 = self.binary_operator_neg(sdi, op.0, &format!("{name}.nonneg"));
        }

        let name = self.name_of(sdi);
        let urem = self.binary_operator(sdi, BinaryOp::URem, ops[0].0, ops[1].0, Flags::default(), &name);

        let mut res = urem;

        // If the divident was non-positive, we need to negate the result.
        if ops[0].1 == Domain::NonPositive {
            let name = self.value_name(res);
            res = self.binary_operator_neg(sdi, res, &format!("{name}.neg"));
        }

        self.replace_and_erase(sdi, res);

        // Try to simplify our new urem.
        let (urem, _) = self.query().instruction_of(urem).expect("an instruction");
        self.process_udiv_or_urem(urem);

        true
    }

    /// See if LazyValueInfo's ability to exploit edge conditions or range
    /// information is sufficient to prove the signs of both operands of this
    /// SDiv. If this is the case, replace the SDiv with a UDiv. Even for
    /// local conditions, this can sometimes prove conditions instcombine
    /// can't by exploiting range information.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1013>
    fn process_sdiv(
        &mut self,
        sdi: InstId,
        lcr: &ConstantRange,
        rcr: &ConstantRange,
    ) -> bool {
        // Check whether the division folds to a constant.
        let div_cr = lcr.sdiv(rcr);
        if let Some(elem) = div_cr.get_single_element() {
            let ty = self.unit.function.instruction(sdi).ty;
            let c = self.int(ty, elem as i128);
            self.replace_and_erase(sdi, c);
            return true;
        }

        let mut ops = [(self.op(sdi, 0), Self::get_domain(lcr)), (self.op(sdi, 1), Self::get_domain(rcr))];
        if ops[0].1 == Domain::Unknown || ops[1].1 == Domain::Unknown {
            return false;
        }

        // We know domains of both of the operands!

        // We need operands to be non-negative, so negate each one that isn't.
        for op in &mut ops {
            if op.1 == Domain::NonNegative {
                continue;
            }
            let name = self.value_name(op.0);
            op.0 = self.binary_operator_neg(sdi, op.0, &format!("{name}.nonneg"));
        }

        let name = self.name_of(sdi);
        let exact = self.unit.function.instruction(sdi).flags.intersect(Flags::EXACT);
        let udiv = self.binary_operator(sdi, BinaryOp::UDiv, ops[0].0, ops[1].0, exact, &name);

        let mut res = udiv;

        // If the operands had two different domains, we need to negate the
        // result.
        if ops[0].1 != ops[1].1 {
            let name = self.value_name(res);
            res = self.binary_operator_neg(sdi, res, &format!("{name}.neg"));
        }

        self.replace_and_erase(sdi, res);

        // Try to simplify our new udiv.
        let (udiv, _) = self.query().instruction_of(udiv).expect("an instruction");
        self.process_udiv_or_urem(udiv);

        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1070>
    fn process_sdiv_or_srem(
        &mut self,
        instr: InstId,
    ) -> bool {
        let Opcode::Binary(opcode) = self.unit.function.instruction(instr).opcode else { unreachable!("a division") };
        let lcr = self.lvi_range_at_use(instr, 0, false);
        // Allow undef for RHS, as we can assume it is division by zero UB.
        let rcr = self.lvi_range_at_use(instr, 1, true);
        if opcode == BinaryOp::SDiv && self.process_sdiv(instr, &lcr, &rcr) {
            return true;
        }

        if opcode == BinaryOp::SRem && self.process_srem(instr, &lcr, &rcr) {
            return true;
        }

        self.narrow_sdiv_or_srem(instr, &lcr, &rcr)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1090>
    fn process_ashr(
        &mut self,
        sdi: InstId,
    ) -> bool {
        let l_range = self.lvi_range_at_use(sdi, 0, false);
        let orig_width = self.width(Operand::Value(self.result(sdi)));
        let neg_one_or_zero = ConstantRange::new(orig_width, mask(orig_width), 1);
        if neg_one_or_zero.contains_range(&l_range) {
            // ashr of -1 or 0 never changes the value, so drop the whole
            // instruction
            let x = self.op(sdi, 0);
            self.replace_and_erase(sdi, x);
            return true;
        }

        if !l_range.is_all_non_negative() {
            return false;
        }

        let name = self.name_of(sdi);
        let exact = self.unit.function.instruction(sdi).flags.intersect(Flags::EXACT);
        let (op0, op1) = (self.op(sdi, 0), self.op(sdi, 1));
        let bo = self.binary_operator(sdi, BinaryOp::LShr, op0, op1, exact, &name);
        self.replace_and_erase(sdi, bo);

        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1119>
    fn process_sext(
        &mut self,
        sdi: InstId,
    ) -> bool {
        let base = self.op(sdi, 0);
        if !self.lvi_range_at_use(sdi, 0, false).is_all_non_negative() {
            return false;
        }

        let name = self.name_of(sdi);
        let ty = self.unit.function.instruction(sdi).ty;
        let zext = self.emit(sdi, Opcode::Cast(CastOp::ZExt), ty, vec![base], Flags::NNEG, &name);
        self.replace_and_erase(sdi, zext);

        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1137>
    fn process_possible_non_neg(
        &mut self,
        i: InstId,
    ) -> bool {
        if self.unit.function.instruction(i).flags.contains(Flags::NNEG) {
            return false;
        }

        if !self.lvi_range_at_use(i, 0, false).is_all_non_negative() {
            return false;
        }

        let mut flags = self.unit.function.instruction(i).flags;
        flags.insert(Flags::NNEG);
        self.unit.function.set_flags(i, flags);

        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1152>
    fn process_zext(
        &mut self,
        zext: InstId,
    ) -> bool {
        self.process_possible_non_neg(zext)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1156>
    fn process_ui_to_fp(
        &mut self,
        ui_to_fp: InstId,
    ) -> bool {
        self.process_possible_non_neg(ui_to_fp)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1160>
    fn process_si_to_fp(
        &mut self,
        si_to_fp: InstId,
    ) -> bool {
        let base = self.op(si_to_fp, 0);
        if !self.lvi_range_at_use(si_to_fp, 0, false).is_all_non_negative() {
            return false;
        }

        let name = self.name_of(si_to_fp);
        let ty = self.unit.function.instruction(si_to_fp).ty;
        let ui_to_fp = self.emit(si_to_fp, Opcode::Cast(CastOp::UIToFP), ty, vec![base], Flags::NNEG, &name);
        self.replace_and_erase(si_to_fp, ui_to_fp);

        true
    }

    // Check if the requested no-wrap flags are valid for \p Opcode on \p LRange
    // and \p RRange.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1187>
    fn compute_no_wrap_flags(
        opcode: BinaryOp,
        l_range: &ConstantRange,
        r_range: &ConstantRange,
        check_nsw: bool,
        check_nuw: bool,
    ) -> NoWrapFlags {
        let mut flags = NoWrapFlags::default();
        if check_nuw {
            flags.nuw = ConstantRange::make_guaranteed_no_wrap_region(opcode, r_range, NO_UNSIGNED_WRAP)
                .contains_range(l_range);
        }
        if check_nsw {
            flags.nsw =
                ConstantRange::make_guaranteed_no_wrap_region(opcode, r_range, NO_SIGNED_WRAP).contains_range(l_range);
        }
        flags
    }

    // Try to prove that \p BinOp does not wrap by looking at the operand ranges
    // constrained at each of its use sites, rather than at the definition. This
    // improves results, e.g. when all uses are constrained by a runtime check.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1207>
    fn infer_no_wrap_from_uses(
        &mut self,
        bin_op: InstId,
        want_nsw: bool,
        want_nuw: bool,
    ) -> NoWrapFlags {
        // Skip analysis, when there are too many uses to check or any use is in
        // the same block.
        const MAX_USES_TO_INSPECT: usize = 4;
        let def_bb = self.parent(bin_op);
        let uses = self.unit.function.users(self.result(bin_op)).snapshot();
        let mut num_uses = 0;
        for u in &uses {
            num_uses += 1;
            if num_uses > MAX_USES_TO_INSPECT {
                return NoWrapFlags::default();
            }
            let user_i = u.user;
            if self.unit.function.instruction(user_i).opcode == Opcode::Phi || self.parent(user_i) == def_bb {
                return NoWrapFlags::default();
            }
        }
        if num_uses == 0 {
            return NoWrapFlags::default();
        }

        let Opcode::Binary(opcode) = self.unit.function.instruction(bin_op).opcode else { unreachable!("a binary op") };
        let mut flags = NoWrapFlags { nsw: want_nsw, nuw: want_nuw };
        for u in &uses {
            let user_i = u.user;
            // Constrain both operands at this use site and see which flags
            // still hold.
            let (op0, op1) = (self.op(bin_op, 0), self.op(bin_op, 1));
            let l_range = self.lvi_range(op0, user_i, false);
            let r_range = self.lvi_range(op1, user_i, false);
            flags = Self::compute_no_wrap_flags(opcode, &l_range, &r_range, flags.nsw, flags.nuw);
            if !flags.nsw && !flags.nuw {
                return NoWrapFlags::default();
            }
        }

        flags
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1244>
    fn process_bin_op(
        &mut self,
        bin_op: InstId,
    ) -> bool {
        let flags = self.unit.function.instruction(bin_op).flags;
        let nsw = flags.contains(Flags::NSW);
        let nuw = flags.contains(Flags::NUW);
        if nsw && nuw {
            return false;
        }

        let Opcode::Binary(opcode) = self.unit.function.instruction(bin_op).opcode else { unreachable!("a binary op") };
        let l_range = self.lvi_range_at_use(bin_op, 0, false);
        let r_range = self.lvi_range_at_use(bin_op, 1, false);

        let mut new = Self::compute_no_wrap_flags(opcode, &l_range, &r_range, !nsw, !nuw);

        // If a still-wanted flag could not be proven at the definition, retry
        // using the operand ranges constrained at the use sites. This
        // is the more expensive path, so it only runs when the cheap
        // query above came up short.
        let want_nsw = !nsw && !new.nsw;
        let want_nuw = !nuw && !new.nuw;
        if want_nsw || want_nuw {
            let from_uses = self.infer_no_wrap_from_uses(bin_op, want_nsw, want_nuw);
            new.nsw |= from_uses.nsw;
            new.nuw |= from_uses.nuw;
        }

        self.set_deduced_overflowing_flags(Operand::Value(self.result(bin_op)), new.nsw, new.nuw);

        new.nsw || new.nuw
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1276>
    fn process_and(
        &mut self,
        bin_op: InstId,
    ) -> bool {
        // Pattern match (and lhs, C) where C includes a superset of bits which
        // might be set in lhs.  This is a common truncation idiom
        // created by instcombine.
        let lhs = self.op(bin_op, 0);
        let Some((rhs, _)) = pattern::low_bit_mask(&self.query(), self.op(bin_op, 1)) else { return false };

        // We can only replace the AND with LHS based on range info if the range
        // does not include undef.
        let l_range = self.lvi_range_at_use(bin_op, 0, false);
        if l_range.get_unsigned_max() > rhs {
            return false;
        }

        self.replace_and_erase(bin_op, lhs);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1299>
    fn process_trunc(
        &mut self,
        ti: InstId,
    ) -> bool {
        let mut flags = self.unit.function.instruction(ti).flags;
        if flags.contains(Flags::NSW) && flags.contains(Flags::NUW) {
            return false;
        }

        let range = self.lvi_range_at_use(ti, 0, false);
        let dest_width = self.width(Operand::Value(self.result(ti)));
        let mut changed = false;

        if !flags.contains(Flags::NUW) && range.get_active_bits() <= dest_width {
            flags.insert(Flags::NUW);
            changed = true;
        }

        if !flags.contains(Flags::NSW) && range.get_min_signed_bits() <= dest_width {
            flags.insert(Flags::NSW);
            changed = true;
        }

        if changed {
            self.unit.function.set_flags(ti, flags);
        }
        changed
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/CorrelatedValuePropagation.cpp#L1327>
    fn run_impl(&mut self) -> bool {
        let mut fn_changed = false;
        let mut ret_range: Option<ConstantRange> = None;
        if self.has_exact_definition()
            && let Some(width) = self.return_width()
        {
            ret_range = Some(ConstantRange::empty(width));
        }

        // Visiting in a pre-order depth-first traversal causes us to simplify
        // early blocks before querying later blocks (which require us
        // to analyze early blocks).  Eagerly simplifying shallow blocks
        // means there is strictly less work to do for deep blocks.
        // This also means we don't visit unreachable blocks.
        let Some(entry) = self.unit.function.entry() else { return false };
        let mut visited = llrm_mir::dense::IdSet::<BlockId>::new();
        visited.insert(entry);
        // `depth_first`: a block's successors are read when the walk leaves
        // it, after its body is processed.
        let mut stack: Vec<(BlockId, Vec<BlockId>, usize)> = Vec::new();
        let mut next = Some(entry);
        while let Some(bb) = next.take() {
            let mut bb_changed = false;
            for ii in self.unit.function.iter_block(bb).collect::<Vec<_>>() {
                if self.unit.function.is_erased(ii) {
                    continue;
                }
                let instruction = self.unit.function.instruction(ii);
                let (opcode, ty) = (instruction.opcode.clone(), instruction.ty);
                let scalar_int = self.unit.context.types.int_bits(ty).is_some();
                bb_changed |= match opcode {
                    Opcode::Select => self.process_select(ii),
                    Opcode::Phi => self.process_phi(ii),
                    Opcode::ICmp(_) => self.process_cmp(ii),
                    Opcode::Call(_) | Opcode::Invoke(_) => self.process_call_site(ii),
                    Opcode::Binary(BinaryOp::SRem | BinaryOp::SDiv) if scalar_int => self.process_sdiv_or_srem(ii),
                    Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) if scalar_int => self.process_udiv_or_urem(ii),
                    Opcode::Binary(BinaryOp::AShr) if scalar_int => self.process_ashr(ii),
                    Opcode::Cast(CastOp::SExt) if scalar_int => self.process_sext(ii),
                    Opcode::Cast(CastOp::ZExt) if scalar_int => self.process_zext(ii),
                    Opcode::Cast(CastOp::UIToFP) if self.width(self.op(ii, 0)) != 0 => self.process_ui_to_fp(ii),
                    Opcode::Cast(CastOp::SIToFP) if self.width(self.op(ii, 0)) != 0 => self.process_si_to_fp(ii),
                    Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl) if scalar_int => {
                        self.process_bin_op(ii)
                    }
                    Opcode::Binary(BinaryOp::And) if scalar_int => self.process_and(ii),
                    Opcode::Cast(CastOp::Trunc) if scalar_int => self.process_trunc(ii),
                    _ => false,
                };
            }

            let term = self.unit.function.terminator(bb).expect("a terminated block");
            match self.unit.function.instruction(term).opcode {
                Opcode::Switch => bb_changed |= self.process_switch(term),
                Opcode::Ret => {
                    // Try to determine the return value if we can.  This is
                    // mainly here to simplify the writing
                    // of unit tests, but also helps to enable IPO by
                    // constant folding the return values of callees.
                    if let Some(&ret_val) = self.unit.function.instruction(term).operands.first() {
                        if let Some(range) = ret_range.as_ref()
                            && !range.is_full_set()
                        {
                            let found = self.lvi_range(ret_val, term, false);
                            ret_range = Some(range.union_with(&found, PreferredRangeType::Smallest));
                        }

                        if !matches!(ret_val, Operand::Constant(_))
                            && let Some(c) = self.get_constant_at(ret_val, term)
                        {
                            self.unit.function.set_operand(term, 0, Operand::Constant(c));
                            bb_changed = true;
                        }
                    }
                }
                _ => {}
            }

            fn_changed |= bb_changed;
            stack.push((bb, self.unit.function.successors(bb).to_vec(), 0));
            next = loop {
                let Some(top) = stack.last_mut() else { break None };
                if let Some(&child) = top.1.get(top.2) {
                    top.2 += 1;
                    if visited.insert(child) {
                        break Some(child);
                    }
                } else {
                    stack.pop();
                }
            };
        }

        // Infer range attribute on return value.
        if let Some(range) = ret_range.as_ref()
            && !range.is_full_set()
        {
            let mut range = *range;
            if let Some(attr) = self.ret_range_attribute() {
                range = range.intersect_with(&attr, PreferredRangeType::Smallest);
            }
            // Don't add attribute for constant integer returns to reduce noise.
            // These are propagated across functions by IPSCCP.
            if !range.is_empty_set() && !range.is_single_element() {
                self.add_range_ret_attr(&range);
                fn_changed = true;
            }
        }
        fn_changed
    }
}

// ---- what the C++ calls in other files, translated where MIR lacks it ----

impl State<'_, '_> {
    /// The scalar integer operands of an intrinsic call (vectors are a
    /// DEVIATE).
    fn scalar_int_args(
        &self,
        call: InstId,
        count: usize,
    ) -> bool {
        (0..count).all(|index| self.width(self.op(call, index)) != 0)
    }

    fn value_name(
        &self,
        operand: Operand,
    ) -> String {
        match operand {
            Operand::Value(value) => self.unit.function.value(value).name.clone().unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn frozen(
        &self,
        operand: Operand,
    ) -> String {
        format!("{}.frozen", self.value_name(operand))
    }

    /// `isGuaranteedNotToBeUndef(V)`: no assumptions, no context.
    fn guaranteed_not_undef(
        &self,
        operand: Operand,
    ) -> bool {
        is_guaranteed_not_to_be_undef(&self.query(), operand, 0)
    }

    /// `IRBuilder::CreateSelect`: three constants fold.
    fn create_select(
        &mut self,
        at: InstId,
        condition: Operand,
        if_true: Operand,
        if_false: Operand,
        ty: TypeId,
    ) -> Operand {
        if let (Operand::Constant(c), Operand::Constant(x), Operand::Constant(y)) = (condition, if_true, if_false)
            && let Some(folded) = constfold::select(self.unit.context, c, x, y)
        {
            return Operand::Constant(folded);
        }
        self.emit(at, Opcode::Select, ty, vec![condition, if_true, if_false], Flags::default(), "")
    }

    /// `IRBuilder::CreateBinaryIntrinsic(umin | umax, lhs, rhs)` before `at`,
    /// which is a call to the signed form: `ConstantFolder` folds no
    /// intrinsic.
    fn create_binary_intrinsic(
        &mut self,
        at: InstId,
        id: IntrinsicId,
        lhs: Operand,
        rhs: Operand,
    ) -> Operand {
        let (Opcode::Call(info) | Opcode::Invoke(info)) = self.unit.function.instruction(at).opcode.clone() else {
            unreachable!("a call")
        };
        let base = if id == IntrinsicId::Umin { "umin" } else { "umax" };
        let ty = self.unit.function.instruction(at).ty;
        let width = self.unit.context.types.int_bits(ty).expect("an integer min or max");
        let name = format!("llvm.{base}.i{width}");
        let callee = self.unit.declared.declare(&name, info.function_type);
        // The module has the declaration after the pass; LVI names the call by
        // it now.
        if callee.0 as usize == self.globals.len() {
            let function = function_declaration(self.unit.context, &name, info.function_type).expect("a function type");
            let declaration = GlobalValue {
                name: name.into(),
                linkage: Linkage::External,
                unnamed_addr: Default::default(),
                address_space: 0,
                kind: GlobalKind::Function(Box::new(function)),
            };
            Rc::make_mut(&mut self.globals).push(declaration);
        }
        let pointer = self.unit.context.types.ptr(0);
        let callee = self.unit.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) });
        let info = CallInfo {
            function_type: info.function_type,
            calling_convention: info.calling_convention,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 2],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        self.emit(at, Opcode::Call(Box::new(info)), ty, vec![lhs, rhs, Operand::Constant(callee)], Flags::default(), "")
    }

    /// `CallBase::setAttributes` with `nonnull` added to argument `arg`, at the
    /// place LLVM's attribute set sorts it: after the enum attributes that
    /// precede `NonNull`, before the integer ones.
    fn add_nonnull(
        &mut self,
        call: InstId,
        arg: usize,
    ) {
        let info = pattern::call_info(self.unit.function.instruction(call)).expect("a call").clone();
        let mut arguments = info.argument_attrs;
        let attrs = &mut arguments[arg];
        let after = |attr: &Attribute| match attr {
            Attribute::Flag(name) => matches!(
                name.as_str(),
                "readnone" | "readonly" | "returned" | "signext" | "swifterror" | "swiftself" | "writeonly" | "zeroext"
            ),
            _ => true,
        };
        let at = attrs.iter().position(after).unwrap_or(attrs.len());
        attrs.insert(at, Attribute::Flag("nonnull".to_owned()));
        self.unit.function.set_call_attributes(call, info.attrs, info.return_attrs, arguments);
    }

    /// `DomTree::dominates(const Instruction *Def, const BasicBlock
    /// *UseBB)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L134>
    fn dominates_block(
        &self,
        dt: &DomTree,
        def: InstId,
        use_bb: BlockId,
    ) -> bool {
        let def_bb = self.parent(def);

        // Any unreachable use is dominated, even if DefBB == UseBB.
        if !dt.is_reachable(use_bb) {
            return true;
        }

        // Unreachable definitions don't dominate anything.
        if !dt.is_reachable(def_bb) {
            return false;
        }

        if def_bb == use_bb {
            return false;
        }

        // Invoke results are only usable in the normal destination, not in the
        // exceptional destination.
        let instruction = self.unit.function.instruction(def);
        if let Opcode::Invoke(_) = instruction.opcode {
            let Operand::Block(normal_dest) = instruction.operands[instruction.operands.len() - 3] else {
                unreachable!("an invoke names its destinations")
            };
            return self.dominates_edge(dt, def_bb, normal_dest, use_bb);
        }

        dt.dominates(def_bb, use_bb)
    }

    /// `DomTree::dominates(const BasicBlockEdge &BBE, const BasicBlock
    /// *UseBB)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Dominators.cpp#L162>
    fn dominates_edge(
        &self,
        dt: &DomTree,
        start: BlockId,
        end: BlockId,
        use_bb: BlockId,
    ) -> bool {
        // If the BB the edge ends in doesn't dominate the use BB, then the
        // edge also doesn't.
        if !dt.dominates(end, use_bb) {
            return false;
        }

        // Simple case: if the end BB has a single predecessor, the fact that it
        // dominates the use block implies that the edge also does.
        let predecessors = self.unit.function.predecessors(end);
        if predecessors.len() == 1 {
            return true;
        }

        // The normal edge from the invoke is critical: Given the definition of
        // dominance, NormalDest is dominated by X (the edge split) iff X
        // dominates all of NormalDest's predecessors. X trivially dominates
        // itself, so we only have to find if it dominates the other
        // predecessors.
        let mut is_duplicate_edge = 0;
        for &bb in predecessors.iter() {
            if bb == start {
                // If there are multiple edges between Start and End, by
                // definition they can't dominate anything.
                if is_duplicate_edge > 0 {
                    return false;
                }
                is_duplicate_edge += 1;
                continue;
            }

            if !dt.dominates(end, bb) {
                return false;
            }
        }
        true
    }

    /// `GlobalValue::hasExactDefinition` of the function being run on.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/GlobalValue.h#L501>
    fn has_exact_definition(&self) -> bool {
        let Some(id) = self.unit.id else { return false };
        let linkage = self.globals[id.0 as usize].linkage;
        let has_flag = |name: &str| {
            self.unit.function.attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name))
        };
        // `mayBeDerefined`, <.../GlobalValue.h#L139>
        let may_be_derefined = match linkage {
            Linkage::WeakOdr | Linkage::LinkOnceOdr | Linkage::AvailableExternally => true,
            _ => {
                // `isInterposable`, <.../lib/IR/Globals.cpp#L178>
                let interposable = has_flag("noipa")
                    || matches!(
                        linkage,
                        Linkage::Weak | Linkage::LinkOnce | Linkage::Common | Linkage::ExternWeak
                    );
                interposable || has_flag("nobuiltin")
            }
        };
        !self.unit.function.is_declaration() && !may_be_derefined
    }

    /// The width of an integer return type.
    fn return_width(&self) -> Option<u32> {
        let Type::Function { returns, .. } = self.unit.context.types.get(self.unit.function.ty) else { return None };
        self.unit.context.types.int_bits(*returns)
    }

    /// `F.getRetAttribute(Attribute::Range).getRange()`.
    fn ret_range_attribute(&self) -> Option<ConstantRange> {
        pattern::range_attribute(&self.query(), &self.unit.function.return_attrs)
    }

    /// `F.addRangeRetAttr(Range)`.
    fn add_range_ret_attr(
        &mut self,
        range: &ConstantRange,
    ) {
        let Type::Function { returns, .. } = self.unit.context.types.get(self.unit.function.ty) else {
            unreachable!("a function type")
        };
        let ty = *returns;
        let attrs = &mut self.unit.function.return_attrs;
        attrs.retain(|one| !matches!(one, Attribute::Range { .. }));
        attrs.push(Attribute::Range { ty, lower: range.lower, upper: range.upper });
    }

    // ---- SwitchInst ----

    fn num_cases(
        &self,
        switch: InstId,
    ) -> usize {
        (self.unit.function.instruction(switch).operands.len() - 2) / 2
    }

    fn default_dest(
        &self,
        switch: InstId,
    ) -> BlockId {
        let Operand::Block(block) = self.op(switch, 1) else { unreachable!("a switch names its default") };
        block
    }

    fn case_value(
        &self,
        switch: InstId,
        index: usize,
    ) -> ConstantId {
        let Operand::Constant(case) = self.op(switch, 2 + 2 * index) else { unreachable!("a constant case") };
        case
    }

    fn case_successor(
        &self,
        switch: InstId,
        index: usize,
    ) -> BlockId {
        let Operand::Block(block) = self.op(switch, 3 + 2 * index) else { unreachable!("a switch names its cases") };
        block
    }

    /// `successors(SI)`: the default, then each case's, one per edge.
    fn switch_edges(
        &self,
        switch: InstId,
    ) -> Vec<BlockId> {
        std::iter::once(self.default_dest(switch))
            .chain((0..self.num_cases(switch)).map(|index| self.case_successor(switch, index)))
            .collect()
    }

    /// `SwitchInst::defaultDestUnreachable`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Instructions.h#L3508>
    fn default_dest_unreachable(
        &self,
        switch: InstId,
    ) -> bool {
        let function = &*self.unit.function;
        function
            .iter_block(self.default_dest(switch))
            .find(|&inst| function.instruction(inst).opcode != Opcode::Phi)
            .is_some_and(|inst| function.instruction(inst).opcode == Opcode::Unreachable)
    }

    /// `SwitchInst::removeCase`: the last case takes the removed one's place.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L4174>
    fn switch_remove_case(
        &mut self,
        switch: InstId,
        index: usize,
    ) {
        let mut operands = self.unit.function.instruction(switch).operands.clone();
        let count = operands.len();

        // Overwrite this case with the end of the list.
        if 2 + 2 * index + 2 != count {
            operands[2 + 2 * index] = operands[count - 2];
            operands[3 + 2 * index] = operands[count - 1];
        }

        // Nuke the last value.
        operands.truncate(count - 2);
        self.unit.function.set_operands(switch, operands);
    }

    // ---- ProfDataUtils.cpp, SwitchInstProfUpdateWrapper ----

    fn metadata_node(
        &self,
        id: MetadataId,
    ) -> Option<&MetadataNode> {
        let held = self.unit.metadata.len();
        match (id.0 as usize).checked_sub(held) {
            None => self.unit.metadata.get(id.0 as usize),
            Some(made) => self.unit.declared.nodes().get(made),
        }
    }

    /// `getBranchWeightMDNode` and `extractFromBranchWeightMD`: the weights of
    /// the `!prof` of `inst` and whether they are `expected`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ProfDataUtils.cpp#L180>
    fn branch_weights(
        &self,
        inst: InstId,
    ) -> Option<(Vec<u64>, bool)> {
        let (_, id) = self.unit.function.instruction(inst).metadata.iter().find(|(kind, _)| kind == "prof")?;
        let node = self.metadata_node(*id)?;
        // `isBranchWeightMD`
        if node.operands.len() < 2 || node.operands[0] != MetadataOperand::String("branch_weights".to_owned()) {
            return None;
        }
        // `hasBranchWeightOrigin`
        let expected = matches!(&node.operands[1], MetadataOperand::String(_));
        let offset = if expected { 2 } else { 1 };
        let weights = node.operands[offset.min(node.operands.len())..]
            .iter()
            .map(|one| match one {
                MetadataOperand::Constant(c) => {
                    self.int_constant(Operand::Constant(*c)).map_or(0, |(bits, _)| bits as u64)
                }
                _ => 0,
            })
            .collect();
        Some((weights, expected))
    }

    /// `getValidBranchWeightMDNode`: the weights, when there is one per
    /// successor.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ProfDataUtils.cpp#L187>
    fn valid_branch_weights(
        &self,
        switch: InstId,
    ) -> Option<(Vec<u64>, bool)> {
        self.branch_weights(switch).filter(|(weights, _)| weights.len() == self.num_cases(switch) + 1)
    }

    /// `setBranchWeights`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ProfDataUtils.cpp#L300>
    fn set_branch_weights(
        &mut self,
        inst: InstId,
        weights: &[u32],
        is_expected: bool,
    ) {
        let i32_ty = self.unit.context.types.int(32);
        let mut operands = vec![MetadataOperand::String("branch_weights".to_owned())];
        if is_expected {
            operands.push(MetadataOperand::String("expected".to_owned()));
        }
        for &weight in weights {
            operands.push(MetadataOperand::Constant(self.unit.context.int(i32_ty, i128::from(weight))));
        }
        let node = self.unit.declared.node(MetadataNode { distinct: false, operands });
        self.unit.function.unannotate(inst, "prof");
        self.unit.function.annotate(inst, "prof", node);
    }

    /// `SwitchInstProfUpdateWrapper::init`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L4208>
    fn prof_wrapper(
        &self,
        switch: InstId,
    ) -> ProfWrapper {
        let weights = self
            .branch_weights(switch)
            .map(
                |(weights, _)| {
                    assert_eq!(weights.len(), self.num_cases(switch) + 1, "branch_weights do not match the successors");
                    weights.into_iter().map(|one| one as u32).collect()
                },
            );
        ProfWrapper { switch, weights, changed: false }
    }

    /// `SwitchInstProfUpdateWrapper::removeCase`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/Instructions.cpp#L4225>
    fn prof_remove_case(
        &mut self,
        wrapper: &mut ProfWrapper,
        index: usize,
    ) {
        if let Some(weights) = &mut wrapper.weights {
            wrapper.changed = true;
            // Copy the last case to the place of the removed one and shrink.
            // This is tightly coupled with the way SwitchInst::removeCase()
            // removes the cases in SwitchInst::removeCase(CaseIt).
            weights[index + 1] = *weights.last().expect("a default weight");
            weights.pop();
        }
        self.switch_remove_case(wrapper.switch, index);
    }

    /// `~SwitchInstProfUpdateWrapper`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/IR/Instructions.h#L3664>
    fn prof_wrapper_drop(
        &mut self,
        wrapper: ProfWrapper,
    ) {
        if wrapper.changed
            && let Some(weights) = wrapper.weights
        {
            if weights.len() >= 2 {
                self.set_branch_weights(wrapper.switch, &weights, false);
                return;
            }
            // In some cases while simplifying switch instructions, we end up
            // with degenerate switch instructions (e.g., only
            // contains the default case). We drop profile metadata
            // in such cases rather than updating given it
            // does not convey anything.
            self.unit.function.unannotate(wrapper.switch, "prof");
        }
    }

    // ---- BasicBlock.cpp, Instructions.cpp, Local.cpp ----

    /// `BasicBlock::removePredecessor`.
    fn remove_predecessor(
        &mut self,
        block: BlockId,
        pred: BlockId,
        keep_one_input_phis: bool,
    ) {
        let function = &*self.unit.function;
        let phis: Vec<(InstId, Operand)> = function
            .iter_block(block)
            .take_while(|&inst| function.instruction(inst).opcode == Opcode::Phi)
            .map(|phi| (phi, Operand::Value(self.result(phi))))
            .collect();
        self.unit.function.predecessor_removed(self.unit.context, block, pred, keep_one_input_phis);
        // The deleted value handle of LVI, for the phis the call erased.
        for (phi, result) in phis {
            if self.unit.function.is_erased(phi) {
                self.lvi.forget_value(result);
            }
        }
    }

    /// `ConstantFoldTerminator(BB, false, nullptr, &DTU)` of a block ending in
    /// a `switch`: its `SwitchInst` arm.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L133>
    fn constant_fold_terminator(
        &mut self,
        bb: BlockId,
    ) -> bool {
        let si = self.unit.function.terminator(bb).expect("a terminator");
        if self.unit.function.instruction(si).opcode != Opcode::Switch {
            return false;
        }
        let void = self.unit.context.types.void();

        // If we are switching on a constant, we can convert the switch to an
        // unconditional branch.
        let mut ci = self.int_constant(self.op(si, 0));
        let default_dest = self.default_dest(si);
        let mut the_only_dest = Some(default_dest);

        // If the default is unreachable, ignore it when searching for
        // TheOnlyDest.
        if self.default_dest_unreachable(si) && self.num_cases(si) > 0 {
            the_only_dest = Some(self.case_successor(si, 0));
        }

        let mut changed = false;

        // Figure out which case it goes to.
        let mut it = 0;
        while it < self.num_cases(si) {
            let case_bits = self.int_constant(Operand::Constant(self.case_value(si, it))).map(|(bits, _)| bits);
            // Found case matching a constant operand?
            if ci.is_some() && case_bits == ci.map(|(bits, _)| bits) {
                the_only_dest = Some(self.case_successor(si, it));
                break;
            }

            // Check to see if this branch is going to the same place as the
            // default dest.  If so, eliminate it as an explicit
            // compare.
            if self.case_successor(si, it) == default_dest {
                let md = self.valid_branch_weights(si);
                let n_cases = self.num_cases(si);
                // Fold the case metadata into the default if there will be any
                // branches left, unless the metadata doesn't
                // match the switch.
                if n_cases > 1
                    && let Some((mut weights, origin)) = md
                {
                    // Merge weight of this case to the default weight.
                    let idx = it;

                    // `fitWeights(Weights)` here discards its result.
                    weights[0] = weights[0].wrapping_add(weights[idx + 1]);
                    // Remove weight for this case.
                    let last = weights.len() - 1;
                    weights.swap(idx + 1, last);
                    weights.pop();
                    let fitted = Self::fit_weights(&weights);
                    self.set_branch_weights(si, &fitted, origin);
                }
                // Remove this entry.
                self.remove_predecessor(default_dest, bb, false);
                self.switch_remove_case(si, it);

                // Removing this case may have made the condition constant. In
                // that case, update CI and restart iteration
                // through the cases.
                if let Some(new_ci) = self.int_constant(self.op(si, 0)) {
                    ci = Some(new_ci);
                    it = 0;
                }

                changed = true;
                continue;
            }

            // Otherwise, check to see if the switch only branches to one
            // destination. We do this by resetting "TheOnlyDest" to
            // null when we find two non-equal destinations.
            if Some(self.case_successor(si, it)) != the_only_dest {
                the_only_dest = None;
            }

            // Increment this iterator as we haven't removed the case.
            it += 1;
        }

        if ci.is_some() && the_only_dest.is_none() {
            // Branching on a constant, but not any of the cases, go to the
            // default successor.
            the_only_dest = Some(self.default_dest(si));
        }

        // If we found a single destination that we can fold the switch into, do
        // so now.
        if let Some(the_only_dest) = the_only_dest {
            // Insert the new branch.
            let br = self
                .unit
                .function
                .create_instruction(Opcode::Br, void, vec![Operand::Block(the_only_dest)], Flags::default(), None);
            self.unit.function.insert(br, Position::Before(si)).expect("a placed switch");

            // Remove entries from PHI nodes which we no longer branch to...
            let mut succ_to_keep = Some(the_only_dest);
            for succ in self.switch_edges(si) {
                if succ != the_only_dest {
                    self.cfg_changed = true;
                }
                // Found case matching a constant operand?
                if Some(succ) == succ_to_keep {
                    // Don't modify the first branch to TheOnlyDest
                    succ_to_keep = None;
                } else {
                    self.remove_predecessor(succ, bb, false);
                }
            }

            // Delete the old switch.
            self.erase(si);
            return true;
        }

        if self.num_cases(si) == 1 {
            // Otherwise, we can fold this switch into a conditional branch
            // instruction if it has only one non-default destination.
            let (first_value, first_successor) = (self.case_value(si, 0), self.case_successor(si, 0));
            let default = self.default_dest(si);
            let condition =
                self.create_icmp(si, IntPredicate::Eq, self.op(si, 0), Operand::Constant(first_value), "cond");

            // Insert the new branch.
            let operands = vec![condition, Operand::Block(first_successor), Operand::Block(default)];
            let new_br = self.unit.function.create_instruction(Opcode::Br, void, operands, Flags::default(), None);
            self.unit.function.insert(new_br, Position::Before(si)).expect("a placed switch");
            if let Some((weights, _)) = self.branch_weights(si)
                && weights.len() == 2
            {
                let (def_weight, case_weight) = (weights[0] as u32, weights[1] as u32);
                // The TrueWeight should be the weight for the single case of
                // SI.
                self.set_branch_weights(new_br, &[case_weight, def_weight], false);
            }

            // Update make.implicit metadata to the newly-created conditional
            // branch.
            let implicit =
                self.unit.function.instruction(si).metadata.iter().find(|(kind, _)| kind == "make.implicit").cloned();
            if let Some((kind, node)) = implicit {
                self.unit.function.annotate(new_br, &kind, node);
            }

            // Delete the old switch.
            self.erase(si);
            return true;
        }
        changed
    }

    /// `fitWeights`: push the weights right to fit in 32 bits.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/ProfDataUtils.cpp#L94>
    fn fit_weights(weights: &[u64]) -> Vec<u32> {
        let max = weights.iter().copied().max().unwrap_or(0);
        if max > u64::from(u32::MAX) {
            let offset = 32 - max.leading_zeros();
            weights.iter().map(|&value| (value >> offset) as u32).collect()
        } else {
            weights.iter().map(|&value| value as u32).collect()
        }
    }
}
