//! Graph optimization passes for DOS-style `buildprs` lowering.

use std::collections::BTreeSet;

use crate::buildprs_compare::{can_merge_states, find_state};
use crate::buildprs_graph::{NodeId, StateFlags, StateGraph, StateKind};

pub fn combine_states(graph: &mut StateGraph, state: Option<NodeId>) -> Option<NodeId> {
    let mut seen = BTreeSet::new();
    combine_states_inner(graph, state, &mut seen)
}

fn combine_states_inner(
    graph: &mut StateGraph,
    state: Option<NodeId>,
    seen: &mut BTreeSet<NodeId>,
) -> Option<NodeId> {
    let state = state?;
    if !seen.insert(state) {
        return Some(state);
    }
    if graph.node(state).flags.contains(StateFlags::INTEGRATED) {
        return Some(state);
    }
    if matches!(
        graph.node(state).kind,
        StateKind::Accept | StateKind::Reject
    ) {
        return Some(state);
    }
    if graph.node(state).kind == StateKind::Branch && graph.node(state).true_link.is_none() {
        return Some(state);
    }

    if let Some(duplicate) = find_state(graph, Some(state)) {
        return Some(duplicate);
    }

    if !graph.node(state).flags.contains(StateFlags::SKIP)
        && !graph.node(state).flags.contains(StateFlags::TRUE_SHARED)
    {
        let next = combine_states_inner(graph, graph.node(state).true_link, seen);
        graph.change_true_link(state, next);
    }

    if !graph.node(state).flags.contains(StateFlags::FALSE_SHARED) {
        let next = combine_states_inner(graph, graph.node(state).false_link, seen);
        graph.change_false_link(state, next);
    }

    Some(state)
}

pub fn share_states(graph: &mut StateGraph, root: Option<NodeId>) {
    let Some(root) = root else {
        return;
    };
    let mut seen = BTreeSet::new();
    share_states_from(graph, root, Some(root), &mut seen);
}

fn share_states_from(
    graph: &mut StateGraph,
    search_root: NodeId,
    current: Option<NodeId>,
    seen: &mut BTreeSet<(NodeId, Option<NodeId>)>,
) {
    let Some(current) = current else {
        return;
    };
    if !seen.insert((search_root, Some(current))) {
        return;
    }

    if !graph.node(current).flags.contains(StateFlags::TRUE_SHARED) {
        let child = graph.node(current).true_link;
        if child.is_some() && can_merge_states(graph, 1, child, Some(search_root)) {
            graph.change_true_link(current, Some(search_root));
            graph
                .node_mut(current)
                .flags
                .insert(StateFlags::TRUE_SHARED);
        } else {
            share_states_from(graph, search_root, child, seen);
        }
    }

    if !graph.node(current).flags.contains(StateFlags::FALSE_SHARED) {
        let child = graph.node(current).false_link;
        if child.is_some() && can_merge_states(graph, 1, child, Some(search_root)) {
            graph.change_false_link(current, Some(search_root));
            graph
                .node_mut(current)
                .flags
                .insert(StateFlags::FALSE_SHARED);
        } else {
            share_states_from(graph, search_root, child, seen);
        }
    }

    if !graph.node(current).flags.contains(StateFlags::TRUE_SHARED) {
        let child = graph.node(current).true_link;
        share_states_from(graph, child.unwrap_or(search_root), child, seen);
    }

    if !graph.node(current).flags.contains(StateFlags::FALSE_SHARED) {
        let child = graph.node(current).false_link;
        share_states_from(graph, child.unwrap_or(search_root), child, seen);
    }
}

#[cfg(test)]
mod tests {
    use crate::buildprs_graph::StateNode;

    use super::*;

    #[test]
    fn combine_reuses_equivalent_integrated_root() {
        let mut graph = StateGraph::new();
        let existing = graph.add_node(StateNode::emit(11));
        let query = graph.add_node(StateNode::emit(11));
        graph.prepend_global(existing);

        assert_eq!(combine_states(&mut graph, Some(query)), Some(existing));
    }

    #[test]
    fn combine_keeps_distinct_payloads_separate() {
        let mut graph = StateGraph::new();
        let existing = graph.add_node(StateNode::emit(11));
        let query = graph.add_node(StateNode::emit(12));
        graph.prepend_global(existing);

        assert_eq!(combine_states(&mut graph, Some(query)), Some(query));
    }

    #[test]
    fn share_states_marks_equivalent_child_as_shared() {
        let mut graph = StateGraph::new();
        let root = graph.add_node(StateNode::emit(7));
        let parent = graph.add_node(StateNode::mark(3));
        let child = graph.add_node(StateNode::emit(7));
        graph.add_true_link(parent, child);

        let mut seen = BTreeSet::new();
        share_states_from(&mut graph, root, Some(parent), &mut seen);

        assert_eq!(graph.node(parent).true_link, Some(root));
        assert!(graph.node(parent).flags.contains(StateFlags::TRUE_SHARED));
    }
}
