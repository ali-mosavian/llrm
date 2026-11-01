//! The Z80's cost model: what an instruction costs in byte-wide operations.
//!
//! The unit is one byte-wide ALU operation (`clc; lda; adc; sta`: 7 bytes, 11
//! clocks). A type is `ceil(bits / native)` of them, the native width read from
//! the data layout (`n8`) as `BasicTTIImplBase::getTypeLegalizationCost` splits
//! a type until it is legal.
//!
//! DEVIATE: `BasicTTIImplBase` scales only `RecipThroughput` and prices an
//! expanded scalar at 1; here every kind scales (as `X86TTIImpl` and
//! `AArch64TTIImpl` do) and a multiply, divide or variable shift is the call it
//! emits. Reason: `SimplifyCFG`'s speculation budget asks `SizeAndLatency`, and
//! a 16-bit `add` or `icmp` is two byte operations here, not one. Exit: none;
//! the numbers below are the target's.
//!
//! Derived by hand from the emitted code (`llrm-cc --target=Z80-sim65 -O2
//! -S`), unit = i8 add:
//!
//! | operation | bytes, clocks | units |
//! |---|---|---|
//! | add/sub/and/or/xor, i8 / i16 | 7 / 13, 11 / 22 | 1 / 2 |
//! | icmp, i8 / i16 (`lda; cmp` / `lda; cmp; lda; sbc`) | 4 / 8 | 1 / 2 |
//! | select, i8 / i16 (`lda #1; bcc; lda #0` per byte) | 7 / 13 | parts + 1 |
//! | load or store through a pointer, i8 / i16 (`ldy #n; lda (rc),y` per byte, one copy) | 4 / 13, 7 / 22 | 1 / 2 (measured 1 / 3) |
//! | shift by a constant (`asl; rol` pairs, per bit) | 4 per bit at i16 | parts |
//! | multiply by a constant (shifts and adds), i16 | 28, 70 | 3 * parts |
//! | `__mulqi3`, `__mulhi3` (8 / 16 iterations of about 25 / 45 clocks) | 200, 700 | 10 * parts^2 + 10 |
//! | `__udivhi3` (16 iterations of about 60 clocks) | 960 | 22 * parts^2 + 3 |
//! | `__ashlhi3`, `__lshrhi3`, `__ashrhi3` (a loop of about 16 clocks per bit, 8 on average) | 130 | 6 * parts |
//! | a call as code (`jsr`, two operands in `rc2..`) | 3 + 4 * parts | 2 + parts |
//!
//! A memory operation is `parts`, as `BasicTTIImplBase::getMemoryOpCost` prices
//! it, not the measured 2 * parts - 1: with 3 for a word load, `SimplifyCFG`'s
//! two-entry-phi budget (4) refused to flatten `c >= '0' && c <= '9'`
//! (compare 2 + load 3), and `InstCombine` never saw its range check (isdigit
//! 28 -> 50 bytes, isalpha 30 -> 74). The speed-priority form of such a limit
//! belongs in the per-target override table.
//!
//! A trunc is free (a register is a byte); a zero extension is the new high
//! bytes cleared, a sign extension twice that.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::tti::*;
use llrm_target::Target as _;

use crate::target::Z80;

/// The cost model of the Z80 and Z180.
#[derive(Clone, Copy, Debug)]
pub struct Z80Tti {
    /// The native integer, in bits (`n8`).
    byte_bits: u32,
    /// The pointer, in bits (`p:16`).
    pointer_bits: u32,
}

impl Z80Tti {
    pub fn new() -> Z80Tti {
        let layout = DataLayout::parse(&Z80.layout().datalayout).expect("the Z80's datalayout parses");
        Z80Tti { byte_bits: layout.largest_legal_integer(), pointer_bits: layout.pointer(0).bits }
    }

    /// The byte-wide operations a value of `ty` takes.
    fn parts(
        &self,
        ty: Ty,
    ) -> i64 {
        let bits = match ty {
            Ty::Int(bits) => bits,
            Ty::Pointer => self.pointer_bits,
            Ty::Float => 32,
            Ty::Double => 64,
            Ty::X87 => 80,
        };
        i64::from(bits.div_ceil(self.byte_bits).max(1))
    }

