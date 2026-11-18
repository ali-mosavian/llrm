//! A function was selected twice, once per DAG scheduler (`Source`, `ILP`),
//! though the DAG up to the scheduler is the same: half of
//! `select_function_with`, 9% of switch_jump_table -O2. The two forms share one
//! DAG, and the scheduler leaves it as it found it, so scheduling it again
//! gives the same units.

use llrm_codegen::backend::selectiondag::schedule::schedule;
use llrm_z80::dagisel::pipeline::{ILP, Z80Sched};
use llrm_z80::driver::{Level, Request, selections_both};

const IR: &str = include_str!("kernels/ir/lz4.ll");

#[test]
fn both_schedules_come_from_one_dag_and_rescheduling_it_changes_nothing() {
    let request = Request::new(Level::O2);
    let both = selections_both(IR, &request).expect("selects");
    let mut differ = false;
    for (name, source, ilp) in both {
        let (source, ilp) = (source.expect("source form"), ilp.expect("ilp form"));
        for (one, other) in source.blocks.iter().zip(&ilp.blocks) {
            assert!(std::sync::Arc::ptr_eq(&one.dag, &other.dag), "{name}: the forms share one DAG");
            differ |= one.units != other.units;
        }
        let want: Vec<_> = ilp.blocks.iter().map(|block| block.units.clone()).collect();
        drop(ilp);
        let mut source = source;
        for (block, ilp_units) in source.blocks.iter_mut().zip(want) {
            let dag = std::sync::Arc::get_mut(&mut block.dag).expect("the one owner");
            let root = dag.get_root();
            for (ilp, expected) in [(false, block.units.clone()), (true, ilp_units)] {
                ILP.with(|one| one.set(ilp));
                let again = schedule(dag, root, &Z80Sched);
                ILP.with(|one| one.set(false));
                assert_eq!(again, expected, "{name}: scheduling the selected DAG again (ilp {ilp})");
            }
        }
    }
    assert!(differ, "the ILP schedule is a different order somewhere (lz4's copy loops)");
}

/// The selector's table was scanned for every block (4% of switch_jump_table
/// -O2).
#[test]
fn a_selector_is_built_once_per_cpu() {
    use llrm_z80::dagisel::hooks::Cpu;
    use llrm_z80::dagisel::pipeline::selector;
    assert!(std::ptr::eq(selector(Cpu::Z80), selector(Cpu::Z80)), "one selector for the Z80");
    assert!(std::ptr::eq(selector(Cpu::Z180), selector(Cpu::Z180)), "one selector for the Z180");
    assert!(!std::ptr::eq(selector(Cpu::Z80), selector(Cpu::Z180)), "each CPU has its own table");
}
