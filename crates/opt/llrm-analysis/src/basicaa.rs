//! BasicAA, a translation of `BasicAliasAnalysis.cpp` at d1106deb71cc behind
//! the interface of [`crate::aa`] (stage S1/S2 of
//! docs/architecture/basicaa-port.md). Per query: nothing is solved for the
//! whole function, and what one query learns lives in its `AAQueryInfo`.
//!
//! Translated: `aliasCheck` (empty locations, casts stripped, the same value,
//! the underlying objects, identified objects, arguments against function-local
//! objects, an escape source against a non-captured local object, an access
//! larger than the whole object), `DecomposeGEPExpression` (constant offsets,
//! variable indices by value and scale), `subtractDecomposedGEPs` and
//! `aliasGEP` as far as it needs no value ranges: the inbounds and
//! constant-difference rules, nuw, and the GCD test. Not yet (deviations in the
//! plan page): `GetLinearExpression` through casts, `analyzeVariableOffsets`'
//! ranges, `aliasPHI`, `aliasSelect`, the mod/ref queries (they answer the top
//! of the lattice).

use llrm_mir::context::ConstantKind;
use llrm_mir::module::{GlobalKind, Instruction, Operand, ValueDef};
use llrm_mir::opcode::{CastOp, Flags, Opcode};

use crate::aa::{AAQueryInfo, AAResults, AliasResult, LocationSize, MemoryEffects, MemoryLocation, ModRefInfo};
use crate::memory::Unit;

/// `MaxLookupSearchDepth`.
const MAX_LOOKUP: usize = 6;

/// A pointer as a base object, a constant offset and variable indices
/// (`DecomposedGEP`).
#[derive(Clone, Debug)]
struct Decomposed {
    base: Operand,
    offset: i128,
    /// `(index value, scale)`, a scale never zero.
    vars: Vec<(Operand, i128)>,
    inbounds: bool,
    nuw: bool,
}

pub struct BasicAA<'a> {
    unit: &'a Unit<'a>,
}

impl<'a> BasicAA<'a> {
    pub fn new(unit: &'a Unit<'a>) -> Self {
        Self { unit }
    }

