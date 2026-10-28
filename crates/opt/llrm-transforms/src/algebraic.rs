//! Algebraic identities over integer values, as LLVM's InstCombine and, for a
//! loop-carried operand, Reassociate.
//!
//! # How it works
//! `simplified` repeats a round of rules until none fires. A round runs, in
//! order: the rules that need an analysis (`_whole_fixed`,
//! `_unsigned_divisions`, `_divisions`, `_redundant_masks`, `_unread_bits`),
//! then the first matching local rule on each instruction (`_rewritten`), then
//! the loop-carried reassociation (`_reassociated_recurrences`) and
//! `_shared_shifts`. A repeated addend (`_doubled_addend`) is tried last, only
//! when nothing else fired, since the other rules may cancel it (`x - (x + 5 +
//! x)`).
//!
//! Every rewrite holds modulo 2^width, so a rewritten instruction drops `nsw`,
//! `nuw`, `exact` and `disjoint`: a reassociated sum may overflow where the
//! original did not. The rules fall in groups.
//!
//! - Constants and chains: neutral terms (`x + 0`, `x * 1`, `x & -1`), `x * 0`,
//!   `x % 1`, decided self-compares, a `select` on a constant; two single-use
//!   shifts, scales, offsets or bitwise constants become one; `(x + a) * k + b`
//!   distributes; `(x + a) + a` is `x + (a << 1)`; `x - (x + y)` is `-y`.
//! - Casts: `trunc(ext x)`, `ext(ext x)`, a mask under an extension, a mask
//!   over a `sext` that reads only the low bits (`and (sext (trunc v)), M` is
//!   `and v, M`; `and (sext x), 2^N-1` over an N-bit `x` is `zext x`),
//!   `inttoptr (add x, y)` of a flat pointer as `gep i8, (inttoptr x), y` (a
//!   constant `y` not under `size`), `sext` of a value whose sign bit is clear,
//!   logic or a phi of like extensions done at the narrow type,
//!   `ptrtoint(inttoptr x)`, a byte offset from a constant near address as the
//!   constant address.
//! - Compares: `icmp P (x + C1), C2` is `icmp P x, C2 - C1` (an ordered test
//!   needs `nsw` or `nuw` for its signedness); a compare of extensions is a
//!   compare of the sources; `xor (icmp P a b), true` is the inverse compare;
//!   `icmp ne (sext i1 x), 0` is `x`.
//! - Divisions: `udiv` and `urem` by a power of two are a shift and a mask;
//!   `sdiv` and `srem` of a dividend proved non-negative (`ranges`, or a clear
//!   sign bit) are unsigned, by a power of two always, by another constant at
//!   any width and target but under `size`, where the signed form is a byte
//!   shorter (selection prices the reciprocal, prefixes included); other `sdiv`
//!   and `srem` by a power of two are an `ashr`, a bias and a shift; a
//!   fixed-point product or quotient by a whole number cancels its scale.
//! - Masks and demanded bits: `and x, C` where `C` covers every bit `x` may set
//!   is `x`; a mask, or an `or`, that touches only bits nothing reads is its
//!   operand.
//! - Phis: a phi repeating an earlier phi of its block from the same
//!   predecessors is that phi.
//!
//! A counter carried round a loop is put at the root of its `add` tree, so the
//! inner sum can be invariant and leave the loop:
//!
//! ```text
//!  loop:  %i = phi [ %i0 ], [ %n, latch ]        %i = phi [ %i0 ], [ %n, .. ]
//!         %t = add %i, %a                  ==>   %s = add %a, %b   (invariant)
//!         %n = add %t, %b                        %n = add %i, %s
//! ```
//!
//! A mask whose operand advances by a constant each trip moves after its scale,
//! `(x & m) * 2^k` as `(x * 2^k) & (m * 2^k)`, so the scale can fold into the
//! loop's step.
//!
//! # Example
//! Before:
//!
//! ```mir
//! define i16 @f(i16 %x, i16 %a) {
//! b0:
//!   %m = add i16 %x, %a
//!   %r = add i16 %m, %a
//!   ret i16 %r
//! }
//! ```
//!
//! After:
//!
//! ```mir
//! define i16 @f(i16 %x, i16 %a) {
//! b0:
//!   %0 = shl i16 %a, 1
//!   %1 = add i16 %x, %0
//!   ret i16 %1
//! }
//! ```
//!
//! A value added twice is added once, doubled.
//!
//! From unit test `test_a_value_added_twice_is_added_once_doubled` in
//! `algebraic_tests.rs`.
//!
//! # Runs
//! `algebraic` runs once in each pass of the fixed point, after `indvars` and
//! before `dead`; it has no flag of its own, so it is on whenever the optimizer
//! is. It runs again after the loop-header copy (`-ftree-ch`, not at -Os). The
//! `size` option is set at -Os and -Oz.
//!
//! `Differences` is a smaller pass of this file, run after `lsr`: it applies
//! only the rules `x - (x + y)` is `-y` and a negated difference, to clean
//! bounds subtracted from counters LSR rebased, without folding the scales LSR
//! just chose back together (LLVM's LSR cleans with InstSimplify for the same
//! reason). It is skipped with `-fno-strength-reduce`.
//!
//! # Needs
//! `ranges` (non-negative dividends, redundant masks), `DemandedBits` (unread
//! bits), `Registers` and `consts` (divisors known through phis), `induction`
//! (recurrences, over the manager's `Shape` and `Registers`, which a round that
//! edited the body invalidates).
//!
//! # Keeps
//! Blocks and edges are unchanged: dominators and loops are preserved; other
//! analyses are invalidated when anything changed.
//!
//! # See also
//! LLVM InstCombine (`visitSub`, `foldICmpAddConstant`, `visitTrunc`),
//! Reassociate, CorrelatedValuePropagation `processAnd`, SimplifyDemandedBits;
//! gcc `fold_comparison`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_analysis::cfg::Shape;
use llrm_analysis::consts::{self, Known};
use llrm_analysis::manager::{DemandedBits, Ranges, Registers};
use llrm_analysis::memory::Unit;
use llrm_analysis::num::Num as BigInt;
use llrm_analysis::scev::analysis::LazyScevView;
use llrm_analysis::{cfg, induction};
use llrm_mir::context::{Constant, ConstantExpr, ConstantKind, Context, mask, signed};
use llrm_mir::datalayout::DataLayout;
use llrm_mir::dense::IdSet;
use llrm_mir::domtree::DomTree;
use llrm_mir::edit::Position;
use llrm_mir::intrinsics::Intrinsic;
use llrm_mir::lazyvalueinfo::{Env, LazyValueInfo};
use llrm_mir::module::{Function, InstId, Operand, Use, ValueDef, ValueId};
use llrm_mir::opcode::{BinaryOp, CastOp, Flags, IntPredicate, Opcode};
use llrm_mir::passes::{self, Analyses, Dominators, FunctionPass, Loops, Outer, PreservedAnalyses};
use llrm_mir::valuetracking::{AssumptionCache, Query, known};
use llrm_support::hash::IndexMap;

use crate::counting;

/// `size`: code size outranks speed, and a division by a constant that is not a
/// power of two stays signed.
#[derive(Default)]
pub struct Algebraic {
    pub size: bool,
    /// The LLVM order's pass (kept for the pipeline's slots; no rule reads it
    /// since `_scaled_chain` went).
    pub llvm: bool,
}

impl FunctionPass for Algebraic {
    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn name(&self) -> &'static str {
        "algebraic"
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        // Blocks and edges are as they were.
        if simplified_in(unit.context, unit.layout, unit.function, analyses, self.size) {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// What a counter rebased by a use leaves behind: `x - (x + y)` is `-y`. LLVM's
/// LSR cleans with InstSimplify, not InstCombine's whole set, which would fold
/// the scales LSR just chose back into one another.
pub struct Differences;

impl FunctionPass for Differences {
    fn name(&self) -> &'static str {
        "differences"
    }

    fn adds_memory_operations(&self) -> bool {
        false
    }

    fn run(
        &mut self,
        unit: &mut passes::Unit,
        _analyses: &mut Analyses,
    ) -> PreservedAnalyses {
        let mut changed = false;
        loop {
            let mut round = false;
            for (_, inst) in unit.function.walk().collect::<Vec<_>>() {
                if !unit.function.is_erased(inst) {
                    round |= _difference_from_sum(unit.context, unit.function, inst)
                        || _negated_difference(unit.context, unit.function, inst);
                }
            }
            if !round {
                break;
            }
            changed = true;
        }
        if changed {
            PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>()
        } else {
            PreservedAnalyses::all()
        }
    }
}

/// Every rule, to a fixed point; whether anything changed.
pub fn simplified(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    size: bool,
) -> bool {
    simplified_in(context, layout, function, analyses, size)
}

/// [`simplified`].
pub fn simplified_in(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    size: bool,
) -> bool {
    // The `size` rows are 16-bit rows: the pin's CVP turns a non-negative
    // `sdiv` by any constant into `udiv` whatever the optsize, and no pin fold
    // reads it for an address.
    let size = size && layout.pointer(0).bits < layout.largest_legal_integer();
    // A number proved before stays proved: every rewrite keeps each
    // surviving value's meaning, and none divides by a value.
    let divided_by_values = function
        .walk()
        .any(
            |(_, inst)| {
                let instruction = function.instruction(inst);
                matches!(
                    instruction.opcode,
                    Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem)
                )
                    && matches!(instruction.operands[1], Operand::Value(_))
            },
        );
    let facts =
        if divided_by_values { analyses.get::<Registers>(context, layout, function) } else { Default::default() };
    let outer = std::rc::Rc::clone(analyses.outer());
    let mut changed = false;
    loop {
        let mut round = _whole_fixed(context, layout, function, &outer);
        round |= _unsigned_divisions(context, layout, function, &outer, analyses, changed || round, &facts, size);
        round |= _divisions(context, layout, function, &outer, &facts);
        round |= _redundant_masks(context, layout, function, &outer, analyses, changed || round);
        round |= _unread_bits(context, layout, function, analyses, changed || round);
        let recurrences = _recurrences(context, layout, function, &outer, analyses, changed || round);
        for (_, inst) in function.walk().collect::<Vec<_>>() {
            if !function.is_erased(inst) {
                round |= _rewritten(context, layout, function, &recurrences, inst, size);
            }
        }
        round |= _shared_shifts(context, function);
        // Last, when no other rule has anything to do: a repeat may be a
        // difference the other rules cancel (`x - (x + 5 + x)`).
        if !round {
            for (_, inst) in function.walk().collect::<Vec<_>>() {
                if !function.is_erased(inst) {
                    round |= _doubled_addend(context, function, inst);
                }
            }
        }
        if !round {
            return changed;
        }
        changed = true;
    }
}

