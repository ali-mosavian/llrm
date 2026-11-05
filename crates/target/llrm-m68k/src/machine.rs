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
#[derive(Clone, Copy, Debug, Default)]
pub struct M68kMachine {
    /// The processor the code is for: a fact of the target description, read
    /// by the cost model through [`M68k::cpu`].
    pub cpu: crate::dagisel::hooks::Cpu,
}

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
        Some(Rc::new(M68k::for_cpu(self.cpu)))
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
    /// The processor the code is for; the 68000 where none is given.
    pub cpu: crate::dagisel::hooks::Cpu,
    /// The 68020's scaled index, `(d8, An, Xn*2/4/8)`; the 68000 has none.
    pub scaled_index: bool,
}

impl M68k {
    /// The cost model of `cpu`.
    pub fn for_cpu(cpu: crate::dagisel::hooks::Cpu) -> M68k {
        M68k { cpu, ..M68k::default() }
    }
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

    /// The per-target override of `SCEVCheapExpansionBudget`: a limit that
    /// costs any operation (`smax(n, 1)` for `i + 1 < n`) is not worth
    /// replacing a compare with in a function built for size; the guard and
    /// the second counter cost more bytes than the compare saved.
    fn lftr_expansion_budget(
        &self,
        optsize: bool,
    ) -> u32 {
        // DEVIATE 18 (exit 2026-11-12): from the 68020 on, a `cmp.l` with a
        // memory limit costs little more than the `subq` of a counter, so the
        // `smax` guard of the rewritten exit (8 B) is not repaid. The 68000
        // keeps the pin's 4: there the compare is 10 clk per trip dearer.
        if optsize || self.cpu.at_least(crate::dagisel::hooks::Cpu::M68020) { 0 } else { 4 }
    }

    /// DEVIATE: the pin's M68k states no preference; `(An)+` steps a pointer
    /// by the access size for free (descriptors in `indexed.rs`), so LSR keeps
    /// a pointer walk rather than an index beside a counter. Exit: 2026-11-20,
    /// when LSR rates the indexed formula beside the walk (M68K-COPY-OS).
    fn preferred_addressing_mode(&self) -> AddressingMode {
        AddressingMode::PostIndexed
    }

    fn is_post_inc_load_legal(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Int(8 | 16 | 32) | Ty::Pointer)
    }

    fn is_post_inc_store_legal(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Int(8 | 16 | 32) | Ty::Pointer)
    }

    /// `(An)+` steps by the access size: a walk by 1, 2 or 4 bytes, upward.
    fn is_post_inc_step_legal(
        &self,
        access_bytes: i64,
        step: i64,
    ) -> bool {
        step == access_bytes
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

    /// DEVIATE 20: each select a two-entry phi would become costs the branch
    /// and move it lowers to, in bytes as in clocks (a fold saves one branch
    /// and adds a move per select). gcd -O2 44 -> 26 B, 236 -> 138 clk; -Os
    /// 44 -> 26 B. Exit 2026-11-12.
    fn select_branch_extra(
        &self,
        _optsize: bool,
    ) -> i64 {
        1
    }

    fn sizes_exit_values(&self) -> bool {
        true
    }

    /// DEVIATE 19: `addi`/`subi`/`addq` and `cmpi` take any 32-bit immediate;
    /// the pin's M68k lowering keeps the base class's `false`. Exit
    /// 2026-11-12.
    fn is_legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    fn is_legal_icmp_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// DEVIATE 17: a size build keeps a store or copy loop (a `memset` call
    /// is three pushes, `jsr` and `lea`: 16 B, the loop 10 to 14). Exit
    /// 2026-11-12.
    fn memory_idioms_at_optsize(&self) -> bool {
        false
    }

    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        left: OperandInfo,
        right: OperandInfo,
    ) -> Cost {
        // DEVIATE: the pin's base class prices a multiply or divide it expands
        // at one operation (`BasicTTIImpl::getArithmeticInstrCost`'s last
        // line). Past 32 bits the m68k makes a library call of it, and the
        // closed form of a sum (`(n-1)(n-2)/2` in `i33`) is then 250 B of
        // `__mulsi3` calls for a loop of 16. Exit: none, a target's prices.
        if matches!(op, ArithOp::Mul | ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem)
            && matches!(ty, Ty::Int(bits) if bits > 32)
        {
            return Cost::EXPENSIVE;
        }
        Basic.arithmetic_cost(op, ty, kind, left, right)
    }
}
