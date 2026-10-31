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
use llrm_mir::tti::{
    AddressingMode, ArithOp, Basic, CastKind, CmpSel, Cost, CostKind, ImmUser, MemoryOp, OperandInfo, Tti, Ty,
};

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
    /// LSR favors a pointer stepped after each access, `(An)+`.
    ///
    /// DEVIATE from the pin's M68k, which has no `getPreferredAddressingMode`
    /// override. Reason: `move.b (%a0)+, (%a1)+` steps a pointer for free, so
    /// two pointers beat one shared index register. Evidence: the PRM's
    /// "Address Register Indirect with Postincrement Mode"; gcc's `memcpy`
    /// loop. `Options::PINNED` leaves it off. Exit: never; a target
    /// description, set with the overlay.
    pub post_indexed: bool,
    /// A strict exit compare of a unit-stride counter that never passes its
    /// bound is `!=` (`exitne`), which LSR counts to zero.
    ///
    /// DEVIATE from the pin, which has no such rewrite. Reason: `subq #1, Dn;
    /// bne` sets the flags and branches in two instructions where `cmpi #1,
    /// Dn; bhi` needs the compare. Evidence: bench/library memcpy, 44 to 36 B
    /// and 64 to 50 insns. `Options::PINNED` leaves it off. Exit: never; a
    /// target description, set with the overlay.
    pub exit_ne: bool,
}

impl Tti for M68k {
    fn name(&self) -> &'static str {
        "M68k"
    }

    fn prefers_exit_ne(&self) -> bool {
        self.exit_ne
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

    fn preferred_addressing_mode(&self) -> AddressingMode {
        if self.post_indexed { AddressingMode::PostIndexed } else { AddressingMode::None }
    }

    fn is_post_inc_load_legal(
        &self,
        _ty: Ty,
    ) -> bool {
        self.post_indexed
    }

    fn is_post_inc_store_legal(
        &self,
        _ty: Ty,
    ) -> bool {
        self.post_indexed
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
