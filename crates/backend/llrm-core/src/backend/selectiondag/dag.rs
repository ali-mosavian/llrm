//! The arena: nodes, values, the CSE map and node construction.

use std::collections::HashMap;

use super::isd::{CondCode, ISD};
use super::mvt::EVT;

/// One result of a node: `SDValue(Node, ResNo)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SDValue {
    pub node: u32,
    pub res: u8,
}

/// `SDNodeFlags`; CSE keeps the flags both builders agree on
/// (`intersectFlagsWith`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SDNodeFlags(pub u16);

impl SDNodeFlags {
    pub const NO_UNSIGNED_WRAP: SDNodeFlags = SDNodeFlags(1);
    pub const NO_SIGNED_WRAP: SDNodeFlags = SDNodeFlags(2);
    pub const EXACT: SDNodeFlags = SDNodeFlags(4);
    pub const NON_NEG: SDNodeFlags = SDNodeFlags(8);
    pub const DISJOINT: SDNodeFlags = SDNodeFlags(16);
    pub const SAME_SIGN: SDNodeFlags = SDNodeFlags(32);
    pub const IN_BOUNDS: SDNodeFlags = SDNodeFlags(64);
    pub const NO_NANS: SDNodeFlags = SDNodeFlags(128);
    pub const NO_INFS: SDNodeFlags = SDNodeFlags(256);
    pub const NO_SIGNED_ZEROS: SDNodeFlags = SDNodeFlags(512);
    pub const ALLOW_RECIPROCAL: SDNodeFlags = SDNodeFlags(1024);
    pub const ALLOW_CONTRACT: SDNodeFlags = SDNodeFlags(2048);
    pub const ALLOW_REASSOC: SDNodeFlags = SDNodeFlags(4096);

    pub fn union(self, other: SDNodeFlags) -> SDNodeFlags {
        SDNodeFlags(self.0 | other.0)
    }

    pub fn with(self, flag: SDNodeFlags, on: bool) -> SDNodeFlags {
        if on { self.union(flag) } else { self }
    }

    pub fn intersect(self, other: SDNodeFlags) -> SDNodeFlags {
        SDNodeFlags(self.0 & other.0)
    }

    pub fn has(self, flag: SDNodeFlags) -> bool {
        self.0 & flag.0 == flag.0
    }
}

/// What a leaf node carries besides its opcode and types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeData {
    None,
    /// `ConstantSDNode`: the bits at the node's width, zero above it.
    Const { bits: u128, opaque: bool },
    /// `ConstantFPSDNode`, as the bits of its type (DEVIATE SD11).
    ConstFp { bits: u128 },
    CondCode(CondCode),
    ValueType(EVT),
    Register(u32),
    /// A physical register of the target being compiled.
    PhysReg(llrm_lir::registers::RegId),
    FrameIndex(i32),
    Global { symbol: u32, offset: i64 },
    Block(u32),
    Symbol(u32),
    Mem(MemInfo),
    JumpTable(u32),
}

/// What identifies a memory access beyond its operands: the pin's
/// `MachineMemOperand` as far as `CSEMap` and the dump use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemInfo {
    pub mem_vt: EVT,
    pub align: u32,
    pub volatile: bool,
    pub base: MemBase,
}

/// `MachinePointerInfo`'s value: what the access is relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemBase {
    Unknown,
    Global(u32),
    Stack(i32),
}

#[derive(Clone, Debug)]
pub struct SDNode {
    pub opcode: ISD,
    pub values: Vec<EVT>,
    pub ops: Vec<SDValue>,
    pub flags: SDNodeFlags,
    pub data: NodeData,
}

/// `TargetLowering::BooleanContent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanContents {
    ZeroOrOne,
    ZeroOrNegativeOne,
    Undefined,
}

