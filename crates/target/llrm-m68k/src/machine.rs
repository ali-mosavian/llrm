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

    /// Frames keep what the shipped targets ask for (m68k Linux, AmigaOS and
    /// TOS: 4) and nothing realigns them (`frame.rs`): an object asking more
    /// is clamped to it, so the optimizer never folds `ptr & 7` to 0 on a
    /// frame that gives an address ending in 4.
    fn kept_stack_align(&self) -> Option<u64> {
        Some(crate::frame::SHIPPED_STACK_ALIGN as u64)
    }

    /// The datalayout gives `double` an 8-byte alignment and `i64` an 8-byte
    /// preferred one, so locals of those are accepted (and clamped to what the
    /// frame keeps). An object aligned further is kept: the frame realigns
    /// the stack pointer for it (`frame::Layout`).
    fn max_alloca_align(&self) -> Option<u64> {
        None
    }

    fn realigned_above(&self) -> Option<u64> {
        Some(crate::frame::REALIGN_ABOVE as u64)
    }

    fn lowering(&self) -> Option<Rc<dyn llrm_mir::lowering::Lowering>> {
        Some(Rc::new(crate::lowering::M68kLowering))
    }

    fn spaces(&self) -> Spaces {
        Spaces::FLAT
    }

    fn costs(&self) -> OperationCosts {
        OperationCosts::default()
    }

    /// Code bytes per operation at this CPU ([`crate::opcosts`]); clocks are
    /// still one per operation.
    fn size_costs(&self) -> OperationCosts {
        crate::opcosts::size_costs(self.cpu).unwrap_or_else(|why| panic!("m68k operation sizes: {why}"))
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
    ///
    /// DEVIATE M68K-TTI-SCALED (owner m68k, review 2026-12-20): the pin's
    /// `isLegalAddressingMode` is the base class's, which allows `2*r` alone
    /// and no scale of 4 or 8
    /// ([TargetLoweringBase.cpp#L2139-L2171](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/TargetLoweringBase.cpp#L2139-L2171));
    /// the 68020 addresses `(d8, An, Xn*2/4/8)` in one instruction. A hardware
    /// fact, so the exit is the pin giving M68k its own hook.
    pub scaled_index: bool,
    /// `CodeSize` in bytes of code, which the inliner counts a callee with
    /// ([`Tti::inline_size_view`]); the other passes' thresholds are in the
    /// pin's units, where it is false.
    pub code_size_bytes: bool,
}

impl M68k {
    /// The cost model of `cpu`.
    pub fn for_cpu(cpu: crate::dagisel::hooks::Cpu) -> M68k {
        M68k { cpu, ..M68k::default() }
    }
}

impl Tti for M68k {
    /// DEVIATE VASTART-PROMOTE (owner x86-m32, exit 2026-11-27; m68k joins it): `va_list` is `char *`, so `va_start` is a
    /// store of the first variadic argument's address and the list lives in a register, as gcc has it.
    fn promote_va_list(&self) -> bool {
        true
    }

    fn va_arg_slot(&self) -> Option<u32> {
        Some(crate::dagisel::lowering::VA_SLOT)
    }

    /// DEVIATE LFTR1 (owner w12, review 2026-12-20): any constant
    /// non-wrapping stride counts; the pin's step-one rule costs bytes here.
    fn lftr_any_stride(&self) -> bool {
        true
    }

    fn opt_size_inline_byte_gate(&self) -> bool {
        true
    }

    /// `UnrollingPreferences::Threshold` of the full unroll, 32 where the pin's
    /// is 300: an iteration's loop control is about 24 clk on the 68000 (an
    /// `addq`, a `cmp` and a taken `bcc`), 6 B at 4 clk/B, so a copy of the
    /// body pays for itself only when the body is a couple of instructions.
    /// The 400% boost stays, which keeps a hot small loop unrolled (crc16
    /// -O2: 3894 against 5264 clk). 300 seeds -O2 68020: 669352 -> 516950 B,
    /// the 24 programs unchanged. DEVIATE: the pin's target sets none. Exit:
    /// review 2026-11-20.
    fn unroll_threshold_overrides(&self) -> (Option<u32>, Option<u32>) {
        (Some(32), Some(400))
    }

    /// `InlineParams::DefaultThreshold`, 50 where the pin's is 225: a call is
    /// about 20 B (arguments, `jsr`, `addq`, `rts`) and 50 clk, so a copy of a
    /// body pays for itself only up to a dozen instructions; a 30-operation
    /// body at six sites was inlined six times. 300 seeds -O2 68020: 516950
    /// -> 472526 B, the 24 programs unchanged. DEVIATE: the pin's target sets
    /// none. Exit: review 2026-11-20.
    fn inline_threshold_override(&self) -> Option<i64> {
        Some(50)
    }

