//! The DAG combiner: peephole rewrites of the DAG to a fixed point (the pin's
//! `DAGCombiner`).
//!
//! # How it works
//! A worklist holds the nodes to look at, newest last. Each pass of
//! [`DagCombiner::run`] takes the last, skips it if nothing reads it, puts
//! its operands back on the worklist, and calls [`DagCombiner::combine`]:
//! the generic `visit` for its opcode, else the target's
//! [`CombineHooks::perform_dag_combine`]. A result other than the node
//! replaces it in every user, and the result and its users go back on the
//! worklist. Nodes made on the way are put on the worklist too. The walk ends
//! when the worklist is empty.
//!
//! # Runs
//! At four levels (`CombineLevel`): before and after type legalization, after
//! vector legalization, and after DAG legalization. Not wired in yet. Only
//! `visitMUL`'s first half is translated here; the other visitors are added
//! in the order the corpus counts them.
//!
//! DEVIATE SD201: no `HandleSDNode` for the root, no `DAGUpdateListener`: the
//! arena keeps replaced nodes, `run` reads [`Rauw`] events and the node count
//! to find what changed. Exit: none. Owner sdag-b. Review 2026-11-13.
//!
//! DEVIATE SD202: no `computeKnownBits`/`MaskedValueIsZero` (SD12): a fold that
//! asks for known bits sees only constants. Exit: a known-bits port. Owner
//! sdag-b. Review 2026-11-13.
//!
//! DEVIATE SD205: `visitSIGN_EXTEND`/`ZERO_EXTEND`/`ANY_EXTEND` fold a
//! simple, non-extending load, before and after operations are legal (legal
//! extending load only, as the pin), with the table given by `with_tli` (no
//! table: only a load nothing else reads). Owner sdag-b. Review 2026-11-27.
//!
//! # See also
//! LLVM's `DAGCombiner::Run`
//! ([L1845](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1845)),
//! `combine`
//! ([L2157](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2157)),
//! `visitMUL`
//! ([L4908](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4908)).

use super::actions::{Action, LoadExt, TargetLowering};
use super::dag::{MemInfo, NodeData, Rauw, SDNodeFlags, SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::mvt::{EVT, MVT};

/// `CombineLevel`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CombineLevel {
    BeforeLegalizeTypes,
    AfterLegalizeTypes,
    AfterLegalizeVectorOps,
    AfterLegalizeDag,
}

/// A multiply by a constant as shifts, adds and subtracts of the multiplicand
/// (and a negate), built by the target's own search
/// ([`CombineHooks::synth_mul_chain`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MulPlan {
    /// The multiplicand.
    Multiplicand,
    /// The value shifted left by a constant.
    Shl(Box<MulPlan>, u32),
    Add(Box<MulPlan>, Box<MulPlan>),
    Sub(Box<MulPlan>, Box<MulPlan>),
    Neg(Box<MulPlan>),
}

/// The machine opcodes of a target's multiply of two half-width operands into
/// a full-width product (`mulu.w` / `muls.w`).
#[derive(Clone, Copy)]
pub struct HalfMul {
    pub half: EVT,
    pub unsigned: u32,
    pub signed: u32,
    /// The target has a full-width multiply instruction (`muls.l`). The half
    /// multiply then needs the operand's extension and a copy of the source
    /// (the tied destination), which the full-width one needs just the same
    /// without the copy: it is used only for operands that are already full
    /// width.
    pub has_full_multiply: bool,
    /// A constant operand that fits the half is the half multiply's immediate
    /// when [`CombineHooks::half_multiply_by_constant`] prices it under the
    /// chain and the full multiply.
    pub constant: bool,
}

/// `TLI.hasTargetDAGCombine` and `TLI.PerformDAGCombine`.
pub trait CombineHooks {
    /// `(mul x, y)` of operands known to fit half the width is the target's
    /// half-by-half multiply. The pin has no such rule: its M68k multiplies
    /// two `i32` with `muls.l` (68020) or a libcall (68000), never `muls.w`.
    /// DEVIATE: off by default; m68k opts in per CPU. Exit: none, while the
    /// pin has no known-bits multiply selection.
    fn mul_of_halves(
        &self,
        _vt: EVT,
    ) -> Option<HalfMul> {
        None
    }

    /// Whether `x * c`, with `x` known to fit the half (signed or unsigned)
    /// and `c` fitting it the same way, is the half multiply with `c` as its
    /// immediate, by the target's price against the chain and the full
    /// multiply. The pin has no such rule. DEVIATE: false by default; m68k
    /// answers per CPU and level (M68K-HALF-MUL-CONST).
    fn half_multiply_by_constant(
        &self,
        _vt: EVT,
        _c: u128,
        _signed: bool,
    ) -> bool {
        false
    }

    /// `TLI.shouldNormalizeToSelectSequence`: `select (or c0, c1), x, y` is
    /// `select c0, x, (select c1, x, y)` (and the `and` form likewise). The
    /// pin's default asks whether the type is split; DEVIATE: false here (the
    /// select-of-select merges instead) for the targets written before the
    /// hook, X86 overrides. Exit: each target answers as the pin does.
    fn normalize_to_select_sequence(
        &self,
        _vt: EVT,
        _cond_vt: EVT,
    ) -> bool {
        false
    }

    /// `TLI.convertSelectOfConstantsToMath`: a select of two constants is an
    /// extend of the condition plus a constant. The pin's default is false;
    /// DEVIATE: true here because this combiner was written for x86, which
    /// asks for it. Exit: x86 returns true and the default is the pin's.
    fn convert_select_of_constants_to_math(
        &self,
        _vt: EVT,
    ) -> bool {
        true
    }

    /// `visitBRCOND`'s `rebuildSetCC` and `SimplifySetCC`'s `assertzext i1`
    /// case: a branch on `xor x, y` is a branch on `setcc x, y, ne` (`xor
    /// (xor x, y), -1` on `eq`), before the types are legal. DEVIATE: off
    /// by default, as below; the `FREEZE` and `BR_CC` folds
    /// of `visitBRCOND` and the legal-types result type are not ported. Exit:
    /// x86 judged.
    fn rebuild_brcond_condition(&self) -> bool {
        false
    }

    /// `foldBinOpIntoSelect`: a binary operator of a select of constants and
    /// a constant is a select of the folded constants. DEVIATE: off by
    /// default, as above; called after the operator's own folds, not within
    /// them; `foldSelectWithIdentityConstant` and the peek through a
    /// truncated shift amount are not ported. Exit: x86 judged.
    fn fold_bin_op_into_select(&self) -> bool {
        false
    }

    /// `visitXOR`'s `(xor (and x, y), y)` is `(and (not x), y)`. DEVIATE: off
    /// by default so x86's rows stay as they are until the judge has seen
    /// them. Exit: x86 judged.
    /// `TLI.hasAndNot(Y)`: the target has an and-not instruction for a value of
    /// this type (`constant`: a non-opaque constant). Off: the pin's x86
    /// without BMI.
    fn has_and_not(
        &self,
        _vt: EVT,
        _constant: bool,
    ) -> bool {
        false
    }

    /// `TLI.hasAndNotCompare(Y)`: `(X & Y) == Y` is cheaper as `(~X & Y) == 0`
    /// for a value of this type (x86: NOT+TEST for any scalar integer).
    fn has_and_not_compare(
        &self,
        _vt: EVT,
    ) -> bool {
        false
    }

    fn fold_xor_of_and_with_mask(&self) -> bool {
        false
    }

    /// `visitSIGN_EXTEND_INREG`'s `(sext_inreg (extload x))` is `(sextload x)`.
    /// DEVIATE: off by default, as above. Exit: x86 judged.
    fn fold_sext_in_reg_of_load(&self) -> bool {
        false
    }

    /// `visitAND`'s `(and (extload x), c)` is `(zextload x)` for every user of
    /// the load when `c` keeps none of the bits above the memory type.
    /// DEVIATE: off by default, as above. Exit: x86 judged.
    fn fold_and_of_extload(&self) -> bool {
        false
    }

    /// `visitFREEZE`'s `freeze(op(x, ...))` is `op(freeze(x), ...)`, so a
    /// frozen compare tree is a compare tree of frozen leaves. DEVIATE: off by
    /// default, as above; and only for an integer `setcc` or bitwise/add
    /// node, with no `ReplaceAllUsesOfValueWith` of the frozen operand.
    /// Exit: x86 judged.
    fn push_freeze_through_operands(&self) -> bool {
        false
    }

    /// `tryStoreMergeOfConstants`' "check whether a truncstore is legal": a run
    /// of narrow constant stores whose joined type is illegal is one
    /// truncating store of the promoted type (AArch64: two `strb` are a
    /// `strh`). DEVIATE: off by default, as above. Exit: x86 judged.
    fn merge_stores_via_trunc_store(&self) -> bool {
        false
    }

    /// `DataLayout::isLittleEndian`: whether the first of several merged
    /// stores is the least significant part of the joined value.
    fn little_endian(&self) -> bool {
        true
    }

    /// `mergeStoresAfterLegalization`: stores of `mem` are also merged once
    /// types are legal, to catch the stores legalization made (a promoted
    /// `i16` store is a truncating `i32` one). The pin's default is true and
    /// x86's is `!mem.is_vector()`. DEVIATE: off by default, as above. Exit:
    /// x86 judged.
    fn merge_stores_after_legalization(
        &self,
        _mem: EVT,
    ) -> bool {
        false
    }

    /// `ReduceLoadWidth` looks through a `freeze` between an `and` mask and its
    /// load, and puts the freeze back on the narrowed load. Default false
    /// until x86 is judged.
    fn reduce_load_width_through_freeze(&self) -> bool {
        false
    }

    /// `TargetLowering::isLegalAddImmediate`: whether an `add` takes the
    /// signed immediate. Default true (arm64 and wasm are not judged); x86
    /// is `isInt<32>`.
    fn legal_add_immediate(
        &self,
        _imm: i64,
    ) -> bool {
        true
    }

    /// `visitMUL`'s `(mul (add x, c1), c2)` is `(add (mul x, c2), c1*c2)`.
    /// Default false until x86 is judged.
    fn fold_mul_of_add_with_constant(&self) -> bool {
        false
    }

    /// `visitMUL`'s `(mul x, c)` for a `c` that is `2^n +- 1` (times a power
    /// of two) as shifts and an add or subtract, where the target says the
    /// pieces are cheaper than the multiply (`decomposeMulByConstant`,
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/TargetLowering.h#L2633>).
    /// The pin's default is false.
    fn decompose_mul_by_constant(
        &self,
        _vt: EVT,
        _c: u128,
    ) -> bool {
        false
    }

    /// DEVIATE: `x * ((2^n +- 1) << k)` is `((x << n) +- x) << k`, not the
    /// pin's `(x << n + k) +- (x << k)`: where a shift costs per bit the
    /// factored form shifts `n + k` bits in all, not `n + 2k`. Owner:
    /// llrm-target-z80. Exit: 2026-11-30, when a second target measures it or
    /// the pin gains it.
    fn factor_trailing_zeros_of_mul(&self) -> bool {
        false
    }

    /// DEVIATE: the cheapest chain for `x * c` by the target's own prices
    /// (Bernstein's search), taken where the pin's `2^n +- 1` shape above does
    /// not apply or costs more. The pin has no such search. None keeps the
    /// multiply. Owner: llrm-target-m68k (G2). Exit: 2026-12-20, when a second
    /// target measures it or the pin gains it.
    fn synth_mul_chain(
        &self,
        _vt: EVT,
        _c: u128,
    ) -> Option<MulPlan> {
        None
    }

    /// DEVIATE: [`Self::decompose_mul_by_constant`] with the multiplicand, for
    /// a target whose multiply expansion gets cheaper with what is known of
    /// it (the Z80's byte-lane multiply of a zero-extended byte). The pin's
    /// hook sees the type and the constant only. Owner: llrm-target-z80.
    /// Exit: 2026-11-30, when a second target needs it or the pin gains it.
    fn decompose_mul_by_constant_of(
        &self,
        _dag: &SelectionDAG,
        _x: SDValue,
        vt: EVT,
        c: u128,
    ) -> bool {
        self.decompose_mul_by_constant(vt, c)
    }

    /// `ExtendSetCCUses`: the compares of a load against a constant, which
    /// `ExtendUsesToFormExtLoad` widens to compare its extension, of the type
    /// `wide`
    /// (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15421>).
    /// The pin always does; a target where the wider compare costs
    /// more than the narrow one (the Z80's word compare against a constant is
    /// two byte compares) leaves them on `trunc` of the extending load.
    fn extend_setcc_uses(
        &self,
        _wide: EVT,
    ) -> bool {
        true
    }

    /// `TargetLowering::isMulAddWithConstProfitable` for a single-use add, on
    /// the constants `c1` and `c2` of the type `vt`.
    fn is_mul_add_with_const_profitable(
        &self,
        _vt: EVT,
        _c1: u128,
        _c2: u128,
    ) -> bool {
        true
    }

    /// `visitSRA`'s `(sra (trunc (srl x, c1)), c2)` is `(trunc (sra x, c1 +
    /// c2))` when the truncate removes `c1` bits. Default false until x86
    /// is judged.
    fn fold_sra_of_trunc_of_shift(&self) -> bool {
        false
    }

    /// `isTruncateFree` as `ExtendUsesToFormExtLoad` asks it: a load another reader
    /// takes narrow may be widened for an extension only if the truncate back is free.
    /// The pin asks the same query as everywhere else; a target may answer it apart.
    fn extend_uses_truncate_free(
        &self,
        from: EVT,
        to: EVT,
    ) -> bool {
        self.is_truncate_free(from, to)
    }

    /// `TLI.isTruncateFree(from, to)`: truncating an integer needs no
    /// instruction. The pin's default is false; x86 says yes for any narrower
    /// integer.
    fn is_truncate_free(
        &self,
        _from: EVT,
        _to: EVT,
    ) -> bool {
        false
    }

    /// `TLI.isZExtFree(from, to)` (the `EVT` form): widening an integer
    /// without a sign needs no instruction. The pin's default is false; x86-64
    /// and AArch64 say yes for `i32` to `i64`.
    fn is_zext_free(
        &self,
        _from: EVT,
        _to: EVT,
    ) -> bool {
        false
    }

    /// `TargetLowering::shouldTransformSignedTruncationCheck`: `(add x,
    /// 1<<(k-1)) u< 1<<k` is `sext_inreg x, k == x`. The pin's default is
    /// false. Default false until x86 is judged.
    fn should_transform_signed_truncation_check(
        &self,
        _vt: EVT,
        _kept_bits: u32,
    ) -> bool {
        false
    }

    /// `TLI.isDesirableToCommuteWithShift`
    /// ([TargetLowering.h#L4763][pin]): whether the constant shift `shift`
    /// may move through its operand (`shl (add x, c1), c2` is `add (shl x,
    /// c2), c1 << c2`). The pin's default, which x86, m68k and wasm use: not
    /// when the operand has other users, nor when it is a sign extension whose
    /// source has other users. AArch64 and ARM override it; no in-tree target
    /// here does.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/include/llvm/CodeGen/TargetLowering.h#L4763
    fn is_desirable_to_commute_with_shift(
        &self,
        dag: &SelectionDAG,
        shift: SDValue,
        _level: CombineLevel,
    ) -> bool {
        let operand = dag.operand(shift, 0);
        if !dag.has_one_use(operand.node) {
            return false;
        }
        !(dag.opcode(operand) == ISD::SignExtend && !dag.has_one_use(dag.operand(operand, 0).node))
    }

    /// `visitSRL`'s `shift (shift x, c1), c2` as one shift by the sum, and
    /// `SimplifyDemandedBits`' `srl (shl x, c1), c2` (and the reverse) as the
    /// one shift by the difference, where the inner shift has other users (the
    /// pin has no use test). DEVIATE: a
    /// target whose word shift is one instruction for both halves says no, so
    /// that the other half is read from the shift it already has, not made by a
    /// second one. Exit: none; the pin's word shifts are pieces by then and
    /// node CSE shares them. Default true, the pin's.
    fn fold_shift_of_shared_shift(
        &self,
        _vt: EVT,
        _inner: u32,
    ) -> bool {
        true
    }

    /// `visitORCommutative`'s absorption folds, `(or (and x, y), x)` is `x`.
    /// Default false until x86 is judged.
    fn fold_or_absorption(&self) -> bool {
        false
    }

    /// `convertSetCCLogicToBitwiseLogic`: `and (seteq a, b), (seteq c, d)` as
    /// one compare of `or (xor a, b), (xor c, d)`. Default true, as X86's
    /// (the pin's default is false).
    fn convert_setcc_logic_to_bitwise_logic(&self) -> bool {
        true
    }

    /// `getShiftAmountTy` for a shift of `shifted`: x86's is `i8`. DEVIATE:
    /// the pin asks the target; AArch64 shifts by a value of the shifted type.
    /// Exit: x86 judged.
    fn shift_amount_type(
        &self,
        _shifted: EVT,
    ) -> EVT {
        EVT::I8
    }

    /// DEVIATE G1: a store of a value the target converted through a scratch
    /// slot (store to the slot, load it back) is the conversion stored to the
    /// store's own address, when nothing else reads the slot. The pin keeps
    /// the slot. Exit: the pin gains it; re-check each pin bump.
    fn store_of_converted_slot(
        &self,
        _dag: &mut SelectionDAG,
        _store: SDValue,
    ) -> Option<SDValue> {
        None
    }

    /// DEVIATE LS1 (owner m16, exit 2026-11-17): a plain `i32` load every use
    /// of which is a word half (`trunc i16`, or `trunc i16 (srl 16)`) is
    /// two `i16` loads. The pin's `ReduceLoadWidth`
    /// ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17095))
    /// narrows a load only when nothing else reads it, so the pin keeps
    /// one `mov eax,[m]` and a `shld`. m16 only: its halves are the registers
    /// a `long` is returned and passed in (DX:AX), and a 16-bit register file
    /// has no dword half to extract for free. Default off.
    fn split_dword_loads(&self) -> bool {
        false
    }

    /// DEVIATE PAIR1: whether the after-legalize combine ends with a sweep that
    /// packs the joins of two words (`pack_words_by_shrd`). Default off.
    fn pack_word_pairs(&self) -> bool {
        false
    }

    /// Whether `user` compares its first operand with its second, so an `or`
    /// feeding it against zero sets the flags it reads. Default false.
    fn is_compare_with_flags(
        &self,
        _user: ISD,
    ) -> bool {
        false
    }

    /// `replaceStoreOfFPConstant` stores a `double` constant as one `i64` when
    /// that type is legal. Default false until x86 is judged.
    fn store_fp_constant_as_i64(&self) -> bool {
        false
    }

    /// `reassociateOpsCommutative`'s pairing of `and`/`or` operands that are
    /// compares of one predicate. DEVIATE: off by default (the pin runs it
    /// for every target). Exit: x86 judged.
    fn reassociate_compare_pairs(&self) -> bool {
        false
    }

    /// `visitSELECT`'s `select (setcc x, y, cc), a, b` to `select_cc x, y, a,
    /// b, cc`, which the pin makes before legalizing when `SELECT_CC` is
    /// legal or custom for `vt`. x86 has no `SELECT_CC`.
    fn select_to_select_cc(
        &self,
        _vt: EVT,
    ) -> bool {
        false
    }

    /// `isTruncateFree(wide, narrow) && isZExtFree(narrow, wide)`: the casts
    /// `ShrinkDemandedOp` and the shift narrowings of `SimplifyDemandedBits`
    /// need to run an operation at the narrower type. X86 has them only from
    /// i64 to i32 on x86-64 (a 32-bit result zero-extends implicitly).
    fn free_narrowing(
        &self,
        _wide: EVT,
        _narrow: EVT,
    ) -> bool {
        false
    }

    /// DEVIATE SD257 (owner llrm-target-6502, exit 2026-12-05):
    /// `ShrinkDemandedOp` sizes the narrow type by the bits that are
    /// demanded and not known zero, not by the demanded bits alone (the
    /// pin). Off elsewhere: x86-64 `div_constants_short` +80 B (the
    /// narrowed `imul` loses its fusion).
    fn known_zero_narrowing(&self) -> bool {
        false
    }

    /// `isNarrowingProfitable(N, wide, narrow)`; X86: not i32 to i16.
    fn narrowing_profitable(
        &self,
        _wide: EVT,
        _narrow: EVT,
    ) -> bool {
        true
    }

    /// `isIndexedLoadLegal`/`isIndexedStoreLegal` of `PRE_INC`/`POST_INC` (the
    /// only modes made) for an access of `vt`. x86 has no indexed forms.
    fn is_indexed_legal(
        &self,
        _is_load: bool,
        _vt: EVT,
        _post: bool,
    ) -> bool {
        false
    }

    /// `getPreIndexedAddressParts`/`getPostIndexedAddressParts`: the base and
    /// the signed offset of `access` (a load or store) when `address` (an
    /// add or subtract) can be its update; the offset is made a constant of
    /// the type of the address's second operand.
    fn indexed_address_parts(
        &self,
        _dag: &super::dag::SelectionDAG,
        _access: SDValue,
        _address: SDValue,
        _post: bool,
    ) -> Option<(SDValue, i64)> {
        None
    }

    /// `isLegalAddressingMode` of `[reg + base_offs]` (scale 0) or `[reg +
    /// reg]` (scale 1) for an access of `vt`.
    fn is_legal_addressing_mode(
        &self,
        _vt: EVT,
        _base_offs: i64,
        _scale: i64,
    ) -> bool {
        false
    }

    /// `visitSUB`'s `sub C, X` is `xor X, C` where X cannot borrow from C's
    /// bits
    /// ([L4651](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4651)).
    /// DEVIATE SW2 (owner w9, 2026-11-08): on only where a target states it;
    /// the x86 targets do (the Dag switch-over: `c ? 4 : 5` at i16 is `setl;
    /// xor 5`). Exit: every target on, after their integration sweeps.
    fn sub_without_borrow_is_xor(&self) -> bool {
        false
    }

    /// `visitSUB`'s algebraic folds (`a - (b - c)` is `a + (c - b)`, the
    /// cancelling pairs, the constants that meet). DEVIATE: off by default.
    /// On x86 the regrouped remainder `x - (8q - q)` keeps `q` live longer and
    /// moved `div_constants_long` at x86_16 -O2 from 2062 to 2066 instructions
    /// through a different spill. Exit: 2026-11-20, the x86_16 spill is fixed
    /// (or the fold is judged with it) and this default turns on.
    fn sub_algebra_folds(&self) -> bool {
        false
    }

    /// `add x, (zext i1 y)` is `sub x, (sext i1 y)` where booleans are 0/-1:
    /// the sign extension of a compare is the cheaper one (`slt; extb.l` 4 B
    /// against `slt; and.l #1` 8 B on the 68020), and the pin's `sub x, (zext
    /// i1 y)` to `add x, (sext i1 y)` fold has no dual for the add.
    /// DEVIATE: not in the pin. Exit: review 2026-11-20.
    fn sub_of_sign_extended_bool(&self) -> bool {
        false
    }

    /// The type `BuildUDIV` widens `vt` to for an uncooperative divisor
    /// ([L7031]: `i32` to `i64`), or `None` where the target does not.
    fn widened_unsigned_divide_type(
        &self,
        _vt: EVT,
    ) -> Option<EVT> {
        None
    }

    /// `isZExtFree(i32, i64)`, which keeps the even-divisor rewrite of
    /// `BuildUDIV` out of the way of the widened multiply (x86-64 and AArch64).
    fn zero_extend_i32_to_i64_free(&self) -> bool {
        false
    }

    /// `BuildSDIV`/`BuildUDIV`'s "a legal multiply of the wide type" for any
    /// width, not only `i16` and below. DEVIATE: off by default, as above.
    /// Exit: x86 judged.
    fn widen_multiply_high_any_width(&self) -> bool {
        false
    }

    /// Whether `BuildUDIV`/`BuildSDIV` put the magic factor first in the
    /// multiply-high, so the dividend is the operand a target that reads one
    /// operand's length (the 486's `mul` early-out) reads. The pin's order is
    /// (dividend, magic). DEVIATE SD444 (owner w2, exit 2026-12-20; upstream
    /// check: gcc `expand_mult_highpart` and i386 `mult_bit` at each pin bump):
    /// off by default. Asked here, not at selection, because only here the
    /// dividend is still a node whose known bits give its length (a `urem`
    /// result has its leading zeros; once expanded to `x - q*c` it has none).
    fn multiply_high_magic_first(
        &self,
        _dag: &SelectionDAG,
        _signed: bool,
        _dividend: SDValue,
        _magic: SDValue,
    ) -> bool {
        false
    }

    /// `SimplifySetCC`'s "Canonicalize GE/LE comparisons to use GT/LT" (`x >=
    /// c` is `x > c - 1`). DEVIATE: off by default. x86's rows are the old
    /// selector's, which keeps a range check as `cmp n; jae`; the rewrite
    /// turns it into `cmp n - 1; ja` and moved 35 of its 424 judged rows
    /// (`switch` in every language). Exit: re-bless the x86 rows.
    fn canonicalize_ge_le(&self) -> bool {
        false
    }

    /// `isTypeDesirableForOp(ISD::SETCC, VT)` for the narrower type
    /// `SimplifySetCC` compares a zero-extended or masked value in
    /// ([TargetLowering.cpp#L4950](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L4950)).
    /// The pin's default is true; x86 returns false for an illegal type. The
    /// default here is x86's, as every judged target's rows are, so a target
    /// that wants the pin's default overrides it.
    fn is_setcc_type_desirable(
        &self,
        _narrow: EVT,
        legal: bool,
    ) -> bool {
        legal
    }