/// The first rule that rewrites `inst`.
fn _rewritten(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    recurrences: &BTreeSet<ValueId>,
    inst: InstId,
    size: bool,
) -> bool {
    _mask_scaled(context, function, recurrences, inst)
        || _constant_address(context, function, inst)
        || _offset_address(context, layout, function, &|value| recurrences.contains(&value), inst, size)
        || _offset_scaled(context, function, inst)
        || _cast_pair(context, function, inst)
        || _narrowed_binary(context, function, inst)
        || _nonnegative_sext(context, function, inst)
        || _unsigned_power_of_two(context, function, inst)
        || _masked_extension(context, layout, function, inst)
        || _masked_truncation(context, function, inst)
        || _extended_mask(context, function, inst)
        || _casted_logic(context, function, inst)
        || _phi_of_casts(context, function, inst)
        || _duplicate_phi(function, inst)
        || _negated_difference(context, function, inst)
        || _difference_from_sum(context, function, inst)
        || _shift_chain(context, function, inst)
        || _identity(context, function, inst)
        || _decided(context, function, inst)
        || _selected(context, function, inst)
        || _inverted_compare(context, function, inst)
        || _offset_compared(context, function, inst)
        || _narrow_compare(context, function, inst)
        || _extended_boolean_tested(context, function, inst)
        || _extended_boolean_negated(context, function, inst)
}

/// The `i1` `operand` extends, read through `sext` or `zext`, and whether
/// the extension makes a true all ones.
fn _extended_boolean(
    function: &Function,
    operand: Operand,
) -> Option<(Operand, bool)> {
    let made = _definition(function, operand)?;
    let instruction = function.instruction(made);
    match instruction.opcode {
        Opcode::Cast(kind @ (CastOp::SExt | CastOp::ZExt)) => Some((instruction.operands[0], kind == CastOp::SExt)),
        _ => None,
    }
}

/// `icmp ne (sext i1 x), 0` is `x`, and `icmp eq` of it `!x`: Nib's booleans
/// are eight-bit 0 and -1, tested against zero.
fn _extended_boolean_tested(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let Opcode::ICmp(predicate @ (IntPredicate::Ne | IntPredicate::Eq)) = instruction.opcode else { return false };
    let [extended, zero] = instruction.operands[..] else { return false };
    if _integer(context, zero) != Some(0) {
        return false;
    }
    let Some((boolean, _)) = _extended_boolean(function, extended) else { return false };
    if function.operand_type(context, boolean).and_then(|ty| context.types.int_bits(ty)) != Some(1) {
        return false;
    }
    if predicate == IntPredicate::Ne {
        _forward(function, inst, boolean);
    } else {
        let truth = counting::constant(context, &BigInt::from(1), 1);
        let bit = context.types.int(1);
        let flipped = function.create_instruction(
            Opcode::Binary(BinaryOp::Xor),
            bit,
            vec![boolean, truth],
            Flags::default(),
            None,
        );
        function.insert(flipped, Position::Before(inst)).expect("a placed compare");
        let value = Operand::Value(function.instruction(flipped).result.expect("a value"));
        _forward(function, inst, value);
    }
    true
}

/// `xor (sext i1 x), -1` is `sext (xor i1 x, true)`, and `xor (zext x), 1`
/// the same zero-extended.
fn _extended_boolean_negated(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((BinaryOp::Xor, left, right, width)) = _binary(context, function, inst) else { return false };
    if width == 1 {
        return false;
    }
    let Some(((boolean, sign), number)) = [(left, right), (right, left)]
        .into_iter()
        .find_map(|(one, other)| Some((_extended_boolean(function, one)?, _integer(context, other)?)))
    else {
        return false;
    };
    if number != if sign { mask(width) } else { 1 }
        || function.operand_type(context, boolean).and_then(|ty| context.types.int_bits(ty)) != Some(1)
    {
        return false;
    }
    let truth = counting::constant(context, &BigInt::from(1), 1);
    let bit = context.types.int(1);
    let flipped =
        function.create_instruction(Opcode::Binary(BinaryOp::Xor), bit, vec![boolean, truth], Flags::default(), None);
    function.insert(flipped, Position::Before(inst)).expect("a placed xor");
    let negated = Operand::Value(function.instruction(flipped).result.expect("a value"));
    let ty = function.instruction(inst).ty;
    let kind = if sign { CastOp::SExt } else { CastOp::ZExt };
    let extended = function.create_instruction(Opcode::Cast(kind), ty, vec![negated], Flags::default(), None);
    function.insert(extended, Position::Before(inst)).expect("a placed xor");
    let value = Operand::Value(function.instruction(extended).result.expect("a value"));
    _forward(function, inst, value);
    true
}

/// `icmp P (x + C1), C2` is `icmp P x, C2 - C1`, and the same of a `sub`.
/// Equality needs no flag: a wrapping sum is a bijection. An ordered test
/// needs the sum not to wrap as its own signedness (`nsw` for a signed
/// predicate, `nuw` for an unsigned one) and the new constant to stay in the
/// type. InstCombine's `foldICmpAddConstant` does the ordered one whoever else
/// reads the sum; its equality fold (`foldICmpBinOpEqualityWithConstant`,
/// InstCombineCompares.cpp:3760, "TODO: Remove the one-use limitation") only
/// when the sum has no other reader.
///
/// DEVIATE (D10, numbered with its third part in docs/architecture/
/// recursion-rows.md): a body that had copies of itself made (`SELF_COPIED`)
/// folds an equality whoever else reads the sum. Those copies test `n - 1 ==
/// 0`, `n - 2 == 0`, ... which are `n == 1`, `n == 2`: the chain of differences
/// each level held in a register, and spilled, is dead (`rectwo`, `hanoi`,
/// `recsum`, `recpow`); the pin has no copies and so no chain. Every other body
/// keeps the pin's one-use rule. Owner: w-6. Exit: when #3422 (SimplifyCFG's
/// conditional-branch value comparisons) lands, re-measure D10's chains; if
/// they collapse the gate goes. Review 2026-11-30.
fn _offset_compared(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let Opcode::ICmp(predicate) = instruction.opcode else { return false };
    let [shifted, number] = instruction.operands[..] else { return false };
    let Some(wanted) = _integer(context, number) else { return false };
    let Some(made) = _definition(function, shifted) else { return false };
    let Some((op @ (BinaryOp::Add | BinaryOp::Sub), left, right, width)) = _binary(context, function, made) else {
        return false;
    };
    let Some((source, offset)) = _value_and_constant(context, op, left, right) else { return false };
    if width > 64 {
        return false;
    }
    // A test of a phi's own next value stays on it: the step is made for the
    // next trip anyway, and testing the phi keeps both live (ivopts takes a
    // use after the increment from the incremented candidate).
    // An exit test sits in the loop's header or its latch.
    if let (Operand::Value(source), Some(result)) = (source, function.instruction(made).result)
        && let ValueDef::Instruction(phi) = function.value(source).def
        && function.instruction(phi).opcode == Opcode::Phi
        && let Some(latch) = function
            .instruction(phi)
            .operands
            .chunks(2)
            .find(|pair| pair[0] == Operand::Value(result))
            .map(|pair| pair[1])
        && let Some(home) = function.parent(inst)
        && (function.parent(phi) == Some(home) || latch == Operand::Block(home))
    {
        return false;
    }
    let signed = matches!(
        predicate,
        IntPredicate::Slt | IntPredicate::Sle | IntPredicate::Sgt | IntPredicate::Sge
    );
    if matches!(predicate, IntPredicate::Eq | IntPredicate::Ne)
        && function.instruction(made).result.is_some_and(|result| function.users(result).into_iter().count() > 1)
        && !function.attrs.iter().any(|one| {
            matches!(
                one,
                llrm_mir::opcode::Attribute::Flag(flag) if flag == crate::walk::SELF_COPIED
            )
        })
    {
        return false;
    }
    let ordered =
        signed || matches!(
            predicate,
            IntPredicate::Ult | IntPredicate::Ule | IntPredicate::Ugt | IntPredicate::Uge
        );
    let ty = function.instruction(made).ty;
    let target = if !ordered {
        let moved = if op == BinaryOp::Add { wanted.wrapping_sub(offset) } else { wanted.wrapping_add(offset) };
        (moved & mask(width)) as i128
    } else {
        let wrap = if signed { Flags::NSW } else { Flags::NUW };
        if !function.instruction(made).flags.contains(wrap) {
            return false;
        }
        let (low, high) =
            if signed { (-(1_i128 << (width - 1)), (1_i128 << (width - 1)) - 1) } else { (0, (1_i128 << width) - 1) };
        let numeric = |value: u128| -> i128 {
            if signed { ((value as i128) << (128 - width)) >> (128 - width) } else { value as i128 }
        };
        let shift = if op == BinaryOp::Add { numeric(offset) } else { -numeric(offset) };
        let bound = numeric(wanted) - shift;
        if !(low..=high).contains(&bound) {
            return false;
        }
        bound
    };
    let constant = Operand::Constant(context.int(ty, target));
    _replace(function, inst, Opcode::ICmp(predicate), vec![source, constant]);
    true
}

