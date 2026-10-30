//! [`SelectionDAG`] as the matcher interpreter's DAG (`llrm_dagisel::dag::Dag`
//! and `AddrDag`).
//!
//! # How it works
//! `llrm-dagisel` numbers opcodes and value types as the pin does;
//! [`pin_opcode`] and [`pin_vt`] translate ours. Nodes the selector makes are
//! `ISD::Machine(pin opcode)`; a target constant or register is the node our
//! DAG already has. The arena never frees a node, so [`DagisAdapter`] keeps the
//! set of nodes that selection removed and every use query skips them.
//!
//! Numbered deviations (reason / owner / exit / review):
//! - **SD300** `ConstantFP` to target form, memory operands on machine nodes
//!   (kept in [`DagisAdapter::mem_refs`]) and extending loads are not in our
//!   DAG yet; the adapter answers conservatively. (`have_no_common_bits_set`
//!   reads the combiner's known bits.) Owner sdag-c. Review 2026-12-10.
//!
//! # Used by
//! The shadow harness (sdag-d) through `llrm_dagisel::interp::Selector`.

use std::collections::{BTreeMap, BTreeSet};

use llrm_dagisel::dag::{Dag, FpClass, LoadExt, MemRef, SDValue as PinValue};
use llrm_dagisel::generated::ids::{isd as pin, mvt};
use llrm_dagisel::matcher::Vt;
use llrm_dagisel::x86_addr::{AddrDag, Symbol, SymbolKind, Wrapped};

use super::SelectionDAG;
use super::dag::{NodeData, SDNodeFlags, SDValue};
use super::isd::ISD;
use super::mvt::{EVT, MVT};

