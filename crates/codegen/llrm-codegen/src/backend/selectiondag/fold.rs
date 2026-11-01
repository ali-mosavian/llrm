//! The folds of `getNode` (constant folding, identities, undef and poison
//! operands), `FoldConstantArithmetic`, `FoldSetCC`, `simplifyShift` and
//! `simplifySelect`.

use super::dag::{Folded, NodeData, SDNodeFlags, SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::mvt::EVT;

/// The low `bits` bits set.
pub fn mask(bits: u32) -> u128 {
    if bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 }
}

/// `bits` as a two's-complement number `width` bits wide.
fn signed(
    bits: u128,
    width: u32,
) -> i128 {
    let shift = 128 - width;
    ((bits << shift) as i128) >> shift
}

fn extend_signed(
    bits: u128,
    from: u32,
    to: u32,
) -> u128 {
    (signed(bits, from) as u128) & mask(to)
}

/// `APInt::getNumSignBits`.
fn sign_bits(
    bits: u128,
    width: u32,
) -> u32 {
    let x = if signed(bits, width) < 0 { !bits } else { bits } & mask(width);
    x.leading_zeros() - (128 - width)
}

/// `FoldValue`: a binary opcode over two constants of `width` bits.
fn fold_value(
    opcode: ISD,
    c1: u128,
    c2: u128,
    width: u32,
) -> Option<u128> {
    let m = mask(width);
    let (s1, s2) = (signed(c1, width), signed(c2, width));
    let amount = if c2 >= width as u128 { width } else { c2 as u32 };
    let result = match opcode {
        ISD::Add => c1.wrapping_add(c2),
        ISD::Sub => c1.wrapping_sub(c2),
        ISD::Mul => c1.wrapping_mul(c2),
        ISD::And => c1 & c2,
        ISD::Or => c1 | c2,
        ISD::Xor => c1 ^ c2,
        ISD::Shl => c1.checked_shl(amount).unwrap_or(0),
        ISD::Srl => c1.checked_shr(amount).unwrap_or(0),
        ISD::Sra => (s1 >> amount.min(width - 1).min(127)) as u128,
        ISD::Rotl | ISD::Rotr => {
            let by = (c2 % width as u128) as u32;
            let by = if opcode == ISD::Rotl { by } else { (width - by) % width };
            if by == 0 { c1 } else { (c1 << by) | (c1 >> (width - by)) }
        }
        ISD::SMin => {
            if s1 <= s2 {
                c1
            } else {
                c2
            }
        }
        ISD::SMax => {
            if s1 >= s2 {
                c1
            } else {
                c2
            }
        }
        ISD::UMin => c1.min(c2),
        ISD::UMax => c1.max(c2),
        ISD::SAddSat | ISD::SSubSat => {
            let exact = if opcode == ISD::SAddSat { s1.checked_add(s2) } else { s1.checked_sub(s2) };
            let (lo, hi) = (signed(1u128 << (width - 1), width), signed(mask(width - 1), width));
            match exact {
                Some(value) if width < 128 => value.clamp(lo, hi) as u128,
                Some(value) => value as u128,
                None => (if s1 < 0 { lo } else { hi }) as u128,
            }
        }
        ISD::UAddSat => c1.checked_add(c2).filter(|sum| *sum <= m).unwrap_or(m),
        ISD::USubSat => c1.saturating_sub(c2),
        ISD::UDiv => c1.checked_div(c2)?,
        ISD::URem => c1.checked_rem(c2)?,
        ISD::SDiv => {
            if c2 == 0 {
                return None;
            }
            s1.wrapping_div(s2) as u128
        }
        ISD::SRem => {
            if c2 == 0 {
                return None;
            }
            s1.wrapping_rem(s2) as u128
        }
        ISD::MulHU if width <= 64 => (c1 * c2) >> width,
        ISD::MulHS if width <= 64 => ((s1 * s2) >> width) as u128,
        _ => return None,
    };
    Some(result & m)
}

impl SelectionDAG {
    /// `simplifyShift`.
    fn simplify_shift(
        &mut self,
        x: SDValue,
        y: SDValue,
    ) -> Option<SDValue> {
        let vt = self.value_type(x);
        if self.is_undef(x) {
            return Some(self.get_constant(0, vt));
        }
        if self.is_undef(y) {
            return Some(self.get_undef(vt));
        }
        if self.is_null_constant(x) || self.is_null_constant(y) {
            return Some(x);
        }
        if self.constant(y).is_some_and(|amount| amount >= vt.size_in_bits() as u128) {
            return Some(self.get_undef(vt));
        }
        (vt == EVT::I1).then_some(x)
    }