/// `xor (icmp P a b), true` is `icmp !P a b` where nothing else reads the
/// compare: Nib's `if !(i < n): break` branched on the negation, which
/// hid the exit from every counting pass and cost `setl; neg; xor`.
fn _inverted_compare(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((BinaryOp::Xor, left, right, 1)) = _binary(context, function, inst) else { return false };
    let Some((compare, _)) =
        [(left, right), (right, left)].into_iter().find(|&(_, other)| _integer(context, other) == Some(1))
    else {
        return false;
    };
    let Some(made) = _definition(function, compare).filter(|_| _single_use(function, compare)) else { return false };
    let Opcode::ICmp(predicate) = function.instruction(made).opcode else { return false };
    let operands = function.instruction(made).operands.clone();
    _replace(function, inst, Opcode::ICmp(predicate.inverse()), operands);
    true
}

/// `icmp P (ext a), (ext b)` or `icmp P (ext a), C` is a compare of the narrow
/// `a`, as InstCombine's `foldICmpWithZextOrSext`: a `zext` pair orders as
/// unsigned, a `sext` pair keeps its predicate, and a constant outside `a`'s
/// range decides the answer. C's `int` promotion makes every `unsigned short`
/// compare one of these. As the pin's, there is no use gate: the extension
/// stays for its other readers and dies if it has none.
fn _narrow_compare(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let Opcode::ICmp(predicate) = instruction.opcode else { return false };
    let (left, right) = (instruction.operands[0], instruction.operands[1]);
    let extension = |function: &Function, operand: Operand| {
        let made = _definition(function, operand)?;
        let Opcode::Cast(kind @ (CastOp::ZExt | CastOp::SExt)) = function.instruction(made).opcode else { return None };
        Some((kind, function.instruction(made).operands[0]))
    };
    let Some((kind, a)) = extension(function, left) else { return false };
    let Some(narrow) = function.operand_type(context, a) else { return false };
    let Some(n) = context.types.int_bits(narrow) else { return false };
    let wide =
        context.types.int_bits(function.operand_type(context, left).expect("a typed operand")).expect("an integer");
    let signed = kind == CastOp::SExt;
    let unsigned_form = |predicate| match predicate {
        IntPredicate::Sgt => IntPredicate::Ugt,
        IntPredicate::Sge => IntPredicate::Uge,
        IntPredicate::Slt => IntPredicate::Ult,
        IntPredicate::Sle => IntPredicate::Ule,
        same => same,
    };
    if let Some((other, b)) = extension(function, right)
        && other == kind
        && function.operand_type(context, b) == Some(narrow)
    {
        let predicate = if signed { predicate } else { unsigned_form(predicate) };
        _replace(function, inst, Opcode::ICmp(predicate), vec![a, b]);
        return true;
    }
    let Some(bits) = _integer(context, right) else { return false };
    if wide > 64 {
        return false;
    }
    // `a`'s range and C, as numbers.
    let (low, high) = if signed { (-(1_i128 << (n - 1)), (1_i128 << (n - 1)) - 1) } else { (0, (1_i128 << n) - 1) };
    let compared_signed =
        matches!(
            predicate,
            IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
        );
    if signed && !compared_signed && !matches!(predicate, IntPredicate::Eq | IntPredicate::Ne) {
        return false;
    }
    let c = if compared_signed || signed { ((bits as i128) << (128 - wide)) >> (128 - wide) } else { bits as i128 };
    if (low..=high).contains(&c) {
        let predicate = if signed { predicate } else { unsigned_form(predicate) };
        let number = Operand::Constant(context.int(narrow, c));
        _replace(function, inst, Opcode::ICmp(predicate), vec![a, number]);
        return true;
    }
    // Every value of `a` is on one side of C.
    let side = if c < low { high } else { low };
    let holds = match predicate {
        IntPredicate::Eq => side == c,
        IntPredicate::Ne => side != c,
        IntPredicate::Ugt | IntPredicate::Sgt => side > c,
        IntPredicate::Uge | IntPredicate::Sge => side >= c,
        IntPredicate::Ult | IntPredicate::Slt => side < c,
        IntPredicate::Ule | IntPredicate::Sle => side <= c,
    };
    let decided = _constant(context, function, inst, u128::from(holds));
    _forward(function, inst, decided);
    true
}

/// `trunc (op (ext a), y)` is `op a, (trunc y)` for an operation that commutes
/// with truncation, as InstCombine's `visitTrunc`: a wrapping `unsigned short`
/// sum is computed in `int` and cut back, and cut first it is one narrow
/// operation. `ext a` must be `a` at the result's width, so a conversion
/// goes; the other operand's cut is loop-invariant where it is.
fn _narrowed_binary(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Cast(CastOp::Trunc) {
        return false;
    }
    let alone = _single_use(function, instruction.operands[0]);
    let narrow = instruction.ty;
    let Some(made) = _definition(function, instruction.operands[0]) else { return false };
    let Some((op, left, right, wide)) = _binary(context, function, made) else { return false };
    if !matches!(
        op,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    )
        || wide > 128
    {
        return false;
    }
    let source = |function: &Function, operand: Operand| {
        let extended = _definition(function, operand)?;
        let Opcode::Cast(CastOp::ZExt | CastOp::SExt) = function.instruction(extended).opcode else { return None };
        let from = function.instruction(extended).operands[0];
        (function.operand_type(context, from) == Some(narrow)).then_some(from)
    };
    let (a, b) = (source(function, left), source(function, right));
    if a.is_none() && b.is_none() {
        return false;
    }
    // The wide sum has other readers and stays for them: the narrow one is made
    // only where it needs no cut of its own, an extension of its width and
    // a constant, so that a counter's step reads as one (`i + 1` in `int`, cut
    // back to `short`).
    let free = |known: &Option<Operand>, operand: Operand| known.is_some() || matches!(operand, Operand::Constant(_));
    if !alone && !(free(&a, left) && free(&b, right)) {
        return false;
    }
    let cut = |function: &mut Function, known: Option<Operand>, operand: Operand| match (known, operand) {
        (Some(from), _) => from,
        (None, Operand::Constant(_)) => {
            _integer(context, operand).map_or(operand, |bits| Operand::Constant(context.int(narrow, bits as i128)))
        }
        _ => _before(function, inst, Opcode::Cast(CastOp::Trunc), vec![operand]),
    };
    let (left, right) = (cut(function, a, left), cut(function, b, right));
    _replace(function, inst, Opcode::Binary(op), vec![left, right]);
    true
}

/// `operand` is a phi: carried round a loop, or merged.
fn _loop_carried(
    function: &Function,
    operand: Operand,
) -> bool {
    _definition(function, operand).is_some_and(|made| function.instruction(made).opcode == Opcode::Phi)
}

fn _definition(
    function: &Function,
    operand: Operand,
) -> Option<InstId> {
    let Operand::Value(value) = operand else { return None };
    match function.value(value).def {
        ValueDef::Instruction(inst) => Some(inst),
        ValueDef::Argument(_) => None,
    }
}

/// `operand` is a value read in one operand slot.
fn _single_use(
    function: &Function,
    operand: Operand,
) -> bool {
    matches!(
        operand,
        Operand::Value(value) if function.users(value).has_one_use()
    )
}

fn _integer(
    context: &Context,
    operand: Operand,
) -> Option<u128> {
    let Operand::Constant(id) = operand else { return None };
    match context.get(id).kind {
        ConstantKind::Int(bits) => Some(bits),
        _ => None,
    }
}

