//! `X86TargetLowering`'s custom lowerings of conditions: `setcc`, `select`
//! and `brcond` become `X86ISD::CMP`, `SETCC`, `CMOV` and `BRCOND` on EFLAGS.
//!
//! # How it works
//! A comparison is an `X86ISD::SUB` (or `CMP` against zero) whose second
//! result is the `i32` flags value. `SETCC` reads a condition code (a target
//! constant, the pin's `X86::CondCode`) and the flags; `CMOV` and `BRCOND`
//! read the same. `select` and `brcond` on an `i1`/`i8` that is not a compare
//! test it against zero. A CPU with no CMOV gets the same DAG: the matcher
//! picks the `CMOV_GR*` pseudos and the emitter expands them to a branch
//! (`EmitLoweredSelect`).
//!
//! # Runs
//! As `Hooks::lower_operation` of
//! [`llrm_codegen::backend::selectiondag::legalize_dag::DagLegalizer`] for
//! the nodes the action table makes Custom (integer types only here; floats
//! are B1d). A global address is the static, non-PIC form (`Wrapper` of the
//! target address); the GOT, PIC, DLL-import and TLS forms are not here, and
//! the action table makes it Custom at `i32` only (m16's `i16` is B0e's).
//!
//! DEVIATE SD200: not translated yet, so the nodes are the plain forms: the
//! reassociated and immediate forms of `LowerAndToBT`, the `SUB 0, x` and `XOR`
//! forms of `EmitCmp` (`EmitTest` does reuse an arithmetic result's flags), the
//! `SETCC_CARRY` and `CTTZ`-select folds of `LowerSELECT`, overflow
//! intrinsics, and the 16-bit immediate widening in `EmitCmp`. Reason: each
//! needs known-bits or use queries the DAG lacks. Exit: with the combiner
//! (B2). Owner llrm-target-x86-m64-c. Review 2026-11-24.
//!
//! # See also
//! LLVM's `LowerSETCC`
//! ([L25293](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L25293)),
//! `LowerSELECT`
//! ([L25834](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L25834)),
//! `LowerConditionalBranch`
//! ([L26550](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L26550)),
//! `EmitCmp`
//! ([L24085](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L24085)).

use llrm_codegen::backend::selectiondag::actions::TargetLowering;
use llrm_codegen::backend::selectiondag::dag::{MemBase, MemInfo, NodeData, SDNodeFlags, SDValue, SelectionDAG};
use llrm_codegen::backend::selectiondag::dag_combiner::{CombineHooks, CombineLevel};
use llrm_codegen::backend::selectiondag::isd::{CondCode, ISD};
use llrm_codegen::backend::selectiondag::legalize_dag::Hooks;
use llrm_codegen::backend::selectiondag::mvt::{EVT, MVT};

/// The pin's `X86ISD` numbers, as the matcher table (`llrm-dagisel`) names
/// them.
pub mod x86isd {
    use llrm_codegen::backend::selectiondag::ISD;

    pub const ADC: ISD = ISD::Target(477);
    pub const FSHL: ISD = ISD::Target(625);
    pub const FSHR: ISD = ISD::Target(626);
    pub const SMUL: ISD = ISD::Target(744);
    pub const UMUL: ISD = ISD::Target(776);
    pub const ADD: ISD = ISD::Target(478);
    pub const AND: ISD = ISD::Target(485);
    pub const BRCOND: ISD = ISD::Target(493);
    /// `X86ISD::BT`: the carry flag is bit `n` of `x`.
    pub const BT: ISD = ISD::Target(497);
    /// `X86ISD::SETCC_CARRY`: `-1` if the condition holds in the carry flag
    /// (`sbb r, r`), else `0`.
    pub const SETCC_CARRY: ISD = ISD::Target(740);
    pub const BSF: ISD = ISD::Target(495);
    pub const BSR: ISD = ISD::Target(496);
    pub const CMOV: ISD = ISD::Target(507);
    pub const CMP: ISD = ISD::Target(508);
    /// `X86ISD::MUL_IMM`: a multiply by 3, 5 or 9 that is one `lea`.
    pub const MUL_IMM: ISD = ISD::Target(694);
    /// Real mode's `les`/`lds` (`tools/dagisel/own_ids.txt`): the offset, a
    /// chain; the segment is read back from `ES`/`DS`.
    pub const LES: ISD = ISD::Target(24576);
    pub const LDS: ISD = ISD::Target(24577);
    /// A huge pointer plus an `i32` byte delta (a GEP the builder cannot
    /// prove stays in its segment): `(far, i32) -> far`. Expanded by the
    /// type legalizer, never selected.
    pub const HUGE_ADD: ISD = ISD::Target(24590);
    /// `llrm.ia16.window.p1.p3` as a node: a huge pointer with whole selector
    /// steps of its offset moved into the selector, `far -> far`. Expanded by
    /// the type legalizer, never selected (24578..24580 are the asm
    /// pass-through nodes).
    pub const LLRM_WINDOW: ISD = ISD::Target(24581);
    /// `llrm.ia16.ptrdiff`: the bytes between two carried huge pointers,
    /// `(far, far) -> i32`. Expanded by the type legalizer, never selected.
    pub const LLRM_PTRDIFF: ISD = ISD::Target(24583);
    pub const OR: ISD = ISD::Target(697);
    pub const SETCC: ISD = ISD::Target(739);
    pub const SBB: ISD = ISD::Target(728);
    pub const SUB: ISD = ISD::Target(762);
    pub const WRAPPER: ISD = ISD::Target(953);
    /// `X86ISD::WrapperRIP`: a symbol addressed from `%rip`.
    pub const WRAPPER_RIP: ISD = ISD::Target(954);
    /// `X86ISD::FP_TO_INT_IN_MEM`: `fist` of an x87 value to a stack slot.
    pub const FP_TO_INT_IN_MEM: ISD = ISD::Target(617);
    /// `X86ISD::FILD`: `fild` of an integer in memory to an x87 value.
    pub const FILD: ISD = ISD::Target(586);
    /// `X86ISD::FIST`: `fist` of an x87 value in the current rounding mode.
    pub const FIST: ISD = ISD::Target(587);
    /// `X86ISD::FCMP`: the flags of an x87 compare (`fucom`, `fnstsw`,
    /// `sahf` on a 486).
    pub const FCMP: ISD = ISD::Target(576);
    /// `X86ISD::FSETCC`: `cmpss`/`cmpsd` with an immediate predicate, all
    /// ones or all zeros in the scalar.
    pub const FSETCC: ISD = ISD::Target(622);
    pub const XOR: ISD = ISD::Target(955);
    /// `X86ISD::FAND`/`FOR`/`FXOR`: `andps`/`orps`/`xorps` on a scalar float
    /// and a 16-byte pool mask.
    pub const FAND: ISD = ISD::Target(574);
    pub const FOR: ISD = ISD::Target(615);
    pub const FXOR: ISD = ISD::Target(634);
}

/// `X86::ES`, the segment `les` loads.
pub const ES: u32 = 31;

/// `X86::CondCode`.
pub mod cond {
    pub const O: u128 = 0;
    pub const NO: u128 = 1;
    pub const B: u128 = 2;
    pub const AE: u128 = 3;
    pub const E: u128 = 4;
    pub const NE: u128 = 5;
    pub const BE: u128 = 6;
    pub const A: u128 = 7;
    pub const S: u128 = 8;
    pub const NS: u128 = 9;
    pub const P: u128 = 10;
    pub const NP: u128 = 11;
    pub const L: u128 = 12;
    pub const GE: u128 = 13;
    pub const LE: u128 = 14;
    pub const G: u128 = 15;
}

/// The custom lowerings, for a CPU with or without CMOV.
pub struct X86Lowering<'a> {
    pub cmov: bool,
    /// DEVIATE X87-SHARE ([`combine_shared_product`]): x87 floats only.
    pub share_products: bool,
    /// Scalar floats live in xmm registers (`isScalarFPTypeInSSEReg`).
    pub sse2: bool,
    /// `hasMinSize`: llrm's `-Os` as well as `-Oz`.
    pub min_size: bool,
    /// `-Os` on a target that keeps `imul` only where it is strictly smaller
    /// than the pin's chain (DEVIATE MUL-OS): `min_size` then holds for the
    /// other rules, not for `combineMul`.
    pub imul_if_smaller: bool,
    /// DEVIATE MUL-HOT (owner x86-m32, exit 2026-11-24): the block runs more than once per run of the program (its
    /// frequency over the function's entry times how often its direct callers run it) at
    /// `-Os`, so `combineMul` decomposes as the pin does at `-Os` (`hasMinSize` is `-Oz`'s): the
    /// chain is bytes the module pays once and clocks it saves on every run.
    pub hot_chains: bool,
    /// The target adds three-address (`lea`) and the build is for speed: flat
    /// 32-bit and 64-bit, not `-Os`. DEVIATE SD448
    /// ([`combine_masked_xor_difference`]) is made there only.
    pub three_address_adds: bool,
    /// The pin chains an `i16` multiply by a constant: it promotes it to `i32`
    /// ([L64277][promote]) and `combineMul` builds the `lea`/shift chain there.
    /// Where `i16` is not promoted (m16, `is_desirable_to_promote_op`) the same
    /// chain is built at `i16`, without the promotion's `66h`/`67h` prefixes.
    /// Consequence of DEVIATE SD441, which holds the exit.
    ///
    /// [promote]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L64277
    pub i16_mul_chains: bool,
    /// `visitSIGN_EXTEND_INREG`'s `(sext_inreg (extload x))` is `(sextload x)`
    /// (`movsx r, [m]`, not a load, `movsx`, `movsxd`). On for the x86 targets
    /// (the 2026-11-03 A/B at m16, m32 and m64 has no larger cell from it).
    pub sext_load: bool,
    /// `X86TargetLowering::isZExtFree(i32, i64)`: `Subtarget.is64Bit()`.
    pub zext_free: bool,
    /// DEVIATE SELB: a select of two constants without a `cmov` stays a select
    /// (a branch), not `setcc` arithmetic.
    pub branch_constant_selects: bool,
    /// What prices a constant multiply that `combineMul` leaves an `imul`
    /// (DEVIATE SD446); none keeps the pin's `imul`.
    pub profile: Option<MulPricing<'a>>,
    /// The target's `mul` reads the length of its r/m operand (the 486's
    /// early-out): `Some(flat)`, `flat` for the 32- and 64-bit address modes.
    /// DEVIATE SD444 ([`CombineHooks::multiply_high_magic_first`]).
    pub multiply_early_out: Option<bool>,
}

/// The sheet, the target and the block a constant multiply is priced against.
#[derive(Clone, Copy)]
pub struct MulPricing<'a> {
    pub cpu: &'a llrm_codegen::backend::cpu::Profile,
    pub arch: &'a dyn llrm_target::Target,
    /// The block's frequency over the entry's.
    pub freq: f64,
    /// `Tti::speed_milliclocks_per_byte`.
    pub milliclocks_per_byte: i64,
    /// `Tuning::multiply_by_clocks`: the fewest clocks, no byte term.
    pub by_clocks: bool,
    /// `Tuning::price_pin_multiplies`: the pin's decompositions are priced
    /// too, and a tie keeps the `imul`.
    pub price_pin: bool,
}

impl X86Lowering<'_> {
    /// SELB applies: no `cmov` to select between the constants with.
    fn branch_constants(&self) -> bool {
        self.branch_constant_selects && !self.cmov
    }
}

impl CombineHooks for X86Lowering<'_> {
    /// `X86TargetLowering::shouldNormalizeToSelectSequence` is true unless CCMP
    /// (APX) keeps the `and`/`or` tree.
    ///
    /// DEVIATE SQ1 (owner greedy-subreg, exit 2026-11-21): only where there is
    /// a `cmov`. Without one the split is two branches where the merged
    /// condition was one `sete`/`or`/branch; the pin's `EmitLoweredSelect`
    /// shares the compare of back-to-back selects, ours does not: +16 B at m32
    /// -O2 (sfm16_*), +4 at m16. Exit: the select expansion shares the flags.
    fn normalize_to_select_sequence(
        &self,
        _vt: EVT,
        _cond_vt: EVT,
    ) -> bool {
        self.cmov
    }

    /// DEVIATE: off at m16 (`i16_mul_chains`), see the trait.
    fn select_cc_to_shift_and(&self) -> bool {
        !self.i16_mul_chains
    }

    /// `convertSelectOfConstantsToMath`: false where SELB keeps the select.
    fn convert_select_of_constants_to_math(
        &self,
        _vt: EVT,
    ) -> bool {
        !self.branch_constants()
    }

    /// `X86TargetLowering::mergeStoresAfterLegalization`: `!MemVT.isVector()`.
    /// The only vector a store here holds is the opaque 16 bytes of an SSE
    /// move (`I128`).
    fn merge_stores_after_legalization(
        &self,
        mem: EVT,
    ) -> bool {
        mem != EVT::Simple(MVT::I128)
    }

    /// `foldBinOpIntoSelect`, `visitXOR`'s `(xor (and x, y), y)` and
    /// `SimplifySetCC`'s GE/LE canonicalization: the pin runs them on x86.
    /// Judged on `bintree`, `jpeg_*` and `particle` (m16 and C,
    /// `-Os`/`-O2`): fewer bytes or clocks, none worse. The
    /// other default-off combines either changed nothing there or cost a byte
    /// (`rebuild_brcond_condition`) and stay off.
    /// DEVIATE: the remaining pin combines stay off for x86. Exit 2026-12-01.
    fn fold_bin_op_into_select(&self) -> bool {
        true
    }

    /// `X86TargetLowering::isLegalAddImmediate`: `isInt<32>`. Without it
    /// `add(mul(add(A, CA), CM), CB)` folded the LCG chain of mul_constants
    /// into 64-bit constants (+875 B at -O2/-O3).
    fn legal_add_immediate(
        &self,
        imm: i64,
    ) -> bool {
        i32::try_from(imm).is_ok()
    }

    /// `visitMUL`'s `(mul (add x, c1), c2)` is `(add (mul x, c2), c1*c2)` when
    /// the other multiply of `x` by `c2` is in the DAG: the pin's x86 runs
    /// it (`isMulAddWithConstProfitable` has no X86 override). Judged on
    /// the whole corpus: 16 cells smaller (-1795 B), m16 -Os lru +1 B (a
    /// 6-byte real-mode `lea`). DEVIATE: off at m16 outside `-Os`: the
    /// constant leaves a 3-part `lea` for a trailing `add`,
    /// regparm3_matrix_4/_22 -40..-62 B but +62..+84 instructions,
    /// +38 and +32 clk-eq at 1 clk = 2 B (the pin splits too). Exit 2026-12-01.
    fn fold_mul_of_add_with_constant(&self) -> bool {
        !self.i16_mul_chains || self.min_size
    }

    /// `replaceStoreOfFPConstant`: a `double` constant is stored as an `i64`
    /// (m64) or two `i32` (m32). The pin runs it on x86 (`isFPImmLegal` keeps
    /// only +-0.0, +-1.0). Judged by all allocated bytes: m64 smaller in 9 of
    /// 9 cells (the pool entry goes).
    fn store_fp_constant_as_i64(&self) -> bool {
        true
    }

    /// `visitORCommutative`'s `(or (and x, y), x)` is `x`: the pin runs it on
    /// x86. No corpus cell reaches it (the mid-end absorbs first).
    fn fold_or_absorption(&self) -> bool {
        true
    }

    /// `reduceLoadWidth` through a `freeze` between the mask and the load: the
    /// pin runs it on x86. No corpus cell reaches it.
    fn reduce_load_width_through_freeze(&self) -> bool {
        true
    }

    /// `visitBRCOND`'s `rebuildSetCC`: `!bad` on an `i1` global is `cmp byte
    /// [bad], 1; jne`, not `mov al; not al; test al, 1; jne` (jpeg_fixed
    /// `-Os` m64, 2 sites, 8 B). The m16 judging cost a byte, so m16 stays off;
    /// m32 asks too (x_hash `-Os`: `not dl; test dl, 1; je`, +5 B on the pin).
    /// DEVIATE: off at m16. Exit 2026-12-01.
    fn rebuild_brcond_condition(&self) -> bool {
        self.zext_free || !self.i16_mul_chains
    }

    /// `visitAND`'s `(and (extload i1), 1)` is a `zextload`, so the branch
    /// above compares the loaded byte without `and al, 1`. DEVIATE: off
    /// at m16, with it.
    fn fold_and_of_extload(&self) -> bool {
        self.zext_free || !self.i16_mul_chains
    }

    fn fold_xor_of_and_with_mask(&self) -> bool {
        true
    }

    fn sub_without_borrow_is_xor(&self) -> bool {
        true
    }

    /// `BuildSDIV`/`BuildUDIV` widen to a legal `i64` multiply for an `i32`
    /// divisor too (`imul rax, rcx, 0xCCCCCCCD; shr rax, 35`), where ours kept
    /// the `div` (an `i32` has no `MULHS` here). Where `i64` is not legal
    /// (m16, m32) `can_do(Mul, i64)` refuses it.
    fn widen_multiply_high_any_width(&self) -> bool {
        true
    }

    /// DEVIATE SD444: a division's full-width magic goes to `EAX`, so the
    /// dividend is the r/m operand whose length the 486 reads (the rule of
    /// `x86_mul::magic_is_wider`, asked here where the dividend still has its
    /// known bits). The pin's order, magic as the r/m operand, is kept where
    /// the multiply has no early-out.
    fn multiply_high_magic_first(
        &self,
        dag: &SelectionDAG,
        signed: bool,
        dividend: SDValue,
        magic: SDValue,
    ) -> bool {
        let Some(flat) = self.multiply_early_out else { return false };
        let Some(value) = dag.constant(magic) else { return false };
        if dag.constant(dividend).is_some() {
            return false;
        }
        let width = dag.value_type(dividend).size_in_bits();
        let all = u64::MAX.checked_shr(u64::BITS - width.min(u64::BITS)).unwrap_or(0);
        let known_zero = llrm_codegen::backend::selectiondag::known_bits::known_bits(dag, dividend).zero as u64 & all;
        llrm_x86_dagisel::x86_mul::magic_is_wider(value as i64, width, signed, flat, known_zero)
    }

    /// `visitSRA`'s `(sra (trunc (srl x, c1)), c2)` is `(trunc (sra x, c1 +
    /// c2))`: the widened divide's `shr 32; sar k` is one `sar 32 + k`.
    fn fold_sra_of_trunc_of_shift(&self) -> bool {
        true
    }

    /// `X86TargetLowering::shouldTransformSignedTruncationCheck`: a scalar
    /// check whose type and kept width are both a size `movsx` takes (8, 16,
    /// 32, 64). `(add x, 1<<(k-1)) u< 1<<k` then is `sext_inreg x, k == x`: no
    /// 64-bit constant, one `movslq; cmp` for the add, the constant and the
    /// compare.
    fn should_transform_signed_truncation_check(
        &self,
        vt: EVT,
        kept_bits: u32,
    ) -> bool {
        let movsx = |bits: u32| matches!(bits, 8 | 16 | 32 | 64);
        vt.is_integer() && movsx(vt.size_in_bits()) && movsx(kept_bits)
    }

    fn canonicalize_ge_le(&self) -> bool {
        true
    }

    /// `X86TargetLowering::hasAndNotCompare`: NOT+TEST for any scalar integer.
    fn has_and_not_compare(
        &self,
        vt: EVT,
    ) -> bool {
        vt.is_integer()
    }

    /// `X86TargetLowering::isTruncateFree`: any narrower integer.
    fn is_truncate_free(
        &self,
        from: EVT,
        to: EVT,
    ) -> bool {
        from.is_integer() && to.is_integer() && from.size_in_bits() > to.size_in_bits()
    }

    /// `X86TargetLowering::isZExtFree(EVT, EVT)`: `i32` to `i64` on x86-64.
    fn is_zext_free(
        &self,
        from: EVT,
        to: EVT,
    ) -> bool {
        self.zext_free && from == EVT::I32 && to == EVT::I64
    }

    /// Where `i16` is native (`i16_mul_chains`: m16) a dword's halves are the
    /// registers it is returned and passed in.
    fn split_dword_loads(&self) -> bool {
        self.i16_mul_chains
    }

    fn pack_word_pairs(&self) -> bool {
        true
    }

    fn push_freeze_through_operands(&self) -> bool {
        true
    }

    fn fold_sext_in_reg_of_load(&self) -> bool {
        self.sext_load
    }

    /// `X86TargetLowering::isTruncateFree` (any narrowing of integers) and
    /// `isZExtFree` (i32 to i64 on x86-64).
    fn free_narrowing(
        &self,
        wide: EVT,
        narrow: EVT,
    ) -> bool {
        self.zext_free && wide == EVT::I64 && narrow == EVT::I32
    }

    /// `X86TargetLowering::isNarrowingProfitable`.
    fn narrowing_profitable(
        &self,
        wide: EVT,
        narrow: EVT,
    ) -> bool {
        !(wide == EVT::I32 && narrow == EVT::I16)
    }

    fn is_compare_with_flags(
        &self,
        user: ISD,
    ) -> bool {
        user == x86isd::CMP
    }

    /// DEVIATE G1 (x87 only: with SSE the conversion is in registers): the
    /// `fist` to a scratch slot, the load of the slot and the store of it
    /// become one `fist` to the store's address.
    ///
    /// Taken only when the slot is the pair's alone (the `fist` is read by
    /// that load, the slot node by those two), the load's value is the store's
    /// only input, all three are plain accesses of one width, and the
    /// conversion does not widen (an unsigned `i32` goes through an `i64` slot
    /// and is left). The new `fist` takes the store's chain, so it runs where
    /// the store did and orders against every access as the store did.
    /// O(1) per store.
    fn store_of_converted_slot(
        &self,
        dag: &mut SelectionDAG,
        store: SDValue,
    ) -> Option<SDValue> {
        if self.sse2 {
            return None;
        }
        let (chain, value, address) = (dag.operand(store, 0), dag.operand(store, 1), dag.operand(store, 2));
        let NodeData::Mem(stored) = dag.node(store).data else { return None };
        let plain = |info: &MemInfo| !info.volatile && info.ordering.is_none() && info.ext.is_none();
        if dag.opcode(value) != ISD::Load || !dag.has_one_use(value.node) || !plain(&stored) {
            return None;
        }
        let NodeData::Mem(loaded) = dag.node(value).data else { return None };
        if !plain(&loaded) || loaded.mem_vt != stored.mem_vt || dag.value_type(value) != stored.mem_vt {
            return None;
        }
        let fist = dag.operand(value, 0);
        if fist.res != 0 || !matches!(dag.opcode(fist), x86isd::FIST | x86isd::FP_TO_INT_IN_MEM) {
            return None;
        }
        let NodeData::Mem(converted) = dag.node(fist).data else { return None };
        let slot = dag.operand(fist, 2);
        if !plain(&converted)
            || converted.mem_vt != loaded.mem_vt
            || dag.opcode(slot) != ISD::FrameIndex
            || dag.operand(value, 1) != slot
            || !dag.has_one_use(fist.node)
            || dag.use_count_up_to(slot.node, 3) != 2
        {
            return None;
        }
        let info = MemInfo { align: stored.align, base: stored.base, space: stored.space, ..converted };
        let opcode = dag.opcode(fist);
        let converted_value = dag.operand(fist, 1);
        // Chained where the store was, so every access that came before it in
        // the program still comes before the `fist`, whatever it aliases.
        Some(dag.intern(
            opcode,
            &[EVT::OTHER],
            &[chain, converted_value, address],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        ))
    }

    fn has_target_combine(
        &self,
        op: ISD,
    ) -> bool {
        op == ISD::Mul
            || (self.share_products && (op == ISD::FAdd || op == ISD::FSub))
            || op == ISD::SetCC
            || op == ISD::Select
            || op == ISD::Add
            || op == ISD::Or
            || op == ISD::Xor
            || op == ISD::And
            || op == ISD::Sub
            || op == x86isd::ADD
            || op == x86isd::SUB
            || op == x86isd::CMOV
            || op == x86isd::SETCC
            || op == x86isd::BRCOND
            || op == x86isd::ADC
            || op == x86isd::SBB
            || op == ISD::UIntToFp
    }

    fn perform_dag_combine(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
        level: CombineLevel,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::Mul if self.imul_if_smaller && !self.hot_chains && dag.value_type(node) != EVT::I64 => {
                combine_mul(dag, node, level, false, self.i16_mul_chains, self.profile).filter(|chain| {
                    let imul = if dag.constant(dag.operand(node, 1)).is_some_and(|bits| {
                        let bits = bits as u32 as i32;
                        (-128..128).contains(&bits)
                    }) {
                        3
                    } else {
                        6
                    };
                    // A trailing `shl 1..3` whose users are all adds folds into
                    // the address's scale (`t[3 * i]` as
                    // `[t + r*4]`): no bytes of its own.
                    let scale =
                        |shift: SDValue| dag.constant(dag.operand(shift, 1)).is_some_and(|by| (1..=3).contains(&by));
                    let folds = dag.opcode(*chain) == ISD::Shl
                        && scale(*chain)
                        && dag.users(node).all(|user| dag.opcode(SDValue { node: user, res: 0 }) == ISD::Add);
                    let body = if folds { dag.operand(*chain, 0) } else { *chain };
                    chain_bytes(dag, body, dag.operand(node, 0)) <= imul
                })
            }
            ISD::Mul => combine_mul(dag, node, level, self.min_size && !self.hot_chains, self.i16_mul_chains, self.profile),
            ISD::SetCC => combine_integer_setcc(dag, node, level),
            ISD::Add => combine_add(dag, node),
            ISD::Or => combine_compare_equal(dag, node, self.sse2, self.zext_free)
                .or_else(|| combine_or_xor_with_setcc(dag, node))
                .or_else(|| combine_or(dag, node)),
            ISD::Xor => combine_or_xor_with_setcc(dag, node),
            ISD::And => combine_compare_equal(dag, node, self.sse2, self.zext_free),
            ISD::Sub => combine_sub(dag, node, self.three_address_adds),
            ISD::FAdd | ISD::FSub => combine_shared_product(dag, node),
            x86isd::ADD | x86isd::SUB => combine_x86_add_sub(dag, node),
            ISD::Select | x86isd::CMOV if self.branch_constants() => None,
            ISD::Select => combine_select_of_two_constants(dag, node, self.zext_free),
            x86isd::CMOV => combine_cmov(dag, node),
            x86isd::SETCC => combine_setcc(dag, node),
            x86isd::BRCOND => combine_brcond(dag, node),
            x86isd::ADC | x86isd::SBB => combine_adc_sbb(dag, node),
            ISD::UIntToFp => combine_uint_to_fp(dag, node),
            _ => None,
        }
    }

    fn target_shrink_demanded_constant(
        &mut self,
        dag: &mut SelectionDAG,
        op: SDValue,
        demanded: u128,
        _level: CombineLevel,
    ) -> Option<Option<SDValue>> {
        x86_shrink_demanded_constant(dag, op, demanded, self.i16_mul_chains)
    }

    fn promote_type(
        &self,
        dag: &SelectionDAG,
        tli: &TargetLowering,
        op: SDValue,
    ) -> Option<EVT> {
        is_desirable_to_promote_op(dag, tli, op)
    }
}

