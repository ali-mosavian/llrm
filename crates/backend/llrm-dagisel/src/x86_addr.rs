//! `X86ISelAddressMode` and the address matchers of `X86DAGToDAGISel`, for 32-bit flat targets.
//!
//! # How it works
//! `selectAddr` and `selectLEAAddr` are the complex patterns the generated table calls to turn a
//! pointer expression into the five operands of an x86 memory reference: base, scale, index,
//! displacement, segment. They run [`Matcher::match_address`] ([L2053][addr]) which folds
//! constants, wrapped symbols, frame indexes, scaled indexes and adds into an
//! [`AddressMode`]; [`Matcher::get_address_operands`] ([L270][ops]) writes the operands.
//! The matchers never change the DAG here, so the pin's `HandleSDNode` guards are not needed.
//!
//! Numbered deviations from the pin (reason / owner / exit / review):
//! - **SD25** Not translated: the mask-and-shift folds (`foldMaskAndShiftToExtract`,
//!   `foldMaskAndShiftToScale`, `foldMaskedShiftToBEXTR`, `foldMaskedShiftToScaledMask`) and the
//!   `SIGN_EXTEND`/`ZERO_EXTEND` re-association of `matchIndexRecursively`, `SRL`, `AND` and
//!   `ZERO_EXTEND` cases of `matchAddressRecursively`: they rewrite the DAG and need the DAG API's
//!   `RepositionNode`. Owner sdag-c. Exit: C1b. Review 2026-12-10.
//! - **SD26** The 64-bit, x32 and 16-bit branches, RIP-relative modes, large code model, the
//!   TLS segment loads (`matchLoadInAddress`) and the vector forms (`matchVectorAddress`) are
//!   left out: no such target here. m16 has its own matcher (C2). Owner sdag-c. Exit: none.
//!   Review 2026-12-10.
//!
//! # Used by
//! [`crate::x86_hooks`].
//!
//! [addr]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2053
//! [ops]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L270

use crate::dag::{Dag, SDValue};
use crate::generated::ids::{isd, mvt, x86, x86isd};
use crate::matcher::Vt;

/// `SelectionDAG::MaxRecursionDepth`.
const MAX_RECURSION_DEPTH: u32 = 6;

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
    /// The global, pool entry, name, label, table or block, in the DAG's own numbering.
    pub id: u32,
}

/// The operand of a `Wrapper`: the symbol, its offset and target flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wrapped {
    pub symbol: Symbol,
    pub offset: i64,
    pub flags: u8,
}

/// The DAG queries and constructors the address matchers need on top of [`Dag`].
pub trait AddrDag: Dag {
    /// `SelectionDAG::isADDLike`: an `OR` of disjoint bits, or an `XOR` with the sign bit.
    fn is_add_like(&self, value: SDValue) -> bool;
    /// `FrameIndexSDNode::getIndex`.
    fn frame_index(&self, value: SDValue) -> Option<i32>;
    /// The symbol a `Wrapper`'s operand names (`TargetGlobalAddress`, `TargetConstantPool`, ...).
    fn wrapped_symbol(&self, operand: SDValue) -> Option<Wrapped>;
    /// `hasMaterializingUse`: a use that puts `value` in a register as a value.
    fn has_materializing_use(&self, value: SDValue) -> bool;
    /// `StoreSDNode::isNonTemporal`.
    fn is_non_temporal(&self, node: u32) -> bool;
    fn target_frame_index(&mut self, index: i32, vt: Vt) -> SDValue;
    /// `getTargetGlobalAddress`, `getTargetConstantPool` and the rest, by symbol kind.
    fn target_symbol(&mut self, symbol: Symbol, disp: i32, flags: u8) -> SDValue;
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

/// The matchers over one DAG. `true` from a `match_*` means it cannot be done, as in the pin.
pub struct Matcher<'a, D: AddrDag> {
    pub dag: &'a mut D,
    pub pointer_type: Vt,
}