/// `inst`'s integer width.
fn _width(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<u32> {
    context.types.int_bits(function.instruction(inst).ty)
}

/// `inst`'s integer operation and operands.
fn _binary(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<(BinaryOp, Operand, Operand, u32)> {
    let instruction = function.instruction(inst);
    let Opcode::Binary(op) = instruction.opcode else { return None };
    Some((op, instruction.operands[0], instruction.operands[1], _width(context, function, inst)?))
}

/// A value and a constant, in either order where `op` commutes.
fn _value_and_constant(
    context: &Context,
    op: BinaryOp,
    left: Operand,
    right: Operand,
) -> Option<(Operand, u128)> {
    let commutes = matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    );
    match (left, _integer(context, right), _integer(context, left)) {
        (Operand::Value(_), Some(constant), _) => Some((left, constant)),
        (_, None, Some(constant)) if commutes && matches!(right, Operand::Value(_)) => Some((right, constant)),
        _ => None,
    }
}

/// `inst` rebuilt as `opcode` over `operands` where it was, flags cleared,
/// and what that orphaned, gone.
fn _replace(
    function: &mut Function,
    inst: InstId,
    opcode: Opcode,
    operands: Vec<Operand>,
) {
    let value = _before(function, inst, opcode, operands);
    _forward(function, inst, value);
}

/// Every use of `inst` now reads `with`, and `inst` and what it alone read,
/// gone.
fn _forward(
    function: &mut Function,
    inst: InstId,
    with: Operand,
) {
    let result = function.instruction(inst).result.expect("a value");
    function.replace_value(result, with);
    _erase(function, inst);
}

fn _erase(
    function: &mut Function,
    inst: InstId,
) {
    let operands = function.instruction(inst).operands.clone();
    function.erase(inst).expect("its uses were replaced");
    for operand in operands {
        let Some(made) = _definition(function, operand) else { continue };
        let orphan = matches!(
            function.instruction(made).opcode,
            Opcode::Binary(_) | Opcode::Cast(_) | Opcode::ICmp(_)
        );
        if orphan
            && !function.is_erased(made)
            && function.users(function.instruction(made).result.expect("a value")).is_empty()
        {
            _erase(function, made);
        }
    }
}

/// A new instruction before `at`, of `at`'s type; its value.
fn _before(
    function: &mut Function,
    at: InstId,
    opcode: Opcode,
    operands: Vec<Operand>,
) -> Operand {
    let ty = function.instruction(at).ty;
    let new = function.create_instruction(opcode, ty, operands, Flags::default(), None);
    function.insert(new, Position::Before(at)).expect("a placed instruction");
    Operand::Value(function.instruction(new).result.expect("a value"))
}

fn _constant(
    context: &Context,
    function: &Function,
    inst: InstId,
    bits: u128,
) -> Operand {
    Operand::Constant(context.int(function.instruction(inst).ty, bits as i128))
}

/// `x * factor`, as `mul` or `shl` of a constant: `(x, factor, op, constant)`.
struct Scale {
    source: Operand,
    factor: u128,
    op: BinaryOp,
    constant: Operand,
}

fn _scale(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<Scale> {
    let (op, left, right, width) = _binary(context, function, inst)?;
    let (source, constant) = _value_and_constant(context, op, left, right)?;
    let factor = match op {
        BinaryOp::Shl if 0 < constant && constant < u128::from(width) => 1 << constant,
        BinaryOp::Mul => constant,
        _ => return None,
    };
    let constant = if source == left { right } else { left };
    Some(Scale { source, factor, op, constant })
}

/// `x + amount`: an `add`, or a `sub` of a constant.
fn _offset(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<(Operand, u128)> {
    let (op, left, right, width) = _binary(context, function, inst)?;
    let (source, amount) = _value_and_constant(context, op, left, right)?;
    match op {
        BinaryOp::Add => Some((source, amount)),
        BinaryOp::Sub => Some((source, amount.wrapping_neg() & mask(width))),
        _ => None,
    }
}

/// `x op constant` for an associative bitwise `op`.
fn _bitwise(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<(BinaryOp, Operand, u128)> {
    let (op, left, right, _) = _binary(context, function, inst)?;
    if !matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::Xor) {
        return None;
    }
    let (source, constant) = _value_and_constant(context, op, left, right)?;
    Some((op, source, constant))
}

/// Values advancing by a constant each trip of some loop, where a mask
/// feeds a scale, as `_mask_scaled` asks.
fn _recurrences(
    context: &Context,
    layout: &DataLayout,
    function: &Function,
    outer: &Outer,
    analyses: &mut Analyses,
    edited: bool,
) -> BTreeSet<ValueId> {
    // The values `_mask_scaled` asks about: the source of each mask that a
    // scale reads. Only a loop holding the definition of one can have it among
    // its recurrences, so only those loops are asked (LLVM asks SCEV of the
    // one value, not of every loop).
    let sources: Vec<i64> = function
        .walk()
        .filter_map(|(_, inst)| {
            let mask = _definition(function, _scale(context, function, inst)?.source)?;
            let (BinaryOp::And, Operand::Value(source), _) = _bitwise(context, function, mask)? else { return None };
            let ValueDef::Instruction(defining) = function.value(source).def else { return None };
            function.parent(defining).map(cfg::id)
        })
        .collect();
    if sources.is_empty() {
        return BTreeSet::new();
    }
    // The manager's shape and register facts, brought up to date by what the
    // rounds before changed, not worked out afresh for each round: 19 ms of 53
    // in this pass on d_faces -O1.
    if edited {
        analyses
            .invalidate_if_edited(function, &PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>());
    }
    let shape = analyses.get::<Shape>(context, layout, function);
    let registers = analyses.get::<Registers>(context, layout, function);
    let analysed = Unit::within(context, layout, function, outer).with_shape(&shape).with_registers(&registers);
    shape
        .loops
        .iter()
        .filter(|one| sources.iter().any(|at| one.body.contains(at)))
        .flat_map(|one| induction::advances(&analysed, one).into_keys())
        .collect()
}

/// `(x & m) * 2^k` is `(x * 2^k) & (m * 2^k)` at every width, where the
/// mask is read only by the scale. Only where `x` advances with a loop,
/// whose step then takes the scale for nothing; elsewhere the scale is as
/// free in an address as it would be in the mask.
fn _mask_scaled(
    context: &Context,
    function: &mut Function,
    recurrences: &BTreeSet<ValueId>,
    inst: InstId,
) -> bool {
    let Some(scale) = _scale(context, function, inst) else { return false };
    if !scale.factor.is_power_of_two() || !_single_use(function, scale.source) {
        return false;
    }
    let Some(mask_op) = _definition(function, scale.source) else { return false };
    let Some((BinaryOp::And, source, bits)) = _bitwise(context, function, mask_op) else { return false };
    if !matches!(source, Operand::Value(value) if recurrences.contains(&value)) {
        return false;
    }
    let width = _width(context, function, inst).expect("an integer");
    let moved = _constant(context, function, inst, bits.wrapping_mul(scale.factor) & mask(width));
    let scaled = _before(function, inst, Opcode::Binary(scale.op), vec![source, scale.constant]);
    _replace(function, inst, Opcode::Binary(BinaryOp::And), vec![scaled, moved]);
    true
}

/// `(x + a) * k + b` is `x * k + (a * k + b)` at every width, each op read
/// only by the next: the inner offset dies.
fn _offset_scaled(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((middle, outer)) = _offset(context, function, inst) else { return false };
    if !_single_use(function, middle) {
        return false;
    }
    let Some(scale) = _definition(function, middle).and_then(|made| _scale(context, function, made)) else {
        return false;
    };
    if !_single_use(function, scale.source) {
        return false;
    }
    let Some((source, inner)) = _definition(function, scale.source).and_then(|made| _offset(context, function, made))
    else {
        return false;
    };
    let width = _width(context, function, inst).expect("an integer");
    let amount = _constant(context, function, inst, inner.wrapping_mul(scale.factor).wrapping_add(outer) & mask(width));
    let scaled = _before(function, inst, Opcode::Binary(scale.op), vec![source, scale.constant]);
    _replace(function, inst, Opcode::Binary(BinaryOp::Add), vec![scaled, amount]);
    true
}

/// `sext x` where `x`'s sign bit is known zero is `zext x`, as InstCombine's
/// `visitSExt`: `zext` has the cheaper selection and folds with masks.
fn _nonnegative_sext(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Cast(CastOp::SExt) {
        return false;
    }
    let source = instruction.operands[0];
    let Some(width) =
        function.operand_type(context, source).and_then(|ty| context.types.int_bits(ty)).filter(|&width| width <= 128)
    else {
        return false;
    };
    if known_zero(context, function, source) >> (width - 1) & 1 == 0 {
        return false;
    }
    _replace(function, inst, Opcode::Cast(CastOp::ZExt), vec![source]);
    true
}

/// A mask over an extension looks only at the extended value's low bits
/// (`M` inside its N bits):
/// - `and (sext (trunc v to iN)), M` is `and v, M`, for any N: the truncation
///   keeps those bits and the extension only copies upward (a `zext` of a
///   truncation is the narrow form already, and its range fits the mask);
/// - `and (sext x), M` is `zext x` where `M` is all of `x`'s N bits (a partial
///   mask stays: it would turn a counter's wide use into a narrow one, and
///   `histo` kept a second 16-bit counter).
///
/// The range proof of `_redundant_masks` then sees `v` itself, not a
/// sign-extended halfword that spans [-2^(N-1), 2^(N-1)) and never fits the
/// mask; and a masked narrow counter is a `zext`, which the induction
/// analysis can follow where the narrow value cannot wrap.
fn _masked_truncation(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((BinaryOp::And, left, right, wide)) = _binary(context, function, inst) else { return false };
    let Some((value, mask)) = _value_and_constant(context, BinaryOp::And, left, right) else { return false };
    let Some(extension) = _definition(function, value) else { return false };
    let kind = match function.instruction(extension).opcode {
        Opcode::Cast(kind @ (CastOp::SExt | CastOp::ZExt)) => kind,
        _ => return false,
    };
    let inner = function.instruction(extension).operands[0];
    let Some(kept) = function.operand_type(context, inner).and_then(|ty| context.types.int_bits(ty)) else {
        return false;
    };
    if kept >= wide || kept > 64 || mask >> kept != 0 {
        return false;
    }
    let constant =
        function.instruction(inst).operands.iter().copied().find(|operand| matches!(operand, Operand::Constant(_)));
    let Some(constant) = constant else { return false };
    if kind != CastOp::SExt {
        return false;
    }
    if let Some(cut) =
        _definition(function, inner).filter(|&cut| function.instruction(cut).opcode == Opcode::Cast(CastOp::Trunc))
    {
        let source = function.instruction(cut).operands[0];
        if function.operand_type(context, source).and_then(|ty| context.types.int_bits(ty)) == Some(wide) {
            _replace(function, inst, Opcode::Binary(BinaryOp::And), vec![source, constant]);
            return true;
        }
    }
    if mask != (1_u128 << kept) - 1 {
        return false;
    }
    _replace(function, inst, Opcode::Cast(CastOp::ZExt), vec![inner]);
    true
}

/// `and (zext x), c` where `c` has no bit above `x`'s width is `zext (and x,
/// c)`, as InstCombine's `visitAnd` narrows a mask under an extension: the
/// extension no longer waits for the mask, and `and x, 2^k-1` is the
/// `trunc` `_masked_extension` takes (one `movzx r32, r8` for the pair).
fn _extended_mask(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((BinaryOp::And, left, right, wide)) = _binary(context, function, inst) else { return false };
    let Some((value, mask)) = _value_and_constant(context, BinaryOp::And, left, right) else { return false };
    if !_single_use(function, value) {
        return false;
    }
    let Some(extension) = _definition(function, value) else { return false };
    if function.instruction(extension).opcode != Opcode::Cast(CastOp::ZExt) {
        return false;
    }
    let source = function.instruction(extension).operands[0];
    let Some(width) = function.operand_type(context, source).and_then(|ty| context.types.int_bits(ty)) else {
        return false;
    };
    if width >= wide || width > 64 || mask >> width != 0 {
        return false;
    }
    let narrow = context.types.int(width);
    let constant = Operand::Constant(context.int(narrow, mask as i128));
    let masked = function.create_instruction(
        Opcode::Binary(BinaryOp::And),
        narrow,
        vec![source, constant],
        Flags::default(),
        None,
    );
    function.insert(masked, Position::Before(inst)).expect("a placed instruction");
    let masked = Operand::Value(function.instruction(masked).result.expect("a value"));
    _replace(function, inst, Opcode::Cast(CastOp::ZExt), vec![masked]);
    true
}

/// `zext(and x, 2^k-1)`, the mask read by nothing else and `k` a native
/// integer width, is `zext(trunc x)`: a native narrow value is a register's
/// low part, where the `and` copies the register and masks it.
fn _masked_extension(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Cast(CastOp::ZExt) || !_single_use(function, instruction.operands[0]) {
        return false;
    }
    let Some(made) = _definition(function, instruction.operands[0]) else { return false };
    let Some((BinaryOp::And, left, right, wide)) = _binary(context, function, made) else { return false };
    let Some((value, mask)) = _value_and_constant(context, BinaryOp::And, left, right) else { return false };
    let low = mask.trailing_ones();
    if !layout.legal_integer(low) || low >= wide || mask != (1_u128 << low) - 1 {
        return false;
    }
    let narrow = context.types.int(low);
    let cut = function.create_instruction(Opcode::Cast(CastOp::Trunc), narrow, vec![value], Flags::default(), None);
    function.insert(cut, Position::Before(inst)).expect("a placed instruction");
    let cut = Operand::Value(function.instruction(cut).result.expect("a value"));
    _replace(function, inst, Opcode::Cast(CastOp::ZExt), vec![cut]);
    true
}

/// Two integer casts that are one, or none: `trunc(ext x)` is `x`, a
/// narrower `trunc x` or a narrower `ext x`; `trunc(trunc x)` is one
/// `trunc`; an extension of a like extension, or `sext` of a `zext`, whose
/// sign bit is clear, is one extension. `ext(trunc(ext x))` is two steps.
fn _cast_pair(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    if _address_round_trip(context, function, inst) {
        return true;
    }
    let cast = |function: &Function, inst: InstId| match function.instruction(inst).opcode {
        Opcode::Cast(op @ (CastOp::Trunc | CastOp::ZExt | CastOp::SExt)) => {
            Some((op, function.instruction(inst).operands[0]))
        }
        _ => None,
    };
    let Some((outer, middle)) = cast(function, inst) else { return false };
    let Some((inner, source)) = _definition(function, middle).and_then(|made| cast(function, made)) else {
        return false;
    };
    let bits = |operand: Operand| function.operand_type(context, operand).and_then(|ty| context.types.int_bits(ty));
    let result = Operand::Value(function.instruction(inst).result.expect("a value"));
    let (Some(from), Some(to)) = (bits(source), bits(result)) else { return false };
    // `zext(trunc x)` where the bits `trunc` drops are known zero reads `x`.
    if (inner, outer) == (CastOp::Trunc, CastOp::ZExt) && from <= 128 {
        let kept = bits(middle).unwrap_or(from);
        let dropped = (u128::MAX >> (128 - from)) & !(u128::MAX >> (128 - kept));
        if known_zero(context, function, source) & dropped == dropped {
            if to == from {
                _forward(function, inst, source);
            } else {
                _replace(
                    function,
                    inst,
                    Opcode::Cast(if to > from { CastOp::ZExt } else { CastOp::Trunc }),
                    vec![source],
                );
            }
            return true;
        }
    }
    let op = match (inner, outer) {
        (CastOp::ZExt | CastOp::SExt, CastOp::Trunc) if to == from => {
            _forward(function, inst, source);
            return true;
        }
        (CastOp::ZExt | CastOp::SExt, CastOp::Trunc) if to < from => CastOp::Trunc,
        (CastOp::ZExt | CastOp::SExt, CastOp::Trunc)
        | (CastOp::ZExt, CastOp::ZExt | CastOp::SExt)
        | (CastOp::SExt, CastOp::SExt) => inner,
        (CastOp::Trunc, CastOp::Trunc) => CastOp::Trunc,
        _ => return false,
    };
    _replace(function, inst, Opcode::Cast(op), vec![source]);
    true
}

/// A byte offset from a constant near address is the constant address,
/// `inttoptr`, as InstCombine's constant folder makes it: `gep i8, ptr null,
/// -4` is `inttoptr (i16 -4 to ptr)`. Isel has one form for such an address.
fn _constant_address(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    // `inttoptr` of a constant is the constant expression.
    if let (&Opcode::Cast(CastOp::IntToPtr), [Operand::Constant(number)], Some(result)) =
        (&instruction.opcode, &instruction.operands[..], instruction.result)
        && matches!(
            context.types.get(instruction.ty),
            llrm_mir::types::Type::Pointer(0)
        )
        && matches!(context.get(*number).kind, ConstantKind::Int(_))
        && context.types.int_bits(context.get(*number).ty) == Some(16)
    {
        let address = context.constant(Constant {
            ty: instruction.ty,
            kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value: *number }),
        });
        function.replace_value(result, Operand::Constant(address));
        _erase(function, inst);
        return true;
    }
    let Opcode::GetElementPtr { source } = instruction.opcode else { return false };
    let [Operand::Constant(base), Operand::Constant(index)] = instruction.operands[..] else { return false };
    let (Some(result), pointer) = (instruction.result, instruction.ty) else { return false };
    let near = matches!(context.types.get(pointer), llrm_mir::types::Type::Pointer(0));
    let (Some(step), Some(8)) = (_integer(context, Operand::Constant(index)), context.types.int_bits(source)) else {
        return false;
    };
    let index_ty = context.get(index).ty;
    let Some(width) = context.types.int_bits(index_ty).filter(|&width| near && width == 16) else { return false };
    let start = match context.get(base).kind.clone() {
        ConstantKind::Null | ConstantKind::Zero => 0,
        ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value }) => match context.get(value).kind {
            ConstantKind::Int(bits) if context.get(value).ty == index_ty => bits,
            _ => return false,
        },
        _ => return false,
    };
    let at = context.int(index_ty, (start.wrapping_add(step) & mask(width)) as i128);
    let address = context.constant(Constant {
        ty: pointer,
        kind: ConstantKind::Expr(ConstantExpr::Cast { op: CastOp::IntToPtr, value: at }),
    });
    function.replace_value(result, Operand::Constant(address));
    _erase(function, inst);
    true
}

