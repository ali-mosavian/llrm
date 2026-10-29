//! What the shared mid-end asks of m68k: [`Machine`] and the
//! [`Tti`] its loop strength reduction prices addresses with.
//!
//! The pin's M68k overrides no `TargetTransformInfo` hook and no
//! `isLegalAddressingMode`, so it takes `BasicTTIImpl`'s defaults over
//! `TargetLoweringBase::isLegalAddressingMode` (a 16-bit offset, no global
//! base, `r+r` or `r+i` but not `r+r+i`, `2*r` only alone); this file states
//! those answers.
//!
//! # See also
//! `llvm/lib/CodeGen/TargetLoweringBase.cpp` (`isLegalAddressingMode`),
//! pin d1106deb71cc.

use std::collections::BTreeSet;
use std::rc::Rc;

use llrm_mir::spaces::Spaces;
use llrm_mir::target::{AddressForm, Machine, OperationCosts};
use llrm_mir::tti::{ArithOp, Basic, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Tti, Ty};

/// The m68k cost model the passes of the mid-end read.
#[derive(Clone, Debug, Default)]
pub struct M68kMachine;

impl Machine for M68kMachine {
    fn foreign_span(
        &self,
        _selectors: (i64, i64),
        _offsets: (i64, i64),
        _width: i64,
    ) -> Option<(i64, i64)> {
        None
    }

    fn tti(&self) -> Option<Rc<dyn Tti>> {
        Some(Rc::new(M68k::default()))
    }

    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    /// `d0`..`d7`, `a0`..`a5`.
    fn registers(&self) -> i64 {
        14
    }

    /// `d2`..`d7`, `a2`..`a5`.
    fn call_registers(&self) -> i64 {
        10
    }

    /// Every arithmetic instruction names its destination as a source.
    fn two_address(&self) -> bool {
        true
    }

    /// `(d8, An, Xn)`: a base and an index of the same size, unscaled (the
    /// 68020's scale is a later subtarget).
    fn address_forms(&self) -> Vec<AddressForm> {
        let scales: BTreeSet<i64> = [1].into_iter().collect();
        vec![AddressForm::new(4, scales, 0, 0, 0, false, None).expect("a well-formed address form")]
    }
}

/// The m68k `TargetTransformInfo`: the base class's answers.
#[derive(Clone, Copy, Debug, Default)]
pub struct M68k {
    /// The 68020's scaled index, `(d8, An, Xn*2/4/8)`; the 68000 has none.
    pub scaled_index: bool,
}

impl Tti for M68k {
    fn name(&self) -> &'static str {
        "M68k"
    }

    /// `TargetLoweringBase::isLegalAddressingMode`.
    fn is_legal_addressing_mode(
        &self,
        base_gv: bool,
        base_offset: i64,
        has_base_reg: bool,
        scale: i64,
    ) -> bool {
        if base_offset <= -(1 << 16) || base_offset >= (1 << 16) - 1 || base_gv {
            return false;
        }
        match scale {
            0 => true,
            1 => !(has_base_reg && base_offset != 0),
            2 if !self.scaled_index => !(has_base_reg || base_offset != 0),
            2 | 4 | 8 => self.scaled_index,
            _ => false,
        }
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