/// `X86TargetLowering::targetShrinkDemandedConstant`, scalar part: an `and`
/// mask is not shrunk below what a `movzx` matches. The mask is rounded up to
/// a byte, then a power of two; if that is the mask, it stays; if it only
/// differs in undemanded bits, it becomes that.
fn x86_shrink_demanded_constant(
    dag: &mut SelectionDAG,
    op: SDValue,
    demanded: u128,
    keep_imm8: bool,
) -> Option<Option<SDValue>> {
    if dag.opcode(op) != ISD::And {
        return None;
    }
    let vt = dag.value_type(op);
    let mask = dag.constant(dag.operand(op, 1))?;
    let shrunk = mask & demanded;
    let active = 128 - shrunk.leading_zeros();
    if active == 0 {
        return None;
    }
    let width = active.max(8).next_power_of_two().min(vt.size_in_bits());
    let zero_extend = llrm_codegen::backend::selectiondag::fold::mask(width);
    if zero_extend == mask {
        return Some(None);
    }
    if zero_extend & !(mask | !demanded) != 0 {
        // DEVIATE SD212c, m16 only (`keep_imm8` is `i16_mul_chains`, the
        // DEVIATE M1 / SD441 switch): the generic shrink would turn an imm8
        // mask (`-64`) into an imm32 (`0x0FFFFFC0`), 3 B longer for the same
        // work. Keep it. Exit: with the M1 review, 2026-12-01; owner
        // llrm-target-wasm.
        return (keep_imm8 && mask & !demanded != 0 && is_imm8_mask(mask, vt.size_in_bits())).then_some(None);
    }
    let new_mask = dag.get_constant(zero_extend, vt);
    let x = dag.operand(op, 0);
    Some(Some(dag.get_node(ISD::And, vt, &[x, new_mask])))
}

/// A mask of 16 bits or more that an `and` encodes as a sign-extended imm8.
fn is_imm8_mask(
    mask: u128,
    bits: u32,
) -> bool {
    if bits < 16 {
        return false;
    }
    let sign = 1u128 << (bits - 1);
    let signed = if mask & sign != 0 { (mask as i128) - (1i128 << bits) } else { mask as i128 };
    (-128..=127).contains(&signed)
}

/// Whether this target promotes `i16` (and `i8` multiply) operations to `i32`
/// (SD441: not at m16). `is_desirable_to_promote_op` and the `i16` multiply
/// chain both ask this, so the chain is built exactly where nothing promotes.
pub fn promotes_i16(tli: &TargetLowering) -> bool {
    !tli.types.has_far_pointer()
}

/// `isTypeDesirableForOp` (undesirable) and `IsDesirableToPromoteOp`: an `i16`
/// operation, or an `i8` multiply by a constant, is done in `i32` unless an
/// operand is a load the operation could fold.
///
/// DEVIATE SD441: not at m16. The pin promotes there too; in real mode each
/// promoted op carries a `66h` prefix (a `lea` of it `67h` as well), which
/// grows every `i16` op and breaks the m16 -Os rule. Exit: promotion priced
/// against those prefix bytes and clocks. Until then `i16_mul_chains` builds
/// the multiply's chain at `i16`.
fn is_desirable_to_promote_op(
    dag: &SelectionDAG,
    tli: &TargetLowering,
    op: SDValue,
) -> Option<EVT> {
    let vt = dag.value_type(op);
    let opcode = dag.opcode(op);
    if !promotes_i16(tli) || !tli.types.is_type_legal(vt) {
        return None;
    }
    let is8_mul_by_constant = vt == EVT::I8 && opcode == ISD::Mul && dag.constant(dag.operand(op, 1)).is_some();
    let undesirable = match vt {
        EVT::I8 => matches!(opcode, ISD::Mul | ISD::Shl),
        EVT::I16 => matches!(
            opcode,
            ISD::SignExtend
                | ISD::ZeroExtend
                | ISD::AnyExtend
                | ISD::Mul
                | ISD::Shl
                | ISD::Sra
                | ISD::Srl
                | ISD::Sub
                | ISD::Add
                | ISD::And
                | ISD::Or
                | ISD::Xor
        ),
        _ => false,
    };
    if !undesirable || (vt != EVT::I16 && !is8_mul_by_constant) {
        return None;
    }
    // `X86::mayFoldLoad`: a single-use plain load.
    let may_fold_load = |v: SDValue| {
        dag.opcode(v) == ISD::Load
            && dag.users(v).count() == 1
            && matches!(dag.node(v).data, NodeData::Mem(info) if info.ext.is_none())
    };
    // `IsFoldableRMW`: the only user is a store to the load's address.
    let foldable_rmw = |load: SDValue| {
        let mut users = dag.users(op);
        let (Some(user), None) = (users.next(), users.next()) else { return false };
        let user = SDValue { node: user, res: 0 };
        dag.opcode(user) == ISD::Store && dag.operand(user, 2) == dag.operand(load, 1)
    };
    let (n0, n1) =
        (dag.operand(op, 0), if dag.node(op).ops.len() > 1 { dag.operand(op, 1) } else { dag.operand(op, 0) });
    match opcode {
        ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend => {}
        ISD::Shl | ISD::Sra | ISD::Srl => {
            if may_fold_load(n0) && foldable_rmw(n0) {
                return None;
            }
        }
        ISD::Add | ISD::And | ISD::Or | ISD::Xor | ISD::Mul | ISD::Sub => {
            let commute = opcode != ISD::Sub;
            let is_const = |v: SDValue| dag.constant(v).is_some();
            if may_fold_load(n1) && (!commute || !is_const(n0) || (opcode != ISD::Mul && foldable_rmw(n1))) {
                return None;
            }
            if may_fold_load(n0) && ((commute && !is_const(n1)) || (opcode != ISD::Mul && foldable_rmw(n0))) {
                return None;
            }
        }
        _ => return None,
    }
    Some(EVT::I32)
}

impl Hooks for X86Lowering<'_> {
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue> {
        match dag.opcode(node) {
            ISD::SetCC => lower_setcc(dag, node, self.min_size),
            ISD::Select => lower_select(dag, node, self.cmov, self.min_size),
            ISD::BrCond => lower_brcond(dag, node, self.min_size),
            ISD::UAddO | ISD::USubO | ISD::SAddO | ISD::SSubO | ISD::UMulO | ISD::SMulO => Some(lower_xalu(dag, node)),
            ISD::UAddOCarry | ISD::USubOCarry | ISD::SAddOCarry | ISD::SSubOCarry => Some(lower_addsub_carry(dag, node)),
            ISD::SetCCCarry => Some(lower_setcc_carry(dag, node)),
            ISD::GlobalAddress => lower_global_address(dag, node),
            ISD::VAStart => lower_vastart(dag, node),
            ISD::FpToSInt | ISD::FpToUInt => lower_fp_to_int(dag, node, self.sse2),
            ISD::LRint | ISD::LLRint => lower_lrint(dag, node),
            ISD::SIntToFp | ISD::UIntToFp => lower_int_to_fp(dag, node, self.sse2),
            ISD::ConstantPool => lower_constant_pool(dag, node),
            ISD::JumpTable => lower_jump_table(dag, node),
            ISD::ShlParts | ISD::SraParts | ISD::SrlParts => Some(lower_shift_parts(dag, node)),
            ISD::Abs => Some(lower_abs(dag, node)),
            ISD::SSubSat => Some(lower_ssubsat(dag, node)),
            ISD::FShl | ISD::FShr => lower_funnel_shift(dag, node),
            ISD::FNeg | ISD::FAbs => lower_fabs_or_fneg(dag, node),
            ISD::FCopySign => lower_fcopysign(dag, node),
            ISD::DynamicStackAlloc => lower_dynamic_stackalloc(dag, node),
            ISD::StackSave => {
                let (chain, vt) = (dag.operand(node, 0), dag.value_type(node));
                Some(dag.get_copy_from_phys_reg_plain(chain, stack_register(vt), vt))
            }
            ISD::StackRestore => {
                let (chain, saved) = (dag.operand(node, 0), dag.operand(node, 1));
                let sp = dag.get_phys_register(dag.value_type(saved), stack_register(dag.value_type(saved)));
                Some(dag.get_node(ISD::CopyToReg, EVT::OTHER, &[chain, sp, saved]))
            }
            // `LowerFP_EXTEND`: the Custom arm exists for `f16`; a scalar
            // `f32` to `f64` is returned as is and selected by pattern.
            // DEVIATE SD107. `lowerFaddFsub`: the node itself unless an operand
            // pair is a horizontal add (SSE3 `haddps`), which the target does
            // not describe; so the scalar op is selected by pattern.
            ISD::FAdd | ISD::FSub => Some(node),
            ISD::FpExtend if dag.value_type(dag.operand(node, 0)) == EVT::Simple(MVT::F32) => Some(node),
            ISD::CtLz | ISD::CtLzZeroPoison => Some(lower_ctlz(dag, node, self.zext_free)),
            ISD::CtTz => Some(lower_cttz(dag, node, self.zext_free)),
            ISD::Parity => Some(lower_parity(dag, node)),
            ISD::CtPop => lower_ctpop(dag, node),
            _ => None,
        }
    }
}

pub(super) fn cond_code(
    dag: &mut SelectionDAG,
    code: u128,
) -> SDValue {
    dag.get_constant_with(code, EVT::I8, true, false)
}

fn cond_of(
    dag: &SelectionDAG,
    setcc: SDValue,
) -> CondCode {
    match dag.node(setcc).data {
        llrm_codegen::backend::selectiondag::dag::NodeData::CondCode(cc) => cc,
        other => unreachable!("a condition code operand, not {other:?}"),
    }
}

/// `TranslateX86CC` for integers: the code, and a right-hand side that may
/// have been replaced (`x > -1` is `NS` on `x`).
fn translate_cc(
    dag: &mut SelectionDAG,
    cc: CondCode,
    rhs: &mut SDValue,
) -> u128 {
    if let Some(bits) = dag.constant(*rhs) {
        let vt = dag.value_type(*rhs);
        let all_ones = bits == llrm_codegen::backend::selectiondag::fold::mask(vt.size_in_bits());
        match cc {
            CondCode::SetGt if all_ones => {
                *rhs = dag.get_constant(0, vt);
                return cond::NS;
            }
            CondCode::SetLt if bits == 0 => return cond::S,
            // The pin gets this from `x <= C` -> `x < C + 1` ([L5358][pin]);
            // x86's
            // `canonicalize_ge_le` is off (DEVIATE in `dag_combiner`), so this
            // is that rewrite's sign-test case. Exit: that hook on.
            //
            // [pin]:
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5358
            CondCode::SetLe if all_ones => {
                *rhs = dag.get_constant(0, vt);
                return cond::S;
            }
            CondCode::SetGe if bits == 0 => return cond::NS,
            CondCode::SetLt if bits == 1 => {
                *rhs = dag.get_constant(0, vt);
                return cond::LE;
            }
            _ => {}
        }
    }
    match cc {
        CondCode::SetEq => cond::E,
        CondCode::SetGt => cond::G,
        CondCode::SetGe => cond::GE,
        CondCode::SetLt => cond::L,
        CondCode::SetLe => cond::LE,
        CondCode::SetNe => cond::NE,
        CondCode::SetULt => cond::B,
        CondCode::SetUGt => cond::A,
        CondCode::SetULe => cond::BE,
        CondCode::SetUGe => cond::AE,
        other => unreachable!("invalid integer condition {other:?}"),
    }
}

/// `isNON_EXTLoad`.
fn is_non_ext_load(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.opcode(value) == ISD::Load
        && matches!(
            dag.node(value).data,
            llrm_codegen::backend::selectiondag::dag::NodeData::Mem(info) if info.ext.is_none()
        )
}

/// `TranslateX86CC` for floats: the code and the operands, swapped where the
/// flags want the other order. `None` for `oeq` and `une`, which the
/// legalizer has already expanded to two compares.
fn translate_fp_cc(
    dag: &SelectionDAG,
    mut cc: CondCode,
    lhs: &mut SDValue,
    rhs: &mut SDValue,
) -> Option<u128> {
    if is_non_ext_load(dag, *lhs) && !is_non_ext_load(dag, *rhs) {
        cc = cc.swapped_operands();
        std::mem::swap(lhs, rhs);
    }
    if matches!(
        cc,
        CondCode::SetOLt | CondCode::SetOLe | CondCode::SetUGt | CondCode::SetUGe
    ) {
        std::mem::swap(lhs, rhs);
    }
    // ZF PF CF: greater 0 0 0, less 0 0 1, equal 1 0 0, unordered 1 1 1.
    Some(match cc {
        CondCode::SetUEq | CondCode::SetEq => cond::E,
        CondCode::SetOLt | CondCode::SetOGt | CondCode::SetGt => cond::A,
        CondCode::SetOLe | CondCode::SetOGe | CondCode::SetGe => cond::AE,
        CondCode::SetUGt | CondCode::SetULt | CondCode::SetLt => cond::B,
        CondCode::SetUGe | CondCode::SetULe | CondCode::SetLe => cond::BE,
        CondCode::SetONe | CondCode::SetNe => cond::NE,
        CondCode::SetUo => cond::P,
        CondCode::SetO => cond::NP,
        _ => return None,
    })
}