/// `inttoptr (add x, y)` of a flat pointer is a byte offset from `inttoptr x`:
/// `gep i8, (inttoptr x), y`. The access reads base + index (+ displacement) as
/// one address (`mov [esi+655360], al`, `movzx ax, byte [ebx+ecx]`), where the
/// integer sum was an `add` or a `lea` before it. A constant addend is not
/// taken in a build for size: each access then carries the displacement (4
/// bytes at m32) where one register held the base for all (textfill +3 bytes);
/// a variable one costs nothing. Of two values, the index is the one that is a
/// loop's recurrence. A far or huge pointer is no flat address (its offset
/// wraps apart from its selector), and a selector-alone space is no address at
/// all.
fn _offset_address(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    is_recurrence: &dyn Fn(ValueId) -> bool,
    inst: InstId,
    size: bool,
) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Cast(CastOp::IntToPtr) || !_single_use(function, instruction.operands[0]) {
        return false;
    }
    let pointer = instruction.ty;
    let llrm_mir::types::Type::Pointer(space) = *context.types.get(pointer) else { return false };
    let Some(made) = _definition(function, instruction.operands[0]) else { return false };
    let Some((BinaryOp::Add, left, right, width)) = _binary(context, function, made) else { return false };
    // Only the default space, the near or flat one: a selector-alone space
    // (`ptr addrspace(2)`) holds a segment, which no access reads as an address
    // and no byte offset moves.
    if space != 0 || layout.is_pair(space) || layout.offset_bits(space) != width {
        return false;
    }
    let (value, index) = match _value_and_constant(context, BinaryOp::Add, left, right) {
        Some(_) if size => return false,
        Some((value, offset)) => {
            let integer = context.types.int(width);
            (value, Operand::Constant(context.int(integer, signed(offset, width))))
        }
        None => {
            let (Operand::Value(_), Operand::Value(_)) = (left, right) else { return false };
            let counted = |operand: Operand| matches!(operand, Operand::Value(value) if is_recurrence(value));
            if counted(left) && !counted(right) { (right, left) } else { (left, right) }
        }
    };
    let base =
        function.create_instruction(Opcode::Cast(CastOp::IntToPtr), pointer, vec![value], Flags::default(), None);
    function.insert(base, Position::Before(inst)).expect("a placed instruction");
    let base = Operand::Value(function.instruction(base).result.expect("a value"));
    let byte = context.types.int(8);
    _replace(function, inst, Opcode::GetElementPtr { source: byte }, vec![base, index]);
    true
}

/// `ptrtoint(inttoptr x)` to x's own width is `x`, as InstCombine folds it:
/// a far null compared as an integer is the constant 0 again.
fn _address_round_trip(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    if function.instruction(inst).opcode != Opcode::Cast(CastOp::PtrToInt) {
        return false;
    }
    let Some(made) = _definition(function, function.instruction(inst).operands[0]) else { return false };
    if function.instruction(made).opcode != Opcode::Cast(CastOp::IntToPtr) {
        return false;
    }
    let source = function.instruction(made).operands[0];
    let result = Operand::Value(function.instruction(inst).result.expect("a value"));
    if function.operand_type(context, source) != function.operand_type(context, result) {
        return false;
    }
    _forward(function, inst, source);
    true
}

/// Logic of two like extensions from one type is that extension of the
/// logic at the narrow type, as InstCombine's `foldCastedBitwiseLogic`:
/// two frontend truths `and`ed, then tested, become one `i1` `and`.
fn _casted_logic(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((op @ (BinaryOp::And | BinaryOp::Or | BinaryOp::Xor), left, right, _)) = _binary(context, function, inst)
    else {
        return false;
    };
    let extended = |operand: Operand| {
        let made = _definition(function, operand)?;
        match function.instruction(made).opcode {
            Opcode::Cast(kind @ (CastOp::ZExt | CastOp::SExt)) => Some((kind, function.instruction(made).operands[0])),
            _ => None,
        }
    };
    let (Some((kind, one)), Some((other, two))) = (extended(left), extended(right)) else { return false };
    let narrow = function.operand_type(context, one);
    if kind != other
        || narrow.is_none()
        || narrow != function.operand_type(context, two)
        || !(_single_use(function, left) || _single_use(function, right))
    {
        return false;
    }
    let logic =
        function.create_instruction(Opcode::Binary(op), narrow.expect("typed"), vec![one, two], Flags::default(), None);
    function.insert(logic, Position::Before(inst)).expect("a placed instruction");
    let logic = Operand::Value(function.instruction(logic).result.expect("a value"));
    _replace(function, inst, Opcode::Cast(kind), vec![logic]);
    true
}