    /// `simplifySelect`; `freeze` of a maybe-poison arm is SD12.
    fn simplify_select(
        &mut self,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
    ) -> Option<SDValue> {
        if self.is_undef(cond) {
            return Some(if self.constant(then).is_some() { then } else { other });
        }
        if self.is_undef(then) {
            return Some(other);
        }
        if self.is_undef(other) {
            return Some(then);
        }
        if let Some(value) = self.bool_constant(cond) {
            return Some(if value { then } else { other });
        }
        (then == other).then_some(then)
    }

    pub(super) fn fold_unary(
        &mut self,
        opcode: ISD,
        vt: EVT,
        n1: SDValue,
        flags: SDNodeFlags,
    ) -> Folded {
        if let Some(value) = self.fold_constant_arithmetic(opcode, vt, &[n1]) {
            return Folded::Done(value);
        }
        let op_opcode = self.opcode(n1);
        let n1_type = self.value_type(n1);
        match opcode {
            ISD::TokenFactor | ISD::MergeValues => return Folded::Done(n1),
            ISD::SignExtend => {
                if n1_type == vt {
                    return Folded::Done(n1);
                }
                if matches!(op_opcode, ISD::SignExtend | ISD::ZeroExtend) {
                    let inner_flags = if op_opcode == ISD::ZeroExtend {
                        self.node(n1).flags.intersect(SDNodeFlags::NON_NEG)
                    } else {
                        SDNodeFlags::default()
                    };
                    let inner = self.operand(n1, 0);
                    return Folded::Done(self.get_node_flags(op_opcode, vt, &[inner], inner_flags));
                }
                if op_opcode == ISD::Poison {
                    return Folded::Done(self.get_poison(vt));
                }
                if op_opcode == ISD::Undef {
                    return Folded::Done(self.get_constant(0, vt));
                }
            }
            ISD::ZeroExtend => {
                if n1_type == vt {
                    return Folded::Done(n1);
                }
                if op_opcode == ISD::ZeroExtend {
                    let inner_flags = self.node(n1).flags.intersect(SDNodeFlags::NON_NEG);
                    let inner = self.operand(n1, 0);
                    return Folded::Done(self.get_node_flags(ISD::ZeroExtend, vt, &[inner], inner_flags));
                }
                if op_opcode == ISD::Poison {
                    return Folded::Done(self.get_poison(vt));
                }
                if op_opcode == ISD::Undef {
                    return Folded::Done(self.get_constant(0, vt));
                }
            }
            ISD::AnyExtend => {
                if n1_type == vt {
                    return Folded::Done(n1);
                }
                if matches!(op_opcode, ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend) {
                    let inner_flags = if op_opcode == ISD::ZeroExtend {
                        self.node(n1).flags.intersect(SDNodeFlags::NON_NEG)
                    } else {
                        SDNodeFlags::default()
                    };
                    let inner = self.operand(n1, 0);
                    return Folded::Done(self.get_node_flags(op_opcode, vt, &[inner], inner_flags));
                }
                if self.is_undef(n1) {
                    return Folded::Done(self.get_undef(vt));
                }
                if op_opcode == ISD::Truncate && self.value_type(self.operand(n1, 0)) == vt {
                    return Folded::Done(self.operand(n1, 0));
                }
            }
            ISD::Truncate => {
                if n1_type == vt {
                    return Folded::Done(n1);
                }
                if op_opcode == ISD::Truncate {
                    let inner = self.operand(n1, 0);
                    return Folded::Done(self.get_node(ISD::Truncate, vt, &[inner]));
                }
                if matches!(op_opcode, ISD::ZeroExtend | ISD::SignExtend | ISD::AnyExtend) {
                    let inner = self.operand(n1, 0);
                    let inner_type = self.value_type(inner);
                    if inner_type.bits_lt(vt) {
                        let inner_flags = if op_opcode == ISD::ZeroExtend {
                            self.node(n1).flags.intersect(SDNodeFlags::NON_NEG)
                        } else {
                            SDNodeFlags::default()
                        };
                        return Folded::Done(self.get_node_flags(op_opcode, vt, &[inner], inner_flags));
                    }
                    if inner_type.bits_gt(vt) {
                        return Folded::Done(self.get_node(ISD::Truncate, vt, &[inner]));
                    }
                    return Folded::Done(inner);
                }
                if self.is_undef(n1) {
                    return Folded::Done(self.get_undef(vt));
                }
            }
            ISD::Abs if self.is_undef(n1) => return Folded::Done(self.get_constant(0, vt)),
            ISD::BSwap => {
                if self.is_undef(n1) {
                    return Folded::Done(self.get_undef(vt));
                }
                if op_opcode == ISD::BSwap {
                    return Folded::Done(self.operand(n1, 0));
                }
            }
            ISD::BitReverse if self.is_undef(n1) => return Folded::Done(self.get_undef(vt)),
            ISD::BitCast => {
                if n1_type == vt {
                    return Folded::Done(n1);
                }
                if op_opcode == ISD::BitCast {
                    let inner = self.operand(n1, 0);
                    return Folded::Done(self.get_node(ISD::BitCast, vt, &[inner]));
                }
                if self.is_undef(n1) {
                    return Folded::Done(self.get_undef(vt));
                }
            }
            ISD::FNeg => {
                if self.is_undef(n1) {
                    return Folded::Done(self.get_undef(vt));
                }
                if op_opcode == ISD::FNeg {
                    return Folded::Done(self.operand(n1, 0));
                }
            }
            ISD::FAbs if op_opcode == ISD::FNeg => {
                let inner = self.operand(n1, 0);
                return Folded::Done(self.get_node(ISD::FAbs, vt, &[inner]));
            }
            // `isGuaranteedNotToBeUndefOrPoison` (DEVIATE SD101)
            ISD::Freeze if matches!(op_opcode, ISD::Constant | ISD::ConstantFP | ISD::Freeze) => {
                return Folded::Done(n1);
            }
            ISD::FpExtend if n1_type == vt => return Folded::Done(n1),
            ISD::FpToSInt | ISD::FpToUInt if self.is_undef(n1) => return Folded::Done(self.get_undef(vt)),
            _ => {}
        }
        let _ = flags;
        Folded::Keep(vec![n1])
    }

