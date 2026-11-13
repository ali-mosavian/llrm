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
//! DEVIATE: the pre-indexed addressing branches (`AMK_PreIndexed`; no target
//! of ours prefers it) are not ported; the post-indexed ones are.
//! Work per use is bounded by the formulae it has, which the caps bound.
//!
//! DEVIATE LG1: `GenerateICmpZeroScales` asks `getExactSDiv` for no overflow
//! of the product, but for an odd factor `x == 0` and `x * c == 0` are the same
//! compare (multiplying by it is a bijection mod 2^bits), so it ignores the
//! significant bits there, for the counter alone compared to zero. The pin
//! keeps a loop's exit counter beside the scaled one it is a multiple of
//! (recsum: `dec; add -3; jne`); this exits on the scaled one (`add -3; jne`),
//! as the stand-in LSR did. Exit: none, a proof; if LLVM does the same, delete
//! the line. A target turns LG1 and LG2 off with
//! `Tti::lsr_scales_zero_compare_past_overflow` (wasm: the pin's formulae).
//!
//! DEVIATE LG2: for an even factor `getExactSDiv` fails where the compared
//! value cannot reach 2^(bits - ctz(factor)) under the loop's guards, although
//! only 0 then has a product of 0. Without it a byte-stepped loop kept a second
//! counter beside the byte index (examples/dot m16: `dec cx` and a spill).
//! Exit: none, a proof; if LLVM's SCEV proves the same, delete the check.
//! Not at size: the doubled limit costs an `add` and a `cmp` over the `dec`
//! (m16 -Os corpus: idioms +51 B, idiom_edges +15, none smaller).

use std::collections::BTreeMap;

use llrm_analysis::scev::expr::{Kind as ScevKind, Scev, Ty, Use as ScevUse};
use llrm_analysis::scev::{Flags, ScalarEvolution};
use llrm_mir::context::GlobalId;
use llrm_mir::tti::{AddressingMode, Ty as TtiTy};
use llrm_support::hash::{HashMap, HashSet};