/// A float compare's flags and condition code (`LowerSETCC`, floating point).
fn float_flags(
    dag: &mut SelectionDAG,
    setcc: SDValue,
) -> Option<(SDValue, SDValue)> {
    let (mut lhs, mut rhs) = (dag.operand(setcc, 0), dag.operand(setcc, 1));
    let code = translate_fp_cc(dag, cond_of(dag, dag.operand(setcc, 2)), &mut lhs, &mut rhs)?;
    let flags = dag.get_node(x86isd::FCMP, EVT::I32, &[lhs, rhs]);
    Some((flags, cond_code(dag, code)))
}

/// `hasNonFlagsUse`: whether `op` has a reader other than a branch, a compare
/// or a select condition (looking past a truncate nothing else reads).
fn has_non_flags_use(
    dag: &SelectionDAG,
    op: SDValue,
) -> bool {
    dag.users(op).any(|user| {
        let mut user = SDValue { node: user, res: 0 };
        let mut through = op;
        if dag.opcode(user) == ISD::Truncate {
            let mut readers = dag.users(user);
            if let (Some(only), None) = (readers.next(), readers.next()) {
                through = user;
                user = SDValue { node: only, res: 0 };
            }
        }
        let condition = dag.opcode(user) == ISD::Select && dag.operand(user, 0) == through;
        !matches!(dag.opcode(user), ISD::BrCond | ISD::SetCC) && !condition
    })
}

/// `isProfitableToUseFlagOp`: every reader of `op` is a register copy, a
/// compare or a store.
fn is_profitable_to_use_flag_op(
    dag: &SelectionDAG,
    op: SDValue,
) -> bool {
    dag.users(op)
        .all(|user| matches!(
            dag.opcode(SDValue { node: user, res: 0 }),
            ISD::CopyToReg | ISD::SetCC | ISD::Store
        ))
}

/// `EmitTest`: the flags of `op` against zero. An arithmetic result that only
/// feeds registers, stores and compares is made by the flag-setting x86 form
/// and the compare reads its flags, when the condition needs neither the carry
/// nor the overflow flag (a `test` clears them).
fn emit_test(
    dag: &mut SelectionDAG,
    op: SDValue,
    code: u128,
) -> SDValue {
    let vt = dag.value_type(op);
    let zero = dag.get_constant(0, vt);
    let test = |dag: &mut SelectionDAG| dag.get_node(x86isd::CMP, EVT::I32, &[op, zero]);
    let need_carry = matches!(code, cond::A | cond::AE | cond::B | cond::BE);
    let wraps = !matches!(dag.opcode(op), ISD::Add | ISD::Sub | ISD::Mul | ISD::Shl)
        || dag.node(op).flags.0 & SDNodeFlags::NO_SIGNED_WRAP.0 == 0;
    let need_overflow = matches!(
        code,
        cond::G | cond::GE | cond::L | cond::LE | cond::O | cond::NO
    ) && wraps;
    if op.res != 0 || need_carry || need_overflow {
        return test(dag);
    }
    let opcode = match dag.opcode(op) {
        ISD::And if !has_non_flags_use(dag, op) => return test(dag),
        ISD::And => x86isd::AND,
        ISD::Add => x86isd::ADD,
        ISD::Sub => x86isd::SUB,
        ISD::Or => x86isd::OR,
        ISD::Xor => x86isd::XOR,
        x86isd::ADD | x86isd::SUB | x86isd::OR | x86isd::XOR | x86isd::AND => {
            return SDValue { node: op.node, res: 1 };
        }
        // `USUBO`/`SSUBO` will become an `X86ISD::SUB`, and its flags are the compare's.
        ISD::USubO | ISD::SSubO => {
            let (lhs, rhs) = (dag.operand(op, 0), dag.operand(op, 1));
            let sub = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[lhs, rhs]);
            return SDValue { node: sub.node, res: 1 };
        }
        _ => return test(dag),
    };
    if !is_profitable_to_use_flag_op(dag, op) {
        return test(dag);
    }
    let (lhs, rhs) = (dag.operand(op, 0), dag.operand(op, 1));
    let new = dag.get_node_multi(opcode, &[vt, EVT::I32], &[lhs, rhs]);
    dag.replace_all_uses_with(op, SDValue { node: new.node, res: 0 });
    SDValue { node: new.node, res: 1 }
}

/// `EmitCmp`: the flags of `op0 - op1`.
fn emit_cmp(
    dag: &mut SelectionDAG,
    op0: SDValue,
    op1: SDValue,
    code: u128,
) -> SDValue {
    if dag.is_null_constant(op1) {
        // a runtime routine's answer in the flags: they are already set
        if dag.opcode(op0) == super::x86isd::LLRM_CALL_FLAGS {
            return dag.operand(op0, 0);
        }
        return emit_test(dag, op0, code);
    }
    let vt = dag.value_type(op0);
    let sub = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[op0, op1]);
    SDValue { node: sub.node, res: 1 }
}

/// `getBT`: `bt src, bit` as flags; the source is any-extended to a dword
/// (there is no byte `bt`, the word form is longer) and the bit number follows
/// its width.
fn get_bt(
    dag: &mut SelectionDAG,
    mut src: SDValue,
    bit: SDValue,
) -> SDValue {
    if dag.value_type(src).size_in_bits() < 32 {
        src = dag.get_node(ISD::AnyExtend, EVT::I32, &[src]);
    }
    let vt = dag.value_type(src);
    let bit = match dag.value_type(bit).size_in_bits().cmp(&vt.size_in_bits()) {
        std::cmp::Ordering::Equal => bit,
        std::cmp::Ordering::Less => dag.get_node(ISD::AnyExtend, vt, &[bit]),
        std::cmp::Ordering::Greater => dag.get_node(ISD::Truncate, vt, &[bit]),
    };
    dag.get_node(x86isd::BT, EVT::I32, &[src, bit])
}

/// `LowerAndToBT`: `(x & (1 << n)) == 0` and `((x >> n) & 1) != 0` test bit `n`
/// of `x`; the carry flag is the bit, so `!= 0` is `B` and `== 0` is `AE`.
/// `(x & 2^k)` is the bit `k` when the immediate does not fit a `test` (32
/// bits; a byte at minsize). (The reassociated three-way `and` is not taken,
/// SD200.)
pub(super) fn lower_and_to_bt(
    dag: &mut SelectionDAG,
    and: SDValue,
    cc: CondCode,
    min_size: bool,
) -> Option<(SDValue, u128)> {
    let (a, b) = (dag.operand(and, 0), dag.operand(and, 1));
    let is_bit_shift =
        |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::Shl && dag.constant(dag.operand(v, 0)) == Some(1);
    let (mut src, bit) = if is_bit_shift(dag, b) {
        (a, dag.operand(b, 1))
    } else if is_bit_shift(dag, a) {
        (b, dag.operand(a, 1))
    } else if dag.constant(b) == Some(1) && dag.opcode(a) == ISD::Srl {
        (dag.operand(a, 0), dag.operand(a, 1))
    } else if dag.constant(a) == Some(1) && dag.opcode(b) == ISD::Srl {
        (dag.operand(b, 0), dag.operand(b, 1))
    } else if let Some(imm) = dag.constant(b).filter(|imm| imm.is_power_of_two()) {
        // `(Src & ConstPow2)`: BT when the immediate cannot be a TEST's.
        let bit_no = imm.trailing_zeros();
        if bit_no < if min_size { 8 } else { 32 } {
            return None;
        }
        let vt = dag.value_type(a);
        (a, dag.get_constant(u128::from(bit_no), vt))
    } else {
        return None;
    };
    let mut cc = cc;
    // A bit flip folds into the condition.
    let ones = llrm_codegen::backend::selectiondag::fold::mask(dag.value_type(src).size_in_bits());
    if dag.opcode(src) == ISD::Xor && dag.constant(dag.operand(src, 1)) == Some(ones) {
        src = dag.operand(src, 0);
        cc = if cc == CondCode::SetEq { CondCode::SetNe } else { CondCode::SetEq };
    }
    let flags = get_bt(dag, src, bit);
    Some((flags, if cc == CondCode::SetEq { cond::AE } else { cond::B }))
}

/// `emitFlagsForSetcc`: the flags and the condition-code constant.
fn emit_flags_for_setcc(
    dag: &mut SelectionDAG,
    op0: SDValue,
    op1: SDValue,
    cc: CondCode,
    min_size: bool,
) -> (SDValue, SDValue) {
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && (dag.is_null_constant(op1) || dag.constant(op1) == Some(1))
        && dag.opcode(op0) == x86isd::SETCC
    {
        // A test of a `setcc` is the `setcc`'s own flags, inverted if needed.
        let invert = (cc == CondCode::SetNe) ^ dag.is_null_constant(op1);
        let code = dag.constant(dag.operand(op0, 0)).expect("a SETCC reads a condition code");
        let code = if invert { code ^ 1 } else { code };
        return (dag.operand(op0, 1), cond_code(dag, code));
    }
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && dag.opcode(op0) == ISD::And
        && dag.users(op0).count() == 1
        && dag.is_null_constant(op1)
    {
        if let Some((flags, code)) = lower_and_to_bt(dag, op0, cc, min_size) {
            return (flags, cond_code(dag, code));
        }
    }
    // The carry of the add stands for the compare: `(seteq (add x, -1), -1)`
    // is `x == 0`, which `x + -1` carries out of unless it is 0 (`dec; cmp -1`
    // was two instructions, `add -1; jb` is one).
    if matches!(cc, CondCode::SetEq | CondCode::SetNe)
        && dag.constant(op1)
            == Some(llrm_codegen::backend::selectiondag::fold::mask(dag.value_type(op1).size_in_bits()))
        && dag.opcode(op0) == ISD::Add
        && dag.operand(op0, 1) == op1
        && is_profitable_to_use_flag_op(dag, op0)
    {
        let vt = dag.value_type(op0);
        let x = dag.operand(op0, 0);
        let new = dag.get_node_multi(x86isd::ADD, &[vt, EVT::I32], &[x, op1]);
        dag.replace_all_uses_with(op0, SDValue { node: new.node, res: 0 });
        let code = if cc == CondCode::SetEq { cond::AE } else { cond::B };
        return (SDValue { node: new.node, res: 1 }, cond_code(dag, code));
    }
    let mut rhs = op1;
    let code = translate_cc(dag, cc, &mut rhs);
    let flags = emit_cmp(dag, op0, rhs, code);
    (flags, cond_code(dag, code))
}

/// `LowerSETCC`, integers.
fn lower_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
    min_size: bool,
) -> Option<SDValue> {
    let (op0, mut op1) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.value_type(op0).is_floating_point() {
        let (flags, code) = float_flags(dag, node)?;
        return Some(dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]));
    }
    if !dag.value_type(op0).is_integer() {
        return None;
    }
    let mut cc = cond_of(dag, dag.operand(node, 2));
    if let Some(bits) = dag.constant(op1).filter(|&bits| bits != 0) {
        let vt = dag.value_type(op1);
        let width = vt.size_in_bits();
        let max_signed = (1u128 << (width - 1)) - 1;
        let max_unsigned = llrm_codegen::backend::selectiondag::fold::mask(width);
        let widens = (cc == CondCode::SetGt && bits != max_signed) || (cc == CondCode::SetUGt && bits != max_unsigned);
        // The constant plus one must be a 32-bit signed immediate, and a byte
        // one if the original was.
        let plus_one = (bits + 1) & max_unsigned;
        let signed = |value: u128, bits_wide: u32| {
            let extended = ((value << (128 - width)) as i128) >> (128 - width);
            extended >= -(1i128 << (bits_wide - 1)) && extended < (1i128 << (bits_wide - 1))
        };
        if widens && signed(plus_one, 32.min(width)) && (!signed(bits, 8.min(width)) || signed(plus_one, 8.min(width)))
        {
            op1 = dag.get_constant(plus_one, vt);
            cc = if cc == CondCode::SetGt { CondCode::SetGe } else { CondCode::SetUGe };
        }
    }
    let (flags, code) = emit_flags_for_setcc(dag, op0, op1, cc, min_size);
    Some(dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]))
}

/// `isX86LogicalCmp`: flags a `CMOV` can read directly.
fn is_logical_cmp(
    dag: &SelectionDAG,
    flags: SDValue,
) -> bool {
    let op = dag.opcode(flags);
    op == x86isd::CMP
        || op == x86isd::FCMP
        || (flags.res == 1 && [x86isd::ADD, x86isd::SUB, x86isd::OR, x86isd::XOR, x86isd::AND].contains(&op))
}

/// `X86::mayFoldLoad`: a plain load nothing else reads.
fn may_fold_load(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    dag.opcode(value) == ISD::Load && dag.users(value).count() == 1
}

/// `LowerSELECTWithCmpZero`, the `(x & 1) == 0` part: a select on the low bit
/// is arithmetic on the bit's mask `-(x & 1)`. Without `cmov`, a select of two
/// constants and a select between `y` and `y op z` become a mask, an `and` and
/// the operation. The `x - 1` / `0 - x` carry forms (`sbb`) are not translated.
fn select_with_cmp_zero(
    dag: &mut SelectionDAG,
    cmp_val: SDValue,
    lhs: SDValue,
    rhs: SDValue,
    code: u128,
    can_cmov: bool,
) -> Option<SDValue> {
    let (cmp_vt, vt) = (dag.value_type(cmp_val), dag.value_type(lhs));
    if !cmp_vt.is_integer() || !vt.is_integer() {
        return None;
    }
    if code != cond::E || dag.opcode(cmp_val) != ISD::And || dag.constant(dag.operand(cmp_val, 1)) != Some(1) {
        return None;
    }
    // `-(x & 1)` in `splat_vt`.
    let splat_lsb = |dag: &mut SelectionDAG, splat_vt: EVT| {
        let neg = if cmp_vt.size_in_bits() > splat_vt.size_in_bits() {
            dag.get_node(ISD::Truncate, splat_vt, &[cmp_val])
        } else if cmp_vt.size_in_bits() < splat_vt.size_in_bits() {
            let wide = dag.get_node(ISD::AnyExtend, splat_vt, &[dag.operand(cmp_val, 0)]);
            let one = dag.get_constant(1, splat_vt);
            dag.get_node(ISD::And, splat_vt, &[wide, one])
        } else {
            cmp_val
        };
        dag.get_negative(neg, splat_vt)
    };
    let all_ones = |dag: &SelectionDAG, v: SDValue| {
        dag.constant(v) == Some(llrm_codegen::backend::selectiondag::fold::mask(dag.width(v)))
    };
    if dag.is_null_constant(lhs) && all_ones(dag, rhs) {
        return Some(splat_lsb(dag, vt));
    }
    if !can_cmov && dag.constant(lhs).is_some() && dag.constant(rhs).is_some() {
        let mask = splat_lsb(dag, vt);
        let diff = dag.get_node(ISD::Xor, vt, &[lhs, rhs]);
        let flip = dag.get_node(ISD::And, vt, &[mask, diff]);
        return Some(dag.get_node(ISD::Xor, vt, &[lhs, flip]));
    }
    if can_cmov {
        return None;
    }
    // `select (x & 1) == 0, y, (y op z)` is `y op (-(x & 1) & z)`.
    let rop = dag.opcode(rhs);
    let (r0, r1) = if dag.node(rhs).ops.len() == 2 { (dag.operand(rhs, 0), dag.operand(rhs, 1)) } else { (rhs, rhs) };
    let identity = match rop {
        ISD::Or | ISD::Xor | ISD::Add if r0 == lhs || r1 == lhs => Some((if r0 == lhs { r1 } else { r0 }, lhs)),
        ISD::Shl | ISD::Sra | ISD::Srl | ISD::Sub if r0 == lhs => Some((r1, lhs)),
        _ => None,
    };
    if let Some((src1, src2)) = identity {
        let src_vt = dag.value_type(src1);
        let mask = splat_lsb(dag, src_vt);
        let and = dag.get_node(ISD::And, src_vt, &[mask, src1]);
        return Some(dag.get_node(rop, vt, &[src2, and]));
    }
    // `select (x & 1) == 0, (y & z), y` is `y & (-(x & 1) | z)`.
    if dag.opcode(lhs) == ISD::And {
        let (l0, l1) = (dag.operand(lhs, 0), dag.operand(lhs, 1));
        if l0 == rhs || l1 == rhs {
            let src1 = if l0 == rhs { l1 } else { l0 };
            let mask = splat_lsb(dag, vt);
            let or = dag.get_node(ISD::Or, vt, &[mask, src1]);
            return Some(dag.get_node(ISD::And, vt, &[rhs, or]));
        }
    }
    None
}

/// `LowerSELECT`, integers: `cmov` of a compare's flags.
fn lower_select(
    dag: &mut SelectionDAG,
    node: SDValue,
    can_cmov: bool,
    min_size: bool,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if !vt.is_integer() && !vt.is_floating_point() {
        return None;
    }
    let (mut cond, op1, op2) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.opcode(cond) == ISD::SetCC {
        if let Some(lowered) = lower_setcc(dag, cond, min_size) {
            cond = lowered;
        }
    }
    // A raw condition `x & 1` is `(x & 1) == 0` with the arms swapped.
    if dag.opcode(cond) == ISD::And && dag.constant(dag.operand(cond, 1)) == Some(1) {
        if let Some(lowered) = select_with_cmp_zero(dag, cond, op2, op1, cond::E, can_cmov) {
            return Some(lowered);
        }
    }
    if dag.opcode(cond) == x86isd::SETCC && dag.opcode(dag.operand(cond, 1)) == x86isd::CMP {
        let cmp = dag.operand(cond, 1);
        if dag.is_null_constant(dag.operand(cmp, 1)) {
            if let Some(code) = dag.constant(dag.operand(cond, 0)) {
                if let Some(lowered) = select_with_cmp_zero(dag, dag.operand(cmp, 0), op1, op2, code, can_cmov) {
                    return Some(lowered);
                }
            }
        }
    }
    let (code, flags);
    if dag.opcode(cond) == x86isd::SETCC && is_logical_cmp(dag, dag.operand(cond, 1)) {
        code = dag.operand(cond, 0);
        flags = dag.operand(cond, 1);
    } else {
        code = cond_code(dag, cond::NE);
        flags = emit_test(dag, cond, cond::NE);
    }
    // DEVIATE SD239: where the pointer is a word (m16) an `i16` select stays
    // a word: widened to `i32` every move takes an operand-size prefix. Owner
    // w10. Exit: none (machine fact). Review 2026-12-10.
    let word16 = dag.config().pointer == EVT::I16;
    let (op1, op2, result_vt) = if (vt == EVT::I8 && can_cmov)
        || (vt == EVT::I16 && !word16 && !may_fold_load(dag, op1) && !may_fold_load(dag, op2))
    {
        (dag.get_node(ISD::AnyExtend, EVT::I32, &[op1]), dag.get_node(ISD::AnyExtend, EVT::I32, &[op2]), EVT::I32)
    } else {
        (op1, op2, vt)
    };
    let cmov = dag.get_node(x86isd::CMOV, result_vt, &[op2, op1, code, flags]);
    Some(if result_vt == vt { cmov } else { dag.get_node(ISD::Truncate, vt, &[cmov]) })
}

/// `LowerConditionalBranch`, integers: `X86ISD::BRCOND chain, dest, cc, flags`.
fn lower_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
    min_size: bool,
) -> Option<SDValue> {
    let (chain, cond, dest) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    if dag.opcode(cond) == ISD::SetCC && dag.value_type(dag.operand(cond, 0)).is_floating_point() {
        if let Some(branches) = lower_float_brcond(dag, node, cond) {
            return Some(branches);
        }
    }
    // `ISD::isOverflowIntrOpRes`: a branch on the overflow of an add or a
    // subtract reads the flags of the arithmetic, itself, its inverse
    // (`xor ov, 1`, what the builder makes of a branch whose true successor
    // follows), or a compare of it with 0 or 1.
    if let Some((code, flags)) = overflow_branch(dag, cond) {
        let code = cond_code(dag, code);
        return Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]));
    }
    let (code, flags) = if dag.opcode(cond) == ISD::SetCC && dag.value_type(dag.operand(cond, 0)).is_integer() {
        let (lhs, rhs) = (dag.operand(cond, 0), dag.operand(cond, 1));
        let cc = cond_of(dag, dag.operand(cond, 2));
        let (flags, code) = emit_flags_for_setcc(dag, lhs, rhs, cc, min_size);
        (code, flags)
    } else {
        // Anything else is tested: `and cond, 1`, then `!= 0`; an `and` is
        // tried as a bit test first.
        if dag.opcode(cond) == ISD::And && dag.users(cond).count() == 1 {
            if let Some((flags, code)) = lower_and_to_bt(dag, cond, CondCode::SetNe, min_size) {
                let code = cond_code(dag, code);
                return Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]));
            }
        }
        let vt = dag.value_type(cond);
        let one = dag.get_constant(1, vt);
        let masked = if dag.opcode(cond) == ISD::And && dag.constant(dag.operand(cond, 1)) == Some(1) {
            cond
        } else {
            dag.get_node(ISD::And, vt, &[cond, one])
        };
        let zero = dag.get_constant(0, vt);
        let (flags, code) = emit_flags_for_setcc(dag, masked, zero, CondCode::SetNe, min_size);
        (code, flags)
    };
    Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]))
}