    pub(super) fn fold_binary(
        &mut self,
        opcode: ISD,
        vt: EVT,
        mut n1: SDValue,
        mut n2: SDValue,
        flags: SDNodeFlags,
    ) -> Folded {
        // canonicalizeCommutativeBinop: constant to the right
        if opcode.is_commutative_binop() && self.constant(n1).is_some() && self.constant(n2).is_none() {
            std::mem::swap(&mut n1, &mut n2);
        }
        let n2_const = self.constant(n2);
        let is_i1 = vt == EVT::I1;
        match opcode {
            ISD::TokenFactor => {
                if self.opcode(n1) == ISD::EntryToken || n1 == n2 {
                    return Folded::Done(if self.opcode(n1) == ISD::EntryToken { n2 } else { n1 });
                }
                if self.opcode(n2) == ISD::EntryToken {
                    return Folded::Done(n1);
                }
            }
            ISD::And => {
                if n2_const == Some(0) {
                    return Folded::Done(n2);
                }
                if n2_const == Some(mask(vt.size_in_bits())) {
                    return Folded::Done(n1);
                }
            }
            ISD::Or | ISD::Xor | ISD::Add | ISD::PtrAdd | ISD::Sub => {
                if n2_const == Some(0) {
                    return Folded::Done(n1);
                }
                if matches!(opcode, ISD::Add | ISD::Sub) && is_i1 {
                    return Folded::Done(self.get_node(ISD::Xor, vt, &[n1, n2]));
                }
            }
            ISD::Mul => {
                if is_i1 {
                    return Folded::Done(self.get_node(ISD::And, vt, &[n1, n2]));
                }
                if n2_const == Some(0) {
                    return Folded::Done(n2);
                }
            }
            ISD::SAddSat | ISD::UAddSat if is_i1 => return Folded::Done(self.get_node(ISD::Or, vt, &[n1, n2])),
            ISD::SSubSat | ISD::USubSat if is_i1 => {
                let not = self.get_not(n2, vt);
                return Folded::Done(self.get_node(ISD::And, vt, &[n1, not]));
            }
            ISD::SMin | ISD::UMax if is_i1 => return Folded::Done(self.get_node(ISD::Or, vt, &[n1, n2])),
            ISD::SMax | ISD::UMin if is_i1 => return Folded::Done(self.get_node(ISD::And, vt, &[n1, n2])),
            ISD::Shl | ISD::Sra | ISD::Srl | ISD::Rotl | ISD::Rotr => {
                if matches!(opcode, ISD::Shl | ISD::Sra | ISD::Srl) {
                    if let Some(value) = self.simplify_shift(n1, n2) {
                        return Folded::Done(value);
                    }
                }
                if is_i1 || n2_const == Some(0) {
                    return Folded::Done(n1);
                }
            }
            ISD::SignExtendInReg => {
                let NodeData::ValueType(from) = self.node(n2).data else {
                    panic!("SIGN_EXTEND_INREG needs a VALUETYPE operand")
                };
                if from == vt {
                    return Folded::Done(n1);
                }
            }
            ISD::ExtractElement => {
                let index = n2_const.expect("EXTRACT_ELEMENT index");
                if self.opcode(n1) == ISD::BuildPair {
                    return Folded::Done(self.operand(n1, index as usize));
                }
                if let Some(value) = self.constant(n1) {
                    let size = vt.size_in_bits();
                    return Folded::Done(self.get_constant(value >> (size * index as u32), vt));
                }
            }
            _ => {}
        }

        let (poison1, poison2) = (self.opcode(n1) == ISD::Poison, self.opcode(n2) == ISD::Poison);
        if poison1 || poison2 {
            use ISD::*;
            if matches!(
                opcode,
                Xor | Add
                    | PtrAdd
                    | Sub
                    | SignExtendInReg
                    | UDiv
                    | SDiv
                    | URem
                    | SRem
                    | Mul
                    | And
                    | SSubSat
                    | USubSat
                    | UMin
                    | Or
                    | SAddSat
                    | UAddSat
                    | UMax
                    | SMax
                    | SMin
            ) {
                return Folded::Done(if poison2 { n2 } else { n1 });
            }
        }

        if self.opcode(n1) == ISD::Undef && self.opcode(n2) != ISD::Undef {
            if opcode.is_commutative_binop() {
                std::mem::swap(&mut n1, &mut n2);
            } else {
                match opcode {
                    ISD::PtrAdd | ISD::Sub => return Folded::Done(n1),
                    ISD::SignExtendInReg
                    | ISD::UDiv
                    | ISD::SDiv
                    | ISD::URem
                    | ISD::SRem
                    | ISD::SSubSat
                    | ISD::USubSat => return Folded::Done(self.get_constant(0, vt)),
                    _ => {}
                }
            }
        }

        if self.opcode(n2) == ISD::Undef {
            let both = self.opcode(n1) == ISD::Undef;
            let value = match opcode {
                ISD::Xor if both => Some(self.get_constant(0, vt)),
                ISD::Xor | ISD::Add | ISD::PtrAdd | ISD::Sub => Some(n2),
                ISD::UDiv | ISD::SDiv | ISD::URem | ISD::SRem => Some(self.get_poison(vt)),
                ISD::Mul | ISD::And | ISD::SSubSat | ISD::USubSat | ISD::UMin => {
                    Some(if both { n2 } else { self.get_constant(0, vt) })
                }
                ISD::Or | ISD::SAddSat | ISD::UAddSat | ISD::UMax => {
                    Some(if both { n2 } else { self.get_all_ones_constant(vt) })
                }
                ISD::SMax => Some(if both { n2 } else { self.get_constant(mask(vt.size_in_bits() - 1), vt) }),
                ISD::SMin => Some(if both { n2 } else { self.get_constant(1u128 << (vt.size_in_bits() - 1), vt) }),
                _ => None,
            };
            if let Some(value) = value {
                return Folded::Done(value);
            }
        }

        if let Some(value) = self.fold_constant_arithmetic(opcode, vt, &[n1, n2]) {
            return Folded::Done(value);
        }
        let _ = flags;
        Folded::Keep(vec![n1, n2])
    }

