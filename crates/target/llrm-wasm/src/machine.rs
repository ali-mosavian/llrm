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
//! first vector the type legalizer takes (undated: after 32-bit parity).
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

    fn lowering(&self) -> Option<Rc<dyn llrm_mir::lowering::Lowering>> {
        Some(Rc::new(crate::ir_lowering::WasmIrLowering))
    }

    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    /// The code bytes of one instruction of each class, as assembled
    /// (`tests/size_costs.rs` reads them back with `wasm-objdump`): a memory
    /// access is the opcode, the alignment and an offset of one LEB byte; a
    /// `br_if` the opcode and its depth; a call the opcode and the padded
    /// five-byte function index of a relocatable object. The ALU classes and
    /// the float operations are one opcode byte, the default's 1. Not
    /// priced here (the default stands): the `local.get`/`local.set` traffic,
    /// the constants, and the `block`/`end` of a branch.
    fn size_costs(&self) -> OperationCosts {
        OperationCosts { load: 3, store: 3, float_load: 3, float_store: 3, branch: 2, call: 6, ..self.costs() }
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
        TUNING
    }

    /// SD443: the pin's IPSCCP runs once; a second run specialised the
    /// arguments of a function whose address was taken before inlining.
    fn second_ipsccp(&self) -> bool {
        false
    }
}

/// `TargetLoweringBase`'s defaults, which WebAssemblyISelLowering does not
/// override: 8/4 stores for a memset, memcpy and memmove. The DAG builder
/// takes the same table.
pub const TUNING: Tuning = Tuning {
    max_stores_per_memset: 8,
    max_stores_per_memset_opt_size: 4,
    max_stores_per_memcpy: 8,
    max_stores_per_memcpy_opt_size: 4,
    max_stores_per_memmove: 8,
    max_stores_per_memmove_opt_size: 4,
    ..Tuning::PIN
};

/// The wasm `TargetTransformInfo`: the base class's answers but the address
/// rule and the unrolling preferences.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wasm;

/// The width of a pointer: wasm32.
const POINTER_BITS: u32 = 32;

/// `BasicTTIImplBase::getTypeLegalizationCost`: how many registers a value of
/// `ty` takes and each one's size in bits. wasm has `i32`, `i64`, `f32` and
/// `f64`: a narrower integer is promoted, a wider one split in halves.
fn legalized(ty: Ty) -> (u32, u32) {
    match ty {
        Ty::Int(bits) if bits <= 32 => (1, 32),
        Ty::Int(bits) if bits <= 64 => (1, 64),
        Ty::Int(bits) => (bits.div_ceil(64).next_power_of_two(), 64),
        Ty::Pointer | Ty::Float => (1, 32),
        Ty::Double => (1, 64),
        Ty::X87 => (1, 80),
    }
}

impl Tti for Wasm {
    /// `FSQRT` is legal on `f32` and `f64`.
    fn have_fast_sqrt(
        &self,
        ty: llrm_mir::tti::Ty,
    ) -> bool {
        matches!(ty, llrm_mir::tti::Ty::Float | llrm_mir::tti::Ty::Double)
    }

    fn name(&self) -> &'static str {
        "WebAssembly"
    }

    /// Wasm locals are virtual: the engine allocates the registers, and the
    /// pin's LICM promotes without a cap (bench nbody_fixed -O2).
    fn licm_register_limit(&self) -> Option<u32> {
        None
    }

    /// DEVIATE LG3: a tie of instructions plus setup keeps the pin's solution:
    /// the base and counter constants the end-biased form sets up are bytes the
    /// saved compare immediate does not repay (quicksort -O2, +10 B).
    fn lsr_pin_wins_setup_tie(&self) -> bool {
        true
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

    /// `BasicTTIImplBase::isLegalAddImmediate` forwards to the lowering, whose
    /// default (WebAssembly does not override it) accepts every immediate.
    fn is_legal_add_immediate(
        &self,
        _imm: i64,
    ) -> bool {
        true
    }

    /// `BasicTTIImplBase::isLegalICmpImmediate`: as above.
    fn is_legal_icmp_immediate(
        &self,
        _imm: i64,
    ) -> bool {
        true
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

    /// `TargetTransformInfoImplBase::getCastInstrCost` and the start of
    /// `BasicTTIImplBase::getCastInstrCost`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/CodeGen/BasicTTIImpl.h#L1257>):
    /// a pointer cast to or from a legal integer of its width, and a `trunc`
    /// or `bitcast` between types that legalize to one register of one size
    /// (`i32` to `i8`: both an `i32`), are free. `Basic` charged each one.
    fn cast_cost(
        &self,
        op: CastKind,
        to: Ty,
        from: Ty,
        kind: CostKind,
    ) -> Cost {
        let legal_integer = |ty: Ty| matches!(ty, Ty::Int(32 | 64));
        let width = |ty: Ty| match ty {
            Ty::Int(bits) => bits,
            _ => 32,
        };
        let free = match op {
            CastKind::IntToPtr => legal_integer(from) && width(from) <= POINTER_BITS,
            CastKind::PtrToInt => legal_integer(to) && width(to) >= POINTER_BITS,
            CastKind::Trunc | CastKind::BitCast => {
                let integral = |ty: Ty| matches!(ty, Ty::Int(_) | Ty::Pointer);
                let (from_count, from_size) = legalized(from);
                let (to_count, to_size) = legalized(to);
                from_count == to_count && from_size == to_size && integral(from) == integral(to)
            }
            _ => false,
        };
        if free { Cost::FREE } else { Basic.cast_cost(op, to, from, kind) }
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

    /// `BasicTTIImplBase::getArithmeticInstrCost` answers only the throughput
    /// kind itself; every other kind is `TargetTransformInfoImplBase`'s: a
    /// division or remainder is `TCC_Expensive`, a floating point operation
    /// costs 3 for latency, the rest 1.
    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        left: OperandInfo,
        right: OperandInfo,
    ) -> Cost {
        if kind == CostKind::RecipThroughput {
            return Basic.arithmetic_cost(op, ty, kind, left, right);
        }
        match op {
            ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem | ArithOp::FDiv | ArithOp::FRem => {
                Cost::EXPENSIVE
            }
            _ if kind == CostKind::Latency && matches!(ty, Ty::Float | Ty::Double) => Cost::of(3),
            _ => Cost::BASIC,
        }
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
