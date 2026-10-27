use std::time::{Duration, Instant};

use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

/// Each visit queued its operands again, so a DAG whose nodes share operands
/// was walked once per path, not once per node (`CombinedNodes`): a chain of
/// `add (n-1), (n-2)` has fib(n) paths, and QCport's `_scr_draw_hud` (a chain
/// of calls sharing their chain and register nodes) never finished at -O2.
#[test]
fn a_dag_with_shared_operands_is_combined_once_per_node() {
    // node k = add (node k+1), (node k+2): the operands are the newer nodes
    // (what a replacement makes), so a visit finds them already visited.
    const N: usize = 40;
    let mut dag = dag();
    let placeholders: Vec<SDValue> = (0..N + 3).map(|j| dag.get_register(EVT::I32, 100 + j as u32)).collect();
    let nodes: Vec<SDValue> =
        (0..N).map(|k| dag.get_node(ISD::Add, EVT::I32, &[placeholders[k + 1], placeholders[k + 2]])).collect();
    for j in 1..N {
        dag.replace_all_uses_with(placeholders[j], nodes[j]);
    }
    dag.set_root(nodes[0]);
    let start = Instant::now();
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
}
