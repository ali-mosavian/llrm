//! Integer arithmetic, conversions, comparisons and `select`. An `iN` lives in
//! a 32-bit register for `N <= 32` and a 64-bit one above, its extra bits
//! undefined unless `Known` says otherwise; an operation that reads them
//! (`lshr`, `icmp`, a widening) extends first, and only when the value is not
//! already extended.

use llrm_mir::{BinaryOp, CastOp, Instruction, IntPredicate, Operand};

use super::val::{AddrExpr, Base, G, Known, Val, width_of};
use super::{Selected, Selector, Src, refuse};
use crate::mc::immediate;
use crate::mc::inst::*;

/// `imm12` and `shift12` for `value`, if one add or subtract immediate holds
/// it.
fn add_sub_immediate(value: u64) -> Option<(u16, bool)> {
    if value < 4096 {
        Some((value as u16, false))
    } else if value & 0xFFF == 0 && value >> 12 < 4096 {
        Some(((value >> 12) as u16, true))
    } else {
        None
    }
}

fn mask(bits: u32) -> u64 {
    if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 }
}

/// What a comparison reads once its operands are in registers.
pub(super) struct CompareOperands {
    pub(super) predicate: IntPredicate,
    width: Width,
    left: Gpr,
    right: CompareRight,
}

enum CompareRight {
    Constant { constant: u64, signed_constant: i64, bits: u32 },
    Register(Gpr),
}

pub(super) fn condition_of(predicate: IntPredicate) -> Cond {
    match predicate {
        IntPredicate::Eq => Cond::Eq,
        IntPredicate::Ne => Cond::Ne,
        IntPredicate::Ugt => Cond::Hi,
        IntPredicate::Uge => Cond::Hs,
        IntPredicate::Ult => Cond::Lo,
        IntPredicate::Ule => Cond::Ls,
        IntPredicate::Sgt => Cond::Gt,
        IntPredicate::Sge => Cond::Ge,
        IntPredicate::Slt => Cond::Lt,
        IntPredicate::Sle => Cond::Le,
    }
}

