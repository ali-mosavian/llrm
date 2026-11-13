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
//! DEVIATE SD203: `ExpandConstantFP` shrinks an `f80` constant by its bit
//! pattern, not `APFloat::convert` (no `APFloat`, SD11): a normal value whose
//! low mantissa bits are zero. The pool entry's alignment is the i386 data
//! layout's preferred one by type. Exit: with an `APFloat`.
//! Owner sdag-b. Review 2026-11-27.
//!
//! # See also
//! LLVM's `SelectionDAG::Legalize`
//! ([L6301](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L6301)),
//! `LegalizeOp`
//! ([L982](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L982)),
//! `PromoteNode`
//! ([L5432](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L5432)).

use llrm_mir::hash::HashSet;
use llrm_sdag::lowering::ShiftAmount;

use super::actions::{Action, TargetLowering};
use super::dag::{MemBase, MemInfo, NodeData, SDNodeFlags, SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
use super::legalize_types::LegalizeError;
use super::mvt::{EVT, MVT};

/// `FloatSignAsInt`: a float's sign bit as part of an integer.
struct FloatSignAsInt {
    float_vt: EVT,
    int_value: SDValue,
    sign_mask: u128,
    sign_bit: u32,
    /// Set where the float went through memory.
    spill: Option<SignSpill>,
}

/// The stack slot a float's sign byte is read from and written back to.
struct SignSpill {
    chain: SDValue,
    float_ptr: SDValue,
    float_info: MemInfo,
    int_ptr: SDValue,
    int_info: MemInfo,
}

/// The bits of an integer type, as a mask.
fn mask_of(vt: EVT) -> u128 {
    match vt.size_in_bits() {
        128 => u128::MAX,
        bits => (1u128 << bits) - 1,
    }
}

/// What the target supplies (`TLI.LowerOperation`).
pub trait Hooks {
    /// The legal replacement of `node` (its first result), or `None` to have
    /// the node expanded instead.
    fn lower_operation(
        &mut self,
        dag: &mut SelectionDAG,
        node: SDValue,
    ) -> Option<SDValue>;

    /// `TargetLowering::makeLibCall`: a call to the routine `name` with
    /// `args`, whose result is a `ret`; `None` where the target cannot make
    /// one.
    /// `getStackPointerRegisterToSaveRestore` and the frame's stack alignment:
    /// the register `STACKSAVE` reads and `STACKRESTORE` and
    /// `DYNAMIC_STACKALLOC` write; none: the target lowers them itself.
    fn stack_pointer(&self) -> Option<(llrm_lir::registers::RegId, u64)> {
        None
    }

    fn make_lib_call(
        &mut self,
        _dag: &mut SelectionDAG,
        _name: &'static str,
        _args: &[SDValue],
        _ret: EVT,
        _signed: bool,
    ) -> Option<SDValue> {
        None
    }

    /// `makeLibCall` after type legalization, for a result too wide for a
    /// register: the routine `name` with `args`, its result the parts `rets`
    /// (the pin's `MERGE_VALUES` operands, in register order); `None` where the
    /// target cannot make one.
    fn make_lib_call_parts(
        &mut self,
        _dag: &mut SelectionDAG,
        _name: &'static str,
        _args: &[SDValue],
        _rets: &[EVT],
        _signed: bool,
    ) -> Option<Vec<SDValue>> {
        None
    }

    /// `DataLayout::isLittleEndian`: which half of a split argument or result
    /// comes first.
    fn little_endian(&self) -> bool {
        true
    }
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
    /// The topological sweeps `run` made.
    pub sweeps: u32,
}

impl<'a, H: Hooks> DagLegalizer<'a, H> {
    pub fn new(
        dag: &'a mut SelectionDAG,
        tli: &'a TargetLowering,
        hooks: &'a mut H,
    ) -> DagLegalizer<'a, H> {
        let legalized = HashSet::with_capacity_and_hasher(dag.nodes().len(), Default::default());
        DagLegalizer { dag, tli, hooks, legalized, sweeps: 0 }
    }

    /// `SelectionDAG::Legalize`.
    pub fn run(&mut self) -> Result<(), LegalizeError> {
        loop {
            let mut any = false;
            let nodes = self.dag.nodes().len();
            self.sweeps += 1;
            for n in self.reverse_topological() {
                if self.is_dead(n) || !self.legalized.insert(n) {
                    continue;
                }
                any = true;
                self.dag.cur_order = self.dag.node(SDValue { node: n, res: 0 }).order;
                self.legalize_op(n)?;
                // `ReplaceAllUsesWith` deletes the node it replaced, and what
                // only it read.
                self.dag.delete_replaced(n);
            }
            // A sweep that built no node leaves nothing new to legalize.
            if !any || self.dag.nodes().len() == nodes {
                return Ok(());
            }
        }
    }

    /// The nodes reachable from the root, users before their operands.
    fn reverse_topological(&self) -> Vec<u32> {
        let mut order = Vec::new();
        let mut seen = HashSet::with_capacity_and_hasher(self.dag.nodes().len(), Default::default());
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
        !(0..results).any(|res| self.dag.has_users(SDValue { node: n, res }))
    }

    /// The type the action table is asked about for `value`'s node (the
    /// switch at the top of `LegalizeOp`).
    fn action_type(
        &self,
        node: SDValue,
    ) -> EVT {
        match self.dag.opcode(node) {
            ISD::Br | ISD::BrCond | ISD::BrCC | ISD::BrJT | ISD::VAArg => EVT::OTHER,
            ISD::SIntToFp | ISD::UIntToFp | ISD::LRint | ISD::LLRint => self.dag.value_type(self.dag.operand(node, 0)),
            ISD::SignExtendInReg => match self.dag.node(self.dag.operand(node, 1)).data {
                NodeData::ValueType(inner) => inner,
                _ => self.dag.value_type(node),
            },
            // `SETCCCARRY` is asked of its operands' type, as `SETCC` is
            // (`LegalizeOp`'s `case ISD::SETCCCARRY`).
            ISD::SetCC | ISD::SetCCCarry | ISD::Store => {
                self.dag.value_type(self.dag.operand(node, if self.dag.opcode(node) == ISD::Store { 1 } else { 0 }))
            }
            _ => self.dag.value_type(node),
        }
    }

    /// `LegalizeOp`'s adjustment of a shift or rotate amount to the target's
    /// shift amount type ([L1298][pin]; X86 has `i8` for scalars):
    /// `getShiftAmountOperand` on the amount, which is operand 1, or
    /// operand 2 of a funnel shift or a double-width shift.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L1298
    fn fix_shift_amount(
        &mut self,
        n: u32,
    ) -> u32 {
        let at = SDValue { node: n, res: 0 };
        let amount_at = match self.dag.opcode(at) {
            ISD::Shl | ISD::Srl | ISD::Sra | ISD::Rotl | ISD::Rotr => 1,
            ISD::FShl | ISD::FShr | ISD::ShlParts | ISD::SrlParts | ISD::SraParts => 2,
            _ => return n,
        };
        // `getShiftAmountOperand`: a vector shift's amount is a vector of the
        // value's type and stays as it is (it was truncated to a scalar).
        if self.dag.value_type(at).is_vector() {
            return n;
        }
        let amount = self.dag.operand(at, amount_at);
        let want = match self.tli.types.shift_amount() {
            ShiftAmount::I8 => EVT::I8,
            ShiftAmount::ByValue => self.dag.value_type(at),
            ShiftAmount::Fixed(vt) => EVT::Simple(vt),
        };
        if !self.dag.value_type(amount).is_integer() || self.dag.value_type(amount) == want {
            return n;
        }
        let mut ops: Vec<SDValue> = (0..=amount_at).map(|i| self.dag.operand(at, i)).collect();
        ops[amount_at] = self.dag.get_zext_or_trunc(amount, want);
        let new = self.dag.update_node_operands(n, &ops);
        if new != n {
            let results = self.dag.node(at).values.len() as u8;
            for res in 0..results {
                self.replace_node(SDValue { node: n, res }, SDValue { node: new, res });
            }
        }
        new
    }

    /// `LegalizeLoadOps` for an extending load of memory that is not a power
    /// of two wide (`i56`, seven bytes): a zero-extending load of each power of
    /// two it is the sum of, shifted into place and or-ed, sign-extended in
    /// the register for a signed load. The value and the chain it replaces.
    fn expand_odd_ext_load(
        &mut self,
        node: SDValue,
    ) -> Option<(SDValue, SDValue)> {
        if self.dag.opcode(node) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        let vt = self.dag.value_type(node);
        let pieces = super::legalize_types::odd_memory(info.mem_vt)
            .filter(|_| vt.size_in_bits() > info.mem_vt.size_in_bits())?;
        let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let mut joined: Option<SDValue> = None;
        let mut chains = Vec::new();
        let mut at = 0u32;
        for piece in pieces {
            let piece_ptr = if at == 0 {
                ptr
            } else {
                let by = self.dag.get_constant(u128::from(at / 8), self.dag.value_type(ptr));
                self.dag.get_node(ISD::Add, self.dag.value_type(ptr), &[ptr, by])
            };
            let piece_info = MemInfo {
                mem_vt: piece,
                align: if at == 0 { info.align } else { info.align.min(at / 8) },
                ext: Some(super::actions::LoadExt::ZExtLoad),
                ..info
            };
            let part = self.dag.get_ext_load(super::actions::LoadExt::ZExtLoad, vt, chain, piece_ptr, piece_info);
            chains.push(SDValue { node: part.node, res: 1 });
            // the pieces lie widest first in memory: the low bits first on a
            // little-endian target, the high bits first on a big-endian one
            let shift =
                if self.dag.config().big_endian { info.mem_vt.size_in_bits() - at - piece.size_in_bits() } else { at };
            let part = if shift == 0 {
                part
            } else {
                let by = self.dag.get_constant(u128::from(shift), EVT::I8);
                self.dag.get_node(ISD::Shl, vt, &[part, by])
            };
            joined = Some(match joined {
                Some(low) => self.dag.get_node(ISD::Or, vt, &[low, part]),
                None => part,
            });
            at += piece.size_in_bits();
        }
        let mut value = joined?;
        if info.ext == Some(super::actions::LoadExt::SExtLoad) {
            let ty = self.dag.get_value_type_node(info.mem_vt);
            value = self.dag.get_node(ISD::SignExtendInReg, vt, &[value, ty]);
        }
        Some((value, self.dag.get_token_factor(&chains)))
    }

    /// `LegalizeLoadOps`, extending loads of memory narrower than its store
    /// size (`i1`): with the action `Promote`, a byte-sized extending load of
    /// the same kind, and for a sign extension the bit sign-extended in the
    /// register. The value and the chain it replaces.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L483>
    fn promote_sub_byte_ext_load(
        &mut self,
        node: SDValue,
    ) -> Option<(SDValue, SDValue)> {
        if self.dag.opcode(node) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        let ext = info.ext?;
        let (EVT::Simple(wide), EVT::Simple(mem)) = (self.dag.value_type(node), info.mem_vt) else { return None };
        let bits = info.mem_vt.size_in_bits();
        if bits == 0 || bits % 8 == 0 || bits > 8 || self.tli.load_ext_action(ext, wide, mem) != Action::Promote {
            return None;
        }
        let vt = self.dag.value_type(node);
        let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let kind = if ext == super::actions::LoadExt::ZExtLoad {
            super::actions::LoadExt::ZExtLoad
        } else {
            super::actions::LoadExt::ExtLoad
        };
        let narrow = MemInfo { mem_vt: EVT::I8, ext: None, ..info };
        let byte = if vt == EVT::I8 {
            self.dag.get_load(vt, chain, ptr, narrow)
        } else {
            self.dag.get_ext_load(kind, vt, chain, ptr, narrow)
        };
        let value = if ext == super::actions::LoadExt::SExtLoad {
            let ty = self.dag.get_value_type_node(info.mem_vt);
            self.dag.get_node(ISD::SignExtendInReg, vt, &[byte, ty])
        } else {
            byte
        };
        Some((value, SDValue { node: byte.node, res: 1 }))
    }

    /// `LegalizeLoadOps`, extending loads: one the target does not have is an
    /// any-extending load and an extension in the register. The value and
    /// the chain it replaces.
    fn expand_ext_load(
        &mut self,
        node: SDValue,
    ) -> Option<(SDValue, SDValue)> {
        if self.dag.opcode(node) != ISD::Load {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        let ext = info.ext.filter(|ext| *ext != super::actions::LoadExt::ExtLoad)?;
        let (EVT::Simple(wide), EVT::Simple(mem)) = (self.dag.value_type(node), info.mem_vt) else { return None };
        if self.tli.load_ext_action(ext, wide, mem) != Action::Expand {
            return None;
        }
        let vt = self.dag.value_type(node);
        let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let any =
            self.dag.get_ext_load(super::actions::LoadExt::ExtLoad, vt, chain, ptr, MemInfo { ext: None, ..info });
        let value = if ext == super::actions::LoadExt::SExtLoad {
            let ty = self.dag.get_value_type_node(info.mem_vt);
            self.dag.get_node(ISD::SignExtendInReg, vt, &[any, ty])
        } else {
            let mask = self.dag.get_constant(super::fold::mask(info.mem_vt.size_in_bits()), vt);
            self.dag.get_node(ISD::And, vt, &[any, mask])
        };
        Some((value, SDValue { node: any.node, res: 1 }))
    }

    /// `LegalizeStoreOps`, a truncating store of memory that is not a whole
    /// number of bytes ("TRUNCSTORE:i1 X -> TRUNCSTORE:i8 (and X, 1)"): a
    /// byte-sized truncating store of the value with its upper bits zero.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L1260>
    fn promote_sub_byte_trunc_store(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(node) != ISD::Store {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        let (chain, value, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let vt = self.dag.value_type(value);
        let bits = info.mem_vt.size_in_bits();
        if !info.mem_vt.is_integer() || bits % 8 == 0 || bits >= vt.size_in_bits() {
            return None;
        }
        let mask = self.dag.get_constant(super::fold::mask(bits), vt);
        let value = self.dag.get_node(ISD::And, vt, &[value, mask]);
        let byte = MemInfo { mem_vt: EVT::integer(bits.div_ceil(8) * 8), ..info };
        Some(self.dag.get_store(chain, value, ptr, byte))
    }

    /// `LegalizeStoreOps`, a truncating store the target does not have
    /// (`getTruncStoreAction` is `Expand`): the value truncated to the memory
    /// type, then a plain store of it.
    ///
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L1346>
    fn expand_trunc_store(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        if self.dag.opcode(node) != ISD::Store {
            return None;
        }
        let NodeData::Mem(info) = self.dag.node(node).data else { return None };
        let (chain, value, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let (EVT::Simple(wide), EVT::Simple(mem)) = (self.dag.value_type(value), info.mem_vt) else { return None };
        if info.mem_vt.size_in_bits() >= self.dag.value_type(value).size_in_bits()
            || self.tli.trunc_store_action(wide, mem) != Action::Expand
        {
            return None;
        }
        let truncated = self.dag.get_node(ISD::Truncate, info.mem_vt, &[value]);
        Some(self.dag.get_store(chain, truncated, ptr, info))
    }

    /// `LegalizeOp`.
    fn legalize_op(
        &mut self,
        n: u32,
    ) -> Result<(), LegalizeError> {
        let n = self.fix_shift_amount(n);
        let node = SDValue { node: n, res: 0 };
        if matches!(self.dag.opcode(node), ISD::TargetConstant | ISD::Register) {
            return Ok(());
        }
        if self.dag.opcode(node) == ISD::ConstantFP {
            return self.legalize_constant_fp(node);
        }
        if let Some(store) = self.promote_sub_byte_trunc_store(node).or_else(|| self.expand_trunc_store(node)) {
            self.replace_node(node, store);
            return Ok(());
        }
        if let Some(result) = self
            .expand_odd_ext_load(node)
            .or_else(|| self.promote_sub_byte_ext_load(node))
            .or_else(|| self.expand_ext_load(node))
        {
            self.replace_node(node, result.0);
            self.replace_node(SDValue { node: n, res: 1 }, result.1);
            return Ok(());
        }
        let vt = self.action_type(node);
        let mut action = match vt {
            EVT::Simple(_) => self.tli.operation_action(self.dag.opcode(node), vt),
            EVT::Int(_) => Action::Expand,
        };
        // `EXTRACT_ELEMENT` lies about being legal: when it claims to be, it is
        // expanded. The same node taking a vector is `EXTRACT_VECTOR_ELT`.
        if self.dag.opcode(node) == ISD::ExtractElement
            && action == Action::Legal
            && !self.dag.value_type(self.dag.operand(node, 0)).is_vector()
        {
            action = Action::Expand;
        }
        if self.dag.opcode(node) == ISD::SetCC
            && let (EVT::Simple(operand), NodeData::CondCode(cc)) = (vt, self.dag.node(self.dag.operand(node, 2)).data)
            && self.tli.cond_code_action(cc, operand) == Action::Expand
        {
            action = Action::Expand;
        }
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
                None => self.convert_node_to_libcall(node, vt),
            },
            Action::LibCall => self.convert_node_to_libcall(node, vt),
            Action::Custom => {
                Err(LegalizeError::Unsupported { op: self.dag.opcode(node), ty: vt, what: "expand node" })
            }
        }
    }

    /// `ConvertNodeToLibcall` for the integer `MUL`, `SDIV`, `UDIV`, `SREM`
    /// and `UREM` (`ExpandIntLibCall`): the target's routine for the type,
    /// called on the entry chain, as the pin does.
    fn convert_node_to_libcall(
        &mut self,
        node: SDValue,
        vt: EVT,
    ) -> Result<(), LegalizeError> {
        let op = self.dag.opcode(node);
        let refused = LegalizeError::Unsupported { op, ty: vt, what: "convert node to libcall" };
        // `ExpandBitCountingLibCall` (CTLZ_ZERO_POISON, CTPOP; DEVIATE BC1 adds CTTZ_ZERO_POISON): the value as is, the
        // routine's `int` result sign-extended or truncated to the node's type.
        if matches!(op, ISD::CtLzZeroPoison | ISD::CtPop | ISD::CtTzZeroPoison) {
            let Some(name) = self.tli.libcall_name(op, vt) else { return Err(refused) };
            let int = EVT::integer(self.tli.types.int_size());
            let arg = self.dag.operand(node, 0);
            let Some(call) = self.hooks.make_lib_call(self.dag, name, &[arg], int, false) else { return Err(refused) };
            let ret = self.dag.value_type(node);
            let result = self.dag.get_sext_or_trunc(call, ret);
            self.replace_node(node, result);
            return Ok(());
        }
        let (signed, arity) = match op {
            ISD::SDiv | ISD::SRem => (true, 2),
            ISD::UDiv | ISD::URem | ISD::Mul => (false, 2),
            // `ExpandFPLibCall`: every operand, none signed.
            ISD::FRem | ISD::FAdd | ISD::FSub | ISD::FMul | ISD::FDiv => (false, 2),
            ISD::FSin
            | ISD::FCos
            | ISD::FAtan
            | ISD::FLog
            | ISD::FLog2
            | ISD::FExp
            | ISD::FExp2
            | ISD::LRint
            | ISD::LLRint => (false, 1),
            _ => return Err(refused),
        };
        let Some(name) = self.tli.libcall_name(op, vt) else { return Err(refused) };
        let args: Vec<SDValue> = (0..arity).map(|at| self.dag.operand(node, at)).collect();
        // `vt` is the type the action is keyed by: `LRINT` of a float returns
        // an integer.
        let ret = self.dag.value_type(node);
        let Some(result) = self.hooks.make_lib_call(self.dag, name, &args, ret, signed) else { return Err(refused) };
        self.replace_node(node, result);
        Ok(())
    }

    /// `ExpandNode` for `BR_JT`: load the entry at `table + index * size` and
    /// branch to it.
    fn expand_br_jt(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let (chain, table, index) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
        let pointer = self.dag.config().pointer;
        // A 64-bit table is position independent: 32-bit offsets from the
        // table (`EK_LabelDifference32`), a pointer's worth of entry otherwise.
        let relative = self.dag.relative_jump_tables();
        let entry = if relative { 4 } else { pointer.size_in_bits() / 8 };
        let index_vt = self.dag.value_type(index);
        let scale = self.dag.get_constant(u128::from(entry.trailing_zeros()), EVT::I8);
        let index = self.dag.get_node(ISD::Shl, index_vt, &[index, scale]);
        let address = self.dag.get_node(ISD::Add, pointer, &[table, index]);
        let info = MemInfo {
            mem_vt: pointer,
            align: entry,
            volatile: false,
            base: MemBase::JumpTable,
            ext: None,
            space: 0,
            ordering: None,
        };
        if relative {
            let info = MemInfo { mem_vt: EVT::I32, ..info };
            let offset = self.dag.get_ext_load(super::actions::LoadExt::SExtLoad, pointer, chain, address, info);
            // `getPICJumpTableRelocBase`: the table itself at 64 bits.
            let target = self.dag.get_node(ISD::Add, pointer, &[offset, table]);
            return self.dag.get_node(ISD::BrInd, EVT::OTHER, &[SDValue { node: offset.node, res: 1 }, target]);
        }
        let target = self.dag.get_load(pointer, chain, address, info);
        self.dag.get_node(ISD::BrInd, EVT::OTHER, &[SDValue { node: target.node, res: 1 }, target])
    }

    /// `getLogicalNOT`: `xor v, 1` (`-1` where booleans are all ones).
    fn get_logical_not(
        &mut self,
        value: SDValue,
    ) -> SDValue {
        let vt = self.dag.value_type(value);
        match self.dag.config().boolean_contents {
            super::dag::BooleanContents::ZeroOrNegativeOne => self.dag.get_not(value, vt),
            _ => {
                let one = self.dag.get_constant(1, vt);
                self.dag.get_node(ISD::Xor, vt, &[value, one])
            }
        }
    }

    /// `TargetLowering::LegalizeSetCCCondCode` of a `SETCC` whose code the
    /// target expands: the operands swapped, the condition inverted (and
    /// the result negated), or two compares joined by `and` or `or`. The `i1`
    /// case is not translated.
    fn expand_setcc_code(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let NodeData::CondCode(cc) = self.dag.node(self.dag.operand(node, 2)).data else { return None };
        let (lhs, rhs, vt) = (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.value_type(node));
        let EVT::Simple(op_vt) = self.dag.value_type(lhs) else { return None };
        let tli = self.tli;
        let legal_or_custom = |cc: CondCode| matches!(
            tli.cond_code_action(cc, op_vt),
            Action::Legal | Action::Custom
        );
        let legal = |cc: CondCode| tli.cond_code_action(cc, op_vt) == Action::Legal;
        let float = self.dag.value_type(lhs).is_floating_point();
        // Swapping the operands.
        let swapped = cc.swapped_operands();
        if legal_or_custom(swapped) {
            return Some(self.dag.get_setcc(vt, rhs, lhs, swapped));
        }
        // Inverting the condition, then swapping on top of it.
        let mut inverse = if float { cc.inverse_float() } else { cc.inverse_integer() };
        let mut swap = false;
        if !legal_or_custom(inverse) {
            inverse = inverse.swapped_operands();
            swap = true;
        }
        if legal_or_custom(inverse) {
            let (l, r) = if swap { (rhs, lhs) } else { (lhs, rhs) };
            let compare = self.dag.get_setcc(vt, l, r, inverse);
            return Some(self.get_logical_not(compare));
        }
        if !float {
            // An integer code with no swap or inverse the target takes: x86's
            // `oeq`/`une` are the only ones here.
            return None;
        }
        let unordered = cc as u8 & 0x8 != 0;
        let generic = |cc: CondCode| {
            (
                CondCode::ALL_CODES[((cc as u8 & 0x7) | 0x10) as usize],
                if unordered { CondCode::SetUo } else { CondCode::SetO },
                if unordered { ISD::Or } else { ISD::And },
                false,
            )
        };
        let (cc1, cc2, opcode, invert) = match cc {
            CondCode::SetUo if legal(CondCode::SetUNe) => (CondCode::SetUNe, CondCode::SetUNe, ISD::Or, false),
            CondCode::SetUo => (CondCode::SetOEq, CondCode::SetOEq, ISD::And, true),
            CondCode::SetO => (CondCode::SetOEq, CondCode::SetOEq, ISD::And, false),
            CondCode::SetONe | CondCode::SetUEq
                if !legal(if unordered { CondCode::SetUo } else { CondCode::SetO })
                    && (legal(CondCode::SetOGt) || legal(CondCode::SetOLt)) =>
            {
                (CondCode::SetOGt, CondCode::SetOLt, ISD::Or, unordered)
            }
            _ => generic(cc),
        };
        let (first, second) = if cc != CondCode::SetO && cc != CondCode::SetUo {
            (self.dag.get_setcc(vt, lhs, rhs, cc1), self.dag.get_setcc(vt, lhs, rhs, cc2))
        } else {
            (self.dag.get_setcc(vt, lhs, lhs, cc1), self.dag.get_setcc(vt, rhs, rhs, cc2))
        };
        let joined = self.dag.get_node(opcode, vt, &[first, second]);
        Some(if invert { self.get_logical_not(joined) } else { joined })
    }

    /// `ExpandBitCast` of a float to or from an integer of its size, with no
    /// register move between the classes: `EmitStackConvert`, a store to a
    /// stack slot of the source type and a load as the destination type.
    fn expand_bitcast(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let src = self.dag.operand(node, 0);
        let (from, to) = (self.dag.value_type(src), self.dag.value_type(node));
        let size = from.size_in_bits().div_ceil(8);
        let cell = self.dag.create_stack_temporary(size, 4);
        let pointer = self.dag.config().pointer;
        let slot = self.dag.get_frame_index(pointer, cell);
        let info = MemInfo {
            mem_vt: from,
            align: 4,
            volatile: false,
            base: MemBase::Stack(cell),
            ext: None,
            space: 0,
            ordering: None,
        };
        let entry = self.dag.entry_node();
        let store = self.dag.get_store(entry, src, slot, info);
        self.dag.get_load(to, store, slot, MemInfo { mem_vt: to, ..info })
    }

    /// `TargetLowering::expandIntMINMAX`: a select on a compare, reusing a
    /// `getSignAsIntValue`: the sign bit of float `value` as part of an
    /// integer. An integer of the float's width, where the target has one;
    /// else the float is stored to a stack slot and the byte with the sign bit
    /// loaded.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L1640>
    fn sign_as_int(
        &mut self,
        value: SDValue,
    ) -> FloatSignAsInt {
        let float_vt = self.dag.value_type(value);
        let bits = float_vt.size_in_bits();
        let int_vt = EVT::integer(bits);
        if self.tli.types.is_type_legal(int_vt) {
            return FloatSignAsInt {
                float_vt,
                int_value: self.dag.get_node(ISD::BitCast, int_vt, &[value]),
                sign_mask: 1u128 << (bits - 1),
                sign_bit: bits - 1,
                spill: None,
            };
        }
        // The register type of `i8`.
        let load_ty = match self.tli.types.type_conversion(EVT::I8) {
            (llrm_sdag::lowering::TypeAction::Legal, _) => EVT::I8,
            (_, wider) => wider,
        };
        // DEVIATE FNEG-ALIGN (owner arm64-phi, review 2026-11-20): the slot is
        // aligned to the float's size up to 16, where the pin asks the
        // data layout for both types' preferred alignment.
        let align = (bits / 8).next_power_of_two().clamp(4, 16);
        let cell = self.dag.create_stack_temporary(bits.div_ceil(8), align);
        let pointer = self.dag.config().pointer;
        let float_ptr = self.dag.get_frame_index(pointer, cell);
        let float_info = MemInfo {
            mem_vt: float_vt,
            align,
            volatile: false,
            base: MemBase::Stack(cell),
            ext: None,
            space: 0,
            ordering: None,
        };
        let entry = self.dag.entry_node();
        let chain = self.dag.get_store(entry, value, float_ptr, float_info);
        let (int_ptr, int_align) = if self.dag.config().big_endian {
            (float_ptr, align)
        } else {
            // The byte that holds the sign bit is the last.
            let at = self.dag.get_constant(u128::from(bits / 8 - 1), pointer);
            (self.dag.get_mem_base_plus_offset(float_ptr, at, SDNodeFlags::default()), 1)
        };
        let int_info = MemInfo { mem_vt: EVT::I8, align: int_align, ..float_info };
        let int_value = self.dag.get_ext_load(super::actions::LoadExt::ExtLoad, load_ty, chain, int_ptr, int_info);
        FloatSignAsInt {
            float_vt,
            int_value,
            sign_mask: 1 << 7,
            sign_bit: 7,
            spill: Some(SignSpill { chain, float_ptr, float_info, int_ptr, int_info }),
        }
    }

    /// `modifySignAsInt`: the float whose sign part is `new_int`.
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L1692>
    fn modify_sign_as_int(
        &mut self,
        state: &FloatSignAsInt,
        new_int: SDValue,
    ) -> SDValue {
        let Some(spill) = &state.spill else { return self.dag.get_node(ISD::BitCast, state.float_vt, &[new_int]) };
        let chain = self.dag.get_store(spill.chain, new_int, spill.int_ptr, spill.int_info);
        self.dag.get_load(state.float_vt, chain, spill.float_ptr, spill.float_info)
    }

    /// `ExpandFNEG`: the sign bit flipped.
    fn expand_fneg(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let state = self.sign_as_int(self.dag.operand(node, 0));
        let int_vt = self.dag.value_type(state.int_value);
        let mask = self.dag.get_constant(state.sign_mask, int_vt);
        let flipped = self.dag.get_node(ISD::Xor, int_vt, &[state.int_value, mask]);
        self.modify_sign_as_int(&state, flipped)
    }

    /// `ExpandFABS`: `FCOPYSIGN(x, 0.0)` where that is legal, else the sign bit
    /// cleared.
    fn expand_fabs(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let value = self.dag.operand(node, 0);
        let float_vt = self.dag.value_type(value);
        if self.tli.is_operation_legal_or_custom(ISD::FCopySign, float_vt) {
            let zero = self.dag.get_constant_fp(0, float_vt);
            return self.dag.get_node(ISD::FCopySign, float_vt, &[value, zero]);
        }
        let state = self.sign_as_int(value);
        let int_vt = self.dag.value_type(state.int_value);
        let keep = self.dag.get_constant(!state.sign_mask & mask_of(int_vt), int_vt);
        let cleared = self.dag.get_node(ISD::And, int_vt, &[state.int_value, keep]);
        self.modify_sign_as_int(&state, cleared)
    }

    /// `ExpandFCOPYSIGN`: the magnitude's bits with the sign operand's sign bit
    /// moved into place.
    fn expand_fcopysign(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let (magnitude, sign) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let sign_state = self.sign_as_int(sign);
        let int_vt = self.dag.value_type(sign_state.int_value);
        let mask = self.dag.get_constant(sign_state.sign_mask, int_vt);
        let mut sign_bit = self.dag.get_node(ISD::And, int_vt, &[sign_state.int_value, mask]);
        let float_vt = self.dag.value_type(magnitude);
        if self.tli.is_operation_legal_or_custom(ISD::FAbs, float_vt)
            && self.tli.is_operation_legal_or_custom(ISD::FNeg, float_vt)
        {
            let abs = self.dag.get_node(ISD::FAbs, float_vt, &[magnitude]);
            let neg = self.dag.get_node(ISD::FNeg, float_vt, &[abs]);
            let zero = self.dag.get_constant(0, int_vt);
            let bool_vt = self.tli.types.set_cc_result_type(int_vt);
            let test = self.dag.get_setcc(bool_vt, sign_bit, zero, CondCode::SetNe);
            return self.dag.get_select(float_vt, test, neg, abs);
        }
        let magnitude_state = self.sign_as_int(magnitude);
        let mag_vt = self.dag.value_type(magnitude_state.int_value);
        let keep = self.dag.get_constant(!magnitude_state.sign_mask & mask_of(mag_vt), mag_vt);
        let cleared = self.dag.get_node(ISD::And, mag_vt, &[magnitude_state.int_value, keep]);
        // The sign bit at the place of the magnitude's.
        let by = i64::from(sign_state.sign_bit) - i64::from(magnitude_state.sign_bit);
        let mut shift_vt = int_vt;
        if int_vt.size_in_bits() < mag_vt.size_in_bits() {
            sign_bit = self.dag.get_node(ISD::ZeroExtend, mag_vt, &[sign_bit]);
            shift_vt = mag_vt;
        }
        if by != 0 {
            let amount = self.dag.get_constant(by.unsigned_abs() as u128, shift_vt);
            let op = if by > 0 { ISD::Srl } else { ISD::Shl };
            sign_bit = self.dag.get_node(op, shift_vt, &[sign_bit, amount]);
        }
        if self.dag.value_type(sign_bit).size_in_bits() > mag_vt.size_in_bits() {
            sign_bit = self.dag.get_node(ISD::Truncate, mag_vt, &[sign_bit]);
        }
        let copied = self.dag.get_node_flags(ISD::Or, mag_vt, &[cleared, sign_bit], SDNodeFlags::DISJOINT);
        self.modify_sign_as_int(&magnitude_state, copied)
    }

    /// compare of the same operands when the DAG has one. (This target has no
    /// `USUBSAT`, so the `usubsat` forms are not taken.)
    fn expand_min_max(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let op = self.dag.opcode(node);
        let (x, y) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
        let vt = self.dag.value_type(node);
        let alternative = match op {
            ISD::SMax => ISD::UMax,
            ISD::SMin => ISD::UMin,
            ISD::UMax => ISD::SMax,
            _ => ISD::SMin,
        };
        let sign_clear = |dag: &SelectionDAG, v: SDValue| {
            super::known_bits::known_bits(dag, v).zero >> (vt.size_in_bits() - 1) & 1 == 1
        };
        if self.tli.is_operation_legal(alternative, vt) && sign_clear(self.dag, x) && sign_clear(self.dag, y) {
            return self.dag.get_node(alternative, vt, &[x, y]);
        }
        let bool_vt = self.tli.types.set_cc_result_type(vt);
        let (preferred, other, preferred_swapped, other_swapped) = match op {
            ISD::SMax => (CondCode::SetGt, CondCode::SetGe, CondCode::SetLt, CondCode::SetLe),
            ISD::SMin => (CondCode::SetLt, CondCode::SetLe, CondCode::SetGt, CondCode::SetGe),
            ISD::UMax => (CondCode::SetUGt, CondCode::SetUGe, CondCode::SetULt, CondCode::SetULe),
            _ => (CondCode::SetULt, CondCode::SetULe, CondCode::SetUGt, CondCode::SetUGe),
        };
        for cc in [preferred, other] {
            if self.dag.does_setcc_exist(bool_vt, x, y, cc) {
                let test = self.dag.get_setcc(bool_vt, x, y, cc);
                return self.dag.get_select(vt, test, x, y);
            }
        }
        for cc in [preferred_swapped, other_swapped] {
            if self.dag.does_setcc_exist(bool_vt, x, y, cc) {
                let test = self.dag.get_setcc(bool_vt, x, y, cc);
                return self.dag.get_select(vt, test, y, x);
            }
        }
        let test = self.dag.get_setcc(bool_vt, x, y, preferred);
        self.dag.get_select(vt, test, x, y)
    }

    /// `ExpandNode` of `UINT_TO_FP`: `TargetLowering::expandUINT_TO_FP`'s
    /// non-negative case, then `ExpandLegalINT_TO_FP`'s unsigned `i32`/`i64`
    /// to `f32` and `i64` to `f64`, the algorithm of compiler-rt's
    /// `__floatundisf`. A value with its top bit set is halved with the lost
    /// bit folded into bit 0 (so the one rounding is right), converted
    /// signed and doubled; any other is converted signed as it is. Both
    /// forms are in the DAG and the select picks.
    ///
    /// DEVIATE UITOFP-F64 (exit: the vector DAG nodes land, then x86 takes
    /// `LowerUINT_TO_FP_i64`): the pin expands `i64` to `f64` through
    /// `expandUINT_TO_FP`'s bit-cast algorithm here and x86 through its SSE2
    /// vector sequence first; neither is reachable without vectors, and the
    /// compiler-rt form of `ExpandLegalINT_TO_FP` is gcc's.
    ///
    /// `TargetLowering::expandUINT_TO_FP`:
    /// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L10142>
    fn expand_uint_to_fp(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let (src, dst) = (self.dag.operand(node, 0), self.dag.value_type(node));
        let src_vt = self.dag.value_type(src);
        let EVT::Simple(src_mvt) = src_vt else { return None };
        if !matches!(self.tli.operation_action(ISD::SIntToFp, src_vt), Action::Legal | Action::Custom) {
            return None;
        }
        if self.dag.node(node).flags.0 & SDNodeFlags::NON_NEG.0 != 0 {
            return Some(self.dag.get_node(ISD::SIntToFp, dst, &[src]));
        }
        let single = matches!(src_mvt, MVT::I32 | MVT::I64) && dst == EVT::Simple(MVT::F32);
        if !single && !(src_mvt == MVT::I64 && dst == EVT::Simple(MVT::F64)) {
            return None;
        }
        let bool_vt = self.tli.types.set_cc_result_type(src_vt);
        let zero = self.dag.get_constant(0, src_vt);
        let sign_bit_test = self.dag.get_setcc(bool_vt, src, zero, CondCode::SetLt);
        let one_bit = self.dag.get_constant(1, EVT::I8);
        let shifted = self.dag.get_node(ISD::Srl, src_vt, &[src, one_bit]);
        let one = self.dag.get_constant(1, src_vt);
        let low = self.dag.get_node(ISD::And, src_vt, &[src, one]);
        let halved = self.dag.get_node(ISD::Or, src_vt, &[low, shifted]);
        let sign_cvt = self.dag.get_node(ISD::SIntToFp, dst, &[halved]);
        let slow = self.dag.get_node(ISD::FAdd, dst, &[sign_cvt, sign_cvt]);
        let fast = self.dag.get_node(ISD::SIntToFp, dst, &[src]);
        Some(self.dag.get_select(dst, sign_bit_test, slow, fast))
    }

    /// `ExpandBUILD_VECTOR`, constants only: a vector whose lanes are all
    /// constants or undefined is loaded from the constant pool, its lanes laid
    /// out in memory order (an undefined lane is zero). Any other `BUILD_VECTOR`
    /// is left to the target (`ExpandVectorBuildThroughStack` is not
    /// translated).
    fn expand_build_vector(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if !vt.is_vector() || bits > 128 {
            return None;
        }
        let lane_bits = vt.element().size_in_bits();
        let mask = u128::MAX >> (128 - lane_bits);
        let mut image = 0u128;
        for (at, lane) in self.dag.node(node).ops.clone().into_iter().enumerate() {
            if self.dag.is_undef(lane) {
                continue;
            }
            // A float lane is its bits; the operand may be wider than the
            // lane (an implicit truncation).
            let value = self.dag.constant(lane).or_else(|| self.dag.fp_constant(lane))?;
            image |= (value & mask) << (at as u32 * lane_bits);
        }
        let pointer = self.dag.config().pointer;
        let index = self.dag.get_constant_pool(pointer, image, vt, false);
        let NodeData::PoolIndex(entry, _) = self.dag.node(index).data else {
            unreachable!("a pool node has its entry")
        };
        let info = MemInfo {
            mem_vt: vt,
            align: bits / 8,
            volatile: false,
            base: MemBase::ConstantPool(entry),
            ext: None,
            space: 0,
            ordering: None,
        };
        let chain = self.dag.entry_node();
        let loaded = self.dag.get_load(vt, chain, index, info);
        // The pool is read-only memory: `isDereferenceableInvariantLoad`.
        self.dag.invariant_loads.insert(loaded.node);
        Some(loaded)
    }

    /// `SelectionDAG::UnrollVectorOp` for the lane-wise operations: each lane
    /// of the operands extracted, the scalar operation on each, a
    /// `BUILD_VECTOR` of the results.
    fn unroll_vector_op(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let vt = self.dag.value_type(node);
        let (op, flags) = (self.dag.opcode(node), self.dag.node(node).flags);
        let operands = self.dag.node(node).ops.clone();
        let lanes: Vec<SDValue> = (0..vt.lanes())
            .map(|lane| {
                let index = self.dag.get_constant(u128::from(lane), EVT::I64);
                let scalars: Vec<SDValue> = operands
                    .iter()
                    .map(|operand| self.dag.get_node(ISD::ExtractElement, vt.element(), &[*operand, index]))
                    .collect();
                self.dag.get_node_flags(op, vt.element(), &scalars, flags)
            })
            .collect();
        self.dag.get_build_vector(vt, &lanes)
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
            // `VectorLegalizer::Expand`: a lane-wise operation the target has
            // not is done a lane at a time.
            ISD::Add | ISD::Sub | ISD::Mul | ISD::And | ISD::Or | ISD::Xor if vt.is_vector() => {
                Some(vec![self.unroll_vector_op(node)])
            }
            ISD::BuildVector => self.expand_build_vector(node).map(|loaded| vec![loaded]),
            ISD::SignExtendInReg => {
                // An in-register sign extension of a boolean is a negation of
                // the masked bit; any other width is a shift pair.
                let NodeData::ValueType(inner) = self.dag.node(self.dag.operand(node, 1)).data else { return None };
                let x = self.dag.operand(node, 0);
                if inner.size_in_bits() == 1 {
                    let one = self.dag.get_constant(1, vt);
                    let bit = self.dag.get_node(ISD::And, vt, &[x, one]);
                    return Some(vec![self.dag.get_negative(bit, vt)]);
                }
                let by = self.dag.get_constant(u128::from(vt.size_in_bits() - inner.size_in_bits()), EVT::I8);
                let up = self.dag.get_node(ISD::Shl, vt, &[x, by]);
                Some(vec![self.dag.get_node(ISD::Sra, vt, &[up, by])])
            }
            ISD::AtomicLoadSub => {
                // `ExpandNode`: a subtract is an add of the negation (a
                // sign-extension of the operand from the memory
                // type is dropped first).
                let NodeData::Mem(info) = self.dag.node(node).data else { return None };
                let (chain, ptr, mut rhs) =
                    (self.dag.operand(node, 0), self.dag.operand(node, 1), self.dag.operand(node, 2));
                if self.dag.opcode(rhs) == ISD::SignExtendInReg
                    && matches!(
                        self.dag.node(self.dag.operand(rhs, 1)).data,
                        NodeData::ValueType(inner) if inner == info.mem_vt
                    )
                {
                    rhs = self.dag.operand(rhs, 0);
                }
                let zero = self.dag.get_constant(0, vt);
                let negated = self.dag.get_node(ISD::Sub, vt, &[zero, rhs]);
                let added = self.dag.get_atomic_rmw(ISD::AtomicLoadAdd, chain, ptr, negated, info);
                Some(vec![added, SDValue { node: added.node, res: 1 }])
            }
            ISD::AtomicCmpSwapWithSuccess => {
                // `ExpandNode`: the swap, and the success flag as the old value
                // equal to the expected one.
                let NodeData::Mem(info) = self.dag.node(node).data else { return None };
                let (chain, ptr) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let (expected, new) = (self.dag.operand(node, 2), self.dag.operand(node, 3));
                let swap = self.dag.get_atomic_cmp_swap(None, chain, ptr, expected, new, info);
                let flag_vt = self.dag.node(node).values[1];
                let success = self.dag.get_setcc(flag_vt, swap, expected, CondCode::SetEq);
                Some(vec![swap, success, SDValue { node: swap.node, res: 1 }])
            }
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
            ISD::UIntToFp => self.expand_uint_to_fp(node).map(|result| vec![result]),
            ISD::BrJT => Some(vec![self.expand_br_jt(node)]),
            ISD::Undef | ISD::Poison => {
                // `ExpandNode`: zero of the type.
                Some(vec![if vt.is_integer() { self.dag.get_constant(0, vt) } else { self.dag.get_constant_fp(0, vt) }])
            }
            ISD::SetCC => self.expand_setcc_code(node).map(|result| vec![result]),
            ISD::SMax | ISD::SMin | ISD::UMax | ISD::UMin => Some(vec![self.expand_min_max(node)]),
            ISD::SAddSat | ISD::UAddSat | ISD::SSubSat | ISD::USubSat => {
                Some(vec![super::expand_sat::expand_add_sub_sat(self.dag, self.tli, node)])
            }
            ISD::UAddO | ISD::USubO => Some(super::expand_overflow::expand_uaddsubo(self.dag, self.tli, node)),
            ISD::UMulO | ISD::SMulO => {
                Some(super::expand_overflow::expand_mulo(self.dag, self.tli, &mut *self.hooks, node))
            }
            ISD::SAddO | ISD::SSubO => Some(super::expand_overflow::expand_saddsubo(self.dag, self.tli, node)),
            ISD::BuildPair => {
                // `ExpandNode`: `zext lo | (anyext hi << half)` ([L4318][pin]).
                //
                // [pin]:
                // https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L4318
                let (lo, hi) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let low = self.dag.get_node(ISD::ZeroExtend, vt, &[lo]);
                let high = self.dag.get_node(ISD::AnyExtend, vt, &[hi]);
                let half = self.dag.get_constant(u128::from(vt.size_in_bits() / 2), EVT::I8);
                let high = self.dag.get_node(ISD::Shl, vt, &[high, half]);
                // The pair's kind goes on the `or` (DEVIATE PAIR1).
                let kind = self
                    .dag
                    .node(node)
                    .flags
                    .intersect(super::dag::SDNodeFlags::PAIR_FAR.union(super::dag::SDNodeFlags::PAIR_LONG));
                Some(vec![self.dag.get_node_flags(ISD::Or, vt, &[low, high], kind)])
            }
            ISD::BitCast => Some(vec![self.expand_bitcast(node)]),
            ISD::FNeg => Some(vec![self.expand_fneg(node)]),
            ISD::FAbs => Some(vec![self.expand_fabs(node)]),
            ISD::FCopySign => Some(vec![self.expand_fcopysign(node)]),
            ISD::ExtractElement => {
                // 0 is the low half, 1 the high: a truncate, of a shift for the
                // high.
                let (wide, which) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let from = self.dag.value_type(wide);
                let low = if self.dag.constant(which) == Some(0) {
                    wide
                } else {
                    let by = self.dag.get_constant(u128::from(from.size_in_bits() / 2), EVT::I8);
                    self.dag.get_node(ISD::Srl, from, &[wide, by])
                };
                Some(vec![self.dag.get_node(ISD::Truncate, vt, &[low])])
            }
            ISD::SMulFix | ISD::UMulFix | ISD::SMulFixSat | ISD::UMulFixSat => {
                super::expand_fixed::expand_fixed_point_mul(self.dag, self.tli, &mut *self.hooks, node)
                    .map(|result| vec![result])
            }
            ISD::FShl | ISD::FShr => Some(vec![super::expand_funnel::expand_funnel_shift(self.dag, self.tli, node)]),
            ISD::Rotl | ISD::Rotr => Some(vec![super::expand_funnel::expand_rotate(self.dag, self.tli, node)]),
            ISD::SShlSat | ISD::UShlSat => Some(vec![super::expand_fixed::expand_shl_sat(self.dag, self.tli, node)]),
            ISD::SCmp | ISD::UCmp => Some(vec![super::expand_fixed::expand_cmp(self.dag, self.tli, node)]),
            ISD::Abs | ISD::AbsMinPoison => Some(vec![self.expand_abs(node)]),
            ISD::Parity => Some(vec![self.expand_parity(node)]),
            ISD::CtPop => self.expand_ctpop(node).map(|result| vec![result]),
            ISD::CtLz | ISD::CtLzZeroPoison => self.expand_ctlz(node).map(|result| vec![result]),
            ISD::CtTz | ISD::CtTzZeroPoison => self.expand_cttz(node).map(|result| vec![result]),
            ISD::BSwap => self.expand_bswap(node).map(|result| vec![result]),
            ISD::BitReverse => expand_bitreverse(self.dag, node).map(|result| vec![result]),
            // `vaend` has nothing to do; `vacopy` is a load of the source list
            // and a store to the destination.
            ISD::VAEnd => Some(vec![self.dag.operand(node, 0)]),
            // `ExpandNode`: a copy from and to the stack pointer register.
            ISD::StackSave if self.hooks.stack_pointer().is_some() => {
                let (sp, _) = self.hooks.stack_pointer().expect("checked");
                let vt = self.dag.value_type(node);
                let chain = self.dag.operand(node, 0);
                let read = self.dag.get_copy_from_phys_reg_plain(chain, sp, vt);
                Some(vec![SDValue { node: read.node, res: 0 }, SDValue { node: read.node, res: 1 }])
            }
            // `ExpandDYNAMIC_STACKALLOC`: the stack pointer lowered by the size
            // and aligned past the frame's alignment, inside a call sequence
            // of no bytes.
            ISD::DynamicStackAlloc if self.hooks.stack_pointer().is_some() => {
                let (sp, stack_align) = self.hooks.stack_pointer().expect("checked");
                let vt = self.dag.value_type(node);
                let (chain, size) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let align = self.dag.constant(self.dag.operand(node, 2)).unwrap_or(0) as u64;
                let chain = self.dag.get_callseq_start(chain, 0);
                let chain = SDValue { node: chain.node, res: 0 };
                let read = self.dag.get_copy_from_phys_reg_plain(chain, sp, vt);
                let (current, chain) = (SDValue { node: read.node, res: 0 }, SDValue { node: read.node, res: 1 });
                let mut lowered = self.dag.get_node(ISD::Sub, vt, &[current, size]);
                if align > stack_align {
                    let mask = self
                        .dag
                        .get_constant(
                            (align as i128).wrapping_neg() as u128 & super::fold::mask(vt.size_in_bits()),
                            vt,
                        );
                    lowered = self.dag.get_node(ISD::And, vt, &[lowered, mask]);
                }
                let register = self.dag.get_phys_register(vt, sp);
                let chain = self.dag.get_node(ISD::CopyToReg, EVT::OTHER, &[chain, register, lowered]);
                let glue = self.dag.get_node(ISD::Undef, EVT::GLUE, &[]);
                let end = self.dag.get_callseq_end(chain, 0, 0, glue);
                Some(vec![lowered, SDValue { node: end.node, res: 0 }])
            }
            ISD::StackRestore if self.hooks.stack_pointer().is_some() => {
                let (sp, _) = self.hooks.stack_pointer().expect("checked");
                let (chain, value) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let register = self.dag.get_phys_register(self.dag.value_type(value), sp);
                Some(vec![self.dag.get_node(ISD::CopyToReg, EVT::OTHER, &[chain, register, value])])
            }
            // `expandVAArg`: load the list pointer, round it up to the
            // type's ABI alignment when that exceeds the minimum stack
            // argument alignment (`getMinStackArgumentAlignment`), step it by the
            // type's allocation size, store it back and load the value.
            ISD::VAArg => {
                let vt = self.dag.value_type(node);
                let (chain, list) = (self.dag.operand(node, 0), self.dag.operand(node, 1));
                let align = self.dag.constant(self.dag.operand(node, 3)).unwrap_or(1) as u32;
                let pointer = self.dag.config().pointer;
                let info = |mem_vt: EVT, align: u32| MemInfo {
                    mem_vt,
                    align,
                    volatile: false,
                    base: MemBase::Unknown,
                    ext: None,
                    space: 0,
                    ordering: None,
                };
                let list_load = self.dag.get_load(pointer, chain, list, info(pointer, pointer.size_in_bits() / 8));
                let mut at = SDValue { node: list_load.node, res: 0 };
                if align > self.tli.min_stack_argument_alignment() {
                    let add = self.dag.get_constant(u128::from(align - 1), pointer);
                    at = self.dag.get_node(ISD::Add, pointer, &[at, add]);
                    let mask = self.dag.get_constant((-i128::from(align)) as u128, pointer);
                    at = self.dag.get_node(ISD::And, pointer, &[at, mask]);
                }
                // `getTypeAllocSize(VT)`: the legalizer holds no layout, and a
                // power-of-two scalar's allocation size is its store size
                // (no layout here aligns one beyond its size). The operand
                // align is the whole type's, so rounding to it would step a
                // half of an expanded `double` by 8, not 4.
                let bytes = vt.size_in_bits().div_ceil(8);
                let size =
                    if vt.size_in_bits().is_power_of_two() { bytes } else { bytes.next_multiple_of(align.max(1)) };
                let step = self.dag.get_constant(u128::from(size), pointer);
                let next = self.dag.get_node(ISD::Add, pointer, &[at, step]);
                let stored = self
                    .dag
                    .get_store(
                        SDValue { node: list_load.node, res: 1 },
                        next,
                        list,
                        info(pointer, pointer.size_in_bits() / 8),
                    );
                let value = self.dag.get_load(vt, stored, at, info(vt, align.max(1)));
                Some(vec![SDValue { node: value.node, res: 0 }, SDValue { node: value.node, res: 1 }])
            }
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
                    space: 0,
                    ordering: None,
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
        // `expandABS` falls back from ABS_MIN_POISON to ABS when it is legal or
        // custom.
        if self.dag.opcode(node) == ISD::AbsMinPoison && self.tli.is_operation_legal_or_custom(ISD::Abs, vt) {
            return self.dag.get_node(ISD::Abs, vt, &[x]);
        }
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

    /// `SelectionDAGLegalize::ExpandPARITY` ([L3148][pin]): the population
    /// count's low bit when `CTPOP` is legal or promoted, else the value xored
    /// with itself shifted by half, a quarter, ... down to one bit.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/LegalizeDAG.cpp#L3148
    fn expand_parity(
        &mut self,
        node: SDValue,
    ) -> SDValue {
        let vt = self.dag.value_type(node);
        let x = self.dag.operand(node, 0);
        let counted = self.tli.types.is_type_legal(vt)
            && matches!(
                self.tli.operation_action(ISD::CtPop, vt),
                Action::Legal | Action::Promote
            );
        let result = if counted {
            self.dag.get_node(ISD::CtPop, vt, &[x])
        } else {
            let mut result = x;
            for shift in (0..vt.size_in_bits().next_power_of_two().trailing_zeros()).rev() {
                let amount = self.dag.get_constant(1 << shift, EVT::I8);
                let shifted = self.dag.get_node(ISD::Srl, vt, &[result, amount]);
                result = self.dag.get_node(ISD::Xor, vt, &[result, shifted]);
            }
            result
        };
        let one = self.dag.get_constant(1, vt);
        self.dag.get_node(ISD::And, vt, &[result, one])
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

    /// `TargetLowering::expandCTLZ`
    /// ([L10964][pin]), scalar: the plain count for a zero-poison one, the
    /// zero-poison count with the zero case selected, or the bits smeared down
    /// and counted.
    ///
    /// [pin]:
    /// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp
    fn expand_ctlz(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if !vt.is_integer() {
            return None;
        }
        let bits = vt.size_in_bits();
        let mut op = self.dag.operand(node, 0);
        if self.dag.opcode(node) == ISD::CtLzZeroPoison && self.tli.is_operation_legal_or_custom(ISD::CtLz, vt) {
            return Some(self.dag.get_node(ISD::CtLz, vt, &[op]));
        }
        if self.tli.is_operation_legal_or_custom(ISD::CtLzZeroPoison, vt) {
            let count = self.dag.get_node(ISD::CtLzZeroPoison, vt, &[op]);
            let zero = self.dag.get_constant(0, vt);
            let flag_vt = self.tli.types.set_cc_result_type(vt);
            let is_zero = self.dag.get_setcc(flag_vt, op, zero, CondCode::SetEq);
            let width = self.dag.get_constant(u128::from(bits), vt);
            return Some(self.dag.get_select(vt, is_zero, width, count));
        }
        let mut shift = 1;
        while shift < bits {
            let by = self.dag.get_constant(u128::from(shift), EVT::I8);
            let down = self.dag.get_node(ISD::Srl, vt, &[op, by]);
            op = self.dag.get_node(ISD::Or, vt, &[op, down]);
            shift *= 2;
        }
        let flipped = self.dag.get_not(op, vt);
        Some(self.dag.get_node(ISD::CtPop, vt, &[flipped]))
    }

    /// `TargetLowering::expandCTTZ`, scalar: the plain count for a zero-poison
    /// one, the zero-poison count with the zero case selected, `nlz(x & -x) ^
    /// (n - 1)` where only `ctlz` is legal and zero is not a case, else
    /// `popcount(~x & (x - 1))` (or `n - nlz` of it). DEVIATE SD532: the
    /// de Bruijn table lookup (for a target with no `ctpop` and no `ctlz`) is
    /// not translated.
    fn expand_cttz(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        if !vt.is_integer() {
            return None;
        }
        let bits = vt.size_in_bits();
        let op = self.dag.operand(node, 0);
        let poison = self.dag.opcode(node) == ISD::CtTzZeroPoison;
        if poison && self.tli.is_operation_legal_or_custom(ISD::CtTz, vt) {
            return Some(self.dag.get_node(ISD::CtTz, vt, &[op]));
        }
        if self.tli.is_operation_legal_or_custom(ISD::CtTzZeroPoison, vt) {
            let count = self.dag.get_node(ISD::CtTzZeroPoison, vt, &[op]);
            let zero = self.dag.get_constant(0, vt);
            let flag_vt = self.tli.types.set_cc_result_type(vt);
            let is_zero = self.dag.get_setcc(flag_vt, op, zero, CondCode::SetEq);
            let width = self.dag.get_constant(u128::from(bits), vt);
            return Some(self.dag.get_select(vt, is_zero, width, count));
        }
        let use_ctlz = self.tli.is_operation_legal(ISD::CtLz, vt) && !self.tli.is_operation_legal(ISD::CtPop, vt);
        if use_ctlz && poison {
            let negated = self.dag.get_negative(op, vt);
            let lowest = self.dag.get_node(ISD::And, vt, &[op, negated]);
            let count = self.dag.get_node(ISD::CtLzZeroPoison, vt, &[lowest]);
            let top = self.dag.get_constant(u128::from(bits - 1), vt);
            return Some(self.dag.get_node(ISD::Xor, vt, &[count, top]));
        }
        let flipped = self.dag.get_not(op, vt);
        let one = self.dag.get_constant(1, vt);
        let below = self.dag.get_node(ISD::Sub, vt, &[op, one]);
        let mask = self.dag.get_node(ISD::And, vt, &[flipped, below]);
        if use_ctlz {
            let count = self.dag.get_node(ISD::CtLz, vt, &[mask]);
            let width = self.dag.get_constant(u128::from(bits), vt);
            return Some(self.dag.get_node(ISD::Sub, vt, &[width, count]));
        }
        Some(self.dag.get_node(ISD::CtPop, vt, &[mask]))
    }

    /// `TargetLowering::expandBSWAP`
    /// ([source](https://github.com/llvm/llvm-project/blob/d1106deb71cc/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L11533)):
    /// `i16` is a rotate by 8, `i32` and `i64` a shift, mask and or of each
    /// byte; `i32` with a legal `ROTR` is the xor form.
    fn expand_bswap(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let op = self.dag.operand(node, 0);
        let amount = |this: &mut Self, bits: u64| this.dag.get_constant(u128::from(bits), EVT::I8);
        let mask = |this: &mut Self, bits: u64| this.dag.get_constant(u128::from(bits), vt);
        let binary =
            |this: &mut Self, opcode: ISD, lhs: SDValue, rhs: SDValue| this.dag.get_node(opcode, vt, &[lhs, rhs]);
        match vt {
            EVT::I16 => {
                let by = amount(self, 8);
                Some(binary(self, ISD::Rotl, op, by))
            }
            EVT::I32 if self.tli.is_operation_legal_or_custom(ISD::Rotr, vt) => {
                let by = amount(self, 16);
                let rotr16 = binary(self, ISD::Rotr, op, by);
                let tmp = binary(self, ISD::Xor, op, rotr16);
                let keep = mask(self, 0xFF00_FFFF);
                let tmp = binary(self, ISD::And, tmp, keep);
                let by = amount(self, 8);
                let tmp = binary(self, ISD::Srl, tmp, by);
                let by = amount(self, 8);
                let rotr8 = binary(self, ISD::Rotr, op, by);
                Some(binary(self, ISD::Xor, tmp, rotr8))
            }
            EVT::I32 => {
                let by = amount(self, 24);
                let tmp4 = binary(self, ISD::Shl, op, by);
                let low = mask(self, 0xFF00);
                let tmp3 = binary(self, ISD::And, op, low);
                let by = amount(self, 8);
                let tmp3 = binary(self, ISD::Shl, tmp3, by);
                let by = amount(self, 8);
                let tmp2 = binary(self, ISD::Srl, op, by);
                let low = mask(self, 0xFF00);
                let tmp2 = binary(self, ISD::And, tmp2, low);
                let by = amount(self, 24);
                let tmp1 = binary(self, ISD::Srl, op, by);
                let tmp4 = binary(self, ISD::Or, tmp4, tmp3);
                let tmp2 = binary(self, ISD::Or, tmp2, tmp1);
                Some(binary(self, ISD::Or, tmp4, tmp2))
            }
            EVT::I64 => {
                // Byte `k` from the bottom goes to byte `7 - k`: the outer
                // two by a shift alone, the six inner ones masked first.
                let by = amount(self, 56);
                let mut tmp8 = binary(self, ISD::Shl, op, by);
                let shifted = |this: &mut Self, byte: u32, opcode: ISD, by: u64| {
                    let keep = mask(this, 255u64 << (8 * byte));
                    let lhs = if opcode == ISD::Shl {
                        binary(this, ISD::And, op, keep)
                    } else {
                        let shift = amount(this, by);
                        binary(this, ISD::Srl, op, shift)
                    };
                    if opcode == ISD::Shl {
                        let shift = amount(this, by);
                        binary(this, ISD::Shl, lhs, shift)
                    } else {
                        binary(this, ISD::And, lhs, keep)
                    }
                };
                let tmp7 = shifted(self, 1, ISD::Shl, 40);
                let tmp6 = shifted(self, 2, ISD::Shl, 24);
                let tmp5 = shifted(self, 3, ISD::Shl, 8);
                let tmp4 = shifted(self, 3, ISD::Srl, 8);
                let tmp3 = shifted(self, 2, ISD::Srl, 24);
                let tmp2 = shifted(self, 1, ISD::Srl, 40);
                let by = amount(self, 56);
                let tmp1 = binary(self, ISD::Srl, op, by);
                tmp8 = binary(self, ISD::Or, tmp8, tmp7);
                let tmp6 = binary(self, ISD::Or, tmp6, tmp5);
                let tmp4 = binary(self, ISD::Or, tmp4, tmp3);
                let tmp2 = binary(self, ISD::Or, tmp2, tmp1);
                let tmp8 = binary(self, ISD::Or, tmp8, tmp6);
                let tmp4 = binary(self, ISD::Or, tmp4, tmp2);
                Some(binary(self, ISD::Or, tmp8, tmp4))
            }
            _ => None,
        }
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
        // A target that sets `ConstantFP` to `Legal` takes every constant
        // (wasm: `f32.const`).
        if self.tli.operation_action(ISD::ConstantFP, vt) == Action::Legal || self.tli.is_fp_imm_legal(mvt, bits) {
            return Ok(());
        }
        // `ShouldShrinkFPConstant` (no SSE2): a double that is exactly a float
        // is stored as one.
        let (pool_vt, pool_bits) = match mvt {
            // Only when the target has a native extending load from the smaller
            // type (`TLI.isLoadLegal(.., EXTLOAD)`); AArch64 expands it.
            MVT::F64
                if self.tli.should_shrink_fp_constant(mvt)
                    && self.tli.load_ext_action(super::actions::LoadExt::ExtLoad, MVT::F64, MVT::F32)
                        == Action::Legal =>
            {
                let value = f64::from_bits(bits as u64);
                let narrow = value as f32;
                if f64::from(narrow).to_bits() == bits as u64 && !value.is_nan() {
                    (EVT::Simple(MVT::F32), u128::from(narrow.to_bits()))
                } else {
                    (vt, bits)
                }
            }
            // `ExpandConstantFP` steps f80, f64, f32 and keeps the narrowest
            // that holds the value exactly.
            MVT::F80 if self.tli.should_shrink_fp_constant(mvt) => match shrunk_f80(bits) {
                Some((narrow, bits)) => (EVT::Simple(narrow), bits),
                None => (vt, bits),
            },
            _ => (vt, bits),
        };
        let pointer = self.dag.config().pointer;
        let index = self.dag.get_constant_pool(pointer, pool_bits, pool_vt, false);
        let align = match pool_vt {
            EVT::Simple(MVT::F128) => 16,
            EVT::Simple(MVT::F64) => 8,
            _ => 4,
        };
        let NodeData::PoolIndex(entry, _) = self.dag.node(index).data else {
            unreachable!("a pool node has its entry")
        };
        let info = MemInfo {
            mem_vt: pool_vt,
            align,
            volatile: false,
            base: MemBase::ConstantPool(entry),
            ext: None,
            space: 0,
            ordering: None,
        };
        let chain = self.dag.entry_node();
        // a narrower entry is read by an extending load (`ExpandConstantFP`'s
        // `getExtLoad`)
        let loaded = if pool_vt == vt {
            self.dag.get_load(vt, chain, index, info)
        } else {
            self.dag.get_ext_load(super::actions::LoadExt::ExtLoad, vt, chain, index, info)
        };
        // The pool is read-only memory (`PseudoSourceValue::isConstant`):
        // `isDereferenceableInvariantLoad`.
        self.dag.invariant_loads.insert(loaded.node);
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

/// The narrowest of `f32` and `f64` that holds a normal `f80` exactly, with its
/// bit pattern (`isValueValidForType`): the sign and 15-bit exponent above a
/// 64-bit mantissa with its leading bit set.
fn shrunk_f80(bits: u128) -> Option<(MVT, u128)> {
    let sign = (bits >> 79) as u64 & 1;
    let exponent = ((bits >> 64) & 0x7fff) as i64;
    let mantissa = bits as u64;
    if exponent == 0 || exponent == 0x7fff || mantissa >> 63 == 0 {
        return None;
    }
    let unbiased = exponent - 16383;
    let fraction = mantissa & (u64::MAX >> 1);
    if (-126..=127).contains(&unbiased) && fraction & ((1 << 40) - 1) == 0 {
        let word = (sign << 31) | ((unbiased + 127) as u64) << 23 | fraction >> 40;
        return Some((MVT::F32, u128::from(word)));
    }
    if (-1022..=1023).contains(&unbiased) && fraction & 0x7ff == 0 {
        let word = (sign << 63) | ((unbiased + 1023) as u64) << 52 | fraction >> 11;
        return Some((MVT::F64, u128::from(word)));
    }
    None
}

/// `TargetLowering::expandBITREVERSE`: a byte swap, then the nibbles, the
/// bit pairs and the bits of each byte swapped. DEVIATE SD218: a width
/// that is not a power of two (the pin's loop of single bits) is not
/// translated; none is legal on the targets here.
pub(super) fn expand_bitreverse(
    dag: &mut SelectionDAG,
    node: SDValue,
) -> Option<SDValue> {
    let vt = dag.value_type(node);
    let bits = vt.size_in_bits();
    if bits < 8 || !bits.is_power_of_two() {
        return None;
    }
    let op = dag.operand(node, 0);
    let splat = |byte: u128| (0..bits / 8).fold(0u128, |acc, i| acc | byte << (8 * i));
    let mut value = if bits > 8 { dag.get_node(ISD::BSwap, vt, &[op]) } else { op };
    for (by, mask) in [(4u128, 0x0F), (2, 0x33), (1, 0x55)] {
        let (by, mask) = (dag.get_constant(by, EVT::I8), dag.get_constant(splat(mask), vt));
        let high = dag.get_node(ISD::Srl, vt, &[value, by]);
        let high = dag.get_node(ISD::And, vt, &[high, mask]);
        let low = dag.get_node(ISD::And, vt, &[value, mask]);
        let low = dag.get_node(ISD::Shl, vt, &[low, by]);
        value = dag.get_node(ISD::Or, vt, &[high, low]);
    }
    Some(value)
}
