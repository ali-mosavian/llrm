//! The arithmetic intrinsics (`visitIntrinsicCall`, `visitUnaryIntrinsic`):
//! integer min and max, bit counts, `bswap`, `abs`, and the float functions
//! and `lrint`, each one node on its operands. The legalizer turns what the
//! target lacks into library calls.

use llrm_mir::context::GlobalId;
use llrm_mir::intrinsics::{BitCount, FloatFunction, Intrinsic, ReduceKind};
use llrm_mir::libfunc::{Generic, LibFunc, TargetLibraryInfo};
use llrm_mir::module::{GlobalKind, Instruction, Linkage, Operand};
use llrm_mir::opcode::{Attribute, BinaryOp};

use super::super::dag::SDValue;
use super::super::isd::ISD;
use super::super::mvt::{EVT, MVT};
use super::{Builder, Refused, flags_of};

impl Builder<'_> {
    /// `SelectionDAGBuilder::visitVectorReduce`
    /// ([L11286](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L11286)):
    /// the integer kinds are one `VECREDUCE_*` node on the vector. The float
    /// kinds (`VECREDUCE_FADD`, `VECREDUCE_SEQ_FADD`, ...) are refused by name.
    pub(super) fn visit_vector_reduce(
        &mut self,
        instruction: &Instruction,
        kind: ReduceKind,
    ) -> Result<(), Refused> {
        let code = match kind {
            ReduceKind::Add => ISD::VecReduceAdd,
            ReduceKind::Mul => ISD::VecReduceMul,
            ReduceKind::And => ISD::VecReduceAnd,
            ReduceKind::Or => ISD::VecReduceOr,
            ReduceKind::Xor => ISD::VecReduceXor,
            ReduceKind::SMax => ISD::VecReduceSMax,
            ReduceKind::SMin => ISD::VecReduceSMin,
            ReduceKind::UMax => ISD::VecReduceUMax,
            ReduceKind::UMin => ISD::VecReduceUMin,
            ReduceKind::FAdd | ReduceKind::FMul => return Err(Refused("a floating-point vector reduction")),
        };
        let vector = self.get_value(instruction.operands[0])?;
        let vt = self.type_of(instruction.ty)?;
        let node = self.dag.get_node(code, vt, &[vector]);
        self.set_value(instruction.result.ok_or(Refused("a vector reduction without a result"))?, node);
        Ok(())
    }

    pub(super) fn visit_math(
        &mut self,
        instruction: &Instruction,
        intrinsic: &Intrinsic,
    ) -> Result<(), Refused> {
        let flags = flags_of(instruction.flags);
        let first = self.get_value(instruction.operands[0])?;
        // `{iN, i1}`: the node's two results, which `extractvalue` picks from
        if let Intrinsic::WithOverflow { op, signed } = intrinsic {
            let second = self.get_value(instruction.operands[1])?;
            let code = match (op, signed) {
                (BinaryOp::Add, false) => ISD::UAddO,
                (BinaryOp::Add, true) => ISD::SAddO,
                (BinaryOp::Sub, false) => ISD::USubO,
                (BinaryOp::Sub, true) => ISD::SSubO,
                (BinaryOp::Mul, false) => ISD::UMulO,
                (BinaryOp::Mul, true) => ISD::SMulO,
                _ => return Err(Refused("overflow of an operation with no overflow node")),
            };
            let vt = self.dag.value_type(first);
            let node = self.dag.get_node_multi(code, &[vt, EVT::I1], &[first, second]);
            self.set_value(instruction.result.ok_or(Refused("an overflow intrinsic without a result"))?, node);
            return Ok(());
        }
        let dest = self.type_of(instruction.ty)?;
        let node = match intrinsic {
            Intrinsic::Compare { signed } => {
                let second = self.get_value(instruction.operands[1])?;
                self.dag.get_node(if *signed { ISD::SCmp } else { ISD::UCmp }, dest, &[first, second])
            }
            // `visitIntrinsicCall`: an `FMA` where `isFMAFasterThanFMulAndFAdd`
            // (arm64); x86 has none before FMA3, so a multiply and an add
            Intrinsic::FMulAdd => {
                let second = self.get_value(instruction.operands[1])?;
                let third = self.get_value(instruction.operands[2])?;
                // AArch64's `isFMAFasterThanFMulAndFAdd` is true for f16, f32
                // and f64 only: an fp128 `fmuladd` is a multiply and an add.
                if self.dag.config().fma_faster && dest != EVT::Simple(MVT::F128) {
                    self.dag.get_node_flags(ISD::FMA, dest, &[first, second, third], flags)
                } else {
                    let product = self.dag.get_node_flags(ISD::FMul, dest, &[first, second], flags);
                    self.dag.get_node_flags(ISD::FAdd, dest, &[product, third], flags)
                }
            }
            Intrinsic::CopySign => {
                let second = self.get_value(instruction.operands[1])?;
                self.dag.get_node(ISD::FCopySign, dest, &[first, second])
            }
            Intrinsic::Fixed { divide: false } => {
                let second = self.get_value(instruction.operands[1])?;
                let scale = self.get_value(instruction.operands[2])?;
                self.dag.get_node(ISD::SMulFix, dest, &[first, second, scale])
            }
            Intrinsic::Fixed { divide: true } => {
                let second = self.get_value(instruction.operands[1])?;
                let scale = self.get_value(instruction.operands[2])?;
                self.expand_div_fix(first, second, scale)
            }
            Intrinsic::FixedMul { signed, saturating } => {
                let second = self.get_value(instruction.operands[1])?;
                let scale = self.get_value(instruction.operands[2])?;
                let code = match (*signed, *saturating) {
                    (true, _) => ISD::SMulFixSat,
                    (false, false) => ISD::UMulFix,
                    (false, true) => ISD::UMulFixSat,
                };
                self.dag.get_node(code, dest, &[first, second, scale])
            }
            Intrinsic::ShiftSat { signed } => {
                // The amount in the target's shift type, as the pin coerces
                // it (SelectionDAGBuilder.cpp#L7608).
                let amount = self.get_value(instruction.operands[1])?;
                let second =
                    self.dag.get_zext_or_trunc(amount, self.target.shift_amount_for(self.dag.value_type(first)));
                self.dag.get_node(if *signed { ISD::SShlSat } else { ISD::UShlSat }, dest, &[first, second])
            }
            Intrinsic::Saturating { op, signed } => {
                let second = self.get_value(instruction.operands[1])?;
                let code = match (op, signed) {
                    (BinaryOp::Add, true) => ISD::SAddSat,
                    (BinaryOp::Add, false) => ISD::UAddSat,
                    (_, true) => ISD::SSubSat,
                    (_, false) => ISD::USubSat,
                };
                self.dag.get_node(code, dest, &[first, second])
            }
            Intrinsic::MinMax { signed, max } => {
                let second = self.get_value(instruction.operands[1])?;
                let code = match (signed, max) {
                    (true, true) => ISD::SMax,
                    (true, false) => ISD::SMin,
                    (false, true) => ISD::UMax,
                    (false, false) => ISD::UMin,
                };
                self.dag.get_node(code, dest, &[first, second])
            }
            Intrinsic::BitCount(count) => {
                // The zero-is-poison flag picks the `_ZERO_UNDEF` form.
                let poison = !matches!(
                    instruction.operands.get(1),
                    Some(Operand::Constant(id)) if self.is_zero(*id)
                );
                let code = match (count, poison) {
                    (BitCount::Population, _) => ISD::CtPop,
                    (BitCount::LeadingZeros, false) => ISD::CtLz,
                    (BitCount::LeadingZeros, true) => ISD::CtLzZeroPoison,
                    (BitCount::TrailingZeros, false) => ISD::CtTz,
                    (BitCount::TrailingZeros, true) => ISD::CtTzZeroPoison,
                };
                self.dag.get_node(code, dest, &[first])
            }
            // `visitIntrinsicCall`: an AND with the mask taken to the pointer's
            // width
            Intrinsic::PtrMask => {
                let mask = self.get_value(instruction.operands[1])?;
                let mask = self.dag.get_zext_or_trunc(mask, dest);
                self.dag.get_node(ISD::And, dest, &[first, mask])
            }
            Intrinsic::ByteSwap => self.dag.get_node(ISD::BSwap, dest, &[first]),
            Intrinsic::BitReverse => self.dag.get_node(ISD::BitReverse, dest, &[first]),
            Intrinsic::Abs => {
                let poison = matches!(
                    instruction.operands.get(1),
                    Some(Operand::Constant(id)) if !self.is_zero(*id)
                );
                self.dag.get_node(if poison { ISD::AbsMinPoison } else { ISD::Abs }, dest, &[first])
            }
            Intrinsic::LRint => self.dag.get_node_flags(ISD::LRint, dest, &[first], flags),
            Intrinsic::LLRint => self.dag.get_node_flags(ISD::LLRint, dest, &[first], flags),
            Intrinsic::Unary(function) => {
                let code = match function {
                    FloatFunction::Fabs => ISD::FAbs,
                    FloatFunction::Sqrt => ISD::FSqrt,
                    FloatFunction::Sin => ISD::FSin,
                    FloatFunction::Cos => ISD::FCos,
                    FloatFunction::Atan => ISD::FAtan,
                    FloatFunction::Log2 => ISD::FLog2,
                    FloatFunction::Exp2 => ISD::FExp2,
                    FloatFunction::Log => ISD::FLog,
                    FloatFunction::Exp => ISD::FExp,
                    FloatFunction::Rint => ISD::FRint,
                    // Calls of the C library before selection (`libmcalls`).
                    FloatFunction::Floor
                    | FloatFunction::Ceil
                    | FloatFunction::Trunc
                    | FloatFunction::Round
                    | FloatFunction::RoundEven
                    | FloatFunction::NearbyInt => return Err(Refused("a rounding intrinsic without its library call")),
                };
                self.dag.get_node_flags(code, dest, &[first], flags)
            }
            _ => return Err(Refused("intrinsic call")),
        };
        self.set_value(instruction.result.ok_or(Refused("an arithmetic intrinsic without a result"))?, node);
        Ok(())
    }

    /// `expandDivFix` for `sdiv.fix`: x86 has no fixed-point divide, so the
    /// operands are widened by one bit and the node is promoted early (a scale
    /// of 0 stays at the type).
    fn expand_div_fix(
        &mut self,
        lhs: SDValue,
        rhs: SDValue,
        scale: SDValue,
    ) -> SDValue {
        let vt = self.dag.value_type(lhs);
        if self.dag.constant(scale).unwrap_or(0) == 0 {
            return self.dag.get_node(ISD::SDivFix, vt, &[lhs, rhs, scale]);
        }
        let wide = EVT::integer(vt.size_in_bits() + 1);
        let lhs = self.dag.get_sext_or_trunc(lhs, wide);
        let rhs = self.dag.get_sext_or_trunc(rhs, wide);
        let result = self.dag.get_node(ISD::SDivFix, wide, &[lhs, rhs, scale]);
        self.dag.get_zext_or_trunc(result, vt)
    }

    /// Whether the constant is the integer zero.
    fn is_zero(
        &self,
        id: llrm_mir::context::ConstantId,
    ) -> bool {
        matches!(
            self.module.context.get(id).kind,
            llrm_mir::context::ConstantKind::Int(0)
        )
    }
}

