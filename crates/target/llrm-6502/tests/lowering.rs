//! The 6502 action table on hand-built DAGs. Expected shapes are worked out by hand: an `i32`
//! add is a word add with carry out (`UADDO`) and a word add with carry in (`UADDO_CARRY`).

use llrm_6502::dagisel::lowering::target_lowering;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::legalize_types::TypeLegalizer;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{Action, DagConfig, MemBase, MemInfo, SDValue, SelectionDAG};

/// A value from a stack slot: a register node has no result type to expand.
fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I16, 7);
    let chain = dag.entry_node();
    let info = MemInfo { mem_vt: vt, align: 1, volatile: false, base: MemBase::Stack(slot), ext: None, space: 0 };
    dag.get_load(vt, chain, ptr, info)
}

/// Before the 6502 had a table, an `i32` add had nowhere to go: no type was legal. The halves of the
/// add are `UADDO` on the low words and `UADDO_CARRY` on the high words, joined by the carry (result 1).
#[test]
fn an_i32_add_is_a_word_uaddo_and_a_word_uaddo_carry() {
    let tli = target_lowering();
    let mut dag = SelectionDAG::new(DagConfig::default());
    let [a, b] = [1, 2].map(|slot| load(&mut dag, EVT::I32, slot));
    let add = dag.get_node(ISD::Add, EVT::I32, &[a, b]);
    let low = dag.get_node(ISD::Truncate, EVT::I16, &[add]);
    dag.set_root(low);

    TypeLegalizer::new(&mut dag, &tli).run().expect("legalizes");

    let (lo, hi) = dag.expanded_integers[&add];
    assert_eq!((dag.opcode(lo), dag.value_type(lo)), (ISD::UAddO, EVT::I16));
    assert_eq!((dag.opcode(hi), dag.value_type(hi)), (ISD::UAddOCarry, EVT::I16));
    let carry = dag.operand(hi, 2);
    assert_eq!((carry.node, carry.res), (lo.node, 1));
}

/// No multiply or divide exists on the 6502: a library call, at every width. A shift is custom: an
/// instruction for a constant count, refused otherwise.
#[test]
fn multiply_and_divide_are_library_calls_and_shifts_are_custom() {
    let tli = target_lowering();
    for (ops, action) in [
        (&[ISD::Mul, ISD::SDiv, ISD::UDiv, ISD::SRem, ISD::URem][..], Action::LibCall),
        (&[ISD::Shl, ISD::Srl, ISD::Sra][..], Action::Custom),
    ] {
        for &op in ops {
            for vt in [EVT::I8, EVT::I16] {
                assert_eq!(tli.operation_action(op, vt), action, "{op:?} {vt:?}");
            }
        }
    }
}
