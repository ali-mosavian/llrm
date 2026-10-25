use super::super::dagisel_adapter::DagisAdapter;
use super::*;

/// The adapter reads users from the DAG's use lists, not a scan of every node
/// (a scan was 20% of d_faces' dag isel instructions): a node given new
/// operands, and one a replacement redirected, must still be found there.
#[test]
fn a_user_made_by_a_replacement_or_new_operands_is_a_live_user() {
    let mut dag = dag();
    let [a, b, c, d] = [100, 101, 102, 103].map(|j| dag.get_register(EVT::I32, j));
    let add = dag.get_node(ISD::Add, EVT::I32, &[a, a]);
    let sub = dag.get_node(ISD::Sub, EVT::I32, &[a, b]);
    let root = dag.get_node(ISD::Xor, EVT::I32, &[add, sub]);
    dag.set_root(root);
    dag.update_node_operands(add.node, &[c, c]);
    dag.replace_all_uses_with(b, d);
    let adapter = DagisAdapter::new(&mut dag);
    assert!(adapter.has_live_users(c.node), "the operands set later");
    assert!(adapter.has_live_users(d.node), "the replacement");
    assert!(adapter.has_live_users(a.node), "the user left");
}