/// Our opcode in the pin's numbering; `u16::MAX` for one no pattern names.
pub fn pin_opcode(opcode: ISD) -> u16 {
    use ISD::*;
    match opcode {
        EntryToken => pin::EntryToken,
        TokenFactor => pin::TokenFactor,
        Constant => pin::Constant,
        TargetConstant => pin::TargetConstant,
        ConstantFP => pin::ConstantFP,
        TargetConstantFP => pin::TargetConstantFP,
        Undef => pin::UNDEF,
        Poison => pin::POISON,
        Register => pin::Register,
        GlobalAddress => pin::GlobalAddress,
        TargetGlobalAddress => pin::TargetGlobalAddress,
        ExternalSymbol => pin::ExternalSymbol,
        FrameIndex => pin::FrameIndex,
        TargetFrameIndex => pin::TargetFrameIndex,
        ConstantPool => pin::ConstantPool,
        TargetConstantPool => pin::TargetConstantPool,
        TargetJumpTable => pin::TargetJumpTable,
        BasicBlock => pin::BasicBlock,
        ValueType => pin::VALUETYPE,
        CondCodeNode => pin::CONDCODE,
        MergeValues => pin::MERGE_VALUES,
        CopyToReg => pin::CopyToReg,
        CopyFromReg => pin::CopyFromReg,
        Add => pin::ADD,
        Sub => pin::SUB,
        Mul => pin::MUL,
        SDiv => pin::SDIV,
        UDiv => pin::UDIV,
        SRem => pin::SREM,
        URem => pin::UREM,
        MulHS => pin::MULHS,
        MulHU => pin::MULHU,
        SMulLoHi => pin::SMUL_LOHI,
        UMulLoHi => pin::UMUL_LOHI,
        SDivRem => pin::SDIVREM,
        UDivRem => pin::UDIVREM,
        AddC => pin::ADDC,
        AddE => pin::ADDE,
        SubC => pin::SUBC,
        SubE => pin::SUBE,
        UAddO => pin::UADDO,
        USubO => pin::USUBO,
        SAddO => pin::SADDO,
        SSubO => pin::SSUBO,
        UMulO => pin::UMULO,
        SMulO => pin::SMULO,
        UAddOCarry => pin::UADDO_CARRY,
        USubOCarry => pin::USUBO_CARRY,
        SetCCCarry => u16::MAX,
        SAddSat => pin::SADDSAT,
        UAddSat => pin::UADDSAT,
        SSubSat => pin::SSUBSAT,
        USubSat => pin::USUBSAT,
        SMin => pin::SMIN,
        SMax => pin::SMAX,
        UMin => pin::UMIN,
        UMax => pin::UMAX,
        Abs => pin::ABS,
        PtrAdd => pin::PTRADD,
        And => pin::AND,
        Or => pin::OR,
        Xor => pin::XOR,
        Shl => pin::SHL,
        Srl => pin::SRL,
        Sra => pin::SRA,
        Rotl => pin::ROTL,
        Rotr => pin::ROTR,
        // The double-width shifts are lowered before selection; no pattern
        // names them.
        ShlParts | SraParts | SrlParts => u16::MAX,
        // dropped before matching (`remove_assertions`)
        AssertZext | AssertSext => u16::MAX,
        FShl => pin::FSHL,
        FShr => pin::FSHR,
        BSwap => pin::BSWAP,
        BitReverse => pin::BITREVERSE,
        CtPop => pin::CTPOP,
        CtLz => pin::CTLZ,
        CtLzZeroPoison => pin::CTLZ_ZERO_POISON,
        CtTz => pin::CTTZ,
        CtTzZeroPoison => pin::CTTZ_ZERO_POISON,
        Parity => pin::PARITY,
        CtLs => pin::CTLS,
        Truncate => pin::TRUNCATE,
        ZeroExtend => pin::ZERO_EXTEND,
        SignExtend => pin::SIGN_EXTEND,
        AnyExtend => pin::ANY_EXTEND,
        SignExtendInReg => pin::SIGN_EXTEND_INREG,
        BitCast => pin::BITCAST,
        BuildPair => pin::BUILD_PAIR,
        ExtractElement => pin::EXTRACT_VECTOR_ELT,
        SIntToFp => pin::SINT_TO_FP,
        UIntToFp => pin::UINT_TO_FP,
        FpToSInt => pin::FP_TO_SINT,
        FpToUInt => pin::FP_TO_UINT,
        FpExtend => pin::FP_EXTEND,
        FpRound => pin::FP_ROUND,
        FAdd => pin::FADD,
        FSub => pin::FSUB,
        FMul => pin::FMUL,
        FDiv => pin::FDIV,
        FRem => pin::FREM,
        FNeg => pin::FNEG,
        FAbs => pin::FABS,
        FSqrt => pin::FSQRT,
        Freeze => pin::FREEZE,
        SCmp => pin::SCMP,
        UCmp => pin::UCMP,
        SMulFix => pin::SMULFIX,
        // Expanded before selection, so no pattern names them.
        UMulFix | SMulFixSat | UMulFixSat | SShlSat | UShlSat => u16::MAX,
        SDivFix => pin::SDIVFIX,
        StackSave => pin::STACKSAVE,
        StackRestore => pin::STACKRESTORE,
        AbsMinPoison => pin::ABS_MIN_POISON,
        FSin => pin::FSIN,
        FCos => pin::FCOS,
        FAtan => pin::FATAN,
        FLog => pin::FLOG,
        FLog2 => pin::FLOG2,
        FExp => pin::FEXP,
        FExp2 => pin::FEXP2,
        FRint => pin::FRINT,
        LRint => pin::LRINT,
        FCopySign => pin::FCOPYSIGN,
        SetCC => pin::SETCC,
        Select => pin::SELECT,
        SelectCC => pin::SELECT_CC,
        BrCC => pin::BR_CC,
        BrCond => pin::BRCOND,
        Br => pin::BR,
        BrJT => pin::BR_JT,
        BrInd => pin::BRIND,
        Load | LoadPre | LoadPost => pin::LOAD,
        Store | StorePre | StorePost => pin::STORE,
        AtomicSwap => pin::ATOMIC_SWAP,
        CallSeqStart => pin::CALLSEQ_START,
        CallSeqEnd => pin::CALLSEQ_END,
        DynamicStackAlloc => pin::DYNAMIC_STACKALLOC,
        Trap => pin::TRAP,
        TargetExternalSymbolNode => pin::TargetExternalSymbol,
        JumpTable => pin::JumpTable,
        RegisterMask => pin::RegisterMask,
        LifetimeStart => pin::LIFETIME_START,
        LifetimeEnd => pin::LIFETIME_END,
        VAStart => pin::VASTART,
        VAEnd => pin::VAEND,
        VACopy => pin::VACOPY,
        SrcValue => pin::SRCVALUE,
        // sdag-a's codes are private and named (`x86isd::name`), the pin's
        // number comes from the name; sdag-b's lowering writes the pin's
        // numbers, which `name` does not know.
        Target(code) => match super::x86isd::private_name(code).and_then(|name| name.strip_prefix("X86ISD::")) {
            Some(name) => llrm_dagisel::generated::ids::x86isd_id(name).unwrap_or(u16::MAX),
            None => code,
        },
        // A machine node reads as the pin's `~opcode`.
        Machine(code) => !(code as u16),
    }
}

/// Our value type in the pin's numbering; a type with none is `Vt(u16::MAX)`.
pub fn pin_vt(vt: EVT) -> Vt {
    match vt {
        EVT::Simple(MVT::I1) => mvt::i1,
        EVT::Simple(MVT::I8) => mvt::i8,
        EVT::Simple(MVT::I16) => mvt::i16,
        EVT::Simple(MVT::I32) => mvt::i32,
        EVT::Simple(MVT::I64) => mvt::i64,
        EVT::Simple(MVT::F32) => mvt::f32,
        EVT::Simple(MVT::F64) => mvt::f64,
        EVT::Simple(MVT::F80) => mvt::f80,
        EVT::Simple(MVT::Other) => mvt::Other,
        EVT::Simple(MVT::Glue) => mvt::Glue,
        _ => Vt(u16::MAX),
    }
}

/// The inverse of [`pin_vt`] for the types a pattern can ask for.
fn our_vt(vt: Vt) -> EVT {
    [
        (mvt::i1, EVT::I1),
        (mvt::i8, EVT::I8),
        (mvt::i16, EVT::I16),
        (mvt::i32, EVT::I32),
        (mvt::i64, EVT::I64),
        (mvt::f32, EVT::Simple(MVT::F32)),
        (mvt::f64, EVT::Simple(MVT::F64)),
        (mvt::f80, EVT::Simple(MVT::F80)),
        (mvt::Other, EVT::OTHER),
        (mvt::Glue, EVT::GLUE),
    ]
    .into_iter()
    .find(|(p, _)| *p == vt)
    .map(|(_, e)| e)
    .unwrap_or_else(|| panic!("pin value type {vt:?} has no counterpart"))
}

