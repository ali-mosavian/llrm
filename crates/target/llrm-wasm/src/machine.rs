//! What the shared mid-end asks of wasm: [`Machine`] and the [`Tti`] its
//! loop strength reduction, unroller and inliner price with.
//!
//! The pin's `WebAssemblyTTIImpl` is `BasicTTIImplBase` with a few answers;
//! the scalar ones are here. `WebAssemblyTargetLowering` gives the address
//! rule and the memset, memcpy and memmove store limits (the base class's 8,
//! 4 at -Os).
//!
//! DEVIATE W22: `vector_register_bits` is 0, where the pin's
//! `getRegisterBitWidth(FixedWidthVector)` is 64 without `simd128`; the
//! backend selects no vector yet, so the SLP vectorizer stays off. Exit: the
//! first vector the type legalizer takes.
//!
//! # See also
//! `llvm/lib/Target/WebAssembly/WebAssemblyTargetTransformInfo.cpp` and
//! `WebAssemblyISelLowering.cpp` (`isLegalAddressingMode`), pin d1106deb71cc.

use std::rc::Rc;

use llrm_mir::spaces::Spaces;
use llrm_mir::target::{AddressForm, Machine, OperationCosts, Tuning};
use llrm_mir::tti::{
    ArithOp, Basic, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Tti, Ty, base_intrinsic_cost,
    is_free_intrinsic,
};

/// The pin's price of each scalar intrinsic by cost kind, `python3
/// tti_intrinsics_oracle.py`: lines `kind name cost`, sorted by byte.
const INTRINSIC_PRICES: &str = include_str!("tti_intrinsics.txt");

/// The price of `name` under `kind`; none when the table has no row.
fn intrinsic_price(
    kind: &str,
    name: &str,
) -> Option<i64> {
    let key = || kind.bytes().chain([b' ']).chain(name.bytes()).chain([b' ']);
    let key_len = kind.len() + name.len() + 2;
    let (mut low, mut high) = (0, INTRINSIC_PRICES.len());
    while low < high {
        let middle = low + (high - low) / 2;
        let start = INTRINSIC_PRICES[low..middle].rfind('\n').map_or(low, |at| low + at + 1);
        let end = INTRINSIC_PRICES[start..].find('\n').map_or(INTRINSIC_PRICES.len(), |at| start + at);
        let line = &INTRINSIC_PRICES[start..end];
        match line.bytes().take(key_len).cmp(key()) {
            std::cmp::Ordering::Less => low = end + 1,
            std::cmp::Ordering::Greater => high = start,
            std::cmp::Ordering::Equal => return line[key_len..].trim_end().parse().ok(),
        }
    }
    None
}

/// The wasm cost model the passes of the mid-end read.
#[derive(Clone, Debug, Default)]
pub struct WasmMachine;

/// `getNumberOfRegisters` of the scalar class: the base class's 8.
const REGISTERS: i64 = 8;

impl Machine for WasmMachine {
    fn foreign_span(
        &self,
        _selectors: (i64, i64),
        _offsets: (i64, i64),
        _width: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn tti(&self) -> Option<Rc<dyn Tti>> {
        Some(Rc::new(Wasm))
    }

    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    fn registers(&self) -> i64 {
        REGISTERS
    }

    /// Locals are the callee's own: a call keeps them all.
    fn call_registers(&self) -> i64 {
        REGISTERS
    }

    /// A base and an unsigned offset: no index register.
    fn address_forms(&self) -> Vec<AddressForm> {
        Vec::new()
    }

    fn tuning(&self) -> Tuning {
        Tuning {
            max_stores_per_memset: 8,
            max_stores_per_memset_opt_size: 4,
            max_stores_per_memcpy: 8,
            max_stores_per_memcpy_opt_size: 4,
            max_stores_per_memmove: 8,
            max_stores_per_memmove_opt_size: 4,
            ..Tuning::PIN
        }
    }
}

/// The wasm `TargetTransformInfo`: the base class's answers but the address
/// rule and the unrolling preferences.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wasm;

impl Tti for Wasm {
    fn name(&self) -> &'static str {
        "WebAssembly"
    }

    /// `WebAssemblyTargetLowering::isLegalAddressingMode`: offsets add as
    /// unsigned without wrapping, so a negative one is refused; no register is
    /// scaled; every other form is legal.
    fn is_legal_addressing_mode(
        &self,
        _base_gv: bool,
        base_offset: i64,
        _has_base_reg: bool,
        scale: i64,
    ) -> bool {
        base_offset >= 0 && scale == 0
    }

