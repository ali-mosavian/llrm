//! An expression graph that ends in a `trunc` is evaluated at the narrow
//! width, as LLVM's `TruncInstCombine` (a part of `AggressiveInstCombine`).
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `TruncInstCombine.cpp`, each function
//! linking the lines it translates. For each `trunc` of the function, in
//! reverse order of the walk that found them, `build_trunc_expression_graph`
//! collects the instructions it reads, as long as they are `add`, `sub`,
//! `mul`, the bit operations, shifts, `udiv`, `urem`, `select`, `phi`, `umin`
//! or `umax`, bounded by `trunc`, `zext`, `sext` and constants.
//! `get_min_bit_width` finds the narrowest legal width every one of them can be
//! evaluated at, from the shift amounts and the known bits;
//! `reduce_expression_graph` then makes the graph again at that width, replaces
//! the `trunc` and erases the old instructions.
//!
//! ```text
//!   before                          after
//!   %a = zext i8 %x to i32          %c = add i8 %x, %y
//!   %b = zext i8 %y to i32          (the trunc's uses take %c)
//!   %s = add i32 %a, %b
//!   %c = trunc i32 %s to i8
//! ```
//!
//! DEVIATE (anything MIR or Rust lacks), numbered after `float2int`'s:
//! 34. MIR has no lane operations, so `insertelement`, `extractelement` and
//!     `shufflevector` are not in the graph, and a `trunc` of a vector is not
//!     tried (its width rules are the scalar's only when no vector is made).
//! 35. `IRBuilder` folds an operation on constants; MIR has none (as DEVIATE 30
//!     of `float2int`): an operation of two constants is made as an
//!     instruction. An integer wider than 128 bits is not tried (`KnownBits` is
//!     128 bits wide).
//! 36. `computeKnownBits(V)` is asked with the data layout alone: no assumption
//!     cache and no context instruction.
//! 37. A `umin`/`umax` made here is declared through [`Declared`], which the
//!     module has after the pass: a later `trunc` of the same run that reads it
//!     sees an unknown call and leaves the graph alone.
//!
//! # Example
//! The graph above is one `add i8`; see `trunc_inst_combine_tests.rs`.
//!
//! # Runs
//! As `aggressive-instcombine@678`, after `instcombine` and before `tailrec`
//! (`PassBuilderPipelines.cpp#L678`); not at -O1, whose simplification pipeline
//! has none. The folds of `foldUnusualPatterns` are not translated yet: the
//! slot is Partial.
//!
//! # Needs
//! The dominator tree, to leave unreachable blocks alone.
//!
//! # Keeps
//! The dominators, loops and shape: the CFG is not touched.

use llrm_mir::context::{Constant, ConstantExpr, ConstantKind, Context, mask};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::knownbits::KnownBits;
use llrm_mir::memory;
use llrm_mir::module::{BlockId, Function, GlobalValue, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CallInfo, CastOp, Flags, Opcode};
use llrm_mir::passes::{Analyses, Declared, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::{Type, TypeId};
use llrm_mir::valuetracking::Query;
use llrm_mir::valuetracking::{known, signbits};

use crate::lcssa::{arms, from_arms};

/// `TruncInstCombine::Info`
/// ([AggressiveInstCombineInternal.h](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/AggressiveInstCombineInternal.h)).
#[derive(Clone, Copy, Default)]
struct Info {
    /// The narrowest width this instruction's users need it valid at.
    valid_bit_width: u32,
    /// The narrowest width the instruction can be evaluated at.
    min_bit_width: u32,
    /// The instruction made at the narrow width.
    new_value: Option<Operand>,
}

/// The pass: `TruncInstCombine::run`
/// ([TruncInstCombine.cpp:560](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/TruncInstCombine.cpp#L560)).
pub struct TruncInstCombine;

impl FunctionPass for TruncInstCombine {
    fn name(&self) -> &'static str {
        "trunc-inst-combine"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("aggressive-instcombine")
    }

    fn run(
        &mut self,
        unit: &mut Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        if unit.function.is_declaration() {
            return PreservedAnalyses::all();
        }
        let shape = analyses.get::<llrm_analysis::cfg::Shape>(unit.context, unit.layout, unit.function);
        let reachable: IdSet<BlockId> = unit
            .function
            .layout()
            .iter()
            .copied()
            .filter(|&block| shape.dominance.tree(unit.function).is_reachable_from_entry(block))
            .collect();
        drop(shape);
        let globals = &analyses.outer().globals;
        let mut state = State::new(unit.context, unit.layout, unit.function, unit.declared, globals);
        if !state.run(&reachable) {
            return PreservedAnalyses::all();
        }
        PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>().preserve::<llrm_analysis::cfg::Shape>()
    }
}

