//! `int f(int a, int b) { return a + b; }` from its built DAG through the
//! shared stages to scheduled machine nodes, against the pin
//! (`llc -mtriple=m68k-linux-gnu -mcpu=M68000 -O2 -stop-after=finalize-isel`:
//! `MOV32rp` of the first argument, `ADD32dp` folding the second, `RET`).

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{DagConfig, SelectionDAG};
use llrm_m68k::dagisel::call::{Ext, In, M68kCalling, Out};
use llrm_m68k::dagisel::generated::m68k_m68000::MACHINE_OPCODES;
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::pipeline::select_block;

fn machine_names(
    dag: &SelectionDAG,
    nodes: impl IntoIterator<Item = u32>,
) -> Vec<&'static str> {
    nodes
        .into_iter()
        .filter_map(|node| match dag.nodes()[node as usize].opcode {
            ISD::Machine(code) => MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, name)| *name),
            _ => None,
        })
        .collect()
}

#[test]
fn an_add_of_two_stack_arguments_is_a_move_and_an_add_with_a_memory_operand() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let calling = M68kCalling::default();
    let mut fixed = {
        let mut made = 0;
        move |_size: u32, _offset: i64, _immutable: bool| {
            made += 1;
            -made
        }
    };
    let ins = [In { vt: EVT::I32, pointer: false, arg_vt: EVT::I32, split: false }, In { vt: EVT::I32, pointer: false, arg_vt: EVT::I32, split: false }];
    let args = calling.lower_formal_arguments(&mut dag, entry, &ins, &mut fixed);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[args[0], args[1]]);
    let ret = calling.lower_return(&mut dag, entry, &[Out { value: sum, ext: Ext::Any, pointer: false, arg_vt: EVT::I32, split: false, variadic: false }]);
    dag.set_root(ret);

    let units = select_block(&mut dag, Cpu::M68000).expect("selects");

    let names = machine_names(&dag, units.iter().flat_map(|unit| unit.nodes.iter().copied()));
    assert_eq!(names, ["MOV32rp", "ADD32dp", "RET"]);
}
