//! BasicAA, a translation of `BasicAliasAnalysis.cpp` at d1106deb71cc behind
//! the interface of [`crate::aa`] (stage S1/S2 of
//! docs/architecture/basicaa-port.md). Per query: nothing is solved for the
//! whole function, and what one query learns lives in its `AAQueryInfo`.
//!
//! # How it works
//! Translated: `aliasCheck` (empty locations, casts stripped, the same value,
//! the underlying objects, identified objects, arguments against function-local
//! objects, an escape source against a non-captured local object, an access
//! larger than the whole object), `DecomposeGEPExpression` (constant offsets,
//! variable indices by value and scale), `subtractDecomposedGEPs` and
//! `aliasGEP` as far as it needs no value ranges: the inbounds and
//! constant-difference rules, nuw, and the GCD test. Not yet (deviations in the
//! plan page): `GetLinearExpression` through casts, `analyzeVariableOffsets`'
//! ranges, `aliasPHI`, `aliasSelect`, and the mod/ref queries
//! other than `getModRefInfoMask` and `getModRefInfo(Call, Loc)` (they answer
//! the top of the lattice). `getMemoryEffects(Call)` and `getArgModRefInfo`
//! read the attributes.
//!
//! # Example
//! Two allocas never alias, and a call cannot touch an alloca nobody holds a
//! pointer to:
//!
//! ```text
//! %a = alloca i32
//! %b = alloca i32        ; alias(%a, %b) = NoAlias
//! call void @g()         ; getModRefInfo(call, %a) = NoModRef
//! ```
//!
//! # Provides
//! [`BasicAA`], an [`crate::aa::AAResults`]. No client reads it yet.

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

    /// `getUnderlyingObjectAggressive` (ValueTracking.cpp:7380,
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/ValueTracking.cpp#L7380>):
    /// the one object every arm of the selects and phis reaches, else the
    /// plain underlying object.
    pub(crate) fn underlying_aggressive(
        &self,
        operand: Operand,
    ) -> Operand {
        const MAX_VISITED: usize = 8;
        let first = self.underlying(operand);
        let mut work = vec![operand];
        let mut visited = Vec::new();
        let mut object = None;
        let mut is_first = true;
        loop {
            let popped = work.pop().expect("a pending pointer");
            let pointer = if is_first { first } else { self.underlying(popped) };
            is_first = false;
            if !visited.contains(&pointer) {
                visited.push(pointer);
                if visited.len() == MAX_VISITED {
                    return first;
                }
                match self.defining(pointer) {
                    Some(one) if matches!(one.opcode, Opcode::Select) => work.extend_from_slice(&one.operands[1..]),
                    // A phi's operands are value, block pairs.
                    Some(one) if matches!(one.opcode, Opcode::Phi) => work.extend(one.operands.iter().step_by(2)),
                    _ => match object {
                        None => object = Some(pointer),
                        Some(found) if found != pointer => return first,
                        Some(_) => {}
                    },
                }
            }
            if work.is_empty() {
                return object.unwrap_or(first);
            }
        }
    }

    /// `isIdentifiedObject`: any global, not only a variable.
    pub(crate) fn is_identified_object(
        &self,
        operand: Operand,
    ) -> bool {
        self.identified(operand)
            || matches!(
                operand,
                Operand::Constant(id) if matches!(self.unit.context.get(id).kind, ConstantKind::Global(_))
            )
    }

    pub(crate) fn is_alloca(
        &self,
        operand: Operand,
    ) -> bool {
        self.defining(operand).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }))
    }

    pub(crate) fn is_argument(
        &self,
        operand: Operand,
    ) -> bool {
        self.argument(operand)
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

impl BasicAA<'_> {
    /// `getModRefInfo(Call, Loc)`: what the call does to the location, from
    /// the call's memory effects and, for what it reaches only through
    /// arguments, from which arguments may alias the location.
    fn call_mod_ref(
        &mut self,
        call: llrm_mir::module::InstId,
        loc: &MemoryLocation,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        use crate::aa::IRMemLocation::{ArgMem, ErrnoMem, InaccessibleMem, Other};
        let op = self.unit.function.instruction(call);
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &op.opcode else { return ModRefInfo::ModRef };
        let object = self.underlying(loc.ptr);
        let local_slot = self.defining(object).is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. }));
        // A tail call cannot reach this frame's slots, but may take one by
        // value.
        let by_value = |attrs: &[llrm_mir::opcode::Attribute]| {
            attrs
                .iter()
                .any(
                    |one| matches!(
                        one,
                        llrm_mir::opcode::Attribute::Flag(name) | llrm_mir::opcode::Attribute::Type(name, _) if name == "byval"
                    ),
                )
        };
        if local_slot
            && info.tail == llrm_mir::opcode::Tail::Tail
            && !(by_value(&info.attrs)
                || by_value(&info.return_attrs)
                || info.argument_attrs.iter().any(|one| by_value(one)))
        {
            return ModRefInfo::NoModRef;
        }
        let effects = self.get_memory_effects(call, aaqi).with(InaccessibleMem, ModRefInfo::NoModRef);
        if effects.does_not_access_memory() {
            return ModRefInfo::NoModRef;
        }
        let mut arg = effects.mod_ref(ArgMem);
        let errno = effects.mod_ref(ErrnoMem);
        let mut other = effects.mod_ref(Other);
        // What does not escape is reached through arguments only.
        let callee =
            llrm_mir::memory::callee(self.unit.context, self.unit.function, call).and_then(|one| {
                match &self.unit.globals.get(one.0 as usize)?.kind {
                    GlobalKind::Function(function) => Some(function),
                    GlobalKind::Variable(_) => None,
                }
            });
        let returns_twice = llrm_mir::memory::has(&info.attrs, "returns_twice")
            || callee.is_some_and(|one| llrm_mir::memory::has(&one.attrs, "returns_twice"));
        let is_call_itself = matches!(object, Operand::Value(value) if op.result == Some(value));
        if other != ModRefInfo::NoModRef
            && matches!(object, Operand::Value(_))
            && !is_call_itself
            && (local_slot || !returns_twice)
            && self.not_captured(object, aaqi)
        {
            other = ModRefInfo::NoModRef;
        }
        if arg.union(other) != other {
            let mut narrowed = ModRefInfo::NoModRef;
            let arguments = op.operands.len().saturating_sub(1);
            for at in 0..arguments {
                if self.unit.space(op.operands[at]).is_none() {
                    continue;
                }
                let at = at as u32;
                let Some(place) = MemoryLocation::get_for_argument(self.unit, call, at) else { continue };
                if self.alias_check(&place, loc, aaqi) != AliasResult::NoAlias {
                    narrowed = narrowed.union(arg.intersect(self.get_arg_mod_ref_info(call, at)));
                }
                if narrowed == arg {
                    break;
                }
            }
            arg = narrowed;
        }
        let mut result = arg.union(other);
        // aliasErrno without the target's int size: only a local slot or an
        // access wider than an int (32 bits) is told apart.
        if errno.union(result) != result {
            let apart = self.identified_local(object) || loc.size.value().is_some_and(|bytes| bytes * 8 > 32);
            if !apart {
                result = result.union(errno);
            }
        }
        if result != ModRefInfo::ModRef {
            return result;
        }
        let invariant_start = llrm_mir::memory::callee(self.unit.context, self.unit.function, call)
            .and_then(|one| self.unit.globals.get(one.0 as usize)?.name.as_deref())
            .is_some_and(|name| name.starts_with("llvm.invariant.start"));
        if invariant_start { ModRefInfo::Ref } else { ModRefInfo::ModRef }
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

    /// `getModRefInfoMask`: a location in a constant global or a
    /// `noalias readonly` argument is never written; one in no object this
    /// walk (through selects and phis of up to eight arms) names is
    /// anything.
    fn get_mod_ref_info_mask(
        &mut self,
        loc: &MemoryLocation,
        _: &mut AAQueryInfo,
        ignore_locals: bool,
    ) -> ModRefInfo {
        let mut max_lookup = 8;
        let mut worklist = vec![loc.ptr];
        let mut visited: Vec<Operand> = Vec::new();
        let mut result = ModRefInfo::NoModRef;
        // `do { ... } while (!Worklist.empty() && --MaxLookup)`: a `continue`
        // of the C++ still counts a step.
        loop {
            let next = worklist.pop().expect("a non-empty worklist");
            'step: {
                let object = self.underlying(next);
                if visited.contains(&object) {
                    break 'step;
                }
                visited.push(object);
                let defining = self.defining(object);
                if ignore_locals && defining.is_some_and(|one| matches!(one.opcode, Opcode::Alloca { .. })) {
                    break 'step;
                }
                if let Some(attrs) = self.argument_attrs(object) {
                    let facts = llrm_mir::facts::Facts::of(attrs);
                    if facts.no_alias() && facts.read_only() {
                        result = result.union(ModRefInfo::Ref);
                        break 'step;
                    }
                }
                if let Operand::Constant(id) = object {
                    if let ConstantKind::Global(global) = self.unit.context.get(id).kind {
                        match self.unit.globals.get(global.0 as usize).map(|one| &one.kind) {
                            Some(GlobalKind::Variable(variable)) if variable.constant => break 'step,
                            _ => return ModRefInfo::ModRef,
                        }
                    }
                }
                match defining.map(|one| (&one.opcode, &one.operands)) {
                    Some((Opcode::Select, operands)) => worklist.extend([operands[1], operands[2]]),
                    Some((Opcode::Phi, operands)) => {
                        if operands.len() / 2 > 8 {
                            return ModRefInfo::ModRef;
                        }
                        worklist.extend(operands.iter().step_by(2).copied());
                    }
                    _ => return ModRefInfo::ModRef,
                }
            }
            max_lookup -= 1;
            if worklist.is_empty() || max_lookup == 0 {
                break;
            }
        }
        if worklist.is_empty() { result } else { ModRefInfo::ModRef }
    }

    /// `BasicAAResult::getArgModRefInfo` (BasicAliasAnalysis.cpp:897,
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L897>):
    /// the call site's attributes on the argument, else the callee's on the
    /// parameter (`CallBase::paramHasAttr`).
    fn get_arg_mod_ref_info(
        &mut self,
        call: llrm_mir::module::InstId,
        arg: u32,
    ) -> ModRefInfo {
        let function = self.unit.function;
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else {
            return ModRefInfo::ModRef;
        };
        let callee = llrm_mir::memory::callee(self.unit.context, function, call)
            .and_then(|one| self.unit.globals.get(one.0 as usize))
            .and_then(|one| one.function());
        let stated = |fact: fn(&llrm_mir::facts::Facts) -> bool| {
            let site =
                info.argument_attrs.get(arg as usize).is_some_and(|attrs| fact(&llrm_mir::facts::Facts::of(attrs)));
            site || callee
                .and_then(|one| one.parameter_attrs.get(arg as usize))
                .is_some_and(|attrs| fact(&llrm_mir::facts::Facts::of(attrs)))
        };
        if stated(llrm_mir::facts::Facts::read_none) {
            ModRefInfo::NoModRef
        } else if stated(llrm_mir::facts::Facts::write_only) {
            ModRefInfo::Mod
        } else if stated(llrm_mir::facts::Facts::read_only) {
            ModRefInfo::Ref
        } else {
            ModRefInfo::ModRef
        }
    }

    /// `BasicAAResult::getMemoryEffects(Call)` (BasicAliasAnalysis.cpp:860,
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/Analysis/BasicAliasAnalysis.cpp#L860>):
    /// the call site's `memory(...)` met with the direct callee's.
    ///
    /// DEVIATE B1: no `hasReadingOperandBundles`/`hasClobberingOperandBundles`/
    /// `isVolatile` terms. MIR has no operand bundles (#2203) and no volatile
    /// call; `CallInfo` carries neither, so there is nothing to read.
    /// DEVIATE B2: no `experimental_guard`/`experimental_deoptimize` case; MIR
    /// has neither intrinsic.
    fn get_memory_effects(
        &mut self,
        call: llrm_mir::module::InstId,
        _: &mut AAQueryInfo,
    ) -> MemoryEffects {
        let function = self.unit.function;
        let (Opcode::Call(info) | Opcode::Invoke(info)) = &function.instruction(call).opcode else {
            return MemoryEffects::unknown();
        };
        let site = MemoryEffects::of_attrs(&info.attrs);
        let callee = llrm_mir::memory::callee(self.unit.context, function, call)
            .and_then(|one| self.unit.globals.get(one.0 as usize))
            .and_then(|one| one.function());
        callee.map_or(site, |one| site.intersect(MemoryEffects::of_attrs(&one.attrs)))
    }

    /// A call against a location is `getModRefInfo(Call, Loc)`; every other
    /// instruction is not answered yet.
    fn get_mod_ref_info(
        &mut self,
        inst: llrm_mir::module::InstId,
        loc: Option<&MemoryLocation>,
        aaqi: &mut AAQueryInfo,
    ) -> ModRefInfo {
        let is_call = matches!(
            self.unit.function.instruction(inst).opcode,
            Opcode::Call(_) | Opcode::Invoke(_)
        );
        match loc {
            Some(loc) if is_call => self.call_mod_ref(inst, loc, aaqi),
            _ => ModRefInfo::ModRef,
        }
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
    use crate::aa::IRMemLocation;
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

    /// The call `%c` of `@f` in `body`: its effects and its argument 0's.
    fn call_effects(body: &str) -> (MemoryEffects, ModRefInfo) {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let call = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|&inst| matches!(function.instruction(inst).opcode, Opcode::Call(_)))
            .expect("a call");
        let mut aa = BasicAA::new(&unit);
        (aa.get_memory_effects(call, &mut AAQueryInfo::default()), aa.get_arg_mod_ref_info(call, 0))
    }

    /// `getMemoryEffects(Call)` met the site's `memory(...)` with the callee's.
    /// The stub answered `unknown` for every call: `memory(argmem: read)` was
    /// `ModRef` everywhere, and `FunctionAttrs` could infer nothing.
    #[test]
    fn a_calls_effects_are_the_meet_of_its_site_and_its_callee() {
        use IRMemLocation::{ArgMem, Other};
        let (declared, _) = call_effects(
            "declare void @g(ptr) memory(argmem: read)\ndefine void @f(ptr %p) {\nentry:\n  call void @g(ptr %p)\n  ret void\n}\n",
        );
        assert_eq!(declared.mod_ref(ArgMem), ModRefInfo::Ref);
        assert_eq!(declared.mod_ref(Other), ModRefInfo::NoModRef);
        let (met, _) = call_effects(
            "declare void @g(ptr) memory(argmem: readwrite)\ndefine void @f(ptr %p) {\nentry:\n  call void @g(ptr %p) memory(argmem: write)\n  ret void\n}\n",
        );
        assert_eq!(met.mod_ref(ArgMem), ModRefInfo::Mod);
        let (none, _) =
            call_effects("declare void @g()\ndefine void @f() {\nentry:\n  call void @g()\n  ret void\n}\n");
        assert_eq!(none, MemoryEffects::unknown());
        let (readnone, _) =
            call_effects("declare void @g() readnone\ndefine void @f() {\nentry:\n  call void @g()\n  ret void\n}\n");
        assert!(readnone.does_not_access_memory());
    }

    /// `getArgModRefInfo`: `readnone`, `writeonly`, `readonly` on the site's
    /// argument or the callee's parameter, in that order.
    #[test]
    fn an_argument_is_what_its_attributes_leave_open() {
        let arg = |callee: &str, site: &str| {
            call_effects(&format!(
                "declare void @g(ptr {callee})\ndefine void @f(ptr %p) {{\nentry:\n  call void @g(ptr {site} %p)\n  ret void\n}}\n"
            ))
            .1
        };
        assert_eq!(arg("", ""), ModRefInfo::ModRef);
        assert_eq!(arg("readonly", ""), ModRefInfo::Ref);
        assert_eq!(arg("", "writeonly"), ModRefInfo::Mod);
        assert_eq!(arg("readonly", "readnone"), ModRefInfo::NoModRef);
    }

    /// Distinct allocas are distinct objects.
    #[test]
    fn two_allocas_are_no_alias_and_one_pointer_is_must_alias() {
        let body = "define i32 @f() {\nentry:\n  %a = alloca i32\n  %b = alloca i32\n  ret i32 0\n}\n";
        assert_eq!(answer(body, "a", "b", 4), AliasResult::NoAlias);
        assert_eq!(answer(body, "a", "a", 4), AliasResult::MustAlias);
    }

    /// `getModRefInfoMask` of `%a` in `@f`.
    fn mask(
        body: &str,
        ignore_locals: bool,
    ) -> ModRefInfo {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let location = MemoryLocation::new(Operand::Value(value(function, "a")), LocationSize::Precise(4));
        BasicAA::new(&unit).get_mod_ref_info_mask(&location, &mut AAQueryInfo::default(), ignore_locals)
    }

    /// A pointer into a constant global is never written, one into a mutable
    /// global may be, a `noalias readonly` argument is read only (the
    /// mask answered ModRef for all of them before).
    #[test]
    fn the_mask_of_a_constant_global_is_none_and_of_a_mutable_one_is_everything() {
        let body = |kind: &str| {
            format!(
                "@g = internal {kind} [8 x i8] zeroinitializer\ndefine void @f() {{\nentry:\n  %a = getelementptr i8, ptr @g, i32 4\n  ret void\n}}\n"
            )
        };
        assert_eq!(mask(&body("constant"), false), ModRefInfo::NoModRef);
        assert_eq!(mask(&body("global"), false), ModRefInfo::ModRef);
        let argument = "define void @f(ptr noalias readonly %a) {\nentry:\n  ret void\n}\n";
        assert_eq!(mask(argument, false), ModRefInfo::Ref);
    }

    /// An alloca is skipped when locals are ignored and anything else when
    /// not: a select of a constant global and an alloca is NoModRef only with
    /// locals ignored.
    #[test]
    fn the_mask_skips_allocas_only_when_asked_and_follows_a_select() {
        let body = "@g = internal constant i32 0\ndefine void @f(i1 %c) {\nentry:\n  %l = alloca i32\n  %a = select i1 %c, ptr @g, ptr %l\n  ret void\n}\n";
        assert_eq!(mask(body, true), ModRefInfo::NoModRef);
        assert_eq!(mask(body, false), ModRefInfo::ModRef);
    }

    /// What the call in `@f` does to `%a`, the one call in the body.
    fn call_on(body: &str) -> ModRefInfo {
        let module = parsed(&format!("{M32}{body}"));
        let layout = layout(&module);
        let function = crate::testing::function(&module, "f");
        let unit = Unit::of(&module, &layout, function);
        let call = function
            .walk()
            .map(|(_, inst)| inst)
            .find(|inst| matches!(function.instruction(*inst).opcode, Opcode::Call(_)))
            .expect("a call");
        let location = MemoryLocation::new(Operand::Value(value(function, "a")), LocationSize::Precise(4));
        BasicAA::new(&unit).get_mod_ref_info(call, Some(&location), &mut AAQueryInfo::default())
    }

    /// A call touches a local slot nobody holds a pointer to only if the slot
    /// is passed; the call answered ModRef for both (the mod/ref methods
    /// were placeholders).
    #[test]
    fn a_call_cannot_touch_a_slot_that_never_escapes_unless_it_is_passed() {
        let body = |pass: &str| {
            format!(
                "declare void @h(ptr)\ndeclare void @g()\ndefine void @f() {{\nentry:\n  %a = alloca i32\n  call void {pass}\n  ret void\n}}\n"
            )
        };
        assert_eq!(call_on(&body("@g()")), ModRefInfo::NoModRef);
        assert_eq!(call_on(&body("@h(ptr %a)")), ModRefInfo::ModRef);
    }

    /// A `tail` call cannot reach the caller's slots, unless it takes one
    /// `byval`.
    #[test]
    fn a_tail_call_cannot_touch_a_slot_of_the_frame_unless_it_takes_one_by_value() {
        let body = |arg: &str| {
            format!(
                "declare void @h(ptr)\ndefine void @f() {{\nentry:\n  %a = alloca i32\n  tail call void @h(ptr {arg} %a)\n  ret void\n}}\n"
            )
        };
        assert_eq!(call_on(&body("")), ModRefInfo::NoModRef);
        assert_eq!(call_on(&body("byval(i32)")), ModRefInfo::ModRef);
    }

    /// A call that only reads through its argument reads the slot it is
    /// given, and touches no other (`getModRefInfo(Call, Loc)`'s argument
    /// refinement); with the effects unknown it was ModRef for both.
    #[test]
    fn a_call_that_reads_through_its_argument_only_reads_what_it_is_given() {
        let body = |pass: &str| {
            format!(
                "declare void @r(ptr) memory(argmem: read) nounwind\n@g = external global i32\ndefine void @f(ptr %q) {{\nentry:\n  %a = alloca i32\n  store ptr %a, ptr @g\n  call void @r(ptr {pass})\n  ret void\n}}\n"
            )
        };
        assert_eq!(call_on(&body("%a")), ModRefInfo::Ref);
        assert_eq!(call_on(&body("%q")), ModRefInfo::NoModRef);
    }
}
