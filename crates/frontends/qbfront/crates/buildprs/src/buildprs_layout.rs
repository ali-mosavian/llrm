//! Work-list construction and branch-size relaxation for graph emission.

use std::collections::BTreeSet;

use crate::buildprs_graph::{NodeId, StateFlags, StateGraph, StateKind, ENCODE1BYTE_QBASIC_11};

const DELTA_LIMIT: isize = (ENCODE1BYTE_QBASIC_11 as isize) / 2;

pub fn sort_state_values(graph: &mut StateGraph) {
    graph.work_head = None;
    let mut seen = BTreeSet::new();
    for root in graph.global_roots() {
        duplicate_state(graph, root, &mut seen);
    }
    assign_sort_keys(graph);
}

pub fn duplicate_state(graph: &mut StateGraph, src: NodeId, seen: &mut BTreeSet<NodeId>) {
    if !seen.insert(src) {
        return;
    }

    if graph.node(src).flags.contains(StateFlags::FALSE_FIRST) {
        if !graph.node(src).flags.contains(StateFlags::TRUE_SHARED) {
            if let Some(true_link) = graph.node(src).true_link {
                duplicate_state(graph, true_link, seen);
            }
        }
        if !graph.node(src).flags.contains(StateFlags::FALSE_SHARED) {
            if let Some(false_link) = graph.node(src).false_link {
                duplicate_state(graph, false_link, seen);
            }
        }
    } else {
        if let (Some(true_link), Some(false_link)) =
            (graph.node(src).true_link, graph.node(src).false_link)
        {
            if is_empty_branch(graph, false_link)
                && target_reachable(graph, true_link, false_link, &mut BTreeSet::new())
            {
                seen.insert(false_link);
                duplicate_state(graph, true_link, seen);
                seen.remove(&false_link);
                duplicate_state(graph, false_link, seen);
                graph.node_mut(src).flags.insert(StateFlags::DUPED);
                graph.prepend_work(src);
                return;
            }
        }
        if !graph.node(src).flags.contains(StateFlags::TRUE_SHARED) {
            if let Some(true_link) = graph.node(src).true_link {
                duplicate_state(graph, true_link, seen);
            }
        }
        if !graph.node(src).flags.contains(StateFlags::FALSE_SHARED) {
            if let Some(false_link) = graph.node(src).false_link {
                duplicate_state(graph, false_link, seen);
            }
        }
    }

    graph.node_mut(src).flags.insert(StateFlags::DUPED);
    graph.prepend_work(src);
}

fn is_empty_branch(graph: &StateGraph, id: NodeId) -> bool {
    graph.node(id).kind == StateKind::Branch && graph.node(id).payload == 4
}

fn target_reachable(
    graph: &StateGraph,
    current: NodeId,
    target: NodeId,
    seen: &mut BTreeSet<NodeId>,
) -> bool {
    if current == target {
        return true;
    }
    if !seen.insert(current) {
        return false;
    }
    graph
        .node(current)
        .true_link
        .is_some_and(|link| target_reachable(graph, link, target, seen))
        || graph
            .node(current)
            .false_link
            .is_some_and(|link| target_reachable(graph, link, target, seen))
}

pub fn assign_sort_keys(graph: &mut StateGraph) {
    let work_items = graph.work_items();
    let mut offset = 0;
    for id in &work_items {
        let size = initial_encoded_size(graph, *id);
        let scratch_word = if can_shrink_branch(graph, *id) { 0 } else { 1 };
        let node = graph.node_mut(*id);
        node.sort_index = offset;
        node.encoded_size = size;
        node.scratch_word = scratch_word;
        offset += size;
    }

    loop {
        let mut shrink_count = 0;
        for id in &work_items {
            graph.node_mut(*id).sort_index -= shrink_count;
            if graph.node(*id).scratch_word == 0 && branch_delta_after_shrink(graph, *id).is_some()
            {
                graph.node_mut(*id).encoded_size -= 1;
                graph.node_mut(*id).scratch_word = 1;
                shrink_count += 1;
            }
        }
        if shrink_count == 0 {
            break;
        }
    }
}

fn initial_encoded_size(graph: &StateGraph, id: NodeId) -> usize {
    let node = graph.node(id);
    match node.kind {
        StateKind::Accept | StateKind::Reject => 1,
        StateKind::Mark => 2,
        StateKind::Emit => 3,
        StateKind::Branch if node.true_link.is_none() => node_id_len(node.payload) + 1,
        StateKind::Branch => node_id_len(node.payload) + 2,
    }
}

fn node_id_len(node_id: u16) -> usize {
    if node_id < u16::from(ENCODE1BYTE_QBASIC_11) {
        1
    } else {
        2
    }
}

fn can_shrink_branch(graph: &StateGraph, id: NodeId) -> bool {
    graph.node(id).kind == StateKind::Branch && graph.node(id).true_link.is_some()
}

fn branch_delta_after_shrink(graph: &StateGraph, id: NodeId) -> Option<isize> {
    let node = graph.node(id);
    let target = graph.node(node.true_link?);
    let delta =
        target.sort_index as isize - node.sort_index as isize - node.encoded_size as isize + 1;
    if (-DELTA_LIMIT + 1..DELTA_LIMIT).contains(&delta) {
        Some(delta)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::buildprs_graph::StateNode;

    use super::*;

    #[test]
    fn layout_assigns_contiguous_offsets_for_reachable_graph() {
        let mut graph = StateGraph::new();
        let root = graph.add_node(StateNode::branch_node(5));
        let accept = graph.add_node(StateNode::accept());
        graph.add_true_link(root, accept);
        graph.prepend_global(root);

        sort_state_values(&mut graph);
        let items = graph.work_items();

        assert_eq!(items, vec![root, accept]);
        assert_eq!(graph.node(root).sort_index, 0);
        assert_eq!(graph.node(accept).sort_index, graph.node(root).encoded_size);
    }

    #[test]
    fn layout_shrinks_near_branch_operand() {
        let mut graph = StateGraph::new();
        let root = graph.add_node(StateNode::branch_node(5));
        let accept = graph.add_node(StateNode::accept());
        graph.add_true_link(root, accept);
        graph.prepend_global(root);

        sort_state_values(&mut graph);

        assert_eq!(graph.node(root).encoded_size, 2);
    }
}