/// `getX86XALUOOp` for the overflow result `ov` of an unsigned add or
/// subtract: the condition that says it overflowed and the flags of the
/// arithmetic.
fn xalu_condition(
    dag: &mut SelectionDAG,
    ov: SDValue,
) -> Option<(u128, SDValue)> {
    if ov.res != 1 || !matches!(dag.opcode(ov), ISD::UAddO | ISD::USubO) {
        return None;
    }
    let (lhs, rhs) = (dag.operand(ov, 0), dag.operand(ov, 1));
    let vt = dag.value_type(lhs);
    let (opcode, code) = match dag.opcode(ov) {
        ISD::UAddO => (x86isd::ADD, if dag.constant(rhs) == Some(1) { cond::E } else { cond::B }),
        _ => (x86isd::SUB, cond::B),
    };
    let value = dag.get_node_multi(opcode, &[vt, EVT::I32], &[lhs, rhs]);
    Some((code, SDValue { node: value.node, res: 1 }))
}

/// A branch on `xor (X86ISD::SETCC cc, flags), 1` or of an overflow bit (under
/// `and 1`, once the boolean is widened): the flags with the opposite
/// condition. Type legalization has widened an overflow bit by the time the
/// branch is lowered; the pin sees it as `setcc ov != 1` and takes the opposite
/// there.
fn inverted_setcc(
    dag: &mut SelectionDAG,
    cond: SDValue,
) -> Option<(u128, SDValue)> {
    let (mut value, mut invert, mut peeled) = (cond, false, false);
    loop {
        match dag.opcode(value) {
            ISD::And if dag.constant(dag.operand(value, 1)) == Some(1) => value = dag.operand(value, 0),
            ISD::Xor if dag.constant(dag.operand(value, 1)).is_some_and(|c| c & 1 == 1) => {
                invert = !invert;
                value = dag.operand(value, 0);
            }
            _ => break,
        }
        peeled = true;
    }
    if !peeled {
        return None;
    }
    let (code, flags) = if dag.opcode(value) == x86isd::SETCC {
        (dag.constant(dag.operand(value, 0))?, dag.operand(value, 1))
    } else {
        xalu_condition(dag, value)?
    };
    Some((if invert { code ^ 1 } else { code }, flags))
}

/// `LowerConditionalBranch`'s overflow cases: the condition is the overflow
/// result, or `setcc ov ==|!= 0|1`, or its `xor` with 1 (the pin's
/// `rebuildSetCC` makes that `setcc ov != 1`).
fn overflow_branch(
    dag: &mut SelectionDAG,
    cond: SDValue,
) -> Option<(u128, SDValue)> {
    if let Some(found) = xalu_condition(dag, cond) {
        return Some(found);
    }
    if let Some(found) = inverted_setcc(dag, cond) {
        return Some(found);
    }
    let (ov, invert) = match dag.opcode(cond) {
        ISD::Xor if dag.constant(dag.operand(cond, 1)) == Some(1) => (dag.operand(cond, 0), true),
        ISD::SetCC if dag.value_type(dag.operand(cond, 0)).is_integer() => {
            let rhs = dag.constant(dag.operand(cond, 1))?;
            let cc = cond_of(dag, dag.operand(cond, 2));
            if !matches!(cc, CondCode::SetEq | CondCode::SetNe) || rhs > 1 {
                return None;
            }
            // `(CC == SETEQ) == isNullConstant(RHS)` takes the opposite.
            (dag.operand(cond, 0), (cc == CondCode::SetEq) == (rhs == 0))
        }
        _ => return None,
    };
    let (code, flags) = xalu_condition(dag, ov)?;
    Some((if invert { code ^ 1 } else { code }, flags))
}

/// `LowerBRCOND` of a float compare. `une` is two branches to the target
/// (not equal, or unordered); `oeq` followed by a `br` is two branches to the
/// other successor and the `br` to this one. Other codes are one branch on
/// the compare's flags; `oeq` with no `br` after it is left to the generic
/// path (the compare expands to two).
fn lower_float_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
    cond: SDValue,
) -> Option<SDValue> {
    let (chain, dest) = (dag.operand(node, 0), dag.operand(node, 2));
    let (lhs, rhs) = (dag.operand(cond, 0), dag.operand(cond, 1));
    let branch = |dag: &mut SelectionDAG, chain, dest, code, flags| {
        let code = cond_code(dag, code);
        dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags])
    };
    match cond_of(dag, dag.operand(cond, 2)) {
        CondCode::SetUNe => {
            let flags = dag.get_node(x86isd::FCMP, EVT::I32, &[lhs, rhs]);
            let first = branch(dag, chain, dest, cond::NE, flags);
            Some(branch(dag, first, dest, cond::P, flags))
        }
        CondCode::SetOEq => {
            let users: Vec<u32> = dag.users(node).collect();
            let [user] = users[..] else { return None };
            let user = SDValue { node: user, res: 0 };
            if dag.opcode(user) != ISD::Br {
                return None;
            }
            let false_block = dag.operand(user, 1);
            dag.update_node_operands(user.node, &[dag.operand(user, 0), dest]);
            let flags = dag.get_node(x86isd::FCMP, EVT::I32, &[lhs, rhs]);
            let first = branch(dag, chain, false_block, cond::NE, flags);
            Some(branch(dag, first, false_block, cond::P, flags))
        }
        _ => {
            let (flags, code) = float_flags(dag, cond)?;
            Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]))
        }
    }
}

/// `checkBoolTestSetCCCombine`: `cmp (setcc c, f), 0` with `==` or `!=` (or
/// against 1, through zero extends, truncates and `and 1`) tests the flags `f`
/// of the inner compare: its condition, or the opposite one.
fn check_bool_test(
    dag: &SelectionDAG,
    cmp: SDValue,
    code: u128,
) -> Option<(SDValue, u128)> {
    let is_cmp = dag.opcode(cmp) == x86isd::CMP
        || (dag.opcode(cmp) == x86isd::SUB
            && cmp.res == 1
            && dag.users(SDValue { node: cmp.node, res: 0 }).next().is_none());
    if !is_cmp || !matches!(code, cond::E | cond::NE) {
        return None;
    }
    let (a, b) = (dag.operand(cmp, 0), dag.operand(cmp, 1));
    let (constant, mut test) = match (dag.constant(a), dag.constant(b)) {
        (Some(c), _) => (c, b),
        (None, Some(c)) => (c, a),
        _ => return None,
    };
    let mut opposite = code == cond::E;
    let against_true = constant == 1;
    if against_true {
        opposite = !opposite;
    } else if constant != 0 {
        return None;
    }
    while matches!(dag.opcode(test), ISD::ZeroExtend | ISD::Truncate | ISD::And) {
        if dag.opcode(test) == ISD::And {
            let at = if dag.constant(dag.operand(test, 0)) == Some(1) {
                1
            } else if dag.constant(dag.operand(test, 1)) == Some(1) {
                0
            } else {
                break;
            };
            test = dag.operand(test, at);
        } else {
            test = dag.operand(test, 0);
        }
    }
    if dag.opcode(test) != x86isd::SETCC {
        return None;
    }
    let inner = dag.constant(dag.operand(test, 0))?;
    Some((dag.operand(test, 1), if opposite { inner ^ 1 } else { inner }))
}

/// `combineSetCC` ([L58770][pin]): `cmpeq(trunc(x), C)` is `cmpeq(x, C)` when
/// the upper bits of `x` are zero, in the legal type `x` has: a `zeroext`
/// `i16` argument is compared as the register it arrived in (`cmpl $5, %edi`,
/// not `cmpw`, which takes a 66h prefix). Only the equality forms; the pin
/// leaves `ult` and `ugt` at the narrow type.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L58770-L58782
fn combine_integer_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
    level: CombineLevel,
) -> Option<SDValue> {
    let NodeData::CondCode(cond) = dag.node(dag.operand(node, 2)).data else { return None };
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    if !matches!(cond, CondCode::SetEq | CondCode::SetNe)
        || level == CombineLevel::BeforeLegalizeTypes
        || dag.opcode(lhs) != ISD::Truncate
        || dag.constant(rhs).is_none()
    {
        return None;
    }
    let source = dag.operand(lhs, 0);
    let (source_vt, op_vt) = (dag.value_type(source), dag.value_type(lhs));
    // A type the target holds in a register: not a pair (m16's dword).
    let legal =
        source_vt.size_in_bits() >= 32 && source_vt.size_in_bits() <= dag.config().pointer.size_in_bits().max(32);
    if !legal {
        return None;
    }
    let upper = llrm_codegen::backend::selectiondag::fold::mask(source_vt.size_in_bits())
        & !llrm_codegen::backend::selectiondag::fold::mask(op_vt.size_in_bits());
    if !llrm_codegen::backend::selectiondag::known_bits::masked_value_is_zero(dag, source, upper) {
        return None;
    }
    let constant = dag.get_zext_or_trunc(rhs, source_vt);
    Some(dag.get_setcc(dag.value_type(node), source, constant, cond))
}

/// `combineX86SetCC`: a `setcc` of a boolean test reads the inner flags.
fn combine_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let code = dag.constant(dag.operand(node, 0))?;
    let (flags, code) = check_bool_test(dag, dag.operand(node, 1), code)?;
    let code = cond_code(dag, code);
    Some(dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]))
}

/// `combineCarryThroughADD`: flags of `(add carry, -1)`, where the carry is a
/// `setb` (through extends, truncates and `and 1`), are the flags the `setb`
/// read. DEVIATE SD242: the `COND_A` form (a swapped `sub`) is not
/// translated. Reason: nothing produces it at 32 bits yet. Exit: with
/// `combineSBB`'s remaining folds. Owner: sdag-b. Review 2026-12-10.
fn carry_through_add(
    dag: &SelectionDAG,
    flags: SDValue,
) -> Option<SDValue> {
    if dag.opcode(flags) != x86isd::ADD {
        return None;
    }
    let minus_one = dag.operand(flags, 1);
    if dag.constant(minus_one)
        != Some(llrm_codegen::backend::selectiondag::fold::mask(dag.value_type(minus_one).size_in_bits()))
    {
        return None;
    }
    let mut carry = dag.operand(flags, 0);
    while matches!(
        dag.opcode(carry),
        ISD::Truncate | ISD::ZeroExtend | ISD::AnyExtend
    )
        || (dag.opcode(carry) == ISD::And && dag.constant(dag.operand(carry, 1)) == Some(1))
    {
        carry = dag.operand(carry, 0);
    }
    if !matches!(dag.opcode(carry), x86isd::SETCC | x86isd::SETCC_CARRY) {
        return None;
    }
    let code = dag.constant(dag.operand(carry, 0))?;
    let tested = dag.operand(carry, 1);
    if code == cond::B {
        return Some(tested);
    }
    // A check of the zero flag of an add of 1 (`getX86XALUOOp` tests `COND_E`
    // for it) is the carry flag of that add: `inc; sete; add al, -1; adc` is
    // `add 1; adc`.
    (code == cond::E
        && dag.opcode(tested) == x86isd::ADD
        && dag.constant(dag.operand(tested, 1)) == Some(1))
    .then_some(tested)
}

/// `needCarryOrOverflowFlag`: whether a reader of the flags result `flags`
/// tests a condition that depends on the carry or overflow flag; any reader
/// other than a `setcc`, `brcond` or `cmov` is assumed to.
fn need_carry_or_overflow_flag(
    dag: &SelectionDAG,
    flags: SDValue,
) -> bool {
    dag.users(flags).any(|user| {
        let user = SDValue { node: user, res: 0 };
        let at = match dag.opcode(user) {
            x86isd::SETCC | x86isd::SETCC_CARRY => 0,
            x86isd::BRCOND | x86isd::CMOV => 2,
            _ => return true,
        };
        let Some(code) = dag.constant(dag.operand(user, at)) else { return true };
        matches!(
            code,
            cond::A | cond::AE | cond::B | cond::BE | cond::O | cond::NO | cond::G | cond::GE | cond::L | cond::LE
        )
    })
}

/// `combineADC` and `combineSBB`, the carry-through-add part: the carry in
/// comes from the flags the `setb` read, so `setb; add al, -1; adc` is `adc`.
/// (The `ADC 0, 0` strength reduction is not translated: it replaces both
/// results.)
fn combine_adc_sbb(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (opcode, vt) = (dag.opcode(node), dag.node(node).values.clone());
    let (lhs, rhs, carry) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let flags_used = dag.has_users(SDValue { node: node.node, res: 1 });
    if opcode == x86isd::ADC {
        let (lhs_c, rhs_c) = (dag.constant(lhs), dag.constant(rhs));
        // Canonicalize constant to RHS.
        if lhs_c.is_some() && rhs_c.is_none() {
            return Some(dag.get_node_multi(opcode, &vt, &[rhs, lhs, carry]));
        }
        // Fold ADC(C1,C2,Carry) -> ADC(0,C1+C2,Carry) iff the flag result is
        // dead or C1+C2 doesn't signed/unsigned overflow.
        if let (Some(c1), Some(c2)) = (lhs_c, rhs_c)
            && c1 != 0
        {
            let ty = dag.value_type(lhs);
            let width = ty.size_in_bits();
            let mask = llrm_codegen::backend::selectiondag::fold::mask(width);
            let sum = (c1 + c2) & mask;
            let signed = |v: u128| ((v << (128 - width)) as i128) >> (128 - width);
            let unsigned_overflow = c1 + c2 > mask;
            let signed_overflow = signed(c1).checked_add(signed(c2)).is_none_or(|s| s != signed(sum));
            if !flags_used || (!unsigned_overflow && !signed_overflow) {
                let zero = dag.get_constant(0, ty);
                let sum = dag.get_constant(sum, ty);
                return Some(dag.get_node_multi(opcode, &vt, &[zero, sum, carry]));
            }
        }
    }
    if let Some(flags) = carry_through_add(dag, carry) {
        return Some(dag.get_node_multi(opcode, &vt, &[lhs, rhs, flags]));
    }
    if opcode == x86isd::ADC && dag.constant(rhs) == Some(0) {
        let flags = SDValue { node: node.node, res: 1 };
        // Fold ADC(ADD(X,Y),0,Carry) -> ADC(X,Y,Carry) iff the flag result is
        // dead.
        if dag.opcode(lhs) == ISD::Add && !need_carry_or_overflow_flag(dag, flags) {
            let (x, y) = (dag.operand(lhs, 0), dag.operand(lhs, 1));
            return Some(dag.get_node_multi(opcode, &vt, &[x, y, carry]));
        }
        // Fold ADC(SHL(X,1),0,Carry) -> ADC(X,X,Carry) iff the flag result is
        // dead.
        if dag.opcode(lhs) == ISD::Shl
            && dag.constant(dag.operand(lhs, 1)) == Some(1)
            && !need_carry_or_overflow_flag(dag, flags)
        {
            let x = dag.get_node(ISD::Freeze, dag.value_type(lhs), &[dag.operand(lhs, 0)]);
            return Some(dag.get_node_multi(opcode, &vt, &[x, x, carry]));
        }
    }
    // Fold SBB(SUB(X,Y),0,Carry) -> SBB(X,Y,Carry) iff the flag result is dead.
    if opcode == x86isd::SBB && dag.opcode(lhs) == ISD::Sub && dag.constant(rhs) == Some(0) && !flags_used {
        let (x, y) = (dag.operand(lhs, 0), dag.operand(lhs, 1));
        return Some(dag.get_node_multi(opcode, &vt, &[x, y, carry]));
    }
    None
}

/// `combineBrCond`: the same for a branch.
fn combine_brcond(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let code = dag.constant(dag.operand(node, 2))?;
    let (flags, code) = check_bool_test(dag, dag.operand(node, 3), code)?;
    let code = cond_code(dag, code);
    let (chain, dest) = (dag.operand(node, 0), dag.operand(node, 1));
    Some(dag.get_node(x86isd::BRCOND, EVT::OTHER, &[chain, dest, code, flags]))
}

/// `combineCompareEqual`: `oeq` (`sete` and `setnp`) or `une` (`setne` or `setp`)
/// of one float compare, used as a value, is `cmpeqss`/`cmpneqss`, a `movd`
/// and an `and 1`: three instructions, no flags and no branch. A user that
/// might want the flags (a branch, a select, anything but a register copy or
/// an extend) keeps the `setcc` pair.
///
/// DEVIATE CE1: the pin's AVX-512 form (`FSETCCM`, mask register) and its
/// 32-bit `f64` form (`SCALAR_TO_VECTOR`, a `v4f32` bitcast) are not
/// translated; the first has no target here, the second leaves the pair at
/// m32 SSE2 doubles. Exit 2026-12-08, owner m64.
fn combine_compare_equal(
    dag: &mut SelectionDAG,
    node: SDValue,
    sse2: bool,
    is_64bit: bool,
) -> Option<SDValue> {
    if !sse2 {
        return None;
    }
    let (left, right) = (dag.operand(node, 0), dag.operand(node, 1));
    if dag.opcode(left) != x86isd::SETCC
        || dag.opcode(right) != x86isd::SETCC
        || !dag.has_one_use(left.node)
        || !dag.has_one_use(right.node)
    {
        return None;
    }
    let (cmp0, cmp1) = (dag.operand(left, 1), dag.operand(right, 1));
    if dag.opcode(cmp0) != x86isd::FCMP || cmp0 != cmp1 {
        return None;
    }
    let (lhs, rhs) = (dag.operand(cmp0, 0), dag.operand(cmp0, 1));
    let vt = dag.value_type(lhs);
    if vt != EVT::Simple(MVT::F32) && !(vt == EVT::Simple(MVT::F64) && is_64bit) {
        return None;
    }
    let wants_flags = dag.users(node).any(|user| {
        !matches!(
            dag.opcode(SDValue { node: user, res: 0 }),
            ISD::CopyToReg | ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend
        )
    });
    if wants_flags {
        return None;
    }
    let (mut cc0, mut cc1) = (dag.constant(dag.operand(left, 0))?, dag.constant(dag.operand(right, 0))?);
    if cc1 == cond::E || cc1 == cond::NE {
        std::mem::swap(&mut cc0, &mut cc1);
    }
    let predicate = match (cc0, cc1) {
        (cond::E, cond::NP) => 0,
        (cond::NE, cond::P) => 4,
        _ => return None,
    };
    let imm = dag.get_constant_with(predicate, EVT::I8, true, false);
    let ones_or_zeros = dag.get_node(x86isd::FSETCC, vt, &[lhs, rhs, imm]);
    let int = if vt == EVT::Simple(MVT::F64) { EVT::I64 } else { EVT::I32 };
    let bits = dag.get_node(ISD::BitCast, int, &[ones_or_zeros]);
    let one = dag.get_constant(1, int);
    let anded = dag.get_node(ISD::And, int, &[bits, one]);
    Some(dag.get_node(ISD::Truncate, EVT::I8, &[anded]))
}

/// `combineOr`'s tail: `combineAddOrSubToADCOrSBB` of a disjoint `or`, which is
/// what `(shl x, 1) + (zext setcc)` has become by the time the target sees it.
fn combine_or(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    if !dag.node(node).flags.has(SDNodeFlags::DISJOINT) {
        return None;
    }
    super::x86_carry::add_or_sub_node_to_adc_or_sbb(dag, node)
}

