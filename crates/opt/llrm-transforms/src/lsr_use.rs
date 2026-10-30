//! The uses and the price of LLVM's loop strength reduction: `LSRFixup`,
//! `LSRUse`, `Cost` and the addressing-mode checks they rate by
//! (LoopStrengthReduce.cpp at `d1106deb71cc`, l.1070-2130).
//!
//! # How it works
//! An `LsrUse` is one thing the loop computes (an address, a compare against
//! zero, a plain value) with the `Formula`s that could compute it. `Cost` rates
//! a formula against the registers already chosen for the other uses: registers
//! first, then add recurrences, IV multiplies, base adds, scale, immediates and
//! the setup in the preheader. The target answers what an address mode folds
//! (`Tti`).
//!
//! # Example
//! `p[i] = 0` in a loop is an address use with one fixup at offset 0; its
//! formula `{%p,+,4}` costs one register and one recurrence (1 instruction).
//!
//! # Runs
//! Nothing runs it yet: stage 2 of the translation of LSR (the old pass in
//! [`crate::lsr_pass`] is the pass that runs them).
//!
//! # Limits
//! DEVIATE LU1: `Cost::rate_formula` adds the target's `scaled_index_insns`
//! (m16: the `lea` that forms the scaled register, and the prefixed form's
//! own cost) to `Insns` once for each scaled register (an `Address` formula
//! with scale above 1), beyond the pin. The pin has no real-mode model: its x86
//! prices a scaled address as part of the address form, and `ScaleCost` is the
//! last key of the comparison, so a scaled index always beat a pointer
//! induction variable (m16 listed `lea bx,[ecx+ecx]` for one array) or lost to
//! six of them (particle's six far arrays, which the narrowing then left only
//! the pointer forms of). 0 elsewhere, so the pin's rule. Owner
//! laa-runtime-checks-loadelim; exit: when the target description prices the
//! prefixed `lea` itself or the pin gains a real-mode model; reviewed
//! 2026-11-30.
//!
//! Known row of LU1: QCport `d_faces.c` -Os, `_clip_w` (six near float arrays
//! on one index, an x87 loop) is 6262 B against main's 6208. The model rates
//! the scaled solution at 10 instructions (9 registers, 2 recurrences) and the
//! pointer recurrences at 13 (9 registers, 9 recurrences), so it keeps the
//! scaled index, whose bases then reload from the frame; particle's `ADVANCE`
//! (six far arrays) wins by 3 more instructions at the same charge. Neither a
//! segment register class (the bases are near), an address-register budget
//! (both forms need the same) nor sparing each use's cheapest formula in the
//! narrowing (tried: the pointer form was kept and still lost) separates the
//! two; only a tuned charge (7 to 8) does, which is not derived. Owner
//! laa-runtime-checks-loadelim; exit: a fill cost for near pointer bases past
//! the four addressing registers; reviewed 2026-11-30.
//!
//! Its -O2 cost is measured, not counted statically (static counts said
//! smaller): QCport's 61-frame BENCH run in the dosrun emulator, every object
//! main's but `d_faces.obj` (the DFACES-DYN harness: build.py, run.py).
//! Executed instructions: main 485,835,518; parity +2,383,382 (+0.49%); head
//! +2,816,018 (+0.58%), so this charge adds +432,636 (+0.09%). The parity loss
//! is row DFACES-DYN-1, not LU1's.
//!
//! DEVIATE: the target hooks have no access type or address space on the
//! legality call, and no per-instruction query (`LSRWithInstrQueries`); x86 has
//! neither. `getPreferredAddressingMode` is none on x86, so the pre- and
//! post-indexed branches of `RateRegister` never apply and are not ported. A
//! scalable `Immediate` does not exist (see [`crate::lsr_formula`]).

use llrm_analysis::scev::expr::{Scev, Type};
use llrm_analysis::scev::{Cx, ScalarEvolution};
use llrm_mir::module::{BlockId, InstId, Operand};
use llrm_mir::opcode::Opcode;
use llrm_mir::tti::{Cost as TtiCost, LsrCost, Tti};
use llrm_mir::valuetracking::pattern::{phi_incoming, result_of};
use llrm_support::hash::{HashSet, IndexSet};

use crate::lsr_formula::{Formula, Immediate, MemAccessTy, RegUseTracker};

