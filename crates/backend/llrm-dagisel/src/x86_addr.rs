//! `X86ISelAddressMode` and the address matchers of `X86DAGToDAGISel`, for
//! 32-bit flat targets.
//!
//! # How it works
//! `selectAddr` and `selectLEAAddr` are the complex patterns the generated
//! table calls to turn a pointer expression into the five operands of an x86
//! memory reference: base, scale, index, displacement, segment. They run
//! [`Matcher::match_address`] ([L2053][addr]) which folds constants, wrapped
//! symbols, frame indexes, scaled indexes and adds into an [`AddressMode`];
//! [`Matcher::get_address_operands`] ([L270][ops]) writes the operands.
//! The matchers never change the DAG here, so the pin's `HandleSDNode` guards
//! are not needed.
//!
//! Numbered deviations from the pin (reason / owner / exit / review):
//! - **SD34** Not translated: `foldMaskedShiftToBEXTR` (no TBM or BMI here),
//!   the `any_extend` look-through of `foldMaskedShiftToScaledMask` (x32), and
//!   the `SIGN_EXTEND`/`ZERO_EXTEND` re-association of `matchIndexRecursively`
//!   and the `ZERO_EXTEND` case of `matchAddressRecursively`. `insertDAGNode`
//!   is nothing: the arena has no order to keep, and the pin's `HandleSDNode`
//!   guards are not modelled. Owner sdag-c. Exit: C3. Review 2026-12-10.
//! - **SD40** `MaskedValueIsZero` is [`known_zero`], a structural subset of
//!   `computeKnownBits` (constants, `AND` with a constant, zero-extension,
//!   constant shifts); the folds decline what it cannot prove. Owner sdag-c.
//!   Exit: when the DAG API has known bits. Review 2026-12-10.
//! - **SD35** The 64-bit and x32 branches, RIP-relative modes, large code
//!   model, the TLS segment loads (`matchLoadInAddress`) and the vector forms
//!   (`matchVectorAddress`) are left out: no such target here. m16 has its own
//!   matcher (C2). Owner sdag-c. Exit: none. Review 2026-12-10.
//!
//! # Used by
//! [`crate::x86_hooks`].
//!
//! [addr]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2053
//! [ops]:
//! https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L270

use crate::dag::{Dag, SDValue};
use crate::generated::ids::{isd, mvt, target_opcode, x86, x86isd};
use crate::matcher::Vt;

/// `SelectionDAG::MaxRecursionDepth`.
const MAX_RECURSION_DEPTH: u32 = 6;

/// The bits of an integer value type; 64 for any other.
pub fn vt_bits(vt: Vt) -> u32 {
    if vt == mvt::i1 {
        1
    } else if vt == mvt::i8 {
        8
    } else if vt == mvt::i16 {
        16
    } else if vt == mvt::i32 {
        32
    } else {
        64
    }
}

fn low_mask(bits: u32) -> u64 {
    if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 }
}

/// The bits of `v` known to be zero, within its width (a structural subset of
/// `computeKnownBits`: constants, `AND` with a constant, zero-extensions,
/// constant shifts; SD40).
pub fn known_zero<D: AddrDag>(
    dag: &D,
    v: SDValue,
    depth: u32,
) -> u64 {
    let width = vt_bits(dag.value_type(v));
    let all = low_mask(width);
    if depth >= 6 {
        return 0;
    }
    let constant = |x: SDValue| (dag.opcode(x.node) == isd::Constant).then(|| dag.constant(x)).flatten();
    let zero = match dag.opcode(v.node) {
        isd::Constant => !(constant(v).unwrap() as u64),
        isd::AND => {
            let rhs = constant(dag.operand(v.node, 1)).map_or(0, |c| !(c as u64));
            known_zero(dag, dag.operand(v.node, 0), depth + 1)
                | known_zero(dag, dag.operand(v.node, 1), depth + 1)
                | rhs
        }
        isd::ZERO_EXTEND => {
            let src = dag.operand(v.node, 0);
            let src_bits = vt_bits(dag.value_type(src));
            known_zero(dag, src, depth + 1) | (all & !low_mask(src_bits))
        }
        isd::SRL => match constant(dag.operand(v.node, 1)) {
            Some(k) if (0..64).contains(&k) => {
                let k = k as u32;
                (known_zero(dag, dag.operand(v.node, 0), depth + 1) >> k) | (all & !low_mask(width.saturating_sub(k)))
            }
            _ => 0,
        },
        isd::SHL => match constant(dag.operand(v.node, 1)) {
            Some(k) if (0..64).contains(&k) => {
                (known_zero(dag, dag.operand(v.node, 0), depth + 1) << k) | low_mask(k as u32)
            }
            _ => 0,
        },
        _ => 0,
    };
    zero & all
}