    /// `SimplifySetCC`'s `(zext x) == C --> x == (trunc C)` and its masked
    /// forms without the pin's `N0->hasOneUse()`
    /// ([TargetLowering.cpp#L4948](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L4948)).
    /// Also an `AssertZext` operand, which is what a mask made in another block
    /// is to a compare (the live-out copies). DEVIATE: off by default (the
    /// pin). The narrowed compare reads `x`, so the extension stays for its
    /// other users and nothing is duplicated. Owner z80,
    /// exit 2026-11-27: a lane rule at the byte expansion of the compare.
    fn narrow_setcc_with_other_uses(&self) -> bool {
        false
    }

    fn has_target_combine(
        &self,
        _op: ISD,
    ) -> bool {
        false
    }

    /// The replacement for `node`, or `None` for no change.
    fn perform_dag_combine(
        &mut self,
        _dag: &mut SelectionDAG,
        _node: SDValue,
        _level: CombineLevel,
    ) -> Option<SDValue> {
        None
    }

    /// `targetShrinkDemandedConstant`: `None` is "not handled" (the generic
    /// shrink runs); `Some(None)` keeps the node as it is; `Some(Some(new))`
    /// replaces it.
    fn target_shrink_demanded_constant(
        &mut self,
        _dag: &mut SelectionDAG,
        _op: SDValue,
        _demanded: u128,
        _level: CombineLevel,
    ) -> Option<Option<SDValue>> {
        None
    }

    /// `IsDesirableToPromoteOp`, with `isTypeDesirableForOp` before it: the
    /// type a legal operation is better done in, or `None`.
    fn promote_type(
        &self,
        _dag: &SelectionDAG,
        _tli: &TargetLowering,
        _op: SDValue,
    ) -> Option<EVT> {
        None
    }
}

/// A target with no combines.
pub struct NoCombines;

impl CombineHooks for NoCombines {}

/// A set of node ids, one flag each (ids are dense).
#[derive(Default)]
struct NodeSet(Vec<bool>);

impl NodeSet {
    /// Whether `node` was not in the set.
    fn insert(
        &mut self,
        node: u32,
    ) -> bool {
        let at = node as usize;
        if self.0.len() <= at {
            self.0.resize(at + 1, false);
        }
        !std::mem::replace(&mut self.0[at], true)
    }

    fn remove(
        &mut self,
        node: u32,
    ) {
        if let Some(flag) = self.0.get_mut(node as usize) {
            *flag = false;
        }
    }

    fn contains(
        &self,
        node: u32,
    ) -> bool {
        self.0.get(node as usize).copied().unwrap_or(false)
    }
}

pub struct DagCombiner<'a, H: CombineHooks> {
    pub(super) dag: &'a mut SelectionDAG,
    pub(super) hooks: &'a mut H,
    pub(super) level: CombineLevel,
    worklist: Vec<u32>,
    /// The last sweep of an after-legalize combine (`pack_word_pairs`).
    pub(super) late: bool,
    queued: NodeSet,
    /// `CombinedNodes`: the nodes already visited, whose operands are not
    /// queued again by a visit of their user.
    combined: NodeSet,
    /// The action table, for the folds that ask what the target can do.
    pub(super) tli: Option<&'a TargetLowering>,
    /// `OptLevel == CodeGenOptLevel::None`: the chain, store and load combines
    /// the pin skips at `-O0` are skipped.
    pub(super) opt_none: bool,
    /// `BatchAA`, for a target whose `useAA` is true.
    pub(super) alias: Option<&'a dyn super::chain_combine::AliasOracle>,
}

fn op_is_srl(op: ISD) -> bool {
    op == ISD::Srl
}