/// `combineOrXorWithSETCC`: `(xor (zext setcc), imm)` with an odd `imm` is a
/// subtract, and `(or (zext setcc), imm)` with an even one an add, of the
/// compare result, which `combineAddOrSubToADCOrSBB` turns into `adc`/`sbb`.
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L54095
fn combine_or_xor_with_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (n0, n1) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    if !vt.is_integer() || dag.opcode(n0) != ISD::ZeroExtend || !dag.has_one_use(n0.node) {
        return None;
    }
    if dag.opcode(dag.operand(n0, 0)) != x86isd::SETCC {
        return None;
    }
    let odd = dag.constant(n1)? & 1 == 1;
    let is_sub = dag.opcode(node) == ISD::Xor;
    if is_sub != odd {
        return None;
    }
    super::x86_carry::add_or_sub_to_adc_or_sbb(dag, is_sub, vt, n1, n0, false)
}

/// `combineAdd`'s `ADD(ADC(Y,0,W),X)` is `ADC(X,Y,W)`, and `ADD(SBB(Y,0,W),C)`
/// is `SBB(Y,-C,W)` (the flags result of either must be dead).
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L60754
fn combine_add_of_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (op0, op1) = (dag.operand(node, 0), dag.operand(node, 1));
    let flags_dead = |dag: &SelectionDAG, n: SDValue| !dag.has_users(SDValue { node: n.node, res: 1 });
    let zero_second = |dag: &SelectionDAG, n: SDValue| dag.constant(dag.operand(n, 1)) == Some(0);
    if dag.opcode(op0) == x86isd::ADC && dag.has_one_use(op0.node) && zero_second(dag, op0) && flags_dead(dag, op0) {
        let vts = dag.node(op0).values.clone();
        let (y, w) = (dag.operand(op0, 0), dag.operand(op0, 2));
        return Some(dag.get_node_multi(x86isd::ADC, &vts, &[op1, y, w]));
    }
    let (sbb, c) = if dag.opcode(op0) == x86isd::SBB { (op0, op1) } else { (op1, op0) };
    if dag.opcode(sbb) == x86isd::SBB && dag.has_one_use(sbb.node) && zero_second(dag, sbb) && flags_dead(dag, sbb) {
        let vts = dag.node(sbb).values.clone();
        let vt = dag.value_type(node);
        let (y, w) = (dag.operand(sbb, 0), dag.operand(sbb, 2));
        let negated = dag.get_negative(c, vt);
        return Some(dag.get_node_multi(x86isd::SBB, &vts, &[y, negated, w]));
    }
    None
}

/// `combineAdd`, the hidden LEA pattern: `(add (sub (shl x, c), y), z)` with
/// `c < 4` is `(sub (add (shl x, c), z), y)`, so the `shl` and the `add` are
/// one `lea` (`y * 7 + z` is `lea z + y*8; sub y`). (The vector, `cmov`,
/// `psadbw` and `sbb` folds are not translated.)
fn combine_add(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I32 && vt != EVT::I64 {
        return combine_add_of_carry(dag, node).or_else(|| super::x86_carry::add_or_sub_node_to_adc_or_sbb(dag, node));
    }
    // `m_Add` matches either operand as the difference.
    let hidden_lea = |difference: SDValue, z: SDValue| {
        if dag.opcode(difference) != ISD::Sub || !dag.has_one_use(difference.node) || dag.constant(z).is_some() {
            return None;
        }
        let (shift, y) = (dag.operand(difference, 0), dag.operand(difference, 1));
        if dag.opcode(shift) != ISD::Shl || !dag.constant(dag.operand(shift, 1)).is_some_and(|amount| amount < 4) {
            return None;
        }
        Some((shift, y, z))
    };
    let (a, b) = (dag.operand(node, 0), dag.operand(node, 1));
    let Some((shift, y, z)) = hidden_lea(a, b).or_else(|| hidden_lea(b, a)) else {
        return combine_add_of_carry(dag, node).or_else(|| super::x86_carry::add_or_sub_node_to_adc_or_sbb(dag, node));
    };
    let sum = dag.get_node(ISD::Add, vt, &[shift, z]);
    Some(dag.get_node(ISD::Sub, vt, &[sum, y]))
}

/// `matchGenericOp`: a generic node made of the same operands reads the
/// result of `node` instead (`negate`: its negation).
fn match_generic_op(
    dag: &mut SelectionDAG,
    opcode: ISD,
    node: SDValue,
    lhs: SDValue,
    rhs: SDValue,
    negate: bool,
) {
    let vt = dag.value_type(node);
    let Some(found) = dag.get_node_if_exists(opcode, &[vt], &[lhs, rhs]) else { return };
    let generic = SDValue { node: found, res: 0 };
    if !dag.has_users(generic) {
        return;
    }
    let mut value = SDValue { node: node.node, res: 0 };
    if negate {
        // A generic node whose one user is also the only user of `node` would
        // make a cycle, or an unprofitable replacement.
        let users: Vec<u32> = dag.users(generic).collect();
        if let ([user], 1) = (&users[..], dag.use_count(found)) {
            let all = (0..dag.node(node).values.len() as u8)
                .all(|res| dag.users(SDValue { node: node.node, res }).all(|other| other == *user));
            if all {
                return;
            }
        }
        value = dag.get_negative(value, vt);
    }
    dag.combine_to(generic, value);
}

/// `combineX86AddSub` (the flags and generic-node parts): an `X86ISD::ADD` or
/// `SUB` whose flags nothing reads is the generic node; one whose flags are
/// read takes over the generic `add`/`sub` of the same operands, and
/// `add x, -c` is `sub x, c`, so `a >= c ? a - c : a` is one `sub` and a
/// branch where it was a `lea` and a `cmp`. (The `ADC`/`SBB` folds and the
/// `combineX86SubCmpForFlags` are not translated.)
fn combine_x86_add_sub(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if !vt.is_integer() {
        return None;
    }
    let is_sub = dag.opcode(node) == x86isd::SUB;
    let generic = if is_sub { ISD::Sub } else { ISD::Add };
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    if !dag.has_users(SDValue { node: node.node, res: 1 }) {
        return Some(dag.get_node(generic, vt, &[lhs, rhs]));
    }
    match_generic_op(dag, generic, node, lhs, rhs, false);
    match_generic_op(dag, generic, node, rhs, lhs, is_sub);
    let mask = llrm_codegen::backend::selectiondag::fold::mask(vt.size_in_bits());
    if let Some(c) = dag.constant(rhs) {
        let negated = dag.get_constant(c.wrapping_neg() & mask, vt);
        if is_sub {
            match_generic_op(dag, ISD::Add, node, lhs, negated, false);
        } else {
            match_generic_op(dag, ISD::Sub, node, negated, lhs, true);
        }
    } else if let (Some(c), true) = (dag.constant(lhs), is_sub) {
        let negated = dag.get_constant(c.wrapping_neg() & mask, vt);
        match_generic_op(dag, ISD::Add, node, rhs, negated, true);
    }
    None
}

/// `combineUIntToFP`: `UINT_TO_FP` is custom, so the generic combine never
/// makes it a `SINT_TO_FP` when the sign bit is clear; here a `nneg` flag or a
/// known clear sign bit does, which is one `fild` of the value itself and not
/// of a 64-bit buffer holding it. (Vector forms are not translated.)
fn combine_uint_to_fp(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let value = dag.operand(node, 0);
    let vt = dag.value_type(node);
    let bits = dag.value_type(value).size_in_bits();
    let non_neg = dag.node(node).flags.has(SDNodeFlags::NON_NEG)
        || llrm_codegen::backend::selectiondag::known_bits::known_bits(dag, value).zero >> (bits - 1) & 1 == 1;
    non_neg.then(|| dag.get_node(ISD::SIntToFp, vt, &[value]))
}

/// The folds of `combineSub` that sit between `combineSubABS` and
/// `combineAddOrSubToADCOrSBB`
/// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L60916>):
/// `SUB(X, ADC(Y, 0, W))` is `SBB(X, Y, W)` (`a - umax(b, 1)` is `cmp b, 1;
/// sbb`, not an `adc` into a register and a `sub`), `SUB(X, SBB(Y, Z, W))` is
/// `SUB(ADC(X, Z, W), Y)`, and `SUB(~A, ~B)` is `SUB(B, A)`.
fn combine_sub_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (op0, op1) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let zero = |dag: &SelectionDAG, value: SDValue| dag.constant(value) == Some(0);
    let one_use = |dag: &SelectionDAG, value: SDValue| dag.has_one_use(value.node);
    if dag.opcode(op1) == x86isd::ADC && one_use(dag, op1) && zero(dag, dag.operand(op1, 1)) {
        let (y, flags) = (dag.operand(op1, 0), dag.operand(op1, 2));
        return Some(dag.get_node_multi(x86isd::SBB, &[vt, EVT::I32], &[op0, y, flags]));
    }
    if dag.opcode(op1) == x86isd::SBB && one_use(dag, op1) && !(zero(dag, op0) && zero(dag, dag.operand(op1, 1))) {
        let (y, z, flags) = (dag.operand(op1, 0), dag.operand(op1, 1), dag.operand(op1, 2));
        let adc = dag.get_node_multi(x86isd::ADC, &[vt, EVT::I32], &[op0, z, flags]);
        return Some(dag.get_node(ISD::Sub, vt, &[adc, y]));
    }
    let not_of = |dag: &SelectionDAG, value: SDValue| -> Option<SDValue> {
        let all_ones = llrm_codegen::backend::selectiondag::fold::mask(dag.value_type(value).size_in_bits());
        (dag.opcode(value) == ISD::Xor && dag.constant(dag.operand(value, 1)) == Some(all_ones) && one_use(dag, value))
            .then(|| dag.operand(value, 0))
    };
    if let (Some(a), Some(b)) = (not_of(dag, op0), not_of(dag, op1)) {
        return Some(dag.get_node(ISD::Sub, vt, &[b, a]));
    }
    None
}

/// `combineSub` ([L60916][pin]): `sub C1, (xor X, C2)` is `add (xor X, ~C2),
/// C1 + 1` ([`combine_sub_xor_constant`]), the `SUB(X, ADC(Y,0,W))`,
/// `SUB(X, SBB(Y,Z,W))` and `~A - ~B` folds ([`combine_sub_carry`]),
/// `combineAddOrSubToADCOrSBB` ([`super::x86_carry`]), then `combineSubSetcc`.
///
/// DEVIATE SD448: where `lea_adds`, [`combine_masked_xor_difference`] runs
/// before the pin's first fold, not after it as the pin has no such fold: for a
/// low-bit mask the pin's `xor`+`add` is two instructions where it is one.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L60916
fn combine_sub(
    dag: &mut SelectionDAG,
    node: SDValue,
    lea_adds: bool,
) -> Option<SDValue> {
    lea_adds
        .then(|| combine_masked_xor_difference(dag, node))
        .flatten()
        .or_else(|| combine_sub_xor_constant(dag, node))
        .or_else(|| combine_sub_carry(dag, node))
        .or_else(|| super::x86_carry::add_or_sub_node_to_adc_or_sbb(dag, node))
        .or_else(|| combine_sub_setcc(dag, node))
}

/// The first fold of `combineSub`: x86 cannot encode an immediate left side of
/// a `sub`, so `sub C1, (xor X, C2)` (the `xor` used once, `C1` not zero: that
/// is a `neg`) becomes `add (xor X, ~C2), C1 + 1`, which saves a register.
/// `100 - (n ^ 0xF0)` is then `(n ^ -241) + 101`.
fn combine_sub_xor_constant(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let (minuend, xor) = (dag.operand(node, 0), dag.operand(node, 1));
    let c1 = dag.constant(minuend).filter(|&c| c != 0 && !dag.is_opaque(minuend))?;
    if dag.opcode(xor) != ISD::Xor || !dag.has_one_use(xor.node) {
        return None;
    }
    let mask = dag.operand(xor, 1);
    let c2 = dag.constant(mask).filter(|_| !dag.is_opaque(mask))?;
    let bits = llrm_codegen::backend::selectiondag::fold::mask(vt.size_in_bits());
    let inverted = dag.get_constant(!c2 & bits, vt);
    let new_xor = dag.get_node(ISD::Xor, vt, &[dag.operand(xor, 0), inverted]);
    let bumped = dag.get_constant(c1.wrapping_add(1) & bits, vt);
    Some(dag.get_node(ISD::Add, vt, &[new_xor, bumped]))
}

/// DEVIATE SD448 (owner x87; recheck 2026-11-20 and at every pin bump,
/// dropped where the pin has the fold): `sub C, (xor X, M)` is `add X, C - M`
/// when `M` is a low-bit mask and `X` has no bits above it, so the `xor` is
/// `M - X`. `__floatsisf`'s shift amount `26 - (n ^ 31)` is `n - 5`: one
/// `lea` where the `sub` took `mov cl, 26; sub cl, bl`. The pin has no such
/// fold, and its `convertToThreeAddressWithLEA` takes no 8-bit add on 32-bit
/// targets (TA-10). 0 larger cells over 18 row-levels; m32 -O2 2 smaller,
/// -O3 9 smaller (to -32 B); m64 and arm64 identical. Where the target has
/// no three-address add (m16) the `add` needs a copy and is a byte longer,
/// so it is not made there.
fn combine_masked_xor_difference(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let (minuend, xor) = (dag.operand(node, 0), dag.operand(node, 1));
    let c = dag.constant(minuend)?;
    if dag.opcode(xor) != ISD::Xor {
        return None;
    }
    let (x, m) = (dag.operand(xor, 0), dag.constant(dag.operand(xor, 1))?);
    if m == 0 || m & m.wrapping_add(1) != 0 {
        return None;
    }
    let mask = llrm_codegen::backend::selectiondag::fold::mask(vt.size_in_bits());
    if (m | llrm_codegen::backend::selectiondag::known_bits::known_bits(dag, x).zero) & mask != mask {
        return None;
    }
    let delta = dag.get_constant(c.wrapping_sub(m) & mask, vt);
    Some(dag.get_node(ISD::Add, vt, &[x, delta]))
}

/// `combineSubSetcc` ([L60838][pin]): `sub C, zext(setcc)` is
/// `zext(setcc inverted) + (C - 1)` for a nonzero `C` (`2 - (a < b)` is
/// `(a >= b) + 1`: a `setge` and an `inc`, not a load of the constant and a
/// `sub`).
///
/// DEVIATE SW1 (owner w9, recheck 2026-11-20 against gcc and clang): at `i8`
/// the setcc is the operand itself, with no `zext` for the pin's match to see
/// (`c ? 5 : 6` is `setl cl; mov al, 6; sub al, cl`). The inverted form is
/// `setge al; add al, 5`: m64 -Os select_of_constants 12987 B with the old
/// selector against 14506. Exit: the pin matches it, or the sweep shows a
/// cell larger.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L60838
fn combine_sub_setcc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let (op0, op1) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let constant = dag.constant(op0).filter(|c| *c != 0)?;
    let bare = vt == EVT::I8 && dag.opcode(op1) == x86isd::SETCC;
    if !bare && (dag.opcode(op1) != ISD::ZeroExtend || !dag.has_one_use(op1.node)) {
        return None;
    }
    let setcc = if bare { op1 } else { dag.operand(op1, 0) };
    if dag.opcode(setcc) != x86isd::SETCC || !dag.has_one_use(setcc.node) {
        return None;
    }
    let code = dag.constant(dag.operand(setcc, 0))?;
    let opposite = cond_code(dag, code ^ 1);
    let inverted = dag.get_node(x86isd::SETCC, EVT::I8, &[opposite, dag.operand(setcc, 1)]);
    let widened = if bare { inverted } else { dag.get_node(ISD::ZeroExtend, vt, &[inverted]) };
    let adjusted = dag.get_constant(constant.wrapping_sub(1), vt);
    Some(dag.get_node(ISD::Add, vt, &[widened, adjusted]))
}

/// `combineSelectOfTwoConstants` ([L48675][pin]): an `i1` select between two
/// constants whose difference is a power of two, or 3, 5, 9 at `i32`/`i64`, is
/// the condition widened, multiplied by the difference and added to the false
/// constant: `c ? 48 : 32` is `zext(c) * 16 + 32` (`setl; shl 4; or 32`),
/// where the `cmov` of two `mov`s was both larger and slower. `is64` is
/// whether `i64` is legal.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L48675
fn combine_select_of_two_constants(
    dag: &mut SelectionDAG,
    node: SDValue,
    is64: bool,
) -> Option<SDValue> {
    let (mut cond, then, other) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let (mut true_c, mut false_c) = (dag.constant(then)?, dag.constant(other)?);
    let vt = dag.value_type(node);
    // Not "crazy integer types", and the condition is `i1`.
    if !(vt == EVT::I8 || vt == EVT::I16 || vt == EVT::I32 || (vt == EVT::I64 && is64))
        || dag.value_type(cond) != EVT::I1
    {
        return None;
    }
    let bits = vt.size_in_bits();
    let mask = llrm_codegen::backend::selectiondag::fold::mask(bits);
    let signed = |value: u128| -> i128 {
        let shift = 128 - bits;
        ((value << shift) as i128) >> shift
    };
    let (true_value, false_value) = (signed(true_c & mask), signed(false_c & mask));
    // `(X == 0) ? Y : -1` has a better lowering with `sbb`.
    if (true_c & mask == mask || false_c & mask == mask) && dag.opcode(cond) == ISD::SetCC {
        let zero = dag.constant(dag.operand(cond, 1)) == Some(0);
        let NodeData::CondCode(code) = dag.node(dag.operand(cond, 2)).data else { return None };
        if zero && matches!(code, CondCode::SetEq | CondCode::SetNe) {
            return None;
        }
    }
    let difference = true_value.checked_sub(false_value)?;
    if !(-(1i128 << (bits - 1))..(1i128 << (bits - 1))).contains(&difference) {
        return None;
    }
    let absolute = difference.unsigned_abs();
    if !(absolute.is_power_of_two() || ((vt == EVT::I32 || vt == EVT::I64) && matches!(absolute, 3 | 5 | 9))) {
        return None;
    }
    // A positive multiplier shifts or `lea`s; the `not` folds into the compare.
    let mut false_op = other;
    if true_value < false_value {
        cond = dag.get_not(cond, EVT::I1);
        std::mem::swap(&mut true_c, &mut false_c);
        false_op = then;
    }
    let mut result = dag.get_node(ISD::ZeroExtend, vt, &[cond]);
    if absolute != 1 {
        let scale = dag.get_constant(absolute, vt);
        result = dag.get_node(ISD::Mul, vt, &[result, scale]);
    }
    if false_c & mask != 0 {
        result = dag.get_node(ISD::Add, vt, &[result, false_op]);
    }
    Some(result)
}

/// `combineCMov`, up to the constant selects: `cmov x, x` is `x`; a select
/// between two integer constants becomes the condition, widened, shifted,
/// added to or scaled: `c ? 8 : 0` is `zext(setcc) << 3`, `c ? k+1 : k` is
/// `zext(setcc) + k`, and a difference of 2, 3, 4, 5, 8 or 9 (one `lea`) is
/// `zext(setcc) * d + k`. (`combineSetCCEFLAGS`, the `x == c` register form,
/// the `adc` form and the double `cmov` are not translated, SD200.)
fn combine_cmov(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let (mut false_op, mut true_op) = (dag.operand(node, 0), dag.operand(node, 1));
    let mut code = dag.constant(dag.operand(node, 2))?;
    let flags = dag.operand(node, 3);
    if true_op == false_op {
        return Some(true_op);
    }
    // The flags of a boolean test are the inner compare's.
    if let Some((flags, simpler)) = check_bool_test(dag, flags, code) {
        let simpler = cond_code(dag, simpler);
        return Some(dag.get_node(x86isd::CMOV, vt, &[false_op, true_op, simpler, flags]));
    }
    // `(cmov 1, T, uge, (sub T, 2))` is `T + (T < 1)`: `adc T, 0` on the carry
    // of `sub T, 1` (X86ISelLowering.cpp#L50411, the `umax(x, 1)` the
    // soft-float runtime builds its exponents with).
    if code == cond::AE
        && dag.constant(false_op) == Some(1)
        && dag.opcode(flags) == x86isd::SUB
        && dag.has_one_use(flags.node)
    {
        let mut tested = dag.operand(flags, 0);
        if dag.opcode(tested) == ISD::Truncate {
            tested = dag.operand(tested, 0);
        }
        if tested == true_op && dag.constant(dag.operand(flags, 1)) == Some(2) {
            let cond_vt = dag.value_type(dag.operand(flags, 0));
            let one = dag.get_constant(1, cond_vt);
            let sub = dag.get_node_multi(x86isd::SUB, &[cond_vt, EVT::I32], &[dag.operand(flags, 0), one]);
            let zero = dag.get_constant(0, vt);
            return Some(dag.get_node_multi(
                x86isd::ADC,
                &[vt, EVT::I32],
                &[true_op, zero, SDValue { node: sub.node, res: 1 }],
            ));
        }
    }
    let (Some(mut true_c), Some(mut false_c)) = (dag.constant(true_op), dag.constant(false_op)) else { return None };
    if !vt.is_integer() {
        return None;
    }
    // The true value is the larger one.
    if true_c < false_c {
        code ^= 1;
        std::mem::swap(&mut true_c, &mut false_c);
        std::mem::swap(&mut true_op, &mut false_op);
    }
    let mask = llrm_codegen::backend::selectiondag::fold::mask(vt.size_in_bits());
    let difference = true_c.wrapping_sub(false_c) & mask;
    let widened = |dag: &mut SelectionDAG| {
        let code = cond_code(dag, code);
        let test = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
        if vt == EVT::I8 { test } else { dag.get_node(ISD::ZeroExtend, vt, &[test]) }
    };
    if false_c == 0 && true_c.is_power_of_two() {
        let test = widened(dag);
        let by = dag.get_constant(u128::from(true_c.trailing_zeros()), EVT::I8);
        return Some(dag.get_node(ISD::Shl, vt, &[test, by]));
    }
    if false_c + 1 == true_c {
        let test = widened(dag);
        return Some(dag.get_node(ISD::Add, vt, &[test, false_op]));
    }
    if vt == EVT::I32 && matches!(difference, 1 | 2 | 3 | 4 | 5 | 8 | 9) {
        let mut result = widened(dag);
        if difference != 1 {
            let scale = dag.get_constant(difference, vt);
            result = dag.get_node(ISD::Mul, vt, &[result, scale]);
        }
        if false_c != 0 {
            result = dag.get_node(ISD::Add, vt, &[result, false_op]);
        }
        return Some(result);
    }
    None
}