thread_local! {
    /// The library table of a pointer width: built once, not per call.
    static LIBRARY: std::cell::RefCell<Vec<(u32, std::rc::Rc<TargetLibraryInfo>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn library_for(pointer_bits: u32) -> std::rc::Rc<TargetLibraryInfo> {
    LIBRARY.with(|held| {
        let mut held = held.borrow_mut();
        if let Some((_, info)) = held.iter().find(|(bits, _)| *bits == pointer_bits) {
            return std::rc::Rc::clone(info);
        }
        let info = std::rc::Rc::new(TargetLibraryInfo::new(&Generic::for_pointer(pointer_bits)));
        held.push((pointer_bits, std::rc::Rc::clone(&info)));
        info
    })
}

/// The node a float library function is, and whether it takes one operand or
/// two (`SelectionDAGBuilder::visitCall`'s `LibFunc_*` cases,
/// [SelectionDAGBuilder.cpp#L9770-L9890](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9770)).
///
/// DEVIATE FL1: `tan`, `asin`, `acos`, `atan2`, `sinh`, `cosh`, `tanh`,
/// `exp10` and `ldexp` have no node in `ISD` yet, so they stay calls.
fn float_libcall(func: LibFunc) -> Option<(ISD, bool)> {
    use LibFunc::*;
    Some(match func {
        Copysign | Copysignf | Copysignl => (ISD::FCopySign, true),
        Sin | Sinf | Sinl => (ISD::FSin, false),
        Cos | Cosf | Cosl => (ISD::FCos, false),
        Atan | Atanf | Atanl => (ISD::FAtan, false),
        Sqrt | Sqrtf | Sqrtl | SqrtFinite | SqrtfFinite | SqrtlFinite => (ISD::FSqrt, false),
        Log2 | Log2f | Log2l => (ISD::FLog2, false),
        Exp2 | Exp2f | Exp2l => (ISD::FExp2, false),
        _ => return None,
    })
}

impl Builder<'_> {
    /// `visitCall`'s library-function cases for the float functions, with
    /// `visitUnaryFloatCall` and `visitBinaryFloatCall`
    /// ([L9709](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9709),
    /// [L9733](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/SelectionDAGBuilder.cpp#L9733)):
    /// a call that only reads memory (it sets no `errno`) is the node on its
    /// operands. Whether one did.
    ///
    /// DEVIATE FL2: the library table is the pin's defaults for the pointer's
    /// width, not the target's `Library` (`hasOptimizedCodeGen`'s
    /// `Unavailable` test reads it).
    pub(super) fn visit_float_libcall(
        &mut self,
        instruction: &Instruction,
        info: &llrm_mir::opcode::CallInfo,
        callee: GlobalId,
    ) -> Result<bool, Refused> {
        let global = self.module.global(callee);
        let callee_attrs: &[Attribute] = match &global.kind {
            GlobalKind::Function(function) => &function.attrs,
            _ => &[],
        };
        let has = |name: &str| {
            let flag =
                |attrs: &[Attribute]| attrs.iter().any(|one| matches!(one, Attribute::Flag(flag) if flag == name));
            flag(&info.attrs) || flag(callee_attrs)
        };
        // `!I.isNoBuiltin() && !F->hasLocalLinkage()`.
        if (has("nobuiltin") && !has("builtin")) || matches!(global.linkage, Linkage::Internal | Linkage::Private) {
            return Ok(false);
        }
        let library = library_for(self.layout.pointer(0).index_bits);
        let Some(func) = library.get_lib_func_of(&self.module.context.types, global) else { return Ok(false) };
        let Some((code, binary)) = float_libcall(func).filter(|_| library.has(func)) else { return Ok(false) };
        // "verify it doesn't modify errno", and no strict floating point.
        let reads_only =
            !llrm_mir::memory::stated(&info.attrs).writes || !llrm_mir::memory::stated(callee_attrs).writes;
        if !reads_only || has("strictfp") {
            return Ok(false);
        }
        let first = self.get_value(instruction.operands[0])?;
        let vt = self.dag.value_type(first);
        let node = if binary {
            // `copysign` takes no fast-math flags.
            let second = self.get_value(instruction.operands[1])?;
            self.dag.get_node(code, vt, &[first, second])
        } else {
            self.dag.get_node_flags(code, vt, &[first], flags_of(instruction.flags))
        };
        self.set_value(instruction.result.ok_or(Refused("a float library call without a result"))?, node);
        Ok(true)
    }
}
