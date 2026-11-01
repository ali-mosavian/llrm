//! The arena: nodes, values, the CSE map and node construction.

use llrm_support::hash::{HashMap, HashSet};

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
    /// DEVIATE PAIR1 (no pin counterpart: the pin's pointers are one word
    /// wide): a `BUILD_PAIR` of a far pointer's offset and selector, and the
    /// `or` its expansion makes. Exit: none; the pin has no far pointer.
    pub const PAIR_FAR: SDNodeFlags = SDNodeFlags(8192);
    /// DEVIATE PAIR1: a `BUILD_PAIR` of the two words of a value the calling
    /// convention returns or passes in two registers (`dx:ax`).
    pub const PAIR_LONG: SDNodeFlags = SDNodeFlags(16384);

    pub fn union(
        self,
        other: SDNodeFlags,
    ) -> SDNodeFlags {
        SDNodeFlags(self.0 | other.0)
    }

    pub fn with(
        self,
        flag: SDNodeFlags,
        on: bool,
    ) -> SDNodeFlags {
        if on { self.union(flag) } else { self }
    }

    pub fn intersect(
        self,
        other: SDNodeFlags,
    ) -> SDNodeFlags {
        SDNodeFlags(self.0 & other.0)
    }

    pub fn has(
        self,
        flag: SDNodeFlags,
    ) -> bool {
        self.0 & flag.0 == flag.0
    }
}

/// What a leaf node carries besides its opcode and types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeData {
    None,
    /// `ConstantSDNode`: the bits at the node's width, zero above it.
    Const {
        bits: u128,
        opaque: bool,
    },
    /// `ConstantFPSDNode`, as the bits of its type (DEVIATE SD11).
    ConstFp {
        bits: u128,
    },
    CondCode(CondCode),
    ValueType(EVT),
    Register(u32),
    /// A physical register of the target being compiled.
    PhysReg(llrm_lir::registers::RegId),
    FrameIndex(i32),
    Global {
        symbol: u32,
        offset: i64,
    },
    Block(u32),
    Symbol(u32),
    /// A constant pool node's entry, an index into
    /// [`SelectionDAG::constant_pool`].
    /// A constant pool entry and the displacement added to its address
    /// (`ConstantPoolSDNode::getOffset`).
    PoolIndex(u32, i32),
    Mem(MemInfo),
    JumpTable(u32),
    /// `SrcValueSDNode`: which IR value, by a key the builder makes.
    Src(u32),
}

/// What identifies a memory access beyond its operands: the pin's
/// `MachineMemOperand` as far as `CSEMap` and the dump use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MemInfo {
    pub mem_vt: EVT,
    pub align: u32,
    pub volatile: bool,
    pub base: MemBase,
    /// The extension of a load narrower than its result (`LoadExtType`); none
    /// for a plain load.
    pub ext: Option<super::actions::LoadExt>,
    /// `MachinePointerInfo::getAddrSpace`: 0, or the segment-override space
    /// (`fs` 257, `gs` 256) the IR's pointer names.
    pub space: u32,
    /// The ordering of an atomic access
    /// (`MachineMemOperand::getSuccessOrdering`); none for a plain one.
    pub ordering: Option<llrm_mir::opcode::AtomicOrdering>,
}

/// `MachinePointerInfo`'s value: what the access is relative to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MemBase {
    Unknown,
    Global(u32),
    Stack(i32),
    /// An outgoing call argument at this offset from the stack pointer
    /// (`MachinePointerInfo::getStack`, `X86TargetLowering::LowerCall`).
    CallArgument(i32),
    /// A constant pool entry (`MachinePointerInfo::getConstantPool`).
    ConstantPool(u32),
    /// A jump table entry (`MachinePointerInfo::getJumpTable`).
    JumpTable,
}

#[derive(Clone, Debug)]
pub struct SDNode {
    /// `SDNode::getIROrder`: the position of the IR instruction the node was
    /// built for (0 for none); a node a combine or the selector makes takes
    /// the order of the node it replaces (`SDLoc(N)`).
    pub order: u32,
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

/// What a target charges for the operations a divide by a constant is made of
/// or replaced by: the divide's own price, and the multiply-high sequence's
/// parts. `None` in [`DagConfig::div_prices`]: the pin's rule (`int_div_cheap`
/// alone).
#[derive(Clone, Debug)]
pub struct DivPrices {
    /// The target's own operand size in bytes: a value of another width runs
    /// under the prefix.
    pub operand: i64,
    pub prefix: i64,
    pub add: i64,
    pub shift: i64,
    pub extend: i64,
    pub multiply: i64,
    pub multiply_long: i64,
    pub divide: i64,
    pub divide_long: i64,
    /// Percent of the divide's price the multiply-high sequence must exceed it
    /// by: below the spread of the sheet's own prices the pin's sequence
    /// stays.
    pub margin: i64,
    /// The clocks of the product `q * d` a remainder subtracts, by `d`: what
    /// the selector emits for it (`Machine::multiply_by`).
    pub product: Option<ProductPrice>,
}

/// A price by the constant `d`.
#[derive(Clone)]
pub struct ProductPrice(pub std::rc::Rc<dyn Fn(i64) -> i64>);

impl std::fmt::Debug for ProductPrice {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_str("ProductPrice")
    }
}

/// The target facts `getNode` asks (`TLI->` in the pin), passed in rather
/// than read from a global.
#[derive(Clone, Debug)]
pub struct DagConfig {
    pub boolean_contents: BooleanContents,
    /// `isOffsetFoldingLegal`: `(add GlobalAddress, c)` folds into the symbol.
    pub offset_folding: bool,
    /// Every symbol is `dso_local`, as clang marks them under `-fno-pic`: an
    /// `extern` symbol's offset then folds too. The pin takes the flag from
    /// the IR; its absence there is this field's default.
    pub dso_local_symbols: bool,
    /// The pointer type (`TLI->getPointerTy`).
    pub pointer: EVT,
    /// `isIntDivCheap`: a divide by a constant stays a divide.
    pub int_div_cheap: bool,
    /// Prices that decide `isIntDivCheap` per divisor and width; see
    /// [`DivPrices`].
    pub div_prices: Option<DivPrices>,
    /// `TargetLowering::BuildSDIVPow2`'s default under `isIntDivCheap`: an
    /// `sdiv` by a power of two stays an `sdiv`.
    pub sdiv_pow2_stays: bool,
    /// The function evaluates floats at the x87's extended precision
    /// (`FP_EVAL`): a constant operation folds only where its result is exact,
    /// as the IR's folder does.
    pub fp_extended: bool,
    /// `computeKnownBitsForTargetNode`: what the target knows of one of its
    /// nodes, given the known bits of operand `n`; `None` for a node it
    /// says nothing about.
    pub known_bits_for_target_node: Option<TargetKnownBits>,
    /// `DataLayout::isBigEndian`: a narrowed load's offset counts from the
    /// other end.
    pub big_endian: bool,
}

/// See [`DagConfig::known_bits_for_target_node`].
pub type TargetKnownBits =
    fn(&SelectionDAG, SDValue, &dyn Fn(usize) -> super::known_bits::KnownBits) -> Option<super::known_bits::KnownBits>;