/// `getGlobalWrapperKind`: a symbol is addressed from `%rip` in 64-bit
/// position-independent code (small code model), the absolute `Wrapper`
/// elsewhere.
pub(super) fn wrapper_kind(
    dag: &SelectionDAG,
    vt: EVT,
) -> ISD {
    if vt == EVT::I64 && !dag.config().absolute_symbols { x86isd::WRAPPER_RIP } else { x86isd::WRAPPER }
}

/// `LowerGlobalOrExternal` for a static, non-PIC global: `Wrapper
/// (TargetGlobalAddress sym+offset)`. A non-negative offset folds into the
/// symbol; a negative one is added after.
fn lower_global_address(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::Global { symbol, offset } = dag.node(node).data else { return None };
    let vt = dag.value_type(node);
    // `classifyGlobalReference`: a symbol that is not `dso_local` in position
    // independent 64-bit code is `MO_GOTPCREL`; `LowerGlobalOrExternal` then
    // loads its address from the GOT and adds the offset after (no folding).
    if vt == EVT::I64
        && !dag.config().absolute_symbols
        && !dag.local_symbols.get(symbol as usize).copied().unwrap_or(true)
    {
        let target = dag.intern(
            ISD::TargetGlobalAddress,
            &[vt],
            &[],
            SDNodeFlags::default(),
            NodeData::Global { symbol, offset: 0 },
        );
        let entry = dag.get_node(x86isd::WRAPPER_RIP, vt, &[target]);
        let info =
            MemInfo { mem_vt: vt, align: 8, volatile: false, base: MemBase::Got, ext: None, space: 0, ordering: None };
        let root = dag.entry_node();
        let load = dag.get_load(vt, root, entry, info);
        dag.invariant_loads.insert(load.node);
        dag.got_loads.insert(load.node);
        return Some(if offset == 0 {
            load
        } else {
            let delta = dag.get_constant(offset as u128, vt);
            dag.get_node(ISD::Add, vt, &[load, delta])
        });
    }
    let (folded, rest) = if offset >= 0 && offset <= i64::from(i32::MAX) { (offset, 0) } else { (0, offset) };
    let target = dag.intern(
        ISD::TargetGlobalAddress,
        &[vt],
        &[],
        SDNodeFlags::default(),
        NodeData::Global { symbol, offset: folded },
    );
    let wrapped = dag.get_node(wrapper_kind(dag, vt), vt, &[target]);
    Some(if rest == 0 {
        wrapped
    } else {
        let delta = dag.get_constant(rest as u128, vt);
        dag.get_node(ISD::Add, vt, &[wrapped, delta])
    })
}

/// `LowerConstantPool`: `Wrapper (TargetConstantPool)`.
fn lower_constant_pool(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::PoolIndex(index, offset) = dag.node(node).data else { return None };
    let vt = dag.value_type(node);
    let target =
        dag.intern(ISD::TargetConstantPool, &[vt], &[], SDNodeFlags::default(), NodeData::PoolIndex(index, offset));
    Some(dag.get_node(wrapper_kind(dag, vt), vt, &[target]))
}

/// `LowerJumpTable`: `Wrapper (TargetJumpTable)`.
fn lower_jump_table(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let NodeData::JumpTable(index) = dag.node(node).data else { return None };
    let vt = dag.value_type(node);
    let target = dag.intern(ISD::TargetJumpTable, &[vt], &[], SDNodeFlags::default(), NodeData::JumpTable(index));
    Some(dag.get_node(wrapper_kind(dag, vt), vt, &[target]))
}

/// `LowerXALUO`: the flags of `X86ISD::ADD`/`SUB`/`UMUL`/`SMUL`, and the
/// overflow as a `setcc` of them (`B` for the unsigned add and subtract, `E`
/// for an add of one, `O` for the rest), merged with the result.
fn lower_xalu(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let vt = dag.value_type(node);
    let (opcode, code) = match dag.opcode(node) {
        ISD::UAddO => (x86isd::ADD, if dag.constant(rhs) == Some(1) { cond::E } else { cond::B }),
        ISD::USubO => (x86isd::SUB, cond::B),
        ISD::SAddO => (x86isd::ADD, cond::O),
        ISD::SSubO => (x86isd::SUB, cond::O),
        ISD::UMulO => (x86isd::UMUL, cond::B),
        _ => (x86isd::SMUL, cond::B),
    };
    let value = dag.get_node_multi(opcode, &[vt, EVT::I32], &[lhs, rhs]);
    let flags = SDValue { node: value.node, res: 1 };
    let code = cond_code(dag, code);
    let overflow = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
    dag.get_node_multi(ISD::MergeValues, &[vt, EVT::I8], &[value, overflow])
}

/// `LowerADDSUBO_CARRY`: the carry in is put into the flags (`carry + -1`),
/// then `ADC`/`SBB` add or subtract it; the carry out is `B` of its flags
/// (`O` for the signed forms).
fn lower_addsub_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs, carry) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let vt = dag.value_type(node);
    let carry_vt = dag.value_type(carry);
    let ones = dag.get_all_ones_constant(carry_vt);
    let set_carry = dag.get_node_multi(x86isd::ADD, &[carry_vt, EVT::I32], &[carry, ones]);
    let flags_in = SDValue { node: set_carry.node, res: 1 };
    let kind = dag.opcode(node);
    let opcode = if matches!(kind, ISD::UAddOCarry | ISD::SAddOCarry) { x86isd::ADC } else { x86isd::SBB };
    let sum = dag.get_node_multi(opcode, &[vt, EVT::I32], &[lhs, rhs, flags_in]);
    let flags = SDValue { node: sum.node, res: 1 };
    let signed = matches!(kind, ISD::SAddOCarry | ISD::SSubOCarry);
    let code = cond_code(dag, if signed { cond::O } else { cond::B });
    let carry_out = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
    dag.get_node_multi(ISD::MergeValues, &[vt, EVT::I8], &[sum, carry_out])
}

/// `LowerSETCCCARRY`: the carry in is put into the flags (`carry + -1`), the
/// high halves are subtracted with it (`SBB`), and the condition is read from
/// the flags.
fn lower_setcc_carry(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lhs, rhs, carry) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let NodeData::CondCode(cc) = dag.node(dag.operand(node, 3)).data else { unreachable!("a SETCCCARRY condition") };
    // `TranslateIntegerX86CC` has no constant rewrites: a stand-in that is
    // no constant keeps `translate_cc` from making any.
    let mut plain = carry;
    let x86_cc = translate_cc(dag, cc, &mut plain);
    let code = cond_code(dag, x86_cc);
    let carry_vt = dag.value_type(carry);
    let ones = dag.get_all_ones_constant(carry_vt);
    let set_carry = dag.get_node_multi(x86isd::ADD, &[carry_vt, EVT::I32], &[carry, ones]);
    let flags_in = SDValue { node: set_carry.node, res: 1 };
    let vt = dag.value_type(lhs);
    let sub = dag.get_node_multi(x86isd::SBB, &[vt, EVT::I32], &[lhs, rhs, flags_in]);
    let flags = SDValue { node: sub.node, res: 1 };
    dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags])
}

/// The two halves of `expandFP_TO_UINT` for an `i64`: `value` less 2^63 when it
/// is at least that, and the sign bit (`1 << 63`) to put back when it was.
fn unsigned_fixup_operands(
    dag: &mut SelectionDAG,
    value: SDValue,
    float: MVT,
) -> (SDValue, SDValue) {
    let value_vt = dag.value_type(value);
    // 2^63, the first value that does not fit a signed `i64`.
    let threshold = match float {
        MVT::F32 => 0x5F00_0000u128,
        MVT::F64 => 0x43E0_0000_0000_0000,
        _ => (0x403Eu128 << 64) | (1u128 << 63),
    };
    let threshold = dag.get_constant_fp(threshold, value_vt);
    let at_least = dag.get_setcc(EVT::I8, value, threshold, CondCode::SetGe);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I64, &[at_least]);
    let sixty_three = dag.get_constant(63, EVT::I8);
    let adjust = dag.get_node(ISD::Shl, EVT::I64, &[wide, sixty_three]);
    let zero = dag.get_constant_fp(0, value_vt);
    let offset = dag.get_select(value_vt, at_least, threshold, zero);
    (dag.get_node(ISD::FSub, value_vt, &[value, offset]), adjust)
}

/// `LowerFP_TO_INT` for an unsigned `i64` at m64 or `i32` at m32 from an SSE
/// float (non-strict): `cvttss2si`/`cvttsd2si` of the value (`small`) and of
/// the value less 2^63 or 2^31 (`big`). The instruction gives the sign bit
/// for a value it cannot hold, so `small >> 63` is all ones exactly when
/// `big` is the answer, and the result is `small | (big & that)` -- no
/// compare, no branch.
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L22414-L22449>
/// The pin lays the float in a vector first (`SCALAR_TO_VECTOR`) for
/// `X86ISD::CVTTS2SI`; here it is `FP_TO_SINT`, which selects the same
/// `cvttsd2si` (the convention of `llrm-dagisel`'s `x86_fplogic`).
fn small_and_big(
    dag: &mut SelectionDAG,
    value: SDValue,
    float: MVT,
    result: EVT,
) -> SDValue {
    let value_vt = dag.value_type(value);
    let bits: u128 = if result == EVT::I64 { 64 } else { 32 };
    // `UINT_TO_FP(1 << (bits - 1))` in the source type.
    let offset = match (float, bits) {
        (MVT::F32, 64) => 0x5F00_0000u128,
        (MVT::F64, 64) => 0x43E0_0000_0000_0000,
        (MVT::F32, _) => 0x4F00_0000,
        _ => 0x41E0_0000_0000_0000,
    };
    let offset = dag.get_constant_fp(offset, value_vt);
    let small = dag.get_node(ISD::FpToSInt, result, &[value]);
    let reduced = dag.get_node(ISD::FSub, value_vt, &[value, offset]);
    let big = dag.get_node(ISD::FpToSInt, result, &[reduced]);
    let shift = dag.get_constant(bits - 1, EVT::I8);
    let overflown = dag.get_node(ISD::Sra, result, &[small, shift]);
    let kept = dag.get_node(ISD::And, result, &[big, overflown]);
    dag.get_node(ISD::Or, result, &[small, kept])
}

/// `FP_TO_INTHelper` for the x87: `fist` of the value to a stack slot of the
/// result's size, then a load of it. An unsigned `i32` goes through an `i64`
/// slot; an unsigned `i64` subtracts 2^63 from a value at or above it and
/// flips the sign bit of the result back.
fn lower_fp_to_int(
    dag: &mut SelectionDAG,
    node: SDValue,
    sse2: bool,
) -> Option<SDValue> {
    let signed = dag.opcode(node) == ISD::FpToSInt;
    let result_vt = dag.value_type(node);
    let mut value = dag.operand(node, 0);
    let value_vt = dag.value_type(value);
    let EVT::Simple(float @ (MVT::F32 | MVT::F64 | MVT::F80)) = value_vt else { return None };
    if sse2 && matches!(float, MVT::F32 | MVT::F64) {
        // `cvttss2si`/`cvttsd2si` truncate to i32 (and i64 at m64); the other
        // widths convert through the next one up.
        let pointer = dag.config().pointer;
        if !signed && matches!(
            (result_vt, pointer),
            (EVT::I64, EVT::I64) | (EVT::I32, EVT::I32)
        ) {
            return Some(small_and_big(dag, value, float, result_vt));
        }
        let via = match (signed, result_vt) {
            (true, EVT::I32) => return Some(node),
            (true, EVT::I64) if pointer == EVT::I64 => return Some(node),
            (true, _) => Some(EVT::I32),
            (false, EVT::I64) => None,
            (false, _) if pointer == EVT::I64 => Some(EVT::I64),
            (false, _) => None,
        };
        if let Some(via) = via {
            let wide = dag.get_node(ISD::FpToSInt, via, &[value]);
            return Some(dag.get_node(ISD::Truncate, result_vt, &[wide]));
        }
    }
    let unsigned_fixup = !signed && result_vt == EVT::I64;
    let stored_vt = if !signed && result_vt != EVT::I64 { EVT::I64 } else { result_vt };
    let size = stored_vt.size_in_bits() / 8;
    let index = dag.create_stack_temporary(size, size);
    let slot = dag.get_frame_index(dag.config().pointer, index);
    let chain = dag.entry_node();
    let mut adjust = None;
    if unsigned_fixup {
        let (shifted, flip) = unsigned_fixup_operands(dag, value, float);
        value = shifted;
        adjust = Some(flip);
    }
    let info = MemInfo {
        mem_vt: stored_vt,
        align: size,
        volatile: false,
        base: MemBase::Stack(index),
        ext: None,
        space: 0,
        ordering: None,
    };
    let fist = dag.intern(
        x86isd::FP_TO_INT_IN_MEM,
        &[EVT::OTHER],
        &[chain, value, slot],
        SDNodeFlags::default(),
        NodeData::Mem(info),
    );
    let load_info = MemInfo { mem_vt: result_vt, ..info };
    let mut result = dag.get_load(result_vt, fist, slot, load_info);
    if let Some(adjust) = adjust {
        result = dag.get_node(ISD::Xor, EVT::I64, &[result, adjust]);
    }
    Some(result)
}

/// `LRINT_LLRINTHelper` for the x87: `fist` of the value, in the current
/// rounding mode, to a stack slot of the result's size, then a load of it.
fn lower_lrint(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let result_vt = dag.value_type(node);
    let value = dag.operand(node, 0);
    let EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80) = dag.value_type(value) else { return None };
    let size = result_vt.size_in_bits() / 8;
    let index = dag.create_stack_temporary(size, size);
    let slot = dag.get_frame_index(dag.config().pointer, index);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: result_vt,
        align: size,
        volatile: false,
        base: MemBase::Stack(index),
        ext: None,
        space: 0,
        ordering: None,
    };
    let fist =
        dag.intern(x86isd::FIST, &[EVT::OTHER], &[chain, value, slot], SDNodeFlags::default(), NodeData::Mem(info));
    Some(dag.get_load(result_vt, fist, slot, info))
}

/// `BuildFILD` without SSE: `fild` of the `src_vt` integer at `slot`.
fn build_fild(
    dag: &mut SelectionDAG,
    dst: EVT,
    src_vt: EVT,
    chain: SDValue,
    slot: SDValue,
    info: MemInfo,
) -> SDValue {
    let info = MemInfo { mem_vt: src_vt, ..info };
    dag.intern(x86isd::FILD, &[dst, EVT::OTHER], &[chain, slot], SDNodeFlags::default(), NodeData::Mem(info))
}

/// `LowerSINT_TO_FP` and `LowerUINT_TO_FP` for the x87: the integer stored to
/// a stack slot and `fild`ed. An unsigned `i32` is stored with a zero high
/// dword and `fild`ed as an `i64`; an unsigned `i64` is `fild`ed signed and
/// 2^64 is added when the sign was set (the constant pool's fudge).
fn lower_int_to_fp(
    dag: &mut SelectionDAG,
    node: SDValue,
    sse2: bool,
) -> Option<SDValue> {
    let signed = dag.opcode(node) == ISD::SIntToFp;
    let dst = dag.value_type(node);
    let src = dag.operand(node, 0);
    let src_vt = dag.value_type(src);
    if !matches!(dst, EVT::Simple(MVT::F32 | MVT::F64 | MVT::F80)) || !src_vt.is_integer() {
        return None;
    }
    // The combiner leaves a custom `UINT_TO_FP` alone, so `LowerUINT_TO_FP`
    // makes it signed itself when the sign bit is known clear: an `i16`
    // promoted by zero extension is one `fild` of a dword.
    if !signed && combine_uint_to_fp(dag, node).is_some() {
        return Some(dag.get_node(ISD::SIntToFp, dst, &[src]));
    }
    let pointer = dag.config().pointer;
    if sse2 && matches!(dst, EVT::Simple(MVT::F32 | MVT::F64)) {
        let in_reg = |bits| src_vt.size_in_bits() == bits;
        if signed && (in_reg(32) || (in_reg(64) && pointer == EVT::I64)) {
            return Some(node);
        }
        if signed && in_reg(16) {
            let wide = dag.get_node(ISD::SignExtend, EVT::I32, &[src]);
            return Some(dag.get_node(ISD::SIntToFp, dst, &[wide]));
        }
        if !signed && in_reg(32) && pointer == EVT::I64 {
            let wide = dag.get_node(ISD::ZeroExtend, EVT::I64, &[src]);
            return Some(dag.get_node(ISD::SIntToFp, dst, &[wide]));
        }
        // `LowerUINT_TO_FP`: a 64-bit `u64` to `f32` or `f64` is `Expand`
        // (`return SDValue()`), the legalizer's signed convert of the halved
        // value, doubled. The x87 `fild` and its 2^64 fudge below are for the
        // targets without a signed 64-bit SSE convert (DEVIATE UITOFP-F64:
        // the pin's `f64` is an SSE2 vector sequence, see `expand_uint_to_fp`).
        if !signed && in_reg(64) && pointer == EVT::I64 {
            return None;
        }
    }
    // DEVIATE SD332 (exit: the pin gains it; re-check at each pin bump): a
    // plain one-use load of an `i16` or `i32` is `fild`ed from its own
    // address, as combineSIntToFP does for `i64` only. The stack slot's store
    // and reload are gone and `fimul`/`fiadd` read the operand in place.
    if signed && matches!(src_vt, EVT::I16 | EVT::I32) && is_non_ext_load(dag, src) && dag.users(src).count() == 1 {
        let NodeData::Mem(info) = dag.node(src).data else { return None };
        if !info.volatile && info.ordering.is_none() && info.mem_vt == src_vt {
            let (load_chain, address) = (dag.operand(src, 0), dag.operand(src, 1));
            let fild = build_fild(dag, dst, src_vt, load_chain, address, info);
            dag.replace_all_uses_with(SDValue { node: src.node, res: 1 }, SDValue { node: fild.node, res: 1 });
            return Some(fild);
        }
    }
    let chain = dag.entry_node();
    let store_info = |index: i32, vt: EVT, size: u32| MemInfo {
        mem_vt: vt,
        align: size,
        volatile: false,
        base: MemBase::Stack(index),
        ext: None,
        space: 0,
        ordering: None,
    };
    if signed || (src_vt != EVT::I32 && src_vt != EVT::I64) {
        let size = src_vt.size_in_bits() / 8;
        let index = dag.create_stack_temporary(size, size);
        let slot = dag.get_frame_index(pointer, index);
        let info = store_info(index, src_vt, size);
        let stored = dag.get_store(chain, src, slot, info);
        return Some(build_fild(dag, dst, src_vt, stored, slot, info));
    }
    let index = dag.create_stack_temporary(8, 8);
    let slot = dag.get_frame_index(pointer, index);
    let info = store_info(index, EVT::I32, 8);
    if src_vt == EVT::I32 {
        let high_slot = {
            let four = dag.get_constant(4, pointer);
            dag.get_node(ISD::Add, pointer, &[slot, four])
        };
        let low = dag.get_store(chain, src, slot, info);
        let zero = dag.get_constant(0, EVT::I32);
        let high = dag.get_store(low, zero, high_slot, info);
        return Some(build_fild(dag, dst, EVT::I64, high, slot, info));
    }
    let info = store_info(index, EVT::I64, 8);
    let stored = dag.get_store(chain, src, slot, info);
    let fild = build_fild(dag, EVT::Simple(MVT::F80), EVT::I64, stored, slot, info);
    let zero = dag.get_constant(0, EVT::I64);
    let sign_set = dag.get_setcc(EVT::I8, src, zero, CondCode::SetLt);
    // The fudge: 0.0f at +0 and 2^64 as a float at +4 of one pool entry.
    let fudge = dag.get_constant_pool(pointer, 0x5F80_0000_0000_0000, EVT::I64, false);
    let (zero_offset, four_offset) = (dag.get_constant(0, pointer), dag.get_constant(4, pointer));
    let offset = dag.get_select(pointer, sign_set, four_offset, zero_offset);
    let fudge_ptr = dag.get_node(ISD::Add, pointer, &[fudge, offset]);
    let NodeData::PoolIndex(entry, _) = dag.node(fudge).data else { return None };
    let fudge_info = MemInfo {
        mem_vt: EVT::Simple(MVT::F32),
        align: 8,
        volatile: false,
        base: MemBase::ConstantPool(entry),
        ext: Some(llrm_codegen::backend::selectiondag::actions::LoadExt::ExtLoad),
        space: 0,
        ordering: None,
    };
    let fudge_value = dag.get_load(EVT::Simple(MVT::F80), SDValue { node: fild.node, res: 1 }, fudge_ptr, fudge_info);
    let sum = dag.get_node(ISD::FAdd, EVT::Simple(MVT::F80), &[fild, fudge_value]);
    let rounded = if dst == EVT::Simple(MVT::F80) {
        sum
    } else {
        let flag = dag.get_constant_with(0, pointer, true, false);
        dag.get_node(ISD::FpRound, dst, &[sum, flag])
    };
    Some(rounded)
}