/// What a `Wrapper` wraps, as the pin's `GlobalAddressSDNode` and kin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Global,
    ConstantPool { align: u32 },
    External,
    McSymbol,
    JumpTable,
    BlockAddress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub kind: SymbolKind,
    /// The global, pool entry, name, label, table or block, in the DAG's own
    /// numbering.
    pub id: u32,
}

/// The operand of a `Wrapper`: the symbol, its offset and target flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wrapped {
    pub symbol: Symbol,
    pub offset: i64,
    pub flags: u8,
}

/// The DAG queries and constructors the address matchers need on top of
/// [`Dag`].
pub trait AddrDag: Dag {
    /// `SelectionDAG::isADDLike`: an `OR` of disjoint bits, or an `XOR` with
    /// the sign bit.
    fn is_add_like(
        &self,
        value: SDValue,
    ) -> bool;
    /// `FrameIndexSDNode::getIndex`.
    fn frame_index(
        &self,
        value: SDValue,
    ) -> Option<i32>;
    /// The symbol a `Wrapper`'s operand names (`TargetGlobalAddress`,
    /// `TargetConstantPool`, ...).
    fn wrapped_symbol(
        &self,
        operand: SDValue,
    ) -> Option<Wrapped>;
    /// `hasMaterializingUse`: a use that puts `value` in a register as a value.
    fn has_materializing_use(
        &self,
        value: SDValue,
    ) -> bool;
    /// `getNode(opcode, vt, ops)` for the opcodes the address folds make:
    /// `ADD`, `AND`, `SHL`, `SRL`, `ZERO_EXTEND`, `ANY_EXTEND`, `TRUNCATE`.
    fn get_node(
        &mut self,
        opcode: u16,
        vt: Vt,
        ops: &[SDValue],
    ) -> SDValue;
    /// `getConstant(value, vt)` (not a target constant).
    fn get_constant(
        &mut self,
        value: i64,
        vt: Vt,
    ) -> SDValue;
    /// The segment register a memory node addresses through (`X86::ES`, ...),
    /// the pin's address space 256..258 for GS, FS and SS; `None` for the
    /// default segment.
    fn segment_override(
        &self,
        node: u32,
    ) -> Option<u16> {
        let _ = node;
        None
    }
    /// `StoreSDNode::isNonTemporal`.
    fn is_non_temporal(
        &self,
        node: u32,
    ) -> bool;
    fn target_frame_index(
        &mut self,
        index: i32,
        vt: Vt,
    ) -> SDValue;
    /// `getTargetGlobalAddress`, `getTargetConstantPool` and the rest, by
    /// symbol kind.
    fn target_symbol(
        &mut self,
        symbol: Symbol,
        disp: i32,
        flags: u8,
    ) -> SDValue;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseType {
    Reg,
    FrameIndex,
}

/// `X86ISelAddressMode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressMode {
    pub base_type: BaseType,
    pub base_reg: Option<SDValue>,
    pub base_frame_index: i32,
    pub scale: u32,
    pub index_reg: Option<SDValue>,
    pub disp: i32,
    pub segment: Option<SDValue>,
    pub symbol: Option<Symbol>,
    pub symbol_flags: u8,
    pub negate_index: bool,
    pub is_for_lea: bool,
}

impl Default for AddressMode {
    fn default() -> AddressMode {
        AddressMode {
            base_type: BaseType::Reg,
            base_reg: None,
            base_frame_index: 0,
            scale: 1,
            index_reg: None,
            disp: 0,
            segment: None,
            symbol: None,
            symbol_flags: 0,
            negate_index: false,
            is_for_lea: false,
        }
    }
}

impl AddressMode {
    pub fn has_symbolic_displacement(&self) -> bool {
        self.symbol.is_some()
    }

    pub fn has_base_or_index_reg(&self) -> bool {
        self.base_type == BaseType::FrameIndex || self.index_reg.is_some() || self.base_reg.is_some()
    }
}

/// The five operands of a memory reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressOperands {
    pub base: SDValue,
    pub scale: SDValue,
    pub index: SDValue,
    pub disp: SDValue,
    pub segment: SDValue,
}

/// `isInt<31>`: a displacement that cannot overflow when the base adds its own.
fn is_disp_safe_for_frame_index_or_reg_base(value: i64) -> bool {
    (-(1 << 30)..(1 << 30)).contains(&value)
}

/// Which address forms exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddrMode {
    /// 32-bit flat: `[base + index*scale + disp32]`.
    Flat32,
    /// 64-bit flat: `[base + index*scale + disp32]` with 64-bit registers.
    Flat64,
    /// Real mode (SD3): `[bx|bp] + [si|di] + disp16`, no scale. Which register
    /// is which is the register class's business (SD4); the matcher only
    /// keeps one base and one index.
    Real16,
}

