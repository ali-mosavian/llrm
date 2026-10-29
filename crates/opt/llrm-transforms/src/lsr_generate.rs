//! The formulae LLVM's loop strength reduction generates for each use
//! (`GenerateReassociations`, `GenerateCombinations`,
//! `GenerateSymbolicOffsets`, `GenerateConstantOffsets`,
//! `GenerateICmpZeroScales`, `GenerateScales`, `GenerateTruncates`,
//! `GenerateCrossUseConstantOffsets`, `GenerateAllReuseFormulae` and
//! `FilterOutUndesirableDedicatedRegisters`; LoopStrengthReduce.cpp at
//! `d1106deb71cc`, l.3880-5000).
//!
//! # How it works
//! From a use's first formula, each generator makes the others the target could
//! take: an add split into several registers or several combined into one, a
//! global moved into the address field, an offset moved between the immediate
//! and a register, a factor of a stride folded into the scale, a wider
//! induction variable truncated, and a register a constant apart from another
//! use's. Each formula is offered to [`Collection::insert_formula`], which
//! keeps one per set of registers. The recursion is capped (depth 3, and
//! sub-expression depth 3), as LLVM's is.
//!
//! # Example
//! For the compare `{(-1 + %n),+,-1}`: `%n` with `{-1,+,-1}` scaled, and `(-1 +
//! %n)` with `{0,+,-1}` scaled. For the store, `%p - 4 + 4*{1,+,1}`, sharing
//! the compare's register.
//!
//! # Runs
//! Nothing runs it yet: stage 4 of the translation of LSR.
//!
//! # Limits
//! DEVIATE: the pre- and post-indexed addressing branches
//! (`AMK_PreIndexed`, `AMK_PostIndexed`; x86 prefers none) are not ported.
//! Work per use is bounded by the formulae it has, which the caps bound.
//!
//! DEVIATE LG1: `GenerateICmpZeroScales` asks `getExactSDiv` for no overflow
//! of the product, but for an odd factor `x == 0` and `x * c == 0` are the same
//! compare (multiplying by it is a bijection mod 2^bits), so it ignores the
//! significant bits there, for the counter alone compared to zero. The pin
//! keeps a loop's exit counter beside the scaled one it is a multiple of
//! (recsum: `dec; add -3; jne`); this exits on the scaled one (`add -3; jne`),
//! as the stand-in LSR did. Exit: none, a proof; if LLVM does the same, delete
//! the line.

use std::collections::BTreeMap;

use llrm_analysis::scev::expr::{Kind as ScevKind, Scev, Ty, Use as ScevUse};
use llrm_analysis::scev::{Flags, ScalarEvolution};
use llrm_mir::context::GlobalId;
use llrm_mir::tti::Ty as TtiTy;
use llrm_support::hash::{HashMap, HashSet};

use crate::lsr_collect::{Collection, extract_immediate, extract_symbol, get_exact_sdiv, is_always_foldable_scev};
use crate::lsr_formula::{Formula, Immediate};
use crate::lsr_use::{Cost, Kind, Rating, is_legal_add_immediate, is_legal_use};

/// Which register of a formula a generator works on.
#[derive(Clone, Copy)]
enum Slot {
    Base(usize),
    Scaled,
}

/// Whether `value` is representable in `bits` signed bits
/// (`ConstantInt::isValueValidForType`).
fn valid_for_bits(
    bits: u32,
    value: i64,
) -> bool {
    bits >= 64 || (value >= -(1i64 << (bits - 1)) && value < (1i64 << (bits - 1)))
}

/// `Log2_32`.
fn log2_floor(n: usize) -> u32 {
    31 - (n as u32).leading_zeros()
}