    pub(super) fn fold_ternary(
        &mut self,
        opcode: ISD,
        vt: EVT,
        n1: SDValue,
        n2: SDValue,
        n3: SDValue,
    ) -> Folded {
        match opcode {
            ISD::SetCC => {
                let NodeData::CondCode(cond) = self.node(n3).data else { panic!("SETCC needs a CONDCODE operand") };
                if let Some(value) = self.fold_setcc(vt, n1, n2, cond) {
                    return Folded::Done(value);
                }
            }
            ISD::Select => {
                if let Some(value) = self.simplify_select(n1, n2, n3) {
                    return Folded::Done(value);
                }
            }
            ISD::FShl | ISD::FShr => {
                if let Some(value) = self.fold_constant_arithmetic(opcode, vt, &[n1, n2, n3]) {
                    return Folded::Done(value);
                }
            }
            _ => {}
        }
        Folded::Keep(vec![n1, n2, n3])
    }

    /// `FoldSetCC`, integers only (floating-point constants are SD11).
    fn fold_setcc(
        &mut self,
        vt: EVT,
        n1: SDValue,
        n2: SDValue,
        cond: CondCode,
    ) -> Option<SDValue> {
        use CondCode::*;
        let op_type = self.value_type(n1);
        match cond {
            SetFalse | SetFalse2 => return Some(self.get_bool_constant(false, vt)),
            SetTrue | SetTrue2 => return Some(self.get_bool_constant(true, vt)),
            _ => {}
        }
        if !op_type.is_integer() {
            return None;
        }
        let (undef1, undef2) = (self.is_undef(n1), self.is_undef(n2));
        if (undef1 || undef2) && matches!(cond, SetEq | SetNe) || undef1 && undef2 {
            return Some(if vt == EVT::I1 { self.get_undef(vt) } else { self.get_constant(0, vt) });
        }
        if undef1 || undef2 || n1 == n2 {
            return Some(self.get_bool_constant(cond.true_when_equal(), vt));
        }
        let (c1, c2) = (self.constant(n1)?, self.constant(n2)?);
        let width = op_type.size_in_bits();
        let (s1, s2) = (signed(c1, width), signed(c2, width));
        let result = match cond {
            SetEq => c1 == c2,
            SetNe => c1 != c2,
            SetGt => s1 > s2,
            SetGe => s1 >= s2,
            SetLt => s1 < s2,
            SetLe => s1 <= s2,
            SetUGt => c1 > c2,
            SetUGe => c1 >= c2,
            SetULt => c1 < c2,
            SetULe => c1 <= c2,
            _ => panic!("floating-point condition {cond:?} on an integer"),
        };
        Some(self.get_bool_constant(result, vt))
    }