/// The id the flags register has: iced numbers no EFLAGS, so it takes the last
/// raw id.
pub const EFLAGS_ID: llrm_lir::registers::RegId = llrm_lir::registers::RegId::from_raw(255);

/// The register of the pin's register number `register` that the builder names
/// as a physical register (`NodeData::PhysReg`); `None` for `$noreg` (0) and
/// the ones with no id.
pub fn phys_reg(register: u16) -> Option<llrm_lir::registers::RegId> {
    use llrm_dagisel::generated::ids::x86;
    use llrm_lir::registers::RegId;
    // The pin's register numbers, in its X86 enum order (alphabetical: AH 1,
    // AL 2, AX 3, ... EDX 27, ES 31); the ids
    // module only names the ones some pattern mentions.
    const AL: u16 = 2;
    Some(match register {
        AL => RegId::AL,
        x86::AH => RegId::AH,
        x86::AX => RegId::AX,
        x86::CL => RegId::CL,
        x86::CX => RegId::CX,
        x86::DX => RegId::DX,
        x86::EAX => RegId::EAX,
        x86::EDX => RegId::EDX,
        x86::RAX => RegId::RAX,
        x86::RIP => RegId::RIP,
        x86::RDX => RegId::RDX,
        x86::CS => RegId::CS,
        x86::DS => RegId::DS,
        x86::ES => RegId::ES,
        x86::FS => RegId::FS,
        x86::GS => RegId::GS,
        x86::SS => RegId::SS,
        x86::EFLAGS => EFLAGS_ID,
        _ => return None,
    })
}

/// The pin's register number of the register the builder names `id`
/// (`phys_reg` inverted); `None` where it has none.
pub fn pin_register(id: llrm_lir::registers::RegId) -> Option<u16> {
    (0..=u16::MAX.min(200)).find(|&number| phys_reg(number) == Some(id))
}

/// A target's physical registers in the pin's numbering: the ids the builder
/// names them by, and back.
pub trait PinRegisters {
    fn phys_reg(
        &self,
        register: u16,
    ) -> Option<llrm_lir::registers::RegId>;
    fn pin_register(
        &self,
        id: llrm_lir::registers::RegId,
    ) -> Option<u16>;
}

/// x86's registers (the default of an adapter).
pub struct X86Registers;

impl PinRegisters for X86Registers {
    fn phys_reg(
        &self,
        register: u16,
    ) -> Option<llrm_lir::registers::RegId> {
        phys_reg(register)
    }
    fn pin_register(
        &self,
        id: llrm_lir::registers::RegId,
    ) -> Option<u16> {
        pin_register(id)
    }
}

impl DagisAdapter<'_> {
    /// The `Register` node for the pin's register number: physical when the
    /// builder names it so.
    fn register_node(
        &mut self,
        vt: EVT,
        register: u16,
    ) -> SDValue {
        match self.registers.phys_reg(register) {
            Some(id) => self.dag.get_phys_register(vt, id),
            None => self.dag.get_register(vt, u32::from(register)),
        }
    }
}

fn ours(v: PinValue) -> SDValue {
    SDValue { node: v.node, res: v.res }
}

fn theirs(v: SDValue) -> PinValue {
    PinValue::new(v.node, v.res)
}

/// A [`SelectionDAG`] under selection.
pub struct DagisAdapter<'a> {
    pub dag: &'a mut SelectionDAG,
    /// Nodes selection removed; the arena keeps them, no query sees them.
    dead: BTreeSet<u32>,
    /// The memory operands each machine node got (`setNodeMemRefs`), as
    /// load/store node ids.
    pub mem_refs: BTreeMap<u32, Vec<u32>>,
    /// `SDNode::getNodeId`: the topological position of each node at the start
    /// of selection (operands before their users), -1 for a node selection
    /// made, and `-(id + 1)` for an unselected successor of a selected node
    /// (`InvalidateNodeId`): topological pruning looks at positive ids only.
    node_id: Vec<i32>,
    /// The target's registers in the pin's numbering.
    registers: &'a dyn PinRegisters,
}

impl<'a> DagisAdapter<'a> {
    pub fn new(dag: &'a mut SelectionDAG) -> DagisAdapter<'a> {
        Self::with_registers(dag, &X86Registers)
    }

    /// An adapter of another target's registers.
    pub fn with_registers(
        dag: &'a mut SelectionDAG,
        registers: &'a dyn PinRegisters,
    ) -> DagisAdapter<'a> {
        // A node nothing reaches from the root is no user of what it read
        // (`hasOneUse`): the combiner deleted it, or it is a chain node it
        // replaced (a `brcond` the lowering turned into `X86ISD::BRCOND`).
        let mut reached = vec![false; dag.nodes().len()];
        let mut work = vec![dag.get_root().node];
        while let Some(node) = work.pop() {
            if std::mem::replace(&mut reached[node as usize], true) {
                continue;
            }
            work.extend(dag.nodes()[node as usize].ops.iter().map(|op| op.node));
        }
        let dead =
            (0..dag.nodes().len() as u32).filter(|&node| dag.is_deleted(node) || !reached[node as usize]).collect();
        let mut node_id = vec![-1; dag.nodes().len()];
        for (position, node) in super::dag_select::users_first(dag).into_iter().rev().enumerate() {
            node_id[node as usize] = position as i32;
        }
        DagisAdapter { dag, dead, mem_refs: BTreeMap::new(), node_id, registers }
    }

