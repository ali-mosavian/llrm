//! Floating-point arithmetic that only ever meets integers is done on
//! integers, as LLVM's `Float2Int`.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `Float2Int.cpp`, each function linking
//! the lines it translates. The roots are the `fptoui`, `fptosi` and the
//! `fcmp` whose predicate has an integer counterpart. `walk_backwards` walks
//! the use-def graph up from them and joins the instructions that meet into
//! one class; `walk_forwards` works out, for each, the range of integers it
//! can hold, from `uitofp`/`sitofp` inputs and the constants; a class whose
//! range fits the mantissa of its float type and is not poison is rewritten
//! on the smallest legal integer type that holds it.
//!
//! ```text
//!   before                          after
//!   %f = sitofp i8 %a to float      %e = sext i8 %a to i32
//!   %g = fadd float %f, 1.0         %g = add i32 %e, 1
//!   %i = fptosi float %g to i32     (the root's uses take %g)
//! ```
//!
//! DEVIATE (anything MIR or Rust lacks), numbered after `tailcallelim`'s:
//! 29. Float constants are read as `f64`: `float` and `double` constants are
//!     exact in it, so `roundToIntegral`, the finite and negative-zero tests
//!     and `convertToInteger` of an integral value into 65 signed bits are
//!     `round_ties_even`, `is_finite`, `is_sign_negative` and a range test. An
//!     `x86_fp80` constant is held by MIR as the nearest double, so it is
//!     refused as `badRange` (`float_constant_is_exact`).
//! 30. `IRBuilder` folds an operation on constants; MIR has no folder, and
//!     every operand of the new operation is a constant only where the original
//!     was foldable and earlier passes have folded it.
//! 31. `convert` is iterative, not recursive: a long chain of `fadd` would
//!     overflow the Rust stack where C++'s is trusted to the machine's. The
//!     order the new instructions are made in differs (operands last to first
//!     on the stack), the order of `ConvertedInsts` stays operands before
//!     users, which `cleanup` relies on.
//! 32. A `uitofp`/`sitofp` of an integer wider than 65 bits is `badRange` at
//!     once: `castOp` gives it a range of its own width, and `validateRange`
//!     turns any range wider than 65 bits into `badRange`.
//! 33. `EquivalenceClasses` is a union-find over instruction ids; classes are
//!     visited in the order their first member was joined, where the C++ set is
//!     ordered by address.
//!
//! # Example
//! `(int)((float)a + 1.0f)` for `a` of `i8` becomes an `add` of `i32` and no
//! float; see `float2int_tests.rs`.
//!
//! # Runs
//! As `float2int@1594`, in the late list after the second `simplifycfg` slot
//! and before `instsimplify`, at every level
//! (`PassBuilderPipelines.cpp#L1594`).
//!
//! # Needs
//! The dominator tree, to leave unreachable blocks alone.
//!
//! # Keeps
//! The dominators, loops and shape: the CFG is not touched.

use std::collections::VecDeque;

use llrm_mir::constantrange::{PreferredRangeType, Range};
use llrm_mir::context::{ConstantKind, Context};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::{IdMap, IdSet};
use llrm_mir::edit::Position;
use llrm_mir::module::{Function, InstId, Operand, ValueDef};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, FloatPredicate, IntPredicate, Opcode};
use llrm_mir::passes::{Analyses, Dominators, FunctionPass, Loops, PreservedAnalyses, Unit};
use llrm_mir::types::{FloatKind, Type, TypeId};

/// `MaxIntegerBW`: the largest integer width worth dealing with
/// ([Float2Int.cpp:43](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L43)).
const MAX_INTEGER_BW: u32 = 64;

/// The ranges are `MaxIntegerBW + 1` bits wide.
const RANGE_BITS: u32 = MAX_INTEGER_BW + 1;

type ConstRange = Range<u128>;

/// `Float2IntPass::run`
/// ([Float2Int.cpp:498](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L498)).
pub struct Float2Int;