    /// `FoldConstantArithmetic`: operands that are constants, a symbol plus
    /// an offset, or a division by zero.
    fn fold_constant_arithmetic(
        &mut self,
        opcode: ISD,
        vt: EVT,
        ops: &[SDValue],
    ) -> Option<SDValue> {
        if matches!(opcode, ISD::Target(_) | ISD::Machine(_)) {
            return None;
        }
        if matches!(opcode, ISD::SDiv | ISD::UDiv | ISD::SRem | ISD::URem)
            && (self.is_undef(ops[1]) || self.is_null_constant(ops[1]))
        {
            return Some(self.get_undef(vt));
        }
        if vt.is_floating_point() {
            return match *ops {
                [n1, n2] => self.fold_constant_fp(opcode, vt, n1, n2),
                _ => None,
            };
        }
        if !vt.is_integer() {
            return None;
        }
        let width = vt.size_in_bits();
        match *ops {
            [n1] => {
                let bits = self.constant(n1)?;
                let opaque = self.is_opaque(n1);
                let target = self.opcode(n1) == ISD::TargetConstant;
                let from = self.width(n1);
                let value = match opcode {
                    ISD::SignExtend => extend_signed(bits, from, width),
                    ISD::Truncate if opaque => return None,
                    ISD::Truncate | ISD::ZeroExtend | ISD::AnyExtend => bits,
                    ISD::Abs => signed(bits, from).unsigned_abs(),
                    ISD::BitReverse => bits.reverse_bits() >> (128 - from),
                    ISD::BSwap => bits.swap_bytes() >> (128 - from),
                    ISD::CtPop => bits.count_ones() as u128,
                    ISD::Parity => (bits.count_ones() & 1) as u128,
                    ISD::CtLz | ISD::CtLzZeroPoison => (bits.leading_zeros() - (128 - from)) as u128,
                    ISD::CtTz | ISD::CtTzZeroPoison => bits.trailing_zeros().min(from) as u128,
                    ISD::CtLs => (sign_bits(bits, from) - 1) as u128,
                    _ => return None,
                };
                Some(self.get_constant_with(value, vt, target, opaque))
            }
            [n1, n2] => {
                if let (Some(c1), Some(c2)) = (self.constant(n1), self.constant(n2)) {
                    if self.is_opaque(n1) || self.is_opaque(n2) {
                        return None;
                    }
                    let value = fold_value(opcode, c1, c2, width)?;
                    return Some(self.get_constant(value, vt));
                }
                if let NodeData::Global { symbol, offset } = self.node(n1).data {
                    return self.fold_symbol_offset(opcode, vt, self.opcode(n1), symbol, offset, n2);
                }
                if opcode.is_commutative_binop() {
                    if let NodeData::Global { symbol, offset } = self.node(n2).data {
                        return self.fold_symbol_offset(opcode, vt, self.opcode(n2), symbol, offset, n1);
                    }
                }
                if opcode == ISD::SignExtendInReg {
                    let c1 = self.constant(n1)?;
                    let NodeData::ValueType(from) = self.node(n2).data else { return None };
                    let from = from.size_in_bits();
                    return Some(self.get_constant(extend_signed(c1 & mask(from), from, width), vt));
                }
                None
            }
            [n1, n2, n3] if matches!(opcode, ISD::FShl | ISD::FShr) => {
                let (c1, c2, c3) = (self.constant(n1)?, self.constant(n2)?, self.constant(n3)?);
                if [n1, n2, n3].iter().any(|op| self.is_opaque(*op)) {
                    return None;
                }
                let amount = (c3 % width as u128) as u32;
                let value = match (opcode, amount) {
                    (ISD::FShl, 0) => c1,
                    (ISD::FShl, _) => (c1 << amount) | (c2 >> (width - amount)),
                    (_, 0) => c2,
                    _ => (c2 >> amount) | (c1 << (width - amount)),
                };
                Some(self.get_constant(value, vt))
            }
            _ => None,
        }
    }

