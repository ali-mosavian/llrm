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
use llrm_mir::tti::{ArithOp, Basic, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Tti, Ty};

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
}