/// A phi of like extensions from one type, each read by it alone, is that
/// extension of a phi of their sources, as InstCombine's
/// `foldPHIArgOpIntoPHI`: an `&&`'s truths as frontend bytes, then tested,
/// become one `i1` phi a branch reads.
fn _phi_of_casts(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    if function.instruction(inst).opcode != Opcode::Phi {
        return false;
    }
    let incoming = crate::lcssa::arms(function, inst);
    let extended = |operand: Operand| {
        let made = _definition(function, operand)?;
        let op = function.instruction(made);
        match op.opcode {
            Opcode::Cast(kind @ (CastOp::ZExt | CastOp::SExt)) if _single_use(function, operand) => {
                Some((kind, op.operands[0]))
            }
            _ => None,
        }
    };
    let Some(sources) = incoming.iter().map(|&(value, _)| extended(value)).collect::<Option<Vec<_>>>() else {
        return false;
    };
    let Some(&(kind, first)) = sources.first() else { return false };
    let narrow = function.operand_type(context, first);
    if narrow.is_none()
        || sources.iter().any(|&(other, source)| other != kind || function.operand_type(context, source) != narrow)
    {
        return false;
    }
    let arms = sources.iter().zip(&incoming).map(|(&(_, source), &(_, from))| (source, from)).collect::<Vec<_>>();
    let phi = function.create_instruction(
        Opcode::Phi,
        narrow.expect("typed"),
        crate::lcssa::from_arms(&arms),
        Flags::default(),
        None,
    );
    function.insert(phi, Position::Before(inst)).expect("a placed phi");
    let block = function.parent(inst).expect("a placed phi");
    let first = function
        .iter_block(block)
        .find(|&one| function.instruction(one).opcode != Opcode::Phi)
        .expect("a terminated block");
    let cast = function.create_instruction(
        Opcode::Cast(kind),
        function.instruction(inst).ty,
        vec![Operand::Value(function.instruction(phi).result.expect("a value"))],
        Flags::default(),
        None,
    );
    function.insert(cast, Position::Before(first)).expect("a placed instruction");
    _forward(function, inst, Operand::Value(function.instruction(cast).result.expect("a value")));
    true
}

/// A phi that takes the values of an earlier phi of its block, from the same
/// predecessors, is that phi: SimplifyCFG's `EliminateDuplicatePHINodes`.
/// gvn's partial redundancy elimination made two where one stood, and the
/// loop carried both in registers (SPHEREMAPLASMA, #386).
fn _duplicate_phi(
    function: &mut Function,
    inst: InstId,
) -> bool {
    if function.instruction(inst).opcode != Opcode::Phi {
        return false;
    }
    let block = function.parent(inst).expect("a placed phi");
    let mine = &function.instruction(inst).operands;
    let ty = function.instruction(inst).ty;
    // One arm per predecessor: the same count, and each arm of this one among
    // the other's.
    let earlier = function
        .iter_block(block)
        .take_while(|&one| one != inst)
        .find(
            |&one| {
                let other = function.instruction(one);
                other.opcode == Opcode::Phi
                    && other.ty == ty
                    && other.operands.len() == mine.len()
                    && mine.chunks(2).all(|arm| other.operands.chunks(2).any(|theirs| theirs == arm))
            },
        );
    let Some(earlier) = earlier else { return false };
    let with = Operand::Value(function.instruction(earlier).result.expect("a phi's value"));
    _forward(function, inst, with);
    true
}

/// Negating a single-use modular difference reverses its operands.
fn _negated_difference(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((BinaryOp::Sub, zero, difference, _)) = _binary(context, function, inst) else { return false };
    if _integer(context, zero) != Some(0) || !_single_use(function, difference) {
        return false;
    }
    let Some(made) = _definition(function, difference) else { return false };
    let Some((BinaryOp::Sub, left, right, _)) = _binary(context, function, made) else { return false };
    _replace(function, inst, Opcode::Binary(BinaryOp::Sub), vec![right, left]);
    true
}

/// `x - (x + y)` and `(x - y) - x` are `-y`, modulo the width: InstCombine's
/// `visitSub`. A counter rebased by what a use subtracts it from (`row - (c +
/// row)` where `c = r - row`) leaves the sum behind it.
fn _difference_from_sum(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((BinaryOp::Sub, left, right, _)) = _binary(context, function, inst) else { return false };
    if _integer(context, left) == Some(0) {
        return false;
    }
    let other = match (
        _definition(function, right).and_then(|made| _binary(context, function, made)),
        _definition(function, left).and_then(|made| _binary(context, function, made)),
    ) {
        (Some((BinaryOp::Add, first, second, _)), _) if first == left => Some(second),
        (Some((BinaryOp::Add, first, second, _)), _) if second == left => Some(first),
        (_, Some((BinaryOp::Sub, minuend, subtrahend, _))) if minuend == right => Some(subtrahend),
        _ => None,
    };
    let Some(other) = other else { return false };
    let zero = _constant(context, function, inst, 0);
    _replace(function, inst, Opcode::Binary(BinaryOp::Sub), vec![zero, other]);
    true
}

/// `(x << a) << b` is `x << (a + b)` while that is short of the width.
fn _shift_chain(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((BinaryOp::Shl, middle, Operand::Constant(_), width)) = _binary(context, function, inst) else {
        return false;
    };
    if !_single_use(function, middle) {
        return false;
    }
    let Some(previous) = _definition(function, middle) else { return false };
    let Some((BinaryOp::Shl, source, first @ Operand::Constant(_), _)) = _binary(context, function, previous) else {
        return false;
    };
    let (Some(first), Some(last)) =
        (_integer(context, first), _integer(context, function.instruction(inst).operands[1]))
    else {
        return false;
    };
    let total = first.saturating_add(last);
    if first.min(last) == 0 || total >= u128::from(width) {
        return false;
    }
    let total = _constant(context, function, inst, total);
    _replace(function, inst, Opcode::Binary(BinaryOp::Shl), vec![source, total]);
    true
}

/// The operands of the tree of single-use `add`s `inst` roots:
/// `((x + a) + b) + a` is `x`, `a`, `b`, `a`.
fn _addends(
    context: &Context,
    function: &Function,
    inst: InstId,
    out: &mut Vec<Operand>,
) {
    let Some((BinaryOp::Add, left, right, _)) = _binary(context, function, inst) else {
        return;
    };
    for operand in [left, right] {
        let inner = _definition(function, operand)
            .filter(|_| _single_use(function, operand) && out.len() < 8)
            .filter(|&made| matches!(_binary(context, function, made), Some((BinaryOp::Add, ..))));
        match inner {
            Some(made) => _addends(context, function, made, out),
            None => out.push(operand),
        }
    }
}

/// A value added twice through single-use `add`s is added once, doubled:
/// `(x + a) + a` is `x + (a << 1)`, and `((x + a) + b) + a` is
/// `(x + b) + (a << 1)`. LLVM's InstCombine (`X + Y + Y` is `X + (Y << 1)`) and
/// Reassociate, which ranks the repeats together. The sum has the operations it
/// had, and the doubled value is one a read-modify-write of a cell can take as
/// its addend (bench/particle's second sweep of the same cells).
fn _doubled_addend(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let mut addends = Vec::new();
    _addends(context, function, inst, &mut addends);
    let repeated = addends
        .iter()
        .enumerate()
        .find(|&(at, one)| {
            // A value the loop carries is Reassociate's: its own rule keeps it
            // at the root.
            matches!(one, Operand::Value(_))
                && addends[..at].contains(one)
                && _definition(function, *one).is_none_or(|made| function.instruction(made).opcode != Opcode::Phi)
        })
        .map(|(_, one)| *one);
    let Some(repeat) = repeated else { return false };
    // Both uses of the repeated value leave; the doubled one is built first,
    // before the sum it joins.
    let mut kept: Vec<Operand> = Vec::new();
    let mut seen = 0;
    for &one in &addends {
        if one == repeat && seen < 2 {
            seen += 1;
            continue;
        }
        kept.push(one);
    }
    let one = _constant(context, function, inst, 1);
    let doubled = _before(function, inst, Opcode::Binary(BinaryOp::Shl), vec![repeat, one]);
    let mut sum = doubled;
    for operand in kept.into_iter().rev() {
        sum = _before(function, inst, Opcode::Binary(BinaryOp::Add), vec![operand, sum]);
    }
    _forward(function, inst, sum);
    true
}

fn _identity(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some(answer) = identity(context, function, inst) else { return false };
    _forward(function, inst, answer);
    true
}

/// The operand `inst` equals by an identity: `x + 0`, `x - 0`, `x | 0`,
/// `x ^ 0`, a shift by 0, `x * 1`, `x / 1` and `x & -1` are `x`; `x * 0` and `x
/// & 0` are 0, and `x | -1` is -1.
pub fn identity(
    context: &Context,
    function: &Function,
    inst: InstId,
) -> Option<Operand> {
    let (op, left, right, width) = _binary(context, function, inst)?;
    let commutes = matches!(
        op,
        BinaryOp::Add | BinaryOp::Mul | BinaryOp::And | BinaryOp::Or | BinaryOp::Xor
    );
    let pairs = [(left, right), (right, left)];
    pairs[..1 + usize::from(commutes)]
        .iter()
        .find_map(
            |&(kept, other)| {
                let number = _integer(context, other)?;
                let all = mask(width);
                match op {
                    BinaryOp::Add
                    | BinaryOp::Sub
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::Shl
                    | BinaryOp::LShr
                    | BinaryOp::AShr
                        if number == 0 =>
                    {
                        Some(kept)
                    }
                    BinaryOp::Mul | BinaryOp::UDiv | BinaryOp::SDiv if number == 1 => Some(kept),
                    BinaryOp::And if number == all => Some(kept),
                    BinaryOp::Mul | BinaryOp::And if number == 0 => Some(other),
                    BinaryOp::Or if number == all => Some(other),
                    _ => None,
                }
            },
        )
}

