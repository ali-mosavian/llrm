//! The 6502's cost model: what an instruction costs in byte-wide operations.
//!
//! The unit is one byte-wide ALU operation (`clc; lda; adc; sta`: 7 bytes, 11 clocks). A type is
//! `ceil(bits / native)` of them, the native width read from the data layout (`n8`) as
//! `BasicTTIImplBase::getTypeLegalizationCost` splits a type until it is legal.
//!
//! DEVIATE: `BasicTTIImplBase` scales only `RecipThroughput` and prices an expanded scalar at 1; here every
//! kind scales (as `X86TTIImpl` and `AArch64TTIImpl` do) and a multiply, divide or variable shift is the call
//! it emits. Reason: `SimplifyCFG`'s speculation budget asks `SizeAndLatency`, and a 16-bit `add` or `icmp` is
//! two byte operations here, not one. Exit: none; the numbers below are the target's.
//!
//! Derived by hand from the emitted code (`llrm-cc --target=6502-sim65 -O2 -S`), unit = i8 add:
//!
//! | operation | bytes, clocks | units |
//! |---|---|---|
//! | add/sub/and/or/xor, i8 / i16 | 7 / 13, 11 / 22 | 1 / 2 |
//! | icmp, i8 / i16 (`lda; cmp` / `lda; cmp; lda; sbc`) | 4 / 8 | 1 / 1 (DEVIATE below) |
//! | select, i8 / i16 (`lda #1; bcc; lda #0` per byte) | 7 / 13 | parts + 1 |
//! | load or store through a pointer, i8 / i16 (`ldy #n; lda (rc),y` per byte, one copy) | 4 / 13, 7 / 22 | 1 / 3 |
//! | shift by a constant (`asl; rol` pairs, per bit) | 4 per bit at i16 | parts |
//! | multiply by a constant (shifts and adds), i16 | 28, 70 | 3 * parts |
//! | `__mulqi3`, `__mulhi3` (8 / 16 iterations of about 25 / 45 clocks) | 200, 700 | 10 * parts^2 + 10 |
//! | `__udivhi3` (16 iterations of about 60 clocks) | 960 | 22 * parts^2 + 3 |
//! | `__ashlhi3`, `__lshrhi3`, `__ashrhi3` (a loop of about 16 clocks per bit, 8 on average) | 130 | 6 * parts |
//! | a call as code (`jsr`, two operands in `rc2..`) | 3 + 4 * parts | 2 + parts |
//!
//! DEVIATE: an `icmp` is 1 at every width, not 1 per byte. Reason: at 2 for a word, `SimplifyCFG` left the two
//! compares of `c >= '0' && c <= '9'` as branches and `InstCombine` never saw the range check
//! (hand corpus: isdigit 28 -> 50 bytes, isalpha 30 -> 74). Exit: the cause card for that ordering.
//!
//! A trunc is free (a register is a byte); a zero extension is the new high bytes cleared, a sign extension
//! twice that.

use llrm_mir::datalayout::DataLayout;
use llrm_target::Target as _;
use llrm_mir::tti::*;

use crate::target::Mos;

/// The cost model of the 6502 and 65C02.
#[derive(Clone, Copy, Debug)]
pub struct Mos6502 {
    /// The native integer, in bits (`n8`).
    byte_bits: u32,
    /// The pointer, in bits (`p:16`).
    pointer_bits: u32,
}

impl Mos6502 {
    pub fn new() -> Mos6502 {
        let layout = DataLayout::parse(&Mos.layout().datalayout).expect("the 6502's datalayout parses");
        Mos6502 { byte_bits: layout.largest_legal_integer(), pointer_bits: layout.pointer(0).bits }
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

impl Default for Mos6502 {
    fn default() -> Mos6502 {
        Mos6502::new()
    }
}

impl Tti for Mos6502 {
    fn name(&self) -> &'static str {
        "mos6502"
    }

    fn cmp_sel_cost(
        &self,
        op: CmpSel,
        ty: Ty,
        kind: CostKind,
    ) -> Cost {
        let parts = self.parts(ty);
        match op {
            CmpSel::ICmp => Cost::of(1),
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
        Cost::of(2 * self.parts(ty) - 1)
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

    fn is_legal_addressing_mode(
        &self,
        gv: bool,
        offset: i64,
        base: bool,
        scale: i64,
    ) -> bool {
        Basic.is_legal_addressing_mode(gv, offset, base, scale)
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