impl FunctionPass for Float2Int {
    fn name(&self) -> &'static str {
        "float2int"
    }

    fn translation(&self) -> Option<&'static str> {
        Some("float2int")
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
        let reachable: IdSet<llrm_mir::module::BlockId> = unit
            .function
            .layout()
            .iter()
            .copied()
            .filter(|&block| shape.dominance.tree(unit.function).is_reachable_from_entry(block))
            .collect();
        drop(shape);
        if !Float2IntState::new(unit.context, unit.layout, unit.function).run_impl(&reachable) {
            return PreservedAnalyses::all();
        }
        PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>().preserve::<llrm_analysis::cfg::Shape>()
    }
}

/// `mapFCmpPred`
/// ([Float2Int.cpp:56](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L56)):
/// the integer predicate a float one maps to, if there is one.
fn map_fcmp_pred(predicate: FloatPredicate) -> Option<IntPredicate> {
    use FloatPredicate::*;
    Some(match predicate {
        Oeq | Ueq => IntPredicate::Eq,
        Ogt | Ugt => IntPredicate::Sgt,
        Oge | Uge => IntPredicate::Sge,
        Olt | Ult => IntPredicate::Slt,
        Ole | Ule => IntPredicate::Sle,
        One | Une => IntPredicate::Ne,
        _ => return None,
    })
}

/// `mapBinOpcode`
/// ([Float2Int.cpp:80](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L80)).
fn map_bin_opcode(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::FAdd => BinaryOp::Add,
        BinaryOp::FSub => BinaryOp::Sub,
        BinaryOp::FMul => BinaryOp::Mul,
        other => unreachable!("Unhandled opcode {other:?}"),
    }
}

/// `badRange`
/// ([Float2Int.cpp:130](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L130)).
fn bad_range() -> ConstRange {
    ConstRange::full(RANGE_BITS)
}

/// `unknownRange`: not worked out yet.
fn unknown_range() -> ConstRange {
    ConstRange::empty(RANGE_BITS)
}

/// The members of the use-def graph's disjoint partitions
/// (`EquivalenceClasses<Instruction *>`), DEVIATE 33.
#[derive(Default)]
struct Classes {
    parent: IdMap<InstId, InstId>,
    joined: Vec<InstId>,
}

impl Classes {
    fn insert(
        &mut self,
        inst: InstId,
    ) {
        if !self.parent.contains_key(&inst) {
            self.parent.insert(inst, inst);
            self.joined.push(inst);
        }
    }

    fn leader(
        &mut self,
        inst: InstId,
    ) -> InstId {
        let mut root = inst;
        while let Some(&up) = self.parent.get(&root) {
            if up == root {
                break;
            }
            root = up;
        }
        let mut at = inst;
        while at != root {
            let up = *self.parent.get(&at).expect("a member");
            self.parent.insert(at, root);
            at = up;
        }
        root
    }

    /// `unionSets`.
    fn union_sets(
        &mut self,
        a: InstId,
        b: InstId,
    ) {
        self.insert(a);
        self.insert(b);
        let (a, b) = (self.leader(a), self.leader(b));
        if a != b {
            self.parent.insert(b, a);
        }
    }

    /// The classes, each with its members in the order they joined.
    fn partitions(&mut self) -> Vec<Vec<InstId>> {
        let mut at: IdMap<InstId, usize> = IdMap::new();
        let mut out: Vec<Vec<InstId>> = Vec::new();
        for inst in self.joined.clone() {
            let leader = self.leader(inst);
            let index = *at.get_or_insert_with(leader, || {
                out.push(Vec::new());
                out.len() - 1
            });
            out[index].push(inst);
        }
        out
    }
}

/// The state of one `runImpl`.
struct Float2IntState<'a> {
    context: &'a mut Context,
    layout: &'a DataLayout,
    function: &'a mut Function,
    classes: Classes,
    /// `SeenInsts`, a `MapVector`: the order is that of first insertion.
    seen_order: Vec<InstId>,
    seen: IdMap<InstId, ConstRange>,
    /// `ConvertedInsts`, a `MapVector`.
    converted_order: Vec<InstId>,
    converted: IdMap<InstId, Operand>,
    /// `Roots`, a `SetVector`.
    roots: Vec<InstId>,
    is_root: IdSet<InstId>,
}