    /// A call to a runtime routine whose loop costs `clocks` units.
    fn call(
        &self,
        parts: i64,
        kind: CostKind,
        clocks: i64,
    ) -> Cost {
        match kind {
            CostKind::CodeSize | CostKind::SizeAndLatency => Cost::of(2 + parts),
            CostKind::RecipThroughput | CostKind::Latency => Cost::of(clocks),
        }
    }
}

impl Default for Z80Tti {
    fn default() -> Z80Tti {
        Z80Tti::new()
    }
}

impl Tti for Z80Tti {
    fn name(&self) -> &'static str {
        "z80"
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        let parts = self.parts(ty);
        match op {
            CmpSel::ICmp => Cost::of(parts),
            CmpSel::Select => Cost::of(parts + 1),
            // soft float: a call
            CmpSel::FCmp => self.call(parts, kind, 10 * parts * parts + 10),
        }
    }

    fn memory_op_cost(
        &self,
        _: MemoryOp,
        ty: Ty,
        _: CostKind,
        _: bool,
    ) -> Cost {
        Cost::of(self.parts(ty))
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
        _: i128,
        bits: u32,
    ) -> Cost {
        Cost::of(i64::from(bits.div_ceil(self.byte_bits).max(1)))
    }

    fn int_imm_cost_inst(
        &self,
        op: ImmUser,
        index: u32,
        imm: i128,
        bits: u32,
    ) -> Cost {
        Basic.int_imm_cost_inst(op, index, imm, bits)
    }

    fn is_truncate_free(
        &self,
        from: Ty,
        to: Ty,
    ) -> bool {
        matches!((from, to), (Ty::Int(from), Ty::Int(to)) if to < from)
    }

    /// What the selector folds into one access: a register alone (`ld a,(hl)`)
    /// or an absolute address with its offset (`ld a,(nn)`). A register
    /// plus a displacement is `(ix+d)`, which is not selected yet, and the Z80
    /// has no register-plus-register or scaled mode. The pin's default
    /// always accepts `r`, so LSR's initial formula is legal; `Basic`
    /// accepts nothing and LSR's `Formula is illegal` assertion fires.
    fn is_legal_addressing_mode(
        &self,
        gv: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        if !matches!(scale, 0 | 1) {
            return false;
        }
        match usize::from(base) + usize::from(scale == 1) {
            0 => true,
            1 => !gv && offset == 0,
            _ => false,
        }
    }

    // The runtime's unsigned divide leaves the remainder behind (`rc6:rc7`), so
    // a pair is one call.
    fn has_div_rem_op(
        &self,
        bits: u32,
        signed: bool,
    ) -> bool {
        !signed && bits <= 2 * self.byte_bits
    }

    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        let (to, from) = (self.parts(dst), self.parts(src));
        match op {
            CastKind::Trunc | CastKind::BitCast => Cost::FREE,
            CastKind::ZExt | CastKind::PtrToInt | CastKind::IntToPtr => Cost::of((to - from).max(0)),
            CastKind::SExt => Cost::of(2 * (to - from).max(0)),
            // soft float: a call
            _ => self.call(to.max(from), kind, 10 * to.max(from).pow(2) + 10),
        }
    }

    fn arithmetic_cost(
        &self,
        op: ArithOp,
        ty: Ty,
        kind: CostKind,
        first: OperandInfo,
        second: OperandInfo,
    ) -> Cost {
        use ArithOp::*;
        let parts = self.parts(ty);
        let constant = first.constant || second.constant;
        match op {
            Add | Sub | And | Or | Xor | FNeg => Cost::of(parts),
            Shl | LShr | AShr if second.constant => Cost::of(parts),
            Shl | LShr | AShr => self.call(parts, kind, 6 * parts),
            Mul if second.power_of_two || first.power_of_two => Cost::of(parts),
            Mul if constant => Cost::of(3 * parts),
            Mul => self.call(parts, kind, 10 * parts * parts + 10),
            UDiv if second.power_of_two => Cost::of(parts),
            URem if second.power_of_two => Cost::of(parts),
            UDiv | URem | SDiv | SRem => self.call(parts, kind, 22 * parts * parts + 3),
            // soft float: a call
            FAdd | FSub | FMul | FDiv | FRem => self.call(parts, kind, 10 * parts * parts + 10),
        }
    }
}