impl Selector<'_> {
    /// A register holding `g` with the bits above its width as `want` says
    /// (`Zero` or `Sign`).
    pub(crate) fn extended(
        &mut self,
        g: G,
        want: Known,
    ) -> G {
        let width = g.width();
        match want {
            Known::Zero => {
                if g.bits == 64 || g.known == Known::Zero {
                    return g;
                }
                if g.bits == 32 && g.known == Known::Hi32Zero {
                    return G { known: Known::Zero, ..g };
                }
                let rd = self.mf.gpr();
                if g.bits == 32 {
                    self.inst(Inst::LogicShifted {
                        op: Logic::Orr,
                        invert: false,
                        width: Width::W,
                        rd,
                        rn: Gpr::R31,
                        rm: g.reg,
                        shift: Shift::Lsl,
                        amount: 0,
                    });
                } else if g.bits < 32 {
                    self.inst(Inst::LogicImm { op: Logic::And, width, rd, rn: g.reg, imm: mask(g.bits) });
                } else {
                    self.inst(Inst::BitfieldMove {
                        op: Bitfield::Ubfm,
                        width,
                        rd,
                        rn: g.reg,
                        immr: 0,
                        imms: (g.bits - 1) as u8,
                    });
                }
                G { reg: rd, bits: g.bits, known: Known::Zero }
            }
            Known::Sign => {
                if g.bits == 64 || g.bits == 32 || g.known == Known::Sign {
                    return g;
                }
                let rd = self.mf.gpr();
                self.inst(Inst::BitfieldMove {
                    op: Bitfield::Sbfm,
                    width,
                    rd,
                    rn: g.reg,
                    immr: 0,
                    imms: (g.bits - 1) as u8,
                });
                G { reg: rd, bits: g.bits, known: Known::Sign }
            }
            _ => g,
        }
    }

    /// `g` as a 64-bit value: its bits above `bits` all zero or all copies of
    /// the top one.
    pub(crate) fn widened(
        &mut self,
        g: G,
        to: u32,
        signed: bool,
    ) -> G {
        if signed {
            let rd = self.mf.gpr();
            self.inst(Inst::BitfieldMove {
                op: Bitfield::Sbfm,
                width: width_of(to),
                rd,
                rn: g.reg,
                immr: 0,
                imms: (g.bits - 1) as u8,
            });
            return G { reg: rd, bits: to, known: if to <= 32 { Known::Sign } else { Known::Nothing } };
        }
        let zero = self.extended(g, Known::Zero);
        // A `w` register's write leaves bits 32 and above zero: it is already
        // the 64-bit value.
        G { reg: zero.reg, bits: to, known: if to <= 32 { Known::Zero } else { Known::Nothing } }
    }

    fn result_g(
        &mut self,
        bits: u32,
    ) -> (Gpr, Known) {
        (
            self.mf.gpr(),
            if bits <= 32 { if bits == 32 { Known::Zero } else { Known::Hi32Zero } } else { Known::Nothing },
        )
    }

    pub(super) fn binary(
        &mut self,
        op: BinaryOp,
        instruction: &Instruction,
    ) -> Selected<()> {
        let ty = instruction.ty;
        let result = instruction.result;
        if matches!(
            op,
            BinaryOp::FAdd | BinaryOp::FSub | BinaryOp::FMul | BinaryOp::FDiv | BinaryOp::FRem
        ) {
            return self.float_binary(op, instruction);
        }
        let Some(bits) = self.types().int_bits(ty) else {
            return refuse(format!("{} of a non-integer type", instruction_name(op)));
        };
        if bits > 64 {
            return refuse(format!("{} of i{bits}", instruction_name(op)));
        }
        let width = width_of(bits);
        if op == BinaryOp::And && instruction.result.is_some_and(|result| self.tested_by_compare(result)) {
            // A `tst` reads the operands where the comparison is.
            self.fused
                .insert(super::value_def(self.function, instruction.result.expect("checked")).expect("an instruction"));
            return Ok(());
        }
        if op == BinaryOp::Mul && result.is_some_and(|result| self.is_widened_product(result)) {
            // `smull` or `smaddl` reads the factors where the extension is.
            self.fused.insert(super::value_def(self.function, result.expect("checked")).expect("an instruction"));
            return Ok(());
        }
        if op == BinaryOp::Mul && result.is_some_and(|result| self.accumulated(result)) {
            // `madd` or `msub` reads the operands where the add or subtract is.
            self.fused.insert(super::value_def(self.function, result.expect("checked")).expect("an instruction"));
            return Ok(());
        }
        if op == BinaryOp::Shl && self.folds_into_index(instruction) {
            // The `getelementptr` that reads it indexes by the unshifted value.
            let inst = super::value_def(self.function, result.expect("a shift has a result")).expect("an instruction");
            self.fused.insert(inst);
            self.indexed.insert(inst);
            return Ok(());
        }
        if matches!(op, BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr) && self.folds_into_reader(instruction) {
            // The reader shifts its operand itself.
            self.fused.insert(
                super::value_def(self.function, result.expect("a shift has a result")).expect("an instruction"),
            );
            return Ok(());
        }
        if let Some(done) = self.shifted_operand(op, bits, instruction)? {
            return self.define(result, Src::Val(Val::Int(done)));
        }
        if matches!(op, BinaryOp::Add | BinaryOp::Sub)
            && let Some(done) = self.multiply_accumulate(op == BinaryOp::Sub, bits, instruction)?
        {
            return self.define(result, Src::Val(Val::Int(done)));
        }
        let (lhs, rhs) = (self.src(instruction.operands[0])?, self.src(instruction.operands[1])?);
        // Division by a constant is shifts and multiplies.
        if let (BinaryOp::UDiv | BinaryOp::SDiv | BinaryOp::URem | BinaryOp::SRem, Src::Int { value, .. }) = (op, &rhs)
        {
            let g = self.gpr_of(lhs.clone())?;
            if let Some(found) = self.divide_by_constant(op, bits, g, *value)? {
                return self.define(result, Src::Val(Val::Int(found)));
            }
        }
        let value = match op {
            BinaryOp::Add | BinaryOp::Sub => self.add_sub(op == BinaryOp::Sub, bits, lhs, rhs)?,
            BinaryOp::And | BinaryOp::Or | BinaryOp::Xor => self.logic(op, bits, lhs, rhs)?,
            BinaryOp::Mul => self.multiply(bits, lhs, rhs)?,
            BinaryOp::Shl | BinaryOp::LShr | BinaryOp::AShr => self.shift(op, bits, lhs, rhs)?,
            BinaryOp::UDiv | BinaryOp::SDiv => {
                let signed = op == BinaryOp::SDiv;
                let want = if signed { Known::Sign } else { Known::Zero };
                let (a, b) = (self.gpr_of(lhs)?, self.gpr_of(rhs)?);
                let (a, b) = (self.extended(a, want), self.extended(b, want));
                let (rd, known) = self.result_g(bits);
                let div = if signed { Div::Sdiv } else { Div::Udiv };
                self.inst(Inst::Divide { op: div, width, rd, rn: a.reg, rm: b.reg });
                G { reg: rd, bits, known }
            }
            BinaryOp::URem | BinaryOp::SRem => {
                let signed = op == BinaryOp::SRem;
                let want = if signed { Known::Sign } else { Known::Zero };
                let (a, b) = (self.gpr_of(lhs)?, self.gpr_of(rhs)?);
                let (a, b) = (self.extended(a, want), self.extended(b, want));
                let quotient = self.mf.gpr();
                let div = if signed { Div::Sdiv } else { Div::Udiv };
                self.inst(Inst::Divide { op: div, width, rd: quotient, rn: a.reg, rm: b.reg });
                let (rd, known) = self.result_g(bits);
                self.inst(Inst::ThreeSource { op: Mul3::Msub, width, rd, rn: quotient, rm: b.reg, ra: a.reg });
                G { reg: rd, bits, known }
            }
            other => return refuse(format!("the operation {}", instruction_name(other))),
        };
        self.define(result, Src::Val(Val::Int(value)))
    }

    fn add_sub(
        &mut self,
        subtract: bool,
        bits: u32,
        lhs: Src,
        rhs: Src,
    ) -> Selected<G> {
        let width = width_of(bits);
        let flags = AddSub { subtract, set_flags: false };
        // A constant first operand of an add is the second.
        let (lhs, rhs) = match (&lhs, &rhs, subtract) {
            (Src::Int { .. }, Src::Val(_), false) => (rhs, lhs),
            _ => (lhs, rhs),
        };
        let (rd, known) = self.result_g(bits);
        if let Src::Int { value, .. } = rhs {
            let signed = llrm_mir::context::signed(value, bits) as i64;
            // `x + c` with a negative `c` is `x - |c|`.
            let (flip, magnitude) = if signed < 0 { (true, signed.unsigned_abs()) } else { (false, signed as u64) };
            if let Some((imm12, shift12)) = add_sub_immediate(magnitude) {
                let a = self.gpr_of(lhs)?;
                self.inst(Inst::AddSubImm {
                    op: AddSub { subtract: subtract != flip, set_flags: false },
                    width,
                    rd,
                    rn: a.reg,
                    imm12,
                    shift12,
                });
                return Ok(G { reg: rd, bits, known });
            }
        }
        let (a, b) = (self.gpr_of(lhs.clone())?, self.gpr_of(rhs)?);
        // `0 - x` is `neg`: the zero register stands for the constant.
        let rn = match lhs {
            Src::Int { value: 0, .. } if subtract => Gpr::R31,
            _ => a.reg,
        };
        self.inst(Inst::AddSubShifted { op: flags, width, rd, rn, rm: b.reg, shift: Shift::Lsl, amount: 0 });
        Ok(G { reg: rd, bits, known })
    }

    /// Whether the product `value` is read only by an `add` or `sub` of its own
    /// block that can take it as the multiply of a `madd` or `msub`: 32 or
    /// 64 bits, neither factor a constant (a constant factor is a shift or
    /// an add), the other addend not a constant, and, for a `sub`, the
    /// product subtracted.
    pub(super) fn accumulated(
        &self,
        value: llrm_mir::ValueId,
    ) -> bool {
        if self.use_count(value) != 1 {
            return false;
        }
        let Some(def) = super::value_def(self.function, value) else { return false };
        let product = self.function.instruction(def);
        if !matches!(self.types().int_bits(product.ty), Some(32 | 64))
            || product.operands.iter().any(|one| matches!(one, Operand::Constant(_)))
        {
            return false;
        }
        let Some(user) = self.function.users(value).into_iter().next() else { return false };
        let reader = self.function.instruction(user.user);
        let [first, second] = reader.operands[..] else { return false };
        let other = match reader.opcode {
            // Of two products added, the second is the one accumulated onto the
            // first.
            llrm_mir::Opcode::Binary(BinaryOp::Add) if first == Operand::Value(value) && !self.is_product(second) => {
                second
            }
            llrm_mir::Opcode::Binary(BinaryOp::Add | BinaryOp::Sub) if second == Operand::Value(value) => first,
            _ => return false,
        };
        self.function.parent(def) == self.function.parent(user.user) && !matches!(other, Operand::Constant(_))
    }

    fn is_product(
        &self,
        operand: Operand,
    ) -> bool {
        let Operand::Value(value) = operand else { return false };
        super::value_def(self.function, value)
            .is_some_and(|def| self.function.instruction(def).opcode == llrm_mir::Opcode::Binary(BinaryOp::Mul))
    }

    /// `madd` or `msub` for an `add` or `sub` of a product left for it by
    /// [`Self::accumulated`].
    fn multiply_accumulate(
        &mut self,
        subtract: bool,
        bits: u32,
        instruction: &Instruction,
    ) -> Selected<Option<G>> {
        // A product, or the extension of a 32-bit one (`widemul`): its factors,
        // and whether it widens.
        let product = |operand: Operand| match operand {
            Operand::Value(value) => super::value_def(self.function, value)
                .filter(|def| self.fused.contains(def))
                .map(|def| self.function.instruction(def).clone())
                .and_then(|one| match one.opcode {
                    llrm_mir::Opcode::Binary(BinaryOp::Mul) => Some((one, None)),
                    llrm_mir::Opcode::Cast(llrm_mir::CastOp::SExt) => {
                        self.widened_product(&one).map(|mul| (mul, Some(true)))
                    }
                    llrm_mir::Opcode::Cast(llrm_mir::CastOp::ZExt) => {
                        self.widened_product(&one).map(|mul| (mul, Some(false)))
                    }
                    _ => None,
                }),
            _ => None,
        };
        let [first, second] = instruction.operands[..] else { return Ok(None) };
        let ((found, wide), other) = match (product(first), product(second)) {
            (Some(found), _) if !subtract => (found, second),
            (_, Some(found)) => (found, first),
            _ => return Ok(None),
        };
        let (lhs, rhs) = (self.src(found.operands[0])?, self.src(found.operands[1])?);
        let (a, b) = (self.gpr_of(lhs)?, self.gpr_of(rhs)?);
        let addend = self.src(other)?;
        let c = self.gpr_of(addend)?;
        let (rd, known) = self.result_g(bits);
        let op = match (wide, subtract) {
            (None, false) => Mul3::Madd,
            (None, true) => Mul3::Msub,
            (Some(true), false) => Mul3::Smaddl,
            (Some(true), true) => Mul3::Smsubl,
            (Some(false), false) => Mul3::Umaddl,
            (Some(false), true) => Mul3::Umsubl,
        };
        self.inst(Inst::ThreeSource { op, width: width_of(bits), rd, rn: a.reg, rm: b.reg, ra: c.reg });
        Ok(Some(G { reg: rd, bits, known }))
    }

    fn logic(
        &mut self,
        op: BinaryOp,
        bits: u32,
        lhs: Src,
        rhs: Src,
    ) -> Selected<G> {
        let width = width_of(bits);
        let logic = match op {
            BinaryOp::And => Logic::And,
            BinaryOp::Or => Logic::Orr,
            _ => Logic::Eor,
        };
        let (lhs, rhs) = match (&lhs, &rhs) {
            (Src::Int { .. }, Src::Val(_)) => (rhs, lhs),
            _ => (lhs, rhs),
        };
        let (rd, known) = self.result_g(bits);
        if let Src::Int { value, .. } = rhs {
            let value = value as u64;
            // `x ^ -1` is `mvn`.
            if logic == Logic::Eor && value == mask(bits) {
                let a = self.gpr_of(lhs)?;
                self.inst(Inst::LogicShifted {
                    op: Logic::Orr,
                    invert: true,
                    width,
                    rd,
                    rn: Gpr::R31,
                    rm: a.reg,
                    shift: Shift::Lsl,
                    amount: 0,
                });
                return Ok(G { reg: rd, bits, known });
            }
            // The bits above the width are free: a mask or its sign-extended
            // form, whichever encodes.
            let register_bits = width.bits();
            let wide = (value | !mask(bits)) & mask(register_bits);
            for candidate in [value, wide] {
                if immediate::logical(candidate, register_bits).is_some() {
                    let a = self.gpr_of(lhs)?;
                    self.inst(Inst::LogicImm { op: logic, width, rd, rn: a.reg, imm: candidate });
                    let known =
                        if logic == Logic::And && candidate == value && bits <= 32 { Known::Zero } else { known };
                    return Ok(G { reg: rd, bits, known });
                }
            }
        }
        let (a, b) = (self.gpr_of(lhs)?, self.gpr_of(rhs)?);
        self.inst(Inst::LogicShifted {
            op: logic,
            invert: false,
            width,
            rd,
            rn: a.reg,
            rm: b.reg,
            shift: Shift::Lsl,
            amount: 0,
        });
        Ok(G { reg: rd, bits, known })
    }

    fn multiply(
        &mut self,
        bits: u32,
        lhs: Src,
        rhs: Src,
    ) -> Selected<G> {
        let width = width_of(bits);
        let (lhs, rhs) = match (&lhs, &rhs) {
            (Src::Int { .. }, Src::Val(_)) => (rhs, lhs),
            _ => (lhs, rhs),
        };
        let (rd, known) = self.result_g(bits);
        if let Src::Int { value, .. } = rhs {
            let value = value as u64 & mask(bits);
            if value.is_power_of_two() {
                let a = self.gpr_of(lhs)?;
                let shift = value.trailing_zeros();
                self.shift_left_immediate(width, rd, a.reg, shift);
                return Ok(G { reg: rd, bits, known });
            }
            // `x * (2^k + 1)` and `x * (2^k - 1)` by one shifted add or
            // subtract.
            let a_shift = |plus: u64| plus.is_power_of_two().then(|| plus.trailing_zeros());
            if let Some(k) = a_shift(value.wrapping_sub(1)) {
                let a = self.gpr_of(lhs)?;
                self.inst(Inst::AddSubShifted {
                    op: AddSub { subtract: false, set_flags: false },
                    width,
                    rd,
                    rn: a.reg,
                    rm: a.reg,
                    shift: Shift::Lsl,
                    amount: k as u8,
                });
                return Ok(G { reg: rd, bits, known });
            }
            if let Some(k) = a_shift(value.wrapping_add(1) & mask(bits)) {
                let a = self.gpr_of(lhs)?;
                // rd = (a << k) - a
                let shifted = self.mf.gpr();
                self.shift_left_immediate(width, shifted, a.reg, k);
                self.inst(Inst::AddSubShifted {
                    op: AddSub { subtract: true, set_flags: false },
                    width,
                    rd,
                    rn: shifted,
                    rm: a.reg,
                    shift: Shift::Lsl,
                    amount: 0,
                });
                return Ok(G { reg: rd, bits, known });
            }
        }
        let (a, b) = (self.gpr_of(lhs)?, self.gpr_of(rhs)?);
        self.inst(Inst::ThreeSource { op: Mul3::Madd, width, rd, rn: a.reg, rm: b.reg, ra: Gpr::R31 });
        Ok(G { reg: rd, bits, known })
    }

    /// `rd = rn << shift` (`lsl` is `ubfm`).
    pub(crate) fn shift_left_immediate(
        &mut self,
        width: Width,
        rd: Gpr,
        rn: Gpr,
        shift: u32,
    ) {
        let size = width.bits();
        if shift == 0 {
            self.inst(Inst::LogicShifted {
                op: Logic::Orr,
                invert: false,
                width,
                rd,
                rn: Gpr::R31,
                rm: rn,
                shift: Shift::Lsl,
                amount: 0,
            });
            return;
        }
        self.inst(Inst::BitfieldMove {
            op: Bitfield::Ubfm,
            width,
            rd,
            rn,
            immr: ((size - shift) % size) as u8,
            imms: (size - 1 - shift) as u8,
        });
    }

    fn shift(
        &mut self,
        op: BinaryOp,
        bits: u32,
        lhs: Src,
        rhs: Src,
    ) -> Selected<G> {
        let width = width_of(bits);
        let size = width.bits();
        let (rd, known) = self.result_g(bits);
        let a = self.gpr_of(lhs)?;
        let want = match op {
            BinaryOp::LShr => Known::Zero,
            BinaryOp::AShr => Known::Sign,
            _ => Known::Nothing,
        };
        let a = self.extended(a, want);
        if let Src::Int { value, .. } = rhs {
            let amount = (value as u32) % size;
            match op {
                BinaryOp::Shl => self.shift_left_immediate(width, rd, a.reg, amount),
                BinaryOp::LShr => self.inst(Inst::BitfieldMove {
                    op: Bitfield::Ubfm,
                    width,
                    rd,
                    rn: a.reg,
                    immr: amount as u8,
                    imms: (size - 1) as u8,
                }),
                _ => self.inst(Inst::BitfieldMove {
                    op: Bitfield::Sbfm,
                    width,
                    rd,
                    rn: a.reg,
                    immr: amount as u8,
                    imms: (size - 1) as u8,
                }),
            }
        } else {
            let b = self.gpr_of(rhs)?;
            let div = match op {
                BinaryOp::Shl => Div::Lslv,
                BinaryOp::LShr => Div::Lsrv,
                _ => Div::Asrv,
            };
            self.inst(Inst::Divide { op: div, width, rd, rn: a.reg, rm: b.reg });
        }
        let known = match op {
            BinaryOp::LShr if bits < size => Known::Zero,
            BinaryOp::AShr if bits < size => Known::Sign,
            _ => known,
        };
        Ok(G { reg: rd, bits, known })
    }

    pub(super) fn cast(
        &mut self,
        op: CastOp,
        instruction: &Instruction,
    ) -> Selected<()> {
        let result = instruction.result;
        let to = instruction.ty;
        if matches!(op, CastOp::FPToSI | CastOp::FPToUI) {
            // Its operand may be a multiply left to it (`fixedpoint`), with no
            // value.
            return self.float_cast(op, instruction);
        }
        if let Some(product) = self.widened_product(instruction) {
            let inst = super::value_def(self.function, result.expect("a cast has a result")).expect("an instruction");
            if self.accumulated(result.expect("checked")) {
                // `smaddl` or `smsubl` reads the product where the add or
                // subtract is.
                self.fused.insert(inst);
                return Ok(());
            }
            return self.widening_multiply(instruction, &product);
        }
        let from_type = self.operand_type(instruction.operands[0])?;
        let source = self.src(instruction.operands[0])?;
        match op {
            CastOp::Trunc => {
                let Some(bits) = self.types().int_bits(to) else { return refuse("a truncation to a non-integer") };
                let g = self.gpr_of(source)?;
                let known = match g.known {
                    Known::Nothing => Known::Nothing,
                    _ if g.bits > 32 => Known::Nothing,
                    _ => Known::Hi32Zero,
                };
                let known = if bits == 32 && known == Known::Hi32Zero { Known::Zero } else { known };
                self.define(result, Src::Val(Val::Int(G { reg: g.reg, bits, known })))
            }
            CastOp::ZExt | CastOp::SExt if self.folds_into_index(instruction) => {
                // The `getelementptr` that reads it extends the index in its
                // addressing mode.
                let inst =
                    super::value_def(self.function, result.expect("a cast has a result")).expect("an instruction");
                self.fused.insert(inst);
                self.indexed.insert(inst);
                Ok(())
            }
            CastOp::ZExt | CastOp::SExt => {
                let (Some(from_bits), Some(bits)) = (self.types().int_bits(from_type), self.types().int_bits(to))
                else {
                    return refuse("an extension of a non-integer");
                };
                if bits > 64 {
                    return refuse(format!("an extension to i{bits}"));
                }
                let g = self.gpr_of(source)?;
                let signed = op == CastOp::SExt;
                let value = if bits > 32 || signed {
                    let narrow = G { bits: from_bits, ..g };
                    if bits <= 32 && signed && g.known == Known::Sign {
                        G { bits, ..narrow }
                    } else {
                        self.widened(narrow, bits, signed)
                    }
                } else {
                    let zero = self.extended(G { bits: from_bits, ..g }, Known::Zero);
                    G { reg: zero.reg, bits, known: Known::Zero }
                };
                self.define(result, Src::Val(Val::Int(value)))
            }
            CastOp::PtrToInt => {
                let g = self.gpr_of(source)?;
                let bits = self.types().int_bits(to).unwrap_or(64);
                self.define(
                    result,
                    Src::Val(Val::Int(G { bits, known: if bits < 64 { Known::Nothing } else { g.known }, ..g })),
                )
            }
            CastOp::IntToPtr => {
                let g = self.gpr_of(source)?;
                let wide = if g.bits < 64 { self.widened(g, 64, false) } else { g };
                self.define(result, Src::Val(Val::Ptr(AddrExpr::of(Base::Reg(wide.reg)))))
            }
            CastOp::BitCast if matches!(self.ty(from_type), llrm_mir::Type::Pointer(_)) => self.define(result, source),
            other => self.float_cast(other, instruction),
        }
    }

    /// `icmp` as a value: `cset`.
    pub(super) fn icmp_value(
        &mut self,
        predicate: IntPredicate,
        instruction: &Instruction,
    ) -> Selected<()> {
        let result = instruction.result.expect("an icmp has a result");
        if self.fusable(result) {
            return Ok(());
        }
        let cond = self.compare(predicate, instruction.operands[0], instruction.operands[1])?;
        let rd = self.mf.gpr();
        // cset rd, cond = csinc rd, wzr, wzr, !cond
        self.inst(Inst::CondSelect {
            op: Select::Csinc,
            width: Width::W,
            rd,
            rn: Gpr::R31,
            rm: Gpr::R31,
            cond: cond.inverted(),
        });
        self.define(Some(result), Src::Val(Val::Int(G { reg: rd, bits: 1, known: Known::Zero })))
    }

    /// Whether the comparison `value` is read only by branches and selects of
    /// its own block, each as the condition, so it is selected at each of
    /// them (`flagcse` drops the repeats the flags already hold, as the
    /// pin's one `cmp` for the selects and the branch of one test). A float
    /// comparison needing two conditions is a value.
    pub(super) fn fusable(
        &self,
        value: llrm_mir::ValueId,
    ) -> bool {
        if self.use_count(value) == 0 {
            return false;
        }
        if let Some(inst) = super::value_def(self.function, value)
            && let llrm_mir::Opcode::FCmp(predicate) = self.function.instruction(inst).opcode
            && super::fp::conditions(predicate).1.is_some()
        {
            return false;
        }
        let own = super::value_def(self.function, value).and_then(|inst| self.function.parent(inst));
        let condition = Operand::Value(value);
        self.function.users(value).into_iter().all(|user| {
            let reader = self.function.instruction(user.user);
            own == self.function.parent(user.user)
                && match reader.opcode {
                    llrm_mir::Opcode::Br => reader.operands.len() == 3 && reader.operands[0] == condition,
                    llrm_mir::Opcode::Select => {
                        reader.operands[0] == condition && reader.operands[1..].iter().all(|one| *one != condition)
                    }
                    _ => false,
                }
        })
    }

    /// Sets the flags for `a predicate b` and names the condition that holds.
    pub(crate) fn compare(
        &mut self,
        predicate: IntPredicate,
        a: Operand,
        b: Operand,
    ) -> Selected<Cond> {
        if let Operand::Value(value) = a
            && self.is_zero(b)
            && let Some(and) = self.fused_and(value)
        {
            self.test_bits(&and)?;
            return Ok(condition_of(predicate));
        }
        let operands = self.compare_operands(predicate, a, b)?;
        self.set_flags_of(&operands);
        Ok(condition_of(operands.predicate))
    }

    /// The registers and constant a comparison of `a` with `b` reads, extended
    /// as the predicate needs (a constant on the left is swapped to the
    /// right, with the predicate).
    pub(super) fn compare_operands(
        &mut self,
        predicate: IntPredicate,
        a: Operand,
        b: Operand,
    ) -> Selected<CompareOperands> {
        let (mut left, mut right, mut predicate) = (self.src(a)?, self.src(b)?, predicate);
        if matches!(left, Src::Int { .. }) && !matches!(right, Src::Int { .. }) {
            std::mem::swap(&mut left, &mut right);
            predicate = predicate.swapped();
        }
        let bits = match (&left, &right) {
            (Src::Val(Val::Int(g)), _) => g.bits,
            (_, Src::Val(Val::Int(g))) => g.bits,
            (Src::Int { bits, .. }, _) => *bits,
            _ => 64,
        };
        let signed = matches!(
            predicate,
            IntPredicate::Sgt | IntPredicate::Sge | IntPredicate::Slt | IntPredicate::Sle
        );
        let want = if signed { Known::Sign } else { Known::Zero };
        let width = width_of(bits);
        let l = self.gpr_of(left)?;
        let l = self.extended(G { bits, ..l }, want);
        let right = if let Src::Int { value, .. } = right {
            // The constant is extended as the register is, so the flags agree.
            let constant = if signed { llrm_mir::context::signed(value, bits) as i64 as u64 } else { value as u64 };
            let constant = constant & mask(width.bits());
            let signed_constant = if width == Width::W { i64::from(constant as u32 as i32) } else { constant as i64 };
            CompareRight::Constant { constant, signed_constant, bits }
        } else {
            let r = self.gpr_of(right)?;
            CompareRight::Register(self.extended(G { bits, ..r }, want).reg)
        };
        Ok(CompareOperands { predicate, width, left: l.reg, right })
    }

    /// `x cmp c` as `subs` of the constant, or `adds` of its negation; else
    /// `subs` of a register.
    pub(super) fn set_flags_of(
        &mut self,
        operands: &CompareOperands,
    ) {
        let CompareOperands { width, left, .. } = *operands;
        match operands.right {
            CompareRight::Constant { constant, signed_constant, bits } => {
                let (add, magnitude) = if signed_constant < 0 && bits > 1 {
                    (true, signed_constant.unsigned_abs())
                } else {
                    (false, constant)
                };
                if let Some((imm12, shift12)) = add_sub_immediate(magnitude) {
                    self.inst(Inst::AddSubImm {
                        op: AddSub { subtract: !add, set_flags: true },
                        width,
                        rd: Gpr::R31,
                        rn: left,
                        imm12,
                        shift12,
                    });
                    return;
                }
                let reg = self.mf.gpr();
                self.load_immediate(reg, bits, constant);
                self.subs_register(width, left, reg);
            }
            CompareRight::Register(right) => self.subs_register(width, left, right),
        }
    }

    fn subs_register(
        &mut self,
        width: Width,
        left: Gpr,
        right: Gpr,
    ) {
        self.inst(Inst::AddSubShifted {
            op: AddSub { subtract: true, set_flags: true },
            width,
            rd: Gpr::R31,
            rn: left,
            rm: right,
            shift: Shift::Lsl,
            amount: 0,
        });
    }

    /// `ccmp`/`ccmn`: where `cond` holds, the flags of the comparison; else
    /// `nzcv`.
    pub(super) fn set_flags_if(
        &mut self,
        operands: &CompareOperands,
        nzcv: u8,
        cond: Cond,
    ) {
        let CompareOperands { width, left, .. } = *operands;
        let (negate, second) = match operands.right {
            CompareRight::Constant { constant, signed_constant, bits } => {
                let (negate, magnitude) = if signed_constant < 0 && bits > 1 {
                    (true, signed_constant.unsigned_abs())
                } else {
                    (false, constant)
                };
                if magnitude <= 31 {
                    (negate, CcOperand::Imm5(magnitude as u8))
                } else {
                    let reg = self.mf.gpr();
                    self.load_immediate(reg, bits, constant);
                    (false, CcOperand::Reg(reg))
                }
            }
            CompareRight::Register(right) => (false, CcOperand::Reg(right)),
        };
        self.inst(Inst::CondCompare { negate, width, rn: left, rm_or_imm: second, nzcv, cond });
    }

    fn is_zero(
        &self,
        operand: Operand,
    ) -> bool {
        matches!(
            operand,
            Operand::Constant(id) if matches!(self.module.context.get(id).kind, llrm_mir::ConstantKind::Int(0) | llrm_mir::ConstantKind::Zero | llrm_mir::ConstantKind::Null)
        )
    }

    /// Whether the `and` `value` is read only by a comparison with zero whose
    /// predicate `tst`'s flags answer (and that is no leaf of a `ccmp` chain):
    /// equality and the signed orders (`tst` leaves C and V clear, as a
    /// compare with zero leaves V).
    pub(super) fn tested_by_compare(
        &self,
        value: llrm_mir::ValueId,
    ) -> bool {
        if self.use_count(value) != 1 {
            return false;
        }
        let Some(user) = self.function.users(value).into_iter().next() else { return false };
        let reader = self.function.instruction(user.user);
        let own = super::value_def(self.function, value).and_then(|inst| self.function.parent(inst));
        let eligible = matches!(
            reader.opcode,
            llrm_mir::Opcode::ICmp(
                IntPredicate::Eq
                    | IntPredicate::Ne
                    | IntPredicate::Slt
                    | IntPredicate::Sge
                    | IntPredicate::Sgt
                    | IntPredicate::Sle
            )
        );
        // A leaf of a `ccmp` chain compares a register: its `and` is made.
        eligible
            && !reader.result.is_some_and(|result| self.conjunctions.contains(result))
            && own == self.function.parent(user.user)
            && reader.operands[0] == Operand::Value(value)
            && self.is_zero(reader.operands[1])
            && self.function.instruction(super::value_def(self.function, value).expect("an instruction")).ty
                != self.types().int(0)
    }

    /// The `and` that `value` is, if it was left for a `tst`.
    pub(super) fn fused_and(
        &self,
        value: llrm_mir::ValueId,
    ) -> Option<Instruction> {
        let inst = super::value_def(self.function, value)?;
        (self.fused.contains(&inst)
            && self.function.instruction(inst).opcode == llrm_mir::Opcode::Binary(BinaryOp::And))
        .then(|| self.function.instruction(inst).clone())
    }

    /// `tst a, b`: the flags of `a & b`.
    pub(super) fn test_bits(
        &mut self,
        and: &Instruction,
    ) -> Selected<()> {
        let bits = self.types().int_bits(and.ty).unwrap_or(64);
        let width = width_of(bits);
        let (mut left, mut right) = (self.src(and.operands[0])?, self.src(and.operands[1])?);
        if matches!(left, Src::Int { .. }) {
            std::mem::swap(&mut left, &mut right);
        }
        let first = self.gpr_of(left)?;
        if let Src::Int { value, .. } = right
            && immediate::logical(value as u64 & mask(bits), width.bits()).is_some()
        {
            self.inst(Inst::LogicImm {
                op: Logic::Ands,
                width,
                rd: Gpr::R31,
                rn: first.reg,
                imm: value as u64 & mask(bits),
            });
            return Ok(());
        }
        let second = self.gpr_of(right)?;
        // One operand extended: the other's bits above the width are cleared.
        let second = self.extended(second, Known::Zero);
        self.inst(Inst::LogicShifted {
            op: Logic::Ands,
            invert: false,
            width,
            rd: Gpr::R31,
            rn: first.reg,
            rm: second.reg,
            shift: Shift::Lsl,
            amount: 0,
        });
        Ok(())
    }

    /// The condition that holds where the i1 `operand` is true, with the flags
    /// set for it.
    pub(crate) fn condition(
        &mut self,
        operand: Operand,
    ) -> Selected<Cond> {
        if let Operand::Value(value) = operand
            && (self.fusable(value) || self.chains.contains(value) || self.conjunctions.contains(value))
            && let Some(inst) = super::value_def(self.function, value)
        {
            let compare = self.function.instruction(inst).clone();
            match compare.opcode {
                llrm_mir::Opcode::ICmp(predicate) => {
                    return self.compare(predicate, compare.operands[0], compare.operands[1]);
                }
                llrm_mir::Opcode::FCmp(predicate) => {
                    return self.float_compare(predicate, compare.operands[0], compare.operands[1]);
                }
                _ => {}
            }
        }
        if let Operand::Value(value) = operand
            && self.is_conjunction_root(value)
        {
            return self.conjunction(value);
        }
        let g = self.operand_g(operand)?;
        // `tst w, #1`
        self.inst(Inst::LogicImm { op: Logic::Ands, width: Width::W, rd: Gpr::R31, rn: g.reg, imm: 1 });
        Ok(Cond::Ne)
    }

    pub(super) fn select(
        &mut self,
        instruction: &Instruction,
    ) -> Selected<()> {
        let result = instruction.result;
        let cond = self.condition(instruction.operands[0])?;
        let (a, b) = (self.src(instruction.operands[1])?, self.src(instruction.operands[2])?);
        let value = match (&a, &b) {
            (Src::Float { .. } | Src::Val(Val::Float { .. }), _)
            | (_, Src::Float { .. } | Src::Val(Val::Float { .. })) => {
                let Val::Float { reg: a, size } = self.materialize(a)? else {
                    return refuse("a select of mixed classes");
                };
                let Val::Float { reg: b, .. } = self.materialize(b)? else {
                    return refuse("a select of mixed classes");
                };
                let rd = self.mf.fpr();
                self.inst(Inst::FpSelect { size, rd, rn: a, rm: b, cond });
                Val::Float { reg: rd, size }
            }
            _ => {
                let pointer = matches!(a, Src::Val(Val::Ptr(_))) || matches!(b, Src::Val(Val::Ptr(_)));
                let bits = match (&a, &b) {
                    (Src::Val(Val::Int(g)), _) | (_, Src::Val(Val::Int(g))) => g.bits,
                    (Src::Int { bits, .. }, _) => *bits,
                    _ => 64,
                };
                let bits = if pointer { 64 } else { bits };
                let width = width_of(bits);
                let rd = self.mf.gpr();
                let ones = mask(bits);
                let constant = |src: &Src| match src {
                    Src::Int { value, .. } => Some(*value as u64 & ones),
                    _ => None,
                };
                match (constant(&a), constant(&b)) {
                    // cset / csetm: the flags choose between 1 or -1 and 0.
                    (Some(1), Some(0)) => self.inst(Inst::CondSelect {
                        op: Select::Csinc,
                        width,
                        rd,
                        rn: Gpr::R31,
                        rm: Gpr::R31,
                        cond: cond.inverted(),
                    }),
                    (Some(0), Some(1)) => {
                        self.inst(Inst::CondSelect { op: Select::Csinc, width, rd, rn: Gpr::R31, rm: Gpr::R31, cond })
                    }
                    (Some(all), Some(0)) if all == ones => self.inst(Inst::CondSelect {
                        op: Select::Csinv,
                        width,
                        rd,
                        rn: Gpr::R31,
                        rm: Gpr::R31,
                        cond: cond.inverted(),
                    }),
                    (Some(0), Some(all)) if all == ones => {
                        self.inst(Inst::CondSelect { op: Select::Csinv, width, rd, rn: Gpr::R31, rm: Gpr::R31, cond })
                    }
                    _ => {
                        let first = self.gpr_or_zero(a)?;
                        let second = self.gpr_or_zero(b)?;
                        self.inst(Inst::CondSelect { op: Select::Csel, width, rd, rn: first, rm: second, cond });
                    }
                }
                let known = if bits <= 32 { Known::Hi32Zero } else { Known::Nothing };
                if pointer { Val::Ptr(AddrExpr::of(Base::Reg(rd))) } else { Val::Int(G { reg: rd, bits, known }) }
            }
        };
        self.define(result, Src::Val(value))
    }

    /// The register of `src`, the zero register for the constant 0.
    pub(crate) fn gpr_or_zero(
        &mut self,
        src: Src,
    ) -> Selected<Gpr> {
        Ok(match src {
            Src::Int { value: 0, .. } => Gpr::R31,
            other => self.gpr_of(other)?.reg,
        })
    }
}

fn instruction_name(op: BinaryOp) -> String {
    format!("{op:?}").to_lowercase()
}