impl<'a> Float2IntState<'a> {
    fn new(
        context: &'a mut Context,
        layout: &'a DataLayout,
        function: &'a mut Function,
    ) -> Self {
        Self {
            context,
            layout,
            function,
            classes: Classes::default(),
            seen_order: Vec::new(),
            seen: IdMap::new(),
            converted_order: Vec::new(),
            converted: IdMap::new(),
            roots: Vec::new(),
            is_root: IdSet::new(),
        }
    }

    /// `runImpl`
    /// ([Float2Int.cpp:470](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L470)).
    fn run_impl(
        mut self,
        reachable: &IdSet<llrm_mir::module::BlockId>,
    ) -> bool {
        self.find_roots(reachable);
        self.walk_backwards();
        self.walk_forwards();
        let modified = self.validate_and_transform();
        if modified {
            self.cleanup();
        }
        modified
    }

    fn is_vector(
        &self,
        ty: TypeId,
    ) -> bool {
        matches!(self.context.types.get(ty), Type::Vector { .. })
    }

    /// `findRoots`
    /// ([Float2Int.cpp:98](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L98)):
    /// the instructions that convert from the FP domain to the integer domain.
    fn find_roots(
        &mut self,
        reachable: &IdSet<llrm_mir::module::BlockId>,
    ) {
        let found: Vec<InstId> = self
            .function
            .walk()
            // Unreachable code can take on strange forms that we are not
            // prepared to handle. For example, an instruction may have itself
            // as an operand.
            .filter(|&(block, _)| reachable.contains(&block))
            .map(|(_, inst)| inst)
            .filter(|&inst| !self.is_vector(self.function.instruction(inst).ty))
            .filter(|&inst| match &self.function.instruction(inst).opcode {
                Opcode::Cast(CastOp::FPToUI | CastOp::FPToSI) => true,
                Opcode::FCmp(predicate) => map_fcmp_pred(*predicate).is_some(),
                _ => false,
            })
            .collect();
        for inst in found {
            if self.is_root.insert(inst) {
                self.roots.push(inst);
            }
        }
    }

    /// `seen`: `inst` has been traversed, and has range `range`.
    fn seen(
        &mut self,
        inst: InstId,
        range: ConstRange,
    ) {
        if self.seen.insert(inst, range).is_none() {
            self.seen_order.push(inst);
        }
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

    /// Whether `operand` is a `ConstantFP`.
    fn is_constant_fp(
        &self,
        operand: Operand,
    ) -> bool {
        let Operand::Constant(id) = operand else { return false };
        let constant = self.context.get(id);
        matches!(self.context.types.get(constant.ty), Type::Float(_))
            && matches!(
                constant.kind,
                ConstantKind::Float(_) | ConstantKind::Null | ConstantKind::Zero
            )
    }

    /// `walkBackwards`
    /// ([Float2Int.cpp:140](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L140)):
    /// a breadth-first walk of the use-def graph; determine the set of nodes
    /// we care about and eagerly determine if some of them are poisonous.
    fn walk_backwards(&mut self) {
        let mut worklist: VecDeque<InstId> = self.roots.iter().copied().collect();
        while let Some(inst) = worklist.pop_back() {
            if self.seen.contains_key(&inst) {
                // Seen already.
                continue;
            }
            match self.function.instruction(inst).opcode.clone() {
                // Path terminated cleanly - use the type of the integer input
                // to seed the analysis.
                Opcode::Cast(cast @ (CastOp::UIToFP | CastOp::SIToFP)) => {
                    let input = self.function.instruction(inst).operands[0];
                    let bits = self
                        .function
                        .operand_type(self.context, input)
                        .and_then(|ty| self.context.types.int_bits(ty))
                        .unwrap_or(u32::MAX);
                    // DEVIATE 32.
                    let range =
                        if bits > RANGE_BITS { bad_range() } else { ConstRange::full(bits).cast_op(cast, RANGE_BITS) };
                    self.seen(inst, range);
                    continue;
                }
                Opcode::FNeg
                | Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul)
                | Opcode::Cast(CastOp::FPToUI | CastOp::FPToSI)
                | Opcode::FCmp(_) => self.seen(inst, unknown_range()),
                // Path terminated uncleanly.
                _ => self.seen(inst, bad_range()),
            }
            for operand in self.function.instruction(inst).operands.clone() {
                if let Some(def) = self.defining(operand) {
                    // Unify def-use chains if they interfere.
                    self.classes.union_sets(inst, def);
                    if self.seen[&inst] != bad_range() {
                        worklist.push_back(def);
                    }
                } else if !self.is_constant_fp(operand) {
                    // Not an instruction or ConstantFP? we can't do anything.
                    self.seen(inst, bad_range());
                }
            }
        }
    }

