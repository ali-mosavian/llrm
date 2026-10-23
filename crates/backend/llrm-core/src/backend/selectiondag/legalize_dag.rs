//! Operation legalization: every node's operation is one the target can
//! select (the pin's `SelectionDAGLegalize`).
//!
//! # How it works
//! Types are already legal ([`super::legalize_types`]). The walk visits nodes
//! users first, so a node is seen with its original operands: the nodes
//! reachable from the root in reverse topological order. For each,
//! [`DagLegalizer::legalize_op`] asks the action table. `Legal` keeps the
//! node. `Custom` asks the target ([`Hooks::lower_operation`]); if it
//! declines the node is expanded. `Expand` is `expand_node`, and what that
//! cannot do goes to the library (`convert_node_to_libcall`). `Promote` makes
//! the node in the wider type and truncates the result. Nodes made on the way
//! are legalized by the next pass, until a pass legalizes nothing.
//!
//! # Runs
//! After the type legalizer and before the third combine; not wired in yet.
//! This piece has the driver, `Promote` of the simple integer operations, and
//! no `Expand` or library cases; a node that needs one is refused by name
//! ([`LegalizeError::Unsupported`]) until B1b and B1e.
//!
//! DEVIATE SD41: no `DAGNodeDeletedListener` and no dead-node deletion
//! (`DeleteNode`, `RemoveDeadNodes`): the arena keeps replaced nodes, and the
//! walk skips a node nothing reads. Exit: none. Owner sdag-b. Review
//! 2026-11-13.
//!
//! DEVIATE SD203: `ExpandConstantFP` does not shrink an `f80` constant to a
//! narrower pool entry (no `APFloat`, SD11), and the pool entry's alignment is
//! the i386 data layout's preferred one by type. Exit: with an `APFloat`.
//! Owner sdag-b. Review 2026-11-27.
//!
//! # See also
//! LLVM's `SelectionDAG::Legalize`
//! ([L6301](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L6301)),
//! `LegalizeOp`
//! ([L982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L982)),
//! `PromoteNode`
//! ([L5432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L5432)).

use std::collections::HashSet;

use super::actions::{Action, TargetLowering};
use super::dag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use super::isd::ISD;
use super::legalize_types::LegalizeError;
use super::mvt::{EVT, MVT};

/// What the target supplies (`TLI.LowerOperation`).
pub trait Hooks {
    /// The legal replacement of `node` (its first result), or `None` to have
    /// the node expanded instead.
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue>;
}

/// A target with no custom lowering.
pub struct NoHooks;

impl Hooks for NoHooks {
    fn lower_operation(
        &mut self,
        _dag: &mut SelectionDAG,
        _node: SDValue,
    ) -> Option<SDValue> {
        None
    }
}

pub struct DagLegalizer<'a, H: Hooks> {
    dag: &'a mut SelectionDAG,
    tli: &'a TargetLowering,
    hooks: &'a mut H,
    legalized: HashSet<u32>,
}