/// `SetupCostDepthLimit`.
const SETUP_COST_DEPTH_LIMIT: u32 = 7;

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1324
/// `LSRUse::KindType`: what kinds of scaled and immediate operands a use takes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Kind {
    /// A normal use, with no folding.
    Basic,
    /// A basic use allowing -1 scales.
    Special,
    /// An address use: folding as the target allows.
    Address,
    /// An equality icmp with both operands folded into one.
    ICmpZero,
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1293
/// An operand of an instruction that is to be replaced with some equivalent,
/// possibly strength-reduced, value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixup {
    /// The instruction that is updated.
    pub user: InstId,
    /// The operand of `user` that is replaced; every instance is.
    pub operand: Operand,
    /// The loops (by header) whose induction variable's post-incremented value
    /// the user takes; empty for the pre-incremented one.
    pub post_inc_loops: Vec<BlockId>,
    /// A constant added to the use's expression, so fixups with different
    /// offsets (an unrolled loop) share one use.
    pub offset: Immediate,
}

impl Fixup {
    pub fn new(
        user: InstId,
        operand: Operand,
    ) -> Self {
        Self { user, operand, post_inc_loops: Vec::new(), offset: Immediate::ZERO }
    }

    /// `LSRFixup::isUseFullyOutsideLoop`: whether the fixup always uses its
    /// value outside the loop of header `l`.
    pub fn is_use_fully_outside_loop(
        &self,
        cx: &Cx,
        l: BlockId,
    ) -> bool {
        let Some(lp) = cx.loops.get_loop_for(l) else { return true };
        let user = cx.function.instruction(self.user);
        // A phi uses its value in its incoming blocks.
        if user.opcode == Opcode::Phi {
            return !phi_incoming(user).any(|(value, from)| value == self.operand && lp.contains(from));
        }
        !lp.contains_instruction(cx.function, self.user)
    }
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1324
/// What LSR keeps for each use: its kind and fixups, and the formulae that
/// could satisfy it.
#[derive(Clone, Debug)]
pub struct Use {
    uniquifier: HashSet<Vec<Scev>>,
    pub kind: Kind,
    pub access_ty: MemAccessTy,
    pub fixups: Vec<Fixup>,
    /// The least and greatest offsets of the fixups.
    pub min_offset: Immediate,
    pub max_offset: Immediate,
    /// Whether all fixups are outside the loop.
    pub all_fixups_outside_loop: bool,
    /// Whether all fixups run on every path to the latch.
    pub all_fixups_unconditional: bool,
    /// The use has the one formula that first matched: some SCEVs cannot be
    /// expanded again, and their registers are still counted.
    pub rigid_formula: bool,
    pub formulae: Vec<Formula>,
    /// The registers of all the formulae, first inserted first.
    pub regs: IndexSet<Scev>,
}

impl Use {
    pub fn new(
        kind: Kind,
        access_ty: MemAccessTy,
    ) -> Self {
        Self {
            uniquifier: HashSet::default(),
            kind,
            access_ty,
            fixups: Vec::new(),
            min_offset: Immediate::FIXED_MAX,
            max_offset: Immediate::FIXED_MIN,
            all_fixups_outside_loop: true,
            all_fixups_unconditional: true,
            rigid_formula: false,
            formulae: Vec::new(),
            regs: IndexSet::default(),
        }
    }

    /// `pushFixup`: the fixup, with the offsets widened to it.
    pub fn push_fixup(
        &mut self,
        fixup: Fixup,
    ) {
        if fixup.offset > self.max_offset {
            self.max_offset = fixup.offset;
        }
        if fixup.offset < self.min_offset {
            self.min_offset = fixup.offset;
        }
        self.fixups.push(fixup);
    }

    /// The registers of `f`, sorted: what makes two formulae the same.
    fn key(f: &Formula) -> Vec<Scev> {
        let mut key = f.base_regs.clone();
        key.extend(f.scaled_reg);
        key.sort_unstable();
        key
    }

    /// `HasFormulaWithSameRegs`.
    pub fn has_formula_with_same_regs(
        &self,
        f: &Formula,
    ) -> bool {
        self.uniquifier.contains(&Self::key(f))
    }