struct State<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    declared: &'a mut Declared,
    globals: &'a [GlobalValue],
    /// `Worklist`: the truncs left to try.
    worklist: Vec<InstId>,
    /// `InstInfoMap`, a `MapVector`: the instructions of the graph, operands
    /// before their users.
    order: Vec<InstId>,
    info: IdMap<InstId, Info>,
    current: Option<InstId>,
}

impl<'a> State<'a> {
    fn new(
        context: &'a mut Context,
        layout: &'a DataLayout,
        function: &'a mut Function,
        declared: &'a mut Declared,
        globals: &'a [GlobalValue],
    ) -> Self {
        Self {
            context,
            layout,
            function,
            declared,
            globals,
            worklist: Vec::new(),
            order: Vec::new(),
            info: IdMap::new(),
            current: None,
        }
    }

    /// `run`: try every `trunc` of the reachable blocks.
    fn run(
        &mut self,
        reachable: &IdSet<BlockId>,
    ) -> bool {
        let mut made_ir_change = false;
        // Collect all TruncInst in the function into the Worklist for
        // evaluating.
        self.worklist = self
            .function
            .walk()
            .filter(|&(block, _)| reachable.contains(&block))
            .map(|(_, inst)| inst)
            .filter(|&inst| matches!(
                self.function.instruction(inst).opcode,
                Opcode::Cast(CastOp::Trunc)
            ))
            .collect();
        // Process all TruncInst in the Worklist, for each instruction:
        //   1. Check if it dominates an eligible expression graph to be
        //      reduced.
        //   2. Create a reduced expression graph and replace the old one with
        //      it.
        while let Some(current) = self.worklist.pop() {
            self.current = Some(current);
            if let Some(width) = self.get_best_truncated_type() {
                self.reduce_expression_graph(width);
                made_ir_change = true;
            }
        }
        made_ir_change
    }

    fn current(&self) -> InstId {
        self.current.expect("a trunc is being tried")
    }