    /// `Node->setNodeId(-1)`.
    pub(super) fn mark_selected(
        &mut self,
        node: u32,
    ) {
        if let Some(id) = self.node_id.get_mut(node as usize) {
            *id = -1;
        }
    }

    pub(super) fn id_of(
        &self,
        node: u32,
    ) -> i32 {
        self.node_id.get(node as usize).copied().unwrap_or(-1)
    }

    /// `SelectionDAGISel::EnforceNodeIdInvariant`: the unselected users of a
    /// node a replacement made, and theirs, are no longer after what they read
    /// in the numbering, so pruning by id must skip them.
    fn enforce_node_id_invariant(
        &mut self,
        node: u32,
    ) {
        let mut work = vec![node];
        while let Some(n) = work.pop() {
            for user in Self::live_user_nodes(self, n) {
                let id = self.id_of(user);
                if id > 0 {
                    self.node_id[user as usize] = -(id + 1);
                    work.push(user);
                }
            }
        }
    }

    pub fn is_dead(
        &self,
        node: u32,
    ) -> bool {
        self.dead.contains(&node)
    }

    /// Whether a node selection has not removed uses `node`'s results
    /// (`!Node->use_empty()`).
    pub fn has_live_users(
        &self,
        node: u32,
    ) -> bool {
        self.dag.use_candidates(node).iter().any(|user| {
            !self.dead.contains(user) && self.dag.nodes()[*user as usize].ops.iter().any(|op| op.node == node)
        })
    }

    /// The live nodes that read a result of `node`, in creation order, once
    /// each: the use list, not a scan of the arena.
    fn live_user_nodes(
        &self,
        node: u32,
    ) -> Vec<u32> {
        let mut users: Vec<u32> =
            self.dag.use_candidates(node).iter().copied().filter(|user| !self.dead.contains(user)).collect();
        users.sort_unstable();
        users.dedup();
        users
    }

    /// The weight of the reads of `node`'s results by live nodes (`weight` of
    /// each read), each user's once, summed until `count_to`
    /// (`hasNUsesOfValue`'s walk). A user the use list names twice is read
    /// once; users that weigh nothing are not remembered, so the repeat test
    /// scans at most `count_to` of them.
    fn reads_of(
        &self,
        node: u32,
        res: Option<u8>,
        count_to: usize,
        weight: impl Fn(SDValue) -> usize,
    ) -> usize {
        // The questions stop at one to three reads: eight users fit the stack.
        let (mut readers, mut count) = ([0u32; 8], 0);
        let candidates = self.dag.use_candidates(node);
        let mut reads = 0;
        for (at, &user) in candidates.iter().enumerate() {
            if self.dead.contains(&user)
                || readers[..count].contains(&user)
                || (count == readers.len() && candidates[..at].contains(&user))
            {
                continue;
            }
            let before = reads;
            for op in &self.dag.nodes()[user as usize].ops {
                if op.node == node && res.is_none_or(|r| op.res == r) {
                    reads += weight(*op);
                }
            }
            if reads > before && count < readers.len() {
                readers[count] = user;
                count += 1;
            }
            if reads >= count_to {
                break;
            }
        }
        reads
    }

    fn live_users(
        &self,
        value: PinValue,
    ) -> Vec<(u32, usize)> {
        let mut out = Vec::new();
        for user in self.live_user_nodes(value.node) {
            for (at, op) in self.dag.nodes()[user as usize].ops.iter().enumerate() {
                if op.node == value.node && op.res == value.res {
                    out.push((user, at));
                }
            }
        }
        out
    }

    fn remove(
        &mut self,
        node: u32,
    ) {
        if !self.dead.insert(node) {
            return;
        }
        self.dag.forget_node(node);
        let operands: Vec<u32> = self.dag.nodes()[node as usize].ops.iter().map(|o| o.node).collect();
        for op in operands {
            if op != self.dag.entry_node().node && !self.has_live_users(op) {
                self.remove(op);
            }
        }
    }
}