    fn defining(
        &self,
        operand: Operand,
    ) -> Option<&'a Instruction> {
        self.unit.defining(operand).map(|(_, one)| one)
    }

    fn signed(
        &self,
        operand: Operand,
        bits: u128,
    ) -> i128 {
        let width = self.unit.int_bits(operand).unwrap_or(128);
        if width >= 128 || bits >> (width - 1) & 1 == 0 { bits as i128 } else { (bits | (!0u128 << width)) as i128 }
    }

    /// `stripPointerCastsForAliasAnalysis`: casts only.
    fn strip(
        &self,
        mut operand: Operand,
    ) -> Operand {
        while let Some(one) = self.defining(operand) {
            match one.opcode {
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => operand = one.operands[0],
                _ => break,
            }
        }
        operand
    }

    /// `getUnderlyingObject`: through geps and casts, `MAX_LOOKUP` steps.
    fn underlying(
        &self,
        mut operand: Operand,
    ) -> Operand {
        for _ in 0..MAX_LOOKUP {
            let Some(one) = self.defining(operand) else { break };
            match one.opcode {
                Opcode::GetElementPtr { .. } | Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => {
                    operand = one.operands[0]
                }
                _ => break,
            }
        }
        operand
    }

    /// `DecomposeGEPExpression`.
    fn decompose(
        &self,
        mut operand: Operand,
    ) -> Decomposed {
        let mut found = Decomposed { base: operand, offset: 0, vars: Vec::new(), inbounds: true, nuw: true };
        for _ in 0..MAX_LOOKUP {
            let Some(one) = self.defining(operand) else { break };
            match &one.opcode {
                Opcode::Cast(CastOp::BitCast | CastOp::AddrSpaceCast) => operand = one.operands[0],
                // A phi of one arm, as LCSSA makes.
                Opcode::Phi if one.operands.len() == 2 => operand = one.operands[0],
                Opcode::GetElementPtr { source } => {
                    found.inbounds &= one.flags.contains(Flags::INBOUNDS);
                    found.nuw &= one.flags.contains(Flags::NUW);
                    let indices: Vec<Option<i128>> = one.operands[1..]
                        .iter()
                        .map(|&index| self.unit.int_constant(index).map(|bits| self.signed(index, bits)))
                        .collect();
                    let (constant, variable) =
                        self.unit.layout.collect_offset(&self.unit.context.types, *source, &indices);
                    found.offset += constant;
                    for (at, scale) in variable {
                        let (index, scale) = (one.operands[1 + at], scale as i128);
                        match found.vars.iter().position(|(had, _)| *had == index) {
                            Some(have) => {
                                found.vars[have].1 += scale;
                                if found.vars[have].1 == 0 {
                                    found.vars.remove(have);
                                }
                            }
                            None if scale != 0 => found.vars.push((index, scale)),
                            None => {}
                        }
                    }
                    operand = one.operands[0];
                }
                _ => break,
            }
            found.base = operand;
        }
        found.base = operand;
        found
    }

    fn global_variable(
        &self,
        operand: Operand,
    ) -> bool {
        let Operand::Constant(id) = operand else { return false };
        let ConstantKind::Global(global) = self.unit.context.get(id).kind else { return false };
        matches!(
            self.unit.globals.get(global.0 as usize).map(|one| &one.kind),
            Some(GlobalKind::Variable(_))
        )
    }

    fn argument_attrs(
        &self,
        operand: Operand,
    ) -> Option<&'a [llrm_mir::opcode::Attribute]> {
        let Operand::Value(value) = operand else { return None };
        let ValueDef::Argument(at) = self.unit.function.value(value).def else { return None };
        self.unit.function.parameter_attrs.get(at as usize).map(Vec::as_slice)
    }

    fn noalias_or_byval_argument(
        &self,
        operand: Operand,
    ) -> bool {
        self.argument_attrs(operand).is_some_and(|attrs| {
            llrm_mir::facts::Facts::of(attrs).no_alias()
                || attrs.iter().any(|one| match one {
                    llrm_mir::opcode::Attribute::Flag(name) | llrm_mir::opcode::Attribute::Type(name, _) => {
                        name == "byval"
                    }
                    _ => false,
                })
        })
    }

    fn noalias_call(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_some_and(|one| match &one.opcode {
            Opcode::Call(info) | Opcode::Invoke(info) => llrm_mir::facts::Facts::of(&info.return_attrs).no_alias(),
            _ => false,
        })
    }

    /// `isIdentifiedFunctionLocal`.
    fn identified_local(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }))
            || self.noalias_call(operand)
            || self.noalias_or_byval_argument(operand)
    }

    /// `isIdentifiedObject`.
    fn identified(
        &self,
        operand: Operand,
    ) -> bool {
        self.identified_local(operand) || self.global_variable(operand)
    }

    fn argument(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            operand,
            Operand::Value(value) if matches!(self.unit.function.value(value).def, ValueDef::Argument(_))
        )
    }

    /// `isEscapeSource`: a call that may return what it was given, a load, an
    /// `inttoptr`.
    fn escape_source(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_some_and(|one| {
            matches!(
                one.opcode,
                Opcode::Call(_) | Opcode::Invoke(_) | Opcode::Load { .. } | Opcode::Cast(CastOp::IntToPtr)
            )
        })
    }

    /// `AAQI.CA->getCapturesBefore(object).capturesNothing`, where the object
    /// is a function-local identified one (the only kind capture tracking
    /// is asked about), remembered in the batch.
    fn not_captured(
        &self,
        object: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> bool {
        let Operand::Value(value) = object else { return false };
        if !self.identified_local(object) || self.argument(object) {
            return false;
        }
        let key = u64::from(value.0);
        if let Some(found) = aaqi.captured.get(&key) {
            return !*found;
        }
        let captured = crate::capturecheck::may_be_captured(self.unit, value);
        aaqi.captured.insert(key, captured);
        !captured
    }

    /// `isObjectSmallerThan(object, size)`: an identified object whose whole
    /// extent is less than the access.
    fn object_smaller_than(
        &self,
        object: Operand,
        size: u64,
    ) -> bool {
        self.identified(object)
            && crate::memory::extent_of(self.unit, object).is_some_and(|extent| (extent as u64) < size)
    }

    fn is_object_size(
        &self,
        object: Operand,
        size: u64,
    ) -> bool {
        self.identified(object) && crate::memory::extent_of(self.unit, object) == Some(size as i64)
    }

    fn alias_check(
        &self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        if a.size.is_zero() || b.size.is_zero() {
            return AliasResult::NoAlias;
        }
        let (v1, v2) = (self.strip(a.ptr), self.strip(b.ptr));
        if v1 == v2 {
            return AliasResult::MustAlias;
        }
        let (o1, o2) = (self.underlying(v1), self.underlying(v2));
        if o1 != o2 {
            // Two different identified objects.
            if self.identified(o1) && self.identified(o2) {
                return AliasResult::NoAlias;
            }
            // Arguments cannot alias what is identified at the function level.
            if (self.argument(o1) && self.identified_local(o2)) || (self.argument(o2) && self.identified_local(o1)) {
                return AliasResult::NoAlias;
            }
            // A call or load result cannot be a non-escaping local object.
            if (self.escape_source(o1) && self.not_captured(o2, aaqi))
                || (self.escape_source(o2) && self.not_captured(o1, aaqi))
            {
                return AliasResult::NoAlias;
            }
        }
        // An access larger than the entire object on the other side is
        // undefined.
        if b.size.value().is_some_and(|size| self.object_smaller_than(o1, size))
            || a.size.value().is_some_and(|size| self.object_smaller_than(o2, size))
        {
            return AliasResult::NoAlias;
        }
        // An access that may be before its pointer is canonicalised to an
        // unbounded one.
        let (s1, s2) = if a.size.may_be_before_pointer() || b.size.may_be_before_pointer() {
            (LocationSize::AfterPointer, LocationSize::AfterPointer)
        } else {
            (a.size, b.size)
        };
        let (l1, l2) = (MemoryLocation { ptr: v1, size: s1, ..*a }, MemoryLocation { ptr: v2, size: s2, ..*b });
        if let Some(found) = aaqi.cached(&l1, &l2) {
            return found;
        }
        let result = self.alias_recursive(&l1, &l2, o1, o2, aaqi);
        aaqi.remember(&l1, &l2, result);
        result
    }

    fn alias_recursive(
        &self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        o1: Operand,
        o2: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        let is_gep = |operand: Operand| {
            self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::GetElementPtr { .. }))
        };
        if is_gep(a.ptr) {
            let result = self.alias_gep(a, b, o1, o2, aaqi);
            if result != AliasResult::MayAlias {
                return result;
            }
        } else if is_gep(b.ptr) {
            let result = self.alias_gep(b, a, o2, o1, aaqi).swap();
            if result != AliasResult::MayAlias {
                return result;
            }
        }
        // (aliasPHI, aliasSelect: not yet.)
        // Both inside one object, one access covering all of it.
        if o1 == o2
            && a.size.is_precise()
            && b.size.is_precise()
            && (self.is_object_size(o1, a.size.value().unwrap_or(0))
                || self.is_object_size(o2, b.size.value().unwrap_or(0)))
        {
            return AliasResult::PartialAlias { offset: None };
        }
        AliasResult::MayAlias
    }

    /// `aliasGEP`, without the range reasoning over variable indices.
    fn alias_gep(
        &self,
        gep: &MemoryLocation,
        other: &MemoryLocation,
        underlying1: Operand,
        underlying2: Operand,
        aaqi: &mut AAQueryInfo,
    ) -> AliasResult {
        let (mut size1, mut size2) = (gep.size, other.size);
        let base_objects = |this: &Self, aaqi: &mut AAQueryInfo| {
            let result = this.alias_check(
                &MemoryLocation::before_or_after(underlying1),
                &MemoryLocation::before_or_after(underlying2),
                aaqi,
            );
            if result == AliasResult::NoAlias { AliasResult::NoAlias } else { AliasResult::MayAlias }
        };
        if size1.value().is_none() && size2.value().is_none() {
            let is_join =
                self.defining(other.ptr).is_some_and(|one| matches!(one.opcode, Opcode::Phi | Opcode::Select));
            if is_join {
                return AliasResult::MayAlias;
            }
            let joins = |operand| {
                self.defining(operand).is_some_and(|one| {
                    matches!(
                        one.opcode,
                        Opcode::GetElementPtr { .. } | Opcode::Phi | Opcode::Select
                    )
                })
            };
            if joins(other.ptr) || joins(underlying1) || joins(underlying2) {
                return base_objects(self, aaqi);
            }
            return AliasResult::MayAlias;
        }
        let (mut d1, mut d2) = (self.decompose(gep.ptr), self.decompose(other.ptr));
        if d1.base == gep.ptr && d2.base == other.ptr {
            return AliasResult::MayAlias;
        }
        // Swap if the second has more variable indices.
        if d1.vars.len() < d2.vars.len() {
            std::mem::swap(&mut d1, &mut d2);
            std::mem::swap(&mut size1, &mut size2);
        }
        // d1 - d2.
        d1.nuw &= d1.offset >= d2.offset;
        d1.offset -= d2.offset;
        for (value, scale) in d2.vars.iter().copied() {
            match d1.vars.iter().position(|(had, _)| *had == value) {
                Some(have) => {
                    if d1.vars[have].1 != scale {
                        d1.nuw &= d1.vars[have].1 >= scale;
                        d1.vars[have].1 -= scale;
                    } else {
                        d1.vars.remove(have);
                    }
                }
                None => {
                    d1.vars.push((value, -scale));
                    d1.nuw = false;
                }
            }
        }
        // An inbounds gep that would have to start out of bounds for the two to
        // alias.
        let base_of_object = |operand: Operand| {
            self.global_variable(operand)
                || self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }))
        };
        if d1.inbounds
            && d1.vars.is_empty()
            && size2.value().is_some()
            && d1.offset >= i128::from(size2.value().unwrap_or(0))
            && base_of_object(d2.base)
        {
            return AliasResult::NoAlias;
        }
        if d2.inbounds
            && d1.vars.is_empty()
            && size1.value().is_some()
            && d1.offset <= -i128::from(size1.value().unwrap_or(0))
            && base_of_object(d1.base)
        {
            return AliasResult::NoAlias;
        }
        // Equal offsets: the sizes are kept for the check of the bases.
        if d1.offset == 0 && d1.vars.is_empty() {
            return self.alias_check(
                &MemoryLocation { ptr: d1.base, size: size1, aa_info: gep.aa_info },
                &MemoryLocation { ptr: d2.base, size: size2, aa_info: other.aa_info },
                aaqi,
            );
        }
        let base_alias = self.alias_check(
            &MemoryLocation::before_or_after(d1.base),
            &MemoryLocation::before_or_after(d2.base),
            aaqi,
        );
        if base_alias != AliasResult::MustAlias {
            return base_alias;
        }
        if d1.vars.is_empty() {
            let (mut offset, mut left, mut right) = (d1.offset, size2, size1);
            let swapped = offset < 0;
            if swapped {
                std::mem::swap(&mut left, &mut right);
                offset = -offset;
            }
            let Some(left_bytes) = left.value() else { return AliasResult::MayAlias };
            if offset < i128::from(left_bytes) {
                let nested = right.value().is_some_and(|bytes| offset + i128::from(bytes) <= i128::from(left_bytes));
                if nested {
                    let shift = i32::try_from(-offset).ok();
                    let result = AliasResult::PartialAlias { offset: shift };
                    return if swapped { result.swap() } else { result };
                }
                return AliasResult::PartialAlias { offset: None };
            }
            return AliasResult::NoAlias;
        }
        // The distance is nuw-bounded below by the constant part.
        if d1.nuw && size2.value().is_some_and(|bytes| d1.offset >= i128::from(bytes)) {
            return AliasResult::NoAlias;
        }
        let (Some(bytes1), Some(bytes2)) = (size1.value(), size2.value()) else { return AliasResult::MayAlias };
        // The variable part is a multiple of the GCD of the scales (a scale
        // without nsw is known only down to its lowest set bit).
        let gcd = d1.vars.iter().map(|(_, scale)| 1i128 << scale.unsigned_abs().trailing_zeros().min(100)).fold(0, gcd);
        if gcd > 0 {
            let modulus = d1.offset.rem_euclid(gcd);
            if modulus >= i128::from(bytes2) && gcd - modulus >= i128::from(bytes1) {
                return AliasResult::NoAlias;
            }
        }
        AliasResult::MayAlias
    }
}

