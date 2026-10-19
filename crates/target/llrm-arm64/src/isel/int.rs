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
        let (lhs, rhs) = (self.src(instruction.operands[0])?, self.src(instruction.operands[1])?);
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
            if let Some(k) = a_shift(value - 1) {
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
            if let Some(k) = a_shift(value + 1) {
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

    /// Whether the comparison `value` is read only by the branch or select that
    /// ends or follows it in its own block, so it is selected there. A float
    /// comparison needing two conditions is a value.
    pub(super) fn fusable(
        &self,
        value: llrm_mir::ValueId,
    ) -> bool {
        if self.use_count(value) != 1 {
            return false;
        }
        if let Some(inst) = super::value_def(self.function, value)
            && let llrm_mir::Opcode::FCmp(predicate) = self.function.instruction(inst).opcode
            && super::fp::conditions(predicate).1.is_some()
        {
            return false;
        }
        let Some(user) = self.function.users(value).into_iter().next() else { return false };
        let reader = self.function.instruction(user.user);
        let own = super::value_def(self.function, value).and_then(|inst| self.function.parent(inst));
        own == self.function.parent(user.user)
            && match reader.opcode {
                llrm_mir::Opcode::Br => reader.operands.len() == 3 && reader.operands[0] == Operand::Value(value),
                llrm_mir::Opcode::Select => reader.operands[0] == Operand::Value(value),
                _ => false,
            }
    }

    /// Sets the flags for `a predicate b` and names the condition that holds.
    pub(crate) fn compare(
        &mut self,
        predicate: IntPredicate,
        a: Operand,
        b: Operand,
    ) -> Selected<Cond> {
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
        if let Src::Int { value, .. } = right {
            // `x cmp c` as `subs` of the constant, or `adds` of its negation.
            // The constant is extended as the register is, so the
            // flags agree.
            let constant = if signed { llrm_mir::context::signed(value, bits) as i64 as u64 } else { value as u64 };
            let constant = constant & mask(width.bits());
            let signed_constant = if width == Width::W { i64::from(constant as u32 as i32) } else { constant as i64 };
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
                    rn: l.reg,
                    imm12,
                    shift12,
                });
                return Ok(condition_of(predicate));
            }
            let reg = self.mf.gpr();
            self.load_immediate(reg, bits, constant);
            self.inst(Inst::AddSubShifted {
                op: AddSub { subtract: true, set_flags: true },
                width,
                rd: Gpr::R31,
                rn: l.reg,
                rm: reg,
                shift: Shift::Lsl,
                amount: 0,
            });
            return Ok(condition_of(predicate));
        }
        let r = self.gpr_of(right)?;
        let r = self.extended(G { bits, ..r }, want);
        self.inst(Inst::AddSubShifted {
            op: AddSub { subtract: true, set_flags: true },
            width,
            rd: Gpr::R31,
            rn: l.reg,
            rm: r.reg,
            shift: Shift::Lsl,
            amount: 0,
        });
        Ok(condition_of(predicate))
    }

    /// The condition that holds where the i1 `operand` is true, with the flags
    /// set for it.
    pub(crate) fn condition(
        &mut self,
        operand: Operand,
    ) -> Selected<Cond> {
        if let Operand::Value(value) = operand
            && self.fusable(value)
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
        let value = match self.materialize(a)? {
            Val::Float { reg: a, size } => {
                let Val::Float { reg: b, .. } = self.materialize(b)? else {
                    return refuse("a select of mixed classes");
                };
                let rd = self.mf.fpr();
                self.inst(Inst::FpSelect { size, rd, rn: a, rm: b, cond });
                Val::Float { reg: rd, size }
            }
            first => {
                let second = self.materialize(b)?;
                let pointer = matches!(first, Val::Ptr(_));
                let (a, b) = (self.gpr_of(Src::Val(first))?, self.gpr_of(Src::Val(second))?);
                let bits = if pointer { 64 } else { a.bits };
                let rd = self.mf.gpr();
                self.inst(Inst::CondSelect { op: Select::Csel, width: width_of(bits), rd, rn: a.reg, rm: b.reg, cond });
                let known = if bits <= 32 { Known::Hi32Zero } else { Known::Nothing };
                if pointer { Val::Ptr(AddrExpr::of(Base::Reg(rd))) } else { Val::Int(G { reg: rd, bits, known }) }
            }
        };
        self.define(result, Src::Val(value))
    }
}

fn instruction_name(op: BinaryOp) -> String {
    format!("{op:?}").to_lowercase()
}
