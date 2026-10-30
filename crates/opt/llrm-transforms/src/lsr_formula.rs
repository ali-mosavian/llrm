//! The data of LLVM's loop strength reduction: `Immediate`, `RegUseTracker`
//! and `Formula` (LoopStrengthReduce.cpp at `d1106deb71cc`, l.212-793).
//!
//! # How it works
//! A `Formula` is one way to compute an address or a value for a use:
//! `BaseGV + BaseOffset + sum(BaseRegs) + Scale * ScaledReg + UnfoldedOffset`,
//! each register a SCEV. `canonicalize` keeps it in the shape the generators
//! and the solver expect. `RegUseTracker` says which uses reference which
//! register.
//!
//! # Example
//! `p[i + 2]` with 4-byte elements: base register `p`, scaled register `i` with
//! scale 4, base offset 8.
//!
//! # Runs
//! Nothing runs it yet: stage 1 of the translation of LSR (the old pass in
//! [`crate::lsr_pass`] is the pass that runs them).
//!
//! # Limits
//! DEVIATE: `Immediate` is fixed. LLVM's also holds a scalable quantity
//! (`vscale * n`) for SVE and RVV; no target here has scalable vectors, and
//! `ScalarEvolution` has no `vscale` node.

use llrm_analysis::scev::expr::{Scev, Ty, Use};
use llrm_analysis::scev::{Cx, Flags, ScalarEvolution};
use llrm_mir::context::GlobalId;
use llrm_mir::module::BlockId;
use llrm_mir::types::TypeId;
use llrm_support::hash::HashMap;

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L212
/// The type and address space of a memory access; `mem_ty` is none (LLVM's
/// `void`) where the access is unknown.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MemAccessTy {
    pub mem_ty: Option<TypeId>,
    pub addr_space: u32,
}

impl MemAccessTy {
    /// `UnknownAddressSpace`.
    pub const UNKNOWN_ADDRESS_SPACE: u32 = u32::MAX;