/// The target facts `getNode` asks (`TLI->` in the pin), passed in rather
/// than read from a global.
#[derive(Clone, Copy, Debug)]
pub struct DagConfig {
    pub boolean_contents: BooleanContents,
    /// `isOffsetFoldingLegal`: `(add GlobalAddress, c)` folds into the symbol.
    pub offset_folding: bool,
    /// The pointer type (`TLI->getPointerTy`).
    pub pointer: EVT,
}

impl Default for DagConfig {
    /// x86: 0 or 1; symbol offsets do not fold (`isOffsetFoldingLegal` is false for the default PIC model).
    fn default() -> DagConfig {
        DagConfig { boolean_contents: BooleanContents::ZeroOrOne, offset_folding: false, pointer: EVT::I32 }
    }
}

/// What a `replace_all_uses_with` did to a user of the replaced value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rauw {
    /// The user reads the new value and is still its own node.
    Updated(u32),
    /// The user became equal to an existing node and its results were
    /// replaced by that node's in turn.
    Merged { from: u32, into: u32 },
}

type CseKey = (ISD, Vec<EVT>, Vec<SDValue>, NodeData);

/// `SelectionDAG`: nodes in an arena indexed by [`SDValue::node`].
pub struct SelectionDAG {
    nodes: Vec<SDNode>,
    /// Per node, the nodes that have read one of its results (may hold
    /// stale or repeated entries; `users` filters them).
    uses: Vec<Vec<u32>>,
    cse: HashMap<CseKey, u32>,
    pub(super) config: DagConfig,
    entry: SDValue,
    root: SDValue,
    /// Names the dump prints for `Global`/`Block` payloads, by id.
    pub global_names: Vec<String>,
    pub block_names: Vec<String>,
    /// Names of external symbols, by id.
    pub symbol_names: Vec<String>,
}

impl SelectionDAG {
    pub fn new(config: DagConfig) -> SelectionDAG {
        let zero = SDValue { node: 0, res: 0 };
        let mut dag = SelectionDAG { nodes: Vec::new(), uses: Vec::new(), cse: HashMap::new(), config, entry: zero, root: zero, global_names: Vec::new(), block_names: Vec::new(), symbol_names: Vec::new() };
        dag.entry = dag.intern(ISD::EntryToken, &[EVT::OTHER, EVT::GLUE], &[], SDNodeFlags::default(), NodeData::None);
        dag.root = dag.entry;
        dag
    }

    pub fn entry_node(&self) -> SDValue {
        self.entry
    }

    pub fn get_root(&self) -> SDValue {
        self.root
    }

    pub fn set_root(&mut self, root: SDValue) {
        self.root = root;
    }

    pub fn config(&self) -> &DagConfig {
        &self.config
    }

    /// How many results of other nodes read `value`'s node (`use_size`).
    pub fn use_count(&self, node: u32) -> usize {
        self.nodes.iter().flat_map(|other| &other.ops).filter(|op| op.node == node).count()
    }

    pub fn nodes(&self) -> &[SDNode] {
        &self.nodes
    }

    /// Every node id in creation order (`allnodes()`).
    pub fn node_ids(&self) -> std::ops::Range<u32> {
        0..self.nodes.len() as u32
    }

