//! `ValueLatticeElement`: what LazyValueInfo knows of a value: nothing yet,
//! undef, one constant, anything but one constant, a range of integers (with
//! or without undef), or anything.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `ValueLattice.h` and
//! `ValueLattice.cpp`, one function per C++ function in file order, each
//! linking its lines. An integer constant is never a `constant`: it is a
//! one-element range, as in LLVM, so [`LatticeConstant`] splits a MIR constant
//! into `Undef` (undef and poison, which LLVM's `UndefValue` covers), `Int` and
//! `Other`.
//!
//! DEVIATE (anything MIR lacks):
//! 1. `ConstantFoldCompareInstOperands` is not translated; `get_compare` takes
//!    the fold as a closure that answers `Some(true/false)` or `None` (the C++
//!    result is a `Constant`, which for two non-undef non-poison operands is
//!    `true`, `false` or unfolded).
//! 2. No vectors: the `getConstant()->getType()->isVectorTy()` arm of `mergeIn`
//!    (lines 444-455) and the vector arms of `toConstantRange` are absent.
//! 3. No FP lattice states: this LLVM's lattice has none (a `constant` may be
//!    an FP constant, which MIR keeps as `Other`).
//! 4. The raw_ostream printer takes the context and the module's globals to
//!    spell a constant.
//!
//! # Example
//! `[0,5)` merged with `[10,20)` is `[0,20)`; merged with overdefined it is
//! overdefined.
//!
//! # See also
//! - Used by: [`crate::lazyvalueinfo`]

use crate::constantrange::{ConstantRange, PreferredRangeType};
use crate::context::{Constant, ConstantId, ConstantKind, Context, mask, signed};
use crate::module::GlobalValue;
use crate::opcode::IntPredicate;
use crate::types::TypeId;

/// A `Constant *` of the lattice, by value: the structural equality of two is
/// LLVM's pointer equality of uniqued constants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LatticeConstant {
    /// `UndefValue` (poison is one).
    Undef,
    /// A `ConstantInt`.
    Int { width: u32, bits: u128 },
    /// Any other constant: a null pointer, a global, a float, an aggregate.
    Other { ty: TypeId, kind: ConstantKind },
}

impl LatticeConstant {
    /// The constant `id` names.
    pub fn of(
        context: &Context,
        id: ConstantId,
    ) -> Self {
        let constant = context.get(id);
        if matches!(constant.kind, ConstantKind::Poison | ConstantKind::Undef) {
            return Self::Undef;
        }
        if let Some(width) = context.types.int_bits(constant.ty) {
            match constant.kind {
                ConstantKind::Int(bits) => return Self::Int { width, bits: bits & mask(width) },
                ConstantKind::Null | ConstantKind::Zero => return Self::Int { width, bits: 0 },
                _ => {}
            }
        }
        Self::Other { ty: constant.ty, kind: constant.kind.clone() }
    }

    /// `Constant::getNullValue(Ty)` of an integer or pointer type.
    pub fn null_value(
        context: &Context,
        ty: TypeId,
    ) -> Self {
        match context.types.int_bits(ty) {
            Some(width) => Self::Int { width, bits: 0 },
            None => Self::Other { ty, kind: ConstantKind::Null },
        }
    }

    /// `Constant::getAllOnesValue(Ty)` of an integer type.
    pub fn all_ones_value(
        context: &Context,
        ty: TypeId,
    ) -> Self {
        let width = context.types.int_bits(ty).expect("an integer type");
        Self::Int { width, bits: mask(width) }
    }

    /// `Constant::isNullValue`.
    pub fn is_null_value(&self) -> bool {
        match self {
            Self::Undef => false,
            Self::Int { bits, .. } => *bits == 0,
            Self::Other { kind, .. } => matches!(kind, ConstantKind::Null | ConstantKind::Zero),
        }
    }

    /// `Constant::toConstantRange` of a scalar: an integer is a point, anything
    /// else is the full set at `width`.
    fn to_constant_range(
        &self,
        width: u32,
    ) -> ConstantRange {
        match self {
            Self::Int { width, bits } => ConstantRange::single(*width, *bits),
            _ => ConstantRange::full(width),
        }
    }
}

/// `ValueLatticeElement::ValueLatticeElementTy`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Tag {
    Unknown,
    Undef,
    Constant,
    NotConstant,
    ConstantRange,
    ConstantRangeIncludingUndef,
    Overdefined,
}

