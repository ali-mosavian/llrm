//! What LLVM's loop strength reduction collects before it generates formulae:
//! the uses and their fixups, the interesting factors and types, and each use's
//! first formula (`LSRInstance::getUse`, `CollectInterestingTypesAndFactors`,
//! `CollectFixupsAndInitialFormulae`, `CollectLoopInvariantFixupsAndFormulae`
//! and the helpers they call; LoopStrengthReduce.cpp at `d1106deb71cc`,
//! l.795-1125 and l.2817-3960).
//!
//! # How it works
//! Each `IvStrideUse` becomes a fixup of an `lsr_use::Use`, found or made by
//! the use's expression (split into a base and an immediate offset) and its
//! kind: an address, an equality compare with its other side folded in
//! (`i == n` as `n - i == 0`), or a plain value. Fixups of one expression share
//! a use; their offsets widen its range while an address mode still holds
//! them. A use's first formula is the expression split into its loop-variant
//! and invariant registers. Invariants that other code in the loop also uses
//! are added as uses, so the solver sees what they pin in registers.
//!
//! # Example
//! `p[i] = 0` with the exit `i + 1 == n`: an address use of `{%p,+,4}` and a
//! compare use of `{(-1 + %n),+,-1}`; factors 4, -4 and -1.
//!
//! # Runs
//! Nothing runs it yet: stage 3 of the translation of LSR.
//!
//! # Limits
//! DEVIATE:
//! 1. No IV chains (`CollectChains`, `isHighCostExpansion`'s chain users),
//!    `OptimizeShadowIV` and `OptimizeLoopTermCond` yet; the later stages of
//!    stage 3. No atomic accesses, funclet pads, scalable immediates or target
//!    memory intrinsics (MIR has none). `canSaveCmp` is false on x86. The swap
//!    of a compare's operands (`setOperand`) is recorded in `swapped_compares`
//!    for the caller to make: the analyses hold the function shared.
//! 2. A callee is named from the module's globals the pass was handed, not the
//!    live module: a function declared during the compile (`runtime.nib` cut to
//!    a program, #3140) has no name there, so an `llvm.lifetime` call declared
//!    that way is not skipped by `collect_loop_invariant_fixups_and_formulae`
//!    and stays a use (worse code at most; the pin reads the callee's intrinsic
//!    id). Exit: the snapshot sees declarations added during the compile.
//!    Boundary: `a_call_to_a_global_the_outer_snapshot_lacks_is_no_intrinsic`.

use llrm_analysis::scev::expr::{Kind as ScevKind, Scev, Ty, Type, Use as ScevUse};
use llrm_analysis::scev::{Cx, Flags, ScalarEvolution};
use llrm_mir::context::GlobalId;
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::{IntPredicate, Opcode};
use llrm_mir::tti::{AddressingMode, Tti};
use llrm_mir::types::Type as MirType;
use llrm_support::hash::{HashMap, HashSet, IndexSet};

use crate::ivusers::IvUsers;
use crate::lsr_formula::{Formula, Immediate, MemAccessTy, RegUseTracker};
use crate::lsr_use::{
    Cost, Fixup, Kind, Rating, Use, is_am_completely_folded_at, is_am_completely_folded_between, is_legal_use_formula,
};

// ---- helpers ----

/// `isAddRecSExtable`: whether the addrec can be sign-extended without changing
/// its value.
fn is_add_rec_sextable(
    se: &mut ScalarEvolution,
    cx: &Cx,
    ar: Scev,
) -> bool {
    let bits = se.get_type_size_in_bits(cx, se.type_of(ar)) + 1;
    let wide = se.get_sign_extend_expr(cx, ScevUse::of(ar), Ty::Int(bits), 0);
    se.scev_type(wide) == Type::AddRec
}

/// `isAddSExtable`.
fn is_add_sextable(
    se: &mut ScalarEvolution,
    cx: &Cx,
    add: Scev,
) -> bool {
    let bits = se.get_type_size_in_bits(cx, se.type_of(add)) + 1;
    let wide = se.get_sign_extend_expr(cx, ScevUse::of(add), Ty::Int(bits), 0);
    se.scev_type(wide) == Type::Add
}

