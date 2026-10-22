use super::*;

fn dag() -> SelectionDAG {
    SelectionDAG::new(DagConfig::default())
}

fn arg(
    dag: &mut SelectionDAG,
    vt: EVT,
    reg: u32,
) -> SDValue {
    dag.get_register(vt, reg)
}

mod action_table;
mod builder;
mod combiner;
mod expand_node;
mod far_load;
mod get_node;
mod global_address;
mod legalize_dag;
mod legalize_types;
mod setcc_result;
mod switch_dags;
mod switches;
mod type_actions;
mod wide_shifts;
mod x86_conditions;
