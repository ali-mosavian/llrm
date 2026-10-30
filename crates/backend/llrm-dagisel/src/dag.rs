//! What the interpreter needs of a SelectionDAG and of the target's selector.
//!
//! The DAG is the plan's arena: nodes are `u32` indices, a value is a node and
//! a result number. [`Dag`] is the part of `SelectionDAG` and `SDNode` that
//! `SelectCodeCommon` calls; [`Hooks`] is the part of `X86DAGToDAGISel` and
//! `SelectionDAGISel` it calls back into.

use crate::matcher::{Opcode, Vt};

/// `SDValue`: result `res` of node `node`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SDValue {
    pub node: u32,
    pub res: u8,
}

impl SDValue {
    pub const fn new(
        node: u32,
        res: u8,
    ) -> SDValue {
        SDValue { node, res }
    }
}

/// `ISD::LoadExtType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadExt {
    NonExt,
    Ext,
    SExt,
    ZExt,
}

/// What a floating-point constant node is, for the `fpimm*` predicates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FpClass {
    PosZero,
    NegZero,
    One,
    MinusOne,
    Other,
}

/// An opaque `MachineMemOperand`.
pub type MemRef = u32;

/// `SmallPtrSet<const SDNode *> &Visited` of `hasPredecessorHelper`: the nodes
/// seen, as one bit per node number so a membership test costs one load (a
/// list searched per operand made a walk of n nodes cost n squared).
#[derive(Clone, Debug, Default)]
pub struct Visited {
    bits: Vec<u64>,
    count: usize,
}

impl Visited {
    pub fn new() -> Self {
        Self::default()
    }

    /// A set holding `node`.
    pub fn of(node: u32) -> Self {
        let mut set = Self::new();
        set.insert(node);
        set
    }

    pub fn contains(
        &self,
        node: u32,
    ) -> bool {
        self.bits.get(node as usize / 64).is_some_and(|word| word >> (node % 64) & 1 != 0)
    }

    /// Adds `node`; false where it was there (`Visited.insert(N).second`).
    pub fn insert(
        &mut self,
        node: u32,
    ) -> bool {
        let at = node as usize / 64;
        if at >= self.bits.len() {
            self.bits.resize(at + 1, 0);
        }
        let mask = 1_u64 << (node % 64);
        let new = self.bits[at] & mask == 0;
        self.bits[at] |= mask;
        self.count += usize::from(new);
        new
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn clear(&mut self) {
        self.bits.clear();
        self.count = 0;
    }
}

pub trait Dag {
    fn opcode(
        &self,
        node: u32,
    ) -> Opcode;
    /// `SDNode::isMachineOpcode`.
    fn is_machine(
        &self,
        node: u32,
    ) -> bool;
    fn num_operands(
        &self,
        node: u32,
    ) -> usize;
    fn operand(
        &self,
        node: u32,
        index: usize,
    ) -> SDValue;
    fn num_values(
        &self,
        node: u32,
    ) -> usize;
    fn value_type(
        &self,
        value: SDValue,
    ) -> Vt;
    /// `SDNode::hasNUsesOfValue(1, res)`.
    fn has_one_use(
        &self,
        value: SDValue,
    ) -> bool;
    /// Uses of `node` through a value that is not a chain, stopping once it
    /// passes `limit`.
    fn count_non_chain_uses(
        &self,
        node: u32,
        limit: usize,
    ) -> usize;
    fn use_empty(
        &self,
        node: u32,
    ) -> bool;
    /// `SDNode::hasOneUse`.
    fn node_has_one_use(
        &self,
        node: u32,
    ) -> bool;
    /// The uses of `value`: the user node and the operand number.
    fn users(
        &self,
        value: SDValue,
    ) -> Vec<(u32, usize)>;
    /// `SDNode::getGluedUser`.
    fn glued_user(
        &self,
        node: u32,
    ) -> Option<u32>;
    /// `SDNode::isOnlyUserOf`: `def` has no user but `node`.
    fn is_only_user_of(
        &self,
        node: u32,
        def: u32,
    ) -> bool;
    /// `SDNode::hasNUsesOfValue(n, res)`.
    fn has_n_uses_of_value(
        &self,
        value: SDValue,
        n: usize,
    ) -> bool;
    /// `ConstantSDNode::getAPIntValue().trySExtValue()`.
    fn constant(
        &self,
        value: SDValue,
    ) -> Option<i64>;
    /// `CondCodeSDNode::get()`.
    fn cond_code(
        &self,
        value: SDValue,
    ) -> Option<u8>;
    /// `VTSDNode::getVT()`.
    /// `RegisterSDNode::getReg`: the register a `Register` node names.
    fn register_of(
        &self,
        node: u32,
    ) -> Option<u16>;
    fn value_type_node(
        &self,
        value: SDValue,
    ) -> Option<Vt>;
    fn is_undef(
        &self,
        value: SDValue,
    ) -> bool;
    /// `Some` for a `MemSDNode`.
    fn mem_operands(
        &self,
        node: u32,
    ) -> Option<Vec<MemRef>>;
    /// Whether `node` stores an outgoing call argument
    /// (`MachinePointerInfo::getStack` of `LowerCall`).
    fn is_call_argument_store(
        &self,
        node: u32,
    ) -> bool;
    /// The extension of a `LoadSDNode` or a load `AtomicSDNode`; `None` for
    /// other nodes.
    fn load_ext_type(
        &self,
        node: u32,
    ) -> Option<LoadExt>;
    /// `LSBaseSDNode::getAddressingMode() != UNINDEXED`.
    fn is_indexed(
        &self,
        node: u32,
    ) -> bool;
    /// `getAddressingMode()` is `POST_INC`/`POST_DEC`: the update comes after the access.
    fn is_post_indexed(
        &self,
        _node: u32,
    ) -> bool {
        false
    }
    fn is_truncating_store(
        &self,
        node: u32,
    ) -> bool;
    /// `MemSDNode::getMemoryVT`.
    fn memory_vt(
        &self,
        node: u32,
    ) -> Vt;
    /// `MemSDNode::getMemoryVT().getStoreSize()`, in bytes.
    fn memory_store_size(
        &self,
        node: u32,
    ) -> u64;
    /// `MemSDNode::getAlign`, in bytes.
    fn alignment(
        &self,
        node: u32,
    ) -> u64;
    /// `MemSDNode::isSimple`: neither volatile nor atomic.
    fn is_simple(
        &self,
        node: u32,
    ) -> bool;
    /// The class of a `ConstantFPSDNode`; `None` for any other node.
    fn fp_class(
        &self,
        value: SDValue,
    ) -> Option<FpClass>;
    /// The bit pattern of a `ConstantFPSDNode` (`getValueAPF().bitcastToAPInt()`); `None` for any
    /// other node.
    fn fp_bits(
        &self,
        _value: SDValue,
    ) -> Option<u128> {
        None
    }
    /// `SDNodeFlags::hasDisjoint`.
    fn has_disjoint_flag(
        &self,
        node: u32,
    ) -> bool;
    /// `SelectionDAG::MaskedValueIsZero`: every bit of `mask` is known zero in
    /// `value`.
    fn masked_value_is_zero(
        &self,
        value: SDValue,
        mask: u64,
    ) -> bool;
    /// `SelectionDAG::haveNoCommonBitsSet`.
    fn have_no_common_bits_set(
        &self,
        a: SDValue,
        b: SDValue,
    ) -> bool;
    fn mem_ref_is_load(
        &self,
        mem: MemRef,
    ) -> bool;
    fn mem_ref_is_store(
        &self,
        mem: MemRef,
    ) -> bool;
    fn set_node_mem_refs(
        &mut self,
        node: u32,
        mems: &[MemRef],
    );

