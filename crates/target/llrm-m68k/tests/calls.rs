//! The m68k call, argument and return lowering on hand-built DAGs, against the
//! pin's initial DAG (`llc -mtriple=m68k-linux-gnu -mcpu=M68000
//! -relocation-model=static -debug-only=isel`, "Initial selection DAG").

use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_codegen::backend::selectiondag::mvt::EVT;
use llrm_codegen::backend::selectiondag::{DagConfig, NodeData, SDValue, SelectionDAG};
use llrm_m68k::dagisel::call::{Ext, In, M68kCalling, Out};
use llrm_m68k::dagisel::lowering::m68kisd;
use llrm_m68k::reg;

fn out(
    value: SDValue,
    pointer: bool,
) -> Out {
    Out { value, ext: Ext::Any, pointer, arg_vt: EVT::I32, split: false }
}

fn register_of(
    dag: &SelectionDAG,
    node: SDValue,
) -> llrm_lir::registers::RegId {
    match dag.node(node).data {
        NodeData::PhysReg(register) => register,
        _ => panic!("not a physical register"),
    }
}

/// The pin lowers `g(i32 %a, i8 %b)` to `callseq_start 8`, `store %a` at the
/// stack pointer, `store (any_extend %b)` at `sp + 4`, `CALL` and `d0` read
/// back. Without this lowering there was no m68k call at all.
#[test]
fn a_call_stores_each_argument_in_a_four_byte_slot() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let a = dag.get_register(EVT::I32, 1);
    let b = dag.get_register(EVT::I8, 2);
    let callee = dag.get_global_address(EVT::I32, 7, 0);

    let (chain, values) = M68kCalling::default().lower_call(
        &mut dag,
        entry,
        callee,
        &[out(a, false), out(b, false)],
        &[In { vt: EVT::I32, pointer: false, arg_vt: EVT::I32, split: false }],
    );

    let end = dag.operand(chain, 0);
    assert_eq!(dag.opcode(chain), ISD::CopyFromReg);
    assert_eq!(dag.opcode(end), ISD::CallSeqEnd);
    assert_eq!(dag.constant(dag.operand(end, 1)), Some(8));
    let call = dag.operand(end, 0);
    assert_eq!(dag.opcode(call), m68kisd::CALL);
    assert_eq!(dag.opcode(dag.operand(call, 1)), ISD::TargetGlobalAddress);
    let stores = dag.node(dag.operand(call, 0)).ops.clone();
    assert_eq!(stores.len(), 2);
    let start = dag.operand(stores[0], 0);
    assert_eq!((dag.opcode(start), dag.constant(dag.operand(start, 1))), (ISD::CallSeqStart, Some(8)));
    // arg 0 at the stack pointer itself, arg 1 widened to 32 bits at sp + 4
    assert_eq!(dag.operand(stores[0], 1), a);
    assert_eq!(dag.opcode(dag.operand(stores[0], 2)), ISD::CopyFromReg);
    let widened = dag.operand(stores[1], 1);
    assert_eq!((dag.opcode(widened), dag.value_type(widened)), (ISD::AnyExtend, EVT::I32));
    let address = dag.operand(stores[1], 2);
    assert_eq!((dag.opcode(address), dag.constant(dag.operand(address, 1))), (ISD::Add, Some(4)));
    assert_eq!(register_of(&dag, dag.operand(chain, 1)), reg::D0);
    assert_eq!(values.len(), 1);
}

/// `RetCC_M68k_C` puts a pointer in `a0` and an integer by its width in `d0`,
/// `wd0` or `bd0`. In the DAG both a pointer and an `i32` are `i32`, so only
/// the pointer flag tells them apart; a lowering that ignored it read `d0`
/// for `ptr @h(ptr)`.
#[test]
fn a_pointer_result_is_in_a0_and_an_integer_by_its_width() {
    let calling = M68kCalling::default();
    for (vt, pointer, expected) in
        [(EVT::I32, true, reg::A0), (EVT::I32, false, reg::D0), (EVT::I16, false, reg::WD0), (EVT::I8, false, reg::BD0)]
    {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let entry = dag.entry_node();
        let callee = dag.get_global_address(EVT::I32, 7, 0);
        let (chain, _) = calling.lower_call(&mut dag, entry, callee, &[], &[In { vt, pointer, arg_vt: vt, split: false }]);
        assert_eq!(register_of(&dag, dag.operand(chain, 1)), expected, "{vt:?} pointer={pointer}");
    }
}

/// The pin loads `i32 %a` from `FrameIndex<-1>` and `i8 %b` from
/// `FrameIndex<-2>`; the byte sits at the high-address end of its slot
/// (offset 4 + 3) because m68k is big-endian.
#[test]
fn formal_arguments_are_loaded_from_their_slots() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let entry = dag.entry_node();
    let mut objects = Vec::new();
    let mut fixed = |size: u32, offset: i64, _immutable: bool| {
        objects.push((size, offset));
        -(objects.len() as i32)
    };

    let values = M68kCalling::default().lower_formal_arguments(
        &mut dag,
        entry,
        &[In { vt: EVT::I32, pointer: false, arg_vt: EVT::I32, split: false }, In { vt: EVT::I8, pointer: false, arg_vt: EVT::I8, split: false }],
        &mut fixed,
    );

    assert_eq!(objects, [(4, 0), (1, 7)]);
    assert_eq!(values.iter().map(|&v| dag.opcode(v)).collect::<Vec<_>>(), [ISD::Load, ISD::Load]);
    assert_eq!(values.iter().map(|&v| dag.value_type(v)).collect::<Vec<_>>(), [EVT::I32, EVT::I8]);
    assert!(matches!(dag.node(dag.operand(values[1], 1)).data, NodeData::FrameIndex(-2)));
}

/// The pin returns `i32` as `RET chain, TargetConstant<0>, Register $d0, glue`
/// after `CopyToReg $d0`; a pointer goes to `$a0`.
#[test]
fn a_return_copies_the_value_to_its_register() {
    let calling = M68kCalling::default();
    for (pointer, expected) in [(false, reg::D0), (true, reg::A0)] {
        let mut dag = SelectionDAG::new(DagConfig::default());
        let entry = dag.entry_node();
        let value = dag.get_register(EVT::I32, 1);

        let ret = calling.lower_return(&mut dag, entry, &[out(value, pointer)]);

        assert_eq!(dag.opcode(ret), m68kisd::RET);
        let copy = dag.operand(ret, 0);
        assert_eq!(dag.opcode(copy), ISD::CopyToReg);
        assert_eq!(register_of(&dag, dag.operand(copy, 1)), expected);
        assert_eq!(dag.constant(dag.operand(ret, 1)), Some(0));
        assert_eq!(register_of(&dag, dag.operand(ret, 2)), expected);
        assert_eq!(dag.node(ret).ops.len(), 4, "chain, pop, register, glue");
    }
}

/// With a static reference the pin wraps the address: `WrapperPC
/// (TargetGlobalAddress)`, so the selector emits `LEA32q`.
#[test]
fn a_global_address_is_a_pc_relative_wrapper() {
    let mut dag = SelectionDAG::new(DagConfig::default());
    let address = dag.get_global_address(EVT::I32, 3, 0);

    let wrapped = llrm_m68k::dagisel::call::lower_global_address(&mut dag, address);

    assert_eq!(dag.opcode(wrapped), m68kisd::WRAPPER_PC);
    assert_eq!(dag.opcode(dag.operand(wrapped, 0)), ISD::TargetGlobalAddress);
}
