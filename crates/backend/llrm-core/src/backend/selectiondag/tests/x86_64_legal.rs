//! The m64 type and operation tables (D64-2).

use super::super::legalize_dag::DagLegalizer;
use super::super::legalize_types::TypeLegalizer;
use super::super::x86_lowering::X86Lowering;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn legalized_add(tli: &TargetLowering) -> (SelectionDAG, SDValue) {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let chain = dag.entry_node();
    let load = |dag: &mut SelectionDAG, reg| {
        let ptr = dag.get_register(EVT::I64, reg);
        dag.get_load(
            EVT::I64,
            chain,
            ptr,
            MemInfo { mem_vt: EVT::I64, align: 1, volatile: false, base: MemBase::Stack(1), ext: None, space: 0 },
        )
    };
    let (a, b) = (load(&mut dag, 7), load(&mut dag, 8));
    let sum = dag.get_node(ISD::Add, EVT::I64, &[a, b]);
    dag.set_root(sum);
    TypeLegalizer::new(&mut dag, tli).run().expect("types legalize");
    DagLegalizer::new(&mut dag, tli, &mut X86Lowering { cmov: true }).run().expect("legalizes");
    let root = dag.get_root();
    (dag, root)
}

/// m64 had no table: an `i64` add expanded to a pair of `i32` adds with a
/// carry, as at 32 bits, instead of staying one `ADD`.
#[test]
fn an_i64_add_stays_one_add() {
    let tli = TargetLowering::x86_64(Features { cmov: true, ..Features::default() }, MVT::I64);
    let (dag, root) = legalized_add(&tli);
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::Add, EVT::I64));
}

/// `select c, a, b` over `i64` is a `CMOV` on `i64` at 64 bits (`LowerSELECT`).
#[test]
fn an_i64_select_is_a_cmov() {
    use super::super::x86_lowering::x86isd;
    let tli = TargetLowering::x86_64(Features { cmov: true, ..Features::default() }, MVT::I64);
    let mut dag = SelectionDAG::new(DagConfig::default());
    let reg = |dag: &mut SelectionDAG, vt, n| dag.get_register(vt, n);
    let (c, a, b) = (reg(&mut dag, EVT::I8, 1), reg(&mut dag, EVT::I64, 2), reg(&mut dag, EVT::I64, 3));
    let select = dag.get_node(ISD::Select, EVT::I64, &[c, a, b]);
    dag.set_root(select);
    TypeLegalizer::new(&mut dag, &tli).run().expect("types legalize");
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: true }).run().expect("legalizes");
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (x86isd::CMOV, EVT::I64));
}

/// The builder had no m64 target: it would size pointers at 32 bits and treat
/// `i64` as too wide for a register.
#[test]
fn the_m64_builder_target_has_64_bit_pointers_and_registers() {
    let target = crate::backend::selectiondag::builder::BuilderTarget::m64();
    assert_eq!((target.dag.pointer, target.legal_ints.last().copied()), (EVT::I64, Some(64)));
}
