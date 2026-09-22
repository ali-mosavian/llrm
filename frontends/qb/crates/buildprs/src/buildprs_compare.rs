//! Structural comparison helpers for DOS-style graph optimization.

use std::collections::BTreeSet;

use crate::buildprs_graph::{NodeId, StateFlags, StateGraph, StateKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeComparison {
    Different,
    Equal,
    CompatibleMark,
}

pub fn compare_trees(graph: &StateGraph, a: Option<NodeId>, b: Option<NodeId>) -> TreeComparison {
    let (Some(a), Some(b)) = (a, b) else {
        return if a == b {
            TreeComparison::Equal
        } else {
            TreeComparison::Different
        };
    };
    let left = graph.node(a);
    let right = graph.node(b);
    if left.kind != right.kind {
        return TreeComparison::Different;
    }

    match left.kind {
        StateKind::Branch | StateKind::Mark | StateKind::Emit => {
            if left.payload == right.payload {
                TreeComparison::Equal
            } else if left.kind == StateKind::Mark {
                TreeComparison::CompatibleMark
            } else {
                TreeComparison::Different
            }
        }
        StateKind::Accept | StateKind::Reject => TreeComparison::Equal,
    }
}

pub fn can_merge_states(
    graph: &mut StateGraph,
    depth: u16,
    query: Option<NodeId>,
    candidate: Option<NodeId>,
) -> bool {
    let mut seen = BTreeSet::new();
    can_merge_states_inner(graph, depth, query, candidate, &mut seen)
}

fn can_merge_states_inner(
    graph: &mut StateGraph,
    depth: u16,
    query: Option<NodeId>,
    candidate: Option<NodeId>,
    seen: &mut BTreeSet<(Option<NodeId>, Option<NodeId>)>,
) -> bool {
    if !seen.insert((query, candidate)) {
        return true;
    }
    if compare_trees(graph, candidate, query) != TreeComparison::Equal {
        return false;
    }
    let Some(query) = query else {
        return true;
    };
    let Some(candidate) = candidate else {
        return false;
    };

    graph.node_mut(candidate).scratch_word = depth;
    graph.node_mut(query).scratch_word = depth;

    if !can_merge_leg(
        graph,
        depth,
        query,
        candidate,
        Edge::True,
        StateFlags::TRUE_SHARED,
        depth + 1,
        seen,
    ) {
        return false;
    }

    can_merge_leg(
        graph,
        depth,
        query,
        candidate,
        Edge::False,
        StateFlags::FALSE_SHARED,
        depth + 2,
        seen,
    )
}

pub fn compare_states(
    graph: &mut StateGraph,
    candidate: Option<NodeId>,
    query: Option<NodeId>,
) -> Option<NodeId> {
    let mut seen = BTreeSet::new();
    compare_states_inner(graph, candidate, query, &mut seen)
}

fn compare_states_inner(
    graph: &mut StateGraph,
    candidate: Option<NodeId>,
    query: Option<NodeId>,
    seen: &mut BTreeSet<NodeId>,
) -> Option<NodeId> {
    let candidate = candidate?;
    if !seen.insert(candidate) {
        return None;
    }
    if Some(candidate) != query && can_merge_states(graph, 1, query, Some(candidate)) {
        return Some(candidate);
    }

    if graph.node(candidate).flags.contains(StateFlags::SKIP) {
        return None;
    }

    if !graph
        .node(candidate)
        .flags
        .contains(StateFlags::TRUE_SHARED)
    {
        if let Some(hit) = compare_states_inner(graph, graph.node(candidate).true_link, query, seen)
        {
            return Some(hit);
        }
    }

    if !graph
        .node(candidate)
        .flags
        .contains(StateFlags::FALSE_SHARED)
    {
        if let Some(hit) =
            compare_states_inner(graph, graph.node(candidate).false_link, query, seen)
        {
            return Some(hit);
        }
    }

    None
}

pub fn find_state(graph: &mut StateGraph, query: Option<NodeId>) -> Option<NodeId> {
    for root in graph.global_roots() {
        if let Some(hit) = compare_states(graph, Some(root), query) {
            return Some(hit);
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    True,
    False,
}

fn can_merge_leg(
    graph: &mut StateGraph,
    _depth: u16,
    query: NodeId,
    candidate: NodeId,
    edge: Edge,
    flag: StateFlags,
    next_depth: u16,
    seen: &mut BTreeSet<(Option<NodeId>, Option<NodeId>)>,
) -> bool {
    let query_child = child(graph, query, edge);
    let candidate_child = child(graph, candidate, edge);
    let query_shared = graph.node(query).flags.contains(flag);
    let candidate_shared = graph.node(candidate).flags.contains(flag);

    if query_shared && candidate_shared {
        return match (query_child, candidate_child) {
            (Some(q), Some(c)) => graph.node(q).scratch_word == graph.node(c).scratch_word,
            (None, None) => true,
            _ => false,
        };
    }

    can_merge_states_inner(graph, next_depth, query_child, candidate_child, seen)
}

fn child(graph: &StateGraph, id: NodeId, edge: Edge) -> Option<NodeId> {
    match edge {
        Edge::True => graph.node(id).true_link,
        Edge::False => graph.node(id).false_link,
    }
}

#[cfg(test)]
mod tests {
    use crate::buildprs_graph::StateNode;

    use super::*;

    #[test]
    fn equivalent_branch_trees_merge_by_payload_and_success_shape() {
        let mut graph = StateGraph::new();
        let accept_a = graph.add_node(StateNode::accept());
        let accept_b = graph.add_node(StateNode::accept());
        let left = graph.add_node(StateNode::branch_node(42));
        let right = graph.add_node(StateNode::branch_node(42));
        graph.add_true_link(left, accept_a);
        graph.add_true_link(right, accept_b);

        assert!(can_merge_states(&mut graph, 1, Some(left), Some(right)));
    }

    #[test]
    fn different_payloads_do_not_merge() {
        let mut graph = StateGraph::new();
        let left = graph.add_node(StateNode::branch_node(42));
        let right = graph.add_node(StateNode::branch_node(43));

        assert!(!can_merge_states(&mut graph, 1, Some(left), Some(right)));
    }

    #[test]
    fn find_state_scans_integrated_roots_for_equivalent_subtree() {
        let mut graph = StateGraph::new();
        let existing = graph.add_node(StateNode::emit(7));
        let query = graph.add_node(StateNode::emit(7));
        graph.prepend_global(existing);

        assert_eq!(find_state(&mut graph, Some(query)), Some(existing));
    }
}