impl<D: AddrDag> Matcher<'_, D> {
    fn opcode(&self, v: SDValue) -> u16 {
        self.dag.opcode(v.node)
    }

    fn operand(&self, v: SDValue, index: usize) -> SDValue {
        self.dag.operand(v.node, index)
    }

    /// `isa<ConstantSDNode>` and `getSExtValue`.
    fn constant(&self, v: SDValue) -> Option<i64> {
        (self.opcode(v) == isd::Constant).then(|| self.dag.constant(v)).flatten()
    }

    fn is_base_with_constant_offset(&self, v: SDValue) -> bool {
        self.dag.num_operands(v.node) == 2
            && self.constant(self.operand(v, 1)).is_some()
            && (self.opcode(v) == isd::ADD || self.dag.is_add_like(v))
    }

    /// `foldOffsetIntoAddress` ([L1883][fold]), 32-bit flat.
    ///
    /// [fold]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L1883
    pub fn fold_offset_into_address(&self, offset: u64, am: &mut AddressMode) -> bool {
        let val = i64::from(am.disp).wrapping_add(offset as i64);
        if val != 0 && matches!(am.symbol, Some(Symbol { kind: SymbolKind::External | SymbolKind::McSymbol, .. })) {
            return true;
        }
        if am.has_base_or_index_reg() && !is_disp_safe_for_frame_index_or_reg_base(val) {
            return true;
        }
        am.disp = val as i32;
        false
    }

    /// `matchWrapper` ([L1976][wrap]).
    ///
    /// [wrap]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L1976
    fn match_wrapper(&self, n: SDValue, am: &mut AddressMode) -> bool {
        if am.has_symbolic_displacement() {
            return true;
        }
        let backup = *am;
        let operand = self.operand(n, 0);
        // A symbol our DAG cannot name (a constant pool, a jump table) stays a register operand.
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
        false
    }

    /// `matchAddress` ([L2053][addr]).
    ///
    /// [addr]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2053
    pub fn match_address(&self, n: SDValue, am: &mut AddressMode) -> bool {
        if self.match_address_recursively(n, am, 0) {
            return true;
        }
        // lea(,%reg,2) is lea(%reg,%reg): shorter, no scaled index. Not for a negated index.
        if am.scale == 2 && !am.negate_index && am.base_type == BaseType::Reg && am.base_reg.is_none() {
            am.base_reg = am.index_reg;
            am.scale = 1;
        }
        false
    }

    /// `matchAdd` ([L2158][add]).
    ///
    /// [add]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2158
    fn match_add(&self, n: SDValue, am: &mut AddressMode, depth: u32) -> bool {
        let is_add_or_add_like = |v: SDValue| self.opcode(v) == isd::ADD || self.dag.is_add_like(v);
        // Splitting an add that is a register already for another user adds an instruction.
        let splits_materialized_value = |am: &AddressMode, op: SDValue| {
            if !am.is_for_lea || !self.dag.has_materializing_use(op) {
                return false;
            }
            if is_add_or_add_like(op) {
                return is_add_or_add_like(self.operand(op, 0)) || is_add_or_add_like(self.operand(op, 1));
            }
            if self.opcode(op) == isd::SHL {
                if let Some(c) = self.constant(self.operand(op, 1)) {
                    return (1..=3).contains(&(c as u64)) && is_add_or_add_like(self.operand(op, 0));
                }
            }
            false
        };
        let match_operand = |am: &mut AddressMode, op: SDValue| {
            if splits_materialized_value(am, op) {
                return self.match_address_base(op, am);
            }
            self.match_address_recursively(op, am, depth + 1)
        };

        let backup = *am;
        if !match_operand(am, self.operand(n, 0)) && !match_operand(am, self.operand(n, 1)) {
            return false;
        }
        *am = backup;
        if !match_operand(am, self.operand(n, 1)) && !match_operand(am, self.operand(n, 0)) {
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

    /// `matchIndexRecursively` ([L2557][idx]); the rewriting cases are SD25.
    ///
    /// [idx]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2557
    fn match_index_recursively(&self, n: SDValue, am: &mut AddressMode, depth: u32) -> SDValue {
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
        if self.opcode(n) == isd::ADD && self.operand(n, 0) == self.operand(n, 1) && am.scale <= 4 {
            am.scale *= 2;
            return self.match_index_recursively(self.operand(n, 0), am, depth + 1);
        }
        n
    }

    /// `matchAddressRecursively` ([L2677][rec]).
    ///
    /// [rec]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L2677
    fn match_address_recursively(&self, n: SDValue, am: &mut AddressMode, depth: u32) -> bool {
        if depth >= MAX_RECURSION_DEPTH {
            return self.match_address_base(n, am);
        }
        let opcode = self.opcode(n);
        match opcode {
            isd::Constant => {
                let val = self.constant(n).unwrap();
                if !self.fold_offset_into_address(val as u64, am) {
                    return false;
                }
            }
            x86isd::Wrapper => {
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
                if am.index_reg.is_none() && am.scale == 1 {
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
            isd::MUL | x86isd::MUL_IMM => {
                if am.base_type == BaseType::Reg && am.base_reg.is_none() && am.index_reg.is_none() {
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
            isd::OR | isd::XOR | isd::ADD if (opcode == isd::ADD || self.dag.is_add_like(n)) && !self.match_add(n, am, depth) => {
                return false;
            }
            _ => {}
        }
        self.match_address_base(n, am)
    }

    /// The right operand of a `SUB` as a negated index, when that pays; `true` if it did not.
    fn fold_sub_rhs(&self, n: SDValue, am: &mut AddressMode, backup: &AddressMode) -> bool {
        // Folding `-rhs` costs a NEG unless it merges with something else.
        let mut cost = 0i32;
        let mut rhs = self.operand(n, 1);
        let mut neg_scale = None;
        if self.opcode(rhs) == isd::SHL && self.dag.node_has_one_use(rhs.node) {
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
    /// [base]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3083
    fn match_address_base(&self, n: SDValue, am: &mut AddressMode) -> bool {
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
    /// [ops]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L270
    pub fn get_address_operands(&mut self, mut am: AddressMode, vt: Vt) -> AddressOperands {
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
    /// [sel]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3202
    pub fn select_addr(&mut self, n: SDValue) -> Option<AddressOperands> {
        let mut am = AddressMode::default();
        if self.match_address(n, &mut am) {
            return None;
        }
        let vt = self.dag.value_type(n);
        Some(self.get_address_operands(am, vt))
    }

    /// `selectLEAAddr` ([L3329][lea]): an address worth an `lea` instruction.
    ///
    /// [lea]: https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86ISelDAGToDAG.cpp#L3329
    pub fn select_lea_addr(&mut self, n: SDValue) -> Option<AddressOperands> {
        let mut am = AddressMode { is_for_lea: true, ..AddressMode::default() };
        let vt = self.dag.value_type(n);
        // A segment placeholder keeps the match from taking a segment of its own.
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