    /// Whether the floating-point constant `operand`, used by `user`, can be
    /// an integer of `RANGE_BITS` bits without loss: its range, or `bad_range`.
    fn constant_range(
        &self,
        user: InstId,
        operand: Operand,
    ) -> ConstRange {
        let Operand::Constant(id) = operand else { unreachable!("Should have already marked this as badRange!") };
        let constant = self.context.get(id);
        let Type::Float(kind) = *self.context.types.get(constant.ty) else { return bad_range() };
        // DEVIATE 29.
        let value = match (kind, &constant.kind) {
            (FloatKind::X86Fp80, _) => return bad_range(),
            (FloatKind::Float, ConstantKind::Float(bits)) => f64::from(f32::from_bits(*bits as u32)),
            (_, ConstantKind::Float(bits)) => f64::from_bits(*bits),
            _ => 0.0,
        };
        let instruction = self.function.instruction(user);
        let math_operator = matches!(
            instruction.opcode,
            Opcode::FNeg | Opcode::FCmp(_) | Opcode::Binary(BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul)
        );
        // First, weed out obviously incorrect values. Non-finite numbers can't
        // be represented and neither can negative zero, unless we're in fast
        // math mode.
        if !value.is_finite()
            || (value == 0.0 && value.is_sign_negative() && !(math_operator && instruction.flags.contains(Flags::NSZ)))
        {
            return bad_range();
        }
        // Round to an integral value and compare with the original: a value
        // with a fraction can't be represented.
        if value.round_ties_even() != value {
            return bad_range();
        }
        // Although the round above is lossless, we still need to check that
        // the value can be represented in the integer type.
        let limit = 2f64.powi(MAX_INTEGER_BW as i32);
        if value < -limit || value >= limit {
            return bad_range();
        }
        ConstRange::single(RANGE_BITS, (value as i128 as u128) & ((1u128 << RANGE_BITS) - 1))
    }

    /// `calcRange`
    /// ([Float2Int.cpp:204](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L204)):
    /// the result range from the operand ranges; none if it cannot be
    /// calculated yet.
    fn calc_range(
        &self,
        inst: InstId,
    ) -> Option<ConstRange> {
        let mut op_ranges: Vec<ConstRange> = Vec::new();
        for operand in self.function.instruction(inst).operands.clone() {
            if let Some(def) = self.defining(operand) {
                let range = self.seen.get(&def).expect("def not seen before use!");
                if *range == unknown_range() {
                    // Wait until operand range has been calculated.
                    return None;
                }
                op_ranges.push(*range);
            } else {
                let range = self.constant_range(inst, operand);
                if range == bad_range() {
                    return Some(bad_range());
                }
                op_ranges.push(range);
            }
        }
        Some(match self.function.instruction(inst).opcode {
            Opcode::FNeg => ConstRange::single(op_ranges[0].width, 0).sub(&op_ranges[0]),
            Opcode::Binary(op @ (BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul)) => {
                op_ranges[0].binary_op(op, &op_ranges[1])
            }
            // Root-only instructions - we'll only see these if they're the
            // first node in a walk. Note: the cast's output size is ignored
            // here, as that's what the caller expects.
            Opcode::Cast(cast @ (CastOp::FPToUI | CastOp::FPToSI)) => op_ranges[0].cast_op(cast, RANGE_BITS),
            Opcode::FCmp(_) => op_ranges[0].union_with(&op_ranges[1], PreferredRangeType::Smallest),
            ref other => unreachable!("Should have been handled in walkForwards: {other:?}"),
        })
    }

    /// `walkForwards`
    /// ([Float2Int.cpp:305](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L305)):
    /// walk forwards down the list of seen instructions, so we visit defs
    /// before uses.
    fn walk_forwards(&mut self) {
        let mut worklist: VecDeque<InstId> =
            self.seen_order.iter().copied().filter(|inst| self.seen[inst] == unknown_range()).collect();
        while let Some(inst) = worklist.pop_back() {
            match self.calc_range(inst) {
                Some(range) => self.seen(inst, range),
                // Reprocess later.
                None => worklist.push_front(inst),
            }
        }
    }