/// `ValueLatticeElement::MergeOptions`.
#[derive(Clone, Copy, Debug)]
pub struct MergeOptions {
    pub may_include_undef: bool,
    pub check_widen: bool,
    pub max_widen_steps: u32,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self::new(false, false, 1)
    }
}

impl MergeOptions {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L121>
    pub fn new(
        may_include_undef: bool,
        check_widen: bool,
        max_widen_steps: u32,
    ) -> Self {
        Self { may_include_undef, check_widen, max_widen_steps }
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L131>
    pub fn set_may_include_undef(
        mut self,
        v: bool,
    ) -> Self {
        self.may_include_undef = v;
        self
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L136>
    pub fn set_check_widen(
        mut self,
        v: bool,
    ) -> Self {
        self.check_widen = v;
        self
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L141>
    pub fn set_max_widen_steps(
        mut self,
        steps: u32,
    ) -> Self {
        self.check_widen = true;
        self.max_widen_steps = steps;
        self
    }
}

/// `llvm::ValueLatticeElement`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValueLatticeElement {
    tag: Tag,
    num_range_extensions: u32,
    may_have_different_provenance: bool,
    const_val: Option<LatticeConstant>,
    range: Option<ConstantRange>,
}

impl Default for ValueLatticeElement {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L151>
    fn default() -> Self {
        Self {
            tag: Tag::Unknown,
            num_range_extensions: 0,
            may_have_different_provenance: false,
            const_val: None,
            range: None,
        }
    }
}

impl ValueLatticeElement {
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L208>
    pub fn get(c: &LatticeConstant) -> Self {
        let mut res = Self::default();
        res.mark_constant(c, false);
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L213>
    pub fn get_not(c: &LatticeConstant) -> Self {
        let mut res = Self::default();
        assert!(*c != LatticeConstant::Undef, "!= undef is not supported");
        res.mark_not_constant(c);
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L219>
    pub fn get_range(
        cr: ConstantRange,
        may_include_undef: bool,
    ) -> Self {
        if cr.is_full_set() {
            return Self::get_overdefined();
        }

        if cr.is_empty_set() {
            let mut res = Self::default();
            if may_include_undef {
                res.mark_undef();
            }
            return res;
        }

        let mut res = Self::default();
        res.mark_constant_range(cr, MergeOptions::default().set_may_include_undef(may_include_undef));
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L236>
    pub fn get_overdefined() -> Self {
        let mut res = Self::default();
        res.mark_overdefined();
        res
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L242>
    pub fn is_undef(&self) -> bool {
        self.tag == Tag::Undef
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L243>
    pub fn is_unknown(&self) -> bool {
        self.tag == Tag::Unknown
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L244>
    pub fn is_unknown_or_undef(&self) -> bool {
        self.tag == Tag::Unknown || self.tag == Tag::Undef
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L245>
    pub fn is_constant(&self) -> bool {
        self.tag == Tag::Constant
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L246>
    pub fn is_not_constant(&self) -> bool {
        self.tag == Tag::NotConstant
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L247>
    pub fn is_constant_range_including_undef(&self) -> bool {
        self.tag == Tag::ConstantRangeIncludingUndef
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L254>
    pub fn is_constant_range(
        &self,
        undef_allowed: bool,
    ) -> bool {
        self.tag == Tag::ConstantRange
            || (self.tag == Tag::ConstantRangeIncludingUndef
                && (undef_allowed || self.range.as_ref().is_some_and(ConstantRange::is_single_element)))
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L258>
    pub fn is_overdefined(&self) -> bool {
        self.tag == Tag::Overdefined
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L260>
    pub fn get_constant(&self) -> &LatticeConstant {
        assert!(self.is_constant(), "Cannot get the constant of a non-constant!");
        self.const_val.as_ref().expect("a constant")
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L265>
    pub fn get_not_constant(&self) -> &LatticeConstant {
        assert!(self.is_not_constant(), "Cannot get the constant of a non-notconstant!");
        self.const_val.as_ref().expect("a constant")
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L274>
    pub fn get_constant_range(
        &self,
        undef_allowed: bool,
    ) -> &ConstantRange {
        assert!(self.is_constant_range(undef_allowed), "Cannot get the constant-range of a non-constant-range!");
        self.range.as_ref().expect("a range")
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L280>
    pub fn as_constant_integer(&self) -> Option<u128> {
        if self.is_constant()
            && let LatticeConstant::Int { bits, .. } = self.get_constant()
        {
            return Some(*bits);
        } else if self.is_constant_range(true) && self.get_constant_range(true).is_single_element() {
            return self.get_constant_range(true).get_single_element();
        }
        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L289>
    pub fn as_constant_range(
        &self,
        bw: u32,
        undef_allowed: bool,
    ) -> ConstantRange {
        if self.is_constant_range(undef_allowed) {
            return *self.get_constant_range(true);
        }
        if self.is_constant() {
            return self.get_constant().to_constant_range(bw);
        }
        if self.is_unknown() {
            return ConstantRange::empty(bw);
        }
        ConstantRange::full(bw)
    }

    /// `asConstantRange(Type *Ty, UndefAllowed)`: `bw` is
    /// `Ty->getScalarSizeInBits()` of an integer type.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L299>
    pub fn as_constant_range_of_type(
        &self,
        bw: u32,
        undef_allowed: bool,
    ) -> ConstantRange {
        self.as_constant_range(bw, undef_allowed)
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L304>
    pub fn mark_overdefined(&mut self) -> bool {
        if self.is_overdefined() {
            return false;
        }
        self.range = None;
        self.const_val = None;
        self.tag = Tag::Overdefined;
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L312>
    pub fn mark_undef(&mut self) -> bool {
        if self.is_undef() {
            return false;
        }

        assert!(self.is_unknown());
        self.tag = Tag::Undef;
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L321>
    pub fn mark_constant(
        &mut self,
        v: &LatticeConstant,
        may_include_undef: bool,
    ) -> bool {
        if *v == LatticeConstant::Undef {
            return self.mark_undef();
        }

        if self.is_constant() {
            assert!(self.get_constant() == v, "Marking constant with different value");
            return false;
        }

        if let LatticeConstant::Int { width, bits } = v {
            return self.mark_constant_range(
                ConstantRange::single(*width, *bits),
                MergeOptions::default().set_may_include_undef(may_include_undef),
            );
        }

        assert!(self.is_unknown() || self.is_undef());
        self.tag = Tag::Constant;
        self.const_val = Some(v.clone());
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L341>
    pub fn mark_not_constant(
        &mut self,
        v: &LatticeConstant,
    ) -> bool {
        if let LatticeConstant::Int { width, bits } = v {
            let w = *width;
            let upper = bits.wrapping_add(1) & mask(w);
            return self.mark_constant_range(ConstantRange::new(w, upper, *bits), MergeOptions::default());
        }

        if *v == LatticeConstant::Undef {
            return false;
        }

        if self.is_not_constant() {
            assert!(self.get_not_constant() == v, "Marking !constant with different value");
            return false;
        }

        assert!(self.is_unknown());
        self.tag = Tag::NotConstant;
        self.const_val = Some(v.clone());
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L367>
    pub fn mark_constant_range(
        &mut self,
        new_r: ConstantRange,
        opts: MergeOptions,
    ) -> bool {
        assert!(!new_r.is_empty_set(), "should only be called for non-empty sets");

        if new_r.is_full_set() {
            return self.mark_overdefined();
        }

        let old_tag = self.tag;
        let new_tag = if self.is_undef() || self.is_constant_range_including_undef() || opts.may_include_undef {
            Tag::ConstantRangeIncludingUndef
        } else {
            Tag::ConstantRange
        };
        if self.is_constant_range(true) {
            self.tag = new_tag;
            if *self.get_constant_range(true) == new_r {
                return self.tag != old_tag;
            }

            // Simple form of widening. If a range is extended multiple times,
            // go to overdefined.
            if opts.check_widen {
                self.num_range_extensions += 1;
                if self.num_range_extensions > opts.max_widen_steps {
                    return self.mark_overdefined();
                }
            }

            assert!(new_r.contains_range(self.get_constant_range(true)), "Existing range must be a subset of NewR");
            self.range = Some(new_r);
            return true;
        }

        assert!(self.is_unknown() || self.is_undef() || self.is_constant());
        assert!(
            !self.is_constant() || new_r.contains_range(&self.get_constant().to_constant_range(new_r.width)),
            "Constant must be subset of new range"
        );

        self.num_range_extensions = 0;
        self.tag = new_tag;
        self.range = Some(new_r);
        true
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L407>
    pub fn merge_in(
        &mut self,
        rhs: &ValueLatticeElement,
        opts: MergeOptions,
    ) -> bool {
        let mut opts = opts;
        if rhs.is_unknown() || self.is_overdefined() {
            return false;
        }
        if rhs.is_overdefined() {
            self.mark_overdefined();
            return true;
        }

        if self.is_undef() {
            assert!(!rhs.is_unknown());
            if rhs.is_undef() {
                return false;
            }
            if rhs.is_constant() {
                return self.mark_constant(&rhs.get_constant().clone(), true);
            }
            if rhs.is_constant_range(true) {
                opts = opts.set_may_include_undef(true);
                return self.mark_constant_range(*rhs.get_constant_range(true), opts);
            }
            return self.mark_overdefined();
        }

        if self.is_unknown() {
            assert!(!rhs.is_unknown(), "Unknow RHS should be handled earlier");
            *self = rhs.clone();
            return true;
        }

        if self.is_constant() {
            if rhs.is_constant() && self.get_constant() == rhs.get_constant() {
                // Equal constants may still differ in provenance, propagate it
                // when merging values.
                let current = self.may_have_different_provenance;
                self.may_have_different_provenance |= rhs.may_have_different_provenance();
                return self.may_have_different_provenance != current;
            }
            if rhs.is_undef() {
                return false;
            }
            self.mark_overdefined();
            return true;
        }

        if self.is_not_constant() {
            if rhs.is_not_constant() && self.get_not_constant() == rhs.get_not_constant() {
                return false;
            }
            self.mark_overdefined();
            return true;
        }

        let old_tag = self.tag;
        assert!(self.is_constant_range(true), "New ValueLattice type?");
        if rhs.is_undef() {
            self.tag = Tag::ConstantRangeIncludingUndef;
            return old_tag != self.tag;
        }

        let l = *self.get_constant_range(true);
        let new_r = l.union_with(&rhs.as_constant_range(l.width, true), PreferredRangeType::Smallest);
        self.mark_constant_range(new_r, opts.set_may_include_undef(rhs.is_constant_range_including_undef()))
    }

    /// `ValueLatticeElement::getCompare`; `fold` is
    /// `ConstantFoldCompareInstOperands(Pred, ., ., DL)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueLattice.cpp#L14>
    pub fn get_compare(
        &self,
        pred: IntPredicate,
        other: &ValueLatticeElement,
        fold: &mut dyn FnMut(IntPredicate, &LatticeConstant, &LatticeConstant) -> Option<bool>,
    ) -> Option<bool> {
        // Not yet resolved.
        if self.is_unknown() || other.is_unknown() {
            return None;
        }

        // TODO: Can be made more precise, but always returning undef would be
        // incorrect.
        if self.is_undef() || other.is_undef() {
            return None;
        }

        if self.is_constant() && other.is_constant() {
            return fold(pred, self.get_constant(), other.get_constant());
        }

        if matches!(pred, IntPredicate::Eq | IntPredicate::Ne) {
            // not(C) != C => true, not(C) == C => false.
            if (self.is_not_constant() && other.is_constant() && self.get_not_constant() == other.get_constant())
                || (self.is_constant() && other.is_not_constant() && self.get_constant() == other.get_not_constant())
            {
                return Some(pred == IntPredicate::Ne);
            }
        }

        // Integer constants are represented as ConstantRanges with single
        // elements.
        if !self.is_constant_range(true) || !other.is_constant_range(true) {
            return None;
        }

        let cr = self.get_constant_range(true);
        let other_cr = other.get_constant_range(true);
        if cr.icmp(pred, other_cr) {
            return Some(true);
        }
        if cr.icmp(pred.inverse(), other_cr) {
            return Some(false);
        }

        None
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueLattice.cpp#L56>
    fn has_single_value(val: &ValueLatticeElement) -> bool {
        if val.is_constant_range(true) && val.get_constant_range(true).is_single_element() {
            // Integer constants are single element ranges
            return true;
        }
        val.is_constant()
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueLattice.cpp#L78>
    pub fn intersect(
        &self,
        other: &ValueLatticeElement,
    ) -> ValueLatticeElement {
        if self.is_unknown() {
            return self.clone();
        }
        if other.is_unknown() {
            return other.clone();
        }

        // If we gave up for one, but got a useable fact from the other, use it.
        if self.is_overdefined() {
            return other.clone();
        }
        if other.is_overdefined() {
            return self.clone();
        }

        // Can't get any more precise than constants.
        if Self::has_single_value(self) {
            return self.clone();
        }
        if Self::has_single_value(other) {
            return other.clone();
        }

        // Could be either constant range or not constant here.
        if !self.is_constant_range(true) || !other.is_constant_range(true) {
            // TODO: Arbitrary choice, could be improved
            return self.clone();
        }

        // Intersect two constant ranges
        let range =
            self.get_constant_range(true).intersect_with(other.get_constant_range(true), PreferredRangeType::Smallest);
        // Note: An empty range is implicitly converted to unknown or undef
        // depending on MayIncludeUndef internally.
        Self::get_range(range, self.is_constant_range_including_undef() || other.is_constant_range_including_undef())
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L505>
    pub fn get_num_range_extensions(&self) -> u32 {
        self.num_range_extensions
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L506>
    pub fn set_num_range_extensions(
        &mut self,
        n: u32,
    ) {
        self.num_range_extensions = n;
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L508>
    pub fn may_have_different_provenance(&self) -> bool {
        self.may_have_different_provenance
    }

    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/Analysis/ValueLattice.h#L509>
    pub fn set_may_have_different_provenance(
        &mut self,
        v: bool,
    ) {
        self.may_have_different_provenance = v;
    }

    /// `operator<<(raw_ostream &, const ValueLatticeElement &)`.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ValueLattice.cpp#L112>
    pub fn print(
        &self,
        context: &Context,
        globals: &[GlobalValue],
    ) -> String {
        if self.is_unknown() {
            return "unknown".to_owned();
        }
        if self.is_undef() {
            return "undef".to_owned();
        }
        if self.is_overdefined() {
            return "overdefined".to_owned();
        }

        let bounds = |cr: &ConstantRange| format!("{}, {}", signed(cr.lower, cr.width), signed(cr.upper, cr.width));
        if self.is_not_constant() {
            return format!("notconstant<{}>", print_constant(context, globals, self.get_not_constant()));
        }

        if self.is_constant_range_including_undef() {
            return format!("constantrange incl. undef <{}>", bounds(self.get_constant_range(true)));
        }

        if self.is_constant_range(true) {
            return format!("constantrange<{}>", bounds(self.get_constant_range(true)));
        }
        format!("constant<{}>", print_constant(context, globals, self.get_constant()))
    }
}

/// `*Constant` of `operator<<`: its type and value as LLVM prints them.
fn print_constant(
    context: &Context,
    globals: &[GlobalValue],
    c: &LatticeConstant,
) -> String {
    use crate::types::Type;
    match c {
        LatticeConstant::Undef => "poison".to_owned(),
        LatticeConstant::Int { width, bits } => format!("i{width} {}", signed(*bits, *width)),
        LatticeConstant::Other { ty, kind } => {
            let ty_text = match context.types.get(*ty) {
                Type::Pointer(0) => "ptr".to_owned(),
                Type::Pointer(space) => format!("ptr addrspace({space})"),
                _ => context.types.display(*ty),
            };
            let value = match kind {
                ConstantKind::Null => "null".to_owned(),
                ConstantKind::Zero => "zeroinitializer".to_owned(),
                ConstantKind::Global(global) => globals
                    .get(global.0 as usize)
                    .and_then(|g| g.name.as_deref())
                    .map_or_else(|| format!("@{}", global.0), |name| format!("@{name}")),
                other => format!("{other:?}"),
            };
            format!("{ty_text} {value}")
        }
    }
}

/// `ConstantFoldCompareInstOperands(Pred, C1, C2, DL)` of two lattice
/// constants: true, false, or unfolded (also for an undef).
///
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Analysis/ConstantFolding.cpp#L1332>
pub fn fold_compare(
    context: &Context,
    globals: &[GlobalValue],
    layout: &crate::datalayout::DataLayout,
    pred: IntPredicate,
    c1: &LatticeConstant,
    c2: &LatticeConstant,
) -> Option<bool> {
    let make = |c: &LatticeConstant| match c {
        LatticeConstant::Undef => None,
        LatticeConstant::Int { width, bits } => {
            let ty = context.types.int(*width);
            Some(context.constant(Constant { ty, kind: ConstantKind::Int(*bits) }))
        }
        LatticeConstant::Other { ty, kind } => Some(context.constant(Constant { ty: *ty, kind: kind.clone() })),
    };
    let (a, b) = (make(c1)?, make(c2)?);
    let folded = crate::constantfolding::constant_fold_compare_inst_operands(context, globals, layout, pred, a, b)?;
    match context.get(folded).kind {
        ConstantKind::Int(bits) => Some(bits & 1 == 1),
        _ => None,
    }
}