impl Dag for DagisAdapter<'_> {
    fn opcode(
        &self,
        node: u32,
    ) -> u16 {
        pin_opcode(self.dag.nodes()[node as usize].opcode)
    }
    fn is_machine(
        &self,
        node: u32,
    ) -> bool {
        matches!(self.dag.nodes()[node as usize].opcode, ISD::Machine(_))
    }
    fn register_of(
        &self,
        node: u32,
    ) -> Option<u16> {
        match self.dag.nodes()[node as usize].data {
            NodeData::PhysReg(id) => self.registers.pin_register(id),
            // `$noreg`
            NodeData::Register(0) => Some(0),
            _ => None,
        }
    }
    fn num_operands(
        &self,
        node: u32,
    ) -> usize {
        self.dag.nodes()[node as usize].ops.len()
    }
    fn operand(
        &self,
        node: u32,
        index: usize,
    ) -> PinValue {
        theirs(self.dag.nodes()[node as usize].ops[index])
    }
    fn num_values(
        &self,
        node: u32,
    ) -> usize {
        self.dag.nodes()[node as usize].values.len()
    }
    fn value_type(
        &self,
        value: PinValue,
    ) -> Vt {
        pin_vt(self.dag.value_type(ours(value)))
    }
    fn has_one_use(
        &self,
        value: PinValue,
    ) -> bool {
        self.has_n_uses_of_value(value, 1)
    }
    fn count_non_chain_uses(
        &self,
        node: u32,
        limit: usize,
    ) -> usize {
        self.reads_of(node, None, limit + 1, |op| usize::from(self.dag.value_type(op) != EVT::OTHER))
    }
    fn use_empty(
        &self,
        node: u32,
    ) -> bool {
        !self.has_live_users(node)
    }
    fn node_has_one_use(
        &self,
        node: u32,
    ) -> bool {
        self.reads_of(node, None, 2, |_| 1) == 1
    }
    fn users(
        &self,
        value: PinValue,
    ) -> Vec<(u32, usize)> {
        self.live_users(value)
    }
    fn glued_user(
        &self,
        node: u32,
    ) -> Option<u32> {
        let last = self.dag.nodes()[node as usize].values.len() - 1;
        if self.dag.nodes()[node as usize].values[last] != EVT::GLUE {
            return None;
        }
        self.dag
            .use_candidates(node)
            .iter()
            .copied()
            .filter(|user| {
                !self.dead.contains(user)
                    && self.dag.nodes()[*user as usize]
                        .ops
                        .iter()
                        .any(|op| op.node == node && usize::from(op.res) == last)
            })
            .min()
    }
    fn is_only_user_of(
        &self,
        node: u32,
        def: u32,
    ) -> bool {
        self.dag
            .use_candidates(def)
            .iter()
            .all(
                |&user| user == node
                    || self.dead.contains(&user)
                    || self.dag.nodes()[user as usize].ops.iter().all(|o| o.node != def),
            )
    }
    fn has_n_uses_of_value(
        &self,
        value: PinValue,
        n: usize,
    ) -> bool {
        self.reads_of(value.node, Some(value.res), n + 1, |_| 1) == n
    }
    fn constant(
        &self,
        value: PinValue,
    ) -> Option<i64> {
        let v = ours(value);
        let bits = self.dag.constant(v)?;
        let width = self.dag.value_type(v).size_in_bits();
        let shift = 128 - width;
        Some((((bits << shift) as i128) >> shift) as i64)
    }
    fn cond_code(
        &self,
        value: PinValue,
    ) -> Option<u8> {
        match self.dag.node(ours(value)).data {
            NodeData::CondCode(cc) => Some(cc as u8),
            _ => None,
        }
    }
    fn value_type_node(
        &self,
        value: PinValue,
    ) -> Option<Vt> {
        match self.dag.node(ours(value)).data {
            NodeData::ValueType(vt) => Some(pin_vt(vt)),
            _ => None,
        }
    }
    fn is_undef(
        &self,
        value: PinValue,
    ) -> bool {
        self.dag.is_undef(ours(value))
    }
    fn mem_operands(
        &self,
        node: u32,
    ) -> Option<Vec<MemRef>> {
        matches!(self.dag.nodes()[node as usize].data, NodeData::Mem(_)).then(|| vec![node])
    }
    fn is_call_argument_store(
        &self,
        node: u32,
    ) -> bool {
        let node = &self.dag.nodes()[node as usize];
        node.opcode == ISD::Store
            && matches!(
                node.data,
                NodeData::Mem(info) if matches!(info.base, super::dag::MemBase::CallArgument(_))
            )
    }
    fn load_ext_type(
        &self,
        node: u32,
    ) -> Option<LoadExt> {
        let node = &self.dag.nodes()[node as usize];
        matches!(node.opcode, ISD::Load | ISD::LoadPre | ISD::LoadPost).then(|| match node.data {
            NodeData::Mem(info) => match info.ext {
                None => LoadExt::NonExt,
                Some(super::actions::LoadExt::ExtLoad) => LoadExt::Ext,
                Some(super::actions::LoadExt::SExtLoad) => LoadExt::SExt,
                Some(super::actions::LoadExt::ZExtLoad) => LoadExt::ZExt,
            },
            _ => LoadExt::NonExt,
        })
    }
    fn is_indexed(
        &self,
        node: u32,
    ) -> bool {
        matches!(
            self.dag.nodes()[node as usize].opcode,
            ISD::LoadPre | ISD::LoadPost | ISD::StorePre | ISD::StorePost
        )
    }
    fn is_post_indexed(
        &self,
        node: u32,
    ) -> bool {
        matches!(self.dag.nodes()[node as usize].opcode, ISD::LoadPost | ISD::StorePost)
    }
    fn is_truncating_store(
        &self,
        node: u32,
    ) -> bool {
        let n = &self.dag.nodes()[node as usize];
        match n.data {
            NodeData::Mem(info) if matches!(n.opcode, ISD::Store | ISD::StorePre | ISD::StorePost) => {
                self.dag.value_type(n.ops[1]).size_in_bits() > info.mem_vt.size_in_bits()
            }
            _ => false,
        }
    }
    fn memory_vt(
        &self,
        node: u32,
    ) -> Vt {
        match self.dag.nodes()[node as usize].data {
            NodeData::Mem(info) => pin_vt(info.mem_vt),
            _ => panic!("node {node} has no memory operand"),
        }
    }
    fn memory_store_size(
        &self,
        node: u32,
    ) -> u64 {
        match self.dag.nodes()[node as usize].data {
            NodeData::Mem(info) => u64::from(info.mem_vt.size_in_bits().div_ceil(8)),
            _ => panic!("node {node} has no memory operand"),
        }
    }
    fn alignment(
        &self,
        node: u32,
    ) -> u64 {
        match self.dag.nodes()[node as usize].data {
            NodeData::Mem(info) => u64::from(info.align),
            _ => panic!("node {node} has no memory operand"),
        }
    }
    fn is_simple(
        &self,
        node: u32,
    ) -> bool {
        match self.dag.nodes()[node as usize].data {
            NodeData::Mem(info) => !info.volatile,
            _ => false,
        }
    }
    fn fp_class(
        &self,
        value: PinValue,
    ) -> Option<FpClass> {
        let NodeData::ConstFp { bits } = self.dag.node(ours(value)).data else { return None };
        let (zero, neg_zero, one, minus_one) = match self.dag.value_type(ours(value)) {
            EVT::Simple(MVT::F32) => (0, 0x8000_0000, 0x3f80_0000, 0xbf80_0000),
            EVT::Simple(MVT::F64) => (0, 1 << 63, 0x3ff0_0000_0000_0000, 0xbff0_0000_0000_0000),
            // x87 extended: sign, 15-bit exponent, an explicit integer bit.
            EVT::Simple(MVT::F80) => (0, 1 << 79, (0x3fff << 64) | (1 << 63), (0xbfff << 64) | (1 << 63)),
            _ => return Some(FpClass::Other),
        };
        Some(if bits == zero {
            FpClass::PosZero
        } else if bits == neg_zero {
            FpClass::NegZero
        } else if bits == one {
            FpClass::One
        } else if bits == minus_one {
            FpClass::MinusOne
        } else {
            FpClass::Other
        })
    }
    fn has_disjoint_flag(
        &self,
        node: u32,
    ) -> bool {
        self.dag.nodes()[node as usize].flags.has(SDNodeFlags::DISJOINT)
    }
    fn masked_value_is_zero(
        &self,
        value: PinValue,
        mask: u64,
    ) -> bool {
        super::known_bits::masked_value_is_zero(self.dag, ours(value), u128::from(mask))
    }
    fn have_no_common_bits_set(
        &self,
        a: PinValue,
        b: PinValue,
    ) -> bool {
        no_common_bits(self.dag, ours(a), ours(b))
    }
    fn mem_ref_is_load(
        &self,
        mem: MemRef,
    ) -> bool {
        self.dag.nodes()[mem as usize].opcode == ISD::Load
    }
    fn mem_ref_is_store(
        &self,
        mem: MemRef,
    ) -> bool {
        self.dag.nodes()[mem as usize].opcode == ISD::Store
    }
    fn set_node_mem_refs(
        &mut self,
        node: u32,
        mems: &[MemRef],
    ) {
        // A node that folded a volatile access is one (`MachineMemOperand`s
        // carry the flag): the LIR instruction keeps it.
        if mems.iter().any(|mem| self.dag.volatile_accesses.contains(mem)) {
            self.dag.volatile_accesses.insert(node);
        }
        self.mem_refs.insert(node, mems.to_vec());
    }

    fn entry_node(&mut self) -> PinValue {
        theirs(self.dag.entry_node())
    }
    fn target_constant(
        &mut self,
        value: i64,
        vt: Vt,
    ) -> PinValue {
        theirs(self.dag.get_constant_with(value as u128, our_vt(vt), true, false))
    }
    fn convert_to_target(
        &mut self,
        value: PinValue,
    ) -> PinValue {
        let v = ours(value);
        if self.dag.opcode(v) == ISD::Constant {
            let bits = self.dag.constant(v).expect("a constant");
            let vt = self.dag.value_type(v);
            return theirs(self.dag.get_constant_with(bits, vt, true, false));
        }
        value
    }
    fn register(
        &mut self,
        register: u16,
        vt: Vt,
    ) -> PinValue {
        let node = self.register_node(our_vt(vt), register);
        theirs(node)
    }
    fn copy_to_reg(
        &mut self,
        chain: PinValue,
        register: u16,
        value: PinValue,
        glue: Option<PinValue>,
    ) -> u32 {
        let vt = self.dag.value_type(ours(value));
        let reg = self.register_node(vt, register);
        let mut ops = vec![ours(chain), reg, ours(value)];
        ops.extend(glue.map(ours));
        self.dag.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &ops).node
    }
    fn copy_from_reg(
        &mut self,
        chain: PinValue,
        register: u16,
        vt: Vt,
        glue: PinValue,
    ) -> u32 {
        let vt = our_vt(vt);
        let reg = self.register_node(vt, register);
        self.dag.get_node_multi(ISD::CopyFromReg, &[vt, EVT::OTHER, EVT::GLUE], &[ours(chain), reg, ours(glue)]).node
    }
    fn copy_from_reg_node(
        &mut self,
        chain: PinValue,
        register: PinValue,
        vt: Vt,
        glue: Option<PinValue>,
    ) -> u32 {
        let vt = our_vt(vt);
        let mut ops = vec![ours(chain), ours(register)];
        ops.extend(glue.map(ours));
        let vts: &[EVT] = if glue.is_some() { &[vt, EVT::OTHER, EVT::GLUE] } else { &[vt, EVT::OTHER] };
        self.dag.get_node_multi(ISD::CopyFromReg, vts, &ops).node
    }
    fn token_factor(
        &mut self,
        chains: &[PinValue],
    ) -> PinValue {
        let chains: Vec<SDValue> = chains.iter().copied().map(ours).collect();
        theirs(self.dag.get_token_factor(&chains))
    }
    fn machine_node(
        &mut self,
        opcode: u16,
        vts: &[Vt],
        operands: &[PinValue],
    ) -> u32 {
        let vts: Vec<EVT> = vts.iter().copied().map(our_vt).collect();
        let ops: Vec<SDValue> = operands.iter().copied().map(ours).collect();
        self.dag.intern(ISD::Machine(u32::from(opcode)), &vts, &ops, SDNodeFlags::default(), NodeData::None).node
    }
    fn morph_node_to(
        &mut self,
        node: u32,
        opcode: u16,
        vts: &[Vt],
        operands: &[PinValue],
    ) -> (u32, Vec<u32>) {
        let vts: Vec<EVT> = vts.iter().copied().map(our_vt).collect();
        let ops: Vec<SDValue> = operands.iter().copied().map(ours).collect();
        let morphed = self.dag.morph_node_to(node, ISD::Machine(u32::from(opcode)), &vts, &ops);
        // `MorphNodeTo` reuses the node with new operands and resets its id
        // ("to the isel this is a newly allocated machine node"): left small
        // it would be pruned from a walk toward the later nodes it now reads.
        if morphed == node {
            self.mark_selected(node);
        }
        self.enforce_node_id_invariant(morphed);
        (morphed, Vec::new())
    }
    fn update_node_operands(
        &mut self,
        node: u32,
        operands: &[PinValue],
    ) -> u32 {
        let ops: Vec<SDValue> = operands.iter().copied().map(ours).collect();
        self.dag.update_node_operands(node, &ops)
    }
    fn replace_uses(
        &mut self,
        from: PinValue,
        to: PinValue,
    ) {
        self.dag.replace_all_uses_with(ours(from), ours(to));
        self.enforce_node_id_invariant(to.node);
    }
    fn replace_node(
        &mut self,
        from: u32,
        to: u32,
    ) {
        for res in 0..self.dag.nodes()[from as usize].values.len() as u8 {
            self.dag.replace_all_uses_with(SDValue { node: from, res }, SDValue { node: to, res });
        }
        self.enforce_node_id_invariant(to);
        self.remove(from);
    }
    fn remove_dead_node(
        &mut self,
        node: u32,
    ) {
        self.remove(node);
    }
    fn remove_dead_nodes(
        &mut self,
        nodes: &[u32],
    ) {
        for &node in nodes {
            self.remove(node);
        }
    }
    /// `SDNode::hasPredecessorHelper` with `TopologicalPrune`, as every caller
    /// in the pin asks: a node whose id is below the searched node's cannot
    /// have it among its predecessors, so the walk does not go through it
    /// (it stays local, where an unpruned one visits everything before it).
    fn has_predecessor_helper(
        &self,
        node: u32,
        visited: &mut llrm_dagisel::dag::Visited,
        worklist: &mut Vec<u32>,
        max: usize,
    ) -> bool {
        if visited.contains(node) {
            return true;
        }
        let searched = match self.id_of(node) {
            id if id < -1 => -(id + 1),
            id => id,
        };
        let mut deferred = Vec::new();
        let mut found = false;
        while let Some(m) = worklist.pop() {
            let id = self.id_of(m);
            if self.dag.nodes()[m as usize].opcode != ISD::TokenFactor && searched > 0 && id > 0 && id < searched {
                deferred.push(m);
                continue;
            }
            for op in &self.dag.nodes()[m as usize].ops {
                if visited.insert(op.node) {
                    worklist.push(op.node);
                }
                if op.node == node {
                    found = true;
                }
            }
            if found || (max != 0 && visited.len() >= max) {
                break;
            }
        }
        worklist.extend(deferred);
        found || (max != 0 && visited.len() >= max)
    }
}

