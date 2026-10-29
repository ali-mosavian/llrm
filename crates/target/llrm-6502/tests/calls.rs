//! The 6502 call, argument and return lowering on hand-built DAGs. Expected registers are worked
//! out by hand from `machines/calling.toml`: a byte takes the first free byte from rc2, a word the
//! first free pair, a byte result leaves in `a`, a word in `a:x`.

use llrm_6502::dagisel::call::{CALL, In, Out, RET_GLUE, lower_call, lower_formal_arguments, lower_return};
use llrm_6502::reg;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{DagConfig, NodeData, SDValue, SelectionDAG};
use llrm_lir::registers::RegId;

fn phys(
    dag: &SelectionDAG,
    node: SDValue,
) -> RegId {
    match dag.node(node).data {
        NodeData::PhysReg(register) => register,
        _ => panic!("not a physical register"),
    }
}

/// `f(i8 a, i16 b)` returning `i16`: `a` goes to rc2, `b` to the first free pair, rc4:rc5 (rs2);
/// the word comes back as `a` (low) and `x` (high) joined.
#[test]
fn a_call_passes_a_byte_in_rc2_and_a_word_in_rs2() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a = dag.get_register(EVT::I8, 1);
    let b = dag.get_register(EVT::I16, 2);
    let callee = dag.get_global_address(EVT::I16, 7, 0);

    let (chain, values) =
        lower_call(&mut dag, entry, callee, &[Out { value: a }, Out { value: b }], &[In { vt: EVT::I16 }]).unwrap();

    assert_eq!(dag.opcode(values[0]), ISD::BuildPair);
    let (lo, hi) = (dag.operand(values[0], 0), dag.operand(values[0], 1));
    assert_eq!((phys(&dag, dag.operand(lo, 1)), phys(&dag, dag.operand(hi, 1))), (reg::A, reg::X));
    // the chain runs back through the copy of `a` to the end of the call sequence
    let end = dag.operand(dag.operand(chain, 0), 0);
    assert_eq!(dag.opcode(end), ISD::CallSeqEnd);
    let call = dag.operand(end, 0);
    assert_eq!(dag.opcode(call), CALL);
    assert_eq!(dag.opcode(dag.operand(call, 1)), ISD::TargetGlobalAddress);
    // chain, callee, then the two argument registers in order
    assert_eq!((phys(&dag, dag.operand(call, 2)), phys(&dag, dag.operand(call, 3))), (reg::RC2, reg::RS2));
}

/// Three words fit rc2..rc7; the fourth fits no register and is stored on the soft stack at `rs0 +
/// 0` inside a call sequence of two bytes.
#[test]
fn a_fourth_word_is_stored_on_the_soft_stack() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let words: Vec<Out> = (1..=4).map(|n| Out { value: dag.get_register(EVT::I16, n) }).collect();
    let callee = dag.get_global_address(EVT::I16, 7, 0);

    let (chain, _) = lower_call(&mut dag, entry, callee, &words, &[]).unwrap();

    assert_eq!((dag.opcode(chain), dag.constant(dag.operand(chain, 1))), (ISD::CallSeqEnd, Some(2)));
    let stores: Vec<SDValue> =
        (0..dag.node_count() as u32).map(|node| SDValue { node, res: 0 }).filter(|&v| dag.opcode(v) == ISD::Store).collect();
    assert_eq!(stores.len(), 1);
    assert_eq!(dag.operand(stores[0], 1), words[3].value);
    // the address is `rs0` (plus the offset 0, which may fold away)
    let address = dag.operand(stores[0], 2);
    let base = if dag.opcode(address) == ISD::Add { dag.operand(address, 0) } else { address };
    assert_eq!((dag.opcode(base), phys(&dag, dag.operand(base, 1))), (ISD::CopyFromReg, reg::RS0));
}

/// A word that reaches the callee on the stack is loaded from a fixed object of two bytes at the
/// offset the convention gave it.
#[test]
fn a_fourth_formal_word_is_loaded_from_a_fixed_object() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let mut created = Vec::new();
    let mut fixed = |size: u32, offset: i64, immutable: bool| {
        created.push((size, offset, immutable));
        created.len() as i32
    };

    let args = lower_formal_arguments(&mut dag, entry, &[In { vt: EVT::I16 }; 4], &mut fixed).unwrap();

    assert_eq!(created, [(2, 0, true)]);
    assert_eq!((dag.opcode(args[3]), dag.value_type(args[3])), (ISD::Load, EVT::I16));
    assert_eq!(dag.opcode(args[0]), ISD::CopyFromReg);
}

/// `g(i8, i16)` reads rc2 and rs2.
#[test]
fn formal_arguments_are_read_from_rc2_and_rs2() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();

    let args = lower_formal_arguments(&mut dag, entry, &[In { vt: EVT::I8 }, In { vt: EVT::I16 }], &mut |_, _, _| 0).unwrap();

    assert_eq!((phys(&dag, dag.operand(args[0], 1)), phys(&dag, dag.operand(args[1], 1))), (reg::RC2, reg::RS2));
    assert_eq!((dag.value_type(args[0]), dag.value_type(args[1])), (EVT::I8, EVT::I16));
}

/// A returned byte is copied to `a` and the return follows the copy, glued to it.
#[test]
fn a_returned_byte_is_copied_to_a() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let v = dag.get_register(EVT::I8, 1);

    let ret = lower_return(&mut dag, entry, &[Out { value: v }]).unwrap();

    assert_eq!(dag.opcode(ret), RET_GLUE);
    assert_eq!(phys(&dag, dag.operand(ret, 1)), reg::A);
    let copy = dag.operand(ret, 0);
    assert_eq!((dag.opcode(copy), phys(&dag, dag.operand(copy, 1))), (ISD::CopyToReg, reg::A));
}