    /// `getUnknown`.
    pub fn unknown(addr_space: u32) -> Self {
        Self { mem_ty: None, addr_space }
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L250
/// An offset from an address, in bytes.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Immediate(pub i64);

impl Immediate {
    pub const ZERO: Immediate = Immediate(0);
    pub const FIXED_MIN: Immediate = Immediate(i64::MIN);
    pub const FIXED_MAX: Immediate = Immediate(i64::MAX);

    pub fn is_zero(self) -> bool {
        self.0 == 0
    }

    pub fn is_non_zero(self) -> bool {
        self.0 != 0
    }

    pub fn is_less_than_zero(self) -> bool {
        self.0 < 0
    }

    pub fn is_greater_than_zero(self) -> bool {
        self.0 > 0
    }

    pub fn is_min(self) -> bool {
        self.0 == i64::MIN
    }

    pub fn is_max(self) -> bool {
        self.0 == i64::MAX
    }

    /// `addUnsigned`: the sum as unsigned numbers, so it wraps.
    pub fn add_unsigned(
        self,
        rhs: Immediate,
    ) -> Immediate {
        Immediate(self.0.wrapping_add(rhs.0))
    }

    /// `subUnsigned`.
    pub fn sub_unsigned(
        self,
        rhs: Immediate,
    ) -> Immediate {
        Immediate(self.0.wrapping_sub(rhs.0))
    }

    /// `mulUnsigned`.
    pub fn mul_unsigned(
        self,
        rhs: i64,
    ) -> Immediate {
        Immediate(self.0.wrapping_mul(rhs))
    }

    /// `getSCEV`: the constant of type `ty`.
    pub fn scev(
        self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        ty: Ty,
    ) -> Scev {
        se.get_constant_of_type(cx, ty, i128::from(self.0))
    }

    /// `getNegativeSCEV`: the constant of type `ty`, negated as unsigned.
    pub fn negative_scev(
        self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        ty: Ty,
    ) -> Scev {
        se.get_constant_of_type(cx, ty, i128::from(self.0.wrapping_neg()))
    }
}

/// `SmallBitVector` of the use indices that reference a register.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UsedBy(Vec<bool>);

impl UsedBy {
    fn set(
        &mut self,
        index: usize,
    ) {
        if self.0.len() <= index {
            self.0.resize(index + 1, false);
        }
        self.0[index] = true;
    }

    fn reset(
        &mut self,
        index: usize,
    ) {
        self.0[index] = false;
    }

    /// `find_first`.
    pub fn first(&self) -> Option<usize> {
        self.0.iter().position(|&one| one)
    }

    /// `find_next`.
    pub fn next(
        &self,
        after: usize,
    ) -> Option<usize> {
        self.0.iter().skip(after + 1).position(|&one| one).map(|at| at + after + 1)
    }

    pub fn contains(
        &self,
        index: usize,
    ) -> bool {
        self.0.get(index).copied().unwrap_or(false)
    }

    /// `set_bits`: the indices that are set, ascending.
    pub fn set_bits(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().filter(|&(_, &one)| one).map(|(at, _)| at)
    }

    pub fn count(&self) -> usize {
        self.0.iter().filter(|&&one| one).count()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L386
/// Which uses reference each register, and the registers in the order they
/// were first counted.
#[derive(Clone, Debug, Default)]
pub struct RegUseTracker {
    used_by: HashMap<Scev, UsedBy>,
    sequence: Vec<Scev>,
}

impl RegUseTracker {
    pub fn count_register(
        &mut self,
        reg: Scev,
        lu_idx: usize,
    ) {
        let sequence = &mut self.sequence;
        self.used_by
            .entry(reg)
            .or_insert_with(|| {
                sequence.push(reg);
                UsedBy::default()
            })
            .set(lu_idx);
    }

    pub fn drop_register(
        &mut self,
        reg: Scev,
        lu_idx: usize,
    ) {
        self.used_by.get_mut(&reg).expect("a counted register").reset(lu_idx);
    }

    /// `swapAndDropUse`: use `last_lu_idx` takes the place of `lu_idx`, and the
    /// last slot goes. Walks every register, as LLVM's does.
    pub fn swap_and_drop_use(
        &mut self,
        lu_idx: usize,
        last_lu_idx: usize,
    ) {
        assert!(lu_idx <= last_lu_idx);
        for used in self.used_by.values_mut() {
            let bits = &mut used.0;
            if lu_idx < bits.len() {
                bits[lu_idx] = last_lu_idx < bits.len() && bits[last_lu_idx];
            }
            bits.truncate(bits.len().min(last_lu_idx));
        }
    }

    pub fn is_reg_used_by_uses_other_than(
        &self,
        reg: Scev,
        lu_idx: usize,
    ) -> bool {
        let Some(used) = self.used_by.get(&reg) else { return false };
        let Some(first) = used.first() else { return false };
        first != lu_idx || used.next(first).is_some()
    }

    pub fn used_by_indices(
        &self,
        reg: Scev,
    ) -> &UsedBy {
        self.used_by.get(&reg).expect("a counted register")
    }

    pub fn clear(&mut self) {
        self.used_by.clear();
        self.sequence.clear();
    }

    /// The registers, first counted first.
    pub fn registers(&self) -> &[Scev] {
        &self.sequence
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L475
/// One way to compute a use: `base_gv + base_offset + base_regs + scale *
/// scaled_reg + unfolded_offset`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Formula {
    /// Global base address used for complex addressing.
    pub base_gv: Option<GlobalId>,
    pub base_offset: Immediate,
    /// Whether any complex addressing has a base register.
    pub has_base_reg: bool,
    pub scale: i64,
    /// `BaseRegs.size() > 1` implies a scaled register; a scaled register
    /// implies `scale != 1` or a base register; the register with the
    /// recurrence of the current loop is the scaled one.
    pub base_regs: Vec<Scev>,
    pub scaled_reg: Option<Scev>,
    /// A constant added near the use: it needs a temporary register, but the
    /// offset lives in an add immediate field.
    pub unfolded_offset: Immediate,
}

/// `containsAddRecDependentOnLoop`.
fn contains_add_rec_dependent_on_loop(
    se: &ScalarEvolution,
    s: Scev,
    l: BlockId,
) -> bool {
    se.expr_contains(s, &|expr| se.addrec_header(expr) == Some(l))
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L543
/// `DoInitialMatch`.
fn do_initial_match(
    se: &mut ScalarEvolution,
    cx: &Cx,
    s: Scev,
    l: BlockId,
    good: &mut Vec<Use>,
    bad: &mut Vec<Use>,
) {
    // Collect expressions which properly dominate the loop header.
    if se.properly_dominates(cx, s, l) {
        good.push(Use::of(s));
        return;
    }

    // Look at add operands.
    if matches!(se.node(s).kind, llrm_analysis::scev::expr::Kind::Add(_)) {
        let operands: Vec<Scev> = se.operands(s).iter().map(|one| one.scev).collect();
        for operand in operands {
            do_initial_match(se, cx, operand, l, good, bad);
        }
        return;
    }

    // Look at addrec operands.
    if let Some((start, step, ar_loop)) = se.match_affine_add_rec(s)
        && se.as_constant(start).is_none_or(|value| !value.is_zero())
    {
        do_initial_match(se, cx, start, l, good, bad);
        let zero = se.get_constant_of_type(cx, se.type_of(s), 0);
        // FIXME (LLVM): AR->getNoWrapFlags()
        let stepped = se.get_add_rec_expr(cx, vec![Use::of(zero), Use::of(step)], ar_loop, Flags::default()).scev;
        do_initial_match(se, cx, stepped, l, good, bad);
        return;
    }

    // Handle a multiplication by -1 (negation) if it didn't fold.
    if matches!(se.node(s).kind, llrm_analysis::scev::expr::Kind::Mul(_)) && se.match_all_ones(se.operands(s)[0].scev) {
        let rest: Vec<Use> = se.operands(s)[1..].to_vec();
        let new_mul = se.get_mul_expr(cx, rest, Flags::default(), 0).scev;

        let (mut my_good, mut my_bad) = (Vec::new(), Vec::new());
        do_initial_match(se, cx, new_mul, l, &mut my_good, &mut my_bad);
        let effective = se.get_effective_scev_type(cx, se.type_of(new_mul));
        let neg_one = se.get_constant_of_type(cx, effective, -1);
        for one in my_good {
            good.push(se.get_mul_expr(cx, vec![Use::of(neg_one), one], Flags::default(), 0));
        }
        for one in my_bad {
            bad.push(se.get_mul_expr(cx, vec![Use::of(neg_one), one], Flags::default(), 0));
        }
        return;
    }

    // Ok, we can't do anything interesting. Just stuff the whole thing into a
    // register and hope for the best.
    bad.push(Use::of(s));
}

impl Formula {
    /// `Formula::initialMatch`: incorporate loop-variant parts of `s` into
    /// this formula, keeping all loop-invariant and loop-computable values in
    /// a single base register.
    pub fn initial_match(
        &mut self,
        se: &mut ScalarEvolution,
        cx: &Cx,
        s: Scev,
        l: BlockId,
    ) {
        let (mut good, mut bad) = (Vec::new(), Vec::new());
        do_initial_match(se, cx, s, l, &mut good, &mut bad);
        for sums in [good, bad] {
            if sums.is_empty() {
                continue;
            }
            let sum = se.get_add_expr(cx, sums, Flags::default(), 0).scev;
            if se.as_constant(sum).is_none_or(|value| !value.is_zero()) {
                self.base_regs.push(sum);
            }
            self.has_base_reg = true;
        }
        self.canonicalize(se, l);
    }

    /// `Formula::isCanonical`.
    pub fn is_canonical(
        &self,
        se: &ScalarEvolution,
        l: BlockId,
    ) -> bool {
        assert!(self.scale == 0 || self.scaled_reg.is_some(), "ScaledReg must be non-null if Scale is non-zero");

        let Some(scaled) = self.scaled_reg else { return self.base_regs.len() <= 1 };

        if self.scale != 1 {
            return true;
        }

        if self.base_regs.is_empty() {
            return false;
        }

        if contains_add_rec_dependent_on_loop(se, scaled, l) {
            return true;
        }

        // If ScaledReg is not a recurrent expr, or it is but its loop is not
        // the current loop, meanwhile BaseRegs contains a recurrent expr reg
        // related with the current loop, we want to swap the reg in BaseRegs
        // with ScaledReg.
        !self.base_regs.iter().any(|&reg| contains_add_rec_dependent_on_loop(se, reg, l))
    }

    /// `Formula::canonicalize`: every formula having more than one base
    /// register must use the scaled register, and `1*reg` is `reg`.
    pub fn canonicalize(
        &mut self,
        se: &ScalarEvolution,
        l: BlockId,
    ) {
        if self.is_canonical(se, l) {
            return;
        }

        if self.base_regs.is_empty() {
            // No base reg? Use scale reg with scale = 1 as such.
            let scaled = self.scaled_reg.take().expect("Expected 1*reg => reg");
            assert_eq!(self.scale, 1, "Expected 1*reg => reg");
            self.base_regs.push(scaled);
            self.scale = 0;
            return;
        }

        // Keep the invariant sum in BaseRegs and one of the variant sum in
        // ScaledReg.
        if self.scaled_reg.is_none() {
            self.scaled_reg = self.base_regs.pop();
            self.scale = 1;
        }

        // If ScaledReg is an invariant with respect to L, find the reg from
        // BaseRegs containing the recurrent expr related with Loop L, and swap
        // the reg with ScaledReg.
        let scaled = self.scaled_reg.expect("a scaled register");
        if !contains_add_rec_dependent_on_loop(se, scaled, l)
            && let Some(at) = self.base_regs.iter().position(|&reg| contains_add_rec_dependent_on_loop(se, reg, l))
        {
            self.scaled_reg = Some(self.base_regs[at]);
            self.base_regs[at] = scaled;
        }
        assert!(self.is_canonical(se, l), "Failed to canonicalize?");
    }

    /// `Formula::unscale`: `reg1 + 1*reg2` into `reg1 + reg2`. The formula may
    /// not be canonical after.
    pub fn unscale(&mut self) -> bool {
        if self.scale != 1 {
            return false;
        }
        self.scale = 0;
        self.base_regs.push(self.scaled_reg.take().expect("a scaled register"));
        true
    }

    /// `Formula::hasZeroEnd`.
    pub fn has_zero_end(&self) -> bool {
        if self.unfolded_offset.is_non_zero() || self.base_offset.is_non_zero() {
            return false;
        }
        self.base_regs.len() == 1 && self.scaled_reg.is_none()
    }

    /// `Formula::countsDownToZero`.
    pub fn counts_down_to_zero(
        &self,
        se: &ScalarEvolution,
    ) -> bool {
        if !self.has_zero_end() {
            return false;
        }
        assert_eq!(self.base_regs.len(), 1, "hasZeroEnd should mean one BaseReg");
        let Some((_, step, _)) = se.match_affine_add_rec(self.base_regs[0]) else { return false };
        se.match_apint(step).is_some_and(|value| value.is_negative())
    }

    /// `Formula::getNumRegs`: the register operands, not those implied by
    /// non-constant addrec strides.
    pub fn num_regs(&self) -> usize {
        usize::from(self.scaled_reg.is_some()) + self.base_regs.len()
    }

    /// `Formula::getType`: meaningless except for the bit size.
    pub fn ty(
        &self,
        se: &ScalarEvolution,
        cx: &Cx,
    ) -> Option<Ty> {
        if let Some(&first) = self.base_regs.first() {
            Some(se.type_of(first))
        } else if let Some(scaled) = self.scaled_reg {
            Some(se.type_of(scaled))
        } else {
            self.base_gv.map(|global| Ty::Ptr(cx.globals[global.0 as usize].address_space))
        }
    }

    /// `Formula::deleteBaseReg`: the register at `at` is swapped with the last
    /// and dropped.
    pub fn delete_base_reg(
        &mut self,
        at: usize,
    ) {
        self.base_regs.swap_remove(at);
    }

    /// `Formula::referencesReg`.
    pub fn references_reg(
        &self,
        s: Scev,
    ) -> bool {
        self.scaled_reg == Some(s) || self.base_regs.contains(&s)
    }

    /// `Formula::hasRegsUsedByUsesOtherThan`.
    pub fn has_regs_used_by_uses_other_than(
        &self,
        lu_idx: usize,
        reg_uses: &RegUseTracker,
    ) -> bool {
        if let Some(scaled) = self.scaled_reg
            && reg_uses.is_reg_used_by_uses_other_than(scaled, lu_idx)
        {
            return true;
        }
        self.base_regs.iter().any(|&reg| reg_uses.is_reg_used_by_uses_other_than(reg, lu_idx))
    }
}