impl AddrDag for DagisAdapter<'_> {
    fn segment_override(
        &self,
        node: u32,
    ) -> Option<u16> {
        use llrm_dagisel::generated::ids::x86;
        match self.dag.nodes()[node as usize].data {
            // The pin's address spaces 256, 257 and 258 are GS, FS and SS.
            NodeData::Mem(info) => match info.space {
                256 => Some(x86::GS),
                257 => Some(x86::FS),
                258 => Some(x86::SS),
                // llrm's own: the segment `les` left in ES, and code-segment
                // reads.
                259 => Some(x86::ES),
                260 => Some(x86::CS),
                _ => None,
            },
            _ => None,
        }
    }
    fn is_add_like(
        &self,
        value: PinValue,
    ) -> bool {
        let node = &self.dag.nodes()[value.node as usize];
        match node.opcode {
            ISD::Or => {
                node.flags.has(SDNodeFlags::DISJOINT)
                    || no_common_bits(self.dag, ours(self.operand(value.node, 0)), ours(self.operand(value.node, 1)))
            }
            // An XOR with the sign bit adds it.
            ISD::Xor => {
                let rhs = ours(self.operand(value.node, 1));
                let width = self.dag.value_type(rhs).size_in_bits();
                self.dag.constant(rhs) == Some(1u128 << (width - 1))
            }
            _ => false,
        }
    }
    fn frame_index(
        &self,
        value: PinValue,
    ) -> Option<i32> {
        match self.dag.node(ours(value)).data {
            NodeData::FrameIndex(i) => Some(i),
            _ => None,
        }
    }
    fn wrapped_symbol(
        &self,
        operand: PinValue,
    ) -> Option<Wrapped> {
        match self.dag.node(ours(operand)).data {
            NodeData::Global { symbol, offset } => {
                Some(Wrapped { symbol: Symbol { kind: SymbolKind::Global, id: symbol }, offset, flags: 0 })
            }
            NodeData::Symbol(id) => {
                Some(Wrapped { symbol: Symbol { kind: SymbolKind::External, id }, offset: 0, flags: 0 })
            }
            NodeData::PoolIndex(index, offset) => {
                // The alignment `getTargetConstantPool` is given: the entry's
                // type size, by `ExpandConstantFP` (SD203).
                let (_, vt) = self.dag.constant_pool[index as usize];
                let align = (vt.size_in_bits() / 8).next_power_of_two().clamp(1, 16);
                Some(Wrapped {
                    symbol: Symbol { kind: SymbolKind::ConstantPool { align }, id: index },
                    offset: i64::from(offset),
                    flags: 0,
                })
            }
            _ => None,
        }
    }
    fn has_materializing_use(
        &self,
        value: PinValue,
    ) -> bool {
        self.dag
            .use_candidates(value.node)
            .iter()
            .any(
                |&user| {
                    let node = &self.dag.nodes()[user as usize];
                    !self.dead.contains(&user)
                        && node.ops.iter().enumerate().any(|(at, op)| {
                            op.node == value.node
                                && op.res == value.res
                                && (node.opcode == ISD::CopyToReg || (node.opcode == ISD::Store && at == 1))
                        })
                },
            )
    }
    fn get_node(
        &mut self,
        opcode: u16,
        vt: Vt,
        ops: &[PinValue],
    ) -> PinValue {
        let ops: Vec<SDValue> = ops.iter().copied().map(ours).collect();
        let isd = match opcode {
            pin::ADD => ISD::Add,
            pin::SUB => ISD::Sub,
            pin::AND => ISD::And,
            pin::OR => ISD::Or,
            pin::XOR => ISD::Xor,
            pin::SHL => ISD::Shl,
            pin::SRL => ISD::Srl,
            pin::TRUNCATE => ISD::Truncate,
            pin::ZERO_EXTEND => ISD::ZeroExtend,
            pin::ANY_EXTEND => ISD::AnyExtend,
            other => panic!("the address folds never make pin opcode {other}"),
        };
        theirs(self.dag.get_node(isd, our_vt(vt), &ops))
    }
    fn get_constant(
        &mut self,
        value: i64,
        vt: Vt,
    ) -> PinValue {
        theirs(self.dag.get_constant(value as u128, our_vt(vt)))
    }
    fn is_non_temporal(
        &self,
        _: u32,
    ) -> bool {
        false
    }
    fn target_frame_index(
        &mut self,
        index: i32,
        vt: Vt,
    ) -> PinValue {
        theirs(self.dag.intern(
            ISD::TargetFrameIndex,
            &[our_vt(vt)],
            &[],
            SDNodeFlags::default(),
            NodeData::FrameIndex(index),
        ))
    }
    fn target_symbol(
        &mut self,
        symbol: Symbol,
        disp: i32,
        _flags: u8,
    ) -> PinValue {
        let (opcode, data) = match symbol.kind {
            SymbolKind::Global => {
                (ISD::TargetGlobalAddress, NodeData::Global { symbol: symbol.id, offset: i64::from(disp) })
            }
            SymbolKind::External => (ISD::ExternalSymbol, NodeData::Symbol(symbol.id)),
            SymbolKind::ConstantPool { .. } => (ISD::TargetConstantPool, NodeData::PoolIndex(symbol.id, disp)),
            other => panic!("{other:?} symbols are not in our DAG"),
        };
        theirs(self.dag.intern(opcode, &[EVT::I32], &[], SDNodeFlags::default(), data))
    }
}

/// `SelectionDAG::haveNoCommonBitsSet`, by the known bits: every bit is known
/// zero in one of the two.
fn no_common_bits(
    dag: &SelectionDAG,
    a: SDValue,
    b: SDValue,
) -> bool {
    let vt = dag.value_type(a);
    if !vt.is_integer() || vt != dag.value_type(b) {
        return false;
    }
    let all = super::fold::mask(vt.size_in_bits());
    (super::known_bits::known_bits(dag, a).zero | super::known_bits::known_bits(dag, b).zero) & all == all
}