impl Default for DagConfig {
    /// x86: 0 or 1; symbol offsets do not fold (`isOffsetFoldingLegal` is false
    /// for the default PIC model).
    fn default() -> DagConfig {
        DagConfig {
            boolean_contents: BooleanContents::ZeroOrOne,
            offset_folding: false,
            dso_local_symbols: false,
            pointer: EVT::I32,
            int_div_cheap: false,
            div_prices: None,
            sdiv_pow2_stays: false,
            fp_extended: false,
            known_bits_for_target_node: None,
            big_endian: false,
        }
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

/// `FoldingSet`: nodes by the hash of (opcode, types, operands, data), chained
/// through `next`; a lookup compares in place, so no key is built or cloned.
#[derive(Default)]
struct Cse {
    head: HashMap<u64, u32>,
    next: Vec<u32>,
}

const NONE: u32 = u32::MAX;

fn cse_hash(
    opcode: ISD,
    values: &[EVT],
    ops: &[SDValue],
    data: &NodeData,
) -> u64 {
    use std::hash::{BuildHasher, Hash, Hasher};
    let mut hasher = llrm_support::hash::FxBuild::default().build_hasher();
    opcode.hash(&mut hasher);
    values.hash(&mut hasher);
    ops.hash(&mut hasher);
    data.hash(&mut hasher);
    hasher.finish()
}

/// `SelectionDAG`: nodes in an arena indexed by [`SDValue::node`].
pub struct SelectionDAG {
    nodes: Vec<SDNode>,
    /// Per node, the nodes that have read one of its results (may hold
    /// stale or repeated entries; `users` filters them).
    uses: Vec<Vec<u32>>,
    /// Whether a node's use list was pushed out of order: a sorted one is
    /// walked in place without checking it again for each query.
    uses_unsorted: Vec<bool>,
    cse: Cse,
    /// Nodes `delete_dead` removed: they stay in the arena but no longer
    /// count as users.
    /// By node id; the arena only grows, so ids index it.
    deleted: Vec<bool>,
    /// `delete_dead`'s worklist, kept between calls: a fresh `vec![node]` per
    /// call was one allocation each (51K on jpeg_fixed -O2).
    dead_work: Vec<u32>,
    /// The `IROrder` of the `SDLoc` in force: nodes made now carry it.
    pub cur_order: u32,
    /// The source line of the IR instruction at each `IROrder` (0: none), from
    /// the `!dbg` the builder read: `SDLoc`'s line.
    pub lines: Vec<Option<crate::model::lir::SrcLoc>>,
    /// The IR instruction each `IROrder` was made for, in order: what a
    /// debug record's `before` names.
    pub inst_order: Vec<(llrm_mir::module::InstId, u32)>,

    /// The (low, high) halves type legalization expanded each integer value
    /// into (`ExpandedIntegers`): where a debug record's value went.
    pub expanded_integers: llrm_support::hash::HashMap<SDValue, (SDValue, SDValue)>,
    /// Each value `replace_all_uses_with` replaced, by what replaced it: where
    /// a debug record's value went (`transferDbgValues`).
    forwarded: llrm_support::hash::HashMap<SDValue, SDValue>,
    /// `CombineTo` calls a target hook made: the combiner replaces each and
    /// looks at the users again.
    replacements: Vec<(SDValue, SDValue)>,
    pub(super) config: DagConfig,
    entry: SDValue,
    root: SDValue,
    /// Names the dump prints for `Global`/`Block` payloads, by id.
    pub global_names: std::rc::Rc<Vec<String>>,
    pub block_names: std::rc::Rc<Vec<String>>,
    /// The names of the blocks the builder splits off, from `block_names.len()`
    /// on (the shared table is not copied for them).
    pub synthetic_names: Vec<String>,
    /// The name of a split-off block nothing named: the block the builder
    /// started from.
    pub synthetic_fill: String,
    /// Names of external symbols, by id.
    pub symbol_names: Vec<String>,
    /// The constants spilled to the pool: bits and type
    /// (`MachineConstantPool`).
    pub constant_pool: Vec<(u128, EVT)>,
    /// The frame index the first stack temporary gets: the driver sets it past
    /// the builder's frame objects.
    pub first_temporary: i32,
    /// The frame index of the first variadic argument (`VarArgsFrameIndex`),
    /// set by the builder where the function takes `...`.
    pub var_args_frame_index: Option<i32>,
    /// The SysV x86-64 `va_list`'s other fields: the register save area's
    /// frame index, `gp_offset` (the bytes of it the named integer arguments
    /// took) and `fp_offset` (48 plus 16 a named vector argument).
    pub var_args_save: Option<(i32, u32, u32)>,
    /// The stack temporaries the legalizer asked for: size and alignment.
    pub temporaries: Vec<(u32, u32)>,
    /// Which globals (by symbol) are local to the object: only those fold a
    /// constant offset into their address (`isOffsetFoldingLegal`).
    pub local_symbols: std::rc::Rc<Vec<bool>>,
    /// The loads of memory nothing writes (`MachineMemOperand::isInvariant`,
    /// which `visitLoad` sets); a node keeps its number when selection morphs
    /// it, so the set follows it into a machine node. A load a combine or the
    /// legalizer remakes is not in it.
    pub invariant_loads: HashSet<u32>,
    /// The offset from the incoming stack pointer of each fixed object (the
    /// negative frame indices the builder makes for stack arguments):
    /// `MachineFrameInfo::getObjectOffset`.
    pub fixed_offsets: std::collections::BTreeMap<i32, i64>,
    /// The loads and stores the program marks volatile: the LIR instructions
    /// they become keep the mark (`MachineMemOperand::isVolatile`).
    pub volatile_accesses: HashSet<u32>,
}

impl SelectionDAG {
    /// The name of `block`.
    pub fn block_name(
        &self,
        block: u32,
    ) -> &str {
        let at = block as usize;
        let base = self.block_names.len();
        if at < base {
            &self.block_names[at]
        } else {
            self.synthetic_names.get(at - base).map_or(self.synthetic_fill.as_str(), String::as_str)
        }
    }

    pub fn set_block_name(
        &mut self,
        block: u32,
        name: String,
    ) {
        let (at, base) = (block as usize, self.block_names.len());
        if at < base {
            std::rc::Rc::make_mut(&mut self.block_names)[at] = name;
            return;
        }
        if self.synthetic_names.len() <= at - base {
            self.synthetic_names.resize(at - base + 1, self.synthetic_fill.clone());
        }
        self.synthetic_names[at - base] = name;
    }

    pub fn new(config: DagConfig) -> SelectionDAG {
        let zero = SDValue { node: 0, res: 0 };
        let mut dag = SelectionDAG {
            nodes: Vec::new(),
            uses: Vec::new(),
            uses_unsorted: Vec::new(),
            cse: Cse::default(),
            deleted: Vec::new(),
            dead_work: Vec::new(),
            cur_order: 0,
            lines: Vec::new(),
            inst_order: Vec::new(),
            expanded_integers: Default::default(),
            forwarded: Default::default(),
            replacements: Vec::new(),
            config,
            entry: zero,
            root: zero,
            global_names: Default::default(),
            block_names: Default::default(),
            synthetic_names: Vec::new(),
            synthetic_fill: String::new(),
            symbol_names: Vec::new(),
            constant_pool: Vec::new(),
            first_temporary: 0,
            var_args_frame_index: None,
            var_args_save: None,
            temporaries: Vec::new(),
            local_symbols: Default::default(),
            invariant_loads: HashSet::default(),
            fixed_offsets: Default::default(),
            volatile_accesses: HashSet::default(),
        };
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

    pub fn set_root(
        &mut self,
        root: SDValue,
    ) {
        self.root = root;
    }

    pub fn config(&self) -> &DagConfig {
        &self.config
    }

    /// How many results of other nodes read `value`'s node (`use_size`).
    pub fn use_count(
        &self,
        node: u32,
    ) -> usize {
        self.live_users(node)
            .map(|user| self.nodes[user as usize].ops.iter().filter(|op| op.node == node).count())
            .sum()
    }

    /// `use_count(node)` or `cap`, whichever is less: it stops reading the
    /// use list there (`hasNUsesOrMore`/`hasOneUse` walk two entries, not all).
    pub fn use_count_up_to(
        &self,
        node: u32,
        cap: usize,
    ) -> usize {
        let mut seen = 0;
        for user in self.live_users(node) {
            seen += self.nodes[user as usize].ops.iter().filter(|op| op.node == node).count();
            if seen >= cap {
                return cap;
            }
        }
        seen
    }

    /// `SDNode::hasOneUse`.
    pub fn has_one_use(
        &self,
        node: u32,
    ) -> bool {
        self.use_count_up_to(node, 2) == 1
    }

    /// The live nodes that read any result of `node`, in creation order, once
    /// each; the use list is usually sorted already and is then walked in
    /// place.
    fn live_users(
        &self,
        node: u32,
    ) -> LiveUsers<'_> {
        let list = self.uses.get(node as usize).map_or(&[][..], Vec::as_slice);
        let inner = if !self.uses_unsorted.get(node as usize).copied().unwrap_or(false) {
            UserList::Sorted(list.iter())
        } else {
            #[cfg(test)]
            USERS_SORTED.with(|n| n.set(n.get() + list.len()));
            let mut sorted = list.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            UserList::Owned(sorted.into_iter())
        };
        LiveUsers { dag: self, node, inner, last: None }
    }

    /// `recursivelyDeleteUnusedNodes`: a node nothing reads (and that is
    /// neither the root nor the entry) lets go of its operands, and so on down.
    /// The arena keeps the slot; what matters is that a dead node no longer
    /// counts as a user (`hasOneUse`), nor as a CSE hit.
    pub fn delete_dead(
        &mut self,
        node: u32,
    ) {
        let mut work = std::mem::take(&mut self.dead_work);
        work.push(node);
        while let Some(n) = work.pop() {
            // Chain and glue nodes stay: the builder keeps handles on some
            // (the pin protects them with `HandleSDNode`).
            if n == self.root.node
                || n == self.entry.node
                || self.is_deleted(n)
                || self.nodes[n as usize].ops.is_empty()
                || self.nodes[n as usize].values.iter().any(|&vt| vt == EVT::OTHER || vt == EVT::GLUE)
            {
                continue;
            }
            let results = self.nodes[n as usize].values.len() as u8;
            if (0..results).any(|res| self.has_users(SDValue { node: n, res })) {
                continue;
            }
            self.cse_forget(n);
            if self.deleted.len() <= n as usize {
                self.deleted.resize(self.nodes.len(), false);
            }
            self.deleted[n as usize] = true;
            work.extend(self.nodes[n as usize].ops.iter().map(|op| op.node));
        }
        self.dead_work = work;
    }

    /// A load or store a combine replaced (`CombineTo`): the pin removes it at
    /// once, and what it read no longer counts it among its users.
    /// `delete_dead` keeps chain nodes, so a replaced store stayed a reader
    /// of its value and an `add` the store was the only user of looked
    /// shared (a second store of it made `IsFoldableRMW` false and the
    /// `i16` read-modify-write was promoted to `i32`).
    pub fn delete_replaced(
        &mut self,
        node: u32,
    ) {
        if node == self.root.node
            || node == self.entry.node
            || self.is_deleted(node)
            || !matches!(
                self.nodes[node as usize].opcode,
                ISD::Load | ISD::Store | ISD::BrCond
            )
        {
            return;
        }
        let results = self.nodes[node as usize].values.len() as u8;
        if (0..results).any(|res| self.has_users(SDValue { node, res })) {
            return;
        }
        self.cse_forget(node);
        if self.deleted.len() <= node as usize {
            self.deleted.resize(self.nodes.len(), false);
        }
        self.deleted[node as usize] = true;
        let operands: Vec<u32> = self.nodes[node as usize].ops.iter().map(|op| op.node).collect();
        for op in operands {
            self.delete_dead(op);
        }
    }

    /// Whether `delete_dead` removed `node`: nothing reads it, and it reads
    /// nothing that counts.
    pub fn is_deleted(
        &self,
        node: u32,
    ) -> bool {
        self.deleted.get(node as usize).copied().unwrap_or(false)
    }

    pub fn nodes(&self) -> &[SDNode] {
        &self.nodes
    }

    /// Every node id in creation order (`allnodes()`).
    pub fn node_ids(&self) -> std::ops::Range<u32> {
        0..self.nodes.len() as u32
    }

    /// The nodes that may read a result of `node` (its use list: stale and
    /// repeated entries included), for a caller that checks the operands.
    pub fn use_candidates(
        &self,
        node: u32,
    ) -> &[u32] {
        self.uses.get(node as usize).map_or(&[], Vec::as_slice)
    }

    /// The nodes that read `value` as an operand, in creation order, once
    /// each (`SDValue::users()`).
    pub fn users(
        &self,
        value: SDValue,
    ) -> impl Iterator<Item = u32> + '_ {
        self.live_users(value.node).filter(move |&user| self.nodes[user as usize].ops.contains(&value))
    }

    /// Whether anything reads `value`: `users(value).next().is_some()`
    /// without collecting and sorting them.
    pub fn has_users(
        &self,
        value: SDValue,
    ) -> bool {
        self.uses
            .get(value.node as usize)
            .into_iter()
            .flatten()
            .any(|&user| !self.is_deleted(user) && self.nodes[user as usize].ops.contains(&value))
    }

    fn note_uses(
        &mut self,
        user: u32,
    ) {
        for at in 0..self.nodes[user as usize].ops.len() {
            let operand = self.nodes[user as usize].ops[at].node as usize;
            self.push_use(operand, user);
        }
    }

    fn push_use(
        &mut self,
        node: usize,
        user: u32,
    ) {
        let list = &mut self.uses[node];
        if list.last().is_some_and(|&last| last > user) {
            self.uses_unsorted[node] = true;
        }
        list.push(user);
    }

    /// The hash `node` is filed under; none for a node with glue.
    fn key_of(
        &self,
        node: u32,
    ) -> Option<u64> {
        let data = &self.nodes[node as usize];
        (!data.values.contains(&EVT::GLUE)).then(|| cse_hash(data.opcode, &data.values, &data.ops, &data.data))
    }

    /// The filed node equal to (opcode, types, operands, data).
    fn cse_find(
        &self,
        opcode: ISD,
        values: &[EVT],
        ops: &[SDValue],
        data: &NodeData,
    ) -> Option<u32> {
        let mut at = *self.cse.head.get(&cse_hash(opcode, values, ops, data))?;
        while at != NONE {
            let node = &self.nodes[at as usize];
            if node.opcode == opcode && node.values == values && node.ops == ops && node.data == *data {
                return Some(at);
            }
            at = self.cse.next[at as usize];
        }
        None
    }

    fn cse_link(
        &mut self,
        hash: u64,
        node: u32,
    ) {
        if self.cse.next.len() <= node as usize {
            self.cse.next.resize(self.nodes.len().max(node as usize + 1), NONE);
        }
        let mut at = self.cse.head.get(&hash).copied().unwrap_or(NONE);
        while at != NONE {
            if at == node {
                return;
            }
            at = self.cse.next[at as usize];
        }
        self.cse.next[node as usize] = self.cse.head.insert(hash, node).unwrap_or(NONE);
    }

    /// Unfiles `node` from the chain of `hash`, if it is filed there.
    fn cse_unlink(
        &mut self,
        hash: u64,
        node: u32,
    ) {
        let Some(&first) = self.cse.head.get(&hash) else { return };
        if first == node {
            match self.cse.next[node as usize] {
                NONE => self.cse.head.remove(&hash),
                after => self.cse.head.insert(hash, after),
            };
            return;
        }
        let mut at = first;
        while at != NONE {
            let after = self.cse.next[at as usize];
            if after == node {
                self.cse.next[at as usize] = self.cse.next[node as usize];
                return;
            }
            at = after;
        }
    }

    /// The filed node equal to `node`'s content.
    fn cse_equal(
        &self,
        node: u32,
    ) -> Option<u32> {
        let data = &self.nodes[node as usize];
        self.cse_find(data.opcode, &data.values, &data.ops, &data.data)
    }

    /// Unfiles `node` under its current content.
    fn cse_forget(
        &mut self,
        node: u32,
    ) {
        if let Some(hash) = self.key_of(node) {
            self.cse_unlink(hash, node);
        }
    }

    /// `getNodeIfExists(Opcode, VTList, Ops)`: the filed node, if one is.
    pub fn get_node_if_exists(
        &self,
        opcode: ISD,
        values: &[EVT],
        ops: &[SDValue],
    ) -> Option<u32> {
        self.cse_find(opcode, values, ops, &NodeData::None)
    }

    /// `DAGCombinerInfo::CombineTo` from a target hook: the users of `from`
    /// read `to` once the hook returns.
    pub fn combine_to(
        &mut self,
        from: SDValue,
        to: SDValue,
    ) {
        self.replacements.push((from, to));
    }

    pub(super) fn take_replacements(&mut self) -> Vec<(SDValue, SDValue)> {
        std::mem::take(&mut self.replacements)
    }

    /// `value` as the nodes built from it left it: followed through every
    /// replacement.
    pub fn resolve(
        &self,
        mut value: SDValue,
    ) -> SDValue {
        // The chain has no cycle: a value is replaced by a later one.
        for _ in 0..self.forwarded.len() + 1 {
            match self.forwarded.get(&value) {
                Some(&next) if next != value => value = next,
                _ => break,
            }
        }
        value
    }

    /// `ReplaceAllUsesWith(From, To)`: every operand `from` becomes `to`. A
    /// user that then equals an existing node is replaced by it in turn
    /// (`AddModifiedNodeToCSEMaps`); the arena only grows, so replaced nodes
    /// stay in it with no users. The root follows.
    pub fn replace_all_uses_with(
        &mut self,
        from: SDValue,
        to: SDValue,
    ) -> Vec<Rauw> {
        let mut events = Vec::new();
        if from == to {
            return events;
        }
        let mut work = vec![(from, to)];
        while let Some((from, to)) = work.pop() {
            self.forwarded.insert(from, to);
            if self.root == from {
                self.root = to;
            }
            for user in self.users(from).collect::<Vec<_>>() {
                self.cse_forget(user);
                for op in &mut self.nodes[user as usize].ops {
                    if *op == from {
                        *op = to;
                    }
                }
                self.push_use(to.node as usize, user);
                let Some(hash) = self.key_of(user) else {
                    events.push(Rauw::Updated(user));
                    continue;
                };
                match self.cse_equal(user) {
                    Some(existing) if existing != user => {
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
                        self.cse_link(hash, user);
                    }
                }
            }
        }
        events
    }

    /// `UpdateNodeOperands`: `node` reads `ops` instead. If that equals an
    /// existing node, that node is returned (and `node` stays as it was,
    /// unused); else `node` itself, re-entered in the CSE map.
    pub fn update_node_operands(
        &mut self,
        node: u32,
        ops: &[SDValue],
    ) -> u32 {
        self.cse_forget(node);
        let old = std::mem::replace(&mut self.nodes[node as usize].ops, ops.to_vec());
        self.note_uses(node);
        match self.key_of(node) {
            Some(hash) => match self.cse_equal(node) {
                Some(existing) if existing != node => {
                    self.nodes[node as usize].ops = old;
                    if let Some(hash) = self.key_of(node) {
                        self.cse_link(hash, node);
                    }
                    existing
                }
                _ => {
                    self.cse_link(hash, node);
                    node
                }
            },
            None => node,
        }
    }

    /// `MorphNodeTo`: `node` becomes `opcode` over `values` and `ops` in place,
    /// leaving the CSE map consistent. When an equal node exists it is
    /// returned instead and `node` is untouched (the caller then replaces
    /// it, as `SelectionDAGISel::MorphNode` does).
    pub(super) fn morph_node_to(
        &mut self,
        node: u32,
        opcode: ISD,
        values: &[EVT],
        ops: &[SDValue],
    ) -> u32 {
        let shareable = !values.contains(&EVT::GLUE);
        if shareable {
            if let Some(found) = self.cse_find(opcode, values, ops, &NodeData::None) {
                if found != node {
                    return found;
                }
            }
        }
        self.cse_forget(node);
        let data = &mut self.nodes[node as usize];
        data.opcode = opcode;
        data.values = values.to_vec();
        data.ops = ops.to_vec();
        data.data = NodeData::None;
        if shareable {
            self.cse_link(cse_hash(opcode, values, ops, &NodeData::None), node);
        }
        // The morphed node reads its new operands: a later `ReplaceAllUsesWith`
        // of one of them must reach it (the SDIVREM of a remainder used twice
        // left the second user, a folded address, reading the unselected node).
        self.note_uses(node);
        node
    }

    /// Drops `node` from the CSE map, so a later request builds a new node
    /// rather than reuse it.
    pub(super) fn forget_node(
        &mut self,
        node: u32,
    ) {
        self.cse_forget(node);
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn node(
        &self,
        value: SDValue,
    ) -> &SDNode {
        &self.nodes[value.node as usize]
    }

    pub fn opcode(
        &self,
        value: SDValue,
    ) -> ISD {
        self.node(value).opcode
    }

    pub fn value_type(
        &self,
        value: SDValue,
    ) -> EVT {
        self.node(value).values[value.res as usize]
    }

    pub fn operand(
        &self,
        value: SDValue,
        index: usize,
    ) -> SDValue {
        self.node(value).ops[index]
    }

    pub fn is_undef(
        &self,
        value: SDValue,
    ) -> bool {
        matches!(self.opcode(value), ISD::Undef | ISD::Poison)
    }

    /// The value of a `ConstantSDNode` (`Constant` or `TargetConstant`).
    pub fn constant(
        &self,
        value: SDValue,
    ) -> Option<u128> {
        match (self.opcode(value), self.node(value).data) {
            (ISD::Constant | ISD::TargetConstant, NodeData::Const { bits, .. }) => Some(bits),
            _ => None,
        }
    }

    pub fn is_opaque(
        &self,
        value: SDValue,
    ) -> bool {
        matches!(self.node(value).data, NodeData::Const { opaque: true, .. })
    }

    pub fn is_null_constant(
        &self,
        value: SDValue,
    ) -> bool {
        self.constant(value) == Some(0)
    }

    pub(super) fn width(
        &self,
        value: SDValue,
    ) -> u32 {
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
        if shareable {
            if let Some(found) = self.cse_find(opcode, values, ops, &data) {
                let node = &mut self.nodes[found as usize];
                node.flags = node.flags.intersect(flags);
                return SDValue { node: found, res: 0 };
            }
        }
        let index = self.nodes.len() as u32;
        self.nodes.push(SDNode {
            order: if has_no_ir_order(opcode) { 0 } else { self.cur_order },
            opcode,
            values: values.to_vec(),
            ops: ops.to_vec(),
            flags,
            data,
        });
        self.uses.push(Vec::new());
        self.uses_unsorted.push(false);
        self.note_uses(index);
        if shareable {
            self.cse_link(cse_hash(opcode, values, ops, &data), index);
        }
        SDValue { node: index, res: 0 }
    }

    pub fn get_undef(
        &mut self,
        vt: EVT,
    ) -> SDValue {
        self.intern(ISD::Undef, &[vt], &[], SDNodeFlags::default(), NodeData::None)
    }

    pub fn get_poison(
        &mut self,
        vt: EVT,
    ) -> SDValue {
        self.intern(ISD::Poison, &[vt], &[], SDNodeFlags::default(), NodeData::None)
    }

    pub fn get_constant(
        &mut self,
        bits: u128,
        vt: EVT,
    ) -> SDValue {
        self.get_constant_with(bits, vt, false, false)
    }

    /// `getConstant` with `isT` (a `TargetConstant`) and `isO` (opaque).
    pub fn get_constant_with(
        &mut self,
        bits: u128,
        vt: EVT,
        target: bool,
        opaque: bool,
    ) -> SDValue {
        assert!(vt.is_integer(), "integer constant of {vt:?}");
        let bits = bits & super::fold::mask(vt.size_in_bits());
        let opcode = if target { ISD::TargetConstant } else { ISD::Constant };
        self.intern(opcode, &[vt], &[], SDNodeFlags::default(), NodeData::Const { bits, opaque })
    }

    /// `getConstantFP` by the bits of the type (DEVIATE SD11: no `APFloat`).
    pub fn get_constant_fp(
        &mut self,
        bits: u128,
        vt: EVT,
    ) -> SDValue {
        assert!(vt.is_floating_point(), "FP constant of {vt:?}");
        self.intern(ISD::ConstantFP, &[vt], &[], SDNodeFlags::default(), NodeData::ConstFp { bits })
    }

    /// `getTargetConstantFP`: a float constant a pattern takes as an immediate.
    pub fn get_target_constant_fp(
        &mut self,
        bits: u128,
        vt: EVT,
    ) -> SDValue {
        self.intern(ISD::TargetConstantFP, &[vt], &[], SDNodeFlags::default(), NodeData::ConstFp { bits })
    }

    pub fn get_all_ones_constant(
        &mut self,
        vt: EVT,
    ) -> SDValue {
        self.get_constant(u128::MAX, vt)
    }

    /// `getBoolConstant`: true is 1, or all ones for `ZeroOrNegativeOne`.
    pub fn get_bool_constant(
        &mut self,
        value: bool,
        vt: EVT,
    ) -> SDValue {
        if !value {
            return self.get_constant(0, vt);
        }
        match self.config.boolean_contents {
            BooleanContents::ZeroOrOne | BooleanContents::Undefined => self.get_constant(1, vt),
            BooleanContents::ZeroOrNegativeOne => self.get_all_ones_constant(vt),
        }
    }

    /// `isBoolConstant`.
    pub fn bool_constant(
        &self,
        value: SDValue,
    ) -> Option<bool> {
        let bits = self.constant(value)?;
        let all_ones = super::fold::mask(self.width(value));
        match self.config.boolean_contents {
            BooleanContents::ZeroOrOne => (bits <= 1).then_some(bits == 1),
            BooleanContents::ZeroOrNegativeOne => (bits == 0 || bits == all_ones).then_some(bits != 0),
            BooleanContents::Undefined => Some(bits != 0),
        }
    }

    pub fn get_basic_block(
        &mut self,
        block: u32,
    ) -> SDValue {
        self.intern(ISD::BasicBlock, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::Block(block))
    }

    pub fn get_target_frame_index(
        &mut self,
        vt: EVT,
        index: i32,
    ) -> SDValue {
        self.intern(ISD::TargetFrameIndex, &[vt], &[], SDNodeFlags::default(), NodeData::FrameIndex(index))
    }

    pub fn get_frame_index(
        &mut self,
        vt: EVT,
        index: i32,
    ) -> SDValue {
        self.intern(ISD::FrameIndex, &[vt], &[], SDNodeFlags::default(), NodeData::FrameIndex(index))
    }

    /// `getTokenFactor`.
    pub fn get_token_factor(
        &mut self,
        chains: &[SDValue],
    ) -> SDValue {
        self.get_node(ISD::TokenFactor, EVT::OTHER, chains)
    }

    /// `getCopyToReg` without glue.
    pub fn get_copy_to_reg(
        &mut self,
        chain: SDValue,
        reg: u32,
        value: SDValue,
    ) -> SDValue {
        let reg_node = self.get_register(self.value_type(value), reg);
        self.get_node(ISD::CopyToReg, EVT::OTHER, &[chain, reg_node, value])
    }

    /// `getCopyFromReg`: the value is result 0, the chain result 1.
    pub fn get_copy_from_reg(
        &mut self,
        chain: SDValue,
        reg: u32,
        vt: EVT,
    ) -> SDValue {
        let reg_node = self.get_register(vt, reg);
        self.get_node_multi(ISD::CopyFromReg, &[vt, EVT::OTHER], &[chain, reg_node])
    }

    /// `getLoad` (unindexed, non-extending): `load chain, ptr, poison`.
    pub fn get_load(
        &mut self,
        vt: EVT,
        chain: SDValue,
        ptr: SDValue,
        info: MemInfo,
    ) -> SDValue {
        let offset = self.get_poison(self.value_type(ptr));
        let load = self.intern(
            ISD::Load,
            &[vt, EVT::OTHER],
            &[chain, ptr, offset],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        );
        if info.volatile {
            self.volatile_accesses.insert(load.node);
        }
        load
    }

    /// `getExtLoad`: a load of `info.mem_vt` extended to `vt`.
    pub fn get_ext_load(
        &mut self,
        ext: super::actions::LoadExt,
        vt: EVT,
        chain: SDValue,
        ptr: SDValue,
        info: MemInfo,
    ) -> SDValue {
        assert!(info.mem_vt.bits_lt(vt), "an extending load widens");
        let offset = self.get_poison(self.value_type(ptr));
        let info = MemInfo { ext: Some(ext), ..info };
        let load = self.intern(
            ISD::Load,
            &[vt, EVT::OTHER],
            &[chain, ptr, offset],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        );
        if info.volatile {
            self.volatile_accesses.insert(load.node);
        }
        load
    }

    /// `getStore` (unindexed, not truncating): `store chain, value, ptr,
    /// poison`.
    pub fn get_store(
        &mut self,
        chain: SDValue,
        value: SDValue,
        ptr: SDValue,
        info: MemInfo,
    ) -> SDValue {
        let offset = self.get_poison(self.value_type(ptr));
        let store = self.intern(
            ISD::Store,
            &[EVT::OTHER],
            &[chain, value, ptr, offset],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        );
        if info.volatile {
            self.volatile_accesses.insert(store.node);
        }
        store
    }

    /// `getIndexedLoad`: `load` as `PRE_INC` or `POST_INC` of `offset` on
    /// `base`. The values are the loaded value, the new base and the chain.
    pub fn get_indexed_load(
        &mut self,
        load: SDValue,
        base: SDValue,
        offset: SDValue,
        post: bool,
    ) -> SDValue {
        let NodeData::Mem(info) = self.node(load).data else { panic!("an indexed load of a load") };
        let vt = self.value_type(load);
        let chain = self.operand(load, 0);
        let result = self.intern(
            if post { ISD::LoadPost } else { ISD::LoadPre },
            &[vt, self.value_type(base), EVT::OTHER],
            &[chain, base, offset],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        );
        if info.volatile {
            self.volatile_accesses.insert(result.node);
        }
        result
    }

    /// `getIndexedStore`: `store` as `PRE_INC` or `POST_INC` of `offset` on
    /// `base`. The values are the new base and the chain.
    pub fn get_indexed_store(
        &mut self,
        store: SDValue,
        base: SDValue,
        offset: SDValue,
        post: bool,
    ) -> SDValue {
        let NodeData::Mem(info) = self.node(store).data else { panic!("an indexed store of a store") };
        let (chain, value) = (self.operand(store, 0), self.operand(store, 1));
        let result = self.intern(
            if post { ISD::StorePost } else { ISD::StorePre },
            &[self.value_type(base), EVT::OTHER],
            &[chain, value, base, offset],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        );
        if info.volatile {
            self.volatile_accesses.insert(result.node);
        }
        result
    }

    /// `getAtomic` of `ATOMIC_SWAP`: the old value at `ptr` and a chain,
    /// `value` stored.
    pub fn get_atomic_swap(
        &mut self,
        chain: SDValue,
        ptr: SDValue,
        value: SDValue,
        info: MemInfo,
    ) -> SDValue {
        let vt = self.value_type(value);
        let swap = self.intern(
            ISD::AtomicSwap,
            &[vt, EVT::OTHER],
            &[chain, ptr, value],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        );
        self.volatile_accesses.insert(swap.node);
        swap
    }

    /// `getAtomic` of an `ATOMIC_LOAD_*` read-modify-write: the old value at
    /// `ptr` and a chain, `value` combined into the memory by `op`.
    pub fn get_atomic_rmw(
        &mut self,
        op: ISD,
        chain: SDValue,
        ptr: SDValue,
        value: SDValue,
        info: MemInfo,
    ) -> SDValue {
        debug_assert!(matches!(
            op,
            ISD::AtomicLoadAdd
                | ISD::AtomicLoadSub
                | ISD::AtomicLoadAnd
                | ISD::AtomicLoadClr
                | ISD::AtomicLoadOr
                | ISD::AtomicLoadXor
        ));
        let vt = self.value_type(value);
        let node =
            self.intern(op, &[vt, EVT::OTHER], &[chain, ptr, value], SDNodeFlags::default(), NodeData::Mem(info));
        self.volatile_accesses.insert(node.node);
        node
    }

    /// `getAtomic` of `ATOMIC_CMP_SWAP_WITH_SUCCESS`, or of `ATOMIC_CMP_SWAP`
    /// when `flag` is none: the old value at `ptr`, the success flag of
    /// type `flag` if asked for, and a chain.
    pub fn get_atomic_cmp_swap(
        &mut self,
        flag: Option<EVT>,
        chain: SDValue,
        ptr: SDValue,
        expected: SDValue,
        new: SDValue,
        info: MemInfo,
    ) -> SDValue {
        let vt = self.value_type(expected);
        let (op, types) = match flag {
            Some(flag) => (ISD::AtomicCmpSwapWithSuccess, vec![vt, flag, EVT::OTHER]),
            None => (ISD::AtomicCmpSwap, vec![vt, EVT::OTHER]),
        };
        let node = self.intern(op, &types, &[chain, ptr, expected, new], SDNodeFlags::default(), NodeData::Mem(info));
        self.volatile_accesses.insert(node.node);
        node
    }

    /// `ATOMIC_FENCE`: only a chain. The ordering (LLVM's numbering) and the
    /// scope are target constants, as `SelectionDAGBuilder::visitFence`
    /// builds them.
    pub fn get_atomic_fence(
        &mut self,
        chain: SDValue,
        ordering: u8,
        single_thread: bool,
    ) -> SDValue {
        let ordering = self.get_constant_with(u128::from(ordering), EVT::I64, true, false);
        let scope = self.get_constant_with(u128::from(!single_thread), EVT::I64, true, false);
        self.intern(ISD::AtomicFence, &[EVT::OTHER], &[chain, ordering, scope], SDNodeFlags::default(), NodeData::None)
    }

    /// `getAtomic` of `ATOMIC_LOAD`: the value at `ptr` and a chain.
    pub fn get_atomic_load(
        &mut self,
        vt: EVT,
        chain: SDValue,
        ptr: SDValue,
        info: MemInfo,
    ) -> SDValue {
        let node =
            self.intern(ISD::AtomicLoad, &[vt, EVT::OTHER], &[chain, ptr], SDNodeFlags::default(), NodeData::Mem(info));
        self.volatile_accesses.insert(node.node);
        node
    }

    /// `getAtomic` of `ATOMIC_STORE`: only a chain. The operands are `(chain,
    /// value, ptr)`, as `SelectionDAGBuilder::visitAtomicStore` builds
    /// them.
    pub fn get_atomic_store(
        &mut self,
        chain: SDValue,
        ptr: SDValue,
        value: SDValue,
        info: MemInfo,
    ) -> SDValue {
        let node = self.intern(
            ISD::AtomicStore,
            &[EVT::OTHER],
            &[chain, value, ptr],
            SDNodeFlags::default(),
            NodeData::Mem(info),
        );
        self.volatile_accesses.insert(node.node);
        node
    }

    /// `getMemBasePlusOffset`: `add ptr, offset` (`PTRADD` only where the
    /// target asks to preserve pointer arithmetic, and ours does not).
    pub fn get_mem_base_plus_offset(
        &mut self,
        ptr: SDValue,
        offset: SDValue,
        flags: SDNodeFlags,
    ) -> SDValue {
        let vt = self.value_type(ptr);
        let flags = SDNodeFlags(flags.0 & !SDNodeFlags::IN_BOUNDS.0);
        self.get_node_flags(ISD::Add, vt, &[ptr, offset], flags)
    }

    /// `getObjectPtrOffset` with a constant offset; the constant node exists
    /// even when it is zero and the add folds away, as in the pin.
    pub fn get_object_ptr_offset(
        &mut self,
        ptr: SDValue,
        offset: i64,
    ) -> SDValue {
        let vt = self.value_type(ptr);
        // a two-word pointer is not an integer: its zero offset is no add,
        // and any other moves the offset word alone
        if offset == 0 && !vt.is_integer() {
            return ptr;
        }
        if vt == EVT::Simple(super::mvt::MVT::Far) {
            let (zero, one) = (self.get_constant(0, EVT::I16), self.get_constant(1, EVT::I16));
            let low = self.get_node(ISD::ExtractElement, EVT::I16, &[ptr, zero]);
            let high = self.get_node(ISD::ExtractElement, EVT::I16, &[ptr, one]);
            let step = self.get_constant(offset as u128 & 0xffff, EVT::I16);
            let moved = self.get_node_flags(
                ISD::Add,
                EVT::I16,
                &[low, step],
                SDNodeFlags::NO_UNSIGNED_WRAP.union(SDNodeFlags::IN_BOUNDS),
            );
            return self.get_node_flags(ISD::BuildPair, vt, &[moved, high], SDNodeFlags::PAIR_FAR);
        }
        let index = self.get_constant(offset as u128, vt);
        self.get_mem_base_plus_offset(ptr, index, SDNodeFlags::NO_UNSIGNED_WRAP.union(SDNodeFlags::IN_BOUNDS))
    }

    pub fn get_zext_or_trunc(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        let from = self.value_type(value);
        if from == vt {
            value
        } else if from.bits_gt(vt) {
            self.get_node(ISD::Truncate, vt, &[value])
        } else {
            self.get_node(ISD::ZeroExtend, vt, &[value])
        }
    }

    pub fn get_any_ext_or_trunc(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        let from = self.value_type(value);
        if from == vt {
            value
        } else if from.bits_gt(vt) {
            self.get_node(ISD::Truncate, vt, &[value])
        } else {
            self.get_node(ISD::AnyExtend, vt, &[value])
        }
    }

    pub fn get_sext_or_trunc(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        let from = self.value_type(value);
        if from == vt {
            value
        } else if from.bits_gt(vt) {
            self.get_node(ISD::Truncate, vt, &[value])
        } else {
            self.get_node(ISD::SignExtend, vt, &[value])
        }
    }

    pub fn get_cond_code(
        &mut self,
        cond: CondCode,
    ) -> SDValue {
        self.intern(ISD::CondCodeNode, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::CondCode(cond))
    }

    pub fn get_value_type_node(
        &mut self,
        vt: EVT,
    ) -> SDValue {
        self.intern(ISD::ValueType, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::ValueType(vt))
    }

    /// `getConstantPool`: the node for an entry holding `bits` of type `vt`
    /// (equal entries share one index), as a pointer of type `ptr`.
    pub fn get_constant_pool(
        &mut self,
        ptr: EVT,
        bits: u128,
        vt: EVT,
        target: bool,
    ) -> SDValue {
        let index = match self.constant_pool.iter().position(|&entry| entry == (bits, vt)) {
            Some(at) => at,
            None => {
                self.constant_pool.push((bits, vt));
                self.constant_pool.len() - 1
            }
        } as u32;
        let opcode = if target { ISD::TargetConstantPool } else { ISD::ConstantPool };
        self.intern(opcode, &[ptr], &[], SDNodeFlags::default(), NodeData::PoolIndex(index, 0))
    }

    /// `CreateStackObject`: a frame object for a lowering's scratch value; its
    /// frame index (the driver turns the indices from
    /// [`SelectionDAG::first_temporary`] on into real frame objects).
    pub fn create_stack_temporary(
        &mut self,
        size: u32,
        align: u32,
    ) -> i32 {
        self.temporaries.push((size, align));
        self.first_temporary + self.temporaries.len() as i32 - 1
    }

    pub fn get_global_address(
        &mut self,
        vt: EVT,
        symbol: u32,
        offset: i64,
    ) -> SDValue {
        self.intern(ISD::GlobalAddress, &[vt], &[], SDNodeFlags::default(), NodeData::Global { symbol, offset })
    }

    pub fn get_phys_register(
        &mut self,
        vt: EVT,
        reg: llrm_lir::registers::RegId,
    ) -> SDValue {
        self.intern(ISD::Register, &[vt], &[], SDNodeFlags::default(), NodeData::PhysReg(reg))
    }

    /// `getCopyToReg` of a physical register with glue: results are the chain
    /// and the glue.
    pub fn get_copy_to_phys_reg(
        &mut self,
        chain: SDValue,
        reg: llrm_lir::registers::RegId,
        value: SDValue,
        glue: Option<SDValue>,
    ) -> SDValue {
        let reg_node = self.get_phys_register(self.value_type(value), reg);
        let mut ops = vec![chain, reg_node, value];
        ops.extend(glue);
        self.get_node_multi(ISD::CopyToReg, &[EVT::OTHER, EVT::GLUE], &ops)
    }

    /// A call's `RegisterMask` operand: the registers the callee preserves
    /// are the allocator's to know, so the node carries none.
    fn symbol_id(
        &mut self,
        name: &str,
    ) -> u32 {
        match self.symbol_names.iter().position(|one| one == name) {
            Some(at) => at as u32,
            None => {
                self.symbol_names.push(name.to_string());
                self.symbol_names.len() as u32 - 1
            }
        }
    }

    pub fn get_external_symbol(
        &mut self,
        vt: EVT,
        name: &str,
    ) -> SDValue {
        let id = self.symbol_id(name);
        self.intern(ISD::ExternalSymbol, &[vt], &[], SDNodeFlags::default(), NodeData::Symbol(id))
    }

    pub fn get_target_external_symbol(
        &mut self,
        vt: EVT,
        name: &str,
    ) -> SDValue {
        let id = self.symbol_id(name);
        self.intern(ISD::TargetExternalSymbolNode, &[vt], &[], SDNodeFlags::default(), NodeData::Symbol(id))
    }

    /// `getSrcValue`.
    pub fn get_src_value(
        &mut self,
        key: u32,
    ) -> SDValue {
        self.intern(ISD::SrcValue, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::Src(key))
    }

    /// `SEGMENT_OF`: the selector of the segment a global lives in, or of
    /// DGROUP, where the near data is, when it names none.
    pub fn get_segment_of(
        &mut self,
        symbol: Option<u32>,
    ) -> SDValue {
        // The global is an operand, not the node's data: selection morphs the
        // node to a pseudo and clears its data, and nodes of two globals with
        // no operands would be one.
        let ops: Vec<SDValue> =
            symbol.map(|symbol| self.get_target_global_address(EVT::I16, symbol, 0)).into_iter().collect();
        self.intern(super::x86isd::SEGMENT_OF, &[EVT::I16], &ops, SDNodeFlags::default(), NodeData::None)
    }

    pub fn get_jump_table(
        &mut self,
        vt: EVT,
        index: u32,
    ) -> SDValue {
        self.intern(ISD::JumpTable, &[vt], &[], SDNodeFlags::default(), NodeData::JumpTable(index))
    }

    /// `getTargetJumpTable`.
    pub fn get_target_jump_table(
        &mut self,
        vt: EVT,
        index: u32,
    ) -> SDValue {
        self.intern(ISD::TargetJumpTable, &[vt], &[], SDNodeFlags::default(), NodeData::JumpTable(index))
    }

    pub fn get_register_mask(&mut self) -> SDValue {
        self.intern(ISD::RegisterMask, &[EVT::OTHER], &[], SDNodeFlags::default(), NodeData::None)
    }

    pub fn get_target_global_address(
        &mut self,
        vt: EVT,
        symbol: u32,
        offset: i64,
    ) -> SDValue {
        self.intern(ISD::TargetGlobalAddress, &[vt], &[], SDNodeFlags::default(), NodeData::Global { symbol, offset })
    }

    /// `getCopyFromReg` of a physical register with glue: the value, the
    /// chain and the glue.
    pub fn get_copy_from_phys_reg(
        &mut self,
        chain: SDValue,
        reg: llrm_lir::registers::RegId,
        vt: EVT,
        glue: SDValue,
    ) -> SDValue {
        let reg_node = self.get_phys_register(vt, reg);
        self.get_node_multi(ISD::CopyFromReg, &[vt, EVT::OTHER, EVT::GLUE], &[chain, reg_node, glue])
    }

    /// `getCopyFromReg` of a physical register without glue.
    pub fn get_copy_from_phys_reg_plain(
        &mut self,
        chain: SDValue,
        reg: llrm_lir::registers::RegId,
        vt: EVT,
    ) -> SDValue {
        let reg_node = self.get_phys_register(vt, reg);
        self.get_node_multi(ISD::CopyFromReg, &[vt, EVT::OTHER], &[chain, reg_node])
    }

    /// `getCALLSEQ_START`: `callseq_start chain, bytes, 0` (chain and glue).
    pub fn get_callseq_start(
        &mut self,
        chain: SDValue,
        bytes: u64,
    ) -> SDValue {
        let size = self.get_constant_with(u128::from(bytes), EVT::I32, true, false);
        let zero = self.get_constant_with(0, EVT::I32, true, false);
        self.get_node_multi(ISD::CallSeqStart, &[EVT::OTHER, EVT::GLUE], &[chain, size, zero])
    }

    /// `getCALLSEQ_END`: `callseq_end chain, bytes, popped, glue`.
    pub fn get_callseq_end(
        &mut self,
        chain: SDValue,
        bytes: u64,
        popped: u64,
        glue: SDValue,
    ) -> SDValue {
        let size = self.get_constant_with(u128::from(bytes), EVT::I32, true, false);
        let pop = self.get_constant_with(u128::from(popped), EVT::I32, true, false);
        self.get_node_multi(ISD::CallSeqEnd, &[EVT::OTHER, EVT::GLUE], &[chain, size, pop, glue])
    }

    pub fn get_register(
        &mut self,
        vt: EVT,
        reg: u32,
    ) -> SDValue {
        self.intern(ISD::Register, &[vt], &[], SDNodeFlags::default(), NodeData::Register(reg))
    }

    /// `getNOT`: `xor v, -1`.
    pub fn get_not(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        let ones = self.get_all_ones_constant(vt);
        self.get_node(ISD::Xor, vt, &[value, ones])
    }

    /// `getNegative`: `sub 0, v`.
    pub fn get_negative(
        &mut self,
        value: SDValue,
        vt: EVT,
    ) -> SDValue {
        let zero = self.get_constant(0, vt);
        self.get_node(ISD::Sub, vt, &[zero, value])
    }

    pub fn get_setcc(
        &mut self,
        vt: EVT,
        lhs: SDValue,
        rhs: SDValue,
        cond: CondCode,
    ) -> SDValue {
        let cc = self.get_cond_code(cond);
        self.get_node(ISD::SetCC, vt, &[lhs, rhs, cc])
    }

    /// `doesNodeExist(SETCC, {vt}, {lhs, rhs, cc})`.
    pub fn does_setcc_exist(
        &mut self,
        vt: EVT,
        lhs: SDValue,
        rhs: SDValue,
        cc: CondCode,
    ) -> bool {
        let code = self.get_cond_code(cc);
        self.cse_find(ISD::SetCC, &[vt], &[lhs, rhs, code], &NodeData::None).is_some()
    }

    pub fn get_select(
        &mut self,
        vt: EVT,
        cond: SDValue,
        then: SDValue,
        other: SDValue,
    ) -> SDValue {
        self.get_node(ISD::Select, vt, &[cond, then, other])
    }

    /// `getNode(Opcode, VT, Ops...)` with no flags.
    pub fn get_node(
        &mut self,
        opcode: ISD,
        vt: EVT,
        ops: &[SDValue],
    ) -> SDValue {
        self.get_node_flags(opcode, vt, ops, SDNodeFlags::default())
    }

    /// `getNode`: the folds of the pin's one-, two- and three-operand forms,
    /// then the CSE lookup. Four or more operands and no folds are the
    /// pin's `ArrayRef` form, which only checks `TokenFactor`-like cases.
    pub fn get_node_flags(
        &mut self,
        opcode: ISD,
        vt: EVT,
        ops: &[SDValue],
        flags: SDNodeFlags,
    ) -> SDValue {
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
    pub fn get_node_multi(
        &mut self,
        opcode: ISD,
        values: &[EVT],
        ops: &[SDValue],
    ) -> SDValue {
        self.intern(opcode, values, ops, SDNodeFlags::default(), NodeData::None)
    }
}

/// A `getNode` fold either ends in a value or lets the node be built with
/// the (possibly canonicalized) operands.
pub(super) enum Folded {
    Done(SDValue),
    Keep(Vec<SDValue>),
}

enum UserList<'a> {
    Sorted(std::slice::Iter<'a, u32>),
    Owned(std::vec::IntoIter<u32>),
}

/// `SelectionDAG::live_users`: no allocation while the use list is sorted.
struct LiveUsers<'a> {
    dag: &'a SelectionDAG,
    node: u32,
    inner: UserList<'a>,
    last: Option<u32>,
}

// The users `LiveUsers` gave out, for a test that a one-use check reads two.
#[cfg(test)]
thread_local! {
    pub(crate) static USERS_READ: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// The use-list entries copied and sorted for a query, for a test that a list
// pushed in order is not.
#[cfg(test)]
thread_local! {
    pub(crate) static USERS_SORTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl Iterator for LiveUsers<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        loop {
            let user = match &mut self.inner {
                UserList::Sorted(list) => *list.next()?,
                UserList::Owned(list) => list.next()?,
            };
            if self.last == Some(user) {
                continue;
            }
            self.last = Some(user);
            if !self.dag.is_deleted(user) && self.dag.nodes[user as usize].ops.iter().any(|op| op.node == self.node) {
                #[cfg(test)]
                USERS_READ.with(|n| n.set(n.get() + 1));
                return Some(user);
            }
        }
    }
}

/// The nodes the pin makes with `IROrder` 0 whatever instruction is being
/// visited (`ConstantSDNode`, `FrameIndexSDNode`, `RegisterSDNode`,
/// `BasicBlockSDNode`, .. take no `SDLoc`; `getUNDEF` and `getPOISON` pass an
/// empty one). The scheduler reads 0 as "no order" and ranks such a node
/// differently from an ordered one.
fn has_no_ir_order(opcode: ISD) -> bool {
    use ISD::*;
    matches!(
        opcode,
        Constant
            | TargetConstant
            | ConstantFP
            | TargetConstantFP
            | Register
            | RegisterMask
            | JumpTable
            | TargetJumpTable
            | SrcValue
            | ExternalSymbol
            | TargetExternalSymbolNode
            | FrameIndex
            | TargetFrameIndex
            | ConstantPool
            | TargetConstantPool
            | BasicBlock
            | ValueType
            | CondCodeNode
            | Undef
            | Poison
    )
}