    /// `validateAndTransform`
    /// ([Float2Int.cpp:322](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L322)):
    /// if there is a valid transform to be done, do it.
    fn validate_and_transform(&mut self) -> bool {
        let mut made_change = false;
        // Iterate over every disjoint partition of the def-use graph.
        for members in self.classes.partitions() {
            let mut range = ConstRange::empty(RANGE_BITS);
            let mut fail = false;
            let mut converted_to_ty: Option<TypeId> = None;
            // For every member of the partition, union all the ranges together.
            for &inst in &members {
                let Some(seen) = self.seen.get(&inst) else { continue };
                range = range.union_with(seen, PreferredRangeType::Smallest);
                // We need to ensure the instruction has no users that have not
                // been seen. If it does, transformation would be illegal.
                //
                // Don't count the roots, as they terminate the graphs.
                if !self.is_root.contains(&inst) {
                    // Set the type of the conversion while we're here.
                    converted_to_ty.get_or_insert(self.function.instruction(inst).ty);
                    if let Some(result) = self.function.instruction(inst).result {
                        fail = self.function.users(result).iter().any(|user| !self.seen.contains_key(&user.user));
                    }
                }
                if fail {
                    break;
                }
            }
            // If the set was empty, or we failed, or the range is poisonous,
            // bail out.
            let Some(converted_to_ty) = converted_to_ty else { continue };
            if members.is_empty() || fail || range.is_full_set() || range.is_sign_wrapped_set() {
                continue;
            }
            // The number of bits required is the maximum of the upper and
            // lower limits, plus one so it can be signed.
            let min_bw = range.get_min_signed_bits() + 1;
            // If we've run off the realms of the exactly representable
            // integers, the floating point result will differ from an integer
            // approximation. `semanticsPrecision` is the mantissa bits plus
            // one for the sign bit.
            let Type::Float(kind) = *self.context.types.get(converted_to_ty) else { continue };
            let max_representable_bits =
                llrm_mir::apfloat::semantics_precision(llrm_mir::apfloat::semantics_of(kind)) - 1;
            if min_bw > max_representable_bits {
                continue;
            }
            // OK, the range is known to be representable. Pick the smallest
            // legal type that will fit.
            let Some(width) = self.smallest_legal_int_width(min_bw) else { continue };
            let ty = self.context.types.int(width);
            for &inst in &members {
                self.convert(inst, ty);
            }
            made_change = true;
        }
        made_change
    }

    /// `DataLayout::getSmallestLegalIntType`, then the fallbacks of
    /// `validateAndTransform`: every supported target supports 64-bit and
    /// 32-bit integers, so a 32 or 64-bit integer if the value fits.
    fn smallest_legal_int_width(
        &self,
        min_bw: u32,
    ) -> Option<u32> {
        (min_bw..=self.layout.largest_legal_integer())
            .find(|&bits| self.layout.legal_integer(bits))
            .or(
                if min_bw <= 32 {
                    Some(32)
                } else if min_bw <= 64 {
                    Some(64)
                } else {
                    None
                },
            )
    }

    fn int_bits_of(
        &self,
        operand: Operand,
    ) -> u32 {
        let ty = self.function.operand_type(self.context, operand).expect("a typed operand");
        self.context.types.int_bits(ty).expect("an integer")
    }

    /// `IRBuilder::CreateZExtOrTrunc` / `CreateSExtOrTrunc` before `at`.
    fn extend_or_trunc(
        &mut self,
        operand: Operand,
        to: TypeId,
        signed: bool,
        at: InstId,
    ) -> Operand {
        let (from_bits, to_bits) = (self.int_bits_of(operand), self.context.types.int_bits(to).expect("an integer"));
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
        self.emit(Opcode::Cast(cast), to, vec![operand], None, at)
    }