    /// `getNotSelectedProbability`: the share of formulae without `reg`.
    pub fn not_selected_probability(
        &self,
        reg: Scev,
    ) -> f32 {
        let with = self.formulae.iter().filter(|f| f.references_reg(reg)).count();
        (self.formulae.len() - with) as f32 / self.formulae.len() as f32
    }

    /// `InsertFormula`: adds `f` unless it is there already (or the use is
    /// rigid and has one); whether it did. `f` must be canonical.
    pub fn insert_formula(
        &mut self,
        se: &ScalarEvolution,
        f: &Formula,
        l: BlockId,
    ) -> bool {
        assert!(f.is_canonical(se, l), "Invalid canonical representation");
        if !self.formulae.is_empty() && self.rigid_formula {
            return false;
        }
        if !self.uniquifier.insert(Self::key(f)) {
            return false;
        }
        // A register holding zero is not profitable.
        debug_assert!(f.scaled_reg.is_none_or(|reg| !se.match_zero(reg)), "Zero allocated in a scaled register!");
        debug_assert!(f.base_regs.iter().all(|&reg| !se.match_zero(reg)), "Zero allocated in a base register!");
        self.formulae.push(f.clone());
        self.regs.extend(f.base_regs.iter().copied());
        self.regs.extend(f.scaled_reg);
        true
    }

    /// `DeleteFormula`: the last formula takes the place of the one at `at`.
    pub fn delete_formula(
        &mut self,
        at: usize,
    ) {
        self.formulae.swap_remove(at);
    }

