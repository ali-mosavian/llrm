//! `EquivalenceClasses`: disjoint sets of values with a stable member order.
//!
//! # How it works
//! A translation of LLVM `d1106deb71cc` `llvm/ADT/EquivalenceClasses.h`. Each
//! value is a node with a leader and a next pointer. A leader heads a list of
//! its class; `union_sets` appends the second class's list to the end of the
//! first's, so the first leader stays the leader and `members` visits the
//! first class's members, then the second's, each in its own order.
//!
//! # Example
//! After `union_sets(1, 2)`, `insert(4)` and `union_sets(5, 1)`, `members(1)`
//! gives `5, 1, 2` and `members(4)` gives `4`.

use std::hash::Hash;

use llrm_support::hash::HashMap;

#[derive(Clone, Debug)]
struct Node<T> {
    data: T,
    /// For a leader, the last node of its list; otherwise the leader or the
    /// node it was last known to be a member through (`getLeader` follows it).
    leader: usize,
    /// The next node of the list.
    next: Option<usize>,
    is_leader: bool,
}

/// `llvm::EquivalenceClasses<T>`.
#[derive(Clone, Debug)]
pub struct EquivalenceClasses<T> {
    mapping: HashMap<T, usize>,
    nodes: Vec<Node<T>>,
}

impl<T> Default for EquivalenceClasses<T> {
    fn default() -> Self {
        Self { mapping: HashMap::default(), nodes: Vec::new() }
    }
}

impl<T: Clone + Eq + Hash> EquivalenceClasses<T> {
    /// `insert(Data)`: the node of `data`, made a class of its own if new.
    fn insert_node(
        &mut self,
        data: &T,
    ) -> usize {
        if let Some(&node) = self.mapping.get(data) {
            return node;
        }
        let node = self.nodes.len();
        self.nodes.push(Node { data: data.clone(), leader: node, next: None, is_leader: true });
        self.mapping.insert(data.clone(), node);
        node
    }

    /// `insert(Data)`.
    pub fn insert(
        &mut self,
        data: &T,
    ) {
        self.insert_node(data);
    }

    /// `contains(V)`.
    pub fn contains(
        &self,
        data: &T,
    ) -> bool {
        self.mapping.contains_key(data)
    }

    /// `ECValue::getLeader`, with path compression.
    fn leader_of(
        &mut self,
        node: usize,
    ) -> usize {
        if self.nodes[node].is_leader {
            return node;
        }
        let up = self.nodes[node].leader;
        if self.nodes[up].is_leader {
            return up;
        }
        let top = self.leader_of(up);
        self.nodes[node].leader = top;
        top
    }

    /// `unionSets(V1, V2)`: both inserted if new; the second class's list is
    /// appended to the first's. The leader of the first class.
    pub fn union_sets(
        &mut self,
        first: &T,
        second: &T,
    ) -> T {
        let (a, b) = (self.insert_node(first), self.insert_node(second));
        let (l1, l2) = (self.leader_of(a), self.leader_of(b));
        if l1 == l2 {
            return self.nodes[l1].data.clone();
        }
        // The end of the L1 list points to the L2 leader node.
        let end = self.nodes[l1].leader;
        self.nodes[end].next = Some(l2);
        // L1's end of list is L2's.
        self.nodes[l1].leader = self.nodes[l2].leader;
        // L2 is no longer a leader; its leader is L1.
        self.nodes[l2].is_leader = false;
        self.nodes[l2].leader = l1;
        self.nodes[l1].data.clone()
    }

    /// `members(V)`: the class of `data` from its leader on; empty if `data`
    /// is not in the set.
    pub fn members(
        &self,
        data: &T,
    ) -> Members<'_, T> {
        let start = self.mapping.get(data).map(|&node| self.find(node));
        Members { classes: self, at: start }
    }

    /// `findLeader` without path compression.
    fn find(
        &self,
        mut node: usize,
    ) -> usize {
        while !self.nodes[node].is_leader {
            node = self.nodes[node].leader;
        }
        node
    }

    /// `getLeaderValue(V)`.
    pub fn leader_value(
        &self,
        data: &T,
    ) -> Option<&T> {
        self.mapping.get(data).map(|&node| &self.nodes[self.find(node)].data)
    }

    /// `eraseClass(V)`: every member of the class of `data` leaves the set.
    /// (The nodes stay in the arena, out of the mapping.)
    pub fn erase_class(
        &mut self,
        data: &T,
    ) {
        if !self.contains(data) {
            return;
        }
        let members: Vec<T> = self.members(data).cloned().collect();
        for member in members {
            self.mapping.remove(&member);
        }
    }

    /// `getNumClasses`.
    pub fn classes(&self) -> usize {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(at, node)| node.is_leader && self.mapping.get(&node.data) == Some(at))
            .count()
    }

    /// `empty`.
    pub fn is_empty(&self) -> bool {
        self.mapping.is_empty()
    }
}

/// `member_iterator`.
pub struct Members<'a, T> {
    classes: &'a EquivalenceClasses<T>,
    at: Option<usize>,
}

impl<'a, T> Iterator for Members<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        let node = &self.classes.nodes[self.at?];
        self.at = node.next;
        Some(&node.data)
    }
}

#[cfg(test)]
#[path = "equivalence_tests.rs"]
mod tests;