/// `LowerVASTART`: at 32 bits a `va_list` is a pointer, so the start is a
/// store of the first variadic argument's address through it; the SysV
/// x86-64 one is `{gp_offset, fp_offset, overflow_arg_area, reg_save_area}`.
fn lower_vastart(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let index = dag.var_args_frame_index?;
    let (chain, list) = (dag.operand(node, 0), dag.operand(node, 1));
    let pointer = dag.config().pointer;
    let base = match dag.node(list).data {
        NodeData::FrameIndex(slot) => MemBase::Stack(slot),
        _ => MemBase::Unknown,
    };
    // Win64's `va_list` is the pointer alone (`LowerWin64_VASTART`).
    if pointer != EVT::I64 || dag.var_args_save.is_none() {
        let first = dag.get_frame_index(pointer, index);
        let info = MemInfo {
            mem_vt: pointer,
            align: pointer.size_in_bits() / 8,
            volatile: false,
            base,
            ext: None,
            space: 0,
            ordering: None,
        };
        return Some(dag.get_store(chain, first, list, info));
    }
    let (save, gp_offset, fp_offset) = dag.var_args_save?;
    let mut stores = Vec::new();
    let fields: [(u128, EVT, SDValue); 4] = [
        (0, EVT::I32, dag.get_constant(u128::from(gp_offset), EVT::I32)),
        (4, EVT::I32, dag.get_constant(u128::from(fp_offset), EVT::I32)),
        (8, pointer, dag.get_frame_index(pointer, index)),
        (16, pointer, dag.get_frame_index(pointer, save)),
    ];
    for (offset, vt, value) in fields {
        let at = dag.get_constant(offset, pointer);
        let address = dag.get_node(ISD::Add, pointer, &[list, at]);
        let info = MemInfo { mem_vt: vt, align: 4, volatile: false, base, ext: None, space: 0, ordering: None };
        stores.push(dag.get_store(chain, value, address, info));
    }
    Some(dag.get_node(ISD::TokenFactor, EVT::OTHER, &stores))
}

fn is_pow2(value: u128) -> bool {
    value != 0 && value & (value - 1) == 0
}

fn log2(value: u128) -> u128 {
    u128::from(127 - value.leading_zeros())
}

/// `X86ISD::MUL_IMM` by 3, 5 or 9: the `lea` the matcher folds it into. An
/// `i16` has no such `lea` pattern, so it is written as the add of a shift
/// (`x + (x << 1)`), which the same matcher turns into one.
fn mul_imm_node(
    dag: &mut SelectionDAG,
    x: SDValue,
    by: u128,
    vt: EVT,
) -> SDValue {
    if vt == EVT::I16 && matches!(by, 3 | 5 | 9) {
        let amount = dag.get_constant(log2(by - 1), EVT::I8);
        let shifted = dag.get_node(ISD::Shl, vt, &[x, amount]);
        return dag.get_node(ISD::Add, vt, &[x, shifted]);
    }
    let by = dag.get_constant(by, vt);
    dag.get_node(x86isd::MUL_IMM, vt, &[x, by])
}

/// `combineMulSpecial`: `x * c` for the constants that are a `lea` and a
/// shift or an add.
fn combine_mul_special(
    dag: &mut SelectionDAG,
    x: SDValue,
    amount: u128,
    vt: EVT,
) -> Option<SDValue> {
    let shl_add_or_sub = |dag: &mut SelectionDAG, mult: u128, shift: u128, add: bool| {
        let product = mul_imm_node(dag, x, mult, vt);
        let amount = dag.get_constant(shift, EVT::I8);
        let shifted = dag.get_node(ISD::Shl, vt, &[product, amount]);
        dag.get_node(if add { ISD::Add } else { ISD::Sub }, vt, &[shifted, x])
    };
    let mul_mul_add = |dag: &mut SelectionDAG, first: u128, second: u128| {
        let once = mul_imm_node(dag, x, first, vt);
        let twice = mul_imm_node(dag, once, second, vt);
        dag.get_node(ISD::Add, vt, &[twice, x])
    };
    match amount {
        11 => return Some(shl_add_or_sub(dag, 5, 1, true)),
        21 => return Some(shl_add_or_sub(dag, 5, 2, true)),
        41 => return Some(shl_add_or_sub(dag, 5, 3, true)),
        22 => {
            let inner = shl_add_or_sub(dag, 5, 2, true);
            return Some(dag.get_node(ISD::Add, vt, &[x, inner]));
        }
        19 => return Some(shl_add_or_sub(dag, 9, 1, true)),
        37 => return Some(shl_add_or_sub(dag, 9, 2, true)),
        73 => return Some(shl_add_or_sub(dag, 9, 3, true)),
        13 => return Some(shl_add_or_sub(dag, 3, 2, true)),
        23 => return Some(shl_add_or_sub(dag, 3, 3, false)),
        26 => return Some(mul_mul_add(dag, 5, 5)),
        28 => return Some(mul_mul_add(dag, 9, 3)),
        29 => {
            let inner = mul_mul_add(dag, 9, 3);
            return Some(dag.get_node(ISD::Add, vt, &[x, inner]));
        }
        _ => {}
    }
    // Two shifts added: `x * (2^a + 2^b)` with the lower one a scale.
    if is_pow2(amount & (amount - 1)) {
        let scale_shift = u128::from(amount.trailing_zeros());
        if (1..4).contains(&scale_shift) {
            let high = log2(amount & (amount - 1));
            let (ha, sa) = (dag.get_constant(high, EVT::I8), dag.get_constant(scale_shift, EVT::I8));
            let (first, second) = (dag.get_node(ISD::Shl, vt, &[x, ha]), dag.get_node(ISD::Shl, vt, &[x, sa]));
            return Some(dag.get_node(ISD::Add, vt, &[first, second]));
        }
    }
    None
}

/// `combineMul`, the scalar integer part: `x * c` as `lea`s, shifts, adds.
/// Bytes of the chain `combine_mul` built for `x * c` (the `x` is free): a
/// shift 3 (2 by one), an add or subtract 2, a `lea` form 3, and 2 for each
/// further read of `x` (the copy a two-address instruction takes).
fn chain_bytes(
    dag: &SelectionDAG,
    chain: SDValue,
    x: SDValue,
) -> u32 {
    if chain == x || dag.constant(chain).is_some() {
        return 0;
    }
    let mut bytes = match dag.opcode(chain) {
        ISD::Shl => 2 + u32::from(dag.constant(dag.operand(chain, 1)) != Some(1)),
        x86isd::MUL_IMM => 3,
        _ => 2,
    };
    for index in 0..dag.node(chain).ops.len() {
        let operand = dag.operand(chain, index);
        if operand == x {
            bytes += if index == 0 { 0 } else { 2 };
        } else {
            bytes += chain_bytes(dag, operand, x);
        }
    }
    bytes
}