use crate::lsr_collect::{Collection, extract_immediate, extract_symbol, get_exact_sdiv, is_always_foldable_scev};
use crate::lsr_formula::{Formula, Immediate, Symbol};
use crate::lsr_use::{Cost, Kind, Rating, is_legal_add_immediate, is_legal_use, may_use_post_inc_mode};

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
            for op in operands.iter().copied() {
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
        let Some(mode) = f.mode() else { return false };
        is_legal_use(
            self.tti,
            lu.min_offset,
            lu.max_offset,
            lu.kind,
            lu.access_ty,
            mode.global,
            f.base_offset,
            mode.has_base_reg,
            mode.scale,
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
        // No reassociation of the base register of a value that may be
        // post-incremented: it would make base+register formulae that win,
        // and the post-increment is better.
        if self.tti.preferred_addressing_mode() == AddressingMode::PostIndexed
            && may_use_post_inc_mode(self.tti, se, cx, &self.uses[lu_idx], reg, self.l)
        {
            return;
        }
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
                (lu.min_offset, lu.max_offset, lu.kind, lu.access_ty)
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
        debug_assert!(base.is_canonical(se, self.l), "Input must be in the canonical form");
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
        let (gv, g) = extract_symbol(se, self.cx, Self::reg_at(base, slot), self.tti.frame_slot_is_free_base());
        let Some(gv) = gv else { return };
        if se.match_zero(g) {
            // DEVIATE LU30: a frame slot that was a register of its own is
            // the frame register and a displacement; the register goes.
            if let (Symbol::Frame(_), Slot::Base(at)) = (gv, slot) {
                let mut f = base.clone();
                f.symbol = Some(gv);
                f.delete_base_reg(at);
                f.has_base_reg = !f.base_regs.is_empty();
                f.canonicalize(se, self.l);
                if self.legal(lu_idx, &f) {
                    let _ = self.insert_formula(se, lu_idx, &f);
                }
            }
            return;
        }
        let mut f = base.clone();
        f.symbol = Some(gv);
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
        if base.symbol.is_some() {
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
        assert!(base.symbol.is_none(), "ICmpZero use is not legal!");

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
                let Some(mode) = f.mode() else { continue };
                if !is_legal_use(
                    self.tti,
                    offset,
                    offset,
                    lu.kind,
                    lu.access_ty,
                    mode.global,
                    f.base_offset,
                    mode.has_base_reg,
                    mode.scale,
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
            let counter_alone = base.scaled_reg.is_none() && base.base_regs.len() == 1;
            let odd = self.tti.lsr_scales_zero_compare_past_overflow()
                && ((factor & 1 == 1 && counter_alone)
                    || (!self.size && self.factor_keeps_zero(se, cx, &base, factor, bits)));
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

    /// `x * c == 0` iff `x` is a multiple of 2^(bits - ctz(c)), so for an even
    /// `c` the compared value `x` of `base` keeps its zero test when its range
    /// under the loop's guards stays below that: only 0 is such a multiple.
    fn factor_keeps_zero(
        &self,
        se: &mut ScalarEvolution,
        cx: &llrm_analysis::scev::Cx,
        base: &Formula,
        factor: i64,
        bits: u32,
    ) -> bool {
        if factor & 1 == 1 {
            return false;
        }
        let mut parts: Vec<ScevUse> = base.base_regs.iter().map(|&r| ScevUse::of(r)).collect();
        if let Some(scaled) = base.scaled_reg {
            let scale = se.get_constant_of_type(cx, se.type_of(scaled), i128::from(base.scale));
            parts.push(se.get_mul_expr(cx, vec![ScevUse::of(scaled), ScevUse::of(scale)], Flags::default(), 0));
        }
        let x = se.get_add_expr(cx, parts, Flags::default(), 0).scev;
        let ops = se.operands(x).to_vec();
        // `applyLoopGuards` leaves a recurrence's start alone, so guard that.
        let guarded = if ops.len() == 2 && se.is_affine(x) {
            let start = se.apply_loop_guards(cx, ops[0].scev, self.l);
            se.get_add_rec_expr(cx, vec![ScevUse::of(start), ops[1]], self.l, Flags::default()).scev
        } else {
            se.apply_loop_guards(cx, x, self.l)
        };
        let lo = se.get_signed_range_min(cx, guarded).sext_value();
        let hi = se.get_signed_range_max(cx, guarded).sext_value();
        zero_test_survives(bits, factor, lo.unsigned_abs().max(hi.unsigned_abs()))
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
                (lu.min_offset, lu.max_offset, lu.kind, lu.all_fixups_outside_loop, lu.access_ty)
            };
            let legal = |kind| {
                base.mode().is_some_and(|mode| {
                    is_legal_use(
                        self.tti,
                        min,
                        max,
                        kind,
                        space,
                        mode.global,
                        base.base_offset,
                        mode.has_base_reg,
                        mode.scale,
                    )
                })
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
            if kind == Kind::ICmpZero && !base.has_base_reg && base.base_offset.is_zero() && base.symbol.is_none() {
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
        if base.symbol.is_some() {
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
                            if self.tti.preferred_addressing_mode() == AddressingMode::PostIndexed
                                && may_use_post_inc_mode(self.tti, se, cx, &self.uses[lu_idx], orig_reg, self.l)
                            {
                                continue;
                            }
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
        GENERATED.with(|n| n.set(n.get() + 1));
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

    /// `GenerateEndBiased` (DEVIATE LG3, gcc's `ubase - ratio * cbase`,
    /// `get_computation_aff_1`): an address that steps by `r` times the step of
    /// a register `Z` that ends at zero on the exit trip is `(a - r * start(Z))
    /// + r * Z`. Its formula shares `Z` with the exit test, so the loop keeps
    /// one counter and the bases sit at the end of their arrays. Neither the
    /// pin nor gcc builds it: the pin's address uses never name the exit
    /// test's register, and gcc compares against the end instead.
    ///
    /// The biased bases are new registers the pin's solution does not hold;
    /// where a base names a global's address each costs the target's setup
    /// (`Tti::lsr_symbol_base_setup_insns`), which the solution's cost ignores:
    /// `plan_loop` prices it in the keep rule.
    ///
    /// Work: one formula for each (zero-ending register, address formula
    /// holding an addrec of this loop), the registers capped at four.
    pub fn generate_end_biased(
        &mut self,
        se: &mut ScalarEvolution,
    ) {
        // A loop instruction saved costs setup ones; at -Os that is bytes lost.
        if self.size {
            return;
        }
        let cx = self.cx;
        // The saving is one compare a trip; the biased bases cost setup
        // instructions once. A loop that runs few trips (an unroll's
        // remainder) never pays them back.
        let trips = se.get_small_constant_max_trip_count(cx, self.l);
        if trips != 0 && trips <= END_BIAS_SETUP_INSNS {
            return;
        }
        // The registers that end at zero, in the exit test's formulae.
        let mut zero_ending: Vec<Scev> = Vec::new();
        for lu in &self.uses {
            if lu.kind != Kind::ICmpZero {
                continue;
            }
            for f in &lu.formulae {
                if f.has_zero_end()
                    && let Some((_, _, header)) = se.match_affine_add_rec(f.base_regs[0])
                    && header == self.l
                    && !zero_ending.contains(&f.base_regs[0])
                {
                    zero_ending.push(f.base_regs[0]);
                }
            }
        }
        zero_ending.truncate(4);
        if zero_ending.is_empty() {
            return;
        }
        for lu_idx in 0..self.uses.len() {
            if self.uses[lu_idx].kind != Kind::Address {
                continue;
            }
            let (min, max, kind, space) = {
                let lu = &self.uses[lu_idx];
                (lu.min_offset, lu.max_offset, lu.kind, lu.access_ty)
            };
            for i in 0..self.uses[lu_idx].formulae.len() {
                let f = self.uses[lu_idx].formulae[i].clone();
                if f.scaled_reg.is_some() {
                    continue;
                }
                for &z in &zero_ending {
                    let Some((z_start, z_step, _)) = se.match_affine_add_rec(z) else { continue };
                    for at in 0..f.base_regs.len() {
                        let reg = f.base_regs[at];
                        let Some((r_start, r_step, header)) = se.match_affine_add_rec(reg) else { continue };
                        if header != self.l || se.type_of(r_step) != se.type_of(z) {
                            continue;
                        }
                        // The address steps by a whole number of Z's steps.
                        let Some(ratio) = get_exact_sdiv(se, cx, r_step, z_step, false) else { continue };
                        let Some(r) = se.as_constant(ratio).map(|c| c.sext_value() as i64) else { continue };
                        if r == 0 || !legal_scale(self.tti, min, max, kind, space, &f, r) {
                            continue;
                        }
                        // a - r * start(Z), with the other registers of the
                        // formula: one invariant register in the preheader.
                        let ratio_s = se.get_constant_of_type(cx, se.type_of(z), -i128::from(r));
                        let shifted = se
                            .get_mul_expr(cx, vec![ScevUse::of(ratio_s), ScevUse::of(z_start)], Flags::default(), 0)
                            .scev;
                        let bias = se
                            .get_add_expr(cx, vec![ScevUse::of(r_start), ScevUse::of(shifted)], Flags::default(), 0)
                            .scev;
                        let mut f2 = f.clone();
                        f2.delete_base_reg(at);
                        let mut sums: Vec<ScevUse> = f2.base_regs.iter().map(|&b| ScevUse::of(b)).collect();
                        sums.push(ScevUse::of(bias));
                        let base = se.get_add_expr(cx, sums, Flags::default(), 0).scev;
                        if !se.is_loop_invariant(cx, base, Some(self.l)) {
                            continue;
                        }
                        f2.base_regs.clear();
                        if se.as_constant(base).is_none_or(|c| !c.is_zero()) {
                            f2.base_regs.push(base);
                        }
                        f2.has_base_reg = !f2.base_regs.is_empty();
                        f2.scaled_reg = Some(z);
                        f2.scale = r;
                        f2.canonicalize(se, self.l);
                        let _ = self.insert_formula(se, lu_idx, &f2);
                    }
                }
            }
        }
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
            as_written: false,
            other_register_units: self.pressure_units(),
            size: self.size,
            bytes_counted: self.bytes_counted,
            across_call: self.across_call,
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

/// Instructions the biased bases of an end-biased formula cost in the
/// preheader: the start scaled and negated, and the sum.
const END_BIAS_SETUP_INSNS: u32 = 3;

thread_local! {
    static GENERATED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread generated a loop's reuse formulae, for a test
/// that a loop's space is generated once however many solutions are rated.
pub fn reuse_formulae_generated() -> usize {
    GENERATED.with(std::cell::Cell::get)
}

/// Whether `x * factor == 0` (mod 2^bits) iff `x == 0` for every `x` with
/// `|x| <= reach`: the product is zero iff `x` is a multiple of
/// 2^(bits - ctz(factor)).
pub(crate) fn zero_test_survives(
    bits: u32,
    factor: i64,
    reach: u128,
) -> bool {
    let kept = bits.saturating_sub(factor.trailing_zeros());
    kept > 0 && kept < 128 && reach < 1u128 << kept
}

/// Whether `f` with its scaled register taken `scale` times is a legal
/// address of the use.
fn legal_scale(
    tti: &dyn llrm_mir::tti::Tti,
    min: Immediate,
    max: Immediate,
    kind: Kind,
    access: crate::lsr_formula::MemAccessTy,
    f: &Formula,
    scale: i64,
) -> bool {
    // The end-biased form is not made for a frame slot.
    !matches!(f.symbol, Some(Symbol::Frame(_)))
        && is_legal_use(tti, min, max, kind, access, f.has_global(), f.base_offset, true, scale)
}