    /// `foldConstantFPMath`: two constants fold to one; an undef operand
    /// makes the result undef (both) or a NaN (one).
    pub(super) fn fold_constant_fp(
        &mut self,
        opcode: ISD,
        vt: EVT,
        n1: SDValue,
        n2: SDValue,
    ) -> Option<SDValue> {
        if !matches!(opcode, ISD::FAdd | ISD::FSub | ISD::FMul | ISD::FDiv) {
            return None;
        }
        if let (NodeData::ConstFp { bits: a }, NodeData::ConstFp { bits: b }) = (self.node(n1).data, self.node(n2).data)
        {
            let folded = if self.config.fp_extended {
                super::fp_value::fold_exact(opcode, vt, a, b)
            } else {
                super::fp_value::fold(opcode, vt, a, b)
            };
            return folded.map(|bits| self.get_constant_fp(bits, vt));
        }
        let (undef1, undef2) = (self.is_undef(n1), self.is_undef(n2));
        // -0.0 - undef --> undef (consistent with "fneg undef")
        if opcode == ISD::FSub && undef2 {
            if let NodeData::ConstFp { bits } = self.node(n1).data {
                if super::fp_value::classify(vt, bits).is_some_and(|c| c.zero && c.negative) {
                    return Some(self.get_undef(vt));
                }
            }
        }
        if undef1 && undef2 {
            return Some(self.get_undef(vt));
        }
        if undef1 || undef2 {
            return super::fp_value::quiet_nan(vt).map(|bits| self.get_constant_fp(bits, vt));
        }
        None
    }

    /// `FoldSymbolOffset`: `(add Sym, c)` and `(sub Sym, c)` as `Sym+c`.
    fn fold_symbol_offset(
        &mut self,
        opcode: ISD,
        vt: EVT,
        global: ISD,
        symbol: u32,
        offset: i64,
        other: SDValue,
    ) -> Option<SDValue> {
        if global != ISD::GlobalAddress
            || !self.config.offset_folding
            || !self.local_symbols.get(symbol as usize).copied().unwrap_or(true)
        {
            return None;
        }
        let amount = signed(self.constant(other)?, self.width(other)) as i64;
        let amount = match opcode {
            ISD::Add | ISD::PtrAdd => amount,
            ISD::Sub => amount.wrapping_neg(),
            _ => return None,
        };
        Some(self.get_global_address(vt, symbol, offset.wrapping_add(amount)))
    }
}