fn combine_mul(
    dag: &mut SelectionDAG,
    node: SDValue,
    level: CombineLevel,
    min_size: bool,
    i16_chains: bool,
    profile: Option<MulPricing<'_>>,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    if vt != EVT::I32 && vt != EVT::I64 && !(vt == EVT::I16 && i16_chains) {
        return None;
    }
    let (x, rhs) = (dag.operand(node, 0), dag.operand(node, 1));
    let bits = dag.constant(rhs)?;
    let width = vt.size_in_bits();
    if bits == 0 {
        return Some(dag.get_constant(0, vt));
    }
    if bits == llrm_codegen::backend::selectiondag::fold::mask(width) {
        return Some(dag.get_negative(x, vt));
    }
    // An imul is usually smaller than the alternative sequence.
    if is_pow2(bits) || min_size || level == CombineLevel::BeforeLegalizeTypes {
        return None;
    }
    let signed = match vt {
        EVT::I16 => i128::from(bits as u16 as i16),
        EVT::I64 => i128::from(bits as u64 as i64),
        _ => i128::from(bits as u32 as i32),
    };
    let abs = signed.unsigned_abs();
    let negate = |dag: &mut SelectionDAG, value: SDValue| if signed < 0 { dag.get_negative(value, vt) } else { value };
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: u128| {
        let by = dag.get_constant(by, EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let mul_imm = |dag: &mut SelectionDAG, value: SDValue, by: u128| mul_imm_node(dag, value, by, vt);
    if matches!(abs, 3 | 5 | 9) {
        let product = mul_imm(dag, x, abs);
        return Some(negate(dag, product));
    }
    // DEVIATE SD447: where the target prices the pin's decompositions, the
    // sheet's chain decides alone: the pin's cases (`lea` pairs, `shl; sub`)
    // cost an instruction and bytes the `imul` does not, and a tie keeps it.
    // The pin:
    // <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L51191>.
    // A product `{3,5,9} << {1,2,3}` summed into an `add` keeps the pin's
    // chain: its last step folds into the `add` as a scaled `lea`, which the
    // sheet does not price. Exit 2026-11-20. Owner llrm-target-x86-m64.
    let summed = signed > 0
        && [3u128, 5, 9].iter().any(|base| abs % base == 0 && matches!(abs / base, 2 | 4 | 8))
        && dag.has_one_use(node.node)
        && dag.users(node).next().is_some_and(|user| dag.opcode(SDValue { node: user, res: 0 }) == ISD::Add);
    // The priced chains are written for 16- and 32-bit products.
    let profile = profile.filter(|_| vt != EVT::I64);
    if let Some(pricing) = profile.filter(|pricing| pricing.price_pin && !summed) {
        return priced_chain(dag, x, signed as i64, vt, pricing);
    }
    let (mut first, mut second) = (0u128, 0u128);
    for divisor in [9u128, 5, 3] {
        if abs % divisor == 0 {
            first = divisor;
            second = abs / divisor;
            break;
        }
    }
    let mut new_mul;
    if second != 0 && (is_pow2(second) || (signed >= 0 && matches!(second, 3 | 5 | 9))) {
        let single_add_user = dag.users(node).count() == 1
            && dag.users(node).next().map(|user| dag.opcode(SDValue { node: user, res: 0 })) == Some(ISD::Add);
        if is_pow2(second) && !(signed >= 0 && single_add_user) {
            std::mem::swap(&mut first, &mut second);
        }
        let mut value = if is_pow2(first) { shift(dag, x, log2(first)) } else { mul_imm(dag, x, first) };
        value = if is_pow2(second) { shift(dag, value, log2(second)) } else { mul_imm(dag, value, second) };
        new_mul = Some(negate(dag, value));
    } else {
        new_mul = combine_mul_special(dag, x, bits & llrm_codegen::backend::selectiondag::fold::mask(width), vt);
    }
    if new_mul.is_none() {
        if is_pow2(abs - 1) {
            let shifted = shift(dag, x, log2(abs - 1));
            let sum = dag.get_node(ISD::Add, vt, &[x, shifted]);
            new_mul = Some(negate(dag, sum));
        } else if is_pow2(abs + 1) {
            let shifted = shift(dag, x, log2(abs + 1));
            new_mul = Some(if signed < 0 {
                dag.get_node(ISD::Sub, vt, &[x, shifted])
            } else {
                dag.get_node(ISD::Sub, vt, &[shifted, x])
            });
        } else if signed >= 0 && abs > 2 && is_pow2(abs - 2) {
            let shifted = shift(dag, x, log2(abs - 2));
            let twice = dag.get_node(ISD::Add, vt, &[x, x]);
            new_mul = Some(dag.get_node(ISD::Add, vt, &[shifted, twice]));
        } else if signed >= 0 && is_pow2(abs + 2) {
            let shifted = shift(dag, x, log2(abs + 2));
            let twice = dag.get_node(ISD::Add, vt, &[x, x]);
            new_mul = Some(dag.get_node(ISD::Sub, vt, &[shifted, twice]));
        }
    }
    // DEVIATE SD446: where the pin keeps `imul`, the sheet's cheaper chain.
    if new_mul.is_none() {
        new_mul = profile.and_then(|profile| priced_chain(dag, x, signed as i64, vt, profile));
    }
    new_mul
}

/// `x * c` as the chain `arithmetic::chain_at_speed` accepts (the clocks it
/// saves pay for its bytes at the block's frequency; the search is cached per
/// constant, O(1) here), built from shifts, adds, subtracts and
/// the `lea` forms. An `i16` has no `lea`: the narrow chain.
///
/// DEVIATE SD446: the pin's `combineMul` leaves an `imul` for a constant none
/// of its cases covers (75, 251, 10000); on the 486 that is 13-42 clocks
/// (Intel 240440-002 Table 10.1) against 2-4 for a chain. The search is
/// `arithmetic`'s, the same as DEVIATE M1 in `arithmetic.rs`. Only where the target trades
/// bytes for clocks at speed (m16: `speed_milliclocks_per_byte`); the block's
/// frequency is the IR `BlockFrequencies` (the machine one does not exist at
/// DAG time). Exit: none while the clocks decide. Owner sdag-d.
fn priced_chain(
    dag: &mut SelectionDAG,
    x: SDValue,
    c: i64,
    vt: EVT,
    pricing: MulPricing<'_>,
) -> Option<SDValue> {
    use llrm_codegen::backend::arithmetic;
    let width = if vt == EVT::I16 { 2 } else { 4 };
    let MulPricing { cpu, arch, freq, milliclocks_per_byte, by_clocks, price_pin } = pricing;
    // -O1 is strict in bytes and clocks: no rate trades one for the other, so
    // the pin's `combineMul` (above) is all there is and an `imul` stays an
    // `imul`.
    if by_clocks && milliclocks_per_byte == 0 {
        return None;
    }
    let chain = if by_clocks {
        arithmetic::chain_by_clocks_then_bytes(c, width, cpu, arch)
    } else {
        arithmetic::priced_chain_at_speed(c, width, cpu, arch, freq, milliclocks_per_byte, price_pin)
    }
    .ok()??;
    let shift = |dag: &mut SelectionDAG, value: SDValue, by: i64| {
        let by = dag.get_constant(by as u128, EVT::I8);
        dag.get_node(ISD::Shl, vt, &[value, by])
    };
    let mut value = x;
    for (step, count) in chain {
        value = match step {
            "shl" => shift(dag, value, count),
            "add" => dag.get_node(ISD::Add, vt, &[value, x]),
            "sub" => dag.get_node(ISD::Sub, vt, &[value, x]),
            "rsub" => dag.get_node(ISD::Sub, vt, &[x, value]),
            "neg" => dag.get_negative(value, vt),
            // `x + value * count`
            "lea" => {
                let scaled = shift(dag, value, i64::from(count.trailing_zeros()));
                dag.get_node(ISD::Add, vt, &[scaled, x])
            }
            // `value + value * count`
            "flea" => mul_imm_node(dag, value, count as u128 + 1, vt),
            "fadd" => {
                let scaled = shift(dag, value, count);
                dag.get_node(ISD::Add, vt, &[value, scaled])
            }
            "fsub" => {
                let scaled = shift(dag, value, count);
                dag.get_node(ISD::Sub, vt, &[scaled, value])
            }
            _ => return None,
        };
    }
    Some(value)
}

/// `LowerFunnelShift`, the scalar part ([L32188][pin]). An `i8` by a variable
/// is done in a dword: `fshl(x,y,z)` is `(((aext x << 8) | zext y) << (z & 7))
/// >> 8`. An `i8` by a constant is two shifts and an `or`
/// (`expandFunnelShift`). An `i16` is the target node with the amount modulo
/// 16 (`SHLD16`/`SHRD16`); the dword is already legal.
///
/// DEVIATE SD247: the `isSHLDSlow` expansion of `i16` is not translated (no
/// target of ours sets it). Owner sdag-b.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L32188
fn lower_funnel_shift(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let is_fshr = dag.opcode(node) == ISD::FShr;
    let (op0, op1, amount) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let amount_vt = dag.value_type(amount);
    let bits = vt.size_in_bits();
    if vt == EVT::I8 {
        if let Some(by) = dag.constant(amount) {
            let by = (by % u128::from(bits)) as u32;
            if by == 0 {
                return Some(if is_fshr { op1 } else { op0 });
            }
            let (left, right) = if is_fshr { (bits - by, by) } else { (by, bits - by) };
            let high = dag.get_constant(u128::from(left), amount_vt);
            let low = dag.get_constant(u128::from(right), amount_vt);
            let high = dag.get_node(ISD::Shl, vt, &[op0, high]);
            let low = dag.get_node(ISD::Srl, vt, &[op1, low]);
            return Some(dag.get_node(ISD::Or, vt, &[high, low]));
        }
        let wide = EVT::I32;
        let mask = dag.get_constant(u128::from(bits - 1), amount_vt);
        let shift = dag.get_constant(u128::from(bits), amount_vt);
        let hi = dag.get_node(ISD::AnyExtend, wide, &[op0]);
        let lo = dag.get_zext_or_trunc(op1, wide);
        let amount = dag.get_node(ISD::And, amount_vt, &[amount, mask]);
        let hi = dag.get_node(ISD::Shl, wide, &[hi, shift]);
        let mut result = dag.get_node(ISD::Or, wide, &[hi, lo]);
        if is_fshr {
            result = dag.get_node(ISD::Srl, wide, &[result, amount]);
        } else {
            result = dag.get_node(ISD::Shl, wide, &[result, amount]);
            result = dag.get_node(ISD::Srl, wide, &[result, shift]);
        }
        return Some(dag.get_node(ISD::Truncate, vt, &[result]));
    }
    if vt == EVT::I16 {
        let mask = dag.get_constant(15, amount_vt);
        let amount = dag.get_node(ISD::And, amount_vt, &[amount, mask]);
        let op = if is_fshr { x86isd::FSHR } else { x86isd::FSHL };
        return Some(dag.get_node(op, vt, &[op0, op1, amount]));
    }
    None
}

/// `LowerShiftParts` (`TargetLowering::expandShiftParts`): a double-width
/// shift by a variable amount as two shifts, a funnel shift and selects on
/// bit 5 of the amount.
fn lower_shift_parts(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let (lo, hi, amount) = (dag.operand(node, 0), dag.operand(node, 1), dag.operand(node, 2));
    let vt = dag.value_type(lo);
    let bits = u128::from(vt.size_in_bits());
    let amount_vt = dag.value_type(amount);
    let is_shl = dag.opcode(node) == ISD::ShlParts;
    let is_sra = dag.opcode(node) == ISD::SraParts;
    let mask = dag.get_constant(bits - 1, amount_vt);
    let safe = dag.get_node(ISD::And, amount_vt, &[amount, mask]);
    let tmp1 = if is_sra {
        let sign = dag.get_constant(bits - 1, amount_vt);
        dag.get_node(ISD::Sra, vt, &[hi, sign])
    } else {
        dag.get_constant(0, vt)
    };
    let (tmp2, tmp3) = if is_shl {
        (dag.get_node(ISD::FShl, vt, &[hi, lo, amount]), dag.get_node(ISD::Shl, vt, &[lo, safe]))
    } else {
        (
            dag.get_node(ISD::FShr, vt, &[hi, lo, amount]),
            dag.get_node(if is_sra { ISD::Sra } else { ISD::Srl }, vt, &[hi, safe]),
        )
    };
    let width = dag.get_constant(bits, amount_vt);
    let and = dag.get_node(ISD::And, amount_vt, &[amount, width]);
    let zero = dag.get_constant(0, amount_vt);
    let cond = dag.get_setcc(EVT::I8, and, zero, CondCode::SetNe);
    let (out_lo, out_hi) = if is_shl {
        (dag.get_select(vt, cond, tmp1, tmp3), dag.get_select(vt, cond, tmp3, tmp2))
    } else {
        (dag.get_select(vt, cond, tmp3, tmp2), dag.get_select(vt, cond, tmp1, tmp3))
    };
    dag.get_node_multi(ISD::MergeValues, &[vt, vt], &[out_lo, out_hi])
}

/// `isKnownNeverZero`, the part known bits answer: some bit is known set.
/// DEVIATE SD304: the pin also looks through `or`, `shl`, `zext`, `smax` and
/// the like; a value they prove non-zero costs one `cmov` more here. Reason:
/// no general query yet. Measured 2026-11-06 12:3xZ: a counter for what the
/// wider query proves beyond known bits (or, select, shl, extensions, max) saw
/// 0 hits in the 10 corpus programs that lower ctlz/cttz (m16/m32/m64 -O2), and
/// a probe with a umax and an `or` argument already dropped the guard. No cell;
/// port with the combiner's `isKnownNeverZero` when one needs it. Owner sdag-c.
/// Review 2026-12-10.
fn is_known_never_zero(
    dag: &SelectionDAG,
    value: SDValue,
) -> bool {
    llrm_codegen::backend::selectiondag::known_bits::known_bits(dag, value).one != 0
}

/// `KnownBits::countMinLeadingZeros`.
fn min_leading_zeros(known: &llrm_codegen::backend::selectiondag::known_bits::KnownBits) -> u32 {
    (!known.zero & llrm_codegen::backend::selectiondag::fold::mask(known.width)).leading_zeros() - (128 - known.width)
}

/// `LowerFABSorFNEG` for a scalar: no scalar bitwise SSE instruction exists, so
/// the sign bit is cleared (`FAND`), flipped (`FXOR`) or set (`FOR`, for
/// `fneg (fabs x)`) by a 16-byte pool mask, the sign mask in every lane.
fn lower_fabs_or_fneg(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let sign = match vt {
        EVT::Simple(MVT::F32) => 1u128 << 31,
        EVT::Simple(MVT::F64) => 1u128 << 63,
        _ => return None,
    };
    let is_fabs = dag.opcode(node) == ISD::FAbs;
    // `fabs` under an `fneg` is left to the `fneg`, which sets the sign.
    if is_fabs && dag.users(node).any(|user| dag.opcode(SDValue { node: user, res: 0 }) == ISD::FNeg) {
        return Some(node);
    }
    let mut x = dag.operand(node, 0);
    let negated_abs = !is_fabs && dag.opcode(x) == ISD::FAbs;
    if negated_abs {
        x = dag.operand(x, 0);
    }
    let (op, mask) = match (is_fabs, negated_abs) {
        (true, _) => (x86isd::FAND, sign - 1),
        (false, true) => (x86isd::FOR, sign),
        (false, false) => (x86isd::FXOR, sign),
    };
    let address = mask_address(dag, mask, vt);
    Some(dag.get_node(op, vt, &[x, address]))
}

/// The address of the 16-byte constant `mask` of a scalar `vt` repeated in each
/// lane: the pin's `getConstantFP` of a `v4f32` or `v2f64` splat, a pool entry
/// the bitwise instruction reads whole.
fn mask_address(
    dag: &mut SelectionDAG,
    mask: u128,
    vt: EVT,
) -> SDValue {
    let lane = vt.size_in_bits();
    let splat = (0..128 / lane).fold(0u128, |all, at| all | mask << (at * lane));
    let pointer = dag.config().pointer;
    let pool = dag.get_constant_pool(pointer, splat, EVT::Simple(MVT::I128), true);
    dag.get_node(wrapper_kind(dag, pointer), pointer, &[pool])
}

/// `LowerFCOPYSIGN` for a scalar: `(mag & ~sign) | (sgn & sign)`, the sign
/// operand first extended or rounded to the magnitude's type.
fn lower_fcopysign(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let sign = match vt {
        EVT::Simple(MVT::F32) => 1u128 << 31,
        EVT::Simple(MVT::F64) => 1u128 << 63,
        _ => return None,
    };
    let (mag, mut sgn) = (dag.operand(node, 0), dag.operand(node, 1));
    let sgn_vt = dag.value_type(sgn);
    if sgn_vt != vt {
        sgn = if sgn_vt.size_in_bits() < vt.size_in_bits() {
            dag.get_node(ISD::FpExtend, vt, &[sgn])
        } else {
            let zero = dag.get_constant_with(0, EVT::I32, true, false);
            dag.get_node(ISD::FpRound, vt, &[sgn, zero])
        };
    }
    let masked = |dag: &mut SelectionDAG, op, value, mask| {
        let address = mask_address(dag, mask, vt);
        dag.get_node(op, vt, &[value, address])
    };
    let magnitude = masked(dag, x86isd::FAND, mag, sign - 1);
    let sign_bit = masked(dag, x86isd::FAND, sgn, sign);
    Some(dag.get_node(x86isd::FOR, vt, &[magnitude, sign_bit]))
}

/// The stack pointer a pointer of type `vt` is saved from and restored to
/// (`getStackPointerRegisterToSaveRestore`): `rsp` at 64 bits.
fn stack_register(vt: EVT) -> llrm_lir::registers::RegId {
    use llrm_lir::registers::RegId;
    match vt {
        EVT::I64 => RegId::RSP,
        EVT::I32 => RegId::ESP,
        _ => RegId::SP,
    }
}

/// `LowerDYNAMIC_STACKALLOC` without probes or split stacks: the stack pointer
/// lowered by the size, rounded down to the alignment when that is more than
/// the stack's (operand 2, else 0), is the address. Bracketed by a
/// `CALLSEQ` pair so nothing else moves the stack pointer meanwhile.
fn lower_dynamic_stackalloc(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let (chain, size) = (dag.operand(node, 0), dag.operand(node, 1));
    let alignment = dag.constant(dag.operand(node, 2)).unwrap_or(0);
    let start = dag.get_callseq_start(chain, 0);
    let sp = dag.get_copy_from_phys_reg_plain(SDValue { node: start.node, res: 0 }, stack_register(vt), vt);
    let mut result = dag.get_node(ISD::Sub, vt, &[sp, size]);
    let mut chain = SDValue { node: sp.node, res: 1 };
    if alignment != 0 {
        let mask = dag.get_constant(!(alignment - 1) & ((1u128 << vt.size_in_bits()) - 1), vt);
        result = dag.get_node(ISD::And, vt, &[result, mask]);
    }
    chain = dag.get_copy_to_phys_reg(chain, stack_register(vt), result, None);
    let chain = SDValue { node: chain.node, res: 0 };
    let end = dag.get_callseq_end(chain, 0, 0, SDValue { node: chain.node, res: 1 });
    Some(dag.get_node_multi(ISD::MergeValues, &[vt, EVT::OTHER], &[result, SDValue { node: end.node, res: 0 }]))
}

/// `LowerABS` for `i16`/`i32`: `x < 0 ? -x : x` as `cmov` on the flags of `0 -
/// x`.
fn lower_abs(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let x = dag.operand(node, 0);
    let zero = dag.get_constant(0, vt);
    let neg = dag.get_node_multi(x86isd::SUB, &[vt, EVT::I32], &[zero, x]);
    let flags = SDValue { node: neg.node, res: 1 };
    let code = cond_code(dag, cond::NS);
    dag.get_node(x86isd::CMOV, vt, &[x, neg, code, flags])
}

/// `LowerADDSAT_SUBSAT` for a scalar `SSUBSAT` ([L29900][pin]): the signed
/// difference with overflow, and on overflow the maximum if the wrapped
/// difference is negative, else the minimum.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L29900
fn lower_ssubsat(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let (x, y) = (dag.operand(node, 0), dag.operand(node, 1));
    let min = 1u128 << (vt.size_in_bits() - 1);
    let zero = dag.get_constant(0, vt);
    let result = dag.get_node_multi(ISD::SSubO, &[vt, EVT::I8], &[x, y]);
    let (difference, overflow) = (result, SDValue { node: result.node, res: 1 });
    let sat_min = dag.get_constant(min, vt);
    let sat_max = dag.get_constant(min - 1, vt);
    let negative = dag.get_setcc(EVT::I8, difference, zero, CondCode::SetLt);
    let saturated = dag.get_select(vt, negative, sat_max, sat_min);
    dag.get_select(vt, overflow, saturated, difference)
}

/// `LowerCTLZ` ([L29730][pin]): `bsr` and `xor bits-1`. A zero source is `bits` through the
/// pass-through operand of `bsr` (`hasBitScanPassThrough`, `is64Bit`: `bsr` leaves its
/// destination alone for a zero source, so `mov eax, 2*bits-1; bsr eax, src`), or through a
/// `cmov` where there is no pass-through.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L29730
fn lower_ctlz(
    dag: &mut SelectionDAG,
    node: SDValue,
    bit_scan_pass_through: bool,
) -> SDValue {
    let vt = dag.value_type(node);
    let bits = u128::from(vt.size_in_bits());
    let mut op = dag.operand(node, 0);
    let mut op_vt = vt;
    if vt == EVT::I8 {
        // There is no 8-bit bsr.
        op_vt = EVT::I32;
        op = dag.get_node(ISD::ZeroExtend, op_vt, &[op]);
    }
    let is_ctlz = dag.opcode(node) == ISD::CtLz;
    let zero_answer = bits + bits - 1;
    // Check if we can safely pass a result though BSR for zero sources.
    let passes = is_ctlz && bit_scan_pass_through && !is_known_never_zero(dag, op);
    let pass_through = if passes { dag.get_constant(zero_answer, op_vt) } else { dag.get_undef(op_vt) };
    let scan = dag.get_node_multi(x86isd::BSR, &[op_vt, EVT::I32], &[pass_through, op]);
    let mut result = SDValue { node: scan.node, res: 0 };
    // Skip the CMOV where a pass-through value is used.
    if is_ctlz && !passes {
        // A zero source sets ZF: the answer is `bits`, which the xor leaves.
        let zero_answer = dag.get_constant(zero_answer, op_vt);
        let code = cond_code(dag, cond::E);
        let flags = SDValue { node: scan.node, res: 1 };
        result = dag.get_node(x86isd::CMOV, op_vt, &[result, zero_answer, code, flags]);
    }
    let last = dag.get_constant(bits - 1, op_vt);
    result = dag.get_node(ISD::Xor, op_vt, &[result, last]);
    if vt == EVT::I8 { dag.get_node(ISD::Truncate, EVT::I8, &[result]) } else { result }
}

/// `LowerCTTZ` ([L29776][pin]) for scalars: `bsf`, with `bits` for a zero source passed through
/// it (`hasBitScanPassThrough`) or selected by a `cmov`, unless the source cannot be zero.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L29776
fn lower_cttz(
    dag: &mut SelectionDAG,
    node: SDValue,
    bit_scan_pass_through: bool,
) -> SDValue {
    let vt = dag.value_type(node);
    let src = dag.operand(node, 0);
    let non_zero = is_known_never_zero(dag, src);
    // Check if we can safely pass a result though BSF for zero sources.
    let passes = !non_zero && bit_scan_pass_through;
    let pass_through = if passes { dag.get_constant(u128::from(vt.size_in_bits()), vt) } else { dag.get_undef(vt) };
    let scan = dag.get_node_multi(x86isd::BSF, &[vt, EVT::I32], &[pass_through, src]);
    // Skip the CMOV if the source is never zero or a pass-through value is used.
    if non_zero || passes {
        return SDValue { node: scan.node, res: 0 };
    }
    let zero_answer = dag.get_constant(u128::from(vt.size_in_bits()), vt);
    let code = cond_code(dag, cond::E);
    let flags = SDValue { node: scan.node, res: 1 };
    dag.get_node(x86isd::CMOV, vt, &[SDValue { node: scan.node, res: 0 }, zero_answer, code, flags])
}

/// `LowerPARITY` ([L33977][pin]) without POPCNT: the parity flag of an
/// 8-bit `test`, or of the xor of the two low bytes after folding the rest.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L33977
fn lower_parity(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> SDValue {
    let vt = dag.value_type(node);
    let mut x = dag.operand(node, 0);
    let known = llrm_codegen::backend::selectiondag::known_bits::known_bits(dag, x);
    let active = known.width - min_leading_zeros(&known);
    let not_parity = |dag: &mut SelectionDAG, flags: SDValue| {
        let code = cond_code(dag, cond::NP);
        let setnp = dag.get_node(x86isd::SETCC, EVT::I8, &[code, flags]);
        if vt == EVT::I8 { setnp } else { dag.get_node(ISD::ZeroExtend, vt, &[setnp]) }
    };
    if vt == EVT::I8 || active <= 8 {
        x = dag.get_zext_or_trunc(x, EVT::I8);
        let zero = dag.get_constant(0, EVT::I8);
        let flags = dag.get_node(x86isd::CMP, EVT::I32, &[x, zero]);
        return not_parity(dag, flags);
    }
    if vt != EVT::I16 {
        let sixteen = dag.get_constant(16, EVT::I8);
        let hi16 = dag.get_node(ISD::Srl, EVT::I32, &[x, sixteen]);
        x = dag.get_node(ISD::Xor, EVT::I32, &[x, hi16]);
    } else {
        x = dag.get_node(ISD::AnyExtend, EVT::I32, &[x]);
    }
    let eight = dag.get_constant(8, EVT::I8);
    let shifted = dag.get_node(ISD::Srl, EVT::I32, &[x, eight]);
    let hi = dag.get_node(ISD::Truncate, EVT::I8, &[shifted]);
    let lo = dag.get_node(ISD::Truncate, EVT::I8, &[x]);
    let xor = dag.get_node_multi(x86isd::XOR, &[EVT::I8, EVT::I32], &[lo, hi]);
    not_parity(dag, SDValue { node: xor.node, res: 1 })
}

/// `LowerCTPOP` ([L33649][pin]) for scalars: a constant source, or a value
/// with few active bits, gets a short sequence; anything else is `None`,
/// the generic expansion.
///
/// [pin]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelLowering.cpp#L33649
fn lower_ctpop(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let mut op = dag.operand(node, 0);
    let known = llrm_codegen::backend::selectiondag::known_bits::known_bits(dag, op);
    if known.is_constant() {
        return Some(dag.get_constant(u128::from(known.one.count_ones()), vt));
    }
    let width = known.width;
    let leading = min_leading_zeros(&known);
    let trailing = known.zero.trailing_ones();
    let active = width - leading;
    let shifted_active = width - (leading + trailing);
    let i8_amount = |dag: &mut SelectionDAG, n: u32| dag.get_constant(u128::from(n), EVT::I8);
    let shift_down = |dag: &mut SelectionDAG, op: SDValue| {
        let amount = i8_amount(dag, trailing);
        dag.get_node(ISD::Srl, vt, &[op, amount])
    };
    if shifted_active <= 2 {
        if active > 2 {
            op = shift_down(dag, op);
        }
        op = dag.get_zext_or_trunc(op, EVT::I32);
        let one = i8_amount(dag, 1);
        let half = dag.get_node(ISD::Srl, EVT::I32, &[op, one]);
        op = dag.get_node(ISD::Sub, EVT::I32, &[op, half]);
        return Some(dag.get_zext_or_trunc(op, vt));
    }
    if shifted_active <= 3 {
        if active > 3 {
            op = shift_down(dag, op);
        }
        op = dag.get_zext_or_trunc(op, EVT::I32);
        let one = i8_amount(dag, 1);
        op = dag.get_node(ISD::Shl, EVT::I32, &[op, one]);
        let table = dag.get_constant(0b1110_1001_1001_0100, EVT::I32);
        op = dag.get_node(ISD::Srl, EVT::I32, &[table, op]);
        let three = dag.get_constant(3, EVT::I32);
        op = dag.get_node(ISD::And, EVT::I32, &[op, three]);
        return Some(dag.get_zext_or_trunc(op, vt));
    }
    // The 4-bit table is an `i64` lookup, and `i64` is not legal here.
    if shifted_active <= 8 {
        let mask11 = dag.get_constant(0x1111_1111, EVT::I32);
        if active > 8 {
            op = shift_down(dag, op);
        }
        op = dag.get_zext_or_trunc(op, EVT::I32);
        let magic = dag.get_constant(0x0804_0201, EVT::I32);
        op = dag.get_node(ISD::Mul, EVT::I32, &[op, magic]);
        let three = i8_amount(dag, 3);
        op = dag.get_node(ISD::Srl, EVT::I32, &[op, three]);
        op = dag.get_node(ISD::And, EVT::I32, &[op, mask11]);
        op = dag.get_node(ISD::Mul, EVT::I32, &[op, mask11]);
        let top = i8_amount(dag, 28);
        op = dag.get_node(ISD::Srl, EVT::I32, &[op, top]);
        return Some(dag.get_zext_or_trunc(op, vt));
    }
    None
}

/// DEVIATE X87-SHARE (owner x87, exit 2026-11-20; the pin canonicalizes
/// `-X * C` to `X * -C`, [InstCombineMulDivRem.cpp#L1036][canon], and shares
/// only equal products): `a + v*(-C)` is `a - v*C` and `a - v*(-C)` is `a +
/// v*C`, exact in IEEE (`v*(-C)` is `-(v*C)`, the add of a negated value the
/// subtract). Where the product with the positive constant is already in the
/// DAG the negative one is not made, so `c*v` and `(-c)*v` are one multiply on
/// the x87 stack, not two. x_jpeg_single -O3 -4.3% clk, -O2 -5.3%, other 43
/// m32 float cells identical.
///
/// [canon]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/InstCombine/InstCombineMulDivRem.cpp#L1036
fn combine_shared_product(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let sign: u128 = match vt {
        EVT::Simple(MVT::F32) => 1 << 31,
        EVT::Simple(MVT::F64) => 1 << 63,
        _ => return None,
    };
    let add = dag.opcode(node) == ISD::FAdd;
    // `x - v*C` has the product in operand 1 only.
    let products: &[usize] = if add { &[0, 1] } else { &[1] };
    for &at in products {
        let product = dag.operand(node, at);
        if dag.opcode(product) != ISD::FMul {
            continue;
        }
        let ops = [dag.operand(product, 0), dag.operand(product, 1)];
        for k in 0..2 {
            let NodeData::ConstFp { bits } = dag.node(ops[k]).data else { continue };
            if bits & sign == 0 || dag.opcode(ops[k]) != ISD::ConstantFP {
                continue;
            }
            let mut positive = ops;
            positive[k] = dag.get_constant_fp(bits ^ sign, vt);
            let Some(twin) = dag.find_node(ISD::FMul, vt, &positive) else { continue };
            if twin.node == product.node || !dag.has_users(twin) {
                continue;
            }
            let other = dag.operand(node, 1 - at);
            let opposite = if add { ISD::FSub } else { ISD::FAdd };
            return Some(dag.get_node(opposite, vt, &[other, twin]));
        }
    }
    None
}