    /// `WebAssemblyTTIImpl::getUnrollingPreferences`: partial and runtime
    /// unrolling to 30, unless the loop calls something lowered to a call.
    fn get_unrolling_preferences(
        &self,
        has_calls: bool,
        up: &mut llrm_mir::tti::UnrollingPreferences,
    ) {
        if has_calls {
            return;
        }
        up.partial = true;
        up.runtime = true;
        up.upper_bound = true;
        up.partial_threshold = 30;
        up.opt_size_threshold = 0;
        up.partial_opt_size_threshold = 0;
        up.be_insns = 2;
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        Basic.cmp_sel_cost(op, ty, kind)
    }

    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        indexed: bool,
    ) -> Cost {
        Basic.memory_op_cost(op, ty, kind, indexed)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        Basic.cf_instr_cost(phi, kind)
    }

    fn int_imm_cost(
        &self,
        value: i128,
        bits: u32,
    ) -> Cost {
        Basic.int_imm_cost(value, bits)
    }

    fn int_imm_cost_inst(
        &self,
        user: ImmUser,
        operand: u32,
        value: i128,
        bits: u32,
    ) -> Cost {
        Basic.int_imm_cost_inst(user, operand, value, bits)
    }

    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        Basic.is_truncate_free(from, to)
    }

    fn cast_cost(
        &self,
        op: CastKind,
        to: Ty,
        from: Ty,
        kind: CostKind,
    ) -> Cost {
        Basic.cast_cost(op, to, from, kind)
    }

    /// A `sext` or `zext` of a load is free when the extending load is legal
    /// (`BasicTTIImplBase::getCastInstrCost`, `isLoadExtLegal`): wasm has
    /// `i32.load8_u`, `i64.load32_s` and the rest, for every integer pair
    /// but from `i1` (`Promote`). `Basic` charged each one.
    fn cast_of_load_cost(
        &self,
        op: CastKind,
        to: Ty,
        from: Ty,
        _kind: CostKind,
    ) -> Option<Cost> {
        let legal = matches!(op, CastKind::SExt | CastKind::ZExt)
            && matches!(
                (from, to),
                (Ty::Int(8 | 16), Ty::Int(32 | 64)) | (Ty::Int(32), Ty::Int(64))
            );
        legal.then_some(Cost::FREE)
    }

    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        left: OperandInfo,
        right: OperandInfo,
    ) -> Cost {
        Basic.arithmetic_cost(op, ty, kind, left, right)
    }

    /// `BasicTTIImplBase::getIntrinsicInstrCost`: the pin's `opt` priced each
    /// scalar intrinsic (`fmuladd` is an `fmul` and an `fadd`). One it has no
    /// row for keeps the base answer.
    fn intrinsic_cost(
        &self,
        name: &str,
        _ret: Option<Ty>,
        _args: &[Ty],
        kind: CostKind,
    ) -> Cost {
        let kind = match kind {
            CostKind::RecipThroughput => "throughput",
            CostKind::Latency => "latency",
            CostKind::CodeSize => "code-size",
            CostKind::SizeAndLatency => "size-latency",
        };
        match intrinsic_price(kind, name) {
            Some(cost) => Cost::of(cost),
            None if is_free_intrinsic(name) => Cost::FREE,
            None => base_intrinsic_cost(name).unwrap_or(Cost::INVALID),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `zext` of a loaded byte cost one unit, where the pin prices it at none
    /// (`i32.load8_u` is legal): crc's rolled loop read 387 against the pin's
    /// 378, which with a shift merged early tipped `-O3` into unrolling it.
    #[test]
    fn an_extension_of_a_load_the_target_has_is_free() {
        let cost = |op, to, from| Wasm.cast_of_load_cost(op, to, from, CostKind::SizeAndLatency);
        assert_eq!(cost(CastKind::ZExt, Ty::Int(32), Ty::Int(8)), Some(Cost::FREE));
        assert_eq!(cost(CastKind::SExt, Ty::Int(64), Ty::Int(32)), Some(Cost::FREE));
        assert_eq!(cost(CastKind::ZExt, Ty::Int(32), Ty::Int(1)), None);
        assert_eq!(cost(CastKind::Trunc, Ty::Int(8), Ty::Int(32)), None);
    }

    /// `llvm.fmuladd` had no price, so a loop that held one could not be
    /// unrolled (floats -O2: 23,042 instructions against the pin's 16,542).
    /// The pin prices it as an `fmul` and an `fadd`.
    #[test]
    fn an_fmuladd_is_priced_as_a_multiply_and_an_add() {
        let cost = |name, kind| Wasm.intrinsic_cost(name, None, &[], kind);
        assert_eq!(cost("llvm.fmuladd.f64", CostKind::CodeSize), Cost::of(2));
        assert_eq!(cost("llvm.fmuladd.f32", CostKind::RecipThroughput), Cost::of(4));
        assert_eq!(cost("llvm.lifetime.start.p0", CostKind::CodeSize), Cost::FREE);
    }
}