fn gcd(
    a: i128,
    b: i128,
) -> i128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

impl AAResults for BasicAA<'_> {
    fn alias(
        &mut self,
        a: &MemoryLocation,
        b: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
        _ctx: Option<llrm_mir::module::InstId>,
    ) -> AliasResult {
        self.alias_check(a, b, aaqi)
    }

    fn get_mod_ref_info_mask(
        &mut self,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
        _: bool,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn get_arg_mod_ref_info(
        &mut self,
        _: llrm_mir::module::InstId,
        _: u32,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn get_memory_effects(
        &mut self,
        _: llrm_mir::module::InstId,
        _: &mut AAQueryInfo,
    ) -> MemoryEffects {
        MemoryEffects::unknown()
    }

    fn get_mod_ref_info(
        &mut self,
        _: llrm_mir::module::InstId,
        _: Option<&MemoryLocation>,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn get_mod_ref_info_call(
        &mut self,
        _: llrm_mir::module::InstId,
        _: llrm_mir::module::InstId,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }

    fn call_captures_before(
        &mut self,
        _: llrm_mir::module::InstId,
        _: &MemoryLocation,
        _: &mut AAQueryInfo,
    ) -> ModRefInfo {
        ModRefInfo::ModRef
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{layout, parsed, value};

    const M32: &str = "target datalayout = \"e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32\"\n";

    /// BasicAA's answer for the pointers `%a` and `%b` of `@f`, each accessed
    /// with `size` bytes.
    fn answer(
        body: &str,
        a: &str,
        b: &str,
        size: u64,
    ) -> AliasResult {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location =
            |name: &str| MemoryLocation::new(Operand::Value(value(function, name)), LocationSize::Precise(size));
        BasicAA::new(&unit).alias(&location(a), &location(b), &mut AAQueryInfo::default(), None)
    }

    /// Two inbounds geps off different bases that share a variable index: the
    /// index cancels, the constant left puts one access past the other's
    /// base object (`aliasGEP`'s inbounds rule). `opt -passes=aa-eval`
    /// (24.0.0git d1106deb71cc) says NoAlias for this text and MayAlias with
    /// the `inbounds` dropped; today's facts say MayAlias for both: 16,000
    /// of the pairs of the replay set.
    #[test]
    fn an_inbounds_gep_past_the_other_base_is_no_alias_and_a_plain_one_is_not() {
        let body = |gep: &str| {
            format!(
                "@g = internal global [820 x i8] zeroinitializer\ndefine i32 @f(ptr %p, i32 %i) {{\nentry:\n  %a = getelementptr {gep} i8, ptr @g, i32 %i\n  %b0 = getelementptr {gep} i8, ptr %p, i32 %i\n  %b = getelementptr {gep} i8, ptr %b0, i32 4\n  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body("inbounds"), "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body(""), "a", "b", 4), AliasResult::MayAlias);
    }

    /// A pointer loaded from memory cannot be a local object whose address is
    /// never stored, passed or returned (`isEscapeSource` against a
    /// non-captured local); once the address is stored it can.
    #[test]
    fn a_loaded_pointer_is_no_alias_with_a_local_that_never_escapes() {
        let body = |escape: &str| {
            format!(
                "@cell = global ptr null\ndefine i32 @f(ptr %p) {{\nentry:\n  %b = alloca i32\n  {escape}\n  %a = load ptr, ptr @cell\n  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body("store i32 0, ptr %b"), "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body("store ptr %b, ptr @cell"), "a", "b", 4), AliasResult::MayAlias);
    }

    /// CaptureTracking explores 150 uses
    /// (`capture-tracking-max-uses-to-explore`) and then takes the
    /// pointer as captured. The slot has two uses besides its loads; the pinned
    /// opt (24.0.0git d1106deb71cc) answers NoAlias with 148 loads and
    /// MayAlias with 149. A cap of 100 gave 32 MayAlias where LLVM said
    /// NoAlias on d_faces-O1 (an alloca with 123 uses).
    #[test]
    fn capture_tracking_gives_up_after_a_hundred_and_fifty_uses() {
        let body = |loads: usize| {
            let reads: String = (0..loads).map(|n| format!("  %w{n} = load ptr, ptr %slot\n")).collect();
            format!(
                "define i32 @f(ptr %p, i32 %i) {{\nentry:\n  %slot = alloca ptr\n  store ptr %p, ptr %slot\n  %q = load ptr, ptr %slot\n  %a = getelementptr inbounds i8, ptr %q, i32 %i\n{reads}  ret i32 0\n}}\n"
            )
        };
        assert_eq!(answer(&body(148), "a", "slot", 4), AliasResult::NoAlias);
        assert_eq!(answer(&body(149), "a", "slot", 4), AliasResult::MayAlias);
    }

    /// Distinct allocas are distinct objects.
    #[test]
    fn two_allocas_are_no_alias_and_one_pointer_is_must_alias() {
        let body = "define i32 @f() {\nentry:\n  %a = alloca i32\n  %b = alloca i32\n  ret i32 0\n}\n";
        assert_eq!(answer(body, "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(body, "a", "a", 4), AliasResult::MustAlias);
    }
}