    fn query(&self) -> Query<'_> {
        // DEVIATE 36.
        Query { globals: self.globals, ..Query::bare(self.context, self.function, self.layout) }
    }

    fn known_bits(
        &self,
        operand: Operand,
    ) -> KnownBits {
        known::compute_known_bits(&self.query(), operand, 0)
    }

    fn bits_of(
        &self,
        operand: Operand,
    ) -> Option<u32> {
        let ty = self.function.operand_type(self.context, operand)?;
        self.context.types.int_bits(ty)
    }

    /// The instruction that defines `operand`, if one does.
    fn defining(
        &self,
        operand: Operand,
    ) -> Option<InstId> {
        match operand {
            Operand::Value(value) => match self.function.value(value).def {
                ValueDef::Instruction(inst) => Some(inst),
                ValueDef::Argument(_) => None,
            },
            _ => None,
        }
    }

    /// `IntrinsicInst::getIntrinsicID` of a `umin` or `umax`: the call's
    /// intrinsic, if it is one.
    fn min_max(
        &self,
        inst: InstId,
    ) -> bool {
        matches!(
            self.intrinsic(inst),
            Some(Intrinsic::MinMax { signed: false, .. })
        )
    }

    fn intrinsic(
        &self,
        inst: InstId,
    ) -> Option<Intrinsic> {
        let global = memory::callee(self.context, self.function, inst)?;
        self.globals.get(global.0 as usize)?.intrinsic()
    }

    /// `isRelevantOperand`
    /// ([TruncInstCombine.cpp:51](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/TruncInstCombine.cpp#L51)):
    /// whether operand `index` of `inst` is reducible. The operands of a
    /// `phi` that name blocks, and a call's callee, are not values.
    fn is_relevant_operand(
        &self,
        inst: InstId,
        index: usize,
    ) -> bool {
        let instruction = self.function.instruction(inst);
        match &instruction.opcode {
            // These CastInst are considered leaves of the evaluated
            // expression, thus, their operands are not relevant.
            Opcode::Cast(_) => false,
            Opcode::Binary(_) => true,
            Opcode::Select => index != 0,
            Opcode::Phi => index % 2 == 0,
            Opcode::Call(_) => self.min_max(inst) && index + 1 < instruction.operands.len(),
            other => unreachable!("Unreachable! {other:?}"),
        }
    }

    /// `getRelevantOperands`: the relevant operands of `inst`.
    fn relevant_operands(
        &self,
        inst: InstId,
    ) -> Vec<Operand> {
        self.function
            .instruction(inst)
            .operands
            .iter()
            .enumerate()
            .filter(|&(index, _)| self.is_relevant_operand(inst, index))
            .map(|(_, &operand)| operand)
            .collect()
    }

    /// `buildTruncExpressionGraph`
    /// ([TruncInstCombine.cpp:105](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/TruncInstCombine.cpp#L105)):
    /// the instructions the trunc reads, or false where one of them cannot be
    /// reduced.
    fn build_trunc_expression_graph(&mut self) -> bool {
        let mut worklist: Vec<Operand> = Vec::new();
        let mut stack: Vec<InstId> = Vec::new();
        // Clear old instructions info.
        self.order.clear();
        self.info.clear();
        worklist.push(self.function.instruction(self.current()).operands[0]);

        while let Some(&curr) = worklist.last() {
            if matches!(curr, Operand::Constant(_)) {
                worklist.pop();
                continue;
            }
            let Some(inst) = self.defining(curr) else { return false };
            if stack.last() == Some(&inst) {
                // Already handled all instruction operands, can remove it from
                // both the Worklist and the Stack, and add it to the
                // instruction info map.
                worklist.pop();
                stack.pop();
                if self.info.insert(inst, Info::default()).is_none() {
                    self.order.push(inst);
                }
                continue;
            }
            if self.info.contains_key(&inst) {
                worklist.pop();
                continue;
            }
            // Add the instruction to the stack before start handling its
            // operands.
            stack.push(inst);
            match &self.function.instruction(inst).opcode {
                // trunc(trunc(x)) -> trunc(x)
                // trunc(ext(x)) -> ext(x) if the source type is smaller than
                // the new dest
                // trunc(ext(x)) -> trunc(x) if the source type is larger than
                // the new dest
                Opcode::Cast(CastOp::Trunc | CastOp::ZExt | CastOp::SExt) => {}
                Opcode::Binary(
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Mul
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::Shl
                    | BinaryOp::LShr
                    | BinaryOp::AShr
                    | BinaryOp::UDiv
                    | BinaryOp::URem,
                )
                | Opcode::Select => worklist.extend(self.relevant_operands(inst)),
                Opcode::Phi => {
                    // Add only operands not in Stack to prevent cycle.
                    let operands = self.relevant_operands(inst);
                    worklist.extend(
                        operands.into_iter().filter(|&op| self.defining(op).is_none_or(|def| !stack.contains(&def))),
                    );
                }
                Opcode::Call(_) if self.min_max(inst) => worklist.extend(self.relevant_operands(inst)),
                // TODO in the pin: sdiv, srem ...
                _ => return false,
            }
        }
        true
    }

    /// `getMinBitWidth`
    /// ([TruncInstCombine.cpp:197](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/TruncInstCombine.cpp#L197)).
    fn get_min_bit_width(&mut self) -> u32 {
        let current = self.current();
        let src = self.function.instruction(current).operands[0];
        let trunc_bit_width = self.context.types.int_bits(self.function.instruction(current).ty).expect("an integer");
        let orig_bit_width = self.bits_of(src).expect("an integer");
        let Some(src_inst) = self.defining(src) else { return trunc_bit_width };

        let mut worklist: Vec<Operand> = vec![src];
        let mut stack: Vec<InstId> = Vec::new();
        self.info.get_or_insert_with(src_inst, Info::default).valid_bit_width = trunc_bit_width;

        while let Some(&curr) = worklist.last() {
            let Some(inst) = self.defining(curr) else {
                worklist.pop();
                continue;
            };
            let operands = self.relevant_operands(inst);
            if stack.last() == Some(&inst) {
                // Already handled all instruction operands, can remove it from
                // both, the Worklist and the Stack, and update MinBitWidth.
                worklist.pop();
                stack.pop();
                let widest = operands
                    .iter()
                    .filter_map(|&op| self.defining(op))
                    .map(|op| self.info.get(&op).map_or(0, |info| info.min_bit_width))
                    .fold(0, u32::max);
                let info = self.info.get_or_insert_with(inst, Info::default);
                info.min_bit_width = info.min_bit_width.max(widest);
                continue;
            }
            // Add the instruction to the stack before start handling its
            // operands.
            stack.push(inst);
            let info = self.info.get_or_insert_with(inst, Info::default);
            let valid_bit_width = info.valid_bit_width;
            // Update minimum bit-width before handling its operands. This is
            // required when the instruction is part of a loop.
            info.min_bit_width = info.min_bit_width.max(info.valid_bit_width);
            for op in operands {
                if let Some(def) = self.defining(op) {
                    // If we already calculated the minimum
                    // // bit-width for this valid bit-width, or for a smaller
                    // valid bit-width, then just keep the answer we already
                    // calculated.
                    let op_width = self.info.get(&def).map_or(0, |info| info.valid_bit_width);
                    if op_width >= valid_bit_width {
                        continue;
                    }
                    self.info.get_or_insert_with(def, Info::default).valid_bit_width = valid_bit_width;
                    worklist.push(op);
                }
            }
        }
        let mut min_bit_width = self.info.get(&src_inst).map_or(0, |info| info.min_bit_width);
        assert!(min_bit_width >= trunc_bit_width);

        if min_bit_width > trunc_bit_width {
            // Use the smallest integer type in the range [MinBitWidth,
            // OrigBitWidth).
            min_bit_width = (min_bit_width..=self.layout.largest_legal_integer())
                .find(|&bits| self.layout.legal_integer(bits))
                .unwrap_or(orig_bit_width);
        } else {
            // In this case the expression can be evaluated with the trunc
            // instruction destination type, and trunc instruction can be
            // omitted. However, we should not perform the evaluation if the
            // original type is a legal scalar type and the target type is
            // illegal.
            let from_legal = min_bit_width == 1 || self.layout.legal_integer(orig_bit_width);
            let to_legal = min_bit_width == 1 || self.layout.legal_integer(min_bit_width);
            if from_legal && !to_legal {
                return orig_bit_width;
            }
        }
        min_bit_width
    }

    /// `getBestTruncatedType`
    /// ([TruncInstCombine.cpp:305](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/TruncInstCombine.cpp#L305)):
    /// the width to reduce the graph to, if there is one.
    fn get_best_truncated_type(&mut self) -> Option<u32> {
        let current = self.current();
        let source = self.function.instruction(current).operands[0];
        // DEVIATE 34 and 35.
        let orig_bit_width = self.bits_of(source).filter(|&bits| bits <= 128)?;
        self.context.types.int_bits(self.function.instruction(current).ty)?;
        if !self.build_trunc_expression_graph() {
            return None;
        }
        // We don't want to duplicate instructions, which isn't profitable.
        // Thus, we can't shrink something that has multiple uses, unless all
        // uses can be reduced and all users are post-dominated by the trunc
        // instruction, i.e., were visited during the expression evaluation.
        let mut desired_bit_width = 0;
        for &inst in &self.order {
            let Some(result) = self.function.instruction(inst).result else { continue };
            if self.function.users(result).has_one_use() {
                continue;
            }
            let is_ext_inst =
                matches!(
                    self.function.instruction(inst).opcode,
                    Opcode::Cast(CastOp::ZExt | CastOp::SExt)
                );
            for used in self.function.users(result).iter() {
                if used.user != current
                    && (!self.info.contains_key(&used.user)
                        || !self.is_relevant_operand(used.user, used.index as usize))
                {
                    if !is_ext_inst {
                        return None;
                    }
                    // If this is an extension from the dest type, we can
                    // eliminate it, even if it has multiple users. Thus,
                    // update the DesiredBitWidth and validate all extension
                    // instructions agrees on same DesiredBitWidth.
                    let ext_width = self.bits_of(self.function.instruction(inst).operands[0])?;
                    if desired_bit_width != 0 && desired_bit_width != ext_width {
                        return None;
                    }
                    desired_bit_width = ext_width;
                }
            }
        }

        // Initialize MinBitWidth for shift instructions with the minimum
        // number that is greater than shift amount (i.e. shift amount + 1).
        // For `lshr` adjust MinBitWidth so that all potentially truncated bits
        // of the value-to-be-shifted are zeros. For `ashr` adjust MinBitWidth
        // so that all potentially truncated bits of the value-to-be-shifted
        // are sign bits (all zeros or ones) and even one (first) untruncated
        // bit is sign bit. Exit early if MinBitWidth is not less than original
        // bitwidth.
        for inst in self.order.clone() {
            let instruction = self.function.instruction(inst);
            let min_bit_width = match instruction.opcode {
                Opcode::Binary(op @ (BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr)) => {
                    let (value, amount) = (instruction.operands[0], instruction.operands[1]);
                    let max = self.known_bits(amount).max_value();
                    let all_ones = if orig_bit_width == 128 { u128::MAX } else { (1u128 << orig_bit_width) - 1 };
                    let mut min_bit_width =
                        max.min(all_ones - 1).saturating_add(1).min(u128::from(orig_bit_width)) as u32;
                    if min_bit_width == orig_bit_width {
                        return None;
                    }
                    if op == BinaryOp::LShr {
                        min_bit_width = min_bit_width.max(self.known_bits(value).count_max_active_bits());
                    }
                    if op == BinaryOp::AShr {
                        let sign_bits = signbits::compute_num_sign_bits(&self.query(), value, 0);
                        min_bit_width = min_bit_width.max(orig_bit_width - sign_bits + 1);
                    }
                    if min_bit_width >= orig_bit_width {
                        return None;
                    }
                    min_bit_width
                }
                Opcode::Binary(BinaryOp::UDiv | BinaryOp::URem) => self.widest_active(inst, 0..2, orig_bit_width)?,
                Opcode::Call(_) => {
                    // `umin` and `umax`: the arguments.
                    self.widest_active(inst, 0..2, orig_bit_width)?
                }
                _ => continue,
            };
            self.info.get_or_insert_with(inst, Info::default).min_bit_width = min_bit_width;
        }

        // Calculate minimum allowed bit-width allowed for shrinking the
        // currently visited truncate's operand.
        let min_bit_width = self.get_min_bit_width();
        // Check that we can shrink to smaller bit-width than original one and
        // that it is similar to the DesiredBitWidth is such exists.
        if min_bit_width >= orig_bit_width || (desired_bit_width != 0 && desired_bit_width != min_bit_width) {
            return None;
        }
        Some(min_bit_width)
    }

    /// The most active bits any operand of `inst` in `range` can have; none
    /// where that is not less than `orig_bit_width`.
    fn widest_active(
        &self,
        inst: InstId,
        range: std::ops::Range<usize>,
        orig_bit_width: u32,
    ) -> Option<u32> {
        let mut min_bit_width = 0;
        for index in range {
            let operand = self.function.instruction(inst).operands[index];
            min_bit_width = min_bit_width.max(self.known_bits(operand).count_max_active_bits());
            if min_bit_width >= orig_bit_width {
                return None;
            }
        }
        Some(min_bit_width)
    }

    /// A new instruction before `at`, named `name`; its value.
    fn emit(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        flags: Flags,
        at: InstId,
    ) -> Operand {
        let new = self.function.create_instruction(opcode, ty, operands, flags, None);
        self.function.insert(new, Position::Before(at)).expect("a placed instruction");
        Operand::Value(self.function.instruction(new).result.expect("a value"))
    }

    /// `IRBuilder::CreateIntCast` before `at`.
    fn create_int_cast(
        &mut self,
        operand: Operand,
        to: TypeId,
        signed: bool,
        at: InstId,
    ) -> Operand {
        let (from_bits, to_bits) =
            (self.bits_of(operand).expect("an integer"), self.context.types.int_bits(to).expect("an integer"));
        if from_bits == to_bits {
            return operand;
        }
        let cast = if to_bits < from_bits {
            CastOp::Trunc
        } else if signed {
            CastOp::SExt
        } else {
            CastOp::ZExt
        };
        self.emit(Opcode::Cast(cast), to, vec![operand], Flags::default(), at)
    }

    /// `getReducedOperand`
    /// ([TruncInstCombine.cpp:471](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/TruncInstCombine.cpp#L471)):
    /// the narrow form of `operand`: a constant truncated, or the instruction
    /// made for it.
    fn get_reduced_operand(
        &mut self,
        operand: Operand,
        ty: TypeId,
    ) -> Operand {
        if let Operand::Constant(id) = operand {
            let width = self.context.types.int_bits(ty).expect("an integer");
            let kind = match &self.context.get(id).kind {
                ConstantKind::Int(bits) => ConstantKind::Int(bits & mask(width)),
                ConstantKind::Undef => ConstantKind::Undef,
                ConstantKind::Poison => ConstantKind::Poison,
                ConstantKind::Zero | ConstantKind::Null => ConstantKind::Zero,
                _ => ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::Trunc, value: id }),
            };
            return Operand::Constant(self.context.constant(Constant { ty, kind }));
        }
        let inst = self.defining(operand).expect("an instruction");
        self.info.get(&inst).and_then(|info| info.new_value).expect("the operand was evaluated")
    }

    /// `reduceExpressionGraph`
    /// ([TruncInstCombine.cpp:487](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/AggressiveInstCombine/TruncInstCombine.cpp#L487)):
    /// the graph made again at `width` bits, replacing the trunc.
    fn reduce_expression_graph(
        &mut self,
        width: u32,
    ) {
        let scalar = self.context.types.int(width);
        // Pairs of old and new phi-nodes.
        let mut old_new_phis: Vec<(InstId, InstId)> = Vec::new();
        for inst in self.order.clone() {
            let instruction = self.function.instruction(inst).clone();
            let new_value = match instruction.opcode.clone() {
                Opcode::Cast(cast @ (CastOp::Trunc | CastOp::ZExt | CastOp::SExt)) => {
                    // If the source type of the cast is the type we're trying
                    // for then we can just return the source. There's no need
                    // to insert it because it is not new.
                    if self.function.operand_type(self.context, instruction.operands[0]) == Some(scalar) {
                        assert!(cast != CastOp::Trunc, "Cannot reach here with TruncInst");
                        self.info.get_or_insert_with(inst, Info::default).new_value = Some(instruction.operands[0]);
                        continue;
                    }
                    // Otherwise, must be the same type of cast, so just
                    // reinsert a new one. This also handles the case of
                    // zext(trunc(x)) -> zext(x).
                    let new = self.create_int_cast(instruction.operands[0], scalar, cast == CastOp::SExt, inst);
                    // Update Worklist entries with new value if needed. There
                    // are three possible changes to the Worklist:
                    // 1. Update Old-TruncInst -> New-TruncInst.
                    // 2. Remove Old-TruncInst (if New node is not TruncInst).
                    // 3. Add New-TruncInst (if Old node was not TruncInst).
                    let new_trunc = self
                        .defining(new)
                        .filter(|&one| matches!(
                            self.function.instruction(one).opcode,
                            Opcode::Cast(CastOp::Trunc)
                        ));
                    match (self.worklist.iter().position(|&one| one == inst), new_trunc) {
                        (Some(at), Some(new_trunc)) => self.worklist[at] = new_trunc,
                        (Some(at), None) => {
                            self.worklist.remove(at);
                        }
                        (None, Some(new_trunc)) => self.worklist.push(new_trunc),
                        (None, None) => {}
                    }
                    new
                }
                Opcode::Binary(op) => {
                    let lhs = self.get_reduced_operand(instruction.operands[0], scalar);
                    let rhs = self.get_reduced_operand(instruction.operands[1], scalar);
                    // Preserve `exact` flag since truncation doesn't change
                    // exactness.
                    let mut flags = Flags::default();
                    if instruction.flags.contains(Flags::EXACT) {
                        flags.insert(Flags::EXACT);
                    }
                    self.emit(Opcode::Binary(op), scalar, vec![lhs, rhs], flags, inst)
                }
                Opcode::Select => {
                    let lhs = self.get_reduced_operand(instruction.operands[1], scalar);
                    let rhs = self.get_reduced_operand(instruction.operands[2], scalar);
                    self.emit(Opcode::Select, scalar, vec![instruction.operands[0], lhs, rhs], Flags::default(), inst)
                }
                Opcode::Phi => {
                    let new = self.emit(Opcode::Phi, scalar, Vec::new(), Flags::default(), inst);
                    old_new_phis.push((inst, self.defining(new).expect("a phi")));
                    new
                }
                Opcode::Call(info) => {
                    let lhs = self.get_reduced_operand(instruction.operands[0], scalar);
                    let rhs = self.get_reduced_operand(instruction.operands[1], scalar);
                    let max = matches!(
                        self.intrinsic(inst),
                        Some(Intrinsic::MinMax { max: true, .. })
                    );
                    self.create_binary_intrinsic(max, scalar, lhs, rhs, &info, inst)
                }
                other => unreachable!("Unhandled instruction {other:?}"),
            };
            self.info.get_or_insert_with(inst, Info::default).new_value = Some(new_value);
            if let (Some(new), Some(old)) = (self.defining(new_value), instruction.result) {
                if let Some(result) = self.function.instruction(new).result {
                    if self.function.value(old).name.is_some() {
                        self.function.take_name(old, result);
                    }
                }
            }
        }

        for (old, new) in old_new_phis.clone() {
            let mut incoming = Vec::new();
            for (value, block) in arms(self.function, old) {
                incoming.push((self.get_reduced_operand(value, scalar), block));
            }
            self.function.set_operands(new, from_arms(&incoming));
        }

        let current = self.current();
        let mut result = self.get_reduced_operand(self.function.instruction(current).operands[0], scalar);
        let dest = self.function.instruction(current).ty;
        if self.function.operand_type(self.context, result) != Some(dest) {
            let name = self.function.instruction(current).result.expect("a trunc has a value");
            result = self.create_int_cast(result, dest, false, current);
            if let Some(new) = self.defining(result).and_then(|one| self.function.instruction(one).result) {
                self.function.take_name(name, new);
            }
        }
        let value = self.function.instruction(current).result.expect("a trunc has a value");
        self.function.replace_all_uses_with(value, result);

        // Erase old expression graph, which was replaced by the reduced
        // expression graph.
        self.function.erase(current).expect("its uses were replaced");
        // First, erase old phi-nodes and its uses.
        for (old, _) in old_new_phis {
            let value = self.function.instruction(old).result.expect("a phi has a value");
            let poison = Operand::Constant(
                self.context.constant(Constant { ty: self.function.instruction(old).ty, kind: ConstantKind::Poison }),
            );
            self.function.replace_all_uses_with(value, poison);
            self.order.retain(|&one| one != old);
            self.info.remove(&old);
            self.function.erase(old).expect("its uses were replaced");
        }
        // Now we have expression graph turned into dag. We iterate backward,
        // which means we visit the instruction before we visit any of its
        // operands, this way, when we get to the operand, we already removed
        // the instructions (from the expression dag) that uses it.
        for &inst in self.order.iter().rev() {
            // We still need to check that the instruction has no users before
            // we erase it, because {SExt, ZExt}Inst Instruction might have
            // other users that was not reduced, in such case, we need to keep
            // that instruction.
            let result = self.function.instruction(inst).result.expect("a value");
            if self.function.users(result).is_empty() {
                self.function.erase(inst).expect("it has no users");
            } else {
                assert!(
                    matches!(self.function.instruction(inst).opcode, Opcode::Cast(CastOp::SExt | CastOp::ZExt)),
                    "Only {{SExt, ZExt}}Inst might have unreduced users"
                );
            }
        }
    }

    /// `IRBuilder::CreateBinaryIntrinsic(umin | umax, lhs, rhs)` before `at`:
    /// a call to the declaration `llvm.umin.iN` / `llvm.umax.iN`, DEVIATE 37.
    fn create_binary_intrinsic(
        &mut self,
        max: bool,
        ty: TypeId,
        lhs: Operand,
        rhs: Operand,
        old: &CallInfo,
        at: InstId,
    ) -> Operand {
        let function_type =
            self.context.types.intern(Type::Function { returns: ty, parameters: vec![ty, ty], variadic: false });
        let width = self.context.types.int_bits(ty).expect("an integer");
        let name = format!("llvm.{}.i{width}", if max { "umax" } else { "umin" });
        let callee = self.declared.declare(&name, function_type);
        let pointer = self.context.types.ptr(0);
        let callee =
            Operand::Constant(self.context.constant(Constant { ty: pointer, kind: ConstantKind::Global(callee) }));
        let info = CallInfo {
            function_type,
            calling_convention: old.calling_convention,
            return_attrs: Vec::new(),
            argument_attrs: vec![Vec::new(); 2],
            attrs: Vec::new(),
            tail: Default::default(),
        };
        self.emit(Opcode::Call(Box::new(info)), ty, vec![lhs, rhs, callee], Flags::default(), at)
    }
}

#[cfg(test)]
#[path = "trunc_inst_combine_tests.rs"]
mod tests;