impl<'a, H: Hooks> DagLegalizer<'a, H> {
    pub fn new(
        dag: &'a mut SelectionDAG,
        tli: &'a TargetLowering,
        hooks: &'a mut H,
    ) -> DagLegalizer<'a, H> {
        DagLegalizer { dag, tli, hooks, legalized: HashSet::new() }
    }

    /// `SelectionDAG::Legalize`.
    pub fn run(&mut self) -> Result<(), LegalizeError> {
        loop {
            let mut any = false;
            for n in self.reverse_topological() {
                if self.is_dead(n) || !self.legalized.insert(n) {
                    continue;
                }
                any = true;
                self.legalize_op(n)?;
            }
            if !any {
                return Ok(());
            }
        }
    }

    /// The nodes reachable from the root, users before their operands.
    fn reverse_topological(&self) -> Vec<u32> {
        let mut order = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![(self.dag.get_root().node, false)];
        while let Some((n, expanded)) = stack.pop() {
            if expanded {
                order.push(n);
                continue;
            }
            if !seen.insert(n) {
                continue;
            }
            stack.push((n, true));
            for op in &self.dag.node(SDValue { node: n, res: 0 }).ops {
                stack.push((op.node, false));
            }
        }
        order.reverse();
        order
    }

    /// A node nothing reads that is not the root.
    fn is_dead(
        &self,
        n: u32,
    ) -> bool {
        if n == self.dag.get_root().node {
            return false;
        }
        let results = self.dag.node(SDValue { node: n, res: 0 }).values.len() as u8;
        (0..results).all(|res| self.dag.users(SDValue { node: n, res }).next().is_none())
    }

    /// The type the action table is asked about for `value`'s node (the
    /// switch at the top of `LegalizeOp`).
    fn action_type(
        &self,
        node: SDValue,
    ) -> EVT {
        match self.dag.opcode(node) {
            ISD::Br | ISD::BrCond | ISD::BrCC | ISD::BrJT => EVT::OTHER,
            ISD::SIntToFp | ISD::UIntToFp => self.dag.value_type(self.dag.operand(node, 0)),
            ISD::SetCC | ISD::Store => {
                self.dag.value_type(self.dag.operand(node, if self.dag.opcode(node) == ISD::Store { 1 } else { 0 }))
            }
            _ => self.dag.value_type(node),
        }
    }

    /// `LegalizeOp`.
    fn legalize_op(
        &mut self,
        n: u32,
    ) -> Result<(), LegalizeError> {
        let node = SDValue { node: n, res: 0 };
        if matches!(self.dag.opcode(node), ISD::TargetConstant | ISD::Register) {
            return Ok(());
        }
        if self.dag.opcode(node) == ISD::ConstantFP {
            return self.legalize_constant_fp(node);
        }
        let vt = self.action_type(node);
        let mut action = match vt {
            EVT::Simple(_) => self.tli.operation_action(self.dag.opcode(node), vt),
            EVT::Int(_) => Action::Expand,
        };
        if action == Action::Custom {
            if let Some(result) = self.hooks.lower_operation(self.dag, node) {
                if result != node {
                    // A node with several results is replaced result by result
                    // (the node of `MERGE_VALUES` the target returns).
                    let results = self.dag.node(node).values.len() as u8;
                    if results > 1 && self.dag.node(result).values.len() == results as usize {
                        for res in 0..results {
                            self.replace_node(SDValue { node: n, res }, SDValue { node: result.node, res });
                        }
                    } else {
                        self.replace_node(node, result);
                    }
                }
                return Ok(());
            }
            action = Action::Expand;
        }
        match action {
            Action::Legal => Ok(()),
            Action::Promote => self.promote_node(node),
            Action::Expand => match self.expand_node(node) {
                Some(results) => {
                    for (res, new) in results.into_iter().enumerate() {
                        self.replace_node(SDValue { node: node.node, res: res as u8 }, new);
                    }
                    Ok(())
                }
                None => Err(LegalizeError::Unsupported {
                    op: self.dag.opcode(node),
                    ty: vt,
                    what: "convert node to libcall",
                }),
            },
            Action::LibCall | Action::Custom => {
                Err(LegalizeError::Unsupported { op: self.dag.opcode(node), ty: vt, what: "expand node" })
            }
        }
    }

    /// `ExpandNode`, for the cases translated: the replacement for each result
    /// of `node`, or `None` if it has to become a library call.
    fn expand_node(
        &mut self,
        node: SDValue,
    ) -> Option<Vec<SDValue>> {
        let op = self.dag.opcode(node);
        let vt = self.dag.value_type(node);
        let ready = |legalizer: &Self, op: ISD| legalizer.tli.is_operation_legal_or_custom(op, vt);
        match op {
            ISD::UDiv | ISD::SDiv => {
                let divrem = if op == ISD::SDiv { ISD::SDivRem } else { ISD::UDivRem };
                if !ready(self, divrem) {
                    return None;
                }
                let (a, b) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                Some(vec![self.dag.get_node_multi(divrem, &[vt, vt], &[a, b])])
            }
            ISD::URem | ISD::SRem => {
                // `TargetLowering::expandREM`.
                let (divrem, div) = if op == ISD::SRem { (ISD::SDivRem, ISD::SDiv) } else { (ISD::UDivRem, ISD::UDiv) };
                let (a, b) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                if ready(self, divrem) {
                    let both = self.dag.get_node_multi(divrem, &[vt, vt], &[a, b]);
                    return Some(vec![SDValue { node: both.node, res: 1 }]);
                }
                if ready(self, div) {
                    let quotient = self.dag.get_node(div, vt, &[a, b]);
                    let product = self.dag.get_node(ISD::Mul, vt, &[quotient, b]);
                    return Some(vec![self.dag.get_node(ISD::Sub, vt, &[a, product])]);
                }
                None
            }
            ISD::MulHU | ISD::MulHS => {
                let lohi = if op == ISD::MulHU { ISD::UMulLoHi } else { ISD::SMulLoHi };
                let (a, b) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let both = self.dag.get_node_multi(lohi, &[vt, vt], &[a, b]);
                Some(vec![SDValue { node: both.node, res: 1 }])
            }
            ISD::Undef | ISD::Poison => {
                // `ExpandNode`: zero of the type.
                Some(vec![if vt.is_integer() {
                    self.dag.get_constant(0, vt)
                } else {
                    self.dag.get_constant_fp(0, vt)
                }])
            }
            ISD::Abs => Some(vec![self.expand_abs(node)]),
            ISD::CtPop => self.expand_ctpop(node).map(|result| vec![result]),
            ISD::BSwap => self.expand_bswap(node).map(|result| vec![result]),
            // `vaend` has nothing to do; `vacopy` is a load of the source list
            // and a store to the destination.
            ISD::VAEnd => Some(vec![self.dag.operand(node, 0)]),
            ISD::VACopy => {
                let (chain, dest, src) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                let pointer = self.dag.config().pointer;
                let info = MemInfo {
                    mem_vt: pointer,
                    align: pointer.size_in_bits() / 8,
                    volatile: false,
                    base: MemBase::Unknown,
                    ext: None,
                };
                let loaded = self.dag.get_load(pointer, chain, src, info);
                Some(vec![self.dag.get_store(SDValue { node: loaded.node, res: 1 }, loaded, dest, info)])
            }
            _ => None,
        }
    }

    /// `TargetLowering::expandABS` ([L11308][pin]) for scalars: `smax(x, 0 -
    /// x)` or `umin` when legal, else `(x ^ s) - s` for `s = x >>s (bits -
    /// 1)`.
    ///
    /// DEVIATE SD306: the pin freezes `x` first because `x` is read twice;
    /// selection deletes the `FREEZE` again, so the machine code is the
    /// same and the node is left out. Owner sdag-c. Exit: when the DAG has
    /// `FREEZE`. Review 2026-12-10.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L11308
    fn expand_abs(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let vt = self.dag.value_type(node);
        let x = self.dag.operand(node, 0);
        if self.tli.is_operation_legal(ISD::Sub, vt) {
            for op in [ISD::SMax, ISD::UMin] {
                if self.tli.is_operation_legal(op, vt) {
                    let zero = self.dag.get_constant(0, vt);
                    let neg = self.dag.get_node(ISD::Sub, vt, &[zero, x]);
                    return self.dag.get_node(op, vt, &[x, neg]);
                }
            }
        }
        let amount = self.dag.get_constant(u128::from(vt.size_in_bits() - 1), EVT::I8);
        let sign = self.dag.get_node(ISD::Sra, vt, &[x, amount]);
        let flipped = self.dag.get_node(ISD::Xor, vt, &[x, sign]);
        self.dag.get_node(ISD::Sub, vt, &[flipped, sign])
    }

    /// `TargetLowering::expandCTPOP` ([L10830][pin]), scalar: the parallel
    /// bit count, with the active bits found from known bits.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L10830
    fn expand_ctpop(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let len = vt.size_in_bits();
        let mut op = self.dag.operand(node, 0);
        let known = super::known_bits::known_bits(self.dag, op);
        let width = known.width;
        let leading = (!known.zero & super::fold::mask(width)).leading_zeros() - (128 - width);
        let trailing = known.zero.trailing_ones().min(width);
        let shifted_active = width.saturating_sub(leading + trailing);
        let effective =
            if shifted_active > 0 && shifted_active < len { (shifted_active.div_ceil(8) * 8).min(len) } else { len };
        if len > 128 || len % 8 != 0 {
            return None;
        }
        if effective < len && trailing > 0 {
            let amount = self.dag.get_constant(u128::from(trailing), EVT::I8);
            op = self.dag.get_node(ISD::Srl, vt, &[op, amount]);
        }
        let splat = |byte: u128| (0..len / 8).fold(0u128, |acc, i| acc | (byte << (8 * i)));
        let (mask55, mask33, mask0f) = (
            self.dag.get_constant(splat(0x55), vt),
            self.dag.get_constant(splat(0x33), vt),
            self.dag.get_constant(splat(0x0f), vt),
        );
        let amount = |dag: &mut SelectionDAG, n: u128| dag.get_constant(n, EVT::I8);
        let one = amount(self.dag, 1);
        let half = self.dag.get_node(ISD::Srl, vt, &[op, one]);
        let half = self.dag.get_node(ISD::And, vt, &[half, mask55]);
        op = self.dag.get_node(ISD::Sub, vt, &[op, half]);
        let low = self.dag.get_node(ISD::And, vt, &[op, mask33]);
        let two = amount(self.dag, 2);
        let high = self.dag.get_node(ISD::Srl, vt, &[op, two]);
        let high = self.dag.get_node(ISD::And, vt, &[high, mask33]);
        op = self.dag.get_node(ISD::Add, vt, &[low, high]);
        let four = amount(self.dag, 4);
        let nibbles = self.dag.get_node(ISD::Srl, vt, &[op, four]);
        let sum = self.dag.get_node(ISD::Add, vt, &[op, nibbles]);
        op = self.dag.get_node(ISD::And, vt, &[sum, mask0f]);
        if effective <= 8 {
            return Some(op);
        }
        if effective == 16 {
            let eight = amount(self.dag, 8);
            let upper = self.dag.get_node(ISD::Srl, vt, &[op, eight]);
            let sum = self.dag.get_node(ISD::Add, vt, &[op, upper]);
            let ff = self.dag.get_constant(0xff, vt);
            return Some(self.dag.get_node(ISD::And, vt, &[sum, ff]));
        }
        let wide = if self.tli.is_operation_legal_or_custom(ISD::Mul, vt) {
            let mask01 = self.dag.get_constant(splat(0x01), vt);
            self.dag.get_node(ISD::Mul, vt, &[op, mask01])
        } else {
            let mut v = op;
            let mut shift = 8;
            while shift < effective {
                let amount = amount(self.dag, u128::from(shift));
                let shifted = self.dag.get_node(ISD::Shl, vt, &[v, amount]);
                v = self.dag.get_node(ISD::Add, vt, &[v, shifted]);
                shift *= 2;
            }
            v
        };
        let top = amount(self.dag, u128::from(len - 8));
        Some(self.dag.get_node(ISD::Srl, vt, &[wide, top]))
    }

    /// `TargetLowering::expandBSWAP`, for `i16` (a rotate by 8). Other widths
    /// are legal on the targets here or not yet translated.
    fn expand_bswap(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if vt != EVT::I16 {
            return None;
        }
        let amount = self.dag.get_constant(8, EVT::I8);
        Some(self.dag.get_node(ISD::Rotl, vt, &[self.dag.operand(node, 0), amount]))
    }

    /// `LegalizeOp` for a float constant: left if the target makes it
    /// (`isFPImmLegal`), else loaded from the constant pool
    /// (`ExpandConstantFP`), through a narrower pool entry when that is exact.
    fn legalize_constant_fp(
        &mut self,
        node: SDValue,
    ) -> Result<(), LegalizeError> {
        let vt = self.dag.value_type(node);
        let NodeData::ConstFp { bits } = self.dag.node(node).data else {
            unreachable!("a float constant has its bits")
        };
        let EVT::Simple(mvt) = vt else { unreachable!("floats are simple types") };
        if self.tli.is_fp_imm_legal(mvt, bits) {
            return Ok(());
        }
        // `ShouldShrinkFPConstant` (no SSE2): a double that is exactly a float
        // is stored as one.
        let (pool_vt, pool_bits) = match mvt {
            MVT::F64 => {
                let value = f64::from_bits(bits as u64);
                let narrow = value as f32;
                if f64::from(narrow).to_bits() == bits as u64 && !value.is_nan() {
                    (EVT::Simple(MVT::F32), u128::from(narrow.to_bits()))
                } else {
                    (vt, bits)
                }
            }
            _ => (vt, bits),
        };
        let pointer = self.dag.config().pointer;
        let index = self.dag.get_constant_pool(pointer, pool_bits, pool_vt, false);
        let align = match pool_vt {
            EVT::Simple(MVT::F64) => 8,
            _ => 4,
        };
        let NodeData::PoolIndex(entry) = self.dag.node(index).data else { unreachable!("a pool node has its entry") };
        let info = MemInfo { mem_vt: pool_vt, align, volatile: false, base: MemBase::ConstantPool(entry), ext: None };
        let chain = self.dag.entry_node();
        let loaded = self.dag.get_load(vt, chain, index, info);
        self.replace_node(node, loaded);
        Ok(())
    }

    /// `PromoteNode` for a conversion between an integer and a float: the
    /// integer side is widened (sign or zero extended going in, truncated
    /// coming out).
    fn promote_conversion(
        &mut self,
        node: SDValue,
    ) -> Result<(), LegalizeError> {
        let op = self.dag.opcode(node);
        let vt = self.dag.value_type(node);
        let result = match op {
            ISD::SIntToFp | ISD::UIntToFp => {
                let src = self.dag.operand(node, 0);
                let EVT::Simple(from) = self.dag.value_type(src) else {
                    unreachable!("promotion applies to simple types")
                };
                let to: EVT = self.tli.type_to_promote_to(op, from).into();
                let widened =
                    self.dag.get_node(if op == ISD::SIntToFp { ISD::SignExtend } else { ISD::ZeroExtend }, to, &[src]);
                self.dag.get_node(op, vt, &[widened])
            }
            _ => {
                let EVT::Simple(from) = vt else { unreachable!("promotion applies to simple types") };
                let to: EVT = self.tli.type_to_promote_to(op, from).into();
                let wide = self.dag.get_node(op, to, &[self.dag.operand(node, 0)]);
                self.dag.get_node(ISD::Truncate, vt, &[wide])
            }
        };
        self.replace_node(node, result);
        Ok(())
    }

    /// `ReplaceNode(Old, New)` for a node with one value result.
    fn replace_node(
        &mut self,
        old: SDValue,
        new: SDValue,
    ) {
        self.dag.replace_all_uses_with(old, new);
    }

    /// `PromoteNode`, for the simple integer operations: make the node in the
    /// wider type from any-extended operands and truncate the result.
    fn promote_node(
        &mut self,
        node: SDValue,
    ) -> Result<(), LegalizeError> {
        let op = self.dag.opcode(node);
        if matches!(
            op,
            ISD::SIntToFp | ISD::UIntToFp | ISD::FpToSInt | ISD::FpToUInt
        ) {
            return self.promote_conversion(node);
        }
        let EVT::Simple(from) = self.dag.value_type(node) else { unreachable!("promotion applies to simple types") };
        let to: EVT = self.tli.type_to_promote_to(op, from).into();
        let result = match op {
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor => {
                let (lhs, rhs) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let lhs = self.dag.get_node(ISD::AnyExtend, to, &[lhs]);
                let rhs = self.dag.get_node(ISD::AnyExtend, to, &[rhs]);
                let wide = self.dag.get_node(op, to, &[lhs, rhs]);
                self.dag.get_node(ISD::Truncate, from.into(), &[wide])
            }
            ISD::CtTz | ISD::CtTzZeroPoison | ISD::CtLz | ISD::CtPop => {
                // The argument is any-extended for a trailing count,
                // zero-extended otherwise.
                let src = self.dag.operand(node, 0);
                let wide = self
                    .dag
                    .get_node(
                        if matches!(op, ISD::CtTz | ISD::CtTzZeroPoison) { ISD::AnyExtend } else { ISD::ZeroExtend },
                        to,
                        &[src],
                    );
                let (old_bits, new_bits) = (u128::from(EVT::from(from).size_in_bits()), u128::from(to.size_in_bits()));
                let (wide, new_op) = if op == ISD::CtTz {
                    // Zero counts the original width: set the bit just above
                    // it.
                    let top = self.dag.get_constant(1 << old_bits, to);
                    (self.dag.get_node(ISD::Or, to, &[wide, top]), ISD::CtTzZeroPoison)
                } else {
                    (wide, op)
                };
                let mut count = self.dag.get_node(new_op, to, &[wide]);
                if new_op == ISD::CtLz {
                    let extra = self.dag.get_constant(new_bits - old_bits, to);
                    count = self.dag.get_node(ISD::Sub, to, &[count, extra]);
                }
                self.dag.get_node(ISD::Truncate, from.into(), &[count])
            }
            ISD::CtLzZeroPoison => {
                // Not zero, so shifting the argument to the top keeps the
                // count.
                let src = self.dag.operand(node, 0);
                let wide = self.dag.get_node(ISD::AnyExtend, to, &[src]);
                let shift =
                    self.dag.get_constant(u128::from(to.size_in_bits() - EVT::from(from).size_in_bits()), EVT::I8);
                let top = self.dag.get_node(ISD::Shl, to, &[wide, shift]);
                let count = self.dag.get_node(op, to, &[top]);
                self.dag.get_node(ISD::Truncate, from.into(), &[count])
            }
            _ => return Err(LegalizeError::Unsupported { op, ty: from.into(), what: "promote node" }),
        };
        self.replace_node(node, result);
        Ok(())
    }
}
