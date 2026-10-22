use super::*;

fn dag() -> SelectionDAG {
    SelectionDAG::new(DagConfig::default())
}

fn arg(
    dag: &mut SelectionDAG,
    vt: EVT,

mod get_node;
mod type_actions;
mod action_table;
mod builder;
mod legalize_types;
mod legalize_dag;
mod expand_node;
mod x86_conditions;
mod setcc_result;
mod switches;
mod global_address;
mod combiner;
mod far_load;
mod switch_dags;
mod i1_branch;
mod wide_shifts;
