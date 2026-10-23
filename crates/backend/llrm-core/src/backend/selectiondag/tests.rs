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
mod and_fold;
mod bit_counts;
mod builder;
mod combiner;
mod expand_node;
mod extract_element;
mod wide_compare;
mod far_load;
mod float_constants;
mod fp_to_int;
mod get_node;
mod global_address;
mod i1_branch;
mod int_to_fp;
mod legalize_dag;
mod legalize_types;
mod m16_coverage;
mod pair_arith;
mod pair_mul;
mod setcc_result;
mod switch_dags;
mod switches;
mod type_actions;
mod vastart;
mod wide_counts;
mod wide_division;
mod wide_shifts;
mod x86_conditions;
mod x86_preprocess;
mod x87_goldens;