    /// `RecomputeRegs`: the registers of the formulae that are left, and the
    /// tracker told of those no longer used by use `lu_idx`.
    pub fn recompute_regs(
        &mut self,
        lu_idx: usize,
        reg_uses: &mut RegUseTracker,
    ) {
        let old = std::mem::take(&mut self.regs);
        for f in &self.formulae {
            self.regs.extend(f.scaled_reg);
            self.regs.extend(f.base_regs.iter().copied());
        }
        for reg in old {
            if !self.regs.contains(&reg) {
                reg_uses.drop_register(reg, lu_idx);
            }
        }
    }
}

// ---- addressing modes ----

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1851
/// `isAMCompletelyFolded`: whether the target folds `base_gv + base_offset +
/// base_reg + scale * index` into a use of `kind`.
pub fn is_am_completely_folded_at(
    tti: &dyn Tti,
    kind: Kind,
    access: MemAccessTy,
    base_gv: bool,
    base_offset: Immediate,
    has_base_reg: bool,
    scale: i64,
) -> bool {
    match kind {
        Kind::Address => tti.is_legal_addressing_mode_at(access.addr_space, access.bytes, base_gv, base_offset.0, has_base_reg, scale),
        Kind::ICmpZero => {
            // There is no hook for folding a global into a compare.
            if base_gv {
                return false;
            }
            // A compare has two operands: no more than two non-trivial parts.
            if scale != 0 && has_base_reg && base_offset.is_non_zero() {
                return false;
            }
            // Only no scale or -1: the scaled register goes to the other side.
            if scale != 0 && scale != -1 {
                return false;
            }
            if base_offset.is_non_zero() {
                // `BaseReg + Off` is `cmp BaseReg, -Off`; `-1*ScaleReg + Off`
                // is `cmp ScaleReg, Off`.
                let immediate = if scale == 0 { base_offset.0.wrapping_neg() } else { base_offset.0 };
                return tti.is_legal_icmp_immediate(immediate);
            }
            // `BaseReg + -1*ScaleReg` is `cmp BaseReg, ScaleReg`.
            true
        }
        Kind::Basic => !base_gv && scale == 0 && base_offset.is_zero(),
        Kind::Special => !base_gv && (scale == 0 || scale == -1) && base_offset.is_zero(),
    }
}

/// The same for every offset from `min_offset` to `max_offset` added to
/// `base_offset`.
#[allow(clippy::too_many_arguments)]
pub fn is_am_completely_folded_between(
    tti: &dyn Tti,
    min_offset: Immediate,
    max_offset: Immediate,
    kind: Kind,
    access: MemAccessTy,
    base_gv: bool,
    base_offset: Immediate,
    has_base_reg: bool,
    scale: i64,
) -> bool {
    // Check for overflow.
    let (base, min, max) = (base_offset.0, min_offset.0, max_offset.0);
    let low = base.wrapping_add(min);
    if (low > base) != (min > 0) {
        return false;
    }
    let high = base.wrapping_add(max);
    if (high > base) != (max > 0) {
        return false;
    }
    is_am_completely_folded_at(tti, kind, access, base_gv, Immediate(low), has_base_reg, scale)
        && is_am_completely_folded_at(tti, kind, access, base_gv, Immediate(high), has_base_reg, scale)
}

/// `isAMCompletelyFolded(TTI, MinOffset, MaxOffset, Kind, AccessTy, F, L)`.
pub fn is_am_completely_folded_formula(
    tti: &dyn Tti,
    se: &ScalarEvolution,
    min_offset: Immediate,
    max_offset: Immediate,
    kind: Kind,
    access: MemAccessTy,
    f: &Formula,
    l: BlockId,
) -> bool {
    // Either a canonical formula or a scale is right; a non-canonical one with
    // no scale is not.
    assert!(f.is_canonical(se, l) || f.scale != 0);
    is_am_completely_folded_between(
        tti,
        min_offset,
        max_offset,
        kind,
        access,
        f.base_gv.is_some(),
        f.base_offset,
        f.has_base_reg,
        f.scale,
    )
}

/// `isAMCompletelyFolded(TTI, LU, F)`: whether `lu` accommodates what `f`
/// defines, up to one base, one scaled register and an offset. A formula with
/// more base registers or an unfolded offset still costs more than this says.
pub fn is_am_completely_folded(
    tti: &dyn Tti,
    lu: &Use,
    f: &Formula,
) -> bool {
    is_am_completely_folded_between(
        tti,
        lu.min_offset,
        lu.max_offset,
        lu.kind,
        lu.access_ty,
        f.base_gv.is_some(),
        f.base_offset,
        f.has_base_reg,
        f.scale,
    )
}

/// `isLegalUse`: whether the formula can be expanded: it folds, or its
/// registers sum into one base register.
#[allow(clippy::too_many_arguments)]
pub fn is_legal_use(
    tti: &dyn Tti,
    min_offset: Immediate,
    max_offset: Immediate,
    kind: Kind,
    access: MemAccessTy,
    base_gv: bool,
    base_offset: Immediate,
    has_base_reg: bool,
    scale: i64,
) -> bool {
    is_am_completely_folded_between(
        tti,
        min_offset,
        max_offset,
        kind,
        access,
        base_gv,
        base_offset,
        has_base_reg,
        scale,
    ) || (scale == 1
        && is_am_completely_folded_between(
            tti,
            min_offset,
            max_offset,
            kind,
            access,
            base_gv,
            base_offset,
            true,
            0,
        ))
}

/// `isLegalUse(TTI, MinOffset, MaxOffset, Kind, AccessTy, F)`.
pub fn is_legal_use_formula(
    tti: &dyn Tti,
    min_offset: Immediate,
    max_offset: Immediate,
    kind: Kind,
    access: MemAccessTy,
    f: &Formula,
) -> bool {
    is_legal_use(
        tti,
        min_offset,
        max_offset,
        kind,
        access,
        f.base_gv.is_some(),
        f.base_offset,
        f.has_base_reg,
        f.scale,
    )
}

/// `isLegalAddImmediate`.
pub fn is_legal_add_immediate(
    tti: &dyn Tti,
    offset: Immediate,
) -> bool {
    tti.is_legal_add_immediate(offset.0)
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1979
/// `getScalingFactorCost`: what the scale of `f` costs `lu`.
pub fn scaling_factor_cost(
    tti: &dyn Tti,
    se: &ScalarEvolution,
    lu: &Use,
    f: &Formula,
    l: BlockId,
) -> TtiCost {
    if f.scale == 0 {
        return TtiCost::FREE;
    }
    // Not completely folded: an extra cost only for a scale other than 1.
    if !is_am_completely_folded_formula(tti, se, lu.min_offset, lu.max_offset, lu.kind, lu.access_ty, f, l) {
        return TtiCost::of(i64::from(f.scale != 1));
    }
    match lu.kind {
        Kind::Address => {
            // The price at both ends of the offsets.
            let at = |offset: Immediate| {
                tti.scaling_factor_cost_at(
                    lu.access_ty.bytes,
                    f.base_gv.is_some(),
                    f.base_offset.add_unsigned(offset).0,
                    f.has_base_reg,
                    f.scale,
                )
            };
            let (min, max) = (at(lu.min_offset), at(lu.max_offset));
            assert!(min.valid && max.valid, "Legal addressing mode has an illegal cost!");
            TtiCost::of(min.value.max(max.value))
        }
        // Completely folded into the instruction.
        Kind::ICmpZero | Kind::Basic | Kind::Special => TtiCost::FREE,
    }
}

// ---- cost ----

/// What `Cost` rates against: the analyses, the target and the loop.
pub struct Rating<'a, 'b> {
    pub se: &'a mut ScalarEvolution,
    pub cx: &'a Cx<'b>,
    pub tti: &'a dyn Tti,
    /// The header of the loop.
    pub l: BlockId,
    /// `TTI.isHardwareLoopProfitable`: a counted-down loop's recurrence is
    /// folded into the loop instruction.
    pub hardware_loop_profitable: bool,
    /// The formulae rated are the loop as it stands: its values exist, and
    /// nothing is expanded (DEVIATE LU5 charges only an expansion).
    pub as_written: bool,
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1232
/// Measures and compares candidate formulae.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Cost {
    pub c: LsrCost,
    /// The registers rated scaled so far, with their scale: the target's
    /// `scaled_index_insns` is paid once for each (DEVIATE LU1).
    scaled: Vec<(Scev, i64)>,
}

impl Cost {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Lose`: every metric at its greatest.
    pub fn lose(&mut self) {
        self.c = LsrCost {
            insns: u32::MAX,
            num_regs: u32::MAX,
            add_rec_cost: u32::MAX,
            num_iv_muls: u32::MAX,
            num_base_adds: u32::MAX,
            imm_cost: u32::MAX,
            setup_cost: u32::MAX,
            scale_cost: u32::MAX,
        };
    }

    pub fn is_loser(&self) -> bool {
        self.c.num_regs == u32::MAX
    }

    /// `isLess`: the target's order. The pin ranks by instructions first only
    /// when `-lsr-insns-cost` is on the command line
    /// (`getNumOccurrences() > 0`), which nothing here passes; x86 puts them
    /// first in its own `isLSRCostLess`, AArch64 second.
    pub fn is_less(
        &self,
        tti: &dyn Tti,
        other: &Cost,
    ) -> bool {
        tti.is_lsr_cost_less(&self.c, &other.c)
    }

    /// `RateFormula`: the cost of `f` for `lu`, `regs` the registers already
    /// chosen. A formula that names a register of `visited` or of `losers` is
    /// a loser.
    pub fn rate_formula(
        &mut self,
        r: &mut Rating,
        f: &Formula,
        regs: &mut HashSet<Scev>,
        visited: &HashSet<Scev>,
        lu: &Use,
        mut losers: Option<&mut HashSet<Scev>>,
    ) {
        if self.is_loser() {
            return;
        }
        assert!(f.is_canonical(r.se, r.l), "Cost is accurate only for canonical formula");
        let (prev_add_rec, prev_regs, prev_base_adds) = (self.c.add_rec_cost, self.c.num_regs, self.c.num_base_adds);
        for reg in f.scaled_reg.into_iter().chain(f.base_regs.iter().copied()) {
            if visited.contains(&reg) {
                self.lose();
                return;
            }
            self.rate_primary_register(r, f, reg, regs, lu, losers.as_deref_mut());
            if self.is_loser() {
                return;
            }
        }

        // The unfolded adds inside the loop.
        let parts = f.num_regs();
        if parts > 1 {
            // Not the base and a possible second register, if the target folds
            // two.
            let folded = f.scale != 0 && is_am_completely_folded(r.tti, lu, f);
            self.c.num_base_adds += (parts - (1 + usize::from(folded))) as u32;
        }
        // DEVIATE LU2: the offset beside two registers is the displacement of
        // the `lea` that adds the registers, when the form is a legal address
        // for the target; only a Basic or ICmpZero use pays an add for it.
        let absorbed = f.unfolded_offset.is_non_zero()
            && lu.kind != Kind::Address
            && parts >= 2
            && r.tti.address_form_adds_offsets()
            && r.tti.is_legal_addressing_mode(f.base_gv.is_some(), f.unfolded_offset.0, true, f.scale);
        if !absorbed {
            self.c.num_base_adds += u32::from(f.unfolded_offset.is_non_zero());
        }

        // Scaling amounts that are not free.
        self.c.scale_cost += scaling_factor_cost(r.tti, r.se, lu, f, r.l).value as u32;
        // DEVIATE LU1: a scaled index is an instruction the backend forms once
        // and every use of the register shares.
        let scaled_insns = r.tti.scaled_index_insns();
        if scaled_insns > 0 && lu.kind == Kind::Address && f.scale > 1 {
            if let Some(reg) = f.scaled_reg {
                if !self.scaled.contains(&(reg, f.scale)) {
                    self.scaled.push((reg, f.scale));
                    self.c.insns += scaled_insns;
                }
            }
        }

        // The non-zero immediates.
        for fixup in &lu.fixups {
            let offset = fixup.offset.add_unsigned(f.base_offset);
            if f.base_gv.is_some() {
                // Symbolic values are rated conservatively.
                self.c.imm_cost += 64;
            } else if offset.is_non_zero() {
                self.c.imm_cost += 65 - (offset.0 ^ (offset.0 >> 63)).leading_zeros();
            }
            // The target may not support this offset for this instruction.
            if lu.kind == Kind::Address
                && offset.is_non_zero()
                && !is_am_completely_folded_at(
                    r.tti,
                    Kind::Address,
                    lu.access_ty,
                    f.base_gv.is_some(),
                    offset,
                    f.has_base_reg,
                    f.scale,
                )
            {
                self.c.num_base_adds += 1;
            }
        }

        // Every register past the target's, less one, is another instruction
        // (a fill).
        let target_regs = r.tti.number_of_gpr_registers() - 1;
        if self.c.num_regs > target_regs {
            // Already past: only the newly added registers add instructions.
            if prev_regs > target_regs {
                self.c.insns += self.c.num_regs - prev_regs;
            } else {
                self.c.insns += self.c.num_regs - target_regs;
            }
        }

        // An ICmpZero formula that does not end at 0 cannot be a plain add or
        // sub: the final value is compared, one more instruction unless the
        // target fuses the compare with its branch.
        if lu.kind == Kind::ICmpZero && !f.has_zero_end() && !r.tti.can_macro_fuse_cmp() {
            self.c.insns += 1;
        }
        // Each new addrec adds one instruction.
        self.c.insns += self.c.add_rec_cost - prev_add_rec;
        // The base adds are instructions for unfolded registers.
        if lu.kind != Kind::ICmpZero {
            self.c.insns += self.c.num_base_adds - prev_base_adds;
        }
    }

    /// `RatePrimaryRegister`: rates `reg` once; a loser register is recorded.
    fn rate_primary_register(
        &mut self,
        r: &mut Rating,
        f: &Formula,
        reg: Scev,
        regs: &mut HashSet<Scev>,
        lu: &Use,
        losers: Option<&mut HashSet<Scev>>,
    ) {
        if losers.as_ref().is_some_and(|losers| losers.contains(&reg)) {
            self.lose();
            return;
        }
        if regs.insert(reg) {
            self.rate_register(r, f, reg, regs, lu);
            if let Some(losers) = losers
                && self.is_loser()
            {
                losers.insert(reg);
            }
        }
    }

    /// `RateRegister`: tallies what `reg` costs.
    fn rate_register(
        &mut self,
        r: &mut Rating,
        f: &Formula,
        reg: Scev,
        regs: &mut HashSet<Scev>,
        lu: &Use,
    ) {
        if let Some(header) = r.se.addrec_header(reg) {
            // An addrec of another loop is invariant in `l` (LSR handles
            // innermost loops only).
            if header != r.l {
                // One that exists as a phi is free.
                if is_existing_phi(r.se, r.cx, reg) {
                    return;
                }
                // A sibling's induction variable is not for this loop to add.
                let contains = cx_loop_contains(r.cx, header, r.l);
                if !contains {
                    self.lose();
                    return;
                }
                self.c.num_regs += 1;
                return;
            }

            let mut loop_cost = 1;
            // A loop that counts down to zero under a hardware loop folds its
            // addrec into the loop instruction.
            if lu.kind == Kind::ICmpZero && f.counts_down_to_zero(r.se) && r.hardware_loop_profitable {
                loop_cost = 0;
            }
            self.c.add_rec_cost += loop_cost;

            // DEVIATE LU5: a start that is a non-affine addrec of an enclosing
            // loop is expanded as an induction variable for each of its steps,
            // live through that loop.
            if r.tti.lsr_charges_outer_nonaffine_ivs() && !r.as_written {
                let start = r.se.operands(reg)[0].scev;
                if let Some(outer) = r.se.addrec_header(start).filter(|&h| h != r.l && cx_loop_contains(r.cx, h, r.l)) {
                    let _ = outer;
                    if !r.se.is_affine(start) {
                        self.c.num_regs += (r.se.operands(start).len() as u32).saturating_sub(1);
                    }
                }
            }

            // The step, if it needs a register of its own.
            let step = r.se.operands(reg)[1].scev;
            if !r.se.is_affine(reg) || r.se.as_constant(step).is_none() {
                if !regs.contains(&step) {
                    self.rate_register(r, f, step, regs, lu);
                    if self.is_loser() {
                        return;
                    }
                }
            }
        }
        self.c.num_regs += 1;

        // Favor registers that need no setup in the preheader.
        self.c.setup_cost += setup_cost(r.se, r.cx, r.tti, reg, SETUP_COST_DEPTH_LIMIT);
        // Even with the depth limit, the cost stays valid.
        self.c.setup_cost = self.c.setup_cost.min(1 << 16);

        if r.se.scev_type(reg) == Type::Mul && r.se.has_computable_loop_evolution(r.cx, reg, Some(r.l)) {
            self.c.num_iv_muls += 1;
        }
    }
}

/// Whether the loop of header `outer` contains the loop of header `inner`.
fn cx_loop_contains(
    cx: &Cx,
    outer: BlockId,
    inner: BlockId,
) -> bool {
    cx.loops.get_loop_for(outer).is_some_and(|lp| lp.contains_loop(cx.loops.get_loop_for(inner)))
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1123
/// `isExistingPhi`: a phi of the addrec's loop header is the addrec.
pub fn is_existing_phi(
    se: &mut ScalarEvolution,
    cx: &Cx,
    ar: Scev,
) -> bool {
    let Some(header) = se.addrec_header(ar) else { return false };
    let ty = se.get_effective_scev_type(cx, se.type_of(ar));
    for inst in cx.function.iter_block(header) {
        let phi = cx.function.instruction(inst);
        if phi.opcode != Opcode::Phi {
            break;
        }
        let value = result_of(phi);
        if se.is_value_scevable(cx, value) && se.get_scev(cx, value) == ar {
            let own = cx.function.operand_type(cx.context, value).and_then(|t| se.ty_of(cx, t));
            if own.is_some_and(|own| se.get_effective_scev_type(cx, own) == ty) {
                return true;
            }
        }
    }
    false
}

/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopStrengthReduce.cpp#L1397
/// `getSetupCost`: the instructions `reg` needs in the preheader, to a depth.
pub fn setup_cost(
    se: &ScalarEvolution,
    cx: &Cx,
    tti: &dyn Tti,
    reg: Scev,
    depth: u32,
) -> u32 {
    match se.scev_type(reg) {
        Type::Unknown => 1,
        Type::Constant => {
            let value = se.as_constant(reg).expect("a constant");
            let bits = se.get_type_size_in_bits(cx, se.type_of(reg));
            u32::from(tti.int_imm_cost(value.sext_value(), bits) != TtiCost::FREE)
        }
        _ if depth == 0 => 0,
        Type::AddRec => setup_cost(se, cx, tti, se.operands(reg)[0].scev, depth - 1),
        Type::Truncate | Type::ZeroExtend | Type::SignExtend | Type::PtrToAddr => {
            setup_cost(se, cx, tti, se.operands(reg)[0].scev, depth - 1)
        }
        Type::Add | Type::Mul | Type::UMax | Type::SMax | Type::UMin | Type::SMin | Type::SequentialUMin => {
            se.operands(reg).iter().map(|op| setup_cost(se, cx, tti, op.scev, depth - 1)).sum()
        }
        Type::UDiv => {
            let (lhs, rhs) = se.udiv_operands(reg);
            setup_cost(se, cx, tti, lhs.scev, depth - 1) + setup_cost(se, cx, tti, rhs.scev, depth - 1)
        }
        _ => 0,
    }
}
