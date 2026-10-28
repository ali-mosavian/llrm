//! Division and remainder by a constant: shifts, multiplies by magic numbers
//! and a fix-up instead of `sdiv`/`udiv`, as `TargetLowering::BuildUDIV`,
//! `BuildSDIV` and AArch64's `BuildSDIVPow2`/`BuildSREMPow2` build them
//! (<https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp>).
//! A value of 8 to 31 bits is extended to 32 first; 33 to 63 bits keep the
//! division instruction.

use llrm_mir::BinaryOp;

use super::val::{G, Known, width_of};
use super::{Selected, Selector};
use crate::divconst;
use crate::mc::inst::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    UDiv,
    SDiv,
    URem,
    SRem,
}

impl Selector<'_> {
    /// The register holding `x / d` or `x % d` for the constant `d`, or None
    /// where the division instruction is to be used.
    pub(super) fn divide_by_constant(
        &mut self,
        op: BinaryOp,
        bits: u32,
        x: G,
        divisor: u128,
    ) -> Selected<Option<G>> {
        let kind = match op {
            BinaryOp::UDiv => Kind::UDiv,
            BinaryOp::SDiv => Kind::SDiv,
            BinaryOp::URem => Kind::URem,
            BinaryOp::SRem => Kind::SRem,
            _ => return Ok(None),
        };
        // `isIntDivCheap`: at minimum size the instruction is smaller.
        let minsize =
            self.function.attrs.iter().any(|one| matches!(one, llrm_mir::Attribute::Flag(name) if name == "minsize"));
        if minsize || (bits > 32 && bits != 64) || bits < 8 {
            return Ok(None);
        }
        let w = if bits <= 32 { 32 } else { 64 };
        let signed = matches!(kind, Kind::SDiv | Kind::SRem);
        let d = divisor & if bits >= 64 { u128::from(u64::MAX) } else { (1u128 << bits) - 1 };
        if d == 0 {
            return Ok(None);
        }
        // The signed value of the divisor at its own width.
        let negative = signed && (d >> (bits - 1)) & 1 == 1;
        let magnitude = if negative { (1u128 << bits) - d } else { d };
        let x = self.extended(x, if signed { Known::Sign } else { Known::Zero });
        let width = width_of(w);
        let result = match kind {
            Kind::UDiv | Kind::URem if d == 1 => {
                if kind == Kind::UDiv {
                    x
                } else {
                    return self.zero_result(bits).map(Some);
                }
            }
            Kind::UDiv if d.is_power_of_two() => self.shifted(width, x.reg, d.trailing_zeros(), Shift::Lsr, bits),
            Kind::URem if d.is_power_of_two() => {
                let rd = self.mf.gpr();
                self.and_mask(width, rd, x.reg, (d - 1) as u64);
                G { reg: rd, bits, known: Known::Zero }
            }
            Kind::UDiv => self.unsigned_quotient(x, d, w, bits),
            Kind::URem => {
                let quotient = self.unsigned_quotient(x, d, w, bits);
                self.remainder_from(x, quotient, d, w, bits)
            }
            Kind::SDiv | Kind::SRem if magnitude == 1 => {
                if kind == Kind::SRem {
                    return self.zero_result(bits).map(Some);
                }
                if negative { self.negated(x, w, bits) } else { x }
            }
            Kind::SRem if magnitude.is_power_of_two() => {
                self.signed_remainder_pow2(x, magnitude.trailing_zeros(), w, bits)
            }
            Kind::SDiv if magnitude.is_power_of_two() => {
                self.signed_quotient_pow2(x, magnitude.trailing_zeros(), negative, w, bits)
            }
            Kind::SDiv => self.signed_quotient(x, d, w, bits),
            Kind::SRem => {
                let quotient = self.signed_quotient(x, d, w, bits);
                self.remainder_from(x, quotient, d, w, bits)
            }
        };
        Ok(Some(result))
    }

    fn zero_result(
        &mut self,
        bits: u32,
    ) -> Selected<G> {
        let rd = self.mf.gpr();
        self.load_immediate(rd, bits, 0);
        Ok(G { reg: rd, bits, known: Known::Zero })
    }

    fn result(
        &mut self,
        bits: u32,
    ) -> (Gpr, Known) {
        (
            self.mf.gpr(),
            if bits == 32 {
                Known::Zero
            } else if bits < 32 {
                Known::Hi32Zero
            } else {
                Known::Nothing
            },
        )
    }

    fn shifted(
        &mut self,
        width: Width,
        rn: Gpr,
        amount: u32,
        shift: Shift,
        bits: u32,
    ) -> G {
        let (rd, known) = self.result(bits);
        let size = width.bits();
        let (op, immr, imms) = match shift {
            Shift::Lsr => (Bitfield::Ubfm, amount, size - 1),
            Shift::Asr => (Bitfield::Sbfm, amount, size - 1),
            _ => unreachable!("a right shift"),
        };
        self.inst(Inst::BitfieldMove { op, width, rd, rn, immr: immr as u8, imms: imms as u8 });
        G { reg: rd, bits, known }
    }

    fn and_mask(
        &mut self,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        mask: u64,
    ) {
        if crate::mc::immediate::logical(mask, width.bits()).is_some() {
            self.inst(Inst::LogicImm { op: Logic::And, width, rd, rn, imm: mask });
        } else {
            let constant = self.mf.gpr();
            self.load_immediate(constant, width.bits(), mask);
            self.inst(Inst::LogicShifted {
                op: Logic::And,
                invert: false,
                width,
                rd,
                rn,
                rm: constant,
                shift: Shift::Lsl,
                amount: 0,
            });
        }
    }

    fn negated(
        &mut self,
        x: G,
        w: u32,
        bits: u32,
    ) -> G {
        let (rd, known) = self.result(bits);
        self.inst(Inst::AddSubShifted {
            op: AddSub { subtract: true, set_flags: false },
            width: width_of(w),
            rd,
            rn: Gpr::R31,
            rm: x.reg,
            shift: Shift::Lsl,
            amount: 0,
        });
        G { reg: rd, bits, known }
    }

    /// `r = x - q * d` (`msub`).
    fn remainder_from(
        &mut self,
        x: G,
        quotient: G,
        d: u128,
        w: u32,
        bits: u32,
    ) -> G {
        let constant = self.mf.gpr();
        self.load_immediate(constant, w, d as u64);
        let (rd, known) = self.result(bits);
        self.inst(Inst::ThreeSource {
            op: Mul3::Msub,
            width: width_of(w),
            rd,
            rn: quotient.reg,
            rm: constant,
            ra: x.reg,
        });
        G { reg: rd, bits, known }
    }

    /// `x / d` for an unsigned `d` that is no power of two: `BuildUDIV`.
    fn unsigned_quotient(
        &mut self,
        x: G,
        d: u128,
        w: u32,
        bits: u32,
    ) -> G {
        // The leading zeros of a narrow dividend simplify the magic number.
        let known_leading = w - bits;
        let divisor_leading = (w - (128 - d.leading_zeros())).min(w);
        let leading = known_leading.min(divisor_leading);
        let wide_ok = w == 32;
        let magics = divconst::unsigned(d, w, leading, !wide_ok, wide_ok);
        let width = width_of(w);
        let (rd, known) = self.result(bits);
        let magic = self.mf.gpr();
        if magics.widen {
            // One 64-bit high multiply of the zero-extended dividend; the magic
            // is pre-shifted so the high half is the quotient.
            self.load_immediate(magic, 64, magics.magic as u64);
            let high = self.mf.gpr();
            self.inst(Inst::ThreeSource {
                op: Mul3::Umulh,
                width: Width::X,
                rd: high,
                rn: x.reg,
                rm: magic,
                ra: Gpr::R31,
            });
            self.inst(Inst::LogicShifted {
                op: Logic::Orr,
                invert: false,
                width: Width::W,
                rd,
                rn: Gpr::R31,
                rm: high,
                shift: Shift::Lsl,
                amount: 0,
            });
            return G { reg: rd, bits, known };
        }
        let mut q = x.reg;
        if magics.pre_shift > 0 {
            let shifted = self.mf.gpr();
            self.inst(Inst::BitfieldMove {
                op: Bitfield::Ubfm,
                width,
                rd: shifted,
                rn: q,
                immr: magics.pre_shift as u8,
                imms: (w - 1) as u8,
            });
            q = shifted;
        }
        self.load_immediate(magic, w, magics.magic as u64);
        let high = self.mf.gpr();
        if w == 64 {
            self.inst(Inst::ThreeSource { op: Mul3::Umulh, width: Width::X, rd: high, rn: q, rm: magic, ra: Gpr::R31 });
        } else {
            // 32 bits: the 64-bit product, its high word.
            let product = self.mf.gpr();
            self.inst(Inst::ThreeSource {
                op: Mul3::Umaddl,
                width: Width::X,
                rd: product,
                rn: q,
                rm: magic,
                ra: Gpr::R31,
            });
            self.inst(Inst::BitfieldMove {
                op: Bitfield::Ubfm,
                width: Width::X,
                rd: high,
                rn: product,
                immr: 32,
                imms: 63,
            });
        }
        let mut result = high;
        if magics.is_add {
            // NPQ: ((x - q) >> 1) + q
            let difference = self.mf.gpr();
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract: true, set_flags: false },
                width,
                rd: difference,
                rn: x.reg,
                rm: high,
                shift: Shift::Lsl,
                amount: 0,
            });
            let sum = self.mf.gpr();
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width,
                rd: sum,
                rn: high,
                rm: difference,
                shift: Shift::Lsr,
                amount: 1,
            });
            result = sum;
        }
        if magics.post_shift > 0 {
            self.inst(Inst::BitfieldMove {
                op: Bitfield::Ubfm,
                width,
                rd,
                rn: result,
                immr: magics.post_shift as u8,
                imms: (w - 1) as u8,
            });
        } else {
            self.inst(Inst::LogicShifted {
                op: Logic::Orr,
                invert: false,
                width,
                rd,
                rn: Gpr::R31,
                rm: result,
                shift: Shift::Lsl,
                amount: 0,
            });
        }
        G { reg: rd, bits, known }
    }

    /// `x / d` for a signed `d` that is no power of two: `BuildSDIV`.
    fn signed_quotient(
        &mut self,
        x: G,
        d: u128,
        w: u32,
        bits: u32,
    ) -> G {
        let negative_divisor = (d >> (bits - 1)) & 1 == 1;
        // The magic number is for the divisor at the register's width.
        let at_width = if w == bits {
            d
        } else if negative_divisor {
            d | (!0u128 << bits)
        } else {
            d
        } & if w == 64 { u128::from(u64::MAX) } else { (1u128 << w) - 1 };
        let magics = divconst::signed(at_width, w);
        let magic_negative = (magics.magic >> (w - 1)) & 1 == 1;
        let width = width_of(w);
        let magic = self.mf.gpr();
        self.load_immediate(magic, w, magics.magic as u64);
        let mut q = self.mf.gpr();
        // Add or subtract the dividend where the signs of divisor and magic
        // differ.
        let adjusted = if !negative_divisor && magic_negative {
            Some(false)
        } else if negative_divisor && !magic_negative && magics.magic != 0 {
            Some(true)
        } else {
            None
        };
        let (mut adjust_done, mut shift_done) = (false, false);
        if w == 64 {
            self.inst(Inst::ThreeSource {
                op: Mul3::Smulh,
                width: Width::X,
                rd: q,
                rn: x.reg,
                rm: magic,
                ra: Gpr::R31,
            });
        } else {
            let product = self.mf.gpr();
            self.inst(Inst::ThreeSource {
                op: Mul3::Smaddl,
                width: Width::X,
                rd: product,
                rn: x.reg,
                rm: magic,
                ra: Gpr::R31,
            });
            // The high half of the product is read where it is used: added to the dividend as a shifted
            // operand, or shifted down with the quotient's own shift (the low 32 bits are all that matter).
            if adjusted == Some(false) {
                self.inst(Inst::AddSubShifted {
                    op: AddSub { subtract: false, set_flags: false },
                    width: Width::X,
                    rd: q,
                    rn: x.reg,
                    rm: product,
                    shift: Shift::Lsr,
                    amount: 32,
                });
                adjust_done = true;
            } else {
                let (op, immr) = if adjusted.is_none() && magics.shift > 0 {
                    shift_done = true;
                    (Bitfield::Sbfm, 32 + magics.shift as u8)
                } else {
                    (Bitfield::Ubfm, 32)
                };
                self.inst(Inst::BitfieldMove { op, width: Width::X, rd: q, rn: product, immr, imms: 63 });
            }
        }
        if let Some(subtract) = adjusted.filter(|_| !adjust_done) {
            let sum = self.mf.gpr();
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract, set_flags: false },
                width,
                rd: sum,
                rn: q,
                rm: x.reg,
                shift: Shift::Lsl,
                amount: 0,
            });
            q = sum;
        }
        if magics.shift > 0 && !shift_done {
            let shifted = self.mf.gpr();
            self.inst(Inst::BitfieldMove {
                op: Bitfield::Sbfm,
                width,
                rd: shifted,
                rn: q,
                immr: magics.shift as u8,
                imms: (w - 1) as u8,
            });
            q = shifted;
        }
        // Add the sign bit: q + (q >>> (w - 1)).
        let (rd, known) = self.result(bits);
        self.inst(Inst::AddSubShifted {
            op: AddSub { subtract: false, set_flags: false },
            width,
            rd,
            rn: q,
            rm: q,
            shift: Shift::Lsr,
            amount: (w - 1) as u8,
        });
        G { reg: rd, bits, known }
    }

    /// `x / (+-2^k)`: `BuildSDIVPow2` (a compare and select for |d| above 2) or
    /// the default expansion (the sign bit added before the shift).
    fn signed_quotient_pow2(
        &mut self,
        x: G,
        k: u32,
        negative: bool,
        w: u32,
        bits: u32,
    ) -> G {
        let width = width_of(w);
        let biased = self.mf.gpr();
        if k == 1 {
            // x + (x >>> (w - 1))
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width,
                rd: biased,
                rn: x.reg,
                rm: x.reg,
                shift: Shift::Lsr,
                amount: (w - 1) as u8,
            });
        } else {
            // cmp x, #0; add t, x, 2^k - 1; csel biased, t, x, lt
            self.inst(Inst::AddSubImm {
                op: AddSub { subtract: true, set_flags: true },
                width,
                rd: Gpr::R31,
                rn: x.reg,
                imm12: 0,
                shift12: false,
            });
            let plus = self.mf.gpr();
            let bias = (1u64 << k) - 1;
            if bias < 4096 {
                self.inst(Inst::AddSubImm {
                    op: AddSub { subtract: false, set_flags: false },
                    width,
                    rd: plus,
                    rn: x.reg,
                    imm12: bias as u16,
                    shift12: false,
                });
            } else {
                let constant = self.mf.gpr();
                self.load_immediate(constant, w, bias);
                self.inst(Inst::AddSubShifted {
                    op: AddSub { subtract: false, set_flags: false },
                    width,
                    rd: plus,
                    rn: x.reg,
                    rm: constant,
                    shift: Shift::Lsl,
                    amount: 0,
                });
            }
            self.inst(Inst::CondSelect { op: Select::Csel, width, rd: biased, rn: plus, rm: x.reg, cond: Cond::Lt });
        }
        let shifted = self.mf.gpr();
        self.inst(Inst::BitfieldMove {
            op: Bitfield::Sbfm,
            width,
            rd: shifted,
            rn: biased,
            immr: k as u8,
            imms: (w - 1) as u8,
        });
        if negative {
            return self.negated(G { reg: shifted, bits, known: Known::Nothing }, w, bits);
        }
        G { reg: shifted, bits, known: if bits == w { Known::Nothing } else { Known::Hi32Zero } }
    }

    /// `x % (+-2^k)`: `BuildSREMPow2`.
    fn signed_remainder_pow2(
        &mut self,
        x: G,
        k: u32,
        w: u32,
        bits: u32,
    ) -> G {
        let width = width_of(w);
        let mask = (1u64 << k) - 1;
        let (rd, known) = self.result(bits);
        if k == 1 {
            // cmp x, #0; and t, x, 1; csneg rd, t, t, ge
            self.inst(Inst::AddSubImm {
                op: AddSub { subtract: true, set_flags: true },
                width,
                rd: Gpr::R31,
                rn: x.reg,
                imm12: 0,
                shift12: false,
            });
            let low = self.mf.gpr();
            self.and_mask(width, low, x.reg, mask);
            self.inst(Inst::CondSelect { op: Select::Csneg, width, rd, rn: low, rm: low, cond: Cond::Ge });
        } else {
            // negs n, x; and p, x, mask; and q, n, mask; csneg rd, p, q, mi
            let negated = self.mf.gpr();
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract: true, set_flags: true },
                width,
                rd: negated,
                rn: Gpr::R31,
                rm: x.reg,
                shift: Shift::Lsl,
                amount: 0,
            });
            let positive = self.mf.gpr();
            self.and_mask(width, positive, x.reg, mask);
            let negative = self.mf.gpr();
            self.and_mask(width, negative, negated, mask);
            self.inst(Inst::CondSelect { op: Select::Csneg, width, rd, rn: positive, rm: negative, cond: Cond::Mi });
        }
        G { reg: rd, bits, known }
    }
}