/// `x % 1` is 0, a comparison of a value with itself is decided by its
/// predicate, and `x >= 0` or `x < 0` unsigned is true or false.
fn _decided(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    let answer = match instruction.opcode {
        Opcode::Binary(BinaryOp::URem | BinaryOp::SRem) if _integer(context, instruction.operands[1]) == Some(1) => {
            Some(0)
        }
        Opcode::ICmp(predicate) => {
            let (left, right) = (instruction.operands[0], instruction.operands[1]);
            if left == right && matches!(left, Operand::Value(_)) {
                Some(u128::from(matches!(
                    predicate,
                    IntPredicate::Eq | IntPredicate::Uge | IntPredicate::Ule | IntPredicate::Sge | IntPredicate::Sle
                )))
            } else {
                match predicate {
                    IntPredicate::Uge if _integer(context, right) == Some(0) => Some(1),
                    IntPredicate::Ult if _integer(context, right) == Some(0) => Some(0),
                    _ => None,
                }
            }
        }
        _ => None,
    };
    let Some(bits) = answer else { return false };
    let decided = _constant(context, function, inst, bits);
    _forward(function, inst, decided);
    true
}

/// A `select` on a constant condition is the arm it chooses.
fn _selected(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let instruction = function.instruction(inst);
    if instruction.opcode != Opcode::Select {
        return false;
    }
    let Some(bits) = _integer(context, instruction.operands[0]) else { return false };
    let chosen = instruction.operands[if bits & 1 == 1 { 1 } else { 2 }];
    _forward(function, inst, chosen);
    true
}

/// Reuse a smaller shift of the same value, read elsewhere, in the same
/// block: `x << 3` after a used `t = x << 1` is `t << 2`.
fn _shared_shifts(
    context: &Context,
    function: &mut Function,
) -> bool {
    let mut changed = false;
    for block in function.layout().to_vec() {
        let mut available = IndexMap::<Operand, BTreeMap<u128, ValueId>>::default();
        for inst in function.iter_block(block).collect::<Vec<_>>() {
            let Some(Scale { source, op: BinaryOp::Shl, constant, .. }) = _scale(context, function, inst) else {
                continue;
            };
            let count = _integer(context, constant).expect("a constant count");
            let candidates = available.entry(source).or_default();
            if let Some((&amount, &previous)) = candidates.range(..count).next_back() {
                let rest = _constant(context, function, inst, count - amount);
                function.set_operands(inst, vec![Operand::Value(previous), rest]);
                function.set_flags(inst, Flags::default());
                changed = true;
            }
            let result = function.instruction(inst).result.expect("a value");
            if !function.users(result).is_empty() {
                candidates.insert(count, result);
            }
        }
    }
    changed
}

/// A fixed-point product or quotient by a whole number `k`, its fraction
/// bits all clear: the scale cancels, leaving `x * k` or `x / k`. A
/// quotient by -1 stays, where the least value wraps and `sdiv` would be
/// undefined.
fn _whole_fixed(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
) -> bool {
    let unit = Unit::within(context, layout, function, outer);
    let whole: Vec<(InstId, BinaryOp, i128)> = function
        .walk()
        .filter_map(|(_, inst)| {
            let Some(Intrinsic::Fixed { divide }) = unit.intrinsic(inst) else { return None };
            let operands = &function.instruction(inst).operands;
            let (width, scale) = (_width(context, function, inst)?, _integer(context, operands[2])?);
            let factor = _integer(context, operands[1])?;
            let signed = ((factor << (128 - width)) as i128) >> (128 - width);
            let whole = signed >> scale;
            let clear = scale < u128::from(width) && factor & ((1 << scale) - 1) == 0 && whole != 0;
            (clear && !(divide && whole == -1)).then_some((
                inst,
                if divide { BinaryOp::SDiv } else { BinaryOp::Mul },
                whole,
            ))
        })
        .collect();
    for &(inst, op, whole) in &whole {
        let x = function.instruction(inst).operands[0];
        let by = _constant(context, function, inst, whole as u128);
        _replace(function, inst, Opcode::Binary(op), vec![x, by]);
    }
    !whole.is_empty()
}

/// `udiv` and `urem` by a power of two: a logical shift and a mask, which the
/// division unit has no part in.
fn _unsigned_power_of_two(
    context: &Context,
    function: &mut Function,
    inst: InstId,
) -> bool {
    let Some((op @ (BinaryOp::UDiv | BinaryOp::URem), left, right, width)) = _binary(context, function, inst) else {
        return false;
    };
    let Some(divisor) = _integer(context, right).filter(|&divisor| {
        divisor.is_power_of_two() && divisor > 1 && width <= 128 && divisor < 1 << (width - 1).min(127)
    }) else {
        return false;
    };
    if op == BinaryOp::UDiv {
        let count = _constant(context, function, inst, u128::from(divisor.trailing_zeros()));
        _replace(function, inst, Opcode::Binary(BinaryOp::LShr), vec![left, count]);
    } else {
        let low = _constant(context, function, inst, divisor - 1);
        _replace(function, inst, Opcode::Binary(BinaryOp::And), vec![left, low]);
    }
    true
}

/// `and x, C` where every bit `x` may have set is in `C` is `x`: LLVM's
/// CorrelatedValuePropagation `processAnd`, over `computeKnownBits`. The bits
/// `x` may set are those a non-negative range's top reaches, less the ones
/// structure clears (`valuetracking::known_zero`: a scale's low bits). A loop
/// counter's range makes tile's `(x + 7) & 63` of `x < 40` a plain `x + 7`.
fn _redundant_masks(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    analyses: &mut Analyses,
    edited: bool,
) -> bool {
    let candidates: Vec<(llrm_mir::module::BlockId, InstId, Operand, u128, u32)> = function
        .walk()
        .filter_map(|(block, inst)| {
            let (BinaryOp::And, left, right, width) = _binary(context, function, inst)? else { return None };
            let (value, mask) = _value_and_constant(context, BinaryOp::And, left, right)?;
            (width <= 128).then_some((block, inst, value, mask, width))
        })
        .collect();
    if candidates.is_empty() {
        return false;
    }
    // The body is as the rounds before left it: the manager's ranges of it,
    // brought up to date by what they changed, not worked out afresh for
    // each round.
    if edited {
        analyses
            .invalidate_if_edited(function, &PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>());
    }
    // CVP's `processAnd`: LazyValueInfo's range at the use first.
    let assumptions = AssumptionCache::of(context, &outer.globals, &outer.metadata, function);
    let tree = DomTree::of(function);
    let mut lvi = LazyValueInfo::new();
    let mut settled: Vec<(InstId, Operand)> = Vec::new();
    let mut candidates = candidates;
    candidates.retain(|&(_, inst, value, mask, width)| {
        let Some(index) = function.instruction(inst).operands.iter().position(|one| *one == value) else { return true };
        let range = {
            let mut env = Env::new(context, &outer.globals, &outer.metadata, function, layout, &assumptions);
            lvi.get_constant_range_at_use(&mut env, Use { user: inst, index: index as u32 }, false)
        };
        let top = range.get_unsigned_max();
        let reach = if top == 0 { 0 } else { u128::MAX >> (top.leading_zeros()) };
        let all = if width == 128 { u128::MAX } else { (1u128 << width) - 1 };
        let query = Query {
            globals: &outer.globals,
            metadata: &outer.metadata,
            dt: Some(&tree),
            assumptions: Some(&assumptions),
            cxt: Some(inst),
            ..Query::bare(context, function, layout)
        };
        let possible = reach & !known::compute_known_bits(&query, value, 0).zero & all;
        if possible & !mask == 0 {
            settled.push((inst, value));
            return false;
        }
        true
    });
    // Then ScalarEvolution's unsigned range: a counted loop's counter.
    let scev = LazyScevView::new(analyses);
    let made: Vec<(InstId, Operand)> = candidates
        .into_iter()
        .filter_map(|(_, inst, value, mask, width)| {
            let range = scev.ask(context, layout, function, |se, cx| {
                let s = se.get_scev(cx, value);
                se.get_unsigned_range(cx, s)
            });
            let top = range.get_unsigned_max();
            let reach = if top == 0 { 0 } else { u128::MAX >> (top.leading_zeros()) };
            let all = if width == 128 { u128::MAX } else { (1u128 << width) - 1 };
            let possible = reach & !known_zero(context, function, value) & all;
            (possible & !mask == 0).then_some((inst, value))
        })
        .chain(settled)
        .collect();
    for &(inst, value) in &made {
        if !function.is_erased(inst) {
            _forward(function, inst, value);
        }
    }
    !made.is_empty()
}

/// A value some of whose bits nothing reads (`DemandedBits`, LLVM's
/// SimplifyDemandedBits): a mask that clears only bits nobody reads, or an `or`
/// that sets only such bits, is its operand; an operation whose every read bit
/// is known zero is zero. `(a + (b & 3)) & 3` is `(a + b) & 3`, and the half of
/// a bit reversal a masked byte never reads is gone.
fn _unread_bits(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    edited: bool,
) -> bool {
    // A value that is gone reads nothing: what only it read is unread in turn,
    // so a stage of a reversal at a time, until none is left, not a round
    // of every rule for each.
    let mut changed = false;
    let mut edited = edited;
    while _unread_bits_once(context, layout, function, analyses, edited) {
        changed = true;
        edited = true;
    }
    changed
}