    fn entry_node(&mut self) -> SDValue;
    fn target_constant(
        &mut self,
        value: i64,
        vt: Vt,
    ) -> SDValue;
    /// `ISD::Constant` or `ConstantFP` to its `Target` form; anything else is
    /// returned as is.
    fn convert_to_target(
        &mut self,
        value: SDValue,
    ) -> SDValue;
    fn register(
        &mut self,
        register: u16,
        vt: Vt,
    ) -> SDValue;
    /// `getCopyToReg`; the node's results are the chain and, last, the glue.
    fn copy_to_reg(
        &mut self,
        chain: SDValue,
        register: u16,
        value: SDValue,
        glue: Option<SDValue>,
    ) -> u32;
    /// `getCopyFromReg` with glue: the value is result 0, the chain 1, the glue
    /// 2.
    fn copy_from_reg(
        &mut self,
        chain: SDValue,
        register: u16,
        vt: Vt,
        glue: SDValue,
    ) -> u32;
    /// `getCopyFromReg` of a `Register` node already in the DAG: the value is
    /// result 0, the chain 1, and with `glue` the glue 2.
    fn copy_from_reg_node(
        &mut self,
        chain: SDValue,
        register: SDValue,
        vt: Vt,
        glue: Option<SDValue>,
    ) -> u32;
    fn token_factor(
        &mut self,
        chains: &[SDValue],
    ) -> SDValue;
    fn machine_node(
        &mut self,
        opcode: u16,
        vts: &[Vt],
        operands: &[SDValue],
    ) -> u32;
    /// `MorphNodeTo`: the node now, and the nodes it deleted when an equal one
    /// existed.
    fn morph_node_to(
        &mut self,
        node: u32,
        opcode: u16,
        vts: &[Vt],
        operands: &[SDValue],
    ) -> (u32, Vec<u32>);
    /// `UpdateNodeOperands`: `node` now has `operands`; the node it is, or the
    /// equal one that existed.
    fn update_node_operands(
        &mut self,
        node: u32,
        operands: &[SDValue],
    ) -> u32;
    fn replace_uses(
        &mut self,
        from: SDValue,
        to: SDValue,
    );
    /// `ReplaceAllUsesWith` for all results, then `RemoveDeadNode`.
    fn replace_node(
        &mut self,
        from: u32,
        to: u32,
    );
    fn remove_dead_node(
        &mut self,
        node: u32,
    );
    fn remove_dead_nodes(
        &mut self,
        nodes: &[u32],
    );
    /// `SDNode::hasPredecessorHelper`.
    fn has_predecessor_helper(
        &self,
        node: u32,
        visited: &mut Visited,
        worklist: &mut Vec<u32>,
        max: usize,
    ) -> bool;
}

/// The target's side of selection (`X86DAGToDAGISel` and `SelectionDAGISel`).
pub trait Hooks<D: Dag> {
    /// `TLI->getPointerTy(DL)`.
    fn pointer_type(&self) -> Vt;
    /// The generated `CheckPatternPredicate`:
    /// `generated::<cpu>::pattern_predicate(self, id)`.
    fn check_pattern_predicate(
        &self,
        predicate: u16,
    ) -> bool;
    /// The generated `CheckNodePredicate`:
    /// `generated::<cpu>::node_predicate(dag, self, value, id)`.
    fn check_node_predicate(
        &self,
        dag: &D,
        value: SDValue,
        predicate: u16,
    ) -> bool;
    /// The generated `CheckComplexPattern`; pushes the results.
    fn check_complex_pattern(
        &mut self,
        dag: &mut D,
        root: u32,
        parent: Option<u32>,
        value: SDValue,
        pattern: u8,
        results: &mut Vec<(SDValue, Option<u32>)>,
    ) -> bool;
    /// `AArch64_AM::getFP32Imm` / `getFP64Imm` of a float constant's bits: the 8-bit code of an
    /// `fmov` immediate, `None` if there is none. A target without the form never has one.
    fn fp_imm_code(
        &self,
        _bits: u128,
        _double: bool,
    ) -> Option<u8> {
        None
    }
    /// The generated `RunSDNodeXForm`.
    fn run_xform(
        &mut self,
        dag: &mut D,
        value: SDValue,
        xform: u8,
    ) -> SDValue;
    fn is_profitable_to_fold(
        &self,
        dag: &D,
        value: SDValue,
        user: u32,
        root: u32,
    ) -> bool;
    fn is_legal_to_fold(
        &self,
        dag: &D,
        value: SDValue,
        user: u32,
        root: u32,
    ) -> bool;
    /// `CheckAndMask(LHS, RHS, DesiredMask)`: `rhs` is the constant operand's
    /// value.
    fn check_and_mask(
        &self,
        dag: &D,
        lhs: SDValue,
        rhs: i64,
        desired: i64,
    ) -> bool;
    fn check_or_mask(
        &self,
        dag: &D,
        lhs: SDValue,
        rhs: i64,
        desired: i64,
    ) -> bool;
    /// `Subtarget->hasSSEUnalignedMem()`.
    fn has_sse_unaligned_mem(&self) -> bool;
    /// `shouldAvoidImmediateInstFormsForSize(N)`.
    fn should_avoid_immediate_inst_forms_for_size(
        &self,
        dag: &D,
        node: u32,
    ) -> bool;
    /// `isUnneededShiftMask(N, width)`.
    fn is_unneeded_shift_mask(
        &self,
        dag: &D,
        node: u32,
        width: u32,
    ) -> bool;
    /// `isSExtAbsoluteSymbolRef(width, N)`.
    fn is_sext_absolute_symbol_ref(
        &self,
        dag: &D,
        width: u32,
        node: u32,
    ) -> bool;
    /// `hasNoCarryFlagUses(Flags)`.
    fn has_no_carry_flag_uses(
        &self,
        dag: &D,
        flags: SDValue,
    ) -> bool;
    /// `isDef32(N)`.
    fn is_def32(
        &self,
        dag: &D,
        node: u32,
    ) -> bool;
    /// `checkTCRetEnoughRegs(N)`.
    fn check_tc_ret_enough_regs(
        &self,
        dag: &D,
        node: u32,
    ) -> bool;
    /// `MCInstrDesc::mayLoad` and `mayStore` of a machine opcode.
    fn may_load_store(
        &self,
        opcode: u16,
    ) -> (bool, bool);
}

/// The pattern predicates that are decided when a function is selected, as
/// `features.toml` names them; the generated `pattern_predicate` calls these.
pub trait PatternContext {
    /// `shouldOptForSize(MF)`.
    fn should_opt_for_size(&self) -> bool;
    /// `MF->getFunction().hasMinSize()`.
    fn has_min_size(&self) -> bool;
    /// `Subtarget->getFrameLowering()->hasFP(*MF)`.
    fn has_frame_pointer(&self) -> bool;
    /// `Subtarget->isLegalToCallImmediateAddr()`.
    fn is_legal_to_call_immediate_addr(&self) -> bool;
}