    fn name(&self) -> &'static str {
        "M68k"
    }

    fn call_sequence_cost(
        &self,
        shape: &llrm_mir::tti::CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        Some(crate::sizes::call_sequence(self.cpu, shape, kind))
    }

    fn callee_frame_cost(
        &self,
        shape: &llrm_mir::tti::CallShape,
        kind: CostKind,
    ) -> Option<Cost> {
        crate::sizes::callee_frame(self.cpu, shape, kind)
    }

    /// A size build refuses an inline of a callee that stays when it grows
    /// the code (SZ1): the callee's bytes against the call sequence's.
    fn opt_size_inline_by_code_size(&self) -> bool {
        true
    }

    /// The inliner counts a callee in bytes of code (`sizes`).
    fn inline_size_view(&self) -> Option<Rc<dyn Tti>> {
        Some(Rc::new(M68k { code_size_bytes: true, ..*self }))
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

    /// The per-target value of `SCEVCheapExpansionBudget`: one operation. A
    /// limit that costs a single operation (`n - 1`) is repaid by the counter
    /// that goes (`subq`+`bne` for `addq`+`cmp`+`blt`); one that costs more
    /// (`smax(n, 1)`) is not. Measured at -Os on the matrix at 0, 1 and the
    /// pin's 4: 1 shrinks partition and memset (4 and 2 B) and changes no
    /// other cell; 4 grows arrmax, arrsum, copy, countgt, fib and sum by 6 to
    /// 14 B. The same value serves every level, so no `optsize` split.
    /// DEVIATE (value; owner m68k, exit none: a per-target table value; the
    /// pin's budget is 4).
    fn lftr_expansion_budget(
        &self,
        _optsize: bool,
    ) -> u32 {
        1
    }

    /// DEVIATE M68K-LSR-FRAME (owner m68k-lsr-addressing, exit none while the
    /// backend folds `(d8,%sp,Xn)`, `frameindexed.rs`): a stack slot is `%sp`
    /// and a displacement, so as the base of an address it takes no register.
    /// The pin counts every `alloca` as one (`llc -debug-only=loop-reduce`,
    /// d1106deb: 7 regs for 4 stack arrays and 3 IVs). Its M68k has no TTI, so
    /// the default registers-first order keeps one pointer IV per array (6
    /// regs); its x86 shares one index (`reg(%a) + 1*reg({0,+,4})`) because
    /// `X86TTIImpl::isLSRCostLess` ranks instructions first (4 against 7,
    /// [X86TargetTransformInfo.cpp#L7202](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86TargetTransformInfo.cpp#L7202)),
    /// not because the slot is free.
    ///
    /// Rows, owner m68k-lsr-addressing, exit 2026-11-30, fix a general
    /// index-versus-pointer cost via Tti (no per-program patch):
    /// - M68K-LSR-IDX-CLOCKS: `(d8,An,Xn)` is 14 clk against 8 for `(An)`.
    ///   O1 cells leaving the bar (clocks, executed): bintree 68000 +1784
    ///   (+0.06%), 68020 +894 (+0.18%), each for -4 B; frameless O1 68000
    ///   +68 (+0.12%), 68020 +10 (+0.05%), each for -2 B. O2/Os frameless
    ///   cost the same clocks.
    /// - M68K-LSR-D2U64-BYTES (owner m68k-lsr-addressing and w12): pin-faithful.
    ///   The pin's LSR picks the single IV with a compare for double_to_uint64's
    ///   loops (`llc -print-after=loop-reduce`); our earlier two-IV
    ///   end-biased form (LG3) was smaller: +10 B at 68000 O1/O2/Os, 68020 O1
    ///   +8, O2 +26, Os +10, executed clocks equal. 68000 -Os: 304 B against
    ///   gcc 330 and clang 386, so below the bar. Closes through the LSR size
    ///   model (the exit compare's `moveq`+`cmp` is in `insns`, which the
    ///   registers-first order ignores).
    fn frame_address_is_free(&self) -> bool {
        true
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
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::cmp_sel(op, ty);
        }
        Basic.cmp_sel_cost(op, ty, kind)
    }

    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        indexed: bool,
    ) -> Cost {
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::memory(op, ty, indexed);
        }
        Basic.memory_op_cost(op, ty, kind, indexed)
    }

    fn cf_instr_cost(
        &self,
        phi: bool,
        kind: CostKind,
    ) -> Cost {
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::control_flow(phi);
        }
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
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::cast(op, to, from);
        }
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
        if kind == CostKind::CodeSize && self.code_size_bytes {
            return crate::sizes::arithmetic(op, ty, right, self.cpu.at_least(crate::dagisel::hooks::Cpu::M68020));
        }
        // DEVIATE: the pin's base class prices a multiply or divide it expands
        // at one operation (`BasicTTIImpl::getArithmeticInstrCost`'s last
        // line). Past 32 bits the m68k makes a library call of it, and the
        // closed form of a sum (`(n-1)(n-2)/2` in `i33`) is then 250 B of
        // `__mulsi3` calls for a loop of 16. Exit: none, a target's prices.
        if matches!(
            op,
            ArithOp::Mul | ArithOp::SDiv | ArithOp::UDiv | ArithOp::SRem | ArithOp::URem
        )
            && matches!(ty, Ty::Int(bits) if bits > 32)
        {
            return Cost::EXPENSIVE;
        }
        Basic.arithmetic_cost(op, ty, kind, left, right)
    }
}

#[cfg(test)]
mod lftr_tests {
    use super::*;
    /// DEVIATE LFTR1: following the pin's step-one rule left byte-loops of
    /// stride 2..8 unreduced and cost bytes; the knob must stay on here.
    #[test]
    fn lftr_counts_any_constant_stride() {
        assert!(M68k::default().lftr_any_stride());
    }
}
