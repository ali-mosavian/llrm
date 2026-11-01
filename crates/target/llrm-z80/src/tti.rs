//! The Z80's cost model, summed from the instruction table (`timing/z80.csv`).
//!
//! Every price is the sum of the forms an operation expands to ([`sum`]): bytes
//! for `CodeSize`, T-states for the other kinds, in units of a cheap
//! instruction (4 T, `ld r,r`), so that `SimplifyCFG`'s speculation budget and
//! the inliner's thresholds, which count in `TCC_Basic`, mean what they mean
//! for the pin.
//!
//! DEVIATE: `BasicTTIImplBase` scales only `RecipThroughput`; here `Latency`
//! and `SizeAndLatency` scale too (as `X86TTIImpl` and `AArch64TTIImpl` do).
//! Reason: a 16-bit `add` is one instruction and an `and` six, which the
//! `Basic` unit price hides. Exit: none; the table is the target's.
//!
//! DEVIATE: the runtime routines (`__mulhi3`, `__udivhi3`, `__ashlhi3`, ...)
//! are not written, so a call is priced as the restoring loop SDCC's measured
//! `mul8` (385 T) agrees with, per bit. Reason: the cost model runs before
//! the runtime exists. Exit: price from the routines' own T-states once
//! `runtime/` has them (by 2026-11-06).
//!
//! DEVIATE: soft float is priced as a multiply-sized call. Reason: no
//! soft-float runtime. Exit: the same.
//!
//! The expansions (operands already in the registers the instruction wants;
//! placing them is the allocator's):
//!
//! | operation | forms |
//! |---|---|
//! | add, sub, and, or, xor, i8 | `add a,r` |
//! | add, i16 | `add hl,rr` |
//! | sub, i16 | `or a; sbc hl,rr` |
//! | and, or, xor, i16 | `ld a,l; and c; ld l,a; ld a,h; and b; ld h,a` |
//! | icmp, i8 / i16 | `cp r` / `push hl; or a; sbc hl,rr; pop hl` |
//! | select, i8 / i16 | `jr nz,e; ld r,r'` per byte |
//! | load, store, i8 / i16 | `ld r,(hl)` / `ld a,(hl); inc hl; ld h,(hl); ld l,a` and its store |
//! | shift by a constant, per bit, i8 / i16 | `sla r` / `add hl,hl` or `srl h; rr l` |
//! | zext, sext i8 to i16 | `ld h,n` / `ld a,l; rla; sbc a,a; ld h,a` |

use llrm_mir::datalayout::DataLayout;
use llrm_mir::tti::*;
use llrm_target::Target as _;

use crate::target::Z80;
use crate::timing::cost;

/// The bytes and T-states of `forms`, each taken on its taken path.
fn sum(forms: &[&str]) -> (i64, i64) {
    forms
        .iter()
        .fold(
            (0, 0),
            |(bytes, clocks), form| {
                let one = cost(form).unwrap_or_else(|| panic!("timing/z80.csv has no form `{form}`"));
                (bytes + i64::from(one.bytes), clocks + i64::from(one.taken))
            },
        )
}