/// The matchers over one DAG. `true` from a `match_*` means it cannot be done,
/// as in the pin.
pub struct Matcher<'a, D: AddrDag> {
    pub dag: &'a mut D,
    pub pointer_type: Vt,
    pub mode: AddrMode,
    /// Optimizing for size (`shouldOptForSize`).
    pub size: bool,
}

impl<D: AddrDag> Matcher<'_, D> {
    fn opcode(
        &self,
        v: SDValue,
    ) -> u16 {
        self.dag.opcode(v.node)
    }

    fn operand(
        &self,
        v: SDValue,
        index: usize,
    ) -> SDValue {
        self.dag.operand(v.node, index)
    }

    /// `isa<ConstantSDNode>` and `getSExtValue`.
    fn constant(
        &self,
        v: SDValue,
    ) -> Option<i64> {
        (self.opcode(v) == isd::Constant).then(|| self.dag.constant(v)).flatten()
    }

    fn is_base_with_constant_offset(
        &mut self,
        v: SDValue,
    ) -> bool {
        self.dag.num_operands(v.node) == 2
            && self.constant(self.operand(v, 1)).is_some()
            && (self.opcode(v) == isd::ADD || self.dag.is_add_like(v))
    }

    fn zext_or_trunc(
        &mut self,
        v: SDValue,
        vt: Vt,
    ) -> SDValue {
        let from = vt_bits(self.dag.value_type(v));
        if self.dag.value_type(v) == vt {
            v
        } else if from > vt_bits(vt) {
            self.dag.get_node(isd::TRUNCATE, vt, &[v])
        } else {
            self.dag.get_node(isd::ZERO_EXTEND, vt, &[v])
        }
    }

    /// `foldMaskAndShiftToExtract` ([L2234][ext]): `(x >> (8-c)) & (0xff << c)`
    /// as `((x >> 8) & 0xff) << c`, so the shift becomes the scale. `true`
    /// when it does not apply.
    ///
    /// [ext]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2234
    fn fold_mask_and_shift_to_extract(
        &mut self,
        n: SDValue,
        mask: u64,
        shift: SDValue,
        x: SDValue,
        am: &mut AddressMode,
    ) -> bool {
        let Some(amount) = self.constant(self.operand(shift, 1)) else { return true };
        if self.opcode(shift) != isd::SRL || !self.dag.has_one_use(shift) {
            return true;
        }
        let scale_log = 8 - amount;
        if scale_log <= 0 || scale_log >= 4 || mask != 0xffu64 << scale_log {
            return true;
        }
        let (xvt, vt) = (self.dag.value_type(x), self.dag.value_type(n));
        let eight = self.dag.get_constant(8, mvt::i8);
        let new_mask = self.dag.get_constant(0xff, xvt);
        let srl = self.dag.get_node(isd::SRL, xvt, &[x, eight]);
        let and = self.dag.get_node(isd::AND, xvt, &[srl, new_mask]);
        let ext = self.zext_or_trunc(and, vt);
        let count = self.dag.get_constant(scale_log, mvt::i8);
        let shl = self.dag.get_node(isd::SHL, vt, &[ext, count]);
        self.dag.replace_uses(n, shl);
        self.dag.remove_dead_node(n.node);
        am.index_reg = Some(ext);
        am.scale = 1 << scale_log;
        false
    }

    /// `foldMaskedShiftToScaledMask` ([L2300][scaled]): `(x << c) & m` as `(x &
    /// (m >> c)) << c`. `true` when it does not apply.
    ///
    /// [scaled]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2300
    fn fold_masked_shift_to_scaled_mask(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
    ) -> bool {
        let shift = self.operand(n, 0);
        let mask = self.constant(self.operand(n, 1)).expect("an AND with a constant");
        // The pin also looks through an `any_extend` of i32 to i64: not at 32
        // bits (SD35).
        if self.opcode(shift) != isd::SHL || !self.dag.has_one_use(n) || !self.dag.has_one_use(shift) {
            return true;
        }
        let Some(amount) = self.constant(self.operand(shift, 1)) else { return true };
        if !(1..=3).contains(&amount) {
            return true;
        }
        let x = self.operand(shift, 0);
        let vt = self.dag.value_type(n);
        let new_mask = self.dag.get_constant(mask >> amount, vt);
        let new_and = self.dag.get_node(isd::AND, vt, &[x, new_mask]);
        let new_shift = self.dag.get_node(isd::SHL, vt, &[new_and, self.operand(shift, 1)]);
        self.dag.replace_uses(n, new_shift);
        self.dag.remove_dead_node(n.node);
        am.scale = 1 << amount;
        am.index_reg = Some(new_and);
        false
    }

    /// `foldMaskAndShiftToScale` ([L2379][scale]): `(x >> s) & (m << a)` as
    /// `((x >> (s+a)) << a)` when the bits the mask drops are known zero.
    /// `true` when it does not apply.
    ///
    /// [scale]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2379
    fn fold_mask_and_shift_to_scale(
        &mut self,
        n: SDValue,
        mask: u64,
        shift: SDValue,
        x: SDValue,
        am: &mut AddressMode,
    ) -> bool {
        if self.opcode(shift) != isd::SRL || !self.dag.has_one_use(shift) {
            return true;
        }
        let Some(shift_amt) = self.constant(self.operand(shift, 1)) else { return true };
        // `isShiftedMask_64`: one run of ones.
        if mask == 0 {
            return true;
        }
        let (mask_idx, mask_len) = (mask.trailing_zeros(), (mask >> mask.trailing_zeros()).trailing_ones());
        if mask_idx + mask_len < 64 && mask >> (mask_idx + mask_len) != 0 {
            return true;
        }
        let mut mask_lz = 64 - (mask_idx + mask_len);
        let am_shift = mask_idx;
        if am_shift == 0 || am_shift > 3 {
            return true;
        }
        let mut x = x;
        let scale_down = (64 - vt_bits(self.dag.value_type(x))) + shift_amt as u32;
        if mask_lz < scale_down {
            return true;
        }
        mask_lz -= scale_down;
        let mut replacing_any_extend = false;
        if self.opcode(x) == isd::ANY_EXTEND {
            let src = self.operand(x, 0);
            let ext_bits = vt_bits(self.dag.value_type(x)) - vt_bits(self.dag.value_type(src));
            x = src;
            mask_lz = mask_lz.saturating_sub(ext_bits);
            replacing_any_extend = true;
        }
        let x_bits = vt_bits(self.dag.value_type(x));
        let high_bits = low_mask(x_bits) & !low_mask(x_bits.saturating_sub(mask_lz));
        if known_zero(self.dag, x, 0) & high_bits != high_bits {
            return true;
        }
        let vt = self.dag.value_type(n);
        if replacing_any_extend {
            x = self.dag.get_node(isd::ZERO_EXTEND, vt, &[x]);
        }
        let xvt = self.dag.value_type(x);
        let new_srl_amt = self.dag.get_constant(shift_amt + i64::from(am_shift), mvt::i8);
        let new_srl = self.dag.get_node(isd::SRL, xvt, &[x, new_srl_amt]);
        let new_ext = self.zext_or_trunc(new_srl, vt);
        let new_shl_amt = self.dag.get_constant(i64::from(am_shift), mvt::i8);
        let new_shl = self.dag.get_node(isd::SHL, vt, &[new_ext, new_shl_amt]);
        self.dag.replace_uses(n, new_shl);
        self.dag.remove_dead_node(n.node);
        am.scale = 1 << am_shift;
        am.index_reg = Some(new_ext);
        false
    }

    /// `foldOffsetIntoAddress` ([L1883][fold]), 32-bit flat.
    ///
    /// [fold]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L1883
    pub fn fold_offset_into_address(
        &mut self,
        offset: u64,
        am: &mut AddressMode,
    ) -> bool {
        let val = i64::from(am.disp).wrapping_add(offset as i64);
        if val != 0 && matches!(
            am.symbol,
            Some(Symbol { kind: SymbolKind::External | SymbolKind::McSymbol, .. })
        ) {
            return true;
        }
        if self.mode == AddrMode::Real16 {
            // `X86AsmBackend::applyFixup` cannot tell the sign of a 16-bit
            // fixup.
            if !(-i64::from(u16::MAX)..=i64::from(u16::MAX)).contains(&val) {
                return true;
            }
        } else if self.mode == AddrMode::Flat64 {
            // `isOffsetSuitableForCodeModel`, small code model.
            let suitable = i32::try_from(val).is_ok() && (!am.has_symbolic_displacement() || val < 16 << 20);
            if val != 0 && !suitable {
                return true;
            }
            if am.base_type == BaseType::FrameIndex && !is_disp_safe_for_frame_index_or_reg_base(val) {
                return true;
            }
        } else if am.has_base_or_index_reg() && !is_disp_safe_for_frame_index_or_reg_base(val) {
            return true;
        }
        am.disp = val as i32;
        false
    }

    /// `X86ISelAddressMode::isRIPRelative`.
    fn is_rip_relative(
        &self,
        am: &AddressMode,
    ) -> bool {
        am.base_type == BaseType::Reg
            && am.base_reg.is_some_and(|base| self.dag.register_of(base.node) == Some(x86::RIP))
    }

    /// `matchWrapper` ([L1976][wrap]).
    ///
    /// [wrap]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L1976
    fn match_wrapper(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
    ) -> bool {
        if am.has_symbolic_displacement() {
            return true;
        }
        let rip_relative = self.opcode(n) == x86isd::WrapperRIP;
        // %rip is the base: it takes no other register.
        if rip_relative && am.has_base_or_index_reg() {
            return true;
        }
        let backup = *am;
        let operand = self.operand(n, 0);
        // A symbol our DAG cannot name (a constant pool, a jump table) stays a
        // register operand.
        let Some(wrapped) = self.dag.wrapped_symbol(operand) else { return true };
        am.symbol = Some(wrapped.symbol);
        am.symbol_flags = wrapped.flags;
        // `ExternalSymbol` and `MCSymbol` carry no offset.
        let offset = match wrapped.symbol.kind {
            SymbolKind::External | SymbolKind::McSymbol | SymbolKind::JumpTable => 0,
            _ => wrapped.offset,
        };
        if self.fold_offset_into_address(offset as u64, am) {
            *am = backup;
            return true;
        }
        if rip_relative {
            am.base_reg = Some(self.dag.register(x86::RIP, mvt::i64));
        }
        false
    }

    /// `matchAddress` ([L2053][addr]).
    ///
    /// [addr]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2053
    pub fn match_address(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
    ) -> bool {
        if self.match_address_recursively(n, am, 0) {
            return true;
        }
        // lea(,%reg,2) is lea(%reg,%reg): shorter, no scaled index. Not for a
        // negated index.
        if am.scale == 2 && !am.negate_index && am.base_type == BaseType::Reg && am.base_reg.is_none() {
            am.base_reg = am.index_reg;
            am.scale = 1;
        }
        false
    }

    /// `matchAdd` ([L2158][add]).
    ///
    /// [add]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2158
    fn match_add(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
        depth: u32,
    ) -> bool {
        let backup = *am;
        // The operands are read again each time: a fold may have rewritten one.
        if !self.match_add_operand(am, self.operand(n, 0), depth)
            && !self.match_add_operand(am, self.operand(n, 1), depth)
        {
            return false;
        }
        *am = backup;
        if !self.match_add_operand(am, self.operand(n, 1), depth)
            && !self.match_add_operand(am, self.operand(n, 0), depth)
        {
            return false;
        }
        *am = backup;

        // Both operands in registers, at least the add folds.
        if am.base_type == BaseType::Reg && am.base_reg.is_none() && am.index_reg.is_none() {
            am.base_reg = Some(self.operand(n, 0));
            am.index_reg = Some(self.operand(n, 1));
            am.scale = 1;
            return false;
        }
        true
    }

    fn is_add_or_add_like(
        &self,
        v: SDValue,
    ) -> bool {
        self.opcode(v) == isd::ADD || self.dag.is_add_like(v)
    }

    /// Splitting an add that is a register already for another user adds an
    /// instruction.
    fn splits_materialized_value(
        &self,
        am: &AddressMode,
        op: SDValue,
    ) -> bool {
        if !am.is_for_lea || !self.dag.has_materializing_use(op) {
            return false;
        }
        if self.is_add_or_add_like(op) {
            return self.is_add_or_add_like(self.operand(op, 0)) || self.is_add_or_add_like(self.operand(op, 1));
        }
        if self.opcode(op) == isd::SHL {
            if let Some(c) = self.constant(self.operand(op, 1)) {
                return (1..=3).contains(&(c as u64)) && self.is_add_or_add_like(self.operand(op, 0));
            }
        }
        false
    }

    fn match_add_operand(
        &mut self,
        am: &mut AddressMode,
        op: SDValue,
        depth: u32,
    ) -> bool {
        if self.splits_materialized_value(am, op) {
            return self.match_address_base(op, am);
        }
        self.match_address_recursively(op, am, depth + 1)
    }

    /// `matchIndexRecursively` ([L2557][idx]); the rewriting cases are SD34.
    ///
    /// [idx]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2557
    fn match_index_recursively(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
        depth: u32,
    ) -> SDValue {
        debug_assert!(am.index_reg.is_none(), "IndexReg already matched");
        if depth >= MAX_RECURSION_DEPTH {
            return n;
        }
        if self.is_base_with_constant_offset(n) {
            let add_val = self.constant(self.operand(n, 1)).unwrap();
            let offset = (add_val as u64).wrapping_mul(u64::from(am.scale));
            if !self.fold_offset_into_address(offset, am) {
                return self.match_index_recursively(self.operand(n, 0), am, depth + 1);
            }
        }
        // x + x is x * 2.
        if self.takes_scale(n, am)
            && self.opcode(n) == isd::ADD
            && self.operand(n, 0) == self.operand(n, 1)
            && am.scale <= 4
        {
            am.scale *= 2;
            return self.match_index_recursively(self.operand(n, 0), am, depth + 1);
        }
        n
    }

    /// Whether the address may carry a scale: real mode has none, except in a
    /// dword `lea`, which takes the 386's 32-bit form.
    ///
    /// DEVIATE SD250 (m16): not when optimizing for size; that `lea` carries
    /// both the 0x66 and 0x67 prefixes and is longer than the `imul` or the
    /// `shl; add` it replaces (+214 bytes over the corpus at -Os).
    fn takes_scale(
        &self,
        n: SDValue,
        am: &AddressMode,
    ) -> bool {
        self.mode != AddrMode::Real16 || (am.is_for_lea && !self.size && vt_bits(self.dag.value_type(n)) == 32)
    }

    /// `matchAddressRecursively` ([L2677][rec]).
    ///
    /// [rec]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2677
    fn match_address_recursively(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
        depth: u32,
    ) -> bool {
        if depth >= MAX_RECURSION_DEPTH {
            return self.match_address_base(n, am);
        }
        // `%rip + disp32`: only an immediate merges into it.
        if self.is_rip_relative(am) {
            if matches!(am.symbol, Some(Symbol { kind: SymbolKind::JumpTable, .. })) {
                return true;
            }
            if let Some(value) = self.constant(n) {
                if !self.fold_offset_into_address(value as u64, am) {
                    return false;
                }
            }
            return true;
        }
        let opcode = self.opcode(n);
        match opcode {
            isd::Constant => {
                let val = self.constant(n).unwrap();
                if !self.fold_offset_into_address(val as u64, am) {
                    return false;
                }
            }
            x86isd::Wrapper | x86isd::WrapperRIP => {
                if !self.match_wrapper(n, am) {
                    return false;
                }
            }
            isd::FrameIndex => {
                if am.base_type == BaseType::Reg && am.base_reg.is_none() {
                    am.base_type = BaseType::FrameIndex;
                    am.base_frame_index = self.dag.frame_index(n).expect("a frame index node");
                    return false;
                }
            }
            isd::SHL => {
                if self.takes_scale(n, am) && am.index_reg.is_none() && am.scale == 1 {
                    if let Some(shift) = self.constant(self.operand(n, 1)) {
                        if (1..=3).contains(&(shift as u64)) {
                            let sh_val = self.operand(n, 0);
                            am.scale = 1 << shift;
                            am.index_reg = Some(self.match_index_recursively(sh_val, am, depth + 1));
                            return false;
                        }
                    }
                }
            }
            isd::SRL => {
                if self.mode != AddrMode::Real16 && am.index_reg.is_none() && am.scale == 1 {
                    let and = self.operand(n, 0);
                    if self.opcode(and) == isd::AND {
                        let x = self.operand(and, 0);
                        if let (Some(amount), Some(and_mask)) =
                            (self.constant(self.operand(n, 1)), self.constant(self.operand(and, 1)))
                        {
                            let mask = (and_mask as u64) >> amount;
                            if !self.fold_mask_and_shift_to_scale(n, mask, n, x, am) {
                                return false;
                            }
                        }
                    }
                }
            }
            isd::AND => {
                if self.mode != AddrMode::Real16 && am.index_reg.is_none() && am.scale == 1 {
                    if let Some(mask) = self.constant(self.operand(n, 1)) {
                        let shift = self.operand(n, 0);
                        if self.opcode(shift) == isd::SRL {
                            let x = self.operand(shift, 0);
                            if !self.fold_mask_and_shift_to_extract(n, mask as u64, shift, x, am)
                                || !self.fold_mask_and_shift_to_scale(n, mask as u64, shift, x, am)
                            {
                                return false;
                            }
                        }
                        // BEXTR needs TBM or fast BMI: neither exists on these
                        // CPUs.
                        if !self.fold_masked_shift_to_scaled_mask(n, am) {
                            return false;
                        }
                    }
                }
            }
            isd::MUL | x86isd::MUL_IMM => {
                // Real mode has no scaled index; a dword `lea` takes the 386's
                // 32-bit form.
                if self.takes_scale(n, am)
                    && am.base_type == BaseType::Reg
                    && am.base_reg.is_none()
                    && am.index_reg.is_none()
                {
                    if let Some(c) = self.constant(self.operand(n, 1)) {
                        if matches!(c as u64, 3 | 5 | 9) {
                            am.scale = c as u32 - 1;
                            let mul_val = self.operand(n, 0);
                            let mut reg = mul_val;
                            if self.opcode(mul_val) == isd::ADD && self.dag.node_has_one_use(mul_val.node) {
                                if let Some(add_val) = self.constant(self.operand(mul_val, 1)) {
                                    reg = self.operand(mul_val, 0);
                                    let disp = (add_val as u64).wrapping_mul(c as u64);
                                    if self.fold_offset_into_address(disp, am) {
                                        reg = self.operand(n, 0);
                                    }
                                }
                            }
                            am.index_reg = Some(reg);
                            am.base_reg = Some(reg);
                            return false;
                        }
                    }
                }
            }
            isd::SUB => {
                let backup = *am;
                if self.match_address_recursively(self.operand(n, 0), am, depth + 1) || am.index_reg.is_some() {
                    *am = backup;
                } else if !self.fold_sub_rhs(n, am, &backup) {
                    return false;
                }
            }
            isd::OR | isd::XOR | isd::ADD
                if (opcode == isd::ADD || self.dag.is_add_like(n)) && !self.match_add(n, am, depth) =>
            {
                return false;
            }
            _ => {}
        }
        self.match_address_base(n, am)
    }

    /// The right operand of a `SUB` as a negated index, when that pays; `true`
    /// if it did not.
    fn fold_sub_rhs(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
        backup: &AddressMode,
    ) -> bool {
        // Folding `-rhs` costs a NEG unless it merges with something else.
        let mut cost = 0i32;
        let mut rhs = self.operand(n, 1);
        let mut neg_scale = None;
        // 16-bit addressing has no scale (SD3): the shift stays a register,
        // except in an `lea`, which takes the 386's 32-bit form.
        if (self.mode != AddrMode::Real16 || am.is_for_lea)
            && self.opcode(rhs) == isd::SHL
            && self.dag.node_has_one_use(rhs.node)
        {
            if let Some(sh) = self.constant(self.operand(rhs, 1)) {
                if (1..=3).contains(&(sh as u64)) {
                    neg_scale = Some(1u32 << sh);
                    rhs = self.operand(rhs, 0);
                    cost -= 1;
                }
            }
        }
        let rhs_opcode = self.opcode(rhs);
        if !self.dag.node_has_one_use(rhs.node)
            || (neg_scale.is_none() && rhs_opcode == isd::CopyFromReg)
            || rhs_opcode == isd::TRUNCATE
            || rhs_opcode == isd::ANY_EXTEND
            || (rhs_opcode == isd::ZERO_EXTEND && self.dag.value_type(self.operand(rhs, 0)) == mvt::i32)
        {
            cost += 1;
        }
        let base_is_negated_value = neg_scale.is_some() && am.base_type == BaseType::Reg && am.base_reg == Some(rhs);
        if ((am.base_type == BaseType::Reg && am.base_reg.is_some_and(|b| !self.dag.node_has_one_use(b.node)))
            || am.base_type == BaseType::FrameIndex)
            && !base_is_negated_value
        {
            cost -= 1;
        }
        let changed = i32::from(am.has_symbolic_displacement() && !backup.has_symbolic_displacement())
            + i32::from(am.disp != 0 && backup.disp == 0)
            + i32::from(am.segment.is_some() && backup.segment.is_none());
        if changed >= 2 {
            cost -= 1;
        }
        if cost >= 0 {
            *am = *backup;
            return true;
        }
        am.index_reg = Some(rhs);
        am.negate_index = true;
        am.scale = neg_scale.unwrap_or(1);
        false
    }

    /// `matchAddressBase` ([L3083][base]).
    ///
    /// [base]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3083
    fn match_address_base(
        &mut self,
        n: SDValue,
        am: &mut AddressMode,
    ) -> bool {
        if am.base_type != BaseType::Reg || am.base_reg.is_some() {
            if am.index_reg.is_none() {
                am.index_reg = Some(n);
                am.scale = 1;
                return false;
            }
            return true;
        }
        am.base_type = BaseType::Reg;
        am.base_reg = Some(n);
        false
    }

    /// `getAddressOperands` ([L270][ops]).
    ///
    /// [ops]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L270
    pub fn get_address_operands(
        &mut self,
        mut am: AddressMode,
        vt: Vt,
    ) -> AddressOperands {
        let base = if am.base_type == BaseType::FrameIndex {
            self.dag.target_frame_index(am.base_frame_index, self.pointer_type)
        } else if let Some(reg) = am.base_reg {
            reg
        } else {
            self.dag.register(0, vt)
        };
        let scale = self.dag.target_constant(i64::from(am.scale), mvt::i8);
        if am.negate_index {
            let neg = if vt == mvt::i32 {
                x86::NEG32r
            } else if vt == mvt::i16 {
                x86::NEG16r
            } else {
                x86::NEG8r
            };
            let index = am.index_reg.expect("a negated index");
            let node = self.dag.machine_node(neg, &[vt, mvt::i32], &[index]);
            am.index_reg = Some(SDValue::new(node, 0));
        }
        let index = match am.index_reg {
            Some(index) => index,
            None => self.dag.register(0, vt),
        };
        let disp = match am.symbol {
            Some(symbol) => self.dag.target_symbol(symbol, am.disp, am.symbol_flags),
            None => self.dag.target_constant(i64::from(am.disp), mvt::i32),
        };
        let segment = match am.segment {
            Some(segment) => segment,
            None => self.dag.register(0, mvt::i16),
        };
        AddressOperands { base, scale, index, disp, segment }
    }

    /// `selectAddr` ([L3202][sel]).
    ///
    /// [sel]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3202
    pub fn select_addr(
        &mut self,
        n: SDValue,
    ) -> Option<AddressOperands> {
        self.select_addr_of(None, n)
    }

    /// `selectAddr(Parent, N)`: `parent` is the memory node whose address `n`
    /// is; its segment override becomes the segment operand.
    pub fn select_addr_of(
        &mut self,
        parent: Option<u32>,
        n: SDValue,
    ) -> Option<AddressOperands> {
        let mut am = AddressMode::default();
        if let Some(reg) = parent.and_then(|p| self.dag.segment_override(p)) {
            am.segment = Some(self.dag.register(reg, mvt::i16));
        }
        if self.match_address(n, &mut am) {
            return None;
        }
        let vt = self.dag.value_type(n);
        Some(self.get_address_operands(am, vt))
    }

    /// `selectLEA64_Addr` ([L3282][lea64]): `selectLEAAddr`, then a register
    /// narrower than 64 bits is widened with `INSERT_SUBREG` into an
    /// `IMPLICIT_DEF`, as `LEA64_32r` and its 8/16-bit kin read 64-bit
    /// registers.
    ///
    /// [lea64]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3282
    pub fn select_lea64_addr(
        &mut self,
        n: SDValue,
    ) -> Option<AddressOperands> {
        let mut ops = self.select_lea_addr(n)?;
        let base_type = self.dag.value_type(ops.base);
        let sub_reg = if base_type == mvt::i8 {
            x86::sub_8bit
        } else if base_type == mvt::i16 {
            x86::sub_16bit
        } else {
            x86::sub_32bit
        };
        if self.dag.register_of(ops.base.node) == Some(0) {
            ops.base = self.dag.register(0, mvt::i64);
        } else if matches!(base_type, t if t == mvt::i8 || t == mvt::i16 || t == mvt::i32)
            && self.dag.opcode(ops.base.node) != isd::FrameIndex
            && self.dag.opcode(ops.base.node) != isd::TargetFrameIndex
        {
            ops.base = self.insert_subreg(sub_reg, ops.base);
        }
        if self.dag.register_of(ops.index.node) == Some(0) {
            ops.index = self.dag.register(0, mvt::i64);
        } else {
            ops.index = self.insert_subreg(sub_reg, ops.index);
        }
        Some(ops)
    }

    /// `getTargetInsertSubreg` into an `IMPLICIT_DEF` of 64 bits.
    fn insert_subreg(
        &mut self,
        sub_reg: u16,
        value: SDValue,
    ) -> SDValue {
        let undef = SDValue::new(self.dag.machine_node(target_opcode::IMPLICIT_DEF, &[mvt::i64], &[]), 0);
        let index = self.dag.target_constant(i64::from(sub_reg), mvt::i32);
        SDValue::new(self.dag.machine_node(target_opcode::INSERT_SUBREG, &[mvt::i64], &[undef, value, index]), 0)
    }

    /// `selectLEAAddr` ([L3329][lea]): an address worth an `lea` instruction.
    ///
    /// [lea]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3329
    pub fn select_lea_addr(
        &mut self,
        n: SDValue,
    ) -> Option<AddressOperands> {
        let mut am = AddressMode { is_for_lea: true, ..AddressMode::default() };
        let vt = self.dag.value_type(n);
        // A segment placeholder keeps the match from taking a segment of its
        // own.
        let placeholder = self.dag.register(0, mvt::i32);
        am.segment = Some(placeholder);
        if self.match_address(n, &mut am) {
            return None;
        }
        debug_assert_eq!(am.segment, Some(placeholder));
        am.segment = None;

        let mut complexity = 0;
        if am.base_type == BaseType::Reg && am.base_reg.is_some() {
            complexity = 1;
        } else if am.base_type == BaseType::FrameIndex {
            complexity = 4;
        }
        if am.index_reg.is_some() {
            complexity += 1;
        }
        if am.scale > 1 {
            complexity += 1;
        }
        if am.has_symbolic_displacement() {
            complexity += 2;
        }
        if self.opcode(n) == isd::ADD {
            let math_with_flags = |v: SDValue| {
                matches!(
                    self.opcode(v),
                    x86isd::ADD | x86isd::SUB | x86isd::ADC | x86isd::SBB | x86isd::SMUL | x86isd::UMUL
                ) && !self.dag.has_n_uses_of_value(SDValue::new(v.node, 1), 0)
            };
            if math_with_flags(self.operand(n, 0)) || math_with_flags(self.operand(n, 1)) {
                complexity += 1;
            }
        }
        if am.disp != 0 {
            complexity += 1;
        }
        if complexity <= 2 {
            return None;
        }
        Some(self.get_address_operands(am, vt))
    }
}