fn _unread_bits_once(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    analyses: &mut Analyses,
    edited: bool,
) -> bool {
    // What can have a read bit cleared or every read bit known zero: a mask, a
    // shift by a constant (a run of its bits is zero) or a zero extension;
    // the rest is asked of `known_zero` only once something reads less than
    // all of a value.
    let candidates: Vec<InstId> = function
        .walk()
        .filter_map(|(_, inst)| {
            let instruction = function.instruction(inst);
            let result = instruction.result?;
            context.types.int_bits(instruction.ty).filter(|&width| width <= 128)?;
            if function.users(result).is_empty() {
                return None;
            }
            let constant = instruction.operands.iter().any(|&operand| _integer(context, operand).is_some());
            match instruction.opcode {
                Opcode::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Shl | BinaryOp::LShr) if constant => Some(inst),
                Opcode::Cast(CastOp::ZExt) => Some(inst),
                _ => None,
            }
        })
        .collect();
    if candidates.is_empty() {
        return false;
    }
    if edited {
        analyses
            .invalidate_if_edited(function, &PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>());
    }
    let read = analyses.get::<DemandedBits>(context, layout, function);
    // Bits read only shrink as the rewrites below go, so the answers stay sound
    // for the batch.
    let mut changed = false;
    for inst in candidates {
        if function.is_erased(inst) {
            continue;
        }
        let instruction = function.instruction(inst);
        let result = instruction.result.expect("a value");
        let Some(&bits) = read.get(&result) else { continue };
        let width = context.types.int_bits(instruction.ty).expect("an integer");
        let all = if width == 128 { u128::MAX } else { (1_u128 << width) - 1 };
        let zero = known_zero(context, function, Operand::Value(result)) & all;
        let (opcode, operands) = (instruction.opcode.clone(), instruction.operands.clone());
        let replacement = if bits & !zero == 0 {
            Some(_constant(context, function, inst, 0))
        } else {
            let constant = |at: usize| operands.get(at).and_then(|&operand| _integer(context, operand));
            match (opcode, constant(0), constant(1)) {
                (Opcode::Binary(BinaryOp::And), None, Some(mask)) if bits & !mask == 0 => Some(operands[0]),
                (Opcode::Binary(BinaryOp::And), Some(mask), None) if bits & !mask == 0 => Some(operands[1]),
                (Opcode::Binary(BinaryOp::Or), None, Some(mask)) if bits & mask == 0 => Some(operands[0]),
                (Opcode::Binary(BinaryOp::Or), Some(mask), None) if bits & mask == 0 => Some(operands[1]),
                _ => None,
            }
        };
        if let Some(with) = replacement {
            // The bits the mask cleared reach what used the result: a sum or
            // product with `nsw`/`nuw` may now overflow (poison), where it did
            // not (InstCombine's SimplifyDemandedUseBits drops the flags of an
            // operation whose operand it changed).
            _poison_flags_dropped(function, result);
            _forward(function, inst, with);
            changed = true;
        }
    }
    changed
}

/// The flags that make an overflow poison (`nsw`, `nuw`) or an inexact
/// division poison (`exact`), cleared on every operation the value `from` flows
/// into through arithmetic, phis and selects: they read bits of it nobody else
/// does.
fn _poison_flags_dropped(
    function: &mut Function,
    from: ValueId,
) {
    let mut seen = IdSet::<ValueId>::new();
    let mut pending = vec![from];
    while let Some(value) = pending.pop() {
        if !seen.insert(value) {
            continue;
        }
        for user in function.users(value).iter().map(|one| one.user).collect::<Vec<_>>() {
            let instruction = function.instruction(user);
            let flows = matches!(
                instruction.opcode,
                Opcode::Binary(BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Shl)
                    | Opcode::Phi
                    | Opcode::Select
            );
            if !flows {
                continue;
            }
            let flags = instruction.flags;
            let kept = flags.without(Flags::NUW).without(Flags::NSW);
            if kept != flags {
                function.set_flags(user, kept);
            }
            if let Some(result) = function.instruction(user).result {
                pending.push(result);
            }
        }
    }
}

/// `sdiv` and `srem` of a dividend proved non-negative are `udiv` and `urem`
/// (by a power of two, a shift and a mask): the same value without the bias a
/// negative dividend needs. By another constant too, but under `size`: its
/// unsigned division is a `div` for an `idiv` (three clocks on a 486) and a
/// byte more (`xor edx,edx` for `cdq`), and its reciprocal has no sign to
/// correct, which selection prices at every target. The
/// proof is `ranges`' (a loop's `rest > 0`, a sum of squares) or the dividend's
/// clear sign bit.
fn _unsigned_divisions(
    context: &mut Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    analyses: &mut Analyses,
    edited: bool,
    facts: &IndexMap<ValueId, Known>,
    size: bool,
) -> bool {
    let candidates: Vec<(llrm_mir::module::BlockId, InstId, u32)> = function
        .walk()
        .filter(|&(_, inst)| {
            matches!(
                function.instruction(inst).opcode,
                Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem)
            )
        })
        .filter_map(|(block, inst)| {
            Some((block, inst, _width(context, function, inst).filter(|&width| layout.legal_integer(width))?))
        })
        .collect();
    if candidates.is_empty() {
        return false;
    }
    // The body is as the rounds before left it: the manager's ranges of it,
    // brought up to date by what they changed, not worked out afresh for
    // each round.
    if edited {
        analyses
            .invalidate_if_edited(function, &PreservedAnalyses::none().preserve::<Dominators>().preserve::<Loops>());
    }
    // CVP's `processSDivOrSRem`: LazyValueInfo's range of the dividend first.
    let assumptions = AssumptionCache::of(context, &outer.globals, &outer.metadata, function);
    let mut lvi = LazyValueInfo::new();
    let non_negative: Vec<bool> = candidates
        .iter()
        .map(|&(_, inst, _)| {
            let mut env = Env::new(context, &outer.globals, &outer.metadata, function, layout, &assumptions);
            lvi.get_constant_range_at_use(&mut env, Use { user: inst, index: 0 }, false).is_all_non_negative()
        })
        .collect();
    let held = Ranges::of(context, layout, function, analyses);
    let unit = held.unit(context, layout, function, outer);
    let scev = LazyScevView::new(analyses);
    let made: Vec<(InstId, BinaryOp)> = candidates
        .into_iter()
        .zip(non_negative)
        .filter_map(|((_, inst, width), non_negative)| {
            let operands = &function.instruction(inst).operands;
            let fact = consts::_operand(&unit, operands[1], facts, None)?;
            let divisor = BigInt::from(consts::masked(&fact.n, width));
            // A power of two is shifts either way. Any other constant is a
            // multiply by its reciprocal, which has no
            // sign to correct where the dividend is never negative; selection
            // prices it, prefixes included, whatever the target, and
            // where it declines `div` is cheaper than `idiv` (the audited
            // bounds, `xor edx, edx` for `cdq`) but a byte, which size does
            // not pay.
            let power = (&divisor & (&divisor - 1u8)) == BigInt::from(0u8);
            let positive = fact.width >= width
                && divisor > BigInt::from(1u8)
                && divisor < BigInt::from(1u8) << (width - 1)
                && (power || !size);
            let proved = non_negative
                || scev.ask(context, layout, function, |se, cx| {
                    let s = se.get_scev(cx, operands[0]);
                    se.get_signed_range(cx, s).is_all_non_negative()
                })
                || known_zero(context, function, operands[0]) >> (width - 1) & 1 == 1;
            let op = if function.instruction(inst).opcode == Opcode::Binary(BinaryOp::SDiv) {
                BinaryOp::UDiv
            } else {
                BinaryOp::URem
            };
            (positive && proved).then_some((inst, op))
        })
        .collect();
    for &(inst, op) in &made {
        let operands = function.instruction(inst).operands.clone();
        _replace(function, inst, Opcode::Binary(op), operands);
    }
    !made.is_empty()
}

/// Divide by a positive power of two, biasing a negative dividend to
/// truncate toward zero: `sdiv` and `srem` by a divisor consts proves, at
/// a legal integer width, where the shifts cost less than the division.
fn _divisions(
    context: &Context,
    layout: &DataLayout,
    function: &mut Function,
    outer: &Outer,
    facts: &IndexMap<ValueId, Known>,
) -> bool {
    let shape = cfg::Shape::of(function);
    let unit = Unit::within(context, layout, function, outer).with_shape(&shape);
    let divisors: Vec<(InstId, u32)> = function
        .walk()
        .map(|(_, inst)| inst)
        .filter(|&inst| matches!(
            function.instruction(inst).opcode,
            Opcode::Binary(BinaryOp::SDiv | BinaryOp::SRem)
        ))
        .filter_map(|inst| {
            let width = _width(context, function, inst).filter(|&width| layout.legal_integer(width))?;
            let fact = consts::_operand(&unit, function.instruction(inst).operands[1], facts, None)?;
            let divisor = u128::try_from(consts::masked(&fact.n, width)).ok().filter(|_| fact.width >= width)?;
            (divisor.is_power_of_two() && divisor > 1 && divisor < 1 << (width - 1))
                .then(|| (inst, divisor.trailing_zeros()))
        })
        .collect();
    for &(inst, shift) in &divisors {
        if function.is_erased(inst) {
            continue;
        }
        let width = _width(context, function, inst).expect("an integer");
        let dividend = function.instruction(inst).operands[0];
        let binary = |op| Opcode::Binary(op);
        let top = _constant(context, function, inst, u128::from(width - 1));
        let sign = _before(function, inst, binary(BinaryOp::AShr), vec![dividend, top]);
        let adjusted = if shift == 1 {
            // The bias is the sign's low bit, 0 or 1: subtracting the sign word
            // adds it.
            _before(function, inst, binary(BinaryOp::Sub), vec![dividend, sign])
        } else {
            let low = _constant(context, function, inst, (1 << shift) - 1);
            let bias = _before(function, inst, binary(BinaryOp::And), vec![sign, low]);
            _before(function, inst, binary(BinaryOp::Add), vec![dividend, bias])
        };
        let count = _constant(context, function, inst, u128::from(shift));
        let quotient = _before(function, inst, binary(BinaryOp::AShr), vec![adjusted, count]);
        if function.instruction(inst).opcode == binary(BinaryOp::SDiv) {
            _forward(function, inst, quotient);
        } else {
            let product = _before(function, inst, binary(BinaryOp::Shl), vec![quotient, count]);
            _replace(function, inst, binary(BinaryOp::Sub), vec![dividend, product]);
        }
    }
    !divisors.is_empty()
}

#[cfg(test)]
#[path = "algebraic_tests.rs"]
mod tests;

/// `computeKnownBits(V).Zero` with no context instruction: the zero half of
/// the ValueTracking translation (the layout is only read for pointer
/// widths, which the callers' integers do not reach).
fn known_zero(
    context: &Context,
    function: &Function,
    operand: Operand,
) -> u128 {
    thread_local! {
        static LAYOUT: DataLayout = DataLayout::default();
    }
    LAYOUT.with(|layout| known::compute_known_bits(&Query::bare(context, function, layout), operand, 0).zero)
}