/// `isMulSExtable`. DEVIATE: SCEV here holds up to 128 bits, so a product
/// whose wide type would be wider is not sign-extable.
fn is_mul_sextable(
    se: &mut ScalarEvolution,
    cx: &Cx,
    mul: Scev,
) -> bool {
    let bits = se.get_type_size_in_bits(cx, se.type_of(mul)) * se.operands(mul).len() as u32;
    if bits > 128 {
        return false;
    }
    let wide = se.get_sign_extend_expr(cx, ScevUse::of(mul), Ty::Int(bits), 0);
    se.scev_type(wide) == Type::Mul
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L819
/// `getExactSDiv`: `lhs /s rhs` where it can be determined and the remainder
/// is zero. With `ignore_significant_bits`, `(X * Y) /s Y` is `X` whether or
/// not the product overflows, for a result whose high bits are ignored.
pub fn get_exact_sdiv(
    se: &mut ScalarEvolution,
    cx: &Cx,
    lhs: Scev,
    rhs: Scev,
    ignore_significant_bits: bool,
) -> Option<Scev> {
    // The trivial case works for any SCEV type.
    if lhs == rhs {
        return Some(se.get_constant_of_type(cx, se.type_of(lhs), 1));
    }
    let rc = se.as_constant(rhs);
    if let Some(ra) = rc {
        // x /s -1 is x * -1, to give SCEV a chance to fold it.
        if ra.is_all_ones() {
            if se.type_of(lhs).is_pointer() {
                return None;
            }
            return Some(se.get_mul_expr(cx, vec![ScevUse::of(lhs), ScevUse::of(rhs)], Flags::default(), 0).scev);
        }
        // x /s 1 is x.
        if ra.is_one() {
            return Some(lhs);
        }
    }
    // A constant by a constant.
    if let Some(la) = se.as_constant(lhs) {
        let ra = rc?;
        if !la.srem(ra).is_zero() {
            return None;
        }
        return Some(se.get_constant(la.sdiv(ra)));
    }
    match se.node(lhs).kind.clone() {
        // The sdiv distributes over addrec operands that do not overflow.
        ScevKind::AddRec(_, header) => {
            if (ignore_significant_bits || is_add_rec_sextable(se, cx, lhs)) && se.is_affine(lhs) {
                let step_of = se.get_step_recurrence(cx, lhs);
                let step = get_exact_sdiv(se, cx, step_of, rhs, ignore_significant_bits)?;
                let start_of = se.operands(lhs)[0].scev;
                let start = get_exact_sdiv(se, cx, start_of, rhs, ignore_significant_bits)?;
                return Some(
                    se.get_add_rec_expr(cx, vec![ScevUse::of(start), ScevUse::of(step)], header, Flags::default()).scev,
                );
            }
            None
        }
        // Over add operands that do not overflow.
        ScevKind::Add(operands) => {
            if ignore_significant_bits || is_add_sextable(se, cx, lhs) {
                let mut ops = Vec::with_capacity(operands.len());
                for op in operands {
                    ops.push(ScevUse::of(get_exact_sdiv(se, cx, op.scev, rhs, ignore_significant_bits)?));
                }
                return Some(se.get_add_expr(cx, ops, Flags::default(), 0).scev);
            }
            None
        }
        // A multiply operand RHS can be pulled out of.
        ScevKind::Mul(operands) => {
            if ignore_significant_bits || is_mul_sextable(se, cx, lhs) {
                // C1*X*Y /s C2*X*Y.
                if let ScevKind::Mul(rhs_operands) = se.node(rhs).kind.clone()
                    && (ignore_significant_bits || is_mul_sextable(se, cx, rhs))
                    && let (Some(_), Some(_)) = (se.as_constant(operands[0].scev), se.as_constant(rhs_operands[0].scev))
                    && operands[1..] == rhs_operands[1..]
                {
                    return get_exact_sdiv(se, cx, operands[0].scev, rhs_operands[0].scev, ignore_significant_bits);
                }
                let mut ops = Vec::with_capacity(operands.len());
                let mut found = false;
                for op in operands {
                    let mut s = op.scev;
                    if !found && let Some(q) = get_exact_sdiv(se, cx, s, rhs, ignore_significant_bits) {
                        s = q;
                        found = true;
                    }
                    ops.push(ScevUse::of(s));
                }
                return found.then(|| se.get_mul_expr(cx, ops, Flags::default(), 0).scev);
            }
            None
        }
        _ => None,
    }
}

/// `ExtractImmediateOperand`: the constant at the front of `ops`, which the
/// sort puts first, replaced by zero.
fn extract_immediate_operand(
    se: &mut ScalarEvolution,
    cx: &Cx,
    ops: &mut [ScevUse],
) -> Immediate {
    let first = ops[0].scev;
    if let Some(c) = se.as_constant(first)
        && !c.is_zero()
        && c.significant_bits() <= 64
    {
        ops[0] = ScevUse::of(se.get_constant_of_type(cx, se.type_of(first), 0));
        return Immediate(c.sext_value() as i64);
    }
    Immediate::ZERO
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L956
/// `ExtractImmediate`: the constant integer added in `s`, and `s` without it.
pub fn extract_immediate(
    se: &mut ScalarEvolution,
    cx: &Cx,
    s: Scev,
) -> (Immediate, Scev) {
    match se.node(s).kind.clone() {
        ScevKind::Add(mut ops) => {
            let mut result = extract_immediate_operand(se, cx, &mut ops);
            if result.is_zero() {
                let (found, rest) = extract_immediate(se, cx, ops[0].scev);
                result = found;
                if result.is_non_zero() {
                    ops[0] = ScevUse::of(rest);
                }
            }
            if result.is_non_zero() {
                return (result, se.get_add_expr(cx, ops, Flags::default(), 0).scev);
            }
            (result, s)
        }
        ScevKind::AddRec(mut ops, header) => {
            let (result, rest) = extract_immediate(se, cx, ops[0].scev);
            if result.is_non_zero() {
                ops[0] = ScevUse::of(rest);
                return (result, se.get_add_rec_expr(cx, ops, header, Flags::default()).scev);
            }
            (result, s)
        }
        _ => {
            let mut one = [ScevUse::of(s)];
            let result = extract_immediate_operand(se, cx, &mut one);
            (result, if result.is_non_zero() { one[0].scev } else { s })
        }
    }
}

/// `ExtractSymbol`: the global whose address is added in `s`, and `s` without
/// it.
pub fn extract_symbol(
    se: &mut ScalarEvolution,
    cx: &Cx,
    s: Scev,
) -> (Option<GlobalId>, Scev) {
    match se.node(s).kind.clone() {
        ScevKind::Unknown(Operand::Constant(id)) => {
            if let llrm_mir::context::ConstantKind::Global(global) = cx.context.get(id).kind {
                return (Some(global), se.get_constant_of_type(cx, se.type_of(s), 0));
            }
            (None, s)
        }
        ScevKind::Add(mut ops) => {
            let last = ops.len() - 1;
            let (result, rest) = extract_symbol(se, cx, ops[last].scev);
            if result.is_some() {
                ops[last] = ScevUse::of(rest);
                return (result, se.get_add_expr(cx, ops, Flags::default(), 0).scev);
            }
            (None, s)
        }
        ScevKind::AddRec(mut ops, header) => {
            let (result, rest) = extract_symbol(se, cx, ops[0].scev);
            if result.is_some() {
                ops[0] = ScevUse::of(rest);
                return (result, se.get_add_rec_expr(cx, ops, header, Flags::default()).scev);
            }
            (None, s)
        }
        _ => (None, s),
    }
}

/// What a call to `name` is for the checks of an address use: `llvm.memset`,
/// `llvm.prefetch` and the like.
fn intrinsic_name<'a>(
    cx: &'a Cx,
    inst: InstId,
) -> Option<&'a str> {
    let callee = llrm_mir::memory::callee(cx.context, cx.function, inst)?;
    // A declaration made since the module's globals were read has no name here
    // (DEVIATE 2).
    cx.globals.get(callee.0 as usize)?.name.as_deref().filter(|name| name.starts_with("llvm."))
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1025
/// `isAddressUse`: whether `inst` uses `operand` as an address.
pub fn is_address_use(
    cx: &Cx,
    inst: InstId,
    operand: Operand,
) -> bool {
    let instruction = cx.function.instruction(inst);
    match &instruction.opcode {
        Opcode::Load { .. } => true,
        Opcode::Store { .. } => instruction.operands[1] == operand,
        Opcode::Call(_) => {
            let Some(name) = intrinsic_name(cx, inst) else { return false };
            let arg = |at: usize| instruction.operands.get(at) == Some(&operand);
            if name.starts_with("llvm.memset")
                || name.starts_with("llvm.prefetch")
                || name.starts_with("llvm.masked.load")
            {
                arg(0)
            } else if name.starts_with("llvm.masked.store") {
                arg(1)
            } else if name.starts_with("llvm.memmove") || name.starts_with("llvm.memcpy") {
                arg(0) || arg(1)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// The address space of the pointer type `ty`, if it is one.
fn address_space(
    cx: &Cx,
    ty: llrm_mir::types::TypeId,
) -> Option<u32> {
    match cx.context.types.get(ty) {
        MirType::Pointer(space) => Some(*space),
        _ => None,
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1074
/// `getAccessType`: the type of the memory `inst` accesses through `operand`.
pub fn get_access_type(
    cx: &Cx,
    inst: InstId,
    operand: Operand,
) -> MemAccessTy {
    let instruction = cx.function.instruction(inst);
    let mut access = MemAccessTy::unknown(MemAccessTy::UNKNOWN_ADDRESS_SPACE);
    match &instruction.opcode {
        Opcode::Load { .. } => {
            access.mem_ty = Some(instruction.ty);
            if let Some(space) =
                cx.function.operand_type(cx.context, instruction.operands[0]).and_then(|t| address_space(cx, t))
            {
                access.addr_space = space;
            }
        }
        Opcode::Store { .. } => {
            access.mem_ty = cx.function.operand_type(cx.context, instruction.operands[0]);
            if let Some(space) =
                cx.function.operand_type(cx.context, instruction.operands[1]).and_then(|t| address_space(cx, t))
            {
                access.addr_space = space;
            }
        }
        Opcode::Call(_) => {
            let name = intrinsic_name(cx, inst).unwrap_or("");
            let space_of = |at: usize| {
                instruction
                    .operands
                    .get(at)
                    .and_then(|&one| cx.function.operand_type(cx.context, one))
                    .and_then(|t| address_space(cx, t))
            };
            let operand_ty = cx.function.operand_type(cx.context, operand);
            if name.starts_with("llvm.prefetch") || name.starts_with("llvm.memset") {
                access.addr_space = space_of(0).unwrap_or(access.addr_space);
                access.mem_ty = operand_ty;
            } else if name.starts_with("llvm.memmove") || name.starts_with("llvm.memcpy") {
                access.addr_space = operand_ty.and_then(|t| address_space(cx, t)).unwrap_or(access.addr_space);
                access.mem_ty = operand_ty;
            } else if name.starts_with("llvm.masked.load") {
                access.addr_space = space_of(0).unwrap_or(access.addr_space);
            } else if name.starts_with("llvm.masked.store") {
                access.addr_space = space_of(1).unwrap_or(access.addr_space);
            }
        }
        _ => {}
    }
    access
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L2049
/// `isAlwaysFoldable`: whether an address mode with the immediate, and a base
/// register and a scale, is always folded into the use.
pub fn is_always_foldable(
    tti: &dyn Tti,
    kind: Kind,
    addr_space: u32,
    base_gv: bool,
    base_offset: Immediate,
    has_base_reg: bool,
) -> bool {
    // Zero is always foldable.
    if base_offset.is_zero() && !base_gv {
        return true;
    }
    // Conservatively, an immediate with a base and a scale.
    let mut scale = if kind == Kind::ICmpZero { -1 } else { 1 };
    let mut has_base_reg = has_base_reg;
    // A scale of 1 is the base register, if the formula has none.
    if !has_base_reg && scale == 1 {
        scale = 0;
        has_base_reg = true;
    }
    is_am_completely_folded_at(tti, kind, addr_space, base_gv, base_offset, has_base_reg, scale)
}

/// `isAlwaysFoldable(TTI, SE, MinOffset, MaxOffset, Kind, AccessTy, S,
/// HasBaseReg)`: whether the whole of `s`, a symbol and an immediate, folds.
pub fn is_always_foldable_scev(
    tti: &dyn Tti,
    se: &mut ScalarEvolution,
    cx: &Cx,
    min_offset: Immediate,
    max_offset: Immediate,
    kind: Kind,
    addr_space: u32,
    s: Scev,
    has_base_reg: bool,
) -> bool {
    if se.match_zero(s) {
        return true;
    }
    let (base_offset, rest) = extract_immediate(se, cx, s);
    let (base_gv, rest) = extract_symbol(se, cx, rest);
    // Anything else involved does not fold.
    if !se.match_zero(rest) {
        return false;
    }
    if base_offset.is_zero() && base_gv.is_none() {
        return true;
    }
    let scale = if kind == Kind::ICmpZero { -1 } else { 1 };
    is_am_completely_folded_between(
        tti,
        min_offset,
        max_offset,
        kind,
        addr_space,
        base_gv.is_some(),
        base_offset,
        has_base_reg,
        scale,
    )
}

// ---- the collection ----

/// What `LSRInstance` holds while it collects: the uses, which registers they
/// use, the interesting factors and types, and the baseline cost.
pub struct Collection<'a, 'b> {
    pub cx: &'a Cx<'b>,
    pub tti: &'a dyn Tti,
    /// The header of the loop.
    pub l: BlockId,
    /// `Rewriter.isSafeToExpand`.
    pub safe_to_expand: &'a dyn Fn(&mut ScalarEvolution, Scev) -> bool,
    pub hardware_loop_profitable: bool,
    /// Where the IV increments go: the latch's terminator, or a point that
    /// dominates the post-increment tests.
    pub iv_inc_insert_pos: InstId,
    pub iv_inc_block: BlockId,
    pub uses: Vec<Use>,
    use_map: HashMap<(Scev, Kind), usize>,
    pub reg_uses: RegUseTracker,
    /// Interesting factors between use strides.
    pub factors: IndexSet<i64>,
    /// Interesting use types, to reuse by truncation.
    pub types: IndexSet<Ty>,
    pub baseline_cost: Cost,
    pub changed: bool,
    /// The equality compares whose operands the caller is to swap, so the
    /// operand to replace is the left one.
    pub swapped_compares: Vec<InstId>,
    /// How many formulae were offered to `insert_formula`: the work generation
    /// did.
    pub insert_attempts: usize,
    /// How many formulae were rated: the work the filter, narrowing and solver
    /// did.
    pub rated: std::cell::Cell<usize>,
    /// The operands that IV chains take over (user and operand index): not
    /// fixups.
    pub chain_uses: HashSet<(InstId, usize)>,
}

impl<'a, 'b> Collection<'a, 'b> {
    pub fn new(
        cx: &'a Cx<'b>,
        tti: &'a dyn Tti,
        l: BlockId,
        safe_to_expand: &'a dyn Fn(&mut ScalarEvolution, Scev) -> bool,
        hardware_loop_profitable: bool,
        iv_inc_insert_pos: InstId,
    ) -> Self {
        let iv_inc_block = cx.function.parent(iv_inc_insert_pos).expect("a placed instruction");
        Self {
            cx,
            tti,
            l,
            safe_to_expand,
            hardware_loop_profitable,
            iv_inc_block,
            iv_inc_insert_pos,
            uses: Vec::new(),
            use_map: HashMap::default(),
            reg_uses: RegUseTracker::default(),
            factors: IndexSet::default(),
            types: IndexSet::default(),
            baseline_cost: Cost::new(),
            changed: false,
            swapped_compares: Vec::new(),
            insert_attempts: 0,
            rated: std::cell::Cell::new(0),
            chain_uses: HashSet::default(),
        }
    }

    /// `reconcileNewOffset`: whether use `lu_idx` can take a fixup at
    /// `new_offset` with these details; if so it is widened to.
    pub fn reconcile_new_offset(
        &mut self,
        lu_idx: usize,
        new_offset: Immediate,
        has_base_reg: bool,
        kind: Kind,
        access_ty: MemAccessTy,
    ) -> bool {
        let lu = &self.uses[lu_idx];
        let (mut new_min, mut new_max) = (lu.min_offset, lu.max_offset);
        let mut new_access = access_ty;
        // A different kind is not folded into this one: that could cost a use
        // with all its fixups outside the loop its special handling.
        if lu.kind != kind {
            return false;
        }
        // A different access type falls back to an unknown one.
        if kind == Kind::Address && access_ty.mem_ty != lu.access_ty.mem_ty {
            new_access = MemAccessTy::unknown(access_ty.addr_space);
        }
        // Conservatively, a base register is assumed.
        if new_offset < lu.min_offset {
            let span = Immediate(lu.max_offset.0.wrapping_sub(new_offset.0));
            if !is_always_foldable(self.tti, kind, access_ty.addr_space, false, span, has_base_reg) {
                return false;
            }
            new_min = new_offset;
        } else if new_offset > lu.max_offset {
            let span = Immediate(new_offset.0.wrapping_sub(lu.min_offset.0));
            if !is_always_foldable(self.tti, kind, access_ty.addr_space, false, span, has_base_reg) {
                return false;
            }
            new_max = new_offset;
        }
        let lu = &mut self.uses[lu_idx];
        lu.min_offset = new_min;
        lu.max_offset = new_max;
        lu.access_ty = new_access;
        true
    }

    /// `getUse`: the index of a use and the offset of a fixup that needs
    /// `expr`, with this kind and access type, and `expr` without that
    /// offset. Either an existing use is reused or a new one made.
    pub fn get_use(
        &mut self,
        se: &mut ScalarEvolution,
        expr: Scev,
        kind: Kind,
        access_ty: MemAccessTy,
    ) -> (usize, Immediate, Scev) {
        let (mut offset, mut stripped) = extract_immediate(se, self.cx, expr);
        // A basic use cannot take any offset.
        if !is_always_foldable(self.tti, kind, access_ty.addr_space, false, offset, true) {
            stripped = expr;
            offset = Immediate::ZERO;
        }
        if let Some(&lu_idx) = self.use_map.get(&(stripped, kind))
            && self.reconcile_new_offset(lu_idx, offset, true, kind, access_ty)
        {
            return (lu_idx, offset, stripped);
        }
        // A new use.
        let lu_idx = self.uses.len();
        self.use_map.insert((stripped, kind), lu_idx);
        let mut lu = Use::new(kind, access_ty);
        lu.min_offset = offset;
        lu.max_offset = offset;
        self.uses.push(lu);
        (lu_idx, offset, stripped)
    }

    /// `DeleteUse`: the last use takes its place.
    pub fn delete_use(
        &mut self,
        lu_idx: usize,
    ) {
        self.uses.swap_remove(lu_idx);
        self.reg_uses.swap_and_drop_use(lu_idx, self.uses.len());
    }

    /// `FindUseWithSimilarFormula`: a use other than `orig` with a formula that
    /// has the registers, symbol, scale and unfolded offset of `f` and no base
    /// offset.
    pub fn find_use_with_similar_formula(
        &self,
        f: &Formula,
        orig: usize,
    ) -> Option<usize> {
        let orig_lu = &self.uses[orig];
        for (idx, lu) in self.uses.iter().enumerate() {
            // Not an ICmpZero: it may hold formulae from
            // GenerateICmpZeroScales, where adding fixup offsets
            // may be invalid.
            if idx != orig
                && lu.kind != Kind::ICmpZero
                && lu.kind == orig_lu.kind
                && orig_lu.access_ty == lu.access_ty
                && lu.has_formula_with_same_regs(f)
            {
                for g in &lu.formulae {
                    if g.base_regs == f.base_regs
                        && g.scaled_reg == f.scaled_reg
                        && g.base_gv == f.base_gv
                        && g.scale == f.scale
                        && g.unfolded_offset == f.unfolded_offset
                    {
                        if g.base_offset.is_zero() {
                            return Some(idx);
                        }
                        // All matched but the offset: there are no others.
                        break;
                    }
                }
            }
        }
        None
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L2957
    /// `CollectInterestingTypesAndFactors`: the types of the uses, and the
    /// constant ratios between the strides of the loop.
    pub fn collect_interesting_types_and_factors(
        &mut self,
        se: &mut ScalarEvolution,
        users: &IvUsers,
    ) {
        let cx = self.cx;
        let mut strides: IndexSet<Scev> = IndexSet::default();
        for u in &users.uses {
            let expr = IvUsers::expr(se, cx, u);
            self.types.insert(se.get_effective_scev_type(cx, se.type_of(expr)));
            // Strides of the loops mentioned.
            let mut worklist = vec![expr];
            while let Some(s) = worklist.pop() {
                match se.node(s).kind.clone() {
                    ScevKind::AddRec(operands, header) => {
                        if header == self.l {
                            strides.insert(se.get_step_recurrence(cx, s));
                        }
                        worklist.push(operands[0].scev);
                    }
                    ScevKind::Add(operands) => worklist.extend(operands.iter().map(|op| op.scev)),
                    _ => {}
                }
            }
        }
        // The factors between each pair of strides.
        let strides: Vec<Scev> = strides.into_iter().collect();
        for (i, &first) in strides.iter().enumerate() {
            for &second in &strides[i + 1..] {
                let (mut old, mut new) = (first, second);
                let (old_bits, new_bits) =
                    (se.get_type_size_in_bits(cx, se.type_of(old)), se.get_type_size_in_bits(cx, se.type_of(new)));
                if old_bits != new_bits {
                    if old_bits > new_bits {
                        new = se.get_sign_extend_expr(cx, ScevUse::of(new), se.type_of(old), 0);
                    } else {
                        old = se.get_sign_extend_expr(cx, ScevUse::of(old), se.type_of(new), 0);
                    }
                }
                let factor = get_exact_sdiv(se, cx, new, old, true)
                    .and_then(|s| se.as_constant(s))
                    .or_else(|| get_exact_sdiv(se, cx, old, new, true).and_then(|s| se.as_constant(s)));
                if let Some(factor) = factor
                    && factor.significant_bits() <= 64
                    && !factor.is_zero()
                {
                    self.factors.insert(factor.sext_value() as i64);
                }
            }
        }
        // All uses of one type: no reuse by truncation to look for.
        if self.types.len() == 1 {
            self.types.clear();
        }
    }

    /// `IsFixupExecutedEachIncrement`: whether the fixup runs each time the IV
    /// increment does.
    pub fn is_fixup_executed_each_increment(
        &self,
        fixup: &Fixup,
    ) -> bool {
        let block = self.cx.function.parent(fixup.user).expect("a placed instruction");
        self.cx.tree.dominates(block, self.iv_inc_block)
    }

    /// `CountRegisters`: the registers `f` uses, noted for use `lu_idx`.
    pub fn count_registers(
        &mut self,
        f: &Formula,
        lu_idx: usize,
    ) {
        if let Some(scaled) = f.scaled_reg {
            self.reg_uses.count_register(scaled, lu_idx);
        }
        for &reg in &f.base_regs {
            self.reg_uses.count_register(reg, lu_idx);
        }
    }

    /// `InsertFormula`: adds `f` to use `lu_idx` unless it is there.
    pub fn insert_formula(
        &mut self,
        se: &ScalarEvolution,
        lu_idx: usize,
        f: &Formula,
    ) -> bool {
        self.insert_attempts += 1;
        let lu = &self.uses[lu_idx];
        // A formula that cannot be expanded is not inserted.
        assert!(
            is_legal_use_formula(self.tti, lu.min_offset, lu.max_offset, lu.kind, lu.access_ty.addr_space, f),
            "Formula is illegal"
        );
        if !self.uses[lu_idx].insert_formula(se, f, self.l) {
            return false;
        }
        self.count_registers(f, lu_idx);
        true
    }

    /// `InsertInitialFormula`: a formula for `s` in use `lu_idx`, its
    /// loop-variant part apart from the invariant and computable parts.
    pub fn insert_initial_formula(
        &mut self,
        se: &mut ScalarEvolution,
        s: Scev,
        lu_idx: usize,
    ) {
        // A use whose expression cannot be expanded stays as it is.
        if !(self.safe_to_expand)(se, s) {
            self.uses[lu_idx].rigid_formula = true;
        }
        let mut f = Formula::default();
        f.initial_match(se, self.cx, s, self.l);
        let inserted = self.insert_formula(se, lu_idx, &f);
        assert!(inserted, "Initial formula already exists!");
    }

    /// `InsertSupplementalFormula`: the single-register formula for `s`.
    pub fn insert_supplemental_formula(
        &mut self,
        se: &ScalarEvolution,
        s: Scev,
        lu_idx: usize,
    ) {
        let f = Formula { base_regs: vec![s], has_base_reg: true, ..Formula::default() };
        let inserted = self.insert_formula(se, lu_idx, &f);
        assert!(inserted, "Supplemental formula already exists!");
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L3584
    /// `CollectFixupsAndInitialFormulae`: a fixup for each IV user, in its use,
    /// and each use's first formula.
    pub fn collect_fixups_and_initial_formulae(
        &mut self,
        se: &mut ScalarEvolution,
        users: &IvUsers,
    ) {
        let cx = self.cx;
        let lp = cx.loops.get_loop_for(self.l).expect("a loop");
        // For the baseline cost.
        let mut regs: HashSet<Scev> = HashSet::default();
        let visited_regs: HashSet<Scev> = HashSet::default();
        let mut visited_uses: HashSet<usize> = HashSet::default();

        for u in &users.uses {
            let instruction = cx.function.instruction(u.user);
            // A user in a profitable chain is the chain's.
            let at = instruction.operands.iter().position(|&operand| operand == u.operand);
            if at.is_some_and(|at| self.chain_uses.contains(&(u.user, at))) {
                continue;
            }
            let mut kind = Kind::Basic;
            let mut access_ty = MemAccessTy { mem_ty: None, addr_space: 0 };
            if is_address_use(cx, u.user, u.operand) {
                kind = Kind::Address;
                access_ty = get_access_type(cx, u.user, u.operand);
            }
            let mut s = IvUsers::expr(se, cx, u);
            let post_inc_loops = u.post_inc_loops.clone();

            // Equality compares are special: `i == n` is `n - i == 0`, so the
            // registers of both sides are weighed together. Every interesting
            // loop has them, thanks to indvars.
            if let Opcode::ICmp(predicate) = instruction.opcode
                && matches!(predicate, IntPredicate::Eq | IntPredicate::Ne)
            {
                // The operand to replace goes on the left.
                let nv = if instruction.operands[1] == u.operand {
                    self.swapped_compares.push(u.user);
                    self.changed = true;
                    instruction.operands[0]
                } else {
                    instruction.operands[1]
                };
                let n = se.get_scev(cx, nv);
                let is_pointer =
                    cx.function.operand_type(cx.context, nv).is_some_and(|ty| address_space(cx, ty).is_some());
                if se.is_loop_invariant(cx, n, Some(self.l))
                    && (self.safe_to_expand)(se, n)
                    && (!is_pointer || se.get_pointer_base(n) == se.get_pointer_base(s))
                {
                    // `s` is normalized, so `n` is too before they combine.
                    let Some(n) = se.normalize_for_post_inc_use(cx, n, &post_inc_loops, true) else { continue };
                    kind = Kind::ICmpZero;
                    s = se.get_minus_scev(cx, ScevUse::of(n), ScevUse::of(s), 0, 0);
                } else if lp.is_loop_invariant(cx.function, nv)
                    && match nv {
                        Operand::Value(v) => match cx.function.value(v).def {
                            llrm_mir::module::ValueDef::Instruction(def) => {
                                let block = cx.function.parent(def).expect("a placed instruction");
                                block != self.l && cx.tree.dominates(block, self.l)
                            }
                            llrm_mir::module::ValueDef::Argument(_) => true,
                        },
                        _ => true,
                    }
                    && !is_pointer
                {
                    // An expression that cannot be expanded in general (it has
                    // a divide) but is already available
                    // before the loop is wrapped in an
                    // unknown, so the expander does not redo it.
                    let n = se.get_unknown(cx, nv);
                    let Some(n) = se.normalize_for_post_inc_use(cx, n, &post_inc_loops, true) else { continue };
                    kind = Kind::ICmpZero;
                    s = se.get_minus_scev(cx, ScevUse::of(n), ScevUse::of(s), 0, 0);
                    assert!(!se.is_could_not_compute(s));
                }
                // -1 and the negation of every factor (but of -1) are
                // interesting now.
                for i in 0..self.factors.len() {
                    let factor = self.factors[i];
                    if factor != -1 {
                        self.factors.insert(factor.wrapping_neg());
                    }
                }
                self.factors.insert(-1);
            }

            // A use, found or made.
            let (lu_idx, offset, stripped) = self.get_use(se, s, kind, access_ty);
            s = stripped;

            // The fixup.
            let mut fixup = Fixup::new(u.user, u.operand);
            fixup.post_inc_loops = post_inc_loops;
            fixup.offset = offset;
            let outside = fixup.is_use_fully_outside_loop(cx, self.l);
            let unconditional = self.is_fixup_executed_each_increment(&fixup);
            let lu = &mut self.uses[lu_idx];
            lu.push_fixup(fixup);
            lu.all_fixups_outside_loop &= outside;
            lu.all_fixups_unconditional &= unconditional;

            // A formula for the baseline cost.
            if !visited_uses.contains(&lu_idx) && !outside {
                let mut f = Formula::default();
                f.initial_match(se, cx, s, self.l);
                let mut rating = Rating {
                    se,
                    cx,
                    tti: self.tti,
                    l: self.l,
                    hardware_loop_profitable: self.hardware_loop_profitable,
                    as_written: true,
                };
                self.baseline_cost.rate_formula(&mut rating, &f, &mut regs, &visited_regs, &self.uses[lu_idx], None);
                visited_uses.insert(lu_idx);
            }

            // The first fixup of a use gives it a formula.
            if self.uses[lu_idx].formulae.is_empty() {
                self.insert_initial_formula(se, s, lu_idx);
            }
        }
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L3774
    /// `CollectLoopInvariantFixupsAndFormulae`: other uses of the
    /// loop-invariant values in the registers are fixups too: they pin
    /// those values in registers, making them less profitable to eliminate.
    pub fn collect_loop_invariant_fixups_and_formulae(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        // Outside uses are not collected when post-indexed addressing is
        // favored: the instructions in the loop matter more.
        if self.tti.preferred_addressing_mode() == AddressingMode::PostIndexed {
            return;
        }
        let cx = self.cx;
        let lp = cx.loops.get_loop_for(self.l).expect("a loop");
        let mut worklist: Vec<Scev> = self.reg_uses.registers().to_vec();
        let mut visited: HashSet<Scev> = HashSet::default();
        while let Some(s) = worklist.pop() {
            if !visited.insert(s) {
                continue;
            }
            match se.node(s).kind.clone() {
                ScevKind::Add(ops)
                | ScevKind::Mul(ops)
                | ScevKind::AddRec(ops, _)
                | ScevKind::UMax(ops)
                | ScevKind::SMax(ops)
                | ScevKind::UMin(ops)
                | ScevKind::SMin(ops)
                | ScevKind::SequentialUMin(ops) => worklist.extend(ops.iter().map(|op| op.scev)),
                ScevKind::Truncate(op)
                | ScevKind::ZeroExtend(op)
                | ScevKind::SignExtend(op)
                | ScevKind::PtrToAddr(op) => worklist.push(op.scev),
                ScevKind::UDiv(lhs, rhs) => worklist.extend([lhs.scev, rhs.scev]),
                ScevKind::Unknown(Operand::Value(value)) => {
                    // An instruction defined in the loop is not invariant;
                    // arguments are.
                    if let llrm_mir::module::ValueDef::Instruction(def) = cx.function.value(value).def
                        && lp.contains_instruction(cx.function, def)
                    {
                        continue;
                    }
                    let uses: Vec<llrm_mir::module::Use> = cx.function.users(value).iter().copied().collect();
                    for u in uses {
                        let user = cx.function.instruction(u.user);
                        // A landing pad is no place for new instructions.
                        if matches!(user.opcode, Opcode::LandingPad { .. }) {
                            continue;
                        }
                        // For a phi, the use is in the incoming block.
                        let use_block = if user.opcode == Opcode::Phi {
                            let Operand::Block(from) = user.operands[u.index as usize + 1] else {
                                unreachable!("a phi's block")
                            };
                            from
                        } else {
                            cx.function.parent(u.user).expect("a placed instruction")
                        };
                        // Not instructions the loop does not dominate.
                        if !cx.tree.dominates(self.l, use_block) {
                            continue;
                        }
                        // Not uses that are part of other SCEV expressions,
                        // analysed once already.
                        if let Some(result) = user.result
                            && se.is_value_scevable(cx, Operand::Value(result))
                        {
                            let user_s = se.get_scev(cx, Operand::Value(result));
                            // A no-op user: look through to its uses.
                            if se.scev_type(user_s) != Type::Unknown {
                                continue;
                            }
                            if user_s == s {
                                worklist.push(se.get_unknown(cx, Operand::Value(result)));
                                continue;
                            }
                        }
                        // Not compares that are already analysed.
                        if matches!(user.opcode, Opcode::ICmp(_)) {
                            let other = user.operands[usize::from(u.index == 0)];
                            let other_s = se.get_scev(cx, other);
                            if se.has_computable_loop_evolution(cx, other_s, Some(self.l)) {
                                continue;
                            }
                        }
                        // Not lifetime markers: they are not materialized.
                        if intrinsic_name(cx, u.user).is_some_and(|name| name.starts_with("llvm.lifetime.")) {
                            continue;
                        }

                        let (lu_idx, offset, stripped) =
                            self.get_use(se, s, Kind::Basic, MemAccessTy { mem_ty: None, addr_space: 0 });
                        let _ = stripped;
                        let mut fixup = Fixup::new(u.user, Operand::Value(value));
                        fixup.offset = offset;
                        let outside = fixup.is_use_fully_outside_loop(cx, self.l);
                        let unconditional = self.is_fixup_executed_each_increment(&fixup);
                        let lu = &mut self.uses[lu_idx];
                        lu.push_fixup(fixup);
                        lu.all_fixups_outside_loop &= outside;
                        lu.all_fixups_unconditional &= unconditional;
                        self.insert_supplemental_formula(se, s, lu_idx);
                        break;
                    }
                }
                _ => {}
            }
        }
    }
}
