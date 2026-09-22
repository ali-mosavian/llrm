//! Normalize and integrate newly built graph roots.

use std::collections::BTreeSet;

use crate::buildprs_compare::{compare_trees, TreeComparison};
use crate::buildprs_graph::{NodeId, OptLevel, StateFlags, StateGraph, StateKind, StateNode};
use crate::buildprs_optimize::{combine_states, share_states};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntegrateError {
    AmbiguousStateGraph { anchor: String },
}

impl std::fmt::Display for IntegrateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AmbiguousStateGraph { anchor } => {
                write!(f, "ambiguous state graph while integrating {anchor}")
            }
        }
    }
}

impl std::error::Error for IntegrateError {}

pub fn integrate(
    graph: &mut StateGraph,
    state: Option<NodeId>,
    anchor: &str,
    opt_level: OptLevel,
) -> Result<Option<NodeId>, IntegrateError> {
    let Some(state) = state else {
        return Ok(None);
    };

    normalize_true(graph, Some(state));
    normalize_false(graph, Some(state));

    if opt_level.shares_states() {
        share_states(graph, Some(state));
    }

    splice_states(graph, Some(state), anchor)?;

    let result = if opt_level.combines_states() {
        combine_states(graph, Some(state)).unwrap_or(state)
    } else {
        state
    };

    if result == state {
        graph.prepend_global(state);
    }
    graph.node_mut(state).flags.insert(StateFlags::INTEGRATED);
    Ok(Some(result))
}

pub fn normalize_true(graph: &mut StateGraph, state: Option<NodeId>) {
    let mut seen = BTreeSet::new();
    normalize_true_inner(graph, state, &mut seen);
}

fn normalize_true_inner(
    graph: &mut StateGraph,
    state: Option<NodeId>,
    seen: &mut BTreeSet<NodeId>,
) {
    let Some(state) = state else {
        return;
    };
    if !seen.insert(state) {
        return;
    }

    while let Some(child) = graph.node(state).true_link {
        if !is_glue_branch(graph, child) {
            break;
        }
        let grandchild = graph.node(child).true_link;
        if grandchild == Some(child) || grandchild == graph.node(state).true_link {
            break;
        }
        graph.change_true_link(state, grandchild);
        if graph.node(child).flags.contains(StateFlags::TRUE_SHARED) {
            graph.node_mut(state).flags.insert(StateFlags::TRUE_SHARED);
        }
        if grandchild.is_none() {
            break;
        }
    }

    if !graph.node(state).flags.contains(StateFlags::TRUE_SHARED) {
        normalize_true_inner(graph, graph.node(state).true_link, seen);
    }
    if !graph.node(state).flags.contains(StateFlags::FALSE_SHARED) {
        normalize_true_inner(graph, graph.node(state).false_link, seen);
    }
}

pub fn normalize_false(graph: &mut StateGraph, state: Option<NodeId>) {
    let mut seen = BTreeSet::new();
    normalize_false_inner(graph, state, &mut seen);
}

fn normalize_false_inner(
    graph: &mut StateGraph,
    state: Option<NodeId>,
    seen: &mut BTreeSet<NodeId>,
) {
    let Some(state) = state else {
        return;
    };
    if !seen.insert(state) {
        return;
    }

    while let Some(child) = graph.node(state).false_link {
        if !is_glue_branch(graph, child) {
            break;
        }
        if graph.node(child).true_link.is_none() && graph.node(child).false_link.is_none() {
            let fresh = graph.add_node(StateNode::default());
            graph.change_false_link(state, Some(fresh));
            break;
        }

        let replacement = graph.node(child).false_link.or(graph.node(child).true_link);
        if replacement == Some(child) || replacement == graph.node(state).false_link {
            break;
        }
        graph.change_false_link(state, replacement);
        if graph.node(child).flags.contains(StateFlags::TRUE_SHARED) {
            graph.node_mut(state).flags.insert(StateFlags::FALSE_SHARED);
        }
    }

    if !graph.node(state).flags.contains(StateFlags::TRUE_SHARED) {
        normalize_false_inner(graph, graph.node(state).true_link, seen);
    }
    if !graph.node(state).flags.contains(StateFlags::FALSE_SHARED) {
        normalize_false_inner(graph, graph.node(state).false_link, seen);
    }
}

fn is_glue_branch(graph: &StateGraph, id: NodeId) -> bool {
    graph.node(id).kind == StateKind::Branch && graph.node(id).payload == 0
}

pub fn splice_states(
    graph: &StateGraph,
    state: Option<NodeId>,
    anchor: &str,
) -> Result<(), IntegrateError> {
    let mut seen = BTreeSet::new();
    splice_states_inner(graph, state, anchor, &mut seen)
}

fn splice_states_inner(
    graph: &StateGraph,
    state: Option<NodeId>,
    anchor: &str,
    seen: &mut BTreeSet<NodeId>,
) -> Result<(), IntegrateError> {
    let Some(state) = state else {
        return Ok(());
    };
    if !seen.insert(state) {
        return Ok(());
    }

    let mut cursor = state;
    let mut cursor_seen = BTreeSet::new();
    loop {
        if !cursor_seen.insert(cursor) {
            break;
        }
        if graph.node(cursor).flags.contains(StateFlags::FALSE_SHARED) {
            break;
        }

        let next = if graph.node(cursor).kind == StateKind::Branch {
            graph.node(cursor).true_link
        } else {
            graph.node(cursor).false_link
        };
        let Some(next) = next else {
            break;
        };

        if graph.node(state).kind != StateKind::Branch
            && compare_trees(graph, Some(state), Some(next)) != TreeComparison::Different
        {
            return Err(IntegrateError::AmbiguousStateGraph {
                anchor: anchor.to_string(),
            });
        }

        cursor = next;
    }

    if !graph.node(state).flags.contains(StateFlags::FALSE_SHARED)
        && graph.node(state).kind != StateKind::Branch
    {
        splice_states_inner(graph, graph.node(state).false_link, anchor, seen)?;
    }

    if !graph.node(state).flags.contains(StateFlags::TRUE_SHARED) {
        splice_states_inner(graph, graph.node(state).true_link, anchor, seen)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrate_prepends_uncombined_root_and_marks_original_integrated() {
        let mut graph = StateGraph::new();
        let root = graph.add_node(StateNode::emit(1));

        let result = integrate(&mut graph, Some(root), "sample", OptLevel::O0)
            .expect("integrate should succeed");

        assert_eq!(result, Some(root));
        assert_eq!(graph.global_roots(), vec![root]);
        assert!(graph.node(root).flags.contains(StateFlags::INTEGRATED));
    }

    #[test]
    fn normalize_false_replaces_leaf_branch_with_fresh_node() {
        let mut graph = StateGraph::new();
        let root = graph.add_node(StateNode::emit(1));
        let leaf_branch = graph.add_node(StateNode::default());
        graph.add_false_link(root, leaf_branch);

        normalize_false(&mut graph, Some(root));

        let replacement = graph.node(root).false_link.expect("fresh false node");
        assert_ne!(replacement, leaf_branch);
        assert_eq!(graph.node(replacement).kind, StateKind::Branch);
    }
}