    /// A new instruction before `at`, named `name`; its value.
    fn emit(
        &mut self,
        opcode: Opcode,
        ty: TypeId,
        operands: Vec<Operand>,
        name: Option<String>,
        at: InstId,
    ) -> Operand {
        let new = self.function.create_instruction(opcode, ty, operands, Flags::default(), name.as_deref());
        self.function.insert(new, Position::Before(at)).expect("a placed instruction");
        Operand::Value(self.function.instruction(new).result.expect("a value"))
    }

    /// `convert`
    /// ([Float2Int.cpp:402](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L402)),
    /// DEVIATE 31: the integer version of `inst` and of what it reads, as
    /// `to`.
    fn convert(
        &mut self,
        inst: InstId,
        to: TypeId,
    ) {
        let mut stack = vec![(inst, false)];
        while let Some((at, expanded)) = stack.pop() {
            if self.converted.contains_key(&at) {
                // Already converted this instruction.
                continue;
            }
            let terminates =
                matches!(
                    self.function.instruction(at).opcode,
                    Opcode::Cast(CastOp::UIToFP | CastOp::SIToFP)
                );
            if !expanded {
                stack.push((at, true));
                if !terminates {
                    let operands = self.function.instruction(at).operands.clone();
                    stack.extend(
                        operands.into_iter().rev().filter_map(|one| self.defining(one)).map(|def| (def, false)),
                    );
                }
                continue;
            }
            let new = self.convert_one(at, to, terminates);
            // If we're a root instruction, RAUW.
            if self.is_root.contains(&at) {
                let result = self.function.instruction(at).result.expect("a root has a value");
                self.function.replace_all_uses_with(result, new);
            }
            self.converted.insert(at, new);
            self.converted_order.push(at);
        }
    }

    /// The new instruction for `inst`, whose operands are converted.
    fn convert_one(
        &mut self,
        inst: InstId,
        to: TypeId,
        terminates: bool,
    ) -> Operand {
        let instruction = self.function.instruction(inst).clone();
        let name = instruction.result.and_then(|value| self.function.value(value).name.clone());
        let mut operands = Vec::with_capacity(instruction.operands.len());
        for &operand in &instruction.operands {
            // Don't recurse if we're an instruction that terminates the path.
            if terminates {
                operands.push(operand);
            } else if let Some(def) = self.defining(operand) {
                operands.push(self.converted[&def]);
            } else {
                let value = self.constant_range(inst, operand).get_single_element().expect("an exact constant");
                operands.push(Operand::Constant(self.context.int(to, sign_extend(value, RANGE_BITS))));
            }
        }
        match instruction.opcode {
            Opcode::Cast(CastOp::FPToUI) => self.extend_or_trunc(operands[0], instruction.ty, false, inst),
            Opcode::Cast(CastOp::FPToSI) => self.extend_or_trunc(operands[0], instruction.ty, true, inst),
            Opcode::FCmp(predicate) => {
                let predicate = map_fcmp_pred(predicate).expect("Unhandled predicate!");
                let bool_type = self.context.types.int(1);
                self.emit(Opcode::ICmp(predicate), bool_type, operands, name, inst)
            }
            Opcode::Cast(CastOp::UIToFP) => self.extend_or_trunc(operands[0], to, false, inst),
            Opcode::Cast(CastOp::SIToFP) => self.extend_or_trunc(operands[0], to, true, inst),
            Opcode::FNeg => {
                let zero = Operand::Constant(self.context.int(to, 0));
                self.emit(Opcode::Binary(BinaryOp::Sub), to, vec![zero, operands[0]], name, inst)
            }
            Opcode::Binary(op) => self.emit(Opcode::Binary(map_bin_opcode(op)), to, operands, name, inst),
            other => unreachable!("Unhandled instruction! {other:?}"),
        }
    }

    /// `cleanup`
    /// ([Float2Int.cpp:462](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/Float2Int.cpp#L462)):
    /// dead code elimination on the instructions just converted.
    fn cleanup(&mut self) {
        for inst in self.converted_order.iter().rev() {
            self.function.erase(*inst).expect("its users are converted or gone");
        }
    }
}

/// `value`, `bits` wide, as a signed number.
fn sign_extend(
    value: u128,
    bits: u32,
) -> i128 {
    ((value << (128 - bits)) as i128) >> (128 - bits)
}

#[cfg(test)]
#[path = "float2int_tests.rs"]
mod tests;
