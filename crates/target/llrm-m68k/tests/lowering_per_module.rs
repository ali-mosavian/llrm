//! `select_block_with` built the whole lowering table (`M68kTargetLowering`'s
//! constructor: 4.9 M instructions over 36 blocks of grep at -O2) for every
//! block; the pin builds it once per subtarget.

use llrm_m68k::dagisel::pipeline::{lowerings_built, select_block};

#[test]
fn blocks_of_one_cpu_share_a_lowering_table() {
    use llrm_codegen::backend::selectiondag::{DagConfig, SelectionDAG};
    let before = lowerings_built();
    for _ in 0..20 {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let _ = select_block(&mut dag, llrm_m68k::dagisel::hooks::Cpu::M68020);
    }
    let built = lowerings_built() - before;
    assert!(built <= 1, "{built} lowering tables built for 20 blocks");
}
