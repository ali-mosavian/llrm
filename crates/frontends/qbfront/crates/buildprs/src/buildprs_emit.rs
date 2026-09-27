//! Emit `tState` bytes from a laid-out graph work list.

use crate::buildprs_graph::{GraphEncodeError, StateGraph};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmitGraphError {
    OffsetMismatch { expected: usize, actual: usize },
    Encode(GraphEncodeError),
}

impl std::fmt::Display for EmitGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OffsetMismatch { expected, actual } => {
                write!(
                    f,
                    "work-list offset mismatch: expected {expected}, actual {actual}"
                )
            }
            Self::Encode(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for EmitGraphError {}

impl From<GraphEncodeError> for EmitGraphError {
    fn from(error: GraphEncodeError) -> Self {
        Self::Encode(error)
    }
}

pub fn out_state(graph: &StateGraph) -> Result<Vec<u8>, EmitGraphError> {
    let mut bytes = Vec::new();
    for id in graph.work_items() {
        if graph.node(id).sort_index != bytes.len() {
            return Err(EmitGraphError::OffsetMismatch {
                expected: graph.node(id).sort_index,
                actual: bytes.len(),
            });
        }
        bytes.extend(graph.encode_node(id)?);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use crate::buildprs_graph::StateNode;
    use crate::buildprs_layout::sort_state_values;

    use super::*;

    #[test]
    fn out_state_emits_work_list_bytes_in_layout_order() {
        let mut graph = StateGraph::new();
        let root = graph.add_node(StateNode::branch_node(5));
        let accept = graph.add_node(StateNode::accept());
        graph.add_true_link(root, accept);
        graph.prepend_global(root);
        sort_state_values(&mut graph);

        assert_eq!(
            out_state(&graph).expect("emit should succeed"),
            vec![5, 0, 0]
        );
    }
}