/// `forms` in the unit of `kind`.
fn priced(
    forms: &[&str],
    kind: CostKind,
) -> Cost {
    let (bytes, clocks) = sum(forms);
    Cost::of(match kind {
        CostKind::CodeSize => bytes,
        _ => (clocks + 3) / 4,
    })
}

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

    /// The legal-sized pieces a value of `ty` splits into: bytes for an `i8`,
    /// words for the rest.
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
        i64::from(bits.div_ceil(16).max(1))
    }

    fn is_byte(
        &self,
        ty: Ty,
    ) -> bool {
        matches!(ty, Ty::Int(bits) if bits <= self.byte_bits.min(8))
    }

    /// A call of a runtime loop of `bits` passes, each the forms `body`, plus
    /// the call and the return.
    fn call(
        &self,
        parts: i64,
        bits: i64,
        body: &[&str],
        kind: CostKind,
    ) -> Cost {
        let (bytes, clocks) = sum(body);
        let (call_bytes, call_clocks) = sum(&["call nn", "ret"]);
        match kind {
            CostKind::CodeSize => Cost::of(call_bytes + 2 * parts),
            _ => Cost::of((call_clocks + clocks * bits + bytes) / 4 * parts),
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
            CmpSel::ICmp if self.is_byte(ty) => priced(&["cp b"], kind),
            CmpSel::ICmp => priced(&["push bc", "or b", "sbc hl;bc", "pop bc"], kind) * parts.min(2),
            CmpSel::Select if self.is_byte(ty) => priced(&["jr nz;e", "ld b;c"], kind),
            CmpSel::Select => priced(&["jr nz;e", "ld b;c", "ld b;c"], kind) * parts,
            // soft float: a call
            CmpSel::FCmp => self.call(parts, 8 * parts, &["add hl;bc", "djnz e"], kind),
        }
    }

    fn memory_op_cost(
        &self,
        op: MemoryOp,
        ty: Ty,
        kind: CostKind,
        _: bool,
    ) -> Cost {
        let load = op == MemoryOp::Load;
        if self.is_byte(ty) {
            return priced(&[if load { "ld b;(hl)" } else { "ld (hl);b" }], kind);
        }
        let word = if load {
            ["ld b;(hl)", "inc hl", "ld b;(hl)", "ld b;c"]
        } else {
            ["ld (hl);b", "inc hl", "ld (hl);b", "inc hl"]
        };
        priced(&word, kind) * self.parts(ty)
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
        Cost::of(i64::from(bits.div_ceil(16).max(1)))
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

    // The runtime has no routine that leaves both quotient and remainder: a
    // pair is two calls.
    fn has_div_rem_op(
        &self,
        _bits: u32,
        _signed: bool,
    ) -> bool {
        false
    }

    fn cast_cost(
        &self,
        op: CastKind,
        dst: Ty,
        src: Ty,
        kind: CostKind,
    ) -> Cost {
        let (to, from) = (self.parts(dst), self.parts(src));
        let widen = self.is_byte(src) && !self.is_byte(dst);
        match op {
            CastKind::Trunc | CastKind::BitCast => Cost::FREE,
            CastKind::ZExt | CastKind::PtrToInt | CastKind::IntToPtr if widen => {
                priced(&["ld b;n"], kind) * (to - from + 1).max(1)
            }
            CastKind::ZExt | CastKind::PtrToInt | CastKind::IntToPtr => priced(&["ld b;n"], kind) * (to - from).max(0),
            CastKind::SExt if widen => priced(&["ld b;c", "rla", "sbc a;b", "ld b;c"], kind),
            CastKind::SExt => priced(&["rla", "sbc a;b", "ld b;c"], kind) * (to - from).max(0),
            // soft float: a call
            _ => self.call(to.max(from), 8 * to.max(from), &["add hl;bc", "djnz e"], kind),
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
        let byte = self.is_byte(ty);
        let bits = if byte { 8 } else { 16 * parts };
        let constant = first.constant || second.constant;
        match op {
            Add if byte => priced(&["add a;b"], kind),
            Sub | And | Or | Xor if byte => priced(&["sub b"], kind),
            Add => priced(&["add hl;bc"], kind) + priced(&["adc hl;bc"], kind) * (parts - 1),
            Sub => priced(&["or b", "sbc hl;bc"], kind) + priced(&["sbc hl;bc"], kind) * (parts - 1),
            And | Or | Xor => priced(&["ld b;c", "and b", "ld b;c", "ld b;c", "and b", "ld b;c"], kind) * parts,
            FNeg => priced(&["xor n"], kind),
            // one step per bit; the amount is not known here, so one step
            // stands for a constant shift
            Shl | LShr | AShr if second.constant && byte => priced(&["sla b"], kind),
            Shl if second.constant => priced(&["add hl;bc"], kind) * parts,
            Shl | LShr | AShr if second.constant => priced(&["srl b", "rr b"], kind) * parts,
            // by a variable: a loop of one step and a `djnz`, 4 or 8 passes on
            // average
            Shl | LShr | AShr => self.call(parts, bits / 2, &["add hl;bc", "djnz e"], kind),
            Mul if second.power_of_two || first.power_of_two => priced(&["add hl;bc"], kind) * parts,
            Mul if constant => priced(&["add hl;bc", "add hl;bc", "add hl;bc"], kind) * parts,
            // shift-and-add: add hl,hl; jr nc; add hl,de; djnz per bit
            Mul => self.call(parts, bits, &["add hl;bc", "jr nc;e", "add hl;bc", "djnz e"], kind),
            UDiv | URem if second.power_of_two => priced(&["srl b", "rr b"], kind) * parts,
            // restoring division: add hl,hl; adc hl,hl; sbc hl,de; jr nc; add
            // hl,de; djnz per bit
            UDiv | URem | SDiv | SRem => {
                self.call(parts, bits, &["add hl;bc", "adc hl;bc", "sbc hl;bc", "jr nc;e", "add hl;bc", "djnz e"], kind)
            }
            // soft float: a call
            FAdd | FSub | FMul | FDiv | FRem => {
                self.call(parts, bits, &["add hl;bc", "jr nc;e", "add hl;bc", "djnz e"], kind)
            }
        }
    }
}