impl<'a, H: CombineHooks> DagCombiner<'a, H> {
    pub fn new(
        dag: &'a mut SelectionDAG,
        hooks: &'a mut H,
        level: CombineLevel,
    ) -> DagCombiner<'a, H> {
        DagCombiner {
            dag,
            hooks,
            level,
            worklist: Vec::new(),
            late: false,
            queued: NodeSet::default(),
            combined: NodeSet::default(),
            tli: None,
            opt_none: false,
            alias: None,
        }
    }

    /// The action table the folds query (`TLI`).
    pub fn with_tli(
        mut self,
        tli: &'a TargetLowering,
    ) -> DagCombiner<'a, H> {
        self.tli = Some(tli);
        self
    }

    /// The alias analysis `mayAlias` asks when the addresses decide nothing.
    pub fn with_alias(
        mut self,
        alias: &'a dyn super::chain_combine::AliasOracle,
    ) -> DagCombiner<'a, H> {
        self.alias = Some(alias);
        self
    }

    /// The pin's `-O0` combiner (`OptLevel == None`).
    pub fn with_opt_none(mut self) -> DagCombiner<'a, H> {
        self.opt_none = true;
        self
    }

    pub(super) fn add_to_worklist(
        &mut self,
        node: u32,
    ) {
        if self.queued.insert(node) {
            self.worklist.push(node);
        }
    }

    pub(super) fn add_to_worklist_with_users(
        &mut self,
        node: u32,
    ) {
        let results = self.dag.node(SDValue { node, res: 0 }).values.len() as u8;
        for res in 0..results {
            let users: Vec<u32> = self.dag.users(SDValue { node, res }).collect();
            for user in users {
                self.add_to_worklist(user);
            }
        }
        self.add_to_worklist(node);
    }

    fn next(&mut self) -> Option<u32> {
        let node = self.worklist.pop()?;
        self.queued.remove(node);
        Some(node)
    }

    /// A node nothing reads that is not the root
    /// (`recursivelyDeleteUnusedNodes`).
    fn is_dead(
        &self,
        node: u32,
    ) -> bool {
        if node == self.dag.get_root().node {
            return false;
        }
        let results = self.dag.node(SDValue { node, res: 0 }).values.len() as u8;
        !(0..results).any(|res| self.dag.has_users(SDValue { node, res }))
    }

    /// `Run`.
    pub fn run(&mut self) {
        // `RemoveDeadNodes`: what an earlier phase replaced no longer counts as
        // a user. Newest first, so a user goes before the nodes it reads.
        for node in self.dag.node_ids().rev() {
            self.dag.delete_dead(node);
        }
        // Oldest first, so the newest is taken first: `Run` without topological
        // sorting, the default of `useTopologicalSorting`
        // ([L1853-L1873][run], [L4203][tls]).
        //
        // [run]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L1853-L1873
        // [tls]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/include/llvm/CodeGen/TargetLowering.h#L4203
        for node in self.dag.node_ids() {
            self.add_to_worklist(node);
        }
        self.drain();
        // DEVIATE PAIR1: what the folds above settled, then the joins of two
        // words that remain (their dword is used whole) are packed.
        if self.level == CombineLevel::AfterLegalizeDag && self.hooks.pack_word_pairs() {
            self.late = true;
            for node in self.dag.node_ids() {
                if self.dag.opcode(SDValue { node, res: 0 }) == ISD::Or {
                    self.add_to_worklist(node);
                }
            }
            self.drain();
            self.late = false;
        }
        // `RemoveDeadNodes` before selection: a node a fold left behind must
        // not count as a user of what it read (a load folds into its one user).
        for node in self.dag.node_ids().rev() {
            self.dag.delete_dead(node);
        }
    }

    /// The worklist walk of `Run`: until it is empty.
    fn drain(&mut self) {
        while let Some(n) = self.next() {
            if self.is_dead(n) {
                continue;
            }
            // "Add any operands of the new node which have not yet been
            // combined to the worklist as well" (`CombinedNodes`): without the
            // test a DAG whose nodes share operands is walked once per path.
            let ops: Vec<u32> = self.dag.node(SDValue { node: n, res: 0 }).ops.iter().map(|op| op.node).collect();
            for op in ops {
                if !self.combined.contains(op) {
                    self.add_to_worklist(op);
                }
            }
            self.combined.insert(n);
            // `SDLoc(N)`: what the combine makes takes the node's order.
            self.dag.cur_order = self.dag.node(SDValue { node: n, res: 0 }).order;
            let before = self.dag.node_count();
            let result = self.combine(SDValue { node: n, res: 0 });
            self.queue_survivors();
            for (old, new) in self.dag.take_replacements() {
                self.replace_value(old, new);
                self.add_to_worklist_with_users(new.node);
                self.dag.delete_dead(old.node);
                self.queue_survivors();
            }
            let Some(result) = result else { continue };
            if result.node == n && result.res == 0 {
                continue;
            }
            let results = self.dag.node(SDValue { node: n, res: 0 }).values.len();
            let replacement = self.dag.node(result).values.len();
            let count = if results == replacement { results } else { 1 };
            for res in 0..count as u8 {
                let new = if results == replacement { SDValue { node: result.node, res } } else { result };
                for event in self.dag.replace_all_uses_with(SDValue { node: n, res }, new) {
                    match event {
                        Rauw::Updated(user) => self.add_to_worklist(user),
                        Rauw::Merged { into, .. } => self.add_to_worklist_with_users(into),
                    }
                }
            }
            if self.dag.opcode(result) != ISD::EntryToken {
                self.add_to_worklist_with_users(result.node);
            }
            self.dag.delete_dead(n);
            self.dag.delete_replaced(n);
            self.queue_survivors();
        }
    }

    /// `CombineTo(old, new)` with `AddTo`: the new node and its users are
    /// queued, then the old node goes and its surviving operands are queued.
    pub(super) fn combine_store_to(
        &mut self,
        old: SDValue,
        new: SDValue,
    ) {
        self.replace_value(old, new);
        self.add_to_worklist_with_users(new.node);
        self.dag.delete_replaced(old.node);
        self.queue_survivors();
    }

    pub(super) fn queue_survivors(&mut self) {
        for node in std::mem::take(&mut self.dag.survivors) {
            self.add_to_worklist(node);
        }
    }

    /// `combine`: the generic fold, else the target's.
    fn combine(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let mut result = self.visit(node);
        if result.is_none() && (matches!(op, ISD::Target(_)) || self.hooks.has_target_combine(op)) {
            result = self.hooks.perform_dag_combine(self.dag, node, self.level);
        }
        // "If nothing happened still, try promoting the operation."
        if result.is_none() && self.level >= CombineLevel::AfterLegalizeVectorOps {
            result = self.promote_int_op(node);
        }
        result
    }

    fn visit(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let result = self.visit_opcode(node);
        if result.is_none()
            && self.hooks.fold_bin_op_into_select()
            && matches!(
                self.dag.opcode(node),
                ISD::Add
                    | ISD::Sub
                    | ISD::Mul
                    | ISD::SDiv
                    | ISD::UDiv
                    | ISD::SRem
                    | ISD::URem
                    | ISD::And
                    | ISD::Or
                    | ISD::Xor
                    | ISD::Shl
                    | ISD::Sra
                    | ISD::Srl
            )
        {
            return self.fold_bin_op_into_select(node);
        }
        result
    }

    /// `DAGCombiner::foldBinOpIntoSelect` for an integer operator: `add
    /// (select c, C1, C2), C3` is `select c, C1 + C3, C2 + C3`, and `and`
    /// or `or` of a select of 0 and -1 takes the other operand into the
    /// select.
    fn fold_bin_op_into_select(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let opcode = self.dag.opcode(node);
        let vt = self.dag.value_type(node);
        if !vt.is_integer() {
            return None;
        }
        // The old select must go away with the operator.
        let (mut at, mut sel) = (0, self.dag.operand(node, 0));
        if self.dag.opcode(sel) != ISD::Select || !self.dag.has_one_use(sel.node) {
            (at, sel) = (1, self.dag.operand(node, 1));
        }
        if self.dag.opcode(sel) != ISD::Select || !self.dag.has_one_use(sel.node) {
            return None;
        }
        let (cond, ct, cf) = (self.dag.operand(sel, 0), self.dag.operand(sel, 1), self.dag.operand(sel, 2));
        let (kt, kf) = (self.dag.constant(ct)?, self.dag.constant(cf)?);
        let mask = super::fold::mask(vt.size_in_bits());
        let (kt, kf) = (kt & mask, kf & mask);
        let other = self.dag.operand(node, at ^ 1);
        let (opaque_t, opaque_f) = (self.dag.is_opaque(ct), self.dag.is_opaque(cf));
        let zero_ones = (kt == 0 && kf == mask) || (kf == 0 && kt == mask);
        let free_other = matches!(opcode, ISD::And | ISD::Or) && zero_ones;
        let constant_other = self.dag.constant(other).is_some() && !self.dag.is_opaque(other);
        if opaque_t || opaque_f || (!free_other && !constant_other) {
            return None;
        }
        let (new_t, new_f) = if free_other {
            // `and (select c, 0, -1), x` is `select c, 0, x`; `or`, with -1.
            let absorbing = if opcode == ISD::And { 0 } else { mask };
            (if kt == absorbing { ct } else { other }, if kf == absorbing { cf } else { other })
        } else if at == 1 {
            (self.dag.get_node(opcode, vt, &[other, ct]), self.dag.get_node(opcode, vt, &[other, cf]))
        } else {
            (self.dag.get_node(opcode, vt, &[ct, other]), self.dag.get_node(opcode, vt, &[cf, other]))
        };
        if !free_other && (self.dag.constant(new_t).is_none() || self.dag.constant(new_f).is_none()) {
            return None;
        }
        Some(self.dag.get_select(vt, cond, new_t, new_f))
    }

    fn visit_opcode(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        match self.dag.opcode(node) {
            ISD::MergeValues => self.visit_merge_values(node),
            ISD::Load => self.visit_load(node).or_else(|| self.indexed(node)),
            ISD::TokenFactor => self.visit_token_factor(node),
            ISD::Select => self.visit_select(node),
            ISD::SelectCC => self.visit_select_cc(node),
            ISD::Store => self.visit_store(node).or_else(|| self.indexed(node)),
            ISD::LifetimeEnd => self.visit_lifetime_end(node),
            ISD::Freeze => self.visit_freeze(node),
            ISD::InsertVectorElt => self.visit_insert_vector_elt(node),
            ISD::ExtractElement => self.visit_extract_vector_elt(node),
            ISD::SetCC => self.visit_setcc(node),
            ISD::BrCond => self.visit_brcond(node),
            ISD::Xor => self.visit_xor(node).or_else(|| self.simplify_demanded(node)),
            ISD::Mul => self.visit_mul(node).or_else(|| self.simplify_demanded(node)),
            ISD::And => self
                .visit_and(node)
                .or_else(|| self.reduce_load_width_of(node))
                .or_else(|| self.simplify_demanded(node))
                .or_else(|| self.and_of_extload(node)),
            ISD::Or => {
                self.visit_or(node).or_else(|| self.simplify_demanded(node)).or_else(|| self.visit_or_as_add(node))
            }
            ISD::Add => self.visit_add(node).or_else(|| self.simplify_demanded(node)),
            ISD::Sub => self.visit_sub(node),
            ISD::UAddO => self.visit_uaddo(node),
            ISD::UMulO | ISD::SMulO => self.visit_mulo(node),
            ISD::UAddOCarry => self.visit_uaddo_carry(node),
            ISD::USubO => self.visit_usubo(node),
            ISD::USubOCarry => self.visit_usubo_carry(node),
            ISD::SetCCCarry => self.visit_setcc_carry(node),
            ISD::FAdd => self.visit_fadd(node),
            ISD::FSub => self.visit_fsub(node),
            ISD::FMul => self.visit_fmul(node),
            ISD::FNeg => self.visit_fneg(node),
            ISD::FpRound => self.visit_fp_round(node),
            ISD::FpExtend => self.visit_fp_extend(node),
            ISD::SIntToFp | ISD::FpToSInt => self.visit_fp_int_convert(node),
            ISD::FAbs => self.visit_fabs(node),
            ISD::FDiv => self.visit_fdiv(node),
            ISD::Shl | ISD::Srl => self
                .visit_shift(node)
                .or_else(|| if op_is_srl(self.dag.opcode(node)) { self.reduce_load_width_of(node) } else { None })
                .or_else(|| self.simplify_demanded(node)),
            ISD::Sra => self
                .visit_sra(node)
                .or_else(|| self.visit_shift(node))
                .or_else(|| self.reduce_load_width_of(node))
                .or_else(|| self.simplify_demanded(node)),
            // `visitFunnelShift`, closing with `SimplifyDemandedBits(N)`.
            ISD::FShl | ISD::FShr => self.visit_funnel_shift(node).or_else(|| self.simplify_demanded(node)),
            ISD::BSwap => self.visit_bswap(node),
            ISD::Abs | ISD::AbsMinPoison => self.visit_abs(node),
            ISD::CtLz | ISD::CtTz => self.visit_ctlz_cttz(node),
            ISD::CtPop => self.visit_ctpop(node),
            ISD::Rotl | ISD::Rotr => self.visit_rotate(node).or_else(|| self.simplify_demanded(node)),
            ISD::SMin | ISD::SMax | ISD::UMin | ISD::UMax => {
                self.visit_iminmax(node).or_else(|| self.simplify_demanded(node))
            }
            ISD::BitReverse => self.visit_bitreverse(node),
            ISD::SDiv => self.visit_sdiv(node),
            ISD::UDiv => self.visit_udiv(node).or_else(|| self.simplify_demanded(node)),
            // `visitMULHU`: only its closing `SimplifyDemandedBits` (constant
            // folding by known bits) is translated.
            ISD::MulHU => self.simplify_demanded(node),
            ISD::SRem | ISD::URem => self.visit_rem(node),
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend => self
                .fold_extend_of_cast(node)
                .or_else(|| self.visit_extend(node))
                .or_else(|| self.sext_of_non_negative(node))
                .or_else(|| self.zext_nneg_of_existing_sext(node)),
            ISD::BitCast => self.visit_bitcast(node),
            ISD::Truncate => self.visit_truncate(node).or_else(|| self.simplify_demanded(node)),
            ISD::SignExtendInReg => self.visit_sext_in_reg(node),
            _ => None,
        }
    }

    /// `CombineToPreIndexedLoadStore || CombineToPostIndexedLoadStore`.
    fn indexed(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        self.combine_to_pre_indexed_load_store(node).or_else(|| self.combine_to_post_indexed_load_store(node))
    }

    /// `if (SimplifyDemandedBits(SDValue(N, 0))) return SDValue(N, 0)`.
    fn simplify_demanded(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        self.simplify_demanded_bits(node).then_some(node)
    }

    /// `visitMERGE_VALUES` ([L2446][pin]): each result is its operand, so a
    /// lowering that returns several values as one `MERGE_VALUES` leaves no
    /// such node for selection. Returns the node itself so it is not looked
    /// at again.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2446
    fn visit_merge_values(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let count = self.dag.node(node).values.len();
        for res in 0..count {
            let operand = self.dag.operand(node, res);
            self.replace_value(SDValue { node: node.node, res: res as u8 }, operand);
        }
        Some(SDValue { node: node.node, res: 0 })
    }

    /// `visitSETCC` through `SimplifySetCC` ([TargetLowering.cpp L4831][pin]),
    /// the integer folds after `FoldSetCC` (which `get_node` already ran): a
    /// constant on the left moves right, and an equality of an extension (or
    /// of an `and` with a low mask) with a constant that fits the narrow type
    /// compares the narrow value.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L4831
    fn visit_setcc(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let NodeData::CondCode(cond) = self.dag.node(self.dag.operand(node, 2)).data else { return None };
        let op_type = self.dag.value_type(n0);
        // `SimplifySetCC` ([L5632][pin]): `setuo|seto x, C` of a constant that
        // is no NaN is `setuo|seto x, x`, which needs no 0.0 materialized.
        // DEVIATE: the `setcc (fneg x), C` arm after it is not ported.
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5632
        if op_type.is_floating_point() {
            if matches!(cond, CondCode::SetO | CondCode::SetUo)
                && self.dag.opcode(n0) != ISD::ConstantFP
                && self.fp_const(n1).is_some_and(|class| !class.nan)
            {
                let vt = self.dag.value_type(node);
                return Some(self.dag.get_setcc(vt, n0, n0, cond));
            }
            return None;
        }
        if !op_type.is_integer() {
            return None;
        }
        let vt = self.dag.value_type(node);
        let (c0, c1) = (self.dag.constant(n0), self.dag.constant(n1));
        if c0.is_some() && c1.is_none() {
            return Some(self.dag.get_setcc(vt, n1, n0, cond.swapped_operands()));
        }
        let c1 = c1?;
        // `ExpandSetCCOperands`: a pair equals a constant when both halves do
        // (`(lo ^ c.lo) | (hi ^ c.hi)` is zero), so a long a call returned in
        // dx:ax is compared by halves, not joined into one dword first.
        if matches!(cond, CondCode::SetEq | CondCode::SetNe)
            && self.dag.opcode(n0) == ISD::BuildPair
            && !self.tli.is_some_and(|tli| tli.byte_equality())
        {
            let (low, high) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let half = self.dag.value_type(low);
            let bits = half.size_in_bits();
            let part = |this: &mut Self, value: SDValue, constant: u128| {
                if constant == 0 {
                    value
                } else {
                    let constant = this.dag.get_constant(constant, half);
                    this.dag.get_node(ISD::Xor, half, &[value, constant])
                }
            };
            let low = part(self, low, c1 & super::fold::mask(bits));
            let high = part(self, high, (c1 >> bits) & super::fold::mask(bits));
            let either = self.dag.get_node(ISD::Or, half, &[low, high]);
            let zero = self.dag.get_constant(0, half);
            return Some(self.dag.get_setcc(vt, either, zero, cond));
        }
        // `SimplifySetCC` ([L5270][pin]): `(setcc (trunc? (assertzext x, i1)),
        // 1, eq|ne)` is `(setcc x, 0, ne|eq)`. DEVIATE: the other two
        // branches of `N1C->isOne()` (a setcc pair, `(x & 1)`) are not
        // ported.
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5270
        // `(setcc (setcc a, b, cc), 0|1, eq|ne)` is the inner compare or its
        // inverse ([L5185][pin]). DEVIATE: only for an inner `setcc` (known 0/1
        // by construction), without `isTypeLegal(VT)`: the combiner visits a
        // branch before the condition it reads is settled.
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L5185
        if self.hooks.rebuild_brcond_condition()
            && matches!(cond, CondCode::SetEq | CondCode::SetNe)
            && matches!(c1, 0 | 1)
            && vt == EVT::I1
            && op_type == EVT::I1
            && self.dag.opcode(n0) == ISD::SetCC
        {
            let same = (cond == CondCode::SetEq) ^ (c1 != 1);
            if same {
                return Some(n0);
            }
            if let NodeData::CondCode(inner) = self.dag.node(self.dag.operand(n0, 2)).data {
                let lhs = self.dag.operand(n0, 0);
                let inverse = if self.dag.value_type(lhs).is_floating_point() {
                    inner.inverse_float()
                } else {
                    inner.inverse_integer()
                };
                return Some(self.dag.get_setcc(vt, lhs, self.dag.operand(n0, 1), inverse));
            }
        }
        if self.hooks.rebuild_brcond_condition() && matches!(cond, CondCode::SetEq | CondCode::SetNe) && c1 == 1 {
            let op0 = if self.dag.opcode(n0) == ISD::Truncate { self.dag.operand(n0, 0) } else { n0 };
            if self.dag.opcode(op0) == ISD::AssertZext
                && matches!(
                    self.dag.node(self.dag.operand(op0, 1)).data,
                    NodeData::ValueType(EVT::I1)
                )
            {
                let zero = self.dag.get_constant(0, self.dag.value_type(op0));
                let flipped = if cond == CondCode::SetEq { CondCode::SetNe } else { CondCode::SetEq };
                return Some(self.dag.get_setcc(vt, op0, zero, flipped));
            }
        }
        // `SimplifySetCC`'s remainder of a division by a constant, unsigned.
        if matches!(cond, CondCode::SetEq | CondCode::SetNe)
            && self.dag.opcode(n0) == ISD::URem
            && let Some(folded) = self.build_urem_eq_fold(vt, n0, c1, cond)
        {
            return Some(folded);
        }
        // `(x + c0) == c1` is `x == c1 - c0`, `(x ^ c0) == c1` is `x == c0 ^
        // c1` and `(c0 - x) == c1` is `x == c0 - c1`, for a single-use
        // operand.
        if matches!(cond, CondCode::SetEq | CondCode::SetNe)
            && matches!(self.dag.opcode(n0), ISD::Add | ISD::Xor | ISD::Sub)
            && self.dag.has_one_use(n0.node)
        {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let width = op_type.size_in_bits();
            let folded = match (self.dag.opcode(n0), self.dag.constant(a), self.dag.constant(b)) {
                (ISD::Add, _, Some(c0)) => Some((a, c1.wrapping_sub(c0))),
                (ISD::Xor, _, Some(c0)) => Some((a, c0 ^ c1)),
                (ISD::Sub, Some(c0), _) => Some((b, c0.wrapping_sub(c1))),
                _ => None,
            };
            if let Some((x, value)) = folded {
                let value = self.dag.get_constant(value & super::fold::mask(width), op_type);
                return Some(self.dag.get_setcc(vt, x, value, cond));
            }
        }
        if let Some(folded) = self.fold_setcc_with_and(vt, n0, c1, cond) {
            return Some(folded);
        }
        if let Some(folded) = self.signed_truncation_check(vt, n0, c1, cond) {
            return Some(folded);
        }
        if let Some(folded) = super::setcc_extreme::fold_against_an_extreme(self.dag, vt, n0, c1, cond) {
            return Some(folded);
        }
        // `SimplifySetCC`, "Canonicalize GE/LE comparisons to use GT/LT": `x >=
        // c` is `x > c - 1` and `x <= c` is `x < c + 1`, with the
        // always-true extremes folded.
        if self.level < CombineLevel::AfterLegalizeVectorOps && self.hooks.canonicalize_ge_le() {
            let bits = op_type.size_in_bits();
            let (unsigned, ge) = match cond {
                CondCode::SetGe => (false, true),
                CondCode::SetUGe => (true, true),
                CondCode::SetLe => (false, false),
                CondCode::SetULe => (true, false),
                _ => (false, matches!(cond, CondCode::SetEq)),
            };
            if matches!(
                cond,
                CondCode::SetGe | CondCode::SetUGe | CondCode::SetLe | CondCode::SetULe
            ) {
                let mask = super::fold::mask(bits);
                let (min, max) = if unsigned { (0, mask) } else { (1u128 << (bits - 1), mask >> 1) };
                let edge = if ge { min } else { max };
                if c1 & mask == edge {
                    return Some(self.dag.get_bool_constant(true, vt));
                }
                let moved = if ge { c1.wrapping_sub(1) } else { c1.wrapping_add(1) } & mask;
                let new_cond = match (ge, unsigned) {
                    (true, false) => CondCode::SetGt,
                    (true, true) => CondCode::SetUGt,
                    (false, false) => CondCode::SetLt,
                    (false, true) => CondCode::SetULt,
                };
                let rhs = self.dag.get_constant(moved, op_type);
                return Some(self.dag.get_setcc(vt, n0, rhs, new_cond));
            }
        }
        if !matches!(cond, CondCode::SetEq | CondCode::SetNe)
            || self.level != CombineLevel::BeforeLegalizeTypes
            || !(self.dag.has_one_use(n0.node) || self.hooks.narrow_setcc_with_other_uses())
        {
            return None;
        }
        let width = op_type.size_in_bits();
        let (min_bits, pre_ext, signed) = match self.dag.opcode(n0) {
            ISD::ZeroExtend => (self.dag.width(self.dag.operand(n0, 0)), self.dag.operand(n0, 0), false),
            ISD::SignExtend => (self.dag.width(self.dag.operand(n0, 0)), self.dag.operand(n0, 0), true),
            ISD::And => {
                let mask = self.dag.constant(self.dag.operand(n0, 1))?;
                if (mask.wrapping_add(1) & mask) != 0 {
                    return None;
                }
                (mask.trailing_ones(), self.dag.operand(n0, 0), false)
            }
            // A value another block made (`assert_live_out_copies`): its
            // upper bits are known zero, so it is its own zero extension.
            ISD::AssertZext if self.hooks.narrow_setcc_with_other_uses() => {
                let NodeData::ValueType(known) = self.dag.node(self.dag.operand(n0, 1)).data else { return None };
                (known.size_in_bits(), self.dag.operand(n0, 0), false)
            }
            _ => return None,
        };
        // Bits the constant needs: `getSignificantBits` or `getActiveBits`.
        let reqd = if signed {
            let s = (c1 << (128 - width)) as i128 >> (128 - width);
            129 - (s ^ (s >> 127)).leading_zeros()
        } else {
            128 - c1.leading_zeros()
        };
        if min_bits == 0 || min_bits >= width || min_bits < reqd {
            return None;
        }
        let mut min_vt = EVT::integer(min_bits);
        let mut pre_ext = pre_ext;
        // DEVIATE (`narrow_setcc_with_other_uses`): a mask that other uses
        // keep, or an assertion, is compared as it stands, truncated to
        // the smallest legal type that holds it. Truncating the value
        // under it to the illegal `min_bits` made the promoted operand
        // a second mask beside the one kept (masked_selector_chain:
        // `and a, #7` at each compare). Owner z80, exit 2026-11-27.
        let shared_mask = self.dag.opcode(n0) == ISD::And && !self.dag.has_one_use(n0.node);
        if (shared_mask || self.dag.opcode(n0) == ISD::AssertZext)
            && let Some(tli) = self.tli
            && !tli.types.is_type_legal(min_vt)
            && let Some(legal) =
                (min_bits + 1..width).map(EVT::integer).find(|candidate| tli.types.is_type_legal(*candidate))
        {
            min_vt = legal;
            pre_ext = n0;
        }
        if let Some(tli) = self.tli {
            if !self.hooks.is_setcc_type_desirable(min_vt, tli.types.is_type_legal(min_vt)) {
                return None;
            }
        }
        let trunc = self.dag.get_node(ISD::Truncate, min_vt, &[pre_ext]);
        let (rhs, cond) = if min_bits == 1 && c1 == 1 {
            let flipped = if cond == CondCode::SetEq { CondCode::SetNe } else { CondCode::SetEq };
            (self.dag.get_constant(0, min_vt), flipped)
        } else {
            (self.dag.get_constant(c1, min_vt), cond)
        };
        Some(self.dag.get_setcc(vt, trunc, rhs, cond))
    }

    /// `foldSetCCWithAnd`'s power-of-two mask arm
    /// ([TargetLowering.cpp#L4340][pin]): `(X & 2^k) == 0` is `(trunc X to
    /// k+1 bits) >= 0` and `!= 0` is `< 0`, when the truncate is free and
    /// both types are legal; and the `(X & Y) == Y` arms for a constant Y.
    /// DEVIATE: a variable Y is not ported.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L4340
    fn fold_setcc_with_and(
        &mut self,
        vt: EVT,
        n0: SDValue,
        c1: u128,
        cond: CondCode,
    ) -> Option<SDValue> {
        if !matches!(cond, CondCode::SetEq | CondCode::SetNe) || self.dag.opcode(n0) != ISD::And {
            return None;
        }
        let op_type = self.dag.value_type(n0);
        let mask = self.dag.constant(self.dag.operand(n0, 1))?;
        let mask = mask & super::fold::mask(op_type.size_in_bits());
        if c1 != 0 {
            // `(X & Y) == Y`, Y a constant: a single bit is `(X & Y) != 0`;
            // else, where the target has and-not, `(~X & Y) == 0`.
            if !op_type.is_integer() || mask == 0 || c1 & super::fold::mask(op_type.size_in_bits()) != mask {
                return None;
            }
            if mask.is_power_of_two() {
                let inverse = if cond == CondCode::SetEq { CondCode::SetNe } else { CondCode::SetEq };
                let zero = self.dag.get_constant(0, op_type);
                return Some(self.dag.get_setcc(vt, n0, zero, inverse));
            }
            if self.dag.has_one_use(n0.node) && self.hooks.has_and_not_compare(op_type) {
                let not_x = self.dag.get_not(self.dag.operand(n0, 0), op_type);
                let y = self.dag.operand(n0, 1);
                let and = self.dag.get_node(ISD::And, op_type, &[not_x, y]);
                let zero = self.dag.get_constant(0, op_type);
                return Some(self.dag.get_setcc(vt, and, zero, cond));
            }
            return None;
        }
        if !mask.is_power_of_two() || !self.dag.has_one_use(n0.node) {
            return None;
        }
        let legal = |vt: EVT| self.tli.is_some_and(|tli| tli.types.is_type_legal(vt));
        let narrow = EVT::integer(128 - mask.leading_zeros());
        if !legal(op_type) || !legal(narrow) || !self.hooks.is_truncate_free(op_type, narrow) {
            return None;
        }
        let truncated = self.dag.get_zext_or_trunc(self.dag.operand(n0, 0), narrow);
        let zero = self.dag.get_constant(0, narrow);
        let new_cond = if cond == CondCode::SetEq { CondCode::SetGe } else { CondCode::SetLt };
        Some(self.dag.get_setcc(vt, truncated, zero, new_cond))
    }

    /// `optimizeSetCCOfSignedTruncationCheck`: `(add x, 1<<(k-1)) u< 1<<k`, or
    /// its inverse form `(add x, -(1<<(k-1))) u>= -(1<<k)`, is `(sext_inreg
    /// x, k) == x` (`!=` for the inverse).
    fn signed_truncation_check(
        &mut self,
        vt: EVT,
        n0: SDValue,
        c1: u128,
        cond: CondCode,
    ) -> Option<SDValue> {
        if self.dag.opcode(n0) != ISD::Add {
            return None;
        }
        let (x, add) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
        let c01 = self.dag.constant(add).filter(|_| !self.dag.is_opaque(add))?;
        let xvt = self.dag.value_type(x);
        let width = xvt.size_in_bits();
        let mask = super::fold::mask(width);
        let (mut i1, mut i01) = (c1 & mask, c01 & mask);
        let mut equal = match cond {
            CondCode::SetULt => true,
            CondCode::SetULe => {
                i1 = i1.wrapping_add(1) & mask;
                true
            }
            CondCode::SetUGt => {
                i1 = i1.wrapping_add(1) & mask;
                false
            }
            CondCode::SetUGe => false,
            _ => return None,
        };
        let fits = |i1: u128, i01: u128| i1 > i01 && i1.is_power_of_two() && i01.is_power_of_two();
        if !fits(i1, i01) {
            i1 = i1.wrapping_neg() & mask;
            i01 = i01.wrapping_neg() & mask;
            equal = !equal;
            if !fits(i1, i01) {
                return None;
            }
        }
        let (kept, kept_minus_one) = (i1.trailing_zeros(), i01.trailing_zeros());
        if kept != kept_minus_one + 1 || kept == 0 || kept >= width {
            return None;
        }
        if !self.hooks.should_transform_signed_truncation_check(xvt, kept) {
            return None;
        }
        let ty = self.dag.get_value_type_node(EVT::integer(kept));
        let extended = self.dag.get_node(ISD::SignExtendInReg, xvt, &[x, ty]);
        let new_cond = if equal { CondCode::SetEq } else { CondCode::SetNe };
        Some(self.dag.get_setcc(vt, extended, x, new_cond))
    }

    /// `visitSELECT` ([L13507][pin]) up to the setcc folds: a flipped
    /// condition swaps the arms; an `i1` select is logic
    /// (`foldBoolSelectToLogic`); a select of a select on the same arm
    /// merges into one with an `and`/`or` condition, and an `and`/`or`
    /// condition splits into a select sequence where the target asks for it
    /// (`normalize_to_select_sequence`, X86 does).
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13507
    /// `visitSELECT_CC`, the `SimplifySetCC` of its compare: a simpler compare
    /// is a new `select_cc`.
    fn visit_select_cc(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let ops: Vec<SDValue> = (0..5).map(|i| self.dag.operand(node, i)).collect();
        if ops[2] == ops[3] {
            return Some(ops[2]);
        }
        let vt = self.dag.value_type(node);
        let NodeData::CondCode(cond) = self.dag.node(ops[4]).data else { return None };
        let compare = self.dag.get_setcc(EVT::I1, ops[0], ops[1], cond);
        // `SimplifySelectCC`: a compare that folded to a constant names the
        // arm.
        if self.dag.opcode(compare) != ISD::SetCC {
            return self.dag.constant(compare).map(|value| if value != 0 { ops[2] } else { ops[3] });
        }
        let simpler = self.visit_setcc(compare)?;
        if self.dag.opcode(simpler) != ISD::SetCC {
            return None;
        }
        let (lhs, rhs, code) =
            (self.dag.operand(simpler, 0), self.dag.operand(simpler, 1), self.dag.operand(simpler, 2));
        Some(self.dag.get_node(ISD::SelectCC, vt, &[lhs, rhs, ops[2], ops[3], code]))
    }

    fn visit_select(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1, n2) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let vt = self.dag.value_type(node);
        let cond_vt = self.dag.value_type(n0);
        if vt == cond_vt && vt == EVT::I1 {
            if let Some(logic) = self.bool_select_to_logic(n0, n1, n2, vt) {
                return Some(logic);
            }
        }
        // `extractBooleanFlip`: `select (not c), a, b` is `select c, b, a`.
        if self.dag.opcode(n0) == ISD::Xor && self.dag.bool_constant(self.dag.operand(n0, 1)) == Some(true) {
            let flipped = self.dag.operand(n0, 0);
            return Some(self.dag.get_select(vt, flipped, n2, n1));
        }
        // The pin reaches the sign mask through `or (sext (setle x, -1)), 1`:
        // `SimplifySetCC` makes the compare `x < 0` and `visitSIGN_EXTEND`
        // folds `sext (setlt x, 0)` to `sra`. Neither is translated, so the
        // sign folds run first, with the same result. Whatever the width of
        // the condition (wasm's compare is an `i32`).
        if let Some(mask) = self.select_sign_folds(n0, n1, n2) {
            return Some(mask);
        }
        if cond_vt != EVT::I1 {
            return None;
        }
        if let Some(math) = self.fold_select_of_constants(n0, n1, n2, vt) {
            return Some(math);
        }
        let arm_vt = self.dag.value_type(n1);
        let normalize = self.hooks.normalize_to_select_sequence(vt, cond_vt);
        // `select (and c0, c1), x, y` is `select c0, (select c1, x, y), y`,
        // `select (or c0, c1), x, y` is `select c0, x, (select c1, x, y)`; the
        // pin also splits when the inner select exists already, which the
        // arena cannot tell from a dead one (only `normalize` splits here).
        if normalize && self.dag.has_one_use(n0.node) && matches!(self.dag.opcode(n0), ISD::And | ISD::Or) {
            let (c0, c1) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            match self.dag.opcode(n0) {
                ISD::And => {
                    let inner = self.dag.get_select(arm_vt, c1, n1, n2);
                    return Some(self.dag.get_select(arm_vt, c0, inner, n2));
                }
                ISD::Or => {
                    let inner = self.dag.get_select(arm_vt, c1, n1, n2);
                    return Some(self.dag.get_select(arm_vt, c0, n1, inner));
                }
                _ => {}
            }
        }
        // `select c0, (select c1, x, y), y` is `select (and c0, c1), x, y`: the
        // `and` itself without a normalizing target, else only where the two
        // compares fold to one (`visitANDLike`; its `add`/`srl` arm is not
        // translated).
        if self.dag.opcode(n1) == ISD::Select && self.dag.has_one_use(n1.node) {
            let (c1, x, y) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1), self.dag.operand(n1, 2));
            if y == n2 && self.dag.value_type(c1) == cond_vt {
                let and = if normalize {
                    self.fold_logic_of_setccs(true, n0, c1)
                } else {
                    Some(self.dag.get_node(ISD::And, cond_vt, &[n0, c1]))
                };
                if let Some(and) = and {
                    return Some(self.dag.get_select(arm_vt, and, x, n2));
                }
            }
        }
        // `select c0, x, (select c1, x, y)` is `select (or c0, c1), x, y`, the
        // same way (`visitORLike`; its `and`-pair arm is not translated).
        if self.dag.opcode(n2) == ISD::Select && self.dag.has_one_use(n2.node) {
            let (c1, x, y) = (self.dag.operand(n2, 0), self.dag.operand(n2, 1), self.dag.operand(n2, 2));
            if x == n1 && self.dag.value_type(c1) == cond_vt {
                let or = if normalize {
                    self.fold_logic_of_setccs(false, n0, c1)
                } else {
                    Some(self.dag.get_node(ISD::Or, cond_vt, &[n0, c1]))
                };
                if let Some(or) = or {
                    return Some(self.dag.get_select(arm_vt, or, n1, y));
                }
            }
        }
        if let Some(folded) = self.select_sign_folds(n0, n1, n2).or_else(|| self.select_single_bit_test(n0, n1, n2)) {
            return Some(folded);
        }
        if self.dag.opcode(n0) == ISD::SetCC && !self.legal_operations() && self.hooks.select_to_select_cc(vt) {
            let (lhs, rhs, code) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1), self.dag.operand(n0, 2));
            return Some(self.dag.get_node(ISD::SelectCC, vt, &[lhs, rhs, n1, n2, code]));
        }
        // The end of `visitSELECT` (DAGCombiner.cpp#L13713).
        self.select_to_pseudo_min_max(node)
    }

    /// `foldSelectOfConstants` ([L13011][pin]) for an `i1` condition before
    /// operations are legal: a select of two constants is an extend of the
    /// condition, plus or or-ed with a constant, instead of a branch. X86's
    /// `convertSelectOfConstantsToMath` is true for scalars and it has no
    /// `SELECT_CC`, so `shouldConvertSelectOfConstantsToMath` always holds. The
    /// non-`i1` condition arm and `foldSelectOfConstantsUsingSra` are not
    /// translated (the latter is covered by [`Self::select_sign_folds`]).
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13011
    fn fold_select_of_constants(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
        vt: EVT,
    ) -> Option<SDValue> {
        if !vt.is_integer()
            || self.level >= CombineLevel::AfterLegalizeVectorOps
            || !self.hooks.convert_select_of_constants_to_math(vt)
        {
            return None;
        }
        let (c1, c2) = (self.dag.constant(then)?, self.dag.constant(other)?);
        let mask = super::fold::mask(vt.size_in_bits());
        let (c1, c2) = (c1 & mask, c2 & mask);
        let amount_vt = self.hooks.shift_amount_type(vt);
        let register_wide = self.tli.is_some_and(|tli| tli.types.is_type_legal(vt));
        let dag = &mut *self.dag;
        if c1 == 1 && c2 == 0 {
            return Some(dag.get_zext_or_trunc(cond, vt));
        }
        if c1 == mask && c2 == 0 {
            return Some(dag.get_sext_or_trunc(cond, vt));
        }
        if c1 == 0 && (c2 == 1 || c2 == mask) {
            let not = dag.get_not(cond, EVT::I1);
            return Some(if c2 == 1 { dag.get_zext_or_trunc(not, vt) } else { dag.get_sext_or_trunc(not, vt) });
        }
        if c1.wrapping_sub(1) & mask == c2 {
            // DEVIATE SC-NEG1 (owner m68k-lsr-addressing, review 2026-12-20;
            // ends if the pin's M68k lowers a select of constants
            // itself or `convertSelectOfConstantsToMath` is
            // answered per target): where a true boolean is all
            // ones, `zext c` costs a mask (`Scc; and #1`),
            // and `sext (not c)` is the inverted `Scc` with no mask:
            // `c ? C : C-1` is `C + sext (!c)`. The pin's own forms: the sext
            // arm ([L13102][sext]) and `or (sext (not c)), C` ([L13122][not]).
            //
            // [sext]:
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13102
            // [not]:
            // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L13122
            // Only in a register-wide type: an expanded one pays the sext's
            // high half (`add` with carry of `zext` is cheaper there).
            if dag.config().boolean_contents == super::dag::BooleanContents::ZeroOrNegativeOne && register_wide {
                let not = dag.get_not(cond, EVT::I1);
                let not = dag.get_sext_or_trunc(not, vt);
                return Some(dag.get_node(ISD::Add, vt, &[not, then]));
            }
            let cond = dag.get_zext_or_trunc(cond, vt);
            return Some(dag.get_node(ISD::Add, vt, &[cond, other]));
        }
        if c1.wrapping_add(1) & mask == c2 {
            let cond = dag.get_sext_or_trunc(cond, vt);
            return Some(dag.get_node(ISD::Add, vt, &[cond, other]));
        }
        if c1.is_power_of_two() && c2 == 0 {
            let cond = dag.get_zext_or_trunc(cond, vt);
            let amount = dag.get_constant(u128::from(c1.trailing_zeros()), amount_vt);
            return Some(dag.get_node(ISD::Shl, vt, &[cond, amount]));
        }
        if c1 == mask {
            let cond = dag.get_sext_or_trunc(cond, vt);
            return Some(dag.get_node(ISD::Or, vt, &[cond, other]));
        }
        if c2 == mask {
            let not = dag.get_not(cond, EVT::I1);
            let not = dag.get_sext_or_trunc(not, vt);
            return Some(dag.get_node(ISD::Or, vt, &[not, then]));
        }
        None
    }

    /// `SimplifySelectCC`, the sign folds: `select (x > -1), C, ~C` is
    /// `(x >>s 31) ^ C`, `select (x < 0), C, ~C` is `(x >>s 31) ^ ~C`, and the
    /// sign `select (x > -1), 1, -1` is `(x >>s 31) | 1`: a mask from the sign
    /// bit instead of a branch (an `i486` has no `cmov`).
    fn select_sign_folds(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(cond) != ISD::SetCC {
            return None;
        }
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(cond, 2)).data else { return None };
        let (x, bound) = (self.dag.operand(cond, 0), self.dag.operand(cond, 1));
        let x_vt = self.dag.value_type(x);
        let vt = self.dag.value_type(then);
        if !x_vt.is_integer() || !vt.is_integer() {
            return None;
        }
        let (bound, c2, c3) = (self.dag.constant(bound)?, self.dag.constant(then)?, self.dag.constant(other)?);
        let all = super::fold::mask(x_vt.size_in_bits());
        let arm_mask = super::fold::mask(vt.size_in_bits());
        let sign = match cc {
            CondCode::SetGt if bound == all => false,
            CondCode::SetLt if bound == 0 => true,
            _ => return None,
        };
        let shift = self.shift_amount(x_vt, x_vt.size_in_bits() - 1);
        let sra = self.dag.get_node(ISD::Sra, x_vt, &[x, shift]);
        let mask = self.dag.get_sext_or_trunc(sra, vt);
        if c2 == !c3 & arm_mask {
            let keep = self.dag.get_constant(if sign { c3 } else { c2 }, vt);
            return Some(self.dag.get_node(ISD::Xor, vt, &[mask, keep]));
        }
        if !sign && c2 == 1 && c3 == arm_mask {
            let one = self.dag.get_constant(1, vt);
            return Some(self.dag.get_node(ISD::Or, vt, &[mask, one]));
        }
        None
    }

    /// `SimplifySelectCC`, the single-bit test: `select ((x & 2^k) == 0), 0, a`
    /// is `a & ((x << (31-k)) >>s 31)`, a mask of the bit instead of a branch
    /// or a `cmov` (an `i486` has none; the pin's x86 does not avoid it).
    fn select_single_bit_test(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(cond) != ISD::SetCC || self.dag.constant(then) != Some(0) {
            return None;
        }
        let NodeData::CondCode(CondCode::SetEq) = self.dag.node(self.dag.operand(cond, 2)).data else { return None };
        let (and, zero) = (self.dag.operand(cond, 0), self.dag.operand(cond, 1));
        let vt = self.dag.value_type(then);
        if self.dag.opcode(and) != ISD::And || self.dag.value_type(and) != vt || self.dag.constant(zero) != Some(0) {
            return None;
        }
        let (x, mask) = (self.dag.operand(and, 0), self.dag.constant(self.dag.operand(and, 1))?);
        if !mask.is_power_of_two() {
            return None;
        }
        let width = vt.size_in_bits();
        let up = self.shift_amount(vt, width - 1 - mask.trailing_zeros());
        let shl = self.dag.get_node(ISD::Shl, vt, &[x, up]);
        let down = self.shift_amount(vt, width - 1);
        let sra = self.dag.get_node(ISD::Sra, vt, &[shl, down]);
        Some(self.dag.get_node(ISD::And, vt, &[sra, other]))
    }

    /// `foldBoolSelectToLogic`: a select of `i1`s is an `or`/`and` of the
    /// condition, with the other arm frozen.
    fn bool_select_to_logic(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
        vt: EVT,
    ) -> Option<SDValue> {
        let is = |dag: &SelectionDAG, v: SDValue, bit: u128| dag.constant(v) == Some(bit);
        if cond == then || is(self.dag, then, 1) {
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[other]);
            return Some(self.dag.get_node(ISD::Or, vt, &[cond, frozen]));
        }
        if cond == other || is(self.dag, other, 0) {
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[then]);
            return Some(self.dag.get_node(ISD::And, vt, &[cond, frozen]));
        }
        let ones = self.dag.get_all_ones_constant(vt);
        if is(self.dag, other, 1) {
            let not = self.dag.get_node(ISD::Xor, vt, &[cond, ones]);
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[then]);
            return Some(self.dag.get_node(ISD::Or, vt, &[not, frozen]));
        }
        if is(self.dag, then, 0) {
            let not = self.dag.get_node(ISD::Xor, vt, &[cond, ones]);
            let frozen = self.dag.get_node(ISD::Freeze, vt, &[other]);
            return Some(self.dag.get_node(ISD::And, vt, &[not, frozen]));
        }
        None
    }

    /// `visitXOR`'s `not (setcc x, y, cc)` fold ([L10560][pin]): `xor (setcc x,
    /// y, cc), true` is `setcc x, y, !cc`, so a branch on the inverted
    /// condition tests the compare, not a materialized and flipped boolean.
    /// Booleans are zero-or-one here, so `true` is 1 (`-1` of an
    /// `i1`). Only the integer compare is translated; the other `visitXOR`
    /// folds are not (SD208).
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L10560
    fn visit_xor(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        // `not (or|and a, b)` of `i1`s, when one is a one-use compare, is the
        // and/or of the nots (De Morgan), each of which flips its compare:
        // `xor (or (setcc), (setcc)), 1` was `or; xor 1; test 1; je`.
        if self.dag.constant(n1) == Some(1)
            && vt == EVT::I1
            && matches!(self.dag.opcode(n0), ISD::Or | ISD::And)
            && self.dag.has_one_use(n0.node)
        {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let one_use_setcc = |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::SetCC && dag.has_one_use(v.node);
            if one_use_setcc(self.dag, a) || one_use_setcc(self.dag, b) {
                let flipped = if self.dag.opcode(n0) == ISD::And { ISD::Or } else { ISD::And };
                // `SDLoc(N00)`, `SDLoc(N01)`: each not keeps the IR order of
                // the compare it flips, which the source-order scheduler reads.
                let here = self.dag.cur_order;
                self.dag.cur_order = self.dag.node(a).order;
                let not_a = self.dag.get_node(ISD::Xor, vt, &[a, n1]);
                self.dag.cur_order = self.dag.node(b).order;
                let not_b = self.dag.get_node(ISD::Xor, vt, &[b, n1]);
                self.dag.cur_order = here;
                self.add_to_worklist(not_a.node);
                self.add_to_worklist(not_b.node);
                return Some(self.dag.get_node(flipped, vt, &[not_a, not_b]));
            }
        }
        // fold (xor (and x, y), y) -> (and (not x), y)
        if self.hooks.fold_xor_of_and_with_mask()
            && self.dag.opcode(n0) == ISD::And
            && self.dag.has_one_use(n0.node)
            && self.dag.operand(n0, 1) == n1
        {
            let x = self.dag.operand(n0, 0);
            let not_x = self.dag.get_not(x, vt);
            self.add_to_worklist(not_x.node);
            return Some(self.dag.get_node(ISD::And, vt, &[not_x, n1]));
        }
        // fold (xor (shl 1, x), -1) -> (rotl ~1, x): a zero placed in a value
        // of all ones, which the target clears a bit with (`btr`) when it
        // meets an `and` (DAGCombiner::visitXOR).
        let ones = match vt.size_in_bits() {
            128.. => u128::MAX,
            bits => (1u128 << bits) - 1,
        };
        if vt.is_integer()
            && self.dag.opcode(n0) == ISD::Shl
            && self.dag.constant(self.dag.operand(n0, 0)) == Some(1)
            && self.dag.constant(n1) == Some(ones)
            && self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(ISD::Rotl, vt))
        {
            let amount = self.dag.operand(n0, 1);
            let not_one = self.dag.get_constant(ones - 1, vt);
            return Some(self.dag.get_node(ISD::Rotl, vt, &[not_one, amount]));
        }
        if let Some(merged) = self.unfold_masked_merge(node) {
            return Some(merged);
        }
        if self.dag.constant(n1) != Some(1) || self.dag.opcode(n0) != ISD::SetCC {
            return self.reassociate_ops(ISD::Xor, n0, n1, self.dag.node(node).flags);
        }
        let (lhs, rhs) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(n0, 2)).data else { return None };
        // A far pointer is an integer to the pin (`i32`), not a float.
        let is_float = self.dag.value_type(lhs).is_floating_point();
        let not_cc = if is_float { cc.inverse_float() } else { cc.inverse_integer() };
        // `isCondCodeLegal` once operations are legal: x86 expands `oeq` and
        // `une` (two flags), wasm the unordered ones, so the fold must
        // not make one the target expands.
        if self.level >= CombineLevel::AfterLegalizeVectorOps
            && let (Some(tli), EVT::Simple(operand)) = (self.tli, self.dag.value_type(lhs))
            && tli.cond_code_action(not_cc, operand) != Action::Legal
        {
            return None;
        }
        Some(self.dag.get_setcc(vt, lhs, rhs, not_cc))
    }

    /// The users of `old` read `new`, and are looked at again.
    pub(super) fn replace_value(
        &mut self,
        old: SDValue,
        new: SDValue,
    ) {
        for event in self.dag.replace_all_uses_with(old, new) {
            match event {
                Rauw::Updated(user) => self.add_to_worklist(user),
                Rauw::Merged { into, .. } => self.add_to_worklist_with_users(into),
            }
        }
    }

    /// `getAnyExtOrTrunc`.
    fn any_ext_or_trunc(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        let from = self.dag.value_type(value);
        if from == vt {
            value
        } else if from.bits_gt(vt) {
            self.dag.get_node(ISD::Truncate, vt, &[value])
        } else {
            self.dag.get_node(ISD::AnyExtend, vt, &[value])
        }
    }

    /// `visitTRUNCATE`, the cast folds: `trunc (trunc x)` is `trunc x`;
    /// `trunc (ext x)` is `x` of the same type, else the narrower of the two
    /// casts.
    fn visit_truncate(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vt, n0) = (self.dag.value_type(node), self.dag.operand(node, 0));
        if self.dag.value_type(n0) == vt {
            return Some(n0);
        }
        if self.dag.constant(n0).is_some() || self.dag.is_undef(n0) {
            let folded = self.dag.get_node(ISD::Truncate, vt, &[n0]);
            return (folded != node).then_some(folded);
        }
        if self.dag.opcode(n0) == ISD::Sub && vt.is_integer() {
            if let Some(saturated) = self.fold_sub_to_usub_sat(vt, n0) {
                return Some(saturated);
            }
        }
        match self.dag.opcode(n0) {
            // `reduceLoadWidth`: the low bytes of a loaded value are a narrower
            // load of the same address (little-endian), which then folds into
            // its user (`test byte [m], 1` where `mov eax, [m]; test al, 1`).
            ISD::Load if n0.res == 0 => self.reduce_load_width(node),
            ISD::Truncate => Some(self.dag.get_node(ISD::Truncate, vt, &[self.dag.operand(n0, 0)])),
            // Narrow a binary operation with a constant operand ahead of the
            // truncate: `trunc (and y, 31)` is `and (trunc y), 31`, which the
            // shift patterns then drop as an unneeded mask. Before
            // legalization only, as the pin does.
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor
                if self.level < CombineLevel::AfterLegalizeVectorOps
                    && self.dag.has_one_use(n0.node)
                    && vt.is_integer()
                    && {
                        let (l, r) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                        l == r || self.dag.constant(l).is_some() || self.dag.constant(r).is_some()
                    } =>
            {
                let (l, r) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                let narrow_l = self.dag.get_node(ISD::Truncate, vt, &[l]);
                let narrow_r = self.dag.get_node(ISD::Truncate, vt, &[r]);
                Some(self.dag.get_node(self.dag.opcode(n0), vt, &[narrow_l, narrow_r]))
            }
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend => {
                let (op, x) = (self.dag.opcode(n0), self.dag.operand(n0, 0));
                let from = self.dag.value_type(x);
                if from == vt {
                    Some(x)
                } else if from.bits_lt(vt) {
                    Some(self.dag.get_node(op, vt, &[x]))
                } else {
                    Some(self.dag.get_node(ISD::Truncate, vt, &[x]))
                }
            }
            _ => None,
        }
    }

    /// `reduceLoadWidth` for `trunc (load p)` when nothing else reads the loaded
    /// value (its chain may have users).
    fn reduce_load_width(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (vt, load) = (self.dag.value_type(node), self.dag.operand(node, 0));
        let NodeData::Mem(info) = self.dag.node(load).data.clone() else { return None };
        let wide = self.dag.value_type(load);
        // `isLegalNarrowLdSt`: an extending load narrows to a plain load of
        // the truncated width when its memory is that wide or wider
        // (`trunc (zextload i8)` to a byte is the byte load); one whose memory
        // is narrower than the result stays an extending load.
        let stored = if info.ext.is_some() { info.mem_vt } else { wide };
        if info.volatile
            || self.dag.users(load).count() != 1
            || !vt.is_integer()
            || vt.size_in_bits() % 8 != 0
            || vt.size_in_bits() >= wide.size_in_bits()
            || stored.size_in_bits() < vt.size_in_bits()
        {
            return None;
        }
        // The load must be legal at the narrower type once types are legal.
        if self.level >= CombineLevel::AfterLegalizeTypes && self.tli.is_some_and(|tli| !tli.types.is_type_legal(vt)) {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        // `AdjustBigEndianShift`: the low bytes of a big-endian value are its
        // last bytes
        let offset =
            if self.dag.config().big_endian { i64::from((stored.size_in_bits() - vt.size_in_bits()) / 8) } else { 0 };
        let ptr = if offset == 0 { ptr } else { self.dag.get_object_ptr_offset(ptr, offset) };
        let base = match info.base {
            super::dag::MemBase::Stack(at) => super::dag::MemBase::Stack(at + offset as i32),
            super::dag::MemBase::CallArgument(at) => super::dag::MemBase::CallArgument(at + offset as i32),
            other => other,
        };
        let align = if offset == 0 { info.align } else { info.align.min(1 << (offset as u32).trailing_zeros()) };
        let narrow = super::dag::MemInfo { mem_vt: vt, ext: None, align, base, ..info };
        let new = self.dag.get_load(vt, chain, ptr, narrow);
        if self.dag.invariant_loads.contains(&load.node) {
            self.dag.invariant_loads.insert(new.node);
        }
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: new.node, res: 1 });
        Some(new)
    }

    /// `visitANY_EXTEND`, `visitZERO_EXTEND` and `visitSIGN_EXTEND`, the folds
    /// of an extension of another cast: `aext (zext x)` is `zext x`, `zext
    /// (aext x)` is `zext x`, `aext (trunc x)` is `x` made the right width,
    /// `zext (trunc x)` its low bits masked, `sext (trunc x)` sign-extended in
    /// register where that is legal.
    fn fold_extend_of_cast(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (op, vt, n0) = (self.dag.opcode(node), self.dag.value_type(node), self.dag.operand(node, 0));
        // An operand a fold made a constant (or undef) since the node was
        // built: `getNode` folds the cast.
        if self.dag.constant(n0).is_some() || self.dag.is_undef(n0) {
            let folded = self.dag.get_node(op, vt, &[n0]);
            return (folded != node).then_some(folded);
        }
        // `zext (and (trunc x), cst)` and `aext (and (trunc x), cst)` are
        // `and x, cst` when a cast is not free
        // ([L16515](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L16515),
        // [L16798](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L16798)):
        // the constant is zero-extended, not sign-extended, so a mask a
        // later fold would widen stays the width it was.
        if matches!(op, ISD::AnyExtend | ISD::ZeroExtend)
            && self.dag.opcode(n0) == ISD::And
            && let Some(constant) = self.dag.constant(self.dag.operand(n0, 1))
            && self.dag.opcode(self.dag.operand(n0, 0)) == ISD::Truncate
        {
            let x = self.dag.operand(self.dag.operand(n0, 0), 0);
            let narrow = self.dag.value_type(n0);
            // Where `free_narrowing` holds (the pin's `isTruncateFree(wide, narrow) &&
            // isZExtFree(narrow, wide)`) `ShrinkDemandedOp` makes the narrow form
            // this fold would undo, and the two would run in turn forever.
            let narrows_free = self.hooks.free_narrowing(vt, narrow);
            let truncate_free = self.hooks.is_truncate_free(self.dag.value_type(x), narrow);
            let zext_free = self.hooks.is_zext_free(narrow, vt);
            let sext_cheaper = self.tli.is_some_and(|tli| tli.is_sext_cheaper_than_zext(narrow, vt));
            let fires = !narrows_free
                && (!truncate_free || if op == ISD::ZeroExtend { !zext_free } else { !zext_free && !sext_cheaper });
            if fires {
                let wide = self.any_ext_or_trunc(x, vt);
                let mask = self.dag.get_constant(constant & super::fold::mask(narrow.size_in_bits()), vt);
                return Some(self.dag.get_node(ISD::And, vt, &[wide, mask]));
            }
        }
        let inner = self.dag.opcode(n0);
        if !matches!(
            inner,
            ISD::AnyExtend | ISD::ZeroExtend | ISD::SignExtend | ISD::Truncate
        ) {
            return None;
        }
        let x = self.dag.operand(n0, 0);
        match (op, inner) {
            (ISD::AnyExtend, ISD::AnyExtend | ISD::ZeroExtend | ISD::SignExtend) => {
                Some(self.dag.get_node(inner, vt, &[x]))
            }
            (ISD::ZeroExtend, ISD::ZeroExtend | ISD::AnyExtend) => Some(self.dag.get_node(ISD::ZeroExtend, vt, &[x])),
            (ISD::SignExtend, ISD::SignExtend | ISD::AnyExtend) => Some(self.dag.get_node(ISD::SignExtend, vt, &[x])),
            (ISD::AnyExtend, ISD::Truncate) => Some(self.any_ext_or_trunc(x, vt)),
            (ISD::ZeroExtend, ISD::Truncate) => {
                // `zext (trunc x)` is `x` resized when the bits the truncate
                // drops are zero, which `nuw` promises
                // ([L2474](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2474)).
                let (op_bits, mid_bits, dest_bits) =
                    (self.dag.value_type(x).size_in_bits(), self.dag.value_type(n0).size_in_bits(), vt.size_in_bits());
                let dropped = if op_bits == mid_bits {
                    0
                } else {
                    super::fold::mask(op_bits.min(dest_bits)) & !super::fold::mask(mid_bits)
                };
                let mut zero = super::known_bits::known_bits(self.dag, x).zero;
                if self.dag.node(n0).flags.has(super::dag::SDNodeFlags::NO_UNSIGNED_WRAP) {
                    zero |= super::fold::mask(op_bits) & !super::fold::mask(mid_bits);
                }
                if dropped & !zero == 0 {
                    return Some(self.dag.get_zext_or_trunc(x, vt));
                }
                // `zext nneg (trunc x)` of an `x` with sign bits to spare is
                // `x` widened by `sext`, or truncated
                // ([L16459](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L16459)).
                let (op_bits, mid_bits) =
                    (self.dag.value_type(x).size_in_bits(), self.dag.value_type(n0).size_in_bits());
                if self.dag.node(node).flags.has(super::dag::SDNodeFlags::NON_NEG)
                    && super::known_bits::num_sign_bits(self.dag, x) > op_bits - mid_bits
                {
                    return Some(self.dag.get_sext_or_trunc(x, vt));
                }
                let wide = self.any_ext_or_trunc(x, vt);
                let narrow = self.dag.value_type(n0);
                let mask = self.dag.get_constant(super::fold::mask(narrow.size_in_bits()), vt);
                Some(self.dag.get_node(ISD::And, vt, &[wide, mask]))
            }
            (ISD::SignExtend, ISD::Truncate)
                if self.tli.is_some_and(|tli| tli.is_operation_legal(ISD::SignExtendInReg, vt)) =>
            {
                let wide = self.any_ext_or_trunc(x, vt);
                let narrow = self.dag.get_value_type_node(self.dag.value_type(n0));
                Some(self.dag.get_node(ISD::SignExtendInReg, vt, &[wide, narrow]))
            }
            _ => None,
        }
    }

    /// The closing arm of `visitZERO_EXTEND` ([L16730][pin]): `(zext nneg x)`
    /// is the `(sext x)` that already exists, if the zext is not free: one
    /// extension serves both, where two left a mask beside a `load16_s`.
    ///
    /// [pin]:
    ///     https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L16730
    fn zext_nneg_of_existing_sext(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(node) != ISD::ZeroExtend
            || !self.dag.node(node).flags.has(super::dag::SDNodeFlags::NON_NEG)
        {
            return None;
        }
        let (n0, vt) = (self.dag.operand(node, 0), self.dag.value_type(node));
        if self.hooks.is_zext_free(self.dag.value_type(n0), vt) {
            return None;
        }
        let existing = self.dag.get_node_if_exists(ISD::SignExtend, &[vt], &[n0])?;
        Some(SDValue { node: existing, res: 0 })
    }

    /// `SelectionDAG::InferPtrAlign` ([L14438][pin]): what the address alone
    /// says of its alignment: a global plus a constant, or a frame object
    /// plus a constant.
    ///
    /// [pin]:
    ///     https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/SelectionDAG.cpp#L14438
    pub(super) fn infer_ptr_align(
        &self,
        ptr: SDValue,
    ) -> Option<u64> {
        let common = |align: u64, offset: u64| if offset == 0 { align } else { align.min(1 << offset.trailing_zeros()) };
        if let Some((symbol, offset)) = self.global_plus_offset(ptr) {
            if let Some(&align) = self.dag.global_alignments.get(&symbol) {
                if align > 1 {
                    return Some(common(align, offset as u64));
                }
            }
        }
        let (index, offset) = match (self.dag.opcode(ptr), self.dag.node(ptr).data) {
            (ISD::FrameIndex, NodeData::FrameIndex(index)) => (index, 0),
            (ISD::Add | ISD::Or, _) => {
                let (base, offset) = (self.dag.operand(ptr, 0), self.dag.operand(ptr, 1));
                let constant = self.dag.constant(offset)?;
                let disjoint = self.dag.opcode(ptr) == ISD::Add
                    || super::known_bits::masked_value_is_zero(self.dag, base, constant);
                match (disjoint, self.dag.opcode(base), self.dag.node(base).data) {
                    (true, ISD::FrameIndex, NodeData::FrameIndex(index)) => (index, constant as u64),
                    _ => return None,
                }
            }
            _ => return None,
        };
        let align = *self.dag.frame_alignments.get(usize::try_from(index).ok()?)?;
        Some(common(align, offset))
    }

    /// `TLI.isGAPlusOffset`: a `GlobalAddress`, or one added to a constant.
    fn global_plus_offset(
        &self,
        ptr: SDValue,
    ) -> Option<(u32, i64)> {
        match (self.dag.opcode(ptr), self.dag.node(ptr).data) {
            (ISD::GlobalAddress | ISD::TargetGlobalAddress, NodeData::Global { symbol, offset }) => Some((symbol, offset)),
            (ISD::Add, _) => {
                let (a, b) = (self.dag.operand(ptr, 0), self.dag.operand(ptr, 1));
                let signed = |c: SDValue| {
                    let bits = self.dag.constant(c)?;
                    let width = self.dag.value_type(c).size_in_bits();
                    Some(((bits << (128 - width)) as i128 >> (128 - width)) as i64)
                };
                match (self.global_plus_offset(a), signed(b), self.global_plus_offset(b), signed(a)) {
                    (Some((symbol, offset)), Some(add), _, _) => Some((symbol, offset.wrapping_add(add))),
                    (None, _, Some((symbol, offset)), Some(add)) => Some((symbol, offset.wrapping_add(add))),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `visitLOAD` and `visitSTORE`'s "try to infer better alignment
    /// information than the access already has" (`OptLevel != None`,
    /// unindexed, not atomic): the node's alignment is refined in place.
    pub(super) fn refine_alignment(
        &mut self,
        node: SDValue,
    ) {
        let NodeData::Mem(info) = self.dag.node(node).data else { return };
        let ptr = self.dag.operand(node, if self.dag.opcode(node) == ISD::Store { 2 } else { 1 });
        if self.opt_none || info.ordering.is_some() {
            return;
        }
        // DEVIATE: `isAligned(*Alignment, getSrcValueOffset())` needs the
        // access's offset in its IR pointer, which `MemInfo` does not hold:
        // an access still described by its IR location has offset 0, any
        // other (an inline memcpy's chunk at +3) is left alone.
        let super::dag::MemBase::Value(location) = info.base else { return };
        if self.dag.mem_locations[location.get() as usize - 1].bytes != info.mem_vt.size_in_bits().div_ceil(8) {
            return;
        }
        if let Some(align) = self.infer_ptr_align(ptr).filter(|&align| align > u64::from(info.align)) {
            self.dag.refine_mem_alignment(node.node, align.min(u64::from(u32::MAX)) as u32);
        }
    }

    /// `visitSIGN_EXTEND` ([L16279][pin]): `(sext x)` is `(zext nneg x)` if the
    /// sign bit is known zero and the target does not extend signed more
    /// cheaply. At m64 that is `mov r32, r32`, not `movslq`.
    ///
    /// [pin]:
    ///     https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L16279
    fn sext_of_non_negative(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(node) != ISD::SignExtend {
            return None;
        }
        let (n0, vt) = (self.dag.operand(node, 0), self.dag.value_type(node));
        let from = self.dag.value_type(n0);
        if self.tli?.is_sext_cheaper_than_zext(from, vt)
            || (self.legal_operations() && !self.fp_op_legal(ISD::ZeroExtend, vt))
            || !super::known_bits::masked_value_is_zero(self.dag, n0, 1u128 << (from.size_in_bits() - 1))
        {
            return None;
        }
        Some(self.dag.get_node_flags(ISD::ZeroExtend, vt, &[n0], super::dag::SDNodeFlags::NON_NEG))
    }

    /// `visitSIGN_EXTEND`, `visitZERO_EXTEND` and `visitANY_EXTEND` for a
    /// loaded operand (`tryToFoldExtOfLoad` and the load case after it): the
    /// extension is the load's own. `(zext (load i8 p))` is `zextload i8 p`;
    /// an already extending load is re-extended to the wider type.
    fn visit_extend(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let load = self.dag.operand(node, 0);
        // The frozen arm is pre-legalization only: it makes an extending load
        // with no `isLoadExtLegal` check.
        if self.dag.opcode(load) == ISD::Freeze {
            return if self.level < CombineLevel::AfterLegalizeVectorOps {
                self.visit_extend_of_frozen_load(node)
            } else {
                None
            };
        }
        if self.dag.opcode(load) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        let op = self.dag.opcode(node);
        let wanted = match op {
            ISD::ZeroExtend => LoadExt::ZExtLoad,
            ISD::SignExtend => LoadExt::SExtLoad,
            _ => LoadExt::ExtLoad,
        };
        let ext = match info.ext {
            None => wanted,
            Some(have) if have == wanted => have,
            Some(LoadExt::ExtLoad) => wanted,
            Some(have) if wanted == LoadExt::ExtLoad => have,
            Some(_) => return None,
        };
        let wide = self.dag.value_type(node);
        let others: Vec<u32> = self.dag.users(load).filter(|&user| user != node.node).collect();
        let mut compares = Vec::new();
        let legal = match (wide, info.mem_vt) {
            (EVT::Simple(wide_mvt), EVT::Simple(mem_mvt)) => {
                self.tli.is_some_and(|tli| tli.load_ext_action(ext, wide_mvt, mem_mvt) == Action::Legal)
            }
            _ => false,
        };
        // `tryToFoldExtOfLoad`: `(!LegalOperations && !isVolatile) ||
        // isLoadExtLegal`: after legalization, and for a volatile load, the
        // extending load must be legal.
        if (info.volatile || self.legal_operations()) && !legal {
            return None;
        }
        if !others.is_empty() {
            // `isLoadExtLegal`, and `ExtendUsesToFormExtLoad`: the other
            // readers are compares against a constant, which can be
            // widened, or take the value back through a (free)
            // truncate.
            if info.ext.is_some() || !legal || !self.extend_uses(node, load, op, &others, &mut compares) {
                return None;
            }
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let info = super::dag::MemInfo { ext: None, ..info };
        let extended = self.dag.get_ext_load(ext, wide, chain, ptr, info);
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        if !others.is_empty() {
            // The narrow value is the extending load, truncated.
            let narrow = self.dag.value_type(load);
            let truncated = self.dag.get_node(ISD::Truncate, narrow, &[extended]);
            for compare in compares {
                self.extend_compare(compare, load, extended, op);
            }
            self.replace_value(load, truncated);
        }
        Some(extended)
    }

    /// `tryToFoldExtOfLoad`'s `Frozen` path
    /// ([DAGCombiner.cpp#L15719][pin]): `(zext (freeze (load x)))` is
    /// `(assertzext (freeze (zextload x)))`, and a sign extension the
    /// `assertsext` of a `sextload`. The extension is the load's own, so no
    /// mask is left to make again once the narrow type is promoted.
    ///
    /// DEVIATE SD548: the freeze and the load are each read once (the pin's
    /// other arm truncates the extended value back for the other readers),
    /// and `any_extend` is left alone. Exit: that arm, wasm, 2026-11-20.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15719
    fn visit_extend_of_frozen_load(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let freeze = self.dag.operand(node, 0);
        let load = self.dag.operand(freeze, 0);
        let kind = match self.dag.opcode(node) {
            ISD::ZeroExtend => (LoadExt::ZExtLoad, ISD::AssertZext),
            ISD::SignExtend => (LoadExt::SExtLoad, ISD::AssertSext),
            _ => return None,
        };
        if self.dag.opcode(load) != ISD::Load || !self.dag.has_one_use(freeze.node) {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        if info.volatile || info.ext.is_some() {
            return None;
        }
        // `Load->hasNUsesOfValue(1, 0)`: the freeze is the only reader of the
        // value.
        let readers: usize = self
            .dag
            .users(load)
            .map(|user| {
                let value = SDValue { node: user, res: 0 };
                (0..self.dag.node(value).ops.len()).filter(|&at| self.dag.operand(value, at) == load).count()
            })
            .sum();
        if readers != 1 {
            return None;
        }
        let (ext, assert) = kind;
        let wide = self.dag.value_type(node);
        let narrow = self.dag.value_type(load);
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let info = super::dag::MemInfo { ext: None, ..info };
        let extended = self.dag.get_ext_load(ext, wide, chain, ptr, info);
        let frozen = self.dag.get_node(ISD::Freeze, wide, &[extended]);
        let narrow_vt = self.dag.get_value_type_node(narrow);
        let asserted = self.dag.get_node(assert, wide, &[frozen, narrow_vt]);
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        Some(asserted)
    }

    /// `ExtendUsesToFormExtLoad`: whether the other users of `load` can live
    /// with its extension; the compares to widen are collected.
    fn extend_uses(
        &self,
        node: SDValue,
        load: SDValue,
        op: ISD,
        others: &[u32],
        compares: &mut Vec<u32>,
    ) -> bool {
        let mut copy_to_reg = false;
        let truncate_free = self.hooks.extend_uses_truncate_free(self.dag.value_type(node), self.dag.value_type(load));
        for &user in others {
            let user_value = SDValue { node: user, res: 0 };
            if op != ISD::AnyExtend && self.dag.opcode(user_value) == ISD::SetCC {
                if !self.hooks.extend_setcc_uses(self.dag.value_type(node)) {
                    continue;
                }
                let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(user_value, 2)).data else { return false };
                if op == ISD::ZeroExtend && cc.is_signed_int() {
                    return false;
                }
                let mut widens = false;
                for at in 0..2 {
                    let operand = self.dag.operand(user_value, at);
                    if operand == load {
                        continue;
                    }
                    if self.dag.constant(operand).is_none() {
                        return false;
                    }
                    widens = true;
                }
                if widens {
                    compares.push(user);
                }
                continue;
            }
            // `ExtendUsesToFormExtLoad`: "if truncates aren't free and there
            // are users we can't extend, it isn't worthwhile."
            if !truncate_free {
                return false;
            }
            if self.dag.opcode(user_value) == ISD::CopyToReg {
                copy_to_reg = true;
            }
        }
        if copy_to_reg {
            // Both the narrow and the wide value are live out: only worth it if
            // a compare is widened.
            let both =
                self.dag.users(node).any(|user| self.dag.opcode(SDValue { node: user, res: 0 }) == ISD::CopyToReg);
            if both {
                return !compares.is_empty();
            }
        }
        true
    }

    /// `ExtendSetCCUses`: the compare of the load against a constant is a
    /// compare of the extending load against the extended constant.
    fn extend_compare(
        &mut self,
        compare: u32,
        load: SDValue,
        extended: SDValue,
        op: ISD,
    ) {
        let compare_value = SDValue { node: compare, res: 0 };
        let wide = self.dag.value_type(extended);
        let mut operands = [self.dag.operand(compare_value, 0), self.dag.operand(compare_value, 1)];
        for operand in &mut operands {
            *operand = if *operand == load { extended } else { self.dag.get_node(op, wide, &[*operand]) };
        }
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(compare_value, 2)).data else { return };
        let vt = self.dag.value_type(compare_value);
        let replacement = self.dag.get_setcc(vt, operands[0], operands[1], cc);
        self.replace_value(compare_value, replacement);
    }

    /// `areBitwiseNotOfEachother`: one is `(xor other, -1)`.
    fn are_bitwise_not_of_each_other(
        &self,
        a: SDValue,
        b: SDValue,
    ) -> bool {
        let is_not = |dag: &SelectionDAG, v: SDValue, of: SDValue| {
            dag.opcode(v) == ISD::Xor
                && dag.operand(v, 0) == of
                && dag.constant(dag.operand(v, 1)) == Some(super::fold::mask(dag.width(v)))
        };
        is_not(self.dag, a, b) || is_not(self.dag, b, a)
    }

    /// `visitAND`, the part `SimplifyDemandedBits` does for it: `(and x, c)` is
    /// `x` when every bit is either set in `c` or known zero in `x`, so
    /// `and (setcc), 1` is the `setcc` (it is 0 or 1 already).
    fn visit_and(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(n0);
        if n0 == n1 {
            return Some(n0);
        }
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node(ISD::And, vt, &[n1, n0]));
        }
        if self.are_bitwise_not_of_each_other(n0, n1) {
            return Some(self.dag.get_constant(0, vt));
        }
        // `(and x, c)` is zero when `x` is known zero wherever `c` is set.
        if let Some(c) = self.dag.constant(n1) {
            if c & !super::known_bits::known_bits(self.dag, n0).zero & super::fold::mask(vt.size_in_bits()) == 0 {
                return Some(self.dag.get_constant(0, vt));
            }
        }
        if let Some(folded) = self.fold_logic_of_setccs(true, n0, n1) {
            return Some(folded);
        }
        if let Some(reassociated) = self.reassociate_ops(ISD::And, n0, n1, self.dag.node(node).flags) {
            return Some(reassociated);
        }
        if let Some(folded) = self.fold_bitwise_op_with_neg(node) {
            return Some(folded);
        }
        if let (Some(c1), true) = (self.dag.constant(n1), !self.dag.is_opaque(n1)) {
            // `(and (or x, C), D)` is `D` when `D` is a subset of `C`.
            if self.dag.opcode(n0) == ISD::Or {
                if let Some(c) = self.dag.constant(self.dag.operand(n0, 1)) {
                    if c1 & !c == 0 {
                        return Some(n1);
                    }
                }
            }
        }
        let all = super::fold::mask(vt.size_in_bits());
        for (other, constant) in [(n0, n1), (n1, n0)] {
            if let Some(c) = self.dag.constant(constant) {
                if self.dag.is_opaque(constant) {
                    continue;
                }
                // `(and (anyext x), c)` is `(zext x)` when `c` keeps no bit
                // above `x` and every bit `x` may set (`SimplifyDemandedBits`
                // drops the `and` of a promoted boolean).
                if self.dag.opcode(other) == ISD::AnyExtend {
                    let source = self.dag.operand(other, 0);
                    let width = self.dag.value_type(source).size_in_bits();
                    let narrow = super::fold::mask(width);
                    let known = super::known_bits::known_bits(self.dag, source);
                    if c & !narrow == 0 && narrow & !(known.zero | c) == 0 {
                        return Some(self.dag.get_node(ISD::ZeroExtend, vt, &[source]));
                    }
                }
                let known = super::known_bits::known_bits(self.dag, other);
                if all & !(known.zero | c) == 0 {
                    return Some(other);
                }
            }
        }
        None
    }

    /// `visitSRA`, by way of `SimplifyDemandedBits`: `(sra (shl x, c), c)` is
    /// `sign_extend_inreg x` from the low `width - c` bits, before operations
    /// are legal or when the target has that extension.
    fn visit_sra(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if self.hooks.fold_sra_of_trunc_of_shift() && self.dag.opcode(n0) == ISD::Truncate {
            if let Some(folded) = self.sra_of_trunc_of_shift(node) {
                return Some(folded);
            }
        }
        if self.dag.opcode(n0) != ISD::Shl {
            return None;
        }
        let (c, up) = (self.dag.constant(n1)?, self.dag.constant(self.dag.operand(n0, 1))?);
        if c != up || c == 0 || c >= u128::from(vt.size_in_bits()) {
            return None;
        }
        let low = EVT::integer(vt.size_in_bits() - c as u32);
        let legal = self.level == CombineLevel::BeforeLegalizeTypes
            || self.tli.is_some_and(|tli| tli.operation_action(ISD::SignExtendInReg, low) == Action::Legal);
        if !legal {
            return None;
        }
        let ty = self.dag.get_value_type_node(low);
        let x = self.dag.operand(n0, 0);
        Some(self.dag.get_node(ISD::SignExtendInReg, vt, &[x, ty]))
    }

    /// `(sra (trunc (srl x, c1)), c2)` and the `sra` form are `(trunc (sra x,
    /// c1 + c2))` when `c1` is the number of bits the truncate removes
    /// ([pin][pin]).
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11664
    fn sra_of_trunc_of_shift(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let inner = self.dag.operand(n0, 0);
        if !matches!(self.dag.opcode(inner), ISD::Srl | ISD::Sra) || !self.dag.has_one_use(inner.node) {
            return None;
        }
        let amount = self.dag.operand(inner, 1);
        if !self.dag.has_one_use(amount.node) {
            return None;
        }
        let (large_shift, c2) = (self.dag.constant(amount)?, self.dag.constant(n1)?);
        let large = self.dag.value_type(inner);
        let removed = large.size_in_bits() - vt.size_in_bits();
        if large_shift != u128::from(removed) {
            return None;
        }
        let total = self.shift_amount(large, removed + c2 as u32);
        let sra = self.dag.get_node(ISD::Sra, large, &[self.dag.operand(inner, 0), total]);
        Some(self.dag.get_node(ISD::Truncate, vt, &[sra]))
    }

    /// `visitADD` through `visitADDLike`, the folds that need no target
    /// query: a constant moves right, `x + ~x` is -1, constants merge through
    /// a `sub` or an `add`, and an `add` of a negation or of a `sub` of the
    /// other operand is a `sub` or the operand itself.
    /// The two forms of `visitADDLike`'s `add(mul(add(A, CA), CM), CB)`:
    /// `n0` is the mul, or a one-use add of it and a `B`; `cb` is `n1`.
    fn fold_add_of_mul_of_add(
        &mut self,
        n0: SDValue,
        n1: SDValue,
        vt: EVT,
    ) -> Option<SDValue> {
        let ones = super::fold::mask(vt.size_in_bits());
        let cb = self.dag.constant(n1)?;
        let bits = vt.size_in_bits();
        // `TLI.isLegalAddImmediate((CA * CM + CB).getSExtValue())`
        let legal = |this: &Self, imm: u128| {
            let shift = 128 - bits;
            this.hooks.legal_add_immediate((((imm << shift) as i128) >> shift) as i64)
        };
        // `m_OneUse(m_Mul(m_Add(m_Value(A), m_ConstInt(CA)), m_ConstInt(CM)))`.
        let product = |this: &Self, v: SDValue| -> Option<(SDValue, u128, u128)> {
            if this.dag.opcode(v) != ISD::Mul || !this.dag.has_one_use(v.node) {
                return None;
            }
            let (inner, cm) = (this.dag.operand(v, 0), this.dag.constant(this.dag.operand(v, 1))?);
            if this.dag.opcode(inner) != ISD::Add {
                return None;
            }
            let ca = this.dag.constant(this.dag.operand(inner, 1))?;
            Some((this.dag.operand(inner, 0), ca, cm))
        };
        if let Some((a, ca, cm)) = product(self, n0) {
            let imm = ca.wrapping_mul(cm).wrapping_add(cb) & ones;
            if !legal(self, imm) {
                return None;
            }
            let (cm, imm) = (self.dag.get_constant(cm, vt), self.dag.get_constant(imm, vt));
            let mul = self.dag.get_node(ISD::Mul, vt, &[a, cm]);
            return Some(self.dag.get_node(ISD::Add, vt, &[mul, imm]));
        }
        if self.dag.opcode(n0) == ISD::Add && self.dag.has_one_use(n0.node) {
            let (x, y) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            for (mul, b) in [(x, y), (y, x)] {
                if let Some((a, ca, cm)) = product(self, mul) {
                    let imm = ca.wrapping_mul(cm).wrapping_add(cb) & ones;
                    if !legal(self, imm) {
                        continue;
                    }
                    let (cm, imm) = (self.dag.get_constant(cm, vt), self.dag.get_constant(imm, vt));
                    let product = self.dag.get_node(ISD::Mul, vt, &[a, cm]);
                    let sum = self.dag.get_node(ISD::Add, vt, &[product, b]);
                    return Some(self.dag.get_node(ISD::Add, vt, &[sum, imm]));
                }
            }
        }
        None
    }

    fn visit_add(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let flags = self.dag.node(node).flags;
        self.visit_add_like(node, flags)
    }

    /// `visitOR`'s last step: "If OR can be rewritten into ADD, try combines
    /// based on ADD" (`isADDLike`: a `disjoint` or whose operands share no
    /// bits), after `SimplifyDemandedBits` as in the pin. `(or disjoint (add
    /// x, c0), c1)` of a truncating store is `(add x, c0 + c1)`.
    fn visit_or_as_add(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if !vt.is_integer() || (self.legal_operations() && !self.fp_op_legal(ISD::Add, vt)) {
            return None;
        }
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        if !(self.dag.node(node).flags.has(SDNodeFlags::DISJOINT)
            || super::dagisel_adapter::no_common_bits(self.dag, n0, n1))
        {
            return None;
        }
        // The add made of the or keeps no wrap flags (`getFlags` of an or has
        // none).
        let combined = self.visit_add_like(node, SDNodeFlags::default())?;
        (combined != node).then_some(combined)
    }

    /// `visitADDLike`: the folds of `n0 + n1` as the add `node` spells them (or
    /// as an add-like `or`); `flags` are the wrap flags the regrouped adds
    /// keep.
    fn visit_add_like(
        &mut self,
        node: SDValue,
        flags: SDNodeFlags,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node(ISD::Add, vt, &[n1, n0]));
        }
        let ones = super::fold::mask(vt.size_in_bits());
        let is_not = |dag: &SelectionDAG, v: SDValue, of: SDValue| {
            dag.opcode(v) == ISD::Xor && dag.operand(v, 0) == of && dag.constant(dag.operand(v, 1)) == Some(ones)
        };
        if is_not(self.dag, n0, n1) || is_not(self.dag, n1, n0) {
            return Some(self.dag.get_constant(ones, vt));
        }
        let plain = |dag: &SelectionDAG, v: SDValue| dag.constant(v).filter(|_| !dag.is_opaque(v)).is_some();
        if self.hooks.sub_of_sign_extended_bool()
            && self.dag.config().boolean_contents == super::dag::BooleanContents::ZeroOrNegativeOne
        {
            for (zext, other) in [(n1, n0), (n0, n1)] {
                if self.dag.opcode(zext) == ISD::ZeroExtend
                    && self.dag.value_type(self.dag.operand(zext, 0)) == EVT::I1
                    && !plain(self.dag, other)
                {
                    let sext = self.dag.get_node(ISD::SignExtend, vt, &[self.dag.operand(zext, 0)]);
                    return Some(self.dag.get_node(ISD::Sub, vt, &[other, sext]));
                }
            }
        }
        if plain(self.dag, n1) {
            if self.dag.opcode(n0) == ISD::Sub {
                let (n00, n01) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
                // `(add (sub x, c0), c1)` is `(add x, c1 - c0)`.
                if plain(self.dag, n01) {
                    let diff = self.dag.get_node(ISD::Sub, vt, &[n1, n01]);
                    return Some(self.dag.get_node(ISD::Add, vt, &[n00, diff]));
                }
                // `(add (sub c0, x), c1)` is `(sub c0 + c1, x)`.
                if plain(self.dag, n00) {
                    let sum = self.dag.get_node(ISD::Add, vt, &[n1, n00]);
                    return Some(self.dag.get_node(ISD::Sub, vt, &[sum, n01]));
                }
            }
            // `visitADDLike`: `add(mul(add(A, CA), CM), CB)` is `add(mul(A,
            // CM), CM*CA + CB)`, also through an intermediate add; "this can
            // help if the inner add has multiple uses"
            // ([pin](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3179)).
            // DEVIATE: the wrap flags of the result are not kept. Exit
            // 2026-11-12.
            if vt.size_in_bits() <= 64 {
                if let Some(found) = self.fold_add_of_mul_of_add(n0, n1, vt) {
                    return Some(found);
                }
            }
            // `(add (add x, c0), c1)` is `(add x, c0 + c1)` (`reassociateOps`).
            if self.dag.opcode(n0) == ISD::Add
                && plain(self.dag, self.dag.operand(n0, 1))
                && !self.reassociation_can_break_addressing_mode_pattern(node, n0, n1)
            {
                let sum = self.dag.get_node(ISD::Add, vt, &[n1, self.dag.operand(n0, 1)]);
                // Both adds `nuw` keep the regrouped one so (`reassociateOps`'s
                // `NewFlags`); DEVIATE: only where `global_addresses_constant`
                // is set, until x86 is judged.
                let nuw = |dag: &SelectionDAG, v: SDValue| dag.node(v).flags.has(SDNodeFlags::NO_UNSIGNED_WRAP);
                let keep = self.dag.config().global_addresses_constant
                    && nuw(self.dag, n0)
                    && flags.has(SDNodeFlags::NO_UNSIGNED_WRAP);
                let flags = if keep { SDNodeFlags::NO_UNSIGNED_WRAP } else { SDNodeFlags::default() };
                return Some(self.dag.get_node_flags(ISD::Add, vt, &[self.dag.operand(n0, 0), sum], flags));
            }
        }
        // `visitADDLike`: `add (sext i1 X), 1` is `zext (not i1 X)`; the pin
        // does not make `add (zext i1 X), -1` a `sext` ("most targets
        // generate better code for the zext form").
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L2991
        if self.dag.opcode(n0) == ISD::SignExtend
            && self.dag.has_one_use(n0.node)
            && self.dag.constant(n1) == Some(1)
            && self.dag.value_type(self.dag.operand(n0, 0)) == EVT::I1
            && (!self.legal_operations()
                || (self.fp_op_legal(ISD::Xor, EVT::I1) && self.fp_op_legal(ISD::ZeroExtend, vt)))
        {
            let not = self.dag.get_not(self.dag.operand(n0, 0), EVT::I1);
            return Some(self.dag.get_node(ISD::ZeroExtend, vt, &[not]));
        }
        // `visitADDLikeCommutative`: where booleans are 0/1, `add (sext i1
        // Y), X` is `sub X, (zext i1 Y)`.
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3534
        if self.dag.config().boolean_contents == super::dag::BooleanContents::ZeroOrOne
            && vt.size_in_bits() > self.dag.config().sext_bool_add_max_bits
        {
            for (sext, other) in [(n0, n1), (n1, n0)] {
                if self.dag.opcode(sext) == ISD::SignExtend && self.dag.value_type(self.dag.operand(sext, 0)) == EVT::I1
                {
                    let zext = self.dag.get_node(ISD::ZeroExtend, vt, &[self.dag.operand(sext, 0)]);
                    return Some(self.dag.get_node(ISD::Sub, vt, &[other, zext]));
                }
            }
        }
        let is_neg =
            |dag: &SelectionDAG, v: SDValue| dag.opcode(v) == ISD::Sub && dag.constant(dag.operand(v, 0)) == Some(0);
        // `(add (sub 0, a), b)` is `(sub b, a)`; likewise on the right.
        if is_neg(self.dag, n0) {
            return Some(self.dag.get_node(ISD::Sub, vt, &[n1, self.dag.operand(n0, 1)]));
        }
        if is_neg(self.dag, n1) {
            return Some(self.dag.get_node(ISD::Sub, vt, &[n0, self.dag.operand(n1, 1)]));
        }
        // `x + (b - x)` and `(b - y) + y` are `b`.
        if self.dag.opcode(n1) == ISD::Sub && self.dag.operand(n1, 1) == n0 {
            return Some(self.dag.operand(n1, 0));
        }
        if self.dag.opcode(n0) == ISD::Sub && self.dag.operand(n0, 1) == n1 {
            return Some(self.dag.operand(n0, 0));
        }
        if self.dag.opcode(n0) == ISD::Sub && self.dag.opcode(n1) == ISD::Sub {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            let (c, d) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1));
            // `(a - b) + (c - a)` is `c - b`; `(a - b) + (b - d)` is `a - d`.
            if d == a {
                return Some(self.dag.get_node(ISD::Sub, vt, &[c, b]));
            }
            if c == b {
                return Some(self.dag.get_node(ISD::Sub, vt, &[a, d]));
            }
        }
        // `visitADDLikeCommutative`: a one-use `(c - x)` of a plain constant
        // added to `y` is `(y - x) + c`, which the compare folds then see.
        for (sub, other) in [(n0, n1), (n1, n0)] {
            if self.dag.opcode(sub) == ISD::Sub
                && self.dag.has_one_use(sub.node)
                && plain(self.dag, self.dag.operand(sub, 0))
            {
                let diff = self.dag.get_node(ISD::Sub, vt, &[other, self.dag.operand(sub, 1)]);
                return Some(self.dag.get_node(ISD::Add, vt, &[diff, self.dag.operand(sub, 0)]));
            }
        }
        if !self.reassociation_can_break_addressing_mode_pattern(node, n0, n1)
            && let Some(regrouped) = self.reassociate_ops(ISD::Add, n0, n1, flags)
        {
            return Some(regrouped);
        }
        // The tail of `visitADDLikeCommutative`
        // ([L3550](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3550)):
        // `(add X, (uaddo_carry Y, 0, c))` is `(uaddo_carry X, Y, c)`, and
        // `(add X, carry)` is `(uaddo_carry X, 0, carry)`.
        for (x, y) in [(n0, n1), (n1, n0)] {
            if self.dag.opcode(y) == ISD::UAddOCarry && self.dag.is_null_constant(self.dag.operand(y, 1)) && y.res == 0
            {
                let values = self.dag.node(y).values.clone();
                return Some(self.dag.get_node_multi(
                    ISD::UAddOCarry,
                    &values,
                    &[x, self.dag.operand(y, 0), self.dag.operand(y, 2)],
                ));
            }
            if self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(ISD::UAddOCarry, vt))
                && let Some(carry) = self.as_carry(y)
            {
                let zero = self.dag.get_constant(0, vt);
                let carry_vt = self.dag.value_type(carry);
                return Some(self.dag.get_node_multi(ISD::UAddOCarry, &[vt, carry_vt], &[x, zero, carry]));
            }
        }
        // `(add a, b)` is `(or disjoint a, b)` where the two share no bits
        // ([L3318][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3318
        if self.dag.config().add_to_or_disjoint && super::dagisel_adapter::no_common_bits(self.dag, n0, n1) {
            return Some(self.dag.get_node_flags(ISD::Or, vt, &[n0, n1], SDNodeFlags::DISJOINT));
        }
        None
    }

    /// `getAsCarry`
    /// ([L3404](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3404)):
    /// the carry result of an add or subtract behind the truncates, zero
    /// extends and `& 1` that legalization put on it.
    fn as_carry(
        &self,
        mut value: SDValue,
    ) -> Option<SDValue> {
        let mut masked = false;
        loop {
            match self.dag.opcode(value) {
                ISD::Truncate | ISD::ZeroExtend => value = self.dag.operand(value, 0),
                ISD::And if self.dag.constant(self.dag.operand(value, 1)) == Some(1) => {
                    masked = true;
                    value = self.dag.operand(value, 0);
                }
                _ => break,
            }
        }
        if value.res != 1 {
            return None;
        }
        let op = self.dag.opcode(value);
        // DEVIATE (owner x86-m32, exit 2026-11-14): the pin also takes the
        // borrow of USUBO and USUBO_CARRY. A target whose borrow is the
        // inverted carry (the 6502's `sbc`) read it as the carry:
        // `sub_overflow` returned 3 for 4 (#7850). Add only until the
        // target states what its borrow is.
        if !matches!(op, ISD::UAddOCarry | ISD::UAddO) {
            return None;
        }
        let vt = self.dag.node(value).values[0];
        if !self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(op, vt)) {
            return None;
        }
        (masked || self.dag.config().boolean_contents == super::dag::BooleanContents::ZeroOrOne).then_some(value)
    }

    /// `visitSHL`, `visitSRL` and `visitSRA`, the constant-amount folds: the
    /// result is 0 when every bit is known zero; a shift of a shift of the
    /// same kind is one shift by the sum (0 once the sum reaches the width,
    /// for `sra` the sign fill).
    fn visit_shift(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let width = u128::from(vt.size_in_bits());
        // `getNode`'s `simplifyShift`, for an operand a fold made undef since
        // the shift was built: `undef << x` is 0.
        if self.dag.is_undef(n0) {
            return Some(self.dag.get_constant(0, vt));
        }
        if op != ISD::Sra {
            let all = super::fold::mask(vt.size_in_bits());
            let known = super::known_bits::known_bits(self.dag, node);
            if all & !known.zero == 0 {
                return Some(self.dag.get_constant(0, vt));
            }
        }
        if op == ISD::Srl && self.dag.opcode(n0) == ISD::Truncate {
            if let Some(folded) = self.srl_of_trunc_of_srl(node) {
                return Some(folded);
            }
        }
        if op == ISD::Shl && matches!(self.dag.opcode(n0), ISD::Srl | ISD::Sra) {
            if let Some(folded) = self.shl_of_exact_shift(node) {
                return Some(folded);
            }
        }
        if op == ISD::Shl && self.dag.opcode(n0) == ISD::Srl {
            if let Some(folded) = self.shl_of_srl(node) {
                return Some(folded);
            }
        }
        // `shl (sra x, c), c` is `and x, (-1 << c)` ([L11245][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11245
        if op == ISD::Shl && self.dag.opcode(n0) == ISD::Sra && self.dag.operand(n0, 1) == n1 {
            if let Some(c) = self.dag.constant(n1).filter(|&c| c < width) {
                let mask = self
                    .dag
                    .get_constant(
                        (super::fold::mask(vt.size_in_bits()) << c) & super::fold::mask(vt.size_in_bits()),
                        vt,
                    );
                return Some(self.dag.get_node(ISD::And, vt, &[self.dag.operand(n0, 0), mask]));
            }
        }
        // `shl (add x, c1), c2` is `add (shl x, c2), c1 << c2`, and the same
        // for `or` ([L11252][pin]): `x << c2` is shared by the address uses
        // and the constant folds into an offset. DEVIATE: scalar constants
        // only (the pin also folds splat vectors). Exit: none planned.
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11252
        if op == ISD::Shl
            && matches!(self.dag.opcode(n0), ISD::Add | ISD::Or)
            && self.hooks.is_desirable_to_commute_with_shift(self.dag, node, self.level)
        {
            let n01 = self.dag.operand(n0, 1);
            if let Some(shl1) = self.dag.fold_constant_arithmetic(ISD::Shl, vt, &[n01, n1]) {
                let shl0 = self.dag.get_node(ISD::Shl, vt, &[self.dag.operand(n0, 0), n1]);
                self.add_to_worklist(shl0.node);
                let disjoint =
                    self.dag.opcode(n0) == ISD::Or && self.dag.node(n0).flags.0 & SDNodeFlags::DISJOINT.0 != 0;
                let flags = if disjoint { SDNodeFlags::DISJOINT } else { SDNodeFlags::default() };
                return Some(self.dag.get_node_flags(self.dag.opcode(n0), vt, &[shl0, shl1], flags));
            }
        }
        // The rest of `visitShiftByConstant` ([L10870][pin]): a one-use logic
        // op under a shift by a constant, when its operand is a shift
        // by a constant or a register copy or select read elsewhere.
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L10870
        if self.dag.constant(n1).is_some()
            && self.dag.has_one_use(n0.node)
            && self.hooks.is_desirable_to_commute_with_shift(self.dag, node, self.level)
            && match self.dag.opcode(n0) {
                ISD::Or | ISD::Xor | ISD::And => true,
                ISD::Add => op == ISD::Shl,
                _ => false,
            }
        {
            let b = self.dag.operand(n0, 0);
            let bo = self.dag.opcode(b);
            let shift_by_const =
                matches!(bo, ISD::Shl | ISD::Sra | ISD::Srl) && self.dag.constant(self.dag.operand(b, 1)).is_some();
            let copy_or_select = matches!(bo, ISD::CopyFromReg | ISD::Select);
            let n01 = self.dag.operand(n0, 1);
            if shift_by_const || (copy_or_select && !self.dag.has_one_use(node.node)) {
                if let Some(shl1) = self.dag.fold_constant_arithmetic(op, vt, &[n01, n1]) {
                    let shl0 = self.dag.get_node(op, vt, &[b, n1]);
                    self.add_to_worklist(shl0.node);
                    let disjoint =
                        self.dag.opcode(n0) == ISD::Or && self.dag.node(n0).flags.0 & SDNodeFlags::DISJOINT.0 != 0;
                    let flags = if disjoint { SDNodeFlags::DISJOINT } else { SDNodeFlags::default() };
                    return Some(self.dag.get_node_flags(self.dag.opcode(n0), vt, &[shl0, shl1], flags));
                }
            }
        }
        // `srl (sra x, y), w-1` is `srl x, w-1`: the sign bit is the one `sra`
        // leaves alone ([L11886][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11886
        if op == ISD::Srl && self.dag.opcode(n0) == ISD::Sra && self.dag.constant(n1) == Some(width - 1) {
            return Some(self.dag.get_node(ISD::Srl, vt, &[self.dag.operand(n0, 0), n1]));
        }
        if let Some(folded) = self.shift_of_shifted_logic(node) {
            return Some(folded);
        }
        if matches!(op, ISD::Srl | ISD::Sra) {
            if let Some(avg) = self.fold_shift_to_avg(node) {
                return Some(avg);
            }
        }
        if self.dag.opcode(n0) != op {
            return None;
        }
        let (c2, inner) = (self.dag.constant(n1)?, self.dag.constant(self.dag.operand(n0, 1))?);
        if !self.dag.has_one_use(n0.node) && !self.hooks.fold_shift_of_shared_shift(vt, inner as u32) {
            return None;
        }
        let (x, sum) = (self.dag.operand(n0, 0), c2 + inner);
        if sum < width {
            let amount = self.dag.get_constant(sum, self.dag.value_type(n1));
            return Some(self.dag.get_node(op, vt, &[x, amount]));
        }
        if op == ISD::Sra {
            let amount = self.dag.get_constant(width - 1, self.dag.value_type(n1));
            return Some(self.dag.get_node(op, vt, &[x, amount]));
        }
        Some(self.dag.get_constant(0, vt))
    }

    /// `combineShiftOfShiftedLogic` (reached from `visitShiftByConstant`):
    /// `shift (logic (shift X, c0), Y), c1` is `logic (shift X, c0 + c1),
    /// (shift Y, c1)`, for a one-use logic op and a one-use inner shift of the
    /// same kind.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L10794>
    fn shift_of_shifted_logic(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (op, logic, c1) = (self.dag.opcode(node), self.dag.operand(node, 0), self.dag.operand(node, 1));
        let amount = self.dag.constant(c1)?;
        if !self.dag.has_one_use(logic.node) {
            return None;
        }
        let logic_op = self.dag.opcode(logic);
        if !matches!(logic_op, ISD::And | ISD::Or | ISD::Xor) {
            return None;
        }
        // "Do not turn a 'not' into a regular xor."
        if logic_op == ISD::Xor
            && self
                .dag
                .constant(self.dag.operand(logic, 1))
                .is_some_and(|c| c == super::fold::mask(self.dag.value_type(logic).size_in_bits()))
        {
            return None;
        }
        let vt = self.dag.value_type(node);
        let width = u128::from(vt.size_in_bits());
        let first = |dag: &SelectionDAG, v: SDValue| -> Option<(SDValue, u128)> {
            if dag.opcode(v) != op || !dag.has_one_use(v.node) {
                return None;
            }
            let c0 = dag.constant(dag.operand(v, 1))?;
            (c0 + amount < width).then(|| (dag.operand(v, 0), c0))
        };
        let (lhs, rhs) = (self.dag.operand(logic, 0), self.dag.operand(logic, 1));
        let ((x, c0), y) = match (first(self.dag, lhs), first(self.dag, rhs)) {
            (Some(found), _) => (found, rhs),
            (None, Some(found)) => (found, lhs),
            _ => return None,
        };
        let sum = self.dag.get_constant(c0 + amount, self.dag.value_type(c1));
        let shifted_x = self.dag.get_node(op, vt, &[x, sum]);
        let shifted_y = self.dag.get_node(op, vt, &[y, c1]);
        Some(self.dag.get_node(logic_op, vt, &[shifted_x, shifted_y]))
    }

    /// `visitSHL`'s exact fold ([L11196][pin]): `shl (sr[la] exact x, c1), c2`
    /// is `shl x, c2 - c1` when `c1 <= c2` and `sr[la] x, c1 - c2` otherwise.
    /// The bits shifted out are zero, so no mask is left, and the inner
    /// shift's other uses do not matter.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L11196
    fn shl_of_exact_shift(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (inner, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        if self.dag.node(inner).flags.0 & SDNodeFlags::EXACT.0 == 0 {
            return None;
        }
        let (c2, c1) = (self.dag.constant(n1)?, self.dag.constant(self.dag.operand(inner, 1))?);
        let vt = self.dag.value_type(node);
        let width = u128::from(vt.size_in_bits());
        if c1 >= width || c2 >= width {
            return None;
        }
        let x = self.dag.operand(inner, 0);
        let amount_vt = self.dag.value_type(n1);
        if c1 <= c2 {
            let by = self.dag.get_constant(c2 - c1, amount_vt);
            return Some(self.dag.get_node(ISD::Shl, vt, &[x, by]));
        }
        let by = self.dag.get_constant(c1 - c2, amount_vt);
        Some(self.dag.get_node(self.dag.opcode(inner), vt, &[x, by]))
    }

    /// `visitSHL`: `shl (srl x, c1), c2` is `and (srl x, c1 - c2), (-1 << c1
    /// >> (c1 - c2))` when `c2 <= c1`, else `and (shl x, c2 - c1), (-1 <<
    /// c2)`; only when the inner shift has no other use or the amounts are
    /// equal (`shouldFoldConstantShiftPairToMask`: true on these CPUs).
    fn shl_of_srl(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (inner, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let (c2, c1) = (self.dag.constant(n1)?, self.dag.constant(self.dag.operand(inner, 1))?);
        let vt = self.dag.value_type(node);
        let width = u128::from(vt.size_in_bits());
        if c1 >= width || c2 >= width || (c1 != c2 && !self.dag.has_one_use(inner.node)) {
            return None;
        }
        let x = self.dag.operand(inner, 0);
        let amount_vt = self.dag.value_type(n1);
        let all = super::fold::mask(vt.size_in_bits());
        if c2 <= c1 {
            let diff = c1 - c2;
            let mask = ((all << c1) & all) >> diff;
            let shifted = if diff == 0 {
                x
            } else {
                let by = self.dag.get_constant(diff, amount_vt);
                self.dag.get_node(ISD::Srl, vt, &[x, by])
            };
            let mask = self.dag.get_constant(mask, vt);
            return Some(self.dag.get_node(ISD::And, vt, &[shifted, mask]));
        }
        let diff = c2 - c1;
        let by = self.dag.get_constant(diff, amount_vt);
        let shifted = self.dag.get_node(ISD::Shl, vt, &[x, by]);
        let mask = self.dag.get_constant((all << c2) & all, vt);
        Some(self.dag.get_node(ISD::And, vt, &[shifted, mask]))
    }

    /// `visitSRL`: `srl (trunc (srl x, c1)), c2` is 0 or `trunc (srl x,
    /// c1+c2)` when the truncated width plus `c1` is the inner width, else
    /// `trunc (and (srl x, c1+c2), mask)` when both nodes have one use.
    fn srl_of_trunc_of_srl(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (trunc, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let inner = self.dag.operand(trunc, 0);
        if self.dag.opcode(inner) != ISD::Srl {
            return None;
        }
        let (c1, c2) = (self.dag.constant(self.dag.operand(inner, 1))?, self.dag.constant(n1)?);
        let (vt, inner_vt) = (self.dag.value_type(node), self.dag.value_type(inner));
        let (op_size, inner_size) = (u128::from(vt.size_in_bits()), u128::from(inner_vt.size_in_bits()));
        let amount_vt = self.dag.value_type(self.dag.operand(inner, 1));
        if c1 + op_size == inner_size {
            if c1 + c2 >= inner_size {
                return Some(self.dag.get_constant(0, vt));
            }
            let amount = self.dag.get_constant(c1 + c2, amount_vt);
            let shifted = self.dag.get_node(ISD::Srl, inner_vt, &[self.dag.operand(inner, 0), amount]);
            return Some(self.dag.get_node(ISD::Truncate, vt, &[shifted]));
        }
        if self.dag.has_one_use(trunc.node) && self.dag.has_one_use(inner.node) && c1 + c2 < inner_size && c2 < op_size
        {
            let amount = self.dag.get_constant(c1 + c2, amount_vt);
            let shifted = self.dag.get_node(ISD::Srl, inner_vt, &[self.dag.operand(inner, 0), amount]);
            let mask = self.dag.get_constant(super::fold::mask((op_size - c2) as u32), inner_vt);
            let masked = self.dag.get_node(ISD::And, inner_vt, &[shifted, mask]);
            return Some(self.dag.get_node(ISD::Truncate, vt, &[masked]));
        }
        None
    }

    /// `visitFREEZE`, the first fold: `freeze x` is `x` when `x` cannot be
    /// undef or poison. (The pin's second fold, hoisting the freeze over a
    /// multi-use operand, is not translated.)
    fn visit_freeze(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let x = self.dag.operand(node, 0);
        if !self.guaranteed_not_undef_or_poison(x, 0) {
            return self.push_freeze_through(x);
        }
        // The users now read `x` directly: their users may fold with it too
        // (`not (or (cmp), freeze (cmp))` was visited with the freeze there).
        let users: Vec<u32> = self.dag.users(node).collect();
        for user in users {
            self.add_to_worklist_with_users(user);
        }
        Some(x)
    }

    /// `freeze(op(x, ...))` to `op(freeze(x), ...)`: the operation cannot make
    /// undef or poison itself (flags ignored), has one use, and has at most one
    /// operand that may be undef or poison (a `setcc` may have two).
    fn push_freeze_through(
        &mut self,
        op: SDValue,
    ) -> Option<SDValue> {
        let opcode = self.dag.opcode(op);
        let is_setcc = opcode == ISD::SetCC;
        let integer_setcc = is_setcc && self.dag.value_type(self.dag.operand(op, 0)).is_integer();
        if !self.hooks.push_freeze_through_operands()
            || !(integer_setcc || matches!(opcode, ISD::And | ISD::Or | ISD::Xor | ISD::Add | ISD::Sub))
            || !self.dag.has_one_use(op.node)
        {
            return None;
        }
        let mut maybe_poison: Vec<usize> = Vec::new();
        // A `setcc`'s third operand is its condition code, not a value.
        let values = if is_setcc { 2 } else { self.dag.node(op).ops.len() };
        for at in 0..values {
            let operand = self.dag.operand(op, at);
            if self.guaranteed_not_undef_or_poison(operand, 0) {
                continue;
            }
            let seen = maybe_poison.iter().any(|&other| self.dag.operand(op, other) == operand);
            if !maybe_poison.is_empty() && !seen && !is_setcc {
                return None;
            }
            if !seen {
                maybe_poison.push(at);
            }
        }
        let mut ops: Vec<SDValue> = (0..self.dag.node(op).ops.len()).map(|at| self.dag.operand(op, at)).collect();
        for at in maybe_poison {
            if self.dag.is_undef(ops[at]) {
                continue;
            }
            let frozen = self.dag.get_node(ISD::Freeze, self.dag.value_type(ops[at]), &[ops[at]]);
            for other in ops.iter_mut().filter(|o| **o == self.dag.operand(op, at)) {
                *other = frozen;
            }
        }
        let vt = self.dag.value_type(op);
        // The node is rebuilt without its poison-generating flags.
        Some(self.dag.get_node(opcode, vt, &ops))
    }

    /// `isGuaranteedNotToBeUndefOrPoison`, for the nodes the combiner builds:
    /// constants, registers, frame indexes and `freeze`; a load through such a
    /// pointer; and an integer operation without a poison-generating flag whose
    /// operands are.
    ///
    /// DEVIATE SD213: no vector, `AssertSext`/`AssertZext`, overflow-flag or
    /// shift-amount range reasoning beyond a constant amount below the width.
    /// Exit: those. Owner sdag-b. Review 2026-12-10.
    fn guaranteed_not_undef_or_poison(
        &self,
        value: SDValue,
        depth: u32,
    ) -> bool {
        if self.dag.opcode(value) == ISD::Freeze {
            return true;
        }
        if depth >= 6 {
            return false;
        }
        if self.dag.constant(value).is_some() {
            return true;
        }
        let poison_flags = SDNodeFlags::NO_UNSIGNED_WRAP.0 | SDNodeFlags::NO_SIGNED_WRAP.0 | SDNodeFlags::EXACT.0;
        let flagged = self.dag.node(value).flags.0 & poison_flags != 0;
        let all = |ops: &[usize]| {
            ops.iter().all(|&i| self.guaranteed_not_undef_or_poison(self.dag.operand(value, i), depth + 1))
        };
        match self.dag.opcode(value) {
            ISD::Register | ISD::FrameIndex | ISD::ValueType | ISD::CopyFromReg => true,
            ISD::Load => all(&[1]),
            ISD::SetCC => all(&[0, 1]),
            ISD::And | ISD::Or | ISD::Xor | ISD::Add | ISD::Sub | ISD::Mul if !flagged => all(&[0, 1]),
            ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend | ISD::Truncate if !flagged => all(&[0]),
            ISD::Shl | ISD::Srl | ISD::Sra if !flagged => {
                let width = u128::from(self.dag.value_type(value).size_in_bits());
                self.dag.constant(self.dag.operand(value, 1)).is_some_and(|by| by < width) && all(&[0])
            }
            _ => false,
        }
    }

    /// `PromoteIntBinOp`, `PromoteIntShiftOp` and `PromoteExtend`: an operation
    /// the target wants in a wider type is done there, and truncated.
    ///
    /// DEVIATE SD211: `AssertSext`/`AssertZext` are not looked through, and a
    /// volatile load is any-extended rather than widened. Exit: those. Owner
    /// sdag-b. Review 2026-12-10.
    fn promote_int_op(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let op = self.dag.opcode(node);
        let class = match op {
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor => 0,
            ISD::Shl | ISD::Sra | ISD::Srl => 1,
            ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend => 2,
            _ => return None,
        };
        let vt = self.dag.value_type(node);
        if !vt.is_integer() {
            return None;
        }
        let tli = self.tli?;
        let wide = self.hooks.promote_type(self.dag, tli, node)?;
        match class {
            2 => {
                let x = self.dag.operand(node, 0);
                Some(self.dag.get_node(op, vt, &[x]))
            }
            0 => {
                let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let mut loads = Vec::new();
                let (w0, w1) =
                    (self.promote_operand(n0, wide, &mut loads)?, self.promote_operand(n1, wide, &mut loads)?);
                let wide_op = self.dag.get_node(op, wide, &[w0, w1]);
                self.replace_loads_with_promoted(loads);
                Some(self.dag.get_node(ISD::Truncate, vt, &[wide_op]))
            }
            _ => {
                let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let flags = self.dag.node(node).flags;
                let mut loads = Vec::new();
                let w0 = match op {
                    ISD::Sra => self.sext_promote_operand(n0, wide, tli, &mut loads)?,
                    ISD::Srl => self.zext_promote_operand(n0, wide, &mut loads)?,
                    _ if flags.0 & SDNodeFlags::NO_UNSIGNED_WRAP.0 != 0 => {
                        self.zext_promote_operand(n0, wide, &mut loads)?
                    }
                    _ if flags.0 & SDNodeFlags::NO_SIGNED_WRAP.0 != 0 => {
                        self.sext_promote_operand(n0, wide, tli, &mut loads)?
                    }
                    _ => self.promote_operand(n0, wide, &mut loads)?,
                };
                let wide_op = self.dag.get_node(op, wide, &[w0, n1]);
                self.replace_loads_with_promoted(loads);
                Some(self.dag.get_node(ISD::Truncate, vt, &[wide_op]))
            }
        }
    }

    /// `PromoteOperand`: a load becomes an extending load of its memory type
    /// (recorded in `loads` for `replace_loads_with_promoted`), a constant is
    /// extended, anything else any-extended.
    fn promote_operand(
        &mut self,
        value: SDValue,
        wide: EVT,
        loads: &mut Vec<(SDValue, SDValue)>,
    ) -> Option<SDValue> {
        if self.dag.opcode(value) == ISD::Load && value.res == 0 {
            if let NodeData::Mem(info) = self.dag.node(value).data.clone() {
                let ext = info.ext.unwrap_or(LoadExt::ExtLoad);
                if !info.volatile
                    && let (EVT::Simple(wide_mvt), EVT::Simple(mem_mvt)) = (wide, info.mem_vt)
                {
                    let legal =
                        self.tli.is_some_and(|tli| tli.load_ext_action(ext, wide_mvt, mem_mvt) == Action::Legal);
                    if legal {
                        let (chain, ptr) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
                        let plain = super::dag::MemInfo { ext: None, ..info };
                        let extended = self.dag.get_ext_load(ext, wide, chain, ptr, plain);
                        loads.push((value, extended));
                        return Some(extended);
                    }
                }
            }
        }
        if self.dag.constant(value).is_some() {
            let byte_sized = self.dag.value_type(value).size_in_bits() % 8 == 0;
            let ext = if byte_sized { ISD::SignExtend } else { ISD::ZeroExtend };
            return Some(self.dag.get_node(ext, wide, &[value]));
        }
        if !self.tli?.is_operation_legal(ISD::AnyExtend, wide) {
            return None;
        }
        Some(self.dag.get_node(ISD::AnyExtend, wide, &[value]))
    }

    /// `ReplaceLoadWithPromotedLoad`: the other users of a promoted load read
    /// the extending load truncated, and its chain users the new chain.
    fn replace_loads_with_promoted(
        &mut self,
        loads: Vec<(SDValue, SDValue)>,
    ) {
        for (load, extended) in loads {
            let narrow = self.dag.value_type(load);
            let truncated = self.dag.get_node(ISD::Truncate, narrow, &[extended]);
            self.replace_value(load, truncated);
            self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        }
    }

    fn sext_promote_operand(
        &mut self,
        value: SDValue,
        wide: EVT,
        tli: &TargetLowering,
        loads: &mut Vec<(SDValue, SDValue)>,
    ) -> Option<SDValue> {
        if !tli.is_operation_legal(ISD::SignExtendInReg, wide) {
            return None;
        }
        let old = self.dag.value_type(value);
        let promoted = self.promote_operand(value, wide, loads)?;
        let ty = self.dag.get_value_type_node(old);
        Some(self.dag.get_node(ISD::SignExtendInReg, wide, &[promoted, ty]))
    }

    /// `ZExtPromoteOperand`: `getZeroExtendInReg` is an `and` with the old
    /// type's mask.
    fn zext_promote_operand(
        &mut self,
        value: SDValue,
        wide: EVT,
        loads: &mut Vec<(SDValue, SDValue)>,
    ) -> Option<SDValue> {
        let old = self.dag.value_type(value);
        let promoted = self.promote_operand(value, wide, loads)?;
        let mask = self.dag.get_constant(super::fold::mask(old.size_in_bits()), wide);
        Some(self.dag.get_node(ISD::And, wide, &[promoted, mask]))
    }

    /// `visitSUB`, the first folds: `x - x` is 0, and `x - C` is `x + -C`, so
    /// the add-shaped folds and the matcher's `dec`/`lea` patterns see one
    /// form. (The pin's `umin`, select, `ctlz` and negation folds are not
    /// translated.)
    fn visit_sub(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        if !vt.is_integer() {
            return None;
        }
        if n0 == n1 {
            return Some(self.dag.get_constant(0, vt));
        }
        if let Some(saturated) = self.fold_sub_to_usub_sat(vt, node) {
            return Some(saturated);
        }
        if self.hooks.sub_algebra_folds()
            && let Some(folded) = self.fold_sub_algebra(vt, n0, n1)
        {
            return Some(folded);
        }
        // `visitSUB` ([L4651][pin]): with no chance of borrowing from the
        // adjacent bits, `sub C0, X` is `xor X, C0` (`5 - zext(c)` is `5 ^ c`).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4651
        if self.hooks.sub_without_borrow_is_xor()
            && let Some(c0) = self.dag.constant(n0).filter(|_| !self.dag.is_opaque(n0))
        {
            let mask = super::fold::mask(vt.size_in_bits());
            let maybe_ones = !super::known_bits::known_bits(self.dag, n1).zero & mask;
            // `sub -1, x` is `~x`, a fold `sub_algebra_folds` keeps off (it
            // moved x86_16 spills).
            if c0 & mask != mask && c0.wrapping_sub(maybe_ones) & mask == (c0 ^ maybe_ones) & mask {
                return Some(self.dag.get_node(ISD::Xor, vt, &[n1, n0]));
            }
        }
        let c = self.dag.constant(n1).filter(|_| !self.dag.is_opaque(n1))?;
        let negated = self.dag.get_constant(c.wrapping_neg() & super::fold::mask(vt.size_in_bits()), vt);
        Some(self.dag.get_node(ISD::Add, vt, &[n0, negated]))
    }

    /// `visitADDO` for `uaddo`
    /// ([L3621](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3621)):
    /// a dead overflow is an `add`, a constant goes right, `x + 0` has no
    /// carry, and a sum that cannot overflow is an `add`. DEVIATE UADDO1: the
    /// `(uaddo (xor a, -1), 1)` fold is not translated (no cell);
    /// `visitUADDOLike` is (`uaddo_like`, for `d / 1000` on a u64).
    fn visit_uaddo(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let carry_vt = self.dag.node(node).values[1];
        let overflow = SDValue { node: node.node, res: 1 };
        if !self.dag.has_users(overflow) {
            let sum = self.dag.get_node(ISD::Add, vt, &[n0, n1]);
            let undef = self.dag.get_undef(carry_vt);
            self.dag.combine_to(node, sum);
            self.dag.combine_to(overflow, undef);
            return None;
        }
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node_multi(ISD::UAddO, &[vt, carry_vt], &[n1, n0]));
        }
        let no_carry = self.dag.get_constant(0, carry_vt);
        if self.dag.is_null_constant(n1) {
            self.dag.combine_to(node, n0);
            self.dag.combine_to(overflow, no_carry);
            return None;
        }
        if self.unsigned_add_never_overflows(n0, n1) {
            let sum = self.dag.get_node(ISD::Add, vt, &[n0, n1]);
            self.dag.combine_to(node, sum);
            self.dag.combine_to(overflow, no_carry);
            return None;
        }
        self.uaddo_like(node, n0, n1).or_else(|| self.uaddo_like(node, n1, n0))
    }

    /// `visitUADDOLike`
    /// ([L3673](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3673)):
    /// `(uaddo X, (uaddo_carry Y, 0, c))` is `(uaddo_carry X, Y, c)` where
    /// `Y + 1` cannot overflow, and `(uaddo X, carry)` is `(uaddo_carry X, 0,
    /// carry)`.
    fn uaddo_like(
        &mut self,
        node: SDValue,
        x: SDValue,
        y: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(x);
        let values = self.dag.node(node).values.clone();
        if self.dag.opcode(y) == ISD::UAddOCarry && self.dag.is_null_constant(self.dag.operand(y, 1)) {
            let low = self.dag.operand(y, 0);
            let one = self.dag.get_constant(1, vt);
            if self.unsigned_add_never_overflows(low, one) {
                return Some(self.dag.get_node_multi(ISD::UAddOCarry, &values, &[x, low, self.dag.operand(y, 2)]));
            }
        }
        if self.tli.is_some_and(|tli| tli.is_operation_legal_or_custom(ISD::UAddOCarry, vt))
            && let Some(carry) = self.as_carry(y)
        {
            let zero = self.dag.get_constant(0, vt);
            return Some(self.dag.get_node_multi(ISD::UAddOCarry, &values, &[x, zero, carry]));
        }
        None
    }

    /// `computeOverflowForUnsignedAdd(..) == OFK_Never` from the known bits:
    /// the two largest values the operands can have do not carry.
    fn unsigned_add_never_overflows(
        &self,
        a: SDValue,
        b: SDValue,
    ) -> bool {
        let (ka, kb) = (super::known_bits::known_bits(self.dag, a), super::known_bits::known_bits(self.dag, b));
        let all = super::fold::mask(ka.width);
        // At 128 bits the sum of the two maxima wraps in a `u128`, which read
        // as "fits".
        (!ka.zero & all).checked_add(!kb.zero & all).is_some_and(|sum| sum <= all)
    }

    /// `visitUADDO_CARRY`
    /// ([L3716](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L3716)):
    /// a constant goes right, a zero carry in is a `uaddo`, and with the
    /// overflow dead `(uaddo_carry (add|uaddo X, Y), 0, c)` is `(uaddo_carry
    /// X, Y, c)`. DEVIATE UADDO1: the `0 + 0 + c`, `(xor a, -1)` and diamond
    /// folds are not translated.
    fn visit_uaddo_carry(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1, carry) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let values = self.dag.node(node).values.clone();
        if self.dag.constant(n0).is_some() && self.dag.constant(n1).is_none() {
            return Some(self.dag.get_node_multi(ISD::UAddOCarry, &values, &[n1, n0, carry]));
        }
        if self.dag.is_null_constant(carry) {
            let legal = match self.tli {
                Some(tli) if self.level >= CombineLevel::AfterLegalizeDag => {
                    tli.is_operation_legal_or_custom(ISD::UAddO, values[0])
                }
                _ => true,
            };
            if legal {
                return Some(self.dag.get_node_multi(ISD::UAddO, &values, &[n0, n1]));
            }
        }
        let dead = !self.dag.has_users(SDValue { node: node.node, res: 1 });
        for (x, zero) in [(n0, n1), (n1, n0)] {
            if !dead || !self.dag.is_null_constant(zero) {
                continue;
            }
            let adds = self.dag.opcode(x) == ISD::Add
                || (self.dag.opcode(x) == ISD::UAddO && x.res == 0 && SDValue { node: x.node, res: 1 } != carry);
            if adds {
                let (a, b) = (self.dag.operand(x, 0), self.dag.operand(x, 1));
                return Some(self.dag.get_node_multi(ISD::UAddOCarry, &values, &[a, b, carry]));
            }
        }
        None
    }

    /// `visitSUBO` for `usubo`
    /// ([L4793](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4793)):
    /// a dead borrow is a `sub`, `x - x` and `x - 0` do not borrow, a
    /// difference that cannot borrow is a `sub`, and `usubo -1, x` is `~x`.
    /// DEVIATE USUBO1: `willNotOverflowSub` reads the operand ranges from the
    /// known bits only (the pin intersects a `ConstantRange` too); signed
    /// `ssubo` is not dispatched (no cell). Exit: with `ssubo`, 2026-11-20.
    fn visit_usubo(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let carry_vt = self.dag.node(node).values[1];
        let borrow = SDValue { node: node.node, res: 1 };
        if !self.dag.has_users(borrow) {
            let diff = self.dag.get_node(ISD::Sub, vt, &[n0, n1]);
            let undef = self.dag.get_undef(carry_vt);
            self.dag.combine_to(node, diff);
            self.dag.combine_to(borrow, undef);
            return None;
        }
        let no_borrow = self.dag.get_constant(0, carry_vt);
        if n0 == n1 {
            let zero = self.dag.get_constant(0, vt);
            self.dag.combine_to(node, zero);
            self.dag.combine_to(borrow, no_borrow);
            return None;
        }
        if self.dag.is_null_constant(n1) {
            self.dag.combine_to(node, n0);
            self.dag.combine_to(borrow, no_borrow);
            return None;
        }
        let (k0, k1) = (super::known_bits::known_bits(self.dag, n0), super::known_bits::known_bits(self.dag, n1));
        let all = super::fold::mask(k0.width);
        if k0.one >= (!k1.zero & all) {
            let diff = self.dag.get_node(ISD::Sub, vt, &[n0, n1]);
            self.dag.combine_to(node, diff);
            self.dag.combine_to(borrow, no_borrow);
            return None;
        }
        if self.dag.constant(n0) == Some(all) {
            let not = self.dag.get_node(ISD::Xor, vt, &[n1, n0]);
            self.dag.combine_to(node, not);
            self.dag.combine_to(borrow, no_borrow);
            return None;
        }
        None
    }

    /// `visitUSUBO_CARRY`
    /// ([L4847](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4847)):
    /// a zero borrow in is a `usubo`, and with the borrow out dead
    /// `(usubo_carry (sub X, Y), 0, c)` is `(usubo_carry X, Y, c)`.
    fn visit_usubo_carry(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1, carry) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let values = self.dag.node(node).values.clone();
        if self.dag.is_null_constant(carry) {
            let legal = match self.tli {
                Some(tli) if self.level >= CombineLevel::AfterLegalizeDag => {
                    tli.is_operation_legal_or_custom(ISD::USubO, values[0])
                }
                _ => true,
            };
            if legal {
                return Some(self.dag.get_node_multi(ISD::USubO, &values, &[n0, n1]));
            }
        }
        if self.dag.opcode(n0) == ISD::Sub
            && self.dag.is_null_constant(n1)
            && !self.dag.has_users(SDValue { node: node.node, res: 1 })
        {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            return Some(self.dag.get_node_multi(ISD::USubOCarry, &values, &[a, b, carry]));
        }
        None
    }

    /// `visitSETCCCARRY`
    /// ([L15188](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L15188)):
    /// with no borrow in, the node is a plain `setcc` of its operands. The low
    /// words' `usubo x, 0` folds to a zero borrow, so a compare of a long with
    /// a constant whose low words are zero reaches here.
    fn visit_setcc_carry(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (lhs, rhs, carry, cond) = (
            self.dag.operand(node, 0),
            self.dag.operand(node, 1),
            self.dag.operand(node, 2),
            self.dag.operand(node, 3),
        );
        if !self.dag.is_null_constant(carry) {
            return None;
        }
        let vt = self.dag.value_type(node);
        Some(self.dag.get_node(ISD::SetCC, vt, &[lhs, rhs, cond]))
    }

    /// The algebraic folds of `visitSUB`
    /// ([L4411](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4411)
    /// to L4470): `(sub -1, x)` is `~x`, `a - (0 - b)` is `a + b`, the
    /// pairs that cancel, the constants that meet, and `a - (b - c)` is
    /// `a + (c - b)`. O(1) per node.
    fn fold_sub_algebra(
        &mut self,
        vt: EVT,
        n0: SDValue,
        n1: SDValue,
    ) -> Option<SDValue> {
        let mask = super::fold::mask(vt.size_in_bits());
        let konst = |this: &Self, v: SDValue| this.dag.constant(v).filter(|_| !this.dag.is_opaque(v));
        let (op0, op1) = (self.dag.opcode(n0), self.dag.opcode(n1));
        if konst(self, n0) == Some(mask) {
            return Some(self.dag.get_node(ISD::Xor, vt, &[n1, n0]));
        }
        // a - (0 - b) -> a + b
        if op1 == ISD::Sub && self.dag.constant(self.dag.operand(n1, 0)) == Some(0) {
            let b = self.dag.operand(n1, 1);
            return Some(self.dag.get_node(ISD::Add, vt, &[n0, b]));
        }
        // a - (a - b) -> b
        if op1 == ISD::Sub && self.dag.operand(n1, 0) == n0 {
            return Some(self.dag.operand(n1, 1));
        }
        if op0 == ISD::Add {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            // (a + b) - a -> b, (a + b) - b -> a
            if a == n1 {
                return Some(b);
            }
            if b == n1 {
                return Some(a);
            }
            // (a + c1) - c2 -> a + (c1 - c2)
            if let (Some(c1), Some(c2)) = (konst(self, b), konst(self, n1)) {
                let folded = self.dag.get_constant(c1.wrapping_sub(c2) & mask, vt);
                return Some(self.dag.get_node(ISD::Add, vt, &[a, folded]));
            }
            // ((a + (b + c)) - b) -> a + c
            let inner = b;
            if self.dag.opcode(inner) == ISD::Add && self.dag.operand(inner, 0) == n1 {
                let c = self.dag.operand(inner, 1);
                return Some(self.dag.get_node(ISD::Add, vt, &[a, c]));
            }
            // ((a + (b - c)) - b) -> a - c
            if self.dag.opcode(inner) == ISD::Sub && self.dag.operand(inner, 0) == n1 {
                let c = self.dag.operand(inner, 1);
                return Some(self.dag.get_node(ISD::Sub, vt, &[a, c]));
            }
        }
        // c2 - (a + c1) -> (c2 - c1) - a
        if op1 == ISD::Add
            && let (Some(c2), Some(c1)) = (konst(self, n0), konst(self, self.dag.operand(n1, 1)))
        {
            let folded = self.dag.get_constant(c2.wrapping_sub(c1) & mask, vt);
            let a = self.dag.operand(n1, 0);
            return Some(self.dag.get_node(ISD::Sub, vt, &[folded, a]));
        }
        if op0 == ISD::Sub {
            let (a, b) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            // (a - c1) - c2 -> a - (c1 + c2)
            if let (Some(c1), Some(c2)) = (konst(self, b), konst(self, n1)) {
                let folded = self.dag.get_constant(c1.wrapping_add(c2) & mask, vt);
                return Some(self.dag.get_node(ISD::Sub, vt, &[a, folded]));
            }
            // (c1 - a) - c2 -> (c1 - c2) - a
            if let (Some(c1), Some(c2)) = (konst(self, a), konst(self, n1)) {
                let folded = self.dag.get_constant(c1.wrapping_sub(c2) & mask, vt);
                return Some(self.dag.get_node(ISD::Sub, vt, &[folded, b]));
            }
            // ((a - (b - c)) - c) -> a - b
            if self.dag.opcode(b) == ISD::Sub && self.dag.operand(b, 1) == n1 {
                let inner = self.dag.operand(b, 0);
                return Some(self.dag.get_node(ISD::Sub, vt, &[a, inner]));
            }
        }
        // a - (b - c) -> a + (c - b)
        if op1 == ISD::Sub && self.dag.has_one_use(n1.node) {
            let (b, c) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1));
            let flipped = self.dag.get_node(ISD::Sub, vt, &[c, b]);
            return Some(self.dag.get_node(ISD::Add, vt, &[n0, flipped]));
        }
        // a - (a & b) -> a & ~b
        if self.dag.opcode(n1) == ISD::And {
            let (x, y) = (self.dag.operand(n1, 0), self.dag.operand(n1, 1));
            let other = if x == n0 {
                Some(y)
            } else if y == n0 {
                Some(x)
            } else {
                None
            };
            if let Some(b) = other
                && (self.dag.has_one_use(n1.node) || konst(self, b).is_some())
            {
                let not = self.dag.get_not(b, vt);
                return Some(self.dag.get_node(ISD::And, vt, &[n0, not]));
            }
        }
        None
    }

    /// `getTruncatedUSUBSAT`
    /// ([L4070](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4070)):
    /// a `usubsat` of `lhs` and
    /// `rhs` in `dst`, the operands clamped and truncated from `src`.
    fn truncated_usub_sat(
        &mut self,
        dst: EVT,
        src: EVT,
        lhs: SDValue,
        rhs: SDValue,
    ) -> Option<SDValue> {
        if dst == src {
            return Some(self.dag.get_node(ISD::USubSat, dst, &[lhs, rhs]));
        }
        // "If the LHS is zero-extended then we can perform the USUBSAT as
        // DstVT by clamping RHS."
        let (src_bits, dst_bits) = (src.size_in_bits(), dst.size_in_bits());
        let upper = super::fold::mask(src_bits) & !super::fold::mask(dst_bits);
        if !super::known_bits::masked_value_is_zero(self.dag, lhs, upper) {
            return None;
        }
        let limit = self.dag.get_constant(super::fold::mask(dst_bits), src);
        let clamped = self.dag.get_node(ISD::UMin, src, &[rhs, limit]);
        let rhs = self.dag.get_node(ISD::Truncate, dst, &[clamped]);
        let lhs = self.dag.get_node(ISD::Truncate, dst, &[lhs]);
        Some(self.dag.get_node(ISD::USubSat, dst, &[lhs, rhs]))
    }

    /// `foldSubToUSubSat`
    /// ([L4100](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4100),
    /// called from `visitSUB`
    /// [L4501](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L4501)
    /// and `visitTRUNCATE`
    /// [L17910](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17910)):
    /// `umax(a, b) - b` and `a - umin(a, b)` are
    /// `usubsat(a, b)`, optionally as a truncated type. `sub` is a `SUB` node
    /// and `dst` the type the result is wanted in. O(1) per node.
    fn fold_sub_to_usub_sat(
        &mut self,
        dst: EVT,
        sub: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(sub) != ISD::Sub {
            return None;
        }
        // `!LegalOperations || hasOperation(USUBSAT, DstVT)`
        if self.legal_operations() && !self.tli?.is_operation_legal_or_custom(ISD::USubSat, dst) {
            return None;
        }
        let sub_vt = self.dag.value_type(sub);
        let (op0, op1) = (self.dag.operand(sub, 0), self.dag.operand(sub, 1));
        // "Try to find umax(a,b) - b or a - umin(a,b) patterns."
        if self.dag.opcode(op0) == ISD::UMax && self.dag.has_one_use(op0.node) {
            let (max_lhs, max_rhs) = (self.dag.operand(op0, 0), self.dag.operand(op0, 1));
            if max_lhs == op1 {
                return self.truncated_usub_sat(dst, sub_vt, max_rhs, op1);
            }
            if max_rhs == op1 {
                return self.truncated_usub_sat(dst, sub_vt, max_lhs, op1);
            }
        }
        if self.dag.opcode(op1) == ISD::UMin && self.dag.has_one_use(op1.node) {
            let (min_lhs, min_rhs) = (self.dag.operand(op1, 0), self.dag.operand(op1, 1));
            if min_lhs == op0 {
                return self.truncated_usub_sat(dst, sub_vt, op0, min_rhs);
            }
            if min_rhs == op0 {
                return self.truncated_usub_sat(dst, sub_vt, op0, min_lhs);
            }
        }
        // `sub(a, trunc(umin(zext(a), b)))` is `usubsat(a, trunc(umin(b,
        // SatLimit)))`.
        if self.dag.opcode(op1) == ISD::Truncate {
            let min = self.dag.operand(op1, 0);
            if self.dag.opcode(min) == ISD::UMin && self.dag.has_one_use(min.node) {
                let (min_lhs, min_rhs) = (self.dag.operand(min, 0), self.dag.operand(min, 1));
                for (zext, other) in [(min_lhs, min_rhs), (min_rhs, min_lhs)] {
                    if self.dag.opcode(zext) == ISD::ZeroExtend && self.dag.operand(zext, 0) == op0 {
                        return self.truncated_usub_sat(dst, self.dag.value_type(zext), zext, other);
                    }
                }
            }
        }
        None
    }

    /// `visitBRCOND`
    /// ([L21199](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L21199)):
    /// the branch on a frozen condition is the branch on the condition, and a
    /// condition that is a one-use `xor` is rebuilt as a compare
    /// ([`Self::rebuild_set_cc`]). DEVIATE: the `BR_CC` fold is left out (x86
    /// has no `BR_CC`), and so are the `FREEZE` folds under a `SETCC`.
    fn visit_brcond(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if !self.hooks.rebuild_brcond_condition() {
            return None;
        }
        let (chain, cond, dest) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        if self.dag.opcode(cond) == ISD::Freeze && self.dag.has_one_use(cond.node) {
            let inner = self.dag.operand(cond, 0);
            return Some(self.dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, inner, dest]));
        }
        if self.dag.has_one_use(cond.node)
            && let Some(rebuilt) = self.rebuild_set_cc(cond)
        {
            return Some(self.dag.get_node(ISD::BrCond, EVT::OTHER, &[chain, rebuilt, dest]));
        }
        None
    }

    /// `rebuildSetCC`
    /// ([L21289](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L21289)):
    /// `(and x, 2^k) >> k` is `setne (and x, 2^k), 0`; `xor x, y` is `setne x,
    /// y` and `xor (xor x, y), -1` is `seteq x, y`, after the `xor` is
    /// simplified as it stands (so a compare pair under it is flipped by
    /// De Morgan, not by this).
    fn rebuild_set_cc(
        &mut self,
        mut n: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(n) == ISD::Srl
            || (self.dag.opcode(n) == ISD::Truncate
                && self.dag.has_one_use(self.dag.operand(n, 0).node)
                && self.dag.opcode(self.dag.operand(n, 0)) == ISD::Srl)
        {
            if self.dag.opcode(n) == ISD::Truncate {
                n = self.dag.operand(n, 0);
            }
            let (op0, op1) = (self.dag.operand(n, 0), self.dag.operand(n, 1));
            if self.dag.opcode(op0) == ISD::And
                && let (Some(shift), Some(mask)) = (self.dag.constant(op1), self.dag.constant(self.dag.operand(op0, 1)))
                && self.dag.opcode(self.dag.operand(op0, 1)) == ISD::Constant
                && mask.is_power_of_two()
                && u128::from(mask.trailing_zeros()) == shift as u128
            {
                let vt = self.dag.value_type(op0);
                let zero = self.dag.get_constant(0, vt);
                let result = self.set_cc_result_type(vt);
                return Some(self.dag.get_setcc(result, op0, zero, CondCode::SetNe));
            }
        }
        if self.dag.opcode(n) == ISD::Xor {
            while self.dag.opcode(n) == ISD::Xor {
                match self.visit_xor(n) {
                    Some(simplified) if simplified.node != n.node => n = simplified,
                    _ => break,
                }
            }
            if self.dag.opcode(n) != ISD::Xor {
                return Some(n);
            }
            let (mut op0, mut op1) = (self.dag.operand(n, 0), self.dag.operand(n, 1));
            if self.dag.opcode(op0) != ISD::SetCC && self.dag.opcode(op1) != ISD::SetCC {
                let mut equal = false;
                let ones = match self.dag.value_type(n).size_in_bits() {
                    128.. => u128::MAX,
                    bits => (1u128 << bits) - 1,
                };
                if self.dag.constant(op1) == Some(ones)
                    && self.dag.has_one_use(op0.node)
                    && self.dag.opcode(op0) == ISD::Xor
                    && self.dag.value_type(op0) == EVT::I1
                {
                    n = op0;
                    op0 = self.dag.operand(n, 0);
                    op1 = self.dag.operand(n, 1);
                    equal = true;
                }
                let cc = if equal { CondCode::SetEq } else { CondCode::SetNe };
                let legal = self.level < CombineLevel::AfterLegalizeVectorOps
                    || match (self.tli, self.dag.value_type(op0)) {
                        (Some(tli), EVT::Simple(operand)) => tli.cond_code_action(cc, operand) == Action::Legal,
                        _ => true,
                    };
                if legal {
                    let result = self.set_cc_result_type(self.dag.value_type(n));
                    return Some(self.dag.get_setcc(result, op0, op1, cc));
                }
            }
        }
        None
    }

    /// `getSetCCResultType` once types are legal, else the type itself.
    fn set_cc_result_type(
        &self,
        vt: EVT,
    ) -> EVT {
        match self.tli {
            Some(tli) if self.level >= CombineLevel::AfterLegalizeTypes => tli.types.set_cc_result_type(vt),
            _ => vt,
        }
    }

    /// `visitSTORE`'s two checks of the store the chain ends in
    /// ([L24849](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24849)):
    /// the same value to the same place again is the chain itself, and a store
    /// nothing else is chained to, to a part of what this one writes, is
    /// dropped.
    fn dead_preceding_store(
        &mut self,
        node: SDValue,
        chain: SDValue,
        value: SDValue,
        ptr: SDValue,
        info: MemInfo,
    ) -> Option<SDValue> {
        if self.opt_none || info.volatile || info.ordering.is_some() {
            return None;
        }
        if let Some(earlier) = self.dead_for_the_next_store(node, chain, info) {
            return Some(earlier);
        }
        if self.dag.opcode(chain) != ISD::Store {
            return None;
        }
        let NodeData::Mem(before) = self.dag.node(chain).data else { return None };
        if before.volatile || before.ordering.is_some() || before.space != info.space {
            return None;
        }
        if self.dag.operand(chain, 2) == ptr && self.dag.operand(chain, 1) == value && before.mem_vt == info.mem_vt {
            return Some(chain);
        }
        if !self.dag.has_one_use(chain.node) || self.dag.opcode(self.dag.operand(chain, 2)) == ISD::Undef {
            return None;
        }
        // `BaseIndexOffset::contains`: the earlier store starts at or after
        // this one and ends inside it.
        let bytes = super::address_analysis::BaseIndexOffset::matching(self.dag, node)
            .equal_base_index(&super::address_analysis::BaseIndexOffset::matching(self.dag, chain), self.dag)?;
        if bytes < 0 || bytes * 8 + i64::from(before.mem_vt.size_in_bits()) > i64::from(info.mem_vt.size_in_bits()) {
            return None;
        }
        let earlier = self.dag.operand(chain, 0);
        self.replace_value(chain, earlier);
        self.dag.delete_replaced(chain.node);
        self.queue_survivors();
        Some(node)
    }

    /// The check above from the earlier store: its one user is a store of the
    /// same chain position that writes all of it. Which of the two is looked at
    /// last depends on the order the chains were improved in.
    fn dead_for_the_next_store(
        &mut self,
        node: SDValue,
        chain: SDValue,
        info: MemInfo,
    ) -> Option<SDValue> {
        if !self.dag.has_one_use(node.node) {
            return None;
        }
        let next = self.dag.users(node).next()?;
        let later = SDValue { node: next, res: 0 };
        if self.dag.opcode(later) != ISD::Store || self.dag.operand(later, 0) != node {
            return None;
        }
        let NodeData::Mem(after) = self.dag.node(later).data else { return None };
        if after.volatile || after.ordering.is_some() || after.space != info.space {
            return None;
        }
        if self.dag.opcode(self.dag.operand(node, 2)) == ISD::Undef {
            return None;
        }
        let bytes = super::address_analysis::BaseIndexOffset::matching(self.dag, later)
            .equal_base_index(&super::address_analysis::BaseIndexOffset::matching(self.dag, node), self.dag)?;
        if bytes < 0 || bytes * 8 + i64::from(info.mem_vt.size_in_bits()) > i64::from(after.mem_vt.size_in_bits()) {
            return None;
        }
        Some(chain)
    }

    /// `visitSTORE`, `replaceStoreOfFPConstant`: `store float 1.5, p` is
    /// `store i32 0x3fc00000, p`, and a double constant the FPU cannot load
    /// directly is two `i32` stores, so the constant is never loaded onto the
    /// x87 stack to be stored. (The pin's `f16`, `f80` and `f128` stay.)
    fn visit_store(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (chain, value, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        // DEVIATE SD256: the call argument the builder cast to its bits stays
        // a float store when the load can never become an integer load
        // (`visit_bitcast`: volatile or extending): the cast alone would cost
        // a temporary.
        if matches!(info.base, super::dag::MemBase::CallArgument(_))
            && self.dag.opcode(value) == ISD::BitCast
            && self.dag.value_type(value) == EVT::I32
        {
            let source = self.dag.operand(value, 0);
            if self.dag.value_type(source) == EVT::Simple(MVT::F32)
                && self.dag.opcode(source) == ISD::Load
                && matches!(
                    self.dag.node(source).data,
                    NodeData::Mem(load) if load.volatile || load.ext.is_some()
                )
            {
                let float = MemInfo { mem_vt: EVT::Simple(MVT::F32), ..info };
                return Some(self.dag.get_store(chain, source, ptr, float));
            }
        }
        if let Some(direct) = self.hooks.store_of_converted_slot(&mut self.dag, node) {
            return Some(direct);
        }
        self.refine_alignment(node);
        // `mergeTruncStores`, before `findBetterNeighborChains` as the pin
        // has it: the chain it walks is the program's order.
        // `mergeTruncStores`: several stores of the parts of one value become
        // a store of it (with a `bswap` or a rotate where the parts are in
        // the other order).
        if let Some(wide) = self.merge_trunc_stores(node) {
            return Some(wide);
        }
        // Walk up the chain skipping the memory nodes this store does not
        // alias, on it and on the stores chained below it.
        if let Some(better) = self.find_better_neighbor_chains(node) {
            return Some(better);
        }
        if let Some(split) = self.split_pair_store(node) {
            return Some(split);
        }
        if let Some(truncating) = self.store_of_trunc(chain, value, ptr, info) {
            return Some(truncating);
        }
        // "There can be multiple store sequences on the same chain. Keep trying
        // to merge store sequences until we are unable to do so or until we
        // merge the last store on the chain": the merged stores are dead, and
        // the node is returned as handled once it is one of them.
        while self.merge_consecutive_stores(node) {
            if self.is_dead(node.node) {
                return Some(node);
            }
        }
        if let Some(done) = self.truncating_store_demanded_bits(node, chain, value, ptr, info) {
            return Some(done);
        }
        if let Some(done) = self.dead_preceding_store(node, chain, value, ptr, info) {
            return Some(done);
        }
        // `replaceStoreOfFPConstant` takes a volatile store too
        // (`isNormalStore`); the hook asks for that.
        let volatile_blocks = info.volatile && !self.hooks.store_fp_constant_as_i64();
        if self.dag.opcode(value) != ISD::ConstantFP || volatile_blocks || info.mem_vt != self.dag.value_type(value) {
            return None;
        }
        let NodeData::ConstFp { bits } = self.dag.node(value).data else { return None };
        let tli = self.tli?;
        let int_info = |mem_vt: EVT, align: u32, base| MemInfo { mem_vt, align, base, ..info };
        match self.dag.value_type(value) {
            // `(isTypeLegal(i32) && !LegalOperations && isSimple) ||
            // legalOrCustom(STORE, i32)` (DAGCombiner.cpp#L24427).
            EVT::Simple(MVT::F32)
                if (tli.types.is_type_legal(EVT::I32) && !self.legal_operations() && !info.volatile)
                    || tli.is_operation_legal_or_custom(ISD::Store, EVT::I32) =>
            {
                let constant = self.dag.get_constant(bits & 0xFFFF_FFFF, EVT::I32);
                Some(self.dag.get_store(chain, constant, ptr, int_info(EVT::I32, info.align, info.base)))
            }
            EVT::Simple(MVT::F64)
                if self.hooks.store_fp_constant_as_i64()
                    && ((tli.types.is_type_legal(EVT::I64) && !self.legal_operations() && !info.volatile)
                        || tli.is_operation_legal_or_custom(ISD::Store, EVT::I64)) =>
            {
                // `replaceStoreOfFPConstant`: "store double 1.0" is "store i64
                // 0x3FF0...".
                let constant = self.dag.get_constant(bits & 0xFFFF_FFFF_FFFF_FFFF, EVT::I64);
                Some(self.dag.get_store(chain, constant, ptr, int_info(EVT::I64, info.align, info.base)))
            }
            // `ST->isSimple() && legalOrCustom(STORE, i32) && !isFPImmLegal`
            // (DAGCombiner.cpp#L24446): a volatile store is not split.
            EVT::Simple(MVT::F64)
                if !info.volatile
                    && tli.is_operation_legal_or_custom(ISD::Store, EVT::I32)
                    && !tli.is_fp_imm_legal(MVT::F64, bits) =>
            {
                let lo = self.dag.get_constant(bits & 0xFFFF_FFFF, EVT::I32);
                let hi = self.dag.get_constant((bits >> 32) & 0xFFFF_FFFF, EVT::I32);
                // the high word comes first on a big-endian target
                let (lo, hi) = if self.hooks.little_endian() { (lo, hi) } else { (hi, lo) };
                let first = self.dag.get_store(chain, lo, ptr, int_info(EVT::I32, info.align, info.base));
                // A far pointer is no integer: its offset word moves, by 4.
                let next = if self.dag.value_type(ptr) == MVT::Far.into() {
                    self.dag.get_object_ptr_offset(ptr, 4)
                } else {
                    let four = self.dag.get_constant(4, self.dag.value_type(ptr));
                    self.dag.get_mem_base_plus_offset(ptr, four, SDNodeFlags::default())
                };
                let second = self
                    .dag
                    .get_store(chain, hi, next, int_info(EVT::I32, info.align.min(4), super::dag::MemBase::Unknown));
                Some(self.dag.get_token_factor(&[first, second]))
            }
            _ => None,
        }
    }

    /// `visitSTORE` of a truncating store of an integer: only the low bits of
    /// the value are read, so `SimplifyDemandedBits` (and
    /// `SimplifyMultipleUseDemandedBits`) may rewrite the value.
    fn truncating_store_demanded_bits(
        &mut self,
        node: SDValue,
        chain: SDValue,
        value: SDValue,
        ptr: SDValue,
        info: MemInfo,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(value);
        if !vt.is_integer()
            || !info.mem_vt.is_integer()
            || info.mem_vt.size_in_bits() >= vt.size_in_bits()
            || vt.size_in_bits() > 64
        {
            return None;
        }
        if self.dag.is_opaque(value) {
            return None;
        }
        let demanded = super::fold::mask(info.mem_vt.size_in_bits());
        self.add_to_worklist(value.node);
        if self.simplify_demanded_bits_of(value, demanded) {
            self.add_to_worklist(node.node);
            return Some(node);
        }
        if let Some(shorter) = self.simplify_multiple_use_demanded_bits(value, demanded, 0) {
            return Some(self.dag.get_store(chain, shorter, ptr, info));
        }
        None
    }

    /// `visitSTORE`'s "if this is an FP_ROUND or TRUNC followed by a store,
    /// fold this into a truncating store"
    /// ([L24881](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L24881)),
    /// with `canCombineTruncStore`: the target takes that truncating store
    /// (legal, or legal or custom before operations are legalized).
    fn store_of_trunc(
        &mut self,
        chain: SDValue,
        value: SDValue,
        ptr: SDValue,
        info: MemInfo,
    ) -> Option<SDValue> {
        if !matches!(self.dag.opcode(value), ISD::Truncate | ISD::FpRound)
            || !self.dag.has_one_use(value.node)
            || info.ordering.is_some()
        {
            return None;
        }
        let tli = self.tli?;
        let source = self.dag.operand(value, 0);
        let (EVT::Simple(wide), EVT::Simple(narrow)) = (self.dag.value_type(source), info.mem_vt) else { return None };
        // `canCombineTruncStore`: `isTruncStoreLegal` after operation legalization, `isTruncStoreLegalOrCustom`
        // before; both need the value type to be legal (a soft-float target has no `f64` to store narrowed).
        let takes = if self.legal_operations() {
            tli.is_trunc_store_legal(wide, narrow)
        } else {
            tli.is_trunc_store_legal_or_custom(wide, narrow)
        };
        takes.then(|| self.dag.get_store(chain, source, ptr, info))
    }

    /// `splitMergedValStore`, for the `BUILD_PAIR` the builder makes before
    /// legalization expands it into the `or`/`shl` the pin matches: a store of
    /// a pair is a store of each half, when the target says that is cheaper
    /// than merging the halves into one register.
    fn split_pair_store(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.opt_none {
            return None;
        }
        let (chain, value, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        if self.dag.opcode(value) != ISD::BuildPair
            || info.volatile
            || info.mem_vt != self.dag.value_type(value)
            || !self.dag.has_one_use(value.node)
        {
            return None;
        }
        let (lo, hi) = (self.dag.operand(value, 0), self.dag.operand(value, 1));
        let (lo_vt, hi_vt) = (self.dag.value_type(lo), self.dag.value_type(hi));
        if !self.tli?.is_multi_stores_cheaper_than_bits_merge(lo_vt, hi_vt) || lo_vt != hi_vt || !lo_vt.is_integer() {
            return None;
        }
        let half = lo_vt.size_in_bits() / 8;
        let part = |mem_vt, align, base| MemInfo { mem_vt, align, base, ..info };
        let first = self.dag.get_store(chain, lo, ptr, part(lo_vt, info.align, info.base));
        let next = if self.dag.value_type(ptr) == MVT::Far.into() {
            self.dag.get_object_ptr_offset(ptr, i64::from(half))
        } else {
            let offset = self.dag.get_constant(u128::from(half), self.dag.value_type(ptr));
            self.dag.get_mem_base_plus_offset(ptr, offset, SDNodeFlags::default())
        };
        Some(self.dag.get_store(first, hi, next, part(hi_vt, info.align.min(half), super::dag::MemBase::Unknown)))
    }

    /// `visitMUL`, up to the multiply-by-power-of-two folds.
    fn visit_mul(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(n0);
        if self.dag.is_undef(n0) || self.dag.is_undef(n1) {
            return Some(self.dag.get_constant(0, vt));
        }
        let is_const = |dag: &SelectionDAG, v: SDValue| dag.constant(v).is_some() && !dag.is_opaque(v);
        if is_const(self.dag, n0) && !is_const(self.dag, n1) {
            return Some(self.dag.get_node(ISD::Mul, vt, &[n1, n0]));
        }
        let c1 = self.dag.constant(n1);
        let opaque = self.dag.is_opaque(n1);
        let width = vt.size_in_bits();
        let ones = super::fold::mask(width);
        match c1 {
            Some(0) => return Some(n1),
            Some(1) => return Some(n0),
            Some(c) if c == ones => return Some(self.dag.get_negative(n0, vt)),
            _ => {}
        }
        if let (Some(c), false) = (c1, opaque) {
            if c.is_power_of_two() {
                let amount = self.shift_amount(vt, c.trailing_zeros());
                return Some(self.dag.get_node(ISD::Shl, vt, &[n0, amount]));
            }
            // A negated power of two: `0 - (x << log2(-c))`.
            let negated = c.wrapping_neg() & ones;
            if negated.is_power_of_two() && negated != c {
                let amount = self.shift_amount(vt, negated.trailing_zeros());
                let shifted = self.dag.get_node(ISD::Shl, vt, &[n0, amount]);
                let zero = self.dag.get_constant(0, vt);
                return Some(self.dag.get_node(ISD::Sub, vt, &[zero, shifted]));
            }
        }
        // x * c with x and c in the half: the half multiply, before a chain or the full multiply.
        if let (Some(c), false) = (c1, opaque) {
            if let Some(half) = self.hooks.mul_of_halves(vt).filter(|half| half.constant) {
                if let Some(product) = self.half_multiply_by_constant(n0, c, vt, half) {
                    return Some(product);
                }
            }
        }
        // `(mul x, 2^n +- 1)` as `(add/sub (shl x, n), x)`, and with a trailing
        // power of two as a shift of both ([pin][pin]).
        //
        // [pin]:
        // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5024
        if let (Some(c), false) = (c1, opaque) {
            if self.hooks.decompose_mul_by_constant_of(self.dag, n0, vt, c) {
                if let Some(decomposed) = self.decompose_mul(n0, vt, c) {
                    return Some(decomposed);
                }
            }
            if let Some(plan) = self.hooks.synth_mul_chain(vt, c) {
                return Some(self.build_mul_plan(&plan, n0, vt));
            }
        }
        // `(mul (shl x, c1), c2)` is `(mul x, c2 << c1)`.
        if self.dag.opcode(n0) == ISD::Shl {
            let (x, c1_shift) = (self.dag.operand(n0, 0), self.dag.operand(n0, 1));
            if let (Some(shift), Some(c)) = (self.dag.constant(c1_shift), c1) {
                if shift < u128::from(width) {
                    let folded = self.dag.get_constant((c << shift) & ones, vt);
                    return Some(self.dag.get_node(ISD::Mul, vt, &[x, folded]));
                }
            }
        }
        // `(mul (add x, c1), c2)` is `(add (mul x, c2), c1*c2)`.
        if self.hooks.fold_mul_of_add_with_constant() && self.dag.opcode(n0) == ISD::Add && !opaque {
            if let (Some(c2), Some(c1)) = (c1, self.dag.constant(self.dag.operand(n0, 1))) {
                if self.mul_add_with_const_profitable(node, n0, vt, c1, c2) {
                    let x = self.dag.operand(n0, 0);
                    let product = self.dag.get_node(ISD::Mul, vt, &[x, n1]);
                    let folded = self.dag.get_constant(c1.wrapping_mul(c2) & ones, vt);
                    return Some(self.dag.get_node(ISD::Add, vt, &[product, folded]));
                }
            }
        }
        if let Some(half) = self.hooks.mul_of_halves(vt) {
            if let Some(product) = self.mul_of_halves(n0, n1, vt, half) {
                return Some(product);
            }
        }
        self.reassociate_ops(ISD::Mul, n0, n1, self.dag.node(node).flags)
    }

    /// The nodes of a [`MulPlan`] over the multiplicand `x`.
    fn build_mul_plan(
        &mut self,
        plan: &MulPlan,
        x: SDValue,
        vt: EVT,
    ) -> SDValue {
        match plan {
            MulPlan::Multiplicand => x,
            MulPlan::Shl(inner, by) => {
                let inner = self.build_mul_plan(inner, x, vt);
                let amount = self.shift_amount(vt, *by);
                self.dag.get_node(ISD::Shl, vt, &[inner, amount])
            }
            MulPlan::Add(a, b) => {
                let (a, b) = (self.build_mul_plan(a, x, vt), self.build_mul_plan(b, x, vt));
                self.dag.get_node(ISD::Add, vt, &[a, b])
            }
            MulPlan::Sub(a, b) => {
                let (a, b) = (self.build_mul_plan(a, x, vt), self.build_mul_plan(b, x, vt));
                self.dag.get_node(ISD::Sub, vt, &[a, b])
            }
            MulPlan::Neg(inner) => {
                let inner = self.build_mul_plan(inner, x, vt);
                self.dag.get_negative(inner, vt)
            }
        }
    }

    /// `x * c` as the half multiply with `c` as the immediate, when `x` fits
    /// the half unsigned and `c` does, or both fit it signed, and the target
    /// prices it under the alternatives.
    fn half_multiply_by_constant(
        &mut self,
        x: SDValue,
        c: u128,
        vt: EVT,
        half: HalfMul,
    ) -> Option<SDValue> {
        let (width, narrow) = (vt.size_in_bits(), half.half.size_in_bits());
        let high = super::fold::mask(width) & !super::fold::mask(narrow);
        let signed = if super::known_bits::masked_value_is_zero(self.dag, x, high) && c & high == 0 {
            false
        } else if super::known_bits::num_sign_bits(self.dag, x) > width - narrow && matches!(c >> (narrow - 1), 0) | (c >> (narrow - 1) == super::fold::mask(width) >> (narrow - 1)) {
            true
        } else {
            return None;
        };
        if !self.hooks.half_multiply_by_constant(vt, c, signed) {
            return None;
        }
        let immediate = self.dag.get_constant(c & super::fold::mask(narrow), half.half);
        let opcode = if signed { half.signed } else { half.unsigned };
        Some(self.dag.get_node(ISD::Machine(opcode), vt, &[x, immediate]))
    }

    /// The half-by-half multiply of `x * y`, where both are known to fit the
    /// half type unsigned (high bits zero) or both signed (sign bits above
    /// the half's width). A constant operand is left to the pin's folds.
    fn mul_of_halves(
        &mut self,
        x: SDValue,
        y: SDValue,
        vt: EVT,
        half: HalfMul,
    ) -> Option<SDValue> {
        if self.dag.constant(x).is_some() || self.dag.constant(y).is_some() {
            return None;
        }
        // an extension of a half-width value, or a load that extends one
        let extends_half = |dag: &SelectionDAG, v: SDValue| {
            if dag.opcode(v) == ISD::Load {
                return matches!(
                    dag.node(v).data,
                    NodeData::Mem(info) if info.ext.is_some() && info.mem_vt == half.half
                );
            }
            matches!(
                dag.opcode(v),
                ISD::SignExtend | ISD::ZeroExtend | ISD::AnyExtend
            )
                && dag.value_type(dag.operand(v, 0)) == half.half
        };
        if half.has_full_multiply && (extends_half(self.dag, x) || extends_half(self.dag, y)) {
            return None;
        }
        let (width, narrow) = (vt.size_in_bits(), half.half.size_in_bits());
        let high = super::fold::mask(width) & !super::fold::mask(narrow);
        let opcode = if [x, y].iter().all(|&v| super::known_bits::masked_value_is_zero(self.dag, v, high)) {
            half.unsigned
        } else if [x, y].iter().all(|&v| super::known_bits::num_sign_bits(self.dag, v) > width - narrow) {
            half.signed
        } else {
            return None;
        };
        let low = self.dag.get_node(ISD::Truncate, half.half, &[y]);
        Some(self.dag.get_node(ISD::Machine(opcode), vt, &[x, low]))
    }

    /// The shifts and the add or subtract of `x * c` for `c = +-(2^n +- 1) <<
    /// k`, or `None` for any other constant.
    fn decompose_mul(
        &mut self,
        x: SDValue,
        vt: EVT,
        c: u128,
    ) -> Option<SDValue> {
        let width = vt.size_in_bits();
        let ones = super::fold::mask(width);
        let negative = c >> (width - 1) & 1 == 1;
        let mut magnitude = if negative { c.wrapping_neg() & ones } else { c };
        // the constant `2` is `2^0 + 1`
        let zeros = if magnitude == 2 { 0 } else { magnitude.trailing_zeros() };
        magnitude >>= zeros;
        let (add, bits) = if (magnitude - 1).is_power_of_two() {
            (true, magnitude - 1)
        } else if (magnitude + 1).is_power_of_two() {
            (false, magnitude + 1)
        } else {
            return None;
        };
        if zeros > 0 && self.hooks.factor_trailing_zeros_of_mul() && bits.trailing_zeros() + zeros < width {
            let amount = self.shift_amount(vt, bits.trailing_zeros());
            let shifted = self.dag.get_node(ISD::Shl, vt, &[x, amount]);
            let inner = self.dag.get_node(if add { ISD::Add } else { ISD::Sub }, vt, &[shifted, x]);
            let by = self.shift_amount(vt, zeros);
            let result = self.dag.get_node(ISD::Shl, vt, &[inner, by]);
            return Some(if negative { self.dag.get_negative(result, vt) } else { result });
        }
        let amount = bits.trailing_zeros() + zeros;
        if amount >= width {
            return None;
        }
        let high_amount = self.shift_amount(vt, amount);
        let high = self.dag.get_node(ISD::Shl, vt, &[x, high_amount]);
        let low = if zeros == 0 {
            x
        } else {
            let low_amount = self.shift_amount(vt, zeros);
            self.dag.get_node(ISD::Shl, vt, &[x, low_amount])
        };
        let result = self.dag.get_node(if add { ISD::Add } else { ISD::Sub }, vt, &[high, low]);
        Some(if negative { self.dag.get_negative(result, vt) } else { result })
    }

    /// `isMulAddWithConstProfitable`: a one-use add the target accepts, or
    /// another multiply by the same constant that shares `x * c2` or will
    /// after its own fold.
    fn mul_add_with_const_profitable(
        &self,
        mul: SDValue,
        add: SDValue,
        vt: EVT,
        c1: u128,
        c2: u128,
    ) -> bool {
        if self.dag.has_one_use(add.node) && self.hooks.is_mul_add_with_const_profitable(vt, c1, c2) {
            return true;
        }
        let constant = self.dag.operand(mul, 1);
        let var = self.dag.operand(add, 0);
        let users: Vec<u32> = self.dag.users(constant).collect();
        users
            .into_iter()
            .filter(|&user| user != mul.node)
            .any(
                |user| {
                    let user = SDValue { node: user, res: 0 };
                    if self.dag.opcode(user) != ISD::Mul {
                        return false;
                    }
                    let other = if self.dag.operand(user, 0) == constant {
                        self.dag.operand(user, 1)
                    } else {
                        self.dag.operand(user, 0)
                    };
                    other == var
                        || (self.dag.opcode(other) == ISD::Add
                            && self.dag.constant(self.dag.operand(other, 1)).is_some()
                            && self.dag.operand(other, 0) == var)
                },
            )
    }
    /// `visitSIGN_EXTEND_INREG` ([pin][pin]): an operand that is already
    /// sign-extended, nested in-regs, a `sext`/`aext`/`zext` of a value no
    /// wider than the type, a clear sign bit; then the demanded bits.
    /// SD230: not the vector in-reg forms, nor `reduceLoadWidth`.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L17400
    fn visit_sext_in_reg(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (n0, n1) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let super::dag::NodeData::ValueType(ext) = self.dag.node(n1).data else { return None };
        let (bits, ext_bits) = (vt.size_in_bits(), ext.size_in_bits());
        if self.dag.is_undef(n0) {
            return Some(self.dag.get_constant(0, vt));
        }
        // already sign extended
        if bits - super::known_bits::num_sign_bits(self.dag, n0) + 1 <= ext_bits {
            return Some(n0);
        }
        if self.dag.opcode(n0) == ISD::SignExtendInReg {
            if let super::dag::NodeData::ValueType(inner) = self.dag.node(self.dag.operand(n0, 1)).data {
                if ext_bits < inner.size_in_bits() {
                    let x = self.dag.operand(n0, 0);
                    return Some(self.dag.get_node(ISD::SignExtendInReg, vt, &[x, n1]));
                }
            }
        }
        let sext_legal = !self.legal_operations() || self.fp_op_legal(ISD::SignExtend, vt);
        let n00 = match self.dag.opcode(n0) {
            ISD::SignExtend | ISD::AnyExtend | ISD::ZeroExtend => self.dag.operand(n0, 0),
            _ => n0,
        };
        match self.dag.opcode(n0) {
            ISD::SignExtend | ISD::AnyExtend if sext_legal => {
                let small = self.dag.value_type(n00).size_in_bits();
                if small <= ext_bits
                    || self.dag.value_type(n00).size_in_bits() - super::known_bits::num_sign_bits(self.dag, n00) + 1
                        <= ext_bits
                {
                    return Some(self.dag.get_node(ISD::SignExtend, vt, &[n00]));
                }
            }
            ISD::ZeroExtend if sext_legal && self.dag.value_type(n00).size_in_bits() == ext_bits => {
                return Some(self.dag.get_node(ISD::SignExtend, vt, &[n00]));
            }
            _ => {}
        }
        // the sign bit is known clear: a zero-extend in register
        let sign = 1u128 << (ext_bits - 1);
        if super::known_bits::known_bits(self.dag, n0).zero & sign != 0 {
            let mask = self.dag.get_constant(super::fold::mask(ext_bits), vt);
            return Some(self.dag.get_node(ISD::And, vt, &[n0, mask]));
        }
        if self.hooks.fold_sext_in_reg_of_load() {
            if let Some(narrow) = self.reduce_load_width_of(node) {
                return Some(narrow);
            }
        }
        if let Some(simplified) = self.simplify_demanded(node) {
            return Some(simplified);
        }
        // `(sext_in_reg (srl X, c), i8)` is `(sra X, c)` when X has sign bits to
        // spare (DAGCombiner.cpp#L17478): the shifted-in zeros are all sign.
        if self.dag.opcode(n0) == ISD::Srl
            && let Some(amount) = self.dag.constant(self.dag.operand(n0, 1))
            && amount <= u128::from(bits - ext_bits)
        {
            let x = self.dag.operand(n0, 0);
            if u128::from(bits - ext_bits) - amount < u128::from(super::known_bits::num_sign_bits(self.dag, x)) {
                return Some(self.dag.get_node(ISD::Sra, vt, &[x, self.dag.operand(n0, 1)]));
            }
        }
        self.sext_in_reg_of_load(node, n0, vt, ext)
    }

    /// `visitAND`: `(zext_inreg (extload x))` is `(zextload x)`, for every
    /// user of the load, and `(zext_inreg (sextload x))` when the `and` is its
    /// one user. Before legalization, or where the zero-extending load is
    /// legal.
    fn and_of_extload(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (load, mask) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        if !self.hooks.fold_and_of_extload() || self.dag.opcode(load) != ISD::Load || load.res != 0 {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        let single_use = self.dag.has_one_use(load.node);
        if self.dag.node(load).values.len() > 2
            || !matches!(info.ext, Some(LoadExt::ExtLoad) | Some(LoadExt::SExtLoad))
            || info.ext == Some(LoadExt::SExtLoad) && !single_use
        {
            return None;
        }
        let (wide, mem) = (self.dag.value_type(node), info.mem_vt);
        let high = super::fold::mask(wide.size_in_bits()) & !super::fold::mask(mem.size_in_bits());
        if super::known_bits::known_bits(self.dag, mask).zero & high != high {
            return None;
        }
        let legal = match (wide, mem) {
            (EVT::Simple(wide), EVT::Simple(mem)) => {
                self.tli.is_some_and(|tli| tli.load_ext_action(LoadExt::ZExtLoad, wide, mem) == Action::Legal)
            }
            _ => false,
        };
        if !(self.legal_operations() && legal || !self.legal_operations() && !info.volatile) {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let extended = self.dag.get_ext_load(LoadExt::ZExtLoad, wide, chain, ptr, MemInfo { ext: None, ..info });
        self.add_to_worklist(node.node);
        self.replace_value(load, extended);
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        Some(node)
    }

    /// `visitSIGN_EXTEND_INREG`: `(sext_inreg (extload x))` and
    /// `(sext_inreg (zextload x))` are `(sextload x)`.
    fn sext_in_reg_of_load(
        &mut self,
        node: SDValue,
        load: SDValue,
        vt: EVT,
        ext: EVT,
    ) -> Option<SDValue> {
        if !self.hooks.fold_sext_in_reg_of_load() || self.dag.opcode(load) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(load).data else { return None };
        if info.mem_vt != ext {
            return None;
        }
        let legal = match (vt, ext) {
            (EVT::Simple(wide), EVT::Simple(mem)) => {
                self.tli.is_some_and(|tli| tli.load_ext_action(LoadExt::SExtLoad, wide, mem) == Action::Legal)
            }
            _ => false,
        };
        let one_use = self.dag.has_one_use(load.node);
        let fold = match info.ext {
            Some(LoadExt::ExtLoad) => (!self.legal_operations() && !info.volatile && one_use) || legal,
            Some(LoadExt::ZExtLoad) => one_use && !self.legal_operations() && !info.volatile && legal,
            _ => false,
        };
        if !fold {
            return None;
        }
        let (chain, ptr) = (self.dag.operand(load, 0), self.dag.operand(load, 1));
        let was_extload = info.ext == Some(LoadExt::ExtLoad);
        let info = super::dag::MemInfo { ext: None, ..info };
        let extended = self.dag.get_ext_load(LoadExt::SExtLoad, vt, chain, ptr, info);
        self.replace_value(SDValue { node: load.node, res: 1 }, SDValue { node: extended.node, res: 1 });
        // The pin's `CombineTo(N0, ExtLoad)`: the extending load's other users
        // read the new load (the bits above its memory type are undefined).
        if was_extload {
            self.replace_value(load, extended);
        }
        let _ = node;
        Some(extended)
    }
}

/// One run of the combiner at `level` (`SelectionDAG::Combine`).
pub fn combine<H: CombineHooks>(
    dag: &mut SelectionDAG,
    hooks: &mut H,
    level: CombineLevel,
) {
    DagCombiner::new(dag, hooks, level).run();
}