/// The cost model's view of a SCEV type.
pub(crate) fn tti_ty(ty: Ty) -> TtiTy {
    match ty {
        Ty::Int(bits) => TtiTy::Int(bits),
        Ty::Ptr(_) => TtiTy::Pointer,
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L3880
/// `CollectSubexprs`: split `s` into subexpressions that can be separate
/// registers, each times `c` if given. Returns the remainder, none when `ops`
/// is complete.
fn collect_subexprs(
    se: &mut ScalarEvolution,
    cx: &llrm_analysis::scev::Cx,
    s: Scev,
    c: Option<Scev>,
    ops: &mut Vec<Scev>,
    l: llrm_mir::module::BlockId,
    depth: u32,
) -> Option<Scev> {
    // Capped, to protect compile time.
    if depth >= 3 {
        return Some(s);
    }
    let times = |se: &mut ScalarEvolution, c: Option<Scev>, r: Scev| match c {
        Some(c) => se.get_mul_expr(cx, vec![ScevUse::of(c), ScevUse::of(r)], Flags::default(), 0).scev,
        None => r,
    };
    match se.node(s).kind.clone() {
        ScevKind::Add(operands) => {
            for op in operands {
                if let Some(remainder) = collect_subexprs(se, cx, op.scev, c, ops, l, depth + 1) {
                    let r = times(se, c, remainder);
                    ops.push(r);
                }
            }
            None
        }
        // Split a non-zero base out of an affine addrec.
        ScevKind::AddRec(operands, header) if operands.len() == 2 => {
            let (start, step) = (operands[0].scev, operands[1].scev);
            if se.match_zero(start) {
                return Some(s);
            }
            let mut remainder = collect_subexprs(se, cx, start, c, ops, l, depth + 1);
            // Split the addrec unless it is part of a nested recurrence that
            // does not pertain to this loop.
            if let Some(r) = remainder
                && (header == l || se.addrec_header(r).is_none())
            {
                let r = times(se, c, r);
                ops.push(r);
                remainder = None;
            }
            if remainder != Some(start) {
                let remainder = remainder.unwrap_or_else(|| se.get_constant_of_type(cx, se.type_of(s), 0));
                return Some(
                    se.get_add_rec_expr(cx, vec![ScevUse::of(remainder), ScevUse::of(step)], header, Flags::default())
                        .scev,
                );
            }
            Some(s)
        }
        // C * (a + b + c) is C*a + C*b + C*c.
        ScevKind::Mul(operands) if operands.len() == 2 && se.as_constant(operands[0].scev).is_some() => {
            let op0 = operands[0].scev;
            let c = Some(match c {
                Some(c) => se.get_mul_expr(cx, vec![ScevUse::of(c), ScevUse::of(op0)], Flags::default(), 0).scev,
                None => op0,
            });
            if let Some(remainder) = collect_subexprs(se, cx, operands[1].scev, c, ops, l, depth + 1) {
                let product = se
                    .get_mul_expr(
                        cx,
                        vec![ScevUse::of(c.expect("a factor")), ScevUse::of(remainder)],
                        Flags::default(),
                        0,
                    )
                    .scev;
                ops.push(product);
            }
            None
        }
        _ => Some(s),
    }
}

impl Collection<'_, '_> {
    /// The formula `f` of use `lu_idx` is legal as it is: used before
    /// inserting a variant.
    fn legal(
        &self,
        lu_idx: usize,
        f: &Formula,
    ) -> bool {
        let lu = &self.uses[lu_idx];
        is_legal_use(
            self.tti,
            lu.min_offset,
            lu.max_offset,
            lu.kind,
            lu.access_ty.addr_space,
            f.base_gv.is_some(),
            f.base_offset,
            f.has_base_reg,
            f.scale,
        )
    }

    fn reg_at(
        base: &Formula,
        slot: Slot,
    ) -> Scev {
        match slot {
            Slot::Base(at) => base.base_regs[at],
            Slot::Scaled => base.scaled_reg.expect("a scaled register"),
        }
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L3979
    /// `GenerateReassociationsImpl`: the register at `slot`, with its add split
    /// into pieces, each piece in turn apart from the rest.
    fn generate_reassociations_impl(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: &Formula,
        depth: u32,
        slot: Slot,
    ) {
        let cx = self.cx;
        let reg = Self::reg_at(base, slot);
        let mut add_ops = Vec::new();
        if let Some(remainder) = collect_subexprs(se, cx, reg, None, &mut add_ops, self.l, 0) {
            add_ops.push(remainder);
        }
        if add_ops.len() == 1 {
            return;
        }
        for (j, &piece) in add_ops.clone().iter().enumerate() {
            // A loop-variant unknown is of no use.
            if se.scev_type(piece) == llrm_analysis::scev::expr::Type::Unknown
                && !se.is_loop_invariant(cx, piece, Some(self.l))
            {
                continue;
            }
            let (min, max, kind, space) = {
                let lu = &self.uses[lu_idx];
                (lu.min_offset, lu.max_offset, lu.kind, lu.access_ty.addr_space)
            };
            // A constant that could be folded into an immediate stays there.
            if is_always_foldable_scev(self.tti, se, cx, min, max, kind, space, piece, base.num_regs() > 1) {
                continue;
            }
            // All operands but this one.
            let inner: Vec<ScevUse> =
                add_ops.iter().enumerate().filter(|&(at, _)| at != j).map(|(_, &op)| ScevUse::of(op)).collect();
            // Not just a constant left behind in a register, if it could fold.
            if inner.len() == 1
                && is_always_foldable_scev(self.tti, se, cx, min, max, kind, space, inner[0].scev, base.num_regs() > 1)
            {
                continue;
            }
            let inner_sum = se.get_add_expr(cx, inner, Flags::default(), 0).scev;
            if se.match_zero(inner_sum) {
                continue;
            }
            let mut f = base.clone();

            // The rest of the add goes back into the new formula.
            let fits = |se: &ScalarEvolution, s: Scev| se.as_constant(s).filter(|c| c.width <= 64);
            let sum_const = fits(se, inner_sum);
            let unfolded_with = |f: &Formula, c: u128| Immediate(f.unfolded_offset.0.wrapping_add(c as u64 as i64));
            if let Some(c) = sum_const
                && self.tti.is_legal_add_immediate(unfolded_with(&f, c.zext_value()).0)
            {
                f.unfolded_offset = unfolded_with(&f, c.zext_value());
                match slot {
                    Slot::Scaled => {
                        f.scaled_reg = None;
                        f.scale = 0;
                    }
                    Slot::Base(at) => {
                        f.base_regs.remove(at);
                    }
                }
            } else {
                match slot {
                    Slot::Scaled => f.scaled_reg = Some(inner_sum),
                    Slot::Base(at) => f.base_regs[at] = inner_sum,
                }
            }

            // The piece as its own register, or an unfolded immediate.
            if let Some(c) = fits(se, piece)
                && self.tti.is_legal_add_immediate(unfolded_with(&f, c.zext_value()).0)
            {
                f.unfolded_offset = unfolded_with(&f, c.zext_value());
            } else {
                f.base_regs.push(piece);
            }
            // The number of base registers may have changed.
            f.canonicalize(se, self.l);

            if self.insert_formula(se, lu_idx, &f) {
                // A new formula: look for more like it. The depth also counts
                // 1 per power of 16 in the number of pieces, which bounds the
                // compile time better than the depth alone.
                let next = self.uses[lu_idx].formulae.last().expect("the inserted formula").clone();
                self.generate_reassociations(se, lu_idx, next, depth + 1 + (log2_floor(add_ops.len()) >> 2));
            }
        }
    }

    /// `GenerateReassociations`: split out subexpressions from adds and the
    /// bases of addrecs.
    pub fn generate_reassociations(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: Formula,
        depth: u32,
    ) {
        assert!(base.is_canonical(se, self.l), "Input must be in the canonical form");
        // Capped, to protect compile time.
        if depth >= 3 {
            return;
        }
        for i in 0..base.base_regs.len() {
            self.generate_reassociations_impl(se, lu_idx, &base, depth, Slot::Base(i));
        }
        if base.scale == 1 {
            self.generate_reassociations_impl(se, lu_idx, &base, depth, Slot::Scaled);
        }
    }

    /// `GenerateCombinations`: a formula with all the registers that dominate
    /// the loop added into one.
    pub fn generate_combinations(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        mut base: Formula,
    ) {
        let cx = self.cx;
        // Of interest only with several registers.
        if base.base_regs.len() + usize::from(base.scale == 1) + usize::from(base.unfolded_offset.is_non_zero()) <= 1 {
            return;
        }
        // reg1 + 1*reg2 is reg1 + reg2, before anything else.
        base.unscale();
        let mut ops = Vec::new();
        let mut new_base = base.clone();
        new_base.base_regs.clear();
        let mut combined_ty = None;
        for &reg in &base.base_regs {
            if se.properly_dominates(cx, reg, self.l) && !se.has_computable_loop_evolution(cx, reg, Some(self.l)) {
                combined_ty.get_or_insert_with(|| se.get_effective_scev_type(cx, se.type_of(reg)));
                ops.push(reg);
            } else {
                new_base.base_regs.push(reg);
            }
        }
        // No register is relevant.
        if ops.is_empty() {
            return;
        }
        let formula = |this: &mut Self, se: &mut ScalarEvolution, new_base: &Formula, sum: Scev| {
            // A zero sum is a fold SCEV missed: not a register to hold it.
            if se.match_zero(sum) {
                return;
            }
            let mut f = new_base.clone();
            f.base_regs.push(sum);
            f.canonicalize(se, this.l);
            let _ = this.insert_formula(se, lu_idx, &f);
        };
        // Two or more registers: a formula combining them.
        if ops.len() > 1 {
            let sum = se.get_add_expr(cx, ops.iter().map(|&op| ScevUse::of(op)).collect(), Flags::default(), 0).scev;
            formula(self, se, &new_base, sum);
        }
        // An unfolded offset: one combining it with the registers.
        if new_base.unfolded_offset.is_non_zero() {
            let ty = combined_ty.expect("a type for the unfolded offset");
            ops.push(se.get_constant_of_type(cx, ty, i128::from(new_base.unfolded_offset.0)));
            new_base.unfolded_offset = Immediate::ZERO;
            let sum = se.get_add_expr(cx, ops.iter().map(|&op| ScevUse::of(op)).collect(), Flags::default(), 0).scev;
            formula(self, se, &new_base, sum);
        }
    }

    /// `GenerateSymbolicOffsetsImpl`.
    fn generate_symbolic_offsets_impl(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: &Formula,
        slot: Slot,
    ) {
        let (gv, g) = extract_symbol(se, self.cx, Self::reg_at(base, slot));
        let Some(gv) = gv else { return };
        if se.match_zero(g) {
            return;
        }
        let mut f = base.clone();
        f.base_gv = Some(gv);
        if !self.legal(lu_idx, &f) {
            return;
        }
        match slot {
            Slot::Scaled => f.scaled_reg = Some(g),
            Slot::Base(at) => f.base_regs[at] = g,
        }
        let _ = self.insert_formula(se, lu_idx, &f);
    }

    /// `GenerateSymbolicOffsets`: formulae with a global in the address field.
    pub fn generate_symbolic_offsets(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: Formula,
    ) {
        // An address with a global has no room for another.
        if base.base_gv.is_some() {
            return;
        }
        for i in 0..base.base_regs.len() {
            self.generate_symbolic_offsets_impl(se, lu_idx, &base, Slot::Base(i));
        }
        if base.scale == 1 {
            self.generate_symbolic_offsets_impl(se, lu_idx, &base, Slot::Scaled);
        }
    }

    /// `GenerateConstantOffsetsImpl`.
    fn generate_constant_offsets_impl(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: &Formula,
        worklist: &[Immediate],
        slot: Slot,
    ) {
        let cx = self.cx;
        let g = Self::reg_at(base, slot);
        for &offset in worklist {
            let mut f = base.clone();
            f.base_offset = base.base_offset.sub_unsigned(offset);
            if !self.legal(lu_idx, &f) {
                continue;
            }
            // The offset goes to the base register.
            let new_offset = offset.scev(se, cx, se.type_of(g));
            let new_g = se.get_add_expr(cx, vec![ScevUse::of(new_offset), ScevUse::of(g)], Flags::default(), 0).scev;
            // If it cancelled out, the register goes.
            if se.match_zero(new_g) {
                match slot {
                    Slot::Scaled => {
                        f.scale = 0;
                        f.scaled_reg = None;
                    }
                    Slot::Base(at) => f.delete_base_reg(at),
                }
                f.canonicalize(se, self.l);
            } else {
                match slot {
                    Slot::Scaled => f.scaled_reg = Some(new_g),
                    Slot::Base(at) => f.base_regs[at] = new_g,
                }
            }
            let _ = self.insert_formula(se, lu_idx, &f);
        }

        // The immediate in the register itself.
        let (imm, g) = extract_immediate(se, cx, g);
        if se.match_zero(g) || imm.is_zero() {
            return;
        }
        let mut f = base.clone();
        f.base_offset = f.base_offset.add_unsigned(imm);
        if !self.legal(lu_idx, &f) {
            return;
        }
        match slot {
            Slot::Scaled => f.scaled_reg = Some(g),
            Slot::Base(at) => {
                f.base_regs[at] = g;
                // `g` may be a recurrence of the loop while the scaled
                // register is not.
                f.canonicalize(se, self.l);
            }
        }
        let _ = self.insert_formula(se, lu_idx, &f);
    }

    /// `GenerateConstantOffsets`: formulae with the use's least and greatest
    /// offsets moved between the immediate and the registers.
    pub fn generate_constant_offsets(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: Formula,
    ) {
        // Only the extremes: what is between is seldom worth it.
        let lu = &self.uses[lu_idx];
        let mut worklist = vec![lu.min_offset];
        if lu.max_offset != lu.min_offset {
            worklist.push(lu.max_offset);
        }
        for i in 0..base.base_regs.len() {
            self.generate_constant_offsets_impl(se, lu_idx, &base, &worklist, Slot::Base(i));
        }
        if base.scale == 1 {
            self.generate_constant_offsets_impl(se, lu_idx, &base, &worklist, Slot::Scaled);
        }
    }

    /// `GenerateICmpZeroScales`: a compare scaled up, `x == y` as `x*c == y*c`.
    pub fn generate_icmp_zero_scales(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: Formula,
    ) {
        let cx = self.cx;
        let lu = &self.uses[lu_idx];
        if lu.kind != Kind::ICmpZero {
            return;
        }
        let Some(int_ty) = base.ty(se, cx) else { return };
        let bits = se.get_type_size_in_bits(cx, int_ty);
        if bits > 64 {
            return;
        }
        // Not with more than one offset.
        if lu.min_offset != lu.max_offset {
            return;
        }
        let min_offset = lu.min_offset;
        // It is illegal to multiply a pointer.
        if base.scaled_reg.is_some_and(|reg| se.type_of(reg).is_pointer())
            || base.base_regs.iter().any(|&reg| se.type_of(reg).is_pointer())
        {
            return;
        }
        assert!(base.base_gv.is_none(), "ICmpZero use is not legal!");

        'factor: for factor in self.factors.clone() {
            if !valid_for_bits(bits, factor) {
                continue;
            }
            // The multiplication must not overflow.
            if base.base_offset.is_min() && factor == -1 {
                continue;
            }
            let new_base_offset = base.base_offset.mul_unsigned(factor);
            assert!(factor != 0, "Zero factor not expected!");
            if new_base_offset.0.checked_div(factor) != Some(base.base_offset.0) {
                continue;
            }
            // An offset truncated at this use must be in bounds.
            if !int_ty.is_pointer() && !valid_for_bits(bits, new_base_offset.0) {
                continue;
            }
            // Nor must it with the use's offset.
            if min_offset.is_min() && factor == -1 {
                continue;
            }
            let offset = min_offset.mul_unsigned(factor);
            if offset.0.checked_div(factor) != Some(min_offset.0) {
                continue;
            }
            if !int_ty.is_pointer() && !valid_for_bits(bits, offset.0) {
                continue;
            }

            let mut f = base.clone();
            f.base_offset = new_base_offset;
            // This scale must be legal.
            {
                let lu = &self.uses[lu_idx];
                if !is_legal_use(
                    self.tti,
                    offset,
                    offset,
                    lu.kind,
                    lu.access_ty.addr_space,
                    f.base_gv.is_some(),
                    f.base_offset,
                    f.has_base_reg,
                    f.scale,
                ) {
                    continue;
                }
            }
            // The use has its minimum offset built in.
            f.base_offset = f.base_offset.add_unsigned(offset).sub_unsigned(min_offset);

            let factor_s = se.get_constant_of_type(cx, int_ty, i128::from(factor));
            // DEVIATE LG1: `x == 0` and `x * c == 0` are the same compare for
            // an odd `c` (multiplying by it is a bijection of the
            // integers mod 2^bits), so the product may wrap where
            // LLVM's `getExactSDiv` wants it not to
            // (`isMulSExtable`). Only for the counter alone
            // compared to zero: with a limit besides it the scaled limit is a
            // multiply in the preheader and an add in the loop, which the cost
            // model prices below `dec; je` (x_adler -O2: 47972 -> 50976
            // clocks).
            let odd = factor & 1 == 1 && base.scaled_reg.is_none() && base.base_regs.len() == 1;
            // No base register may overflow when multiplied.
            for i in 0..f.base_regs.len() {
                f.base_regs[i] = se
                    .get_mul_expr(cx, vec![ScevUse::of(f.base_regs[i]), ScevUse::of(factor_s)], Flags::default(), 0)
                    .scev;
                if get_exact_sdiv(se, cx, f.base_regs[i], factor_s, odd) != Some(base.base_regs[i]) {
                    continue 'factor;
                }
            }
            // Nor the scaled register.
            if let Some(scaled) = f.scaled_reg {
                let product =
                    se.get_mul_expr(cx, vec![ScevUse::of(scaled), ScevUse::of(factor_s)], Flags::default(), 0).scev;
                f.scaled_reg = Some(product);
                if get_exact_sdiv(se, cx, product, factor_s, odd) != base.scaled_reg {
                    continue;
                }
            }
            // Nor the unfolded offset.
            if f.unfolded_offset.is_non_zero() {
                if f.unfolded_offset.is_min() && factor == -1 {
                    continue;
                }
                f.unfolded_offset = f.unfolded_offset.mul_unsigned(factor);
                if f.unfolded_offset.0.checked_div(factor) != Some(base.unfolded_offset.0) {
                    continue;
                }
                if !int_ty.is_pointer() && !valid_for_bits(bits, f.unfolded_offset.0) {
                    continue;
                }
            }
            // Legal: add it.
            let _ = self.insert_formula(se, lu_idx, &f);
        }
    }

    /// `GenerateScales`: formulae using a factor of the strides as the scale of
    /// a scaled-offset address mode, for example.
    pub fn generate_scales(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        mut base: Formula,
    ) {
        let cx = self.cx;
        let Some(int_ty) = base.ty(se, cx) else { return };
        // A formula with a scaled register cannot take another: unscale it to
        // find a better scale.
        if base.scale != 0 && !base.unscale() {
            return;
        }
        assert_eq!(base.scale, 0, "unscale did not did its job!");

        for factor in self.factors.clone() {
            base.scale = factor;
            base.has_base_reg = base.base_regs.len() > 1;
            // Whether this scale will be legal.
            let (min, max, kind, outside, space) = {
                let lu = &self.uses[lu_idx];
                (lu.min_offset, lu.max_offset, lu.kind, lu.all_fixups_outside_loop, lu.access_ty.addr_space)
            };
            let legal = |kind| {
                is_legal_use(
                    self.tti,
                    min,
                    max,
                    kind,
                    space,
                    base.base_gv.is_some(),
                    base.base_offset,
                    base.has_base_reg,
                    base.scale,
                )
            };
            if !legal(kind) {
                // An out-of-loop basic use is special, and takes a -1 scale.
                if kind == Kind::Basic && legal(Kind::Special) && outside {
                    self.uses[lu_idx].kind = Kind::Special;
                } else {
                    continue;
                }
            }
            let kind = self.uses[lu_idx].kind;
            // Negating a solitary base register of a compare gives nothing new.
            if kind == Kind::ICmpZero && !base.has_base_reg && base.base_offset.is_zero() && base.base_gv.is_none() {
                continue;
            }
            // Each addrec base register of this loop is scaled.
            for i in 0..base.base_regs.len() {
                let reg = base.base_regs[i];
                let Some(header) = se.addrec_header(reg) else { continue };
                if header != self.l && !outside {
                    continue;
                }
                let factor_s = se.get_constant_of_type(cx, int_ty, i128::from(factor));
                if se.match_zero(factor_s) {
                    continue;
                }
                // The factor divides out, ignoring the high bits: the value is
                // scaled back up in the end.
                let Some(quotient) = get_exact_sdiv(se, cx, reg, factor_s, true) else { continue };
                if se.match_zero(quotient) {
                    continue;
                }
                let mut f = base.clone();
                f.scaled_reg = Some(quotient);
                f.delete_base_reg(i);
                // 1*reg is reg, which the base formula already is: it would be
                // rejected.
                if f.scale == 1 && (f.base_regs.is_empty() || (header != self.l && outside)) {
                    continue;
                }
                // With all fixups outside the loop and a scale of 1, the
                // scaled register's loop may not be this one.
                if f.scale == 1 && outside {
                    f.canonicalize(se, self.l);
                }
                let _ = self.insert_formula(se, lu_idx, &f);
            }
        }
    }

    /// `getAnyExtendConsideringPostIncUses`: `expr` extended to `to`, for each
    /// post-increment loop set of the fixups. Each is denormalized, extended
    /// and normalized again, as the normalized form folds in ways that are
    /// not valid in the post-inc context. They must all agree.
    fn any_extend_considering_post_inc_uses(
        se: &mut ScalarEvolution,
        cx: &llrm_analysis::scev::Cx,
        loops: &[Vec<llrm_mir::module::BlockId>],
        expr: Scev,
        to: Ty,
    ) -> Option<Scev> {
        let mut result: Option<Scev> = None;
        for l in loops {
            let denormalized = se.denormalize_for_post_inc_use(cx, expr, l);
            let extended = se.get_any_extend_expr(cx, ScevUse::of(denormalized), to);
            let new = se.normalize_for_post_inc_use(cx, extended, l, true)?;
            if result.is_some_and(|result| new != result) {
                return None;
            }
            result = Some(new);
        }
        Some(result.expect("failed to create expression"))
    }

    /// `GenerateTruncates`: formulae from a wider induction variable.
    pub fn generate_truncates(
        &mut self,
        se: &mut ScalarEvolution,
        lu_idx: usize,
        base: Formula,
    ) {
        let cx = self.cx;
        // Symbolic values are not truncated.
        if base.base_gv.is_some() {
            return;
        }
        let Some(dst) = base.ty(se, cx) else { return };
        if dst.is_pointer() {
            return;
        }
        // A pointer cannot be extended.
        if base.scaled_reg.is_some_and(|reg| se.type_of(reg).is_pointer())
            || base.base_regs.iter().any(|&reg| se.type_of(reg).is_pointer())
        {
            return;
        }
        let loops: Vec<Vec<llrm_mir::module::BlockId>> =
            self.uses[lu_idx].fixups.iter().map(|fixup| fixup.post_inc_loops.clone()).collect();
        for src in self.types.clone() {
            if src == dst || !self.tti.is_truncate_free(tti_ty(src), tti_ty(dst)) {
                continue;
            }
            let mut f = base.clone();
            // SCEV may prove zero while extending what it could not while
            // building the node (its depth limit).
            if let Some(scaled) = f.scaled_reg {
                match Self::any_extend_considering_post_inc_uses(se, cx, &loops, scaled, src) {
                    Some(new) if !se.match_zero(new) => f.scaled_reg = Some(new),
                    _ => continue,
                }
            }
            let mut has_zero = false;
            for reg in f.base_regs.iter_mut() {
                match Self::any_extend_considering_post_inc_uses(se, cx, &loops, *reg, src) {
                    Some(new) if !se.match_zero(new) => *reg = new,
                    _ => {
                        has_zero = true;
                        break;
                    }
                }
            }
            if has_zero {
                continue;
            }
            // This assumes the basic processing of all uses is done and the
            // register usage is known.
            if !f.has_regs_used_by_uses_other_than(lu_idx, &self.reg_uses) {
                continue;
            }
            f.canonicalize(se, self.l);
            let _ = self.insert_formula(se, lu_idx, &f);
        }
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L4608
    /// `GenerateCrossUseConstantOffsets`: registers a constant apart, with
    /// formulae that take the one from the other.
    pub fn generate_cross_use_constant_offsets(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        let cx = self.cx;
        // Group the registers by their value without a constant offset.
        let mut map: HashMap<Scev, BTreeMap<Immediate, Scev>> = HashMap::default();
        let mut used_by: HashMap<Scev, HashSet<usize>> = HashMap::default();
        let mut sequence = Vec::new();
        for &use_reg in self.reg_uses.registers() {
            let (imm, reg) = extract_immediate(se, cx, use_reg);
            let entry = map
                .entry(reg)
                .or_insert_with(
                    || {
                        sequence.push(reg);
                        BTreeMap::new()
                    },
                );
            entry.entry(imm).or_insert(use_reg);
            used_by.entry(reg).or_default().extend(self.reg_uses.used_by_indices(use_reg).set_bits());
        }

        // The work is found first and done after, so no formula or register
        // count changes under the search.
        struct Item {
            lu_idx: usize,
            imm: Immediate,
            orig_reg: Scev,
        }
        let mut items: Vec<Item> = Vec::new();
        let mut unique: HashSet<(usize, Immediate)> = HashSet::default();
        for &reg in &sequence {
            let imms = &map[&reg];
            // One offset is nothing to reuse.
            if imms.len() == 1 {
                continue;
            }
            for (&j_imm, &orig_reg) in imms {
                if se.as_constant(orig_reg).is_none() && used_by[&reg].len() == 1 {
                    continue;
                }
                // The offsets to the first, the last and the middle.
                let first = *imms.keys().next().expect("an offset");
                let last = *imms.keys().next_back().expect("an offset");
                // (First + Last) / 2 without overflow, rounded toward zero.
                let (fi, li) = (first.0, last.0);
                let mut avg = (fi & li) + ((fi ^ li) >> 1);
                avg += (fi ^ li) & (((avg as u64) >> 63) as i64);
                let middle = imms.range(Immediate(avg)..).next().map(|(&k, _)| k);
                for other in [Some(first), Some(last), middle].into_iter().flatten() {
                    if other == j_imm {
                        continue;
                    }
                    let imm = j_imm.sub_unsigned(other);
                    for lu_idx in self.reg_uses.used_by_indices(orig_reg).set_bits() {
                        if unique.insert((lu_idx, imm)) {
                            items.push(Item { lu_idx, imm, orig_reg });
                        }
                    }
                }
            }
        }

        for item in items {
            let Item { lu_idx, imm, orig_reg } = item;
            let int_ty = se.get_effective_scev_type(cx, se.type_of(orig_reg));
            let neg_imm = imm.negative_scev(se, cx, int_ty);
            let bits = se.get_type_size_in_bits(cx, int_ty);
            for at in 0..self.uses[lu_idx].formulae.len() {
                let mut f = self.uses[lu_idx].formulae[at].clone();
                f.unscale();
                // The immediate in the scaled register.
                if f.scaled_reg == Some(orig_reg) {
                    let offset = f.base_offset.add_unsigned(imm.mul_unsigned(f.scale));
                    // Not 50 + reg(-50).
                    let s = offset.negative_scev(se, cx, int_ty);
                    if f.references_reg(s) {
                        continue;
                    }
                    let mut new_f = f.clone();
                    new_f.base_offset = offset;
                    if !self.legal(lu_idx, &new_f) {
                        continue;
                    }
                    let scaled = new_f.scaled_reg.expect("a scaled register");
                    let new_scaled =
                        se.get_add_expr(cx, vec![ScevUse::of(neg_imm), ScevUse::of(scaled)], Flags::default(), 0).scev;
                    new_f.scaled_reg = Some(new_scaled);
                    // A constant in the register that brings the immediate no
                    // nearer zero makes the formula not worth it.
                    if let Some(c) = se.as_constant(new_scaled) {
                        let (negative, off) = (c.is_negative(), new_f.base_offset.0);
                        let mask = if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
                        let product = c.sext_value().unsigned_abs().wrapping_mul(f.scale as u128) & mask;
                        if negative != new_f.base_offset.is_less_than_zero()
                            && product <= u128::from(off.unsigned_abs()) & mask
                        {
                            continue;
                        }
                    }
                    new_f.canonicalize(se, self.l);
                    let _ = self.insert_formula(se, lu_idx, &new_f);
                } else {
                    // The immediate in a base register.
                    for n in 0..f.base_regs.len() {
                        let base_reg = f.base_regs[n];
                        if base_reg != orig_reg {
                            continue;
                        }
                        let mut new_f = f.clone();
                        new_f.base_offset = new_f.base_offset.add_unsigned(imm);
                        if !self.legal(lu_idx, &new_f) {
                            let unfolded = new_f.unfolded_offset.add_unsigned(imm);
                            if !is_legal_add_immediate(self.tti, unfolded) {
                                continue;
                            }
                            new_f = f.clone();
                            new_f.unfolded_offset = unfolded;
                        }
                        new_f.base_regs[n] = se
                            .get_add_expr(cx, vec![ScevUse::of(neg_imm), ScevUse::of(base_reg)], Flags::default(), 0)
                            .scev;
                        // A constant in a register that brings the immediate no
                        // nearer zero makes the formula not worth it.
                        let off = new_f.base_offset.0;
                        let skip = new_f
                            .base_regs
                            .iter()
                            .any(
                                |&reg| {
                                    let Some(c) = se.as_constant(reg) else { return false };
                                    let sum = c.sext_value().wrapping_add(i128::from(off));
                                    sum.unsigned_abs() < u128::from(off.unsigned_abs())
                                        && (sum as u64).trailing_zeros() >= (off as u64).trailing_zeros()
                                },
                            );
                        if skip {
                            continue;
                        }
                        new_f.canonicalize(se, self.l);
                        let _ = self.insert_formula(se, lu_idx, &new_f);
                        break;
                    }
                }
            }
        }
    }

    /// `GenerateAllReuseFormulae`: the formulae for each use. Split into loops
    /// so `hasRegsUsedByUsesOtherThan` is as precise as it can be.
    pub fn generate_all_reuse_formulae(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        for lu_idx in 0..self.uses.len() {
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                self.generate_reassociations(se, lu_idx, f, 0);
            }
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                self.generate_combinations(se, lu_idx, f);
            }
        }
        for lu_idx in 0..self.uses.len() {
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                self.generate_symbolic_offsets(se, lu_idx, f);
            }
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                self.generate_constant_offsets(se, lu_idx, f);
            }
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                self.generate_icmp_zero_scales(se, lu_idx, f);
            }
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                self.generate_scales(se, lu_idx, f);
            }
        }
        for lu_idx in 0..self.uses.len() {
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                self.generate_truncates(se, lu_idx, f);
            }
        }
        self.generate_cross_use_constant_offsets(se);
    }

    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L4792
    /// `FilterOutUndesirableDedicatedRegisters`: where formulae share the same
    /// registers other uses use, the best is kept and the others deleted.
    pub fn filter_out_undesirable_dedicated_registers(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        let visited: HashSet<Scev> = HashSet::default();
        let mut losers: HashSet<Scev> = HashSet::default();
        for lu_idx in 0..self.uses.len() {
            // The best formula for each unique set of shared registers, per
            // use.
            let mut best_formulae: HashMap<Vec<Scev>, usize> = HashMap::default();
            let mut any = false;
            let mut f_idx = 0;
            while f_idx < self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[f_idx].clone();
                // Some formulae lose at once (they depend on addrecs of other
                // loops that do not exist); the losers are recorded to spare
                // the work for the next that names one.
                let mut cost_f = Cost::new();
                let mut regs = HashSet::default();
                self.rate(se, &mut cost_f, &f, &mut regs, &visited, lu_idx, Some(&mut losers));
                let mut delete = true;
                if !cost_f.is_loser() {
                    let mut key: Vec<Scev> = f
                        .base_regs
                        .iter()
                        .copied()
                        .filter(|&reg| self.reg_uses.is_reg_used_by_uses_other_than(reg, lu_idx))
                        .collect();
                    if let Some(scaled) = f.scaled_reg
                        && self.reg_uses.is_reg_used_by_uses_other_than(scaled, lu_idx)
                    {
                        key.push(scaled);
                    }
                    key.sort_unstable();
                    match best_formulae.get(&key).copied() {
                        None => {
                            best_formulae.insert(key, f_idx);
                            delete = false;
                        }
                        Some(best_idx) => {
                            let best = self.uses[lu_idx].formulae[best_idx].clone();
                            let mut cost_best = Cost::new();
                            let mut regs = HashSet::default();
                            self.rate(se, &mut cost_best, &best, &mut regs, &visited, lu_idx, None);
                            if cost_f.is_less(self.tti, &cost_best) {
                                self.uses[lu_idx].formulae.swap(f_idx, best_idx);
                            }
                        }
                    }
                }
                if delete {
                    self.uses[lu_idx].delete_formula(f_idx);
                    any = true;
                } else {
                    f_idx += 1;
                }
            }
            // The registers of what is left.
            if any {
                let lu = &mut self.uses[lu_idx];
                lu.recompute_regs(lu_idx, &mut self.reg_uses);
            }
        }
    }

    /// `Cost::RateFormula` of `f` for use `lu_idx`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn rate(
        &self,
        se: &mut ScalarEvolution,
        cost: &mut Cost,
        f: &Formula,
        regs: &mut HashSet<Scev>,
        visited: &HashSet<Scev>,
        lu_idx: usize,
        losers: Option<&mut HashSet<Scev>>,
    ) {
        self.rated.set(self.rated.get() + 1);
        let mut rating = Rating {
            se,
            cx: self.cx,
            tti: self.tti,
            l: self.l,
            hardware_loop_profitable: self.hardware_loop_profitable,
        };
        cost.rate_formula(&mut rating, f, regs, visited, &self.uses[lu_idx], losers);
    }

    /// Whether `s` is the address of a global: its symbol.
    pub(crate) fn global_of(
        &self,
        se: &ScalarEvolution,
        s: Scev,
    ) -> Option<GlobalId> {
        match se.unknown_value(s)? {
            llrm_mir::module::Operand::Constant(id) => match self.cx.context.get(id).kind {
                llrm_mir::context::ConstantKind::Global(global) => Some(global),
                _ => None,
            },
            _ => None,
        }
    }
}
