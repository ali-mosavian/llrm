use super::super::actions::LoadExt;
use super::super::dag_combiner::{CombineLevel, NoCombines, combine};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    volatile: bool,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info =
        MemInfo { mem_vt: EVT::I8, align: 1, volatile, base: MemBase::Stack(1), ext: None, space: 0, ordering: None };
    dag.get_load(EVT::I8, chain, ptr, info)
}

fn folded(
    op: ISD,
    volatile: bool,
) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let byte = load(&mut dag, volatile);
    let wide = dag.get_node(op, EVT::I32, &[byte]);
    dag.set_root(wide);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let root = dag.get_root();
    (dag, root)
}

fn ext_of(
    dag: &SelectionDAG,
    load: SDValue,
) -> (EVT, EVT, Option<LoadExt>) {
    let NodeData::Mem(info) = dag.node(load).data else { panic!("a load") };
    (dag.value_type(load), info.mem_vt, info.ext)
}

/// `(zext (load i8 p))` stayed a load and a `zext`, so selection emitted
/// `mov al, [p]` and `movzx`, where the pin has one `movzx eax, byte [p]`
/// (10 pin-only MOVZX/MOVSX rows of the m32 shadow).
#[test]
fn an_extension_of_a_load_is_an_extending_load() {
    for (op, ext) in
        [(ISD::ZeroExtend, LoadExt::ZExtLoad), (ISD::SignExtend, LoadExt::SExtLoad), (ISD::AnyExtend, LoadExt::ExtLoad)]
    {
        let (dag, root) = folded(op, false);
        assert_eq!(dag.opcode(root), ISD::Load, "{op:?}");
        assert_eq!(ext_of(&dag, root), (EVT::I32, EVT::I8, Some(ext)), "{op:?}");
    }
}

/// With no target table nothing says the extending load is legal: a volatile
/// load keeps its width and the extension stays a node.
#[test]
fn a_volatile_load_is_not_extended_without_a_target_table() {
    let (dag, root) = folded(ISD::ZeroExtend, true);
    assert_eq!(dag.opcode(root), ISD::ZeroExtend);
}

/// The load's chain users follow the new load, or the load could be
/// scheduled after what it must precede.
#[test]
fn the_chain_of_the_old_load_moves_to_the_extending_load() {
    let mut dag = dag();
    let byte = load(&mut dag, false);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[byte]);
    let chain = SDValue { node: byte.node, res: 1 };
    let ptr = dag.get_register(EVT::I32, 8);
    let info = MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Stack(2),
        ext: None,
        space: 0,
        ordering: None,
    };
    let store = dag.get_store(chain, wide, ptr, info);
    dag.set_root(store);
    combine(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes);
    let root = dag.get_root();
    let ordering = dag.operand(root, 0);
    assert_eq!((dag.opcode(ordering), ordering.res), (ISD::Load, 1));
    assert_eq!(ext_of(&dag, SDValue { node: ordering.node, res: 0 }).2, Some(LoadExt::ZExtLoad));
}

/// A word load read by a sign extension and by a compare against a constant
/// (`if (x < 0) ... (int)x`): the pin makes one `movsx` load and compares it
/// (`pin only MOVSX32rm16`, R_PFLAG_CLEAR of the m32 shadow); ours kept the
/// word load, a `movsx` of it and a word compare.
fn shared(
    op: ISD,
    cc: CondCode,
    space: u32,
) -> (SelectionDAG, SDValue, SDValue) {
    use super::super::dag_combiner::DagCombiner;
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info = MemInfo {
        mem_vt: EVT::I16,
        align: 2,
        volatile: false,
        base: MemBase::Stack(1),
        ext: None,
        space,
        ordering: None,
    };
    let word = dag.get_load(EVT::I16, chain, ptr, info);
    let wide = dag.get_node(op, EVT::I32, &[word]);
    let five = dag.get_constant(5, EVT::I16);
    let test = dag.get_setcc(EVT::I8, word, five, cc);
    let test_wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[test]);
    let both = dag.get_node(ISD::Add, EVT::I32, &[wide, test_wide]);
    dag.set_root(both);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root, word)
}

#[test]
fn a_compare_of_the_loaded_value_follows_the_extending_load() {
    let (dag, root, _) = shared(ISD::SignExtend, CondCode::SetLt, 0);
    // add (sextload), (zext (setcc (sextload), 5))
    let (extended, wide_test) = (dag.operand(root, 0), dag.operand(root, 1));
    assert_eq!(ext_of(&dag, extended), (EVT::I32, EVT::I16, Some(LoadExt::SExtLoad)));
    let test = dag.operand(wide_test, 0);
    assert_eq!(dag.opcode(test), ISD::SetCC);
    assert_eq!(dag.operand(test, 0), extended, "the compare reads the extending load");
    assert_eq!((dag.value_type(dag.operand(test, 1)), dag.constant(dag.operand(test, 1))), (EVT::I32, Some(5)));
}

/// A zero extension cannot serve a signed compare: the sign bit is gone.
#[test]
fn a_signed_compare_keeps_the_word_load_under_a_zero_extension() {
    let (dag, root, word) = shared(ISD::ZeroExtend, CondCode::SetLt, 0);
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::ZeroExtend);
    assert_eq!(dag.operand(dag.operand(root, 0), 0), word);
}

/// An address space (a segment override) is the load's own: the extending
/// load keeps it.
#[test]
fn the_extending_load_keeps_the_address_space() {
    let (dag, root, _) = shared(ISD::SignExtend, CondCode::SetLt, 257);
    let extended = dag.operand(root, 0);
    let NodeData::Mem(info) = dag.node(extended).data else { panic!("a load") };
    assert_eq!(info.space, 257);
}

/// `load volatile i8` + `zext`: the pin's `tryToFoldExtOfLoad` makes the
/// extending load (the memory operand stays volatile) where the target has it
/// legal; ours left the pair at every target, an `and.l #255` after each
/// volatile byte read at m68k.
#[test]
fn a_volatile_load_is_extended_where_the_target_has_the_extending_load() {
    use super::super::dag_combiner::DagCombiner;
    let tli = TargetLowering::x86_64(Features::default(), MVT::I64);
    for (op, ext) in [(ISD::ZeroExtend, LoadExt::ZExtLoad), (ISD::SignExtend, LoadExt::SExtLoad)] {
        let mut dag = dag();
        let byte = load(&mut dag, true);
        let wide = dag.get_node(op, EVT::I32, &[byte]);
        dag.set_root(wide);
        DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
        let root = dag.get_root();
        assert_eq!(ext_of(&dag, root), (EVT::I32, EVT::I8, Some(ext)), "{op:?}");
        let NodeData::Mem(info) = dag.node(root).data else { panic!("a load") };
        assert!(info.volatile, "{op:?}: the extending load stays volatile");
    }
}

/// A target that gates the fold (z80, x86 m32 and m16: the extending form
/// lowered larger there, +37 B on z80 stackdeep) keeps the volatile load and
/// the extension apart, as before the fold.
#[test]
fn a_target_that_gates_the_fold_keeps_the_volatile_load_unextended() {
    use super::super::dag_combiner::DagCombiner;
    let mut tli = TargetLowering::x86_64(Features::default(), MVT::I64);
    tli.set_fold_extend_of_volatile_load(false);
    let mut dag = dag();
    let byte = load(&mut dag, true);
    let wide = dag.get_node(ISD::ZeroExtend, EVT::I32, &[byte]);
    dag.set_root(wide);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    assert_eq!(dag.opcode(dag.get_root()), ISD::ZeroExtend, "the extension stays a separate node");
}