    /// The nodes that read `value` as an operand, in creation order, once
    /// each (`SDValue::users()`).
    pub fn users(&self, value: SDValue) -> impl Iterator<Item = u32> + '_ {
        let mut users: Vec<u32> = self.uses.get(value.node as usize).into_iter().flatten().copied().filter(|&user| self.nodes[user as usize].ops.contains(&value)).collect();
        users.sort_unstable();
        users.dedup();
        users.into_iter()
    }

    fn note_uses(&mut self, user: u32) {
        for at in 0..self.nodes[user as usize].ops.len() {
            let operand = self.nodes[user as usize].ops[at].node as usize;
            self.uses[operand].push(user);
        }
    }

    fn key_of(&self, node: u32) -> Option<CseKey> {
        let data = &self.nodes[node as usize];
        (!data.values.contains(&EVT::GLUE)).then(|| (data.opcode, data.values.clone(), data.ops.clone(), data.data))
    }

    /// `ReplaceAllUsesWith(From, To)`: every operand `from` becomes `to`. A
    /// user that then equals an existing node is replaced by it in turn
    /// (`AddModifiedNodeToCSEMaps`); the arena only grows, so replaced nodes
    /// stay in it with no users. The root follows.
    pub fn replace_all_uses_with(&mut self, from: SDValue, to: SDValue) -> Vec<Rauw> {
        let mut events = Vec::new();
        if from == to {
            return events;
        }
        let mut work = vec![(from, to)];
        while let Some((from, to)) = work.pop() {
            if self.root == from {
                self.root = to;
            }
            for user in self.users(from).collect::<Vec<_>>() {
                if let Some(key) = self.key_of(user) {
                    if self.cse.get(&key) == Some(&user) {
                        self.cse.remove(&key);
                    }
                }
                for op in &mut self.nodes[user as usize].ops {
                    if *op == from {
                        *op = to;
                    }
                }
                self.uses[to.node as usize].push(user);
                let Some(key) = self.key_of(user) else {
                    events.push(Rauw::Updated(user));
                    continue;
                };
                match self.cse.get(&key) {
                    Some(&existing) if existing != user => {
                        events.push(Rauw::Merged { from: user, into: existing });
                        let flags = self.nodes[existing as usize].flags.intersect(self.nodes[user as usize].flags);
                        self.nodes[existing as usize].flags = flags;
                        let results = self.nodes[user as usize].values.len() as u8;
                        for res in 0..results {
                            work.push((SDValue { node: user, res }, SDValue { node: existing, res }));
                        }
                    }
                    _ => {
                        events.push(Rauw::Updated(user));
                        self.cse.insert(key, user);
                    }
                }
            }
        }
        events
    }

    /// `UpdateNodeOperands`: `node` reads `ops` instead. If that equals an
    /// existing node, that node is returned (and `node` stays as it was,
    /// unused); else `node` itself, re-entered in the CSE map.
    pub fn update_node_operands(&mut self, node: u32, ops: &[SDValue]) -> u32 {
        if let Some(key) = self.key_of(node) {
            if self.cse.get(&key) == Some(&node) {
                self.cse.remove(&key);
            }
        }
        let old = std::mem::replace(&mut self.nodes[node as usize].ops, ops.to_vec());
        self.note_uses(node);
        match self.key_of(node) {
            Some(key) => match self.cse.get(&key) {
                Some(&existing) if existing != node => {
                    self.nodes[node as usize].ops = old;
                    if let Some(key) = self.key_of(node) {
                        self.cse.insert(key, node);
                    }
                    existing
                }
                _ => {
                    self.cse.insert(key, node);
                    node
                }
            },
            None => node,
        }
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(&self, value: SDValue) -> &SDNode {
        &self.nodes[value.node as usize]
    }

    pub fn opcode(&self, value: SDValue) -> ISD {
        self.node(value).opcode
    }

    pub fn value_type(&self, value: SDValue) -> EVT {
        self.node(value).values[value.res as usize]
    }

    pub fn operand(&self, value: SDValue, index: usize) -> SDValue {
        self.node(value).ops[index]
    }

    pub fn is_undef(&self, value: SDValue) -> bool {
        matches!(self.opcode(value), ISD::Undef | ISD::Poison)
    }

    /// The value of a `ConstantSDNode` (`Constant` or `TargetConstant`).
    pub fn constant(&self, value: SDValue) -> Option<u128> {
        match (self.opcode(value), self.node(value).data) {
            (ISD::Constant | ISD::TargetConstant, NodeData::Const { bits, .. }) => Some(bits),
            _ => None,
        }
    }

    pub fn is_opaque(&self, value: SDValue) -> bool {
        matches!(self.node(value).data, NodeData::Const { opaque: true, .. })
    }

    pub fn is_null_constant(&self, value: SDValue) -> bool {
        self.constant(value) == Some(0)
    }

    pub(super) fn width(&self, value: SDValue) -> u32 {
        self.value_type(value).size_in_bits()
    }

    /// `lookupNode` then `newSDNode`: one node per (opcode, types, operands,
    /// data); glue is never shared.
    pub(super) fn intern(
        &mut self,
        opcode: ISD,
        values: &[EVT],
        ops: &[SDValue],
        flags: SDNodeFlags,
        data: NodeData,
    ) -> SDValue {
        let shareable = !values.contains(&EVT::GLUE);
        let key = (opcode, values.to_vec(), ops.to_vec(), data);
        if shareable {
            if let Some(&found) = self.cse.get(&key) {
                let node = &mut self.nodes[found as usize];
                node.flags = node.flags.intersect(flags);
                return SDValue { node: found, res: 0 };
            }
        }
        let index = self.nodes.len() as u32;
        self.nodes.push(SDNode { opcode, values: key.1.clone(), ops: key.2.clone(), flags, data });
        self.uses.push(Vec::new());
        self.note_uses(index);
        if shareable {
            self.cse.insert(key, index);
        }
        SDValue { node: index, res: 0 }
    }

    pub fn get_undef(&mut self, vt: EVT) -> SDValue {
        self.intern(ISD::Undef, &[vt], &[], SDNodeFlags::default(), NodeData::None)
    }

    pub fn get_poison(&mut self, vt: EVT) -> SDValue {
        self.intern(ISD::Poison, &[vt], &[], SDNodeFlags::default(), NodeData::None)
    }

    pub fn get_constant(&mut self, bits: u128, vt: EVT) -> SDValue {
        self.get_constant_with(bits, vt, false, false)
    }

    /// `getConstant` with `isT` (a `TargetConstant`) and `isO` (opaque).
    pub fn get_constant_with(&mut self, bits: u128, vt: EVT, target: bool, opaque: bool) -> SDValue {
        assert!(vt.is_integer(), "integer constant of {vt:?}");
        let bits = bits & super::fold::mask(vt.size_in_bits());
        let opcode = if target { ISD::TargetConstant } else { ISD::Constant };
        self.intern(opcode, &[vt], &[], SDNodeFlags::default(), NodeData::Const { bits, opaque })
    }

    /// `getConstantFP` by the bits of the type (DEVIATE SD11: no `APFloat`).
    pub fn get_constant_fp(&mut self, bits: u128, vt: EVT) -> SDValue {
        assert!(vt.is_floating_point(), "FP constant of {vt:?}");
        self.intern(ISD::ConstantFP, &[vt], &[], SDNodeFlags::default(), NodeData::ConstFp { bits })
    }

    pub fn get_all_ones_constant(&mut self, vt: EVT) -> SDValue {
        self.get_constant(u128::MAX, vt)
    }

    /// `getBoolConstant`: true is 1, or all ones for `ZeroOrNegativeOne`.
    pub fn get_bool_constant(&mut self, value: bool, vt: EVT) -> SDValue {
        if !value {
            return self.get_constant(0, vt);
        }
        match self.config.boolean_contents {
            BooleanContents::ZeroOrOne | BooleanContents::Undefined => self.get_constant(1, vt),
            BooleanContents::ZeroOrNegativeOne => self.get_all_ones_constant(vt),
        }
    }

    /// `isBoolConstant`.
    pub fn bool_constant(&self, value: SDValue) -> Option<bool> {
        let bits = self.constant(value)?;
        let all_ones = super::fold::mask(self.width(value));
        match self.config.boolean_contents {
            BooleanContents::ZeroOrOne => (bits <= 1).then_some(bits == 1),
            BooleanContents::ZeroOrNegativeOne => (bits == 0 || bits == all_ones).then_some(bits != 0),
            BooleanContents::Undefined => Some(bits != 0),
        }
    }

    pub fn get_basic_block(&mut self, block: u32) -> SDValue {
        self.intern(ISD::BasicBlock, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::Block(block))
    }

    pub fn get_frame_index(&mut self, vt: EVT, index: i32) -> SDValue {
        self.intern(ISD::FrameIndex, &[vt], &[], SDNodeFlags::default(), NodeData::FrameIndex(index))
    }

    /// `getTokenFactor`.
    pub fn get_token_factor(&mut self, chains: &[SDValue]) -> SDValue {
        self.get_node(ISD::TokenFactor, EVT::OTHER, chains)
    }

    /// `getCopyToReg` without glue.
    pub fn get_copy_to_reg(&mut self, chain: SDValue, reg: u32, value: SDValue) -> SDValue {
        let reg_node = self.get_register(self.value_type(value), reg);
        self.get_node(ISD::CopyToReg, EVT::OTHER, &[chain, reg_node, value])
    }

    /// `getCopyFromReg`: the value is result 0, the chain result 1.
    pub fn get_copy_from_reg(&mut self, chain: SDValue, reg: u32, vt: EVT) -> SDValue {
        let reg_node = self.get_register(vt, reg);
        self.get_node_multi(ISD::CopyFromReg, &[vt, EVT::OTHER], &[chain, reg_node])
    }

    /// `getLoad` (unindexed, non-extending): `load chain, ptr, poison`.
    pub fn get_load(&mut self, vt: EVT, chain: SDValue, ptr: SDValue, info: MemInfo) -> SDValue {
        let offset = self.get_poison(self.value_type(ptr));
        self.intern(ISD::Load, &[vt, EVT::OTHER], &[chain, ptr, offset], SDNodeFlags::default(), NodeData::Mem(info))
    }

    /// `getStore` (unindexed, not truncating): `store chain, value, ptr, poison`.
    pub fn get_store(&mut self, chain: SDValue, value: SDValue, ptr: SDValue, info: MemInfo) -> SDValue {
        let offset = self.get_poison(self.value_type(ptr));
        self.intern(ISD::Store, &[EVT::OTHER], &[chain, value, ptr, offset], SDNodeFlags::default(), NodeData::Mem(info))
    }

    /// `getMemBasePlusOffset`: `add ptr, offset` (`PTRADD` only where the
    /// target asks to preserve pointer arithmetic, and ours does not).
    pub fn get_mem_base_plus_offset(&mut self, ptr: SDValue, offset: SDValue, flags: SDNodeFlags) -> SDValue {
        let vt = self.value_type(ptr);
        let flags = SDNodeFlags(flags.0 & !SDNodeFlags::IN_BOUNDS.0);
        self.get_node_flags(ISD::Add, vt, &[ptr, offset], flags)
    }

    /// `getObjectPtrOffset` with a constant offset; the constant node exists
    /// even when it is zero and the add folds away, as in the pin.
    pub fn get_object_ptr_offset(&mut self, ptr: SDValue, offset: i64) -> SDValue {
        let vt = self.value_type(ptr);
        let index = self.get_constant(offset as u128, vt);
        self.get_mem_base_plus_offset(ptr, index, SDNodeFlags::NO_UNSIGNED_WRAP.union(SDNodeFlags::IN_BOUNDS))
    }

    pub fn get_zext_or_trunc(&mut self, value: SDValue, vt: EVT) -> SDValue {
        let from = self.value_type(value);
        if from == vt {
            value
        } else if from.bits_gt(vt) {
            self.get_node(ISD::Truncate, vt, &[value])
        } else {
            self.get_node(ISD::ZeroExtend, vt, &[value])
        }
    }

    pub fn get_sext_or_trunc(&mut self, value: SDValue, vt: EVT) -> SDValue {
        let from = self.value_type(value);
        if from == vt {
            value
        } else if from.bits_gt(vt) {
            self.get_node(ISD::Truncate, vt, &[value])
        } else {
            self.get_node(ISD::SignExtend, vt, &[value])
        }
    }

    pub fn get_cond_code(&mut self, cond: CondCode) -> SDValue {
        self.intern(ISD::CondCodeNode, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::CondCode(cond))
    }

    pub fn get_value_type_node(&mut self, vt: EVT) -> SDValue {
        self.intern(ISD::ValueType, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::ValueType(vt))
    }

    pub fn get_global_address(&mut self, vt: EVT, symbol: u32, offset: i64) -> SDValue {
        self.intern(ISD::GlobalAddress, &[vt], &[], SDNodeFlags::default(), NodeData::Global { symbol, offset })
    }

    pub fn get_phys_register(&mut self, vt: EVT, reg: llrm_lir::registers::RegId) -> SDValue {
        self.intern(ISD::Register, &[vt], &[], SDNodeFlags::default(), NodeData::PhysReg(reg))
    }

    /// `getCopyToReg` of a physical register with glue: results are the chain
    /// and the glue.
    pub fn get_copy_to_phys_reg(&mut self, chain: SDValue, reg: llrm_lir::registers::RegId, value: SDValue, glue: Option<SDValue>) -> SDValue {
        let reg_node = self.get_phys_register(self.value_type(value), reg);
        let mut ops = vec![chain, reg_node, value];
        ops.extend(glue);
        self.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &ops)
    }

    /// A call's `RegisterMask` operand: the registers the callee preserves
    /// are the allocator's to know, so the node carries none.
    fn symbol_id(&mut self, name: &str) -> u32 {
        match self.symbol_names.iter().position(|one| one == name) {
            Some(at) => at as u32,
            None => {
                self.symbol_names.push(name.to_string());
                self.symbol_names.len() as u32 - 1
            }
        }
    }

    pub fn get_external_symbol(&mut self, vt: EVT, name: &str) -> SDValue {
        let id = self.symbol_id(name);
        self.intern(ISD::ExternalSymbol, &[vt], &[], SDNodeFlags::default(), NodeData::Symbol(id))
    }

    pub fn get_target_external_symbol(&mut self, vt: EVT, name: &str) -> SDValue {
        let id = self.symbol_id(name);
        self.intern(ISD::TargetExternalSymbolNode, &[vt], &[], SDNodeFlags::default(), NodeData::Symbol(id))
    }

    pub fn get_jump_table(&mut self, vt: EVT, index: u32) -> SDValue {
        self.intern(ISD::JumpTable, &[vt], &[], SDNodeFlags::default(), NodeData::JumpTable(index))
    }

    pub fn get_register_mask(&mut self) -> SDValue {
        self.intern(ISD::RegisterMask, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::None)
    }

    pub fn get_target_global_address(&mut self, vt: EVT, symbol: u32, offset: i64) -> SDValue {
        self.intern(ISD::TargetGlobalAddress, &[vt], &[], SDNodeFlags::default(), NodeData::Global { symbol, offset })
    }

    /// `getCopyFromReg` of a physical register with glue: the value, the
    /// chain and the glue.
    pub fn get_copy_from_phys_reg(&mut self, chain: SDValue, reg: llrm_lir::registers::RegId, vt: EVT, glue: SDValue) -> SDValue {
        let reg_node = self.get_phys_register(vt, reg);
        self.get_node_multi(ISD::CopyFromReg, &[vt, EVT::OTHER, EVT::GLUE], &[chain, reg_node, glue])
    }

    /// `getCopyFromReg` of a physical register without glue.
    pub fn get_copy_from_phys_reg_plain(&mut self, chain: SDValue, reg: llrm_lir::registers::RegId, vt: EVT) -> SDValue {
        let reg_node = self.get_phys_register(vt, reg);
        self.get_node_multi(ISD::CopyFromReg, &[vt, EVT::OTHER], &[chain, reg_node])
    }

    /// `getCALLSEQ_START`: `callseq_start chain, bytes, 0` (chain and glue).
    pub fn get_callseq_start(&mut self, chain: SDValue, bytes: u64) -> SDValue {
        let size = self.get_constant_with(u128::from(bytes), EVT::I32, true, false);
        let zero = self.get_constant_with(0, EVT::I32, true, false);
        self.get_node_multi(ISD::CallSeqStart, &[EVT::OTHER, EVT::GLUE], &[chain, size, zero])
    }

    /// `getCALLSEQ_END`: `callseq_end chain, bytes, popped, glue`.
    pub fn get_callseq_end(&mut self, chain: SDValue, bytes: u64, popped: u64, glue: SDValue) -> SDValue {
        let size = self.get_constant_with(u128::from(bytes), EVT::I32, true, false);
        let pop = self.get_constant_with(u128::from(popped), EVT::I32, true, false);
        self.get_node_multi(ISD::CallSeqEnd, &[EVT::OTHER, EVT::GLUE], &[chain, size, pop, glue])
    }

    pub fn get_register(&mut self, vt: EVT, reg: u32) -> SDValue {
        self.intern(ISD::Register, &[vt], &[], SDNodeFlags::default(), NodeData::Register(reg))
    }

    /// `getNOT`: `xor v, -1`.
    pub fn get_not(&mut self, value: SDValue, vt: EVT) -> SDValue {
        let ones = self.get_all_ones_constant(vt);
        self.get_node(ISD::Xor, vt, &[value, ones])
    }

    /// `getNegative`: `sub 0, v`.
    pub fn get_negative(&mut self, value: SDValue, vt: EVT) -> SDValue {
        let zero = self.get_constant(0, vt);
        self.get_node(ISD::Sub, vt, &[zero, value])
    }

    pub fn get_setcc(&mut self, vt: EVT, lhs: SDValue, rhs: SDValue, cond: CondCode) -> SDValue {
        let cc = self.get_cond_code(cond);
        self.get_node(ISD::SetCC, vt, &[lhs, rhs, cc])
    }

    pub fn get_select(&mut self, vt: EVT, cond: SDValue, then: SDValue, other: SDValue) -> SDValue {
        self.get_node(ISD::Select, vt, &[cond, then, other])
    }

    /// `getNode(Opcode, VT, Ops...)` with no flags.
    pub fn get_node(&mut self, opcode: ISD, vt: EVT, ops: &[SDValue]) -> SDValue {
        self.get_node_flags(opcode, vt, ops, SDNodeFlags::default())
    }

    /// `getNode`: the folds of the pin's one-, two- and three-operand forms,
    /// then the CSE lookup. Four or more operands and no folds are the
    /// pin's `ArrayRef` form, which only checks `TokenFactor`-like cases.
    pub fn get_node_flags(&mut self, opcode: ISD, vt: EVT, ops: &[SDValue], flags: SDNodeFlags) -> SDValue {
        let folded = match *ops {
            [n1] => self.fold_unary(opcode, vt, n1, flags),
            [n1, n2] => self.fold_binary(opcode, vt, n1, n2, flags),
            [n1, n2, n3] => self.fold_ternary(opcode, vt, n1, n2, n3),
            _ => Folded::Keep(ops.to_vec()),
        };
        match folded {
            Folded::Done(value) => value,
            Folded::Keep(ops) => self.intern(opcode, &[vt], &ops, flags, NodeData::None),
        }
    }

    /// `getNode(Opcode, VTList, Ops)` for nodes with several results; no folds.
    pub fn get_node_multi(&mut self, opcode: ISD, values: &[EVT], ops: &[SDValue]) -> SDValue {
        self.intern(opcode, values, ops, SDNodeFlags::default(), NodeData::None)
    }
}

/// A `getNode` fold either ends in a value or lets the node be built with
/// the (possibly canonicalized) operands.
pub(super) enum Folded {
    Done(SDValue),
    Keep(Vec<SDValue>),
}
