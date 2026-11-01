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
use super::dag::{MemBase, MemInfo, NodeData, SDValue, SelectionDAG};
use super::isd::{CondCode, ISD};
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

    /// `TargetLowering::makeLibCall`: a call to the routine `name` with
    /// `args`, whose result is a `ret`; `None` where the target cannot make
    /// one.
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
            ISD::Br | ISD::BrCond | ISD::BrCC | ISD::BrJT => EVT::OTHER,
            ISD::SIntToFp | ISD::UIntToFp | ISD::LRint => self.dag.value_type(self.dag.operand(node, 0)),
            ISD::SignExtendInReg => match self.dag.node(self.dag.operand(node, 1)).data {
                NodeData::ValueType(inner) => inner,
                _ => self.dag.value_type(node),
            },
            ISD::SetCC | ISD::Store => {
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
        if let Some(result) = self.expand_ext_load(node) {
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
        // expanded.
        if self.dag.opcode(node) == ISD::ExtractElement && action == Action::Legal {
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
        let (signed, arity) = match op {
            ISD::SDiv | ISD::SRem => (true, 2),
            ISD::UDiv | ISD::URem | ISD::Mul => (false, 2),
            // `ExpandFPLibCall`: every operand, none signed.
            ISD::FRem => (false, 2),
            ISD::FSin | ISD::FCos | ISD::FAtan | ISD::FLog | ISD::FLog2 | ISD::FExp | ISD::FExp2 | ISD::LRint => {
                (false, 1)
            }
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
        let entry = pointer.size_in_bits() / 8;
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
                super::expand_fixed::expand_fixed_point_mul(self.dag, self.tli, node).map(|result| vec![result])
            }
            ISD::FShl | ISD::FShr => Some(vec![super::expand_funnel::expand_funnel_shift(self.dag, self.tli, node)]),
            ISD::SShlSat | ISD::UShlSat => Some(vec![super::expand_fixed::expand_shl_sat(self.dag, self.tli, node)]),
            ISD::Abs | ISD::AbsMinPoison => Some(vec![self.expand_abs(node)]),
            ISD::CtPop => self.expand_ctpop(node).map(|result| vec![result]),
            ISD::BSwap => self.expand_bswap(node).map(|result| vec![result]),
            ISD::BitReverse => self.expand_bitreverse(node).map(|result| vec![result]),
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

    /// `TargetLowering::expandBITREVERSE`: a byte swap, then the nibbles, the
    /// bit pairs and the bits of each byte swapped. DEVIATE SD218: a width
    /// that is not a power of two (the pin's loop of single bits) is not
    /// translated; none is legal on the targets here.
    fn expand_bitreverse(
        &mut self,
        node: SDValue,
    ) -> Option<SDValue> {
        let vt = self.dag.value_type(node);
        let bits = vt.size_in_bits();
        if bits < 8 || !bits.is_power_of_two() {
            return None;
        }
        let op = self.dag.operand(node, 0);
        let splat = |byte: u128| (0..bits / 8).fold(0u128, |acc, i| acc | byte << (8 * i));
        let mut value = if bits > 8 { self.dag.get_node(ISD::BSwap, vt, &[op]) } else { op };
        for (by, mask) in [(4u128, 0x0F), (2, 0x33), (1, 0x55)] {
            let (by, mask) = (self.dag.get_constant(by, EVT::I8), self.dag.get_constant(splat(mask), vt));
            let high = self.dag.get_node(ISD::Srl, vt, &[value, by]);
            let high = self.dag.get_node(ISD::And, vt, &[high, mask]);
            let low = self.dag.get_node(ISD::And, vt, &[value, mask]);
            let low = self.dag.get_node(ISD::Shl, vt, &[low, by]);
            value = self.dag.get_node(ISD::Or, vt, &[high, low]);
        }
        Some(value)
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
        // A target that sets `ConstantFP` to `Legal` takes every constant
        // (wasm: `f32.const`).
        if self.tli.operation_action(ISD::ConstantFP, vt) == Action::Legal || self.tli.is_fp_imm_legal(mvt, bits) {
            return Ok(());
        }
        // `ShouldShrinkFPConstant` (no SSE2): a double that is exactly a float
        // is stored as one.
        let (pool_vt, pool_bits) = match mvt {
            MVT::F64 if self.tli.should_shrink_fp_constant(mvt) => {
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
