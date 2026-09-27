//! DOS-style state graph substrate for the `buildprs` graph compiler.
//!
//! This module models the 48-byte DOS `StateNode` record with typed links and
//! phase fields. The current pattern-based lowering remains the active
//! generator; this graph is the new backend scaffold that later passes will use.

pub const STATE_RECORD_SIZE: usize = 0x30;
pub const ENCODE1BYTE_QBASIC_11: u8 = 0xE0;

const RELATIVE_DELTA_LIMIT: isize = (ENCODE1BYTE_QBASIC_11 as isize) / 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum OptLevel {
    O0 = 0,
    O1 = 1,
    O2 = 2,
}

impl OptLevel {
    pub fn combines_states(self) -> bool {
        !matches!(self, Self::O0)
    }

    pub fn shares_states(self) -> bool {
        matches!(self, Self::O2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum StateKind {
    Branch = 0,
    Mark = 1,
    Emit = 2,
    Accept = 3,
    Reject = 4,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateFlags(u8);

impl StateFlags {
    pub const SKIP: Self = Self(0x01);
    pub const TRUE_SHARED: Self = Self(0x02);
    pub const FALSE_SHARED: Self = Self(0x04);
    pub const DUPED: Self = Self(0x08);
    pub const INTEGRATED: Self = Self(0x10);
    pub const FALSE_FIRST: Self = Self(0x20);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn contains(self, flag: Self) -> bool {
        self.0 & flag.0 == flag.0
    }

    pub fn insert(&mut self, flag: Self) {
        self.0 |= flag.0;
    }

    pub fn remove(&mut self, flag: Self) {
        self.0 &= !flag.0;
    }
}

impl std::ops::BitOr for StateFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateNode {
    pub true_link: Option<NodeId>,
    pub false_link: Option<NodeId>,
    pub final_or_out: Option<NodeId>,
    pub next_work: Option<NodeId>,
    pub next_global: Option<NodeId>,
    pub scratch_word: u16,
    pub refcount: u16,
    pub flags: StateFlags,
    pub sort_index: usize,
    pub encoded_size: usize,
    pub aux_or_span: Option<NodeId>,
    pub kind: StateKind,
    pub payload: u16,
    pub aux_a: Option<NodeId>,
    pub aux_b: Option<NodeId>,
}

impl Default for StateNode {
    fn default() -> Self {
        Self {
            true_link: None,
            false_link: None,
            final_or_out: None,
            next_work: None,
            next_global: None,
            scratch_word: 0,
            refcount: 0,
            flags: StateFlags::empty(),
            sort_index: 0,
            encoded_size: 0,
            aux_or_span: None,
            kind: StateKind::Branch,
            payload: 0,
            aux_a: None,
            aux_b: None,
        }
    }
}

impl StateNode {
    pub fn branch_node(node_id: u16) -> Self {
        Self {
            kind: StateKind::Branch,
            payload: node_id,
            encoded_size: 2,
            ..Self::default()
        }
    }

    pub fn mark(mark: u8) -> Self {
        Self {
            kind: StateKind::Mark,
            payload: u16::from(mark),
            encoded_size: 2,
            ..Self::default()
        }
    }

    pub fn emit(opcode: u16) -> Self {
        Self {
            kind: StateKind::Emit,
            payload: opcode,
            encoded_size: 3,
            ..Self::default()
        }
    }

    pub fn accept() -> Self {
        Self {
            kind: StateKind::Accept,
            encoded_size: 1,
            ..Self::default()
        }
    }

    pub fn reject() -> Self {
        Self {
            kind: StateKind::Reject,
            encoded_size: 1,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphEncodeError {
    BranchOffsetOutOfRange { offset: usize },
    NodeIdOutOfRange { node_id: u16 },
    SortIndexUnderflow { node: NodeId, target: NodeId },
}

impl std::fmt::Display for GraphEncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BranchOffsetOutOfRange { offset } => {
                write!(f, "branch target offset {offset} cannot be encoded")
            }
            Self::NodeIdOutOfRange { node_id } => {
                write!(f, "node id {node_id} cannot be encoded")
            }
            Self::SortIndexUnderflow { node, target } => {
                write!(f, "branch delta from {node:?} to {target:?} underflowed")
            }
        }
    }
}

impl std::error::Error for GraphEncodeError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraph {
    nodes: Vec<StateNode>,
    pub global_head: Option<NodeId>,
    pub work_head: Option<NodeId>,
}

impl StateGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_node(&mut self) -> NodeId {
        self.add_node(StateNode::default())
    }

    pub fn add_node(&mut self, node: StateNode) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(node);
        id
    }

    pub fn node(&self, id: NodeId) -> &StateNode {
        &self.nodes[id.0]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut StateNode {
        &mut self.nodes[id.0]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.nodes.len()).map(NodeId)
    }

    pub fn add_true_link(&mut self, parent: NodeId, child: NodeId) {
        self.change_true_link(parent, Some(child));
    }

    pub fn add_false_link(&mut self, parent: NodeId, child: NodeId) {
        self.change_false_link(parent, Some(child));
    }

    pub fn change_true_link(&mut self, parent: NodeId, child: Option<NodeId>) {
        self.bump_refcount(child);
        let old = self.node(parent).true_link;
        self.node_mut(parent).true_link = child;
        self.release_refcount(old);
    }

    pub fn change_false_link(&mut self, parent: NodeId, child: Option<NodeId>) {
        self.bump_refcount(child);
        let old = self.node(parent).false_link;
        self.node_mut(parent).false_link = child;
        self.release_refcount(old);
    }

    pub fn replace_link_target(&mut self, old: NodeId, new: NodeId) {
        let ids = self.ids().collect::<Vec<_>>();
        for id in ids {
            if self.node(id).true_link == Some(old) {
                self.change_true_link(id, Some(new));
            }
            if self.node(id).false_link == Some(old) {
                self.change_false_link(id, Some(new));
            }
        }
    }

    pub fn prepend_global(&mut self, id: NodeId) {
        let old = self.global_head;
        self.node_mut(id).next_global = old;
        self.global_head = Some(id);
    }

    pub fn global_roots(&self) -> Vec<NodeId> {
        let mut roots = Vec::new();
        let mut current = self.global_head;
        while let Some(id) = current {
            roots.push(id);
            current = self.node(id).next_global;
        }
        roots
    }

    pub fn prepend_work(&mut self, id: NodeId) {
        let old = self.work_head;
        self.node_mut(id).next_work = old;
        self.work_head = Some(id);
    }

    pub fn work_items(&self) -> Vec<NodeId> {
        let mut items = Vec::new();
        let mut current = self.work_head;
        while let Some(id) = current {
            items.push(id);
            current = self.node(id).next_work;
        }
        items
    }

    pub fn encode_node(&self, id: NodeId) -> Result<Vec<u8>, GraphEncodeError> {
        let node = self.node(id);
        match node.kind {
            StateKind::Accept => Ok(vec![0x00]),
            StateKind::Reject => Ok(vec![0x01]),
            StateKind::Mark => Ok(vec![0x02, node.payload as u8]),
            StateKind::Emit => {
                let [lo, hi] = node.payload.to_le_bytes();
                Ok(vec![0x03, lo, hi])
            }
            StateKind::Branch => {
                let mut encoded = encode_node_id(node.payload)?;
                encoded.extend(self.encode_branch_operand(id)?);
                Ok(encoded)
            }
        }
    }

    fn encode_branch_operand(&self, id: NodeId) -> Result<Vec<u8>, GraphEncodeError> {
        let node = self.node(id);
        let Some(target) = node.true_link else {
            return Ok(vec![0xFF]);
        };

        let target_node = self.node(target);
        let delta =
            target_node.sort_index as isize - node.sort_index as isize - node.encoded_size as isize;

        if (-RELATIVE_DELTA_LIMIT + 1..RELATIVE_DELTA_LIMIT).contains(&delta) {
            if delta < 0 {
                return Ok(vec![(delta + ENCODE1BYTE_QBASIC_11 as isize) as u8]);
            }
            return Ok(vec![delta as u8]);
        }

        encode_absolute_branch_operand(target_node.sort_index)
    }

    fn bump_refcount(&mut self, id: Option<NodeId>) {
        if let Some(id) = id {
            let node = self.node_mut(id);
            node.refcount = node
                .refcount
                .checked_add(1)
                .expect("too many pointers to state");
        }
    }

    fn release_refcount(&mut self, id: Option<NodeId>) {
        if let Some(id) = id {
            let node = self.node_mut(id);
            node.refcount = node
                .refcount
                .checked_sub(1)
                .expect("state refcount underflow");
        }
    }
}

fn encode_node_id(node_id: u16) -> Result<Vec<u8>, GraphEncodeError> {
    if node_id < u16::from(ENCODE1BYTE_QBASIC_11) {
        return Ok(vec![node_id as u8]);
    }

    let encoded = u32::from(node_id) + 255 * u32::from(ENCODE1BYTE_QBASIC_11);
    if encoded > u32::from(u16::MAX) {
        return Err(GraphEncodeError::NodeIdOutOfRange { node_id });
    }

    Ok(vec![(encoded >> 8) as u8, (encoded & 0xFF) as u8])
}

fn encode_absolute_branch_operand(offset: usize) -> Result<Vec<u8>, GraphEncodeError> {
    let encoded = offset as u32 + 255 * u32::from(ENCODE1BYTE_QBASIC_11);
    if encoded > u32::from(u16::MAX) {
        return Err(GraphEncodeError::BranchOffsetOutOfRange { offset });
    }

    Ok(vec![(encoded >> 8) as u8, (encoded & 0xFF) as u8])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_allocates_nodes_with_dos_defaults_and_refcounted_links() {
        let mut graph = StateGraph::new();
        let parent = graph.alloc_node();
        let child = graph.alloc_node();

        assert_eq!(parent, NodeId(0));
        assert_eq!(child, NodeId(1));
        assert_eq!(graph.node(parent).kind, StateKind::Branch);
        assert_eq!(graph.node(parent).refcount, 0);
        assert_eq!(graph.node(parent).flags, StateFlags::empty());
        assert_eq!(graph.node(parent).true_link, None);

        graph.add_true_link(parent, child);

        assert_eq!(graph.node(parent).true_link, Some(child));
        assert_eq!(graph.node(child).refcount, 1);

        graph.change_true_link(parent, None);

        assert_eq!(graph.node(parent).true_link, None);
        assert_eq!(graph.node(child).refcount, 0);
    }

    #[test]
    fn state_node_encoding_matches_dos_directives_and_little_endian_payloads() {
        let mut graph = StateGraph::new();
        let accept = graph.add_node(StateNode::accept());
        let reject = graph.add_node(StateNode::reject());
        let mark = graph.add_node(StateNode::mark(0x12));
        let emit = graph.add_node(StateNode::emit(0x3456));

        graph.node_mut(accept).sort_index = 0;
        graph.node_mut(reject).sort_index = 1;
        graph.node_mut(mark).sort_index = 2;
        graph.node_mut(emit).sort_index = 4;

        assert_eq!(graph.encode_node(accept).unwrap(), vec![0x00]);
        assert_eq!(graph.encode_node(reject).unwrap(), vec![0x01]);
        assert_eq!(graph.encode_node(mark).unwrap(), vec![0x02, 0x12]);
        assert_eq!(graph.encode_node(emit).unwrap(), vec![0x03, 0x56, 0x34]);
    }

    #[test]
    fn branch_operand_uses_accept_relative_and_absolute_forms() {
        let mut graph = StateGraph::new();
        let accept_branch = graph.add_node(StateNode::branch_node(4));
        let relative_branch = graph.add_node(StateNode::branch_node(5));
        let relative_target = graph.add_node(StateNode::accept());
        let absolute_branch = graph.add_node(StateNode::branch_node(6));
        let absolute_target = graph.add_node(StateNode::reject());

        graph.node_mut(accept_branch).sort_index = 0;
        graph.node_mut(accept_branch).encoded_size = 2;
        graph.node_mut(relative_branch).sort_index = 10;
        graph.node_mut(relative_branch).encoded_size = 2;
        graph.node_mut(relative_target).sort_index = 20;
        graph.node_mut(absolute_branch).sort_index = 30;
        graph.node_mut(absolute_branch).encoded_size = 3;
        graph.node_mut(absolute_target).sort_index = 200;

        graph.add_true_link(relative_branch, relative_target);
        graph.add_true_link(absolute_branch, absolute_target);

        assert_eq!(graph.encode_node(accept_branch).unwrap(), vec![0x04, 0xFF]);
        assert_eq!(
            graph.encode_node(relative_branch).unwrap(),
            vec![0x05, 0x08]
        );
        assert_eq!(
            graph.encode_node(absolute_branch).unwrap(),
            vec![0x06, 0xDF, 0xE8]
        );
    }

    #[test]
    fn branch_operand_encodes_near_negative_delta_with_encode1byte_bias() {
        let mut graph = StateGraph::new();
        let target = graph.add_node(StateNode::accept());
        let branch = graph.add_node(StateNode::branch_node(7));

        graph.node_mut(target).sort_index = 10;
        graph.node_mut(branch).sort_index = 20;
        graph.node_mut(branch).encoded_size = 2;
        graph.add_true_link(branch, target);

        assert_eq!(graph.encode_node(branch).unwrap(), vec![0x07, 0xD4]);
    }
}
