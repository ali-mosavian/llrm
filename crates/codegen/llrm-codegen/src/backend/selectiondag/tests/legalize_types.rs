use super::super::legalize_types::{LegalizeError, TypeLegalizer};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn load(
    dag: &mut SelectionDAG,
    vt: EVT,
    slot: i32,
) -> SDValue {
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: vt,
        align: 1,
        volatile: false,
        base: MemBase::Stack(slot),
        ext: None,
        space: 0,
        ordering: None,
    };
    dag.get_load(vt, chain, ptr, info)
}

fn legalize(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
) -> Result<bool, LegalizeError> {
    TypeLegalizer::new(dag, tli).run()
}

fn all_legal(
    dag: &mut SelectionDAG,
    tli: &TargetLowering,
) -> bool {
    TypeLegalizer::new(dag, tli).all_legal()
}

/// A promoted `i12` load read two bytes as a plain `i16` load, which wasm's
/// `SetP2AlignOperands` asserted on (its alignment is the access size, 2 bytes
/// of a 1-byte access). `PromoteIntRes_LOAD` makes it an any-extending load.
#[test]
fn a_promoted_load_is_an_any_extending_load() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let i12 = EVT::Int(12);
    let a = load(&mut dag, i12, 1);
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[a]);
    dag.set_root(byte);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let wide = dag.operand(dag.get_root(), 0);
    assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::Load, EVT::I16));
    let NodeData::Mem(info) = dag.node(wide).data else { panic!("mem") };
    assert_eq!(info.ext, Some(crate::backend::selectiondag::actions::LoadExt::ExtLoad));
}

/// `sitofp i12` and `fptosi` to `i12` had no promotion: the legalizer refused
/// them as "promote operand" / "promote result" (wasm's `sitofp i16`).
/// `PromoteIntOp_SINT_TO_FP` extends the promoted source in the register;
/// `PromoteIntRes_FP_TO_XINT` converts wide and asserts the narrow type.
#[test]
fn an_odd_width_integer_converts_to_and_from_a_float() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let i12 = EVT::Int(12);
    let mut dag = dag();
    let a = load(&mut dag, i12, 1);
    let float = dag.get_node(ISD::SIntToFp, EVT::Simple(MVT::F64), &[a]);
    dag.set_root(float);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let source = dag.operand(dag.get_root(), 0);
    assert_eq!((dag.opcode(source), dag.value_type(source)), (ISD::SignExtendInReg, EVT::I16));

    let mut dag = super::dag();
    let f = dag.get_register(EVT::Simple(MVT::F64), 9);
    let int = dag.get_node(ISD::FpToSInt, i12, &[f]);
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[int]);
    dag.set_root(byte);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let wide = dag.operand(dag.get_root(), 0);
    assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::AssertSext, EVT::I16));
    assert_eq!(dag.opcode(dag.operand(wide, 0)), ISD::FpToSInt);
}

/// An `lrint` to a narrow integer had no promotion ("promote result");
/// `PromoteIntRes_XRINT` rounds to the wider type.
#[test]
fn a_narrow_lrint_rounds_to_the_promoted_type() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = super::dag();
    let f = dag.get_register(EVT::Simple(MVT::F64), 9);
    let rint = dag.get_node(ISD::LRint, EVT::Int(12), &[f]);
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[rint]);
    dag.set_root(byte);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let wide = dag.operand(dag.get_root(), 0);
    assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::LRint, EVT::I16));
}

/// `shl i16 8; ashr i16 8` is `SIGN_EXTEND_INREG i16` of an `i8`; the type
/// legalizer had no `PromoteIntRes_SIGN_EXTEND_INREG`, so llrm-qb --wasm32
/// refused bench/nibbles ("Unsupported SignExtendInReg, promote result").
/// After: the same extension of the promoted operand, from the same type.
#[test]
fn a_narrow_sign_extend_in_reg_extends_the_promoted_operand() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let i12 = EVT::Int(12);
    let value = load(&mut dag, i12, 1);
    let from = dag.get_value_type_node(EVT::I8);
    let extended = dag.get_node(ISD::SignExtendInReg, i12, &[value, from]);
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[extended]);
    dag.set_root(byte);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let wide = dag.operand(dag.get_root(), 0);
    assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::SignExtendInReg, EVT::I16));
    assert_eq!(dag.operand(wide, 1), from, "the extension is from the same type");
}

/// `trunc(load i12 + load i12)` to a byte: before legalization the root
/// reads an `i12`, which no register holds. After it the add is an `i16`
/// add of extending loads, and the root is rewritten to read it.
#[test]
fn an_odd_width_add_is_promoted_and_its_user_follows() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let i12 = EVT::Int(12);
    let (a, b) = (load(&mut dag, i12, 1), load(&mut dag, i12, 2));
    let sum = dag.get_node(ISD::Add, i12, &[a, b]);
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[sum]);
    dag.set_root(byte);
    assert!(!all_legal(&mut dag, &tli));
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    assert!(all_legal(&mut dag, &tli));
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Truncate);
    let wide = dag.operand(root, 0);
    assert_eq!((dag.opcode(wide), dag.value_type(wide)), (ISD::Add, EVT::I16));
    assert_ne!(dag.operand(wide, 0), dag.operand(wide, 1), "each operand is its own promoted load");
    for operand in [dag.operand(wide, 0), dag.operand(wide, 1)] {
        assert_eq!((dag.opcode(operand), dag.value_type(operand)), (ISD::Load, EVT::I16));
    }
}

/// The load's chain result stands for the promoted load's: a node that
/// read the old chain would read a load nothing uses.
#[test]
fn a_promoted_loads_chain_users_follow() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let a = load(&mut dag, EVT::Int(12), 1);
    let chain_user = dag.get_node(ISD::TokenFactor, EVT::OTHER, &[SDValue { node: a.node, res: 1 }, dag.entry_node()]);
    dag.set_root(chain_user);
    legalize(&mut dag, &tli).unwrap();
    // A token factor of one chain and the entry is that chain.
    let chain = dag.get_root();
    assert_eq!(chain.res, 1);
    assert_eq!(dag.value_type(SDValue { node: chain.node, res: 0 }), EVT::I16, "the promoted load");
}

/// An `i64` load truncated to a word reads the low half; the constants of
/// an `i64` are two `i32`.
#[test]
fn an_i64_constant_is_two_halves_and_logic_follows() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let k = dag.get_constant_with(0x1122_3344_5566_7788, EVT::I64, false, true);
    let word = dag.get_node(ISD::Truncate, EVT::I32, &[k]);
    dag.set_root(word);
    legalize(&mut dag, &tli).unwrap();
    assert!(all_legal(&mut dag, &tli));
    assert_eq!(dag.constant(dag.get_root()), Some(0x5566_7788), "trunc of the low half is the low half");
}

/// A node nothing translated handles (a 12-bit popcount) is refused by name
/// rather than left with an illegal type.
#[test]
fn an_unhandled_node_is_refused_by_name() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let a = load(&mut dag, EVT::Int(12), 1);
    let count = dag.get_node(ISD::CtPop, EVT::Int(12), &[a]);
    let byte = dag.get_node(ISD::Truncate, EVT::I8, &[count]);
    dag.set_root(byte);
    let error = legalize(&mut dag, &tli).unwrap_err();
    assert!(matches!(error, LegalizeError::Unsupported { op: ISD::CtPop, .. }), "{error:?}");
}

/// A zero-extending `i64` load from a dword expanded as two dword loads (the
/// second from +4), so the high half was memory instead of zero: a wrong value
/// (bitfield_long_long, 7 m16/m32 execution programs) and a degenerate load
/// `zext from i32` of an `i32`.
#[test]
fn a_zero_extending_i64_load_from_a_dword_has_a_zero_high_half() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let (ptr, chain) = (dag.get_register(EVT::I32, 7), dag.entry_node());
    let info = MemInfo {
        mem_vt: EVT::I32,
        align: 4,
        volatile: false,
        base: MemBase::Stack(1),
        ext: None,
        space: 0,
        ordering: None,
    };
    let wide = dag.get_ext_load(super::super::actions::LoadExt::ZExtLoad, EVT::I64, chain, ptr, info);
    let thirty_two = dag.get_constant(32, EVT::I8);
    let shifted = dag.get_node(ISD::Srl, EVT::I64, &[wide, thirty_two]);
    let high = dag.get_node(ISD::Truncate, EVT::I32, &[shifted]);
    dag.set_root(high);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let root = dag.get_root();
    assert_eq!(dag.constant(root), Some(0), "the high dword is zero");
}

/// An `i64` load through a far pointer split into two dword loads whose second
/// address was `far + 4` with a constant of type `Far`: the legalizer panicked
/// (struct_with_i64 at m16, 3 shadow crashes). The second address keeps the
/// selector and moves the offset.
#[test]
fn the_second_half_of_a_far_i64_load_moves_the_offset_and_keeps_the_selector() {
    let tli = TargetLowering::x86_16(Features::default(), MVT::I16);
    let mut dag = dag();
    let (chain, offset, selector) = (dag.entry_node(), dag.get_register(EVT::I16, 7), dag.get_register(EVT::I16, 8));
    let far = dag.get_node(ISD::BuildPair, EVT::Simple(MVT::Far), &[offset, selector]);
    let info = MemInfo {
        mem_vt: EVT::I64,
        align: 2,
        volatile: false,
        base: MemBase::Unknown,
        ext: None,
        space: 259,
        ordering: None,
    };
    let wide = dag.get_load(EVT::I64, chain, far, info);
    let zero = dag.get_constant(0, EVT::I8);
    let shifted = dag.get_node(ISD::Srl, EVT::I64, &[wide, zero]);
    let low = dag.get_node(ISD::Truncate, EVT::I16, &[shifted]);
    dag.set_root(low);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
}

/// `smin`/`umin` of an odd width were refused ("promote result"), so any
/// min/max of a type below the register size failed to legalize. Signed
/// operands are sign-extended in register, unsigned ones masked.
#[test]
fn an_odd_width_min_max_is_promoted_with_extended_operands() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    for (op, extend) in [(ISD::SMin, ISD::Sra), (ISD::UMax, ISD::And)] {
        let mut dag = dag();
        let i12 = EVT::Int(12);
        let (a, b) = (load(&mut dag, i12, 1), load(&mut dag, i12, 2));
        let best = dag.get_node(op, i12, &[a, b]);
        let byte = dag.get_node(ISD::Truncate, EVT::I8, &[best]);
        dag.set_root(byte);
        assert_eq!(legalize(&mut dag, &tli), Ok(true), "{op:?}");
        assert!(all_legal(&mut dag, &tli));
        let wide = dag.operand(dag.get_root(), 0);
        assert_eq!((dag.opcode(wide), dag.value_type(wide)), (op, EVT::I16));
        for operand in [dag.operand(wide, 0), dag.operand(wide, 1)] {
            assert_eq!(dag.opcode(operand), extend, "{op:?} extends its operands");
        }
    }
}

/// With no `i8` or `i16` register, a store of one was refused ("promote
/// operand"): it is a truncating store of the promoted value, as
/// `PromoteIntOp_STORE`.
#[test]
fn a_store_of_a_value_below_the_register_size_is_a_truncating_store() {
    let tli = TargetLowering::empty(llrm_sdag::lowering::TargetLoweringInfo::new(&[MVT::I32, MVT::I64], false));
    let mut dag = dag();
    let value = load(&mut dag, EVT::Simple(MVT::I16), 1);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: EVT::Simple(MVT::I16),
        align: 2,
        volatile: false,
        base: MemBase::Stack(2),
        ext: None,
        space: 0,
        ordering: None,
    };
    let store = dag.get_store(chain, value, ptr, info);
    dag.set_root(store);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Store);
    assert_eq!(dag.value_type(dag.operand(root, 1)), EVT::I32, "the promoted value");
    let NodeData::Mem(stored) = dag.node(root).data else { panic!("a store has a memory operand") };
    assert_eq!(stored.mem_vt, EVT::Simple(MVT::I16), "still stores 16 bits");
}

/// The sign-extend of a promoted operand shifted by an `i8` constant whatever
/// the target's shift-amount type; AArch64 shifts by an `i64`.
#[test]
fn a_promotion_shifts_by_the_targets_shift_amount_type() {
    let info =
        llrm_sdag::lowering::TargetLoweringInfo::new(&[MVT::I32, MVT::I64], false).with_shift_amount_type(MVT::I64);
    let tli = TargetLowering::empty(info);
    let mut dag = dag();
    let i16 = EVT::Simple(MVT::I16);
    let (a, b) = (load(&mut dag, i16, 1), load(&mut dag, i16, 2));
    let (a, b) = (dag.get_node(ISD::Add, i16, &[a, b]), dag.get_node(ISD::Sub, i16, &[a, b]));
    let best = dag.get_node(ISD::SMin, i16, &[a, b]);
    let word = dag.get_node(ISD::Truncate, EVT::I8, &[best]);
    dag.set_root(word);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let shifts: Vec<_> = (0..dag.nodes().len() as u32)
        .map(|node| SDValue { node, res: 0 })
        .filter(|value| matches!(dag.opcode(*value), ISD::Shl | ISD::Sra))
        .collect();
    assert!(!shifts.is_empty(), "the operands are sign-extended in the register");
    for shift in shifts {
        assert_eq!(dag.value_type(dag.operand(shift, 1)), EVT::I64, "the shift amount of {:?}", dag.opcode(shift));
    }
}

/// A store promoted to a wider register is rebuilt from the old one's memory
/// info: its atomic ordering must come along, or a `release` store is selected
/// as a plain one.
#[test]
fn a_promoted_store_keeps_its_atomic_ordering() {
    use llrm_mir::opcode::AtomicOrdering;
    let tli = TargetLowering::empty(llrm_sdag::lowering::TargetLoweringInfo::new(&[MVT::I32, MVT::I64], false));
    let mut dag = dag();
    let value = load(&mut dag, EVT::Simple(MVT::I16), 1);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: EVT::Simple(MVT::I16),
        align: 2,
        volatile: false,
        base: MemBase::Stack(2),
        ext: None,
        space: 0,
        ordering: Some(AtomicOrdering::Release),
    };
    let store = dag.get_store(chain, value, ptr, info);
    dag.set_root(store);
    assert_eq!(legalize(&mut dag, &tli), Ok(true));
    let NodeData::Mem(stored) = dag.node(dag.get_root()).data else { panic!("a store has a memory operand") };
    assert_eq!(stored.ordering, Some(AtomicOrdering::Release));
}

/// A divide in a promoted type was refused ("promote result"; wasm's `sdiv
/// i8`). `PromoteIntRes_SDIV/UDIV` extends both operands in the register, sign
/// for the signed forms and zero for the unsigned.
#[test]
fn a_promoted_divide_extends_both_operands_in_the_register() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    for (op, extend) in [
        (ISD::SDiv, ISD::SignExtendInReg),
        (ISD::SRem, ISD::SignExtendInReg),
        (ISD::UDiv, ISD::And),
        (ISD::URem, ISD::And),
    ] {
        let mut dag = super::dag();
        let (a, b) = (load(&mut dag, EVT::Int(12), 1), load(&mut dag, EVT::Int(12), 2));
        let quotient = dag.get_node(op, EVT::Int(12), &[a, b]);
        let byte = dag.get_node(ISD::Truncate, EVT::I8, &[quotient]);
        dag.set_root(byte);
        assert_eq!(legalize(&mut dag, &tli), Ok(true), "{op:?}");
        let wide = dag.operand(dag.get_root(), 0);
        assert_eq!((dag.opcode(wide), dag.value_type(wide)), (op, EVT::I16), "{op:?}");
        for at in 0..2 {
            assert_eq!(dag.opcode(dag.operand(wide, at)), extend, "{op:?} operand {at}");
        }
    }
}

/// A shift whose amount is a byte on a target with no byte register was
/// refused ("promote operand"; wasm's `shl i32 x, i8`). `PromoteIntOp_Shift`
/// zero extends the amount in the register.
#[test]
fn a_promoted_shift_amount_is_zero_extended_in_the_register() {
    let info = llrm_sdag::lowering::TargetLoweringInfo::new(&[MVT::I32], false).with_shift_amount_type(MVT::I32);
    let tli = TargetLowering::empty(info);
    for op in [ISD::Shl, ISD::Srl, ISD::Sra] {
        let mut dag = dag();
        let (value, amount) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I8, 2));
        let shifted = dag.get_node(op, EVT::I32, &[value, amount]);
        dag.set_root(shifted);
        assert_eq!(legalize(&mut dag, &tli), Ok(true), "{op:?}");
        let shift = dag.get_root();
        let by = dag.operand(shift, 1);
        assert_eq!((dag.opcode(by), dag.value_type(by)), (ISD::And, EVT::I32), "{op:?}");
    }
}

/// `abs` of an `i64` on a 32-bit target stopped the legalizer ("Abs: expand
/// result"; QB's `_currency_text` did not link at x86 m16/m32 with a CURRENCY
/// value). `ExpandIntRes_ABS`: sra+xor+sub with a borrow where the target has
/// one, else the negation selected by the sign of the high half.
#[test]
fn an_i64_abs_is_expanded_to_halves_with_and_without_a_borrow_chain() {
    let no_borrow = TargetLowering::empty(llrm_sdag::lowering::TargetLoweringInfo::new(&[MVT::I8, MVT::I32], false));
    let borrow = TargetLowering::x86_32(Features::default(), MVT::I32);
    for (name, tli) in [("select", &no_borrow), ("borrow", &borrow)] {
        for op in [ISD::Abs, ISD::AbsMinPoison] {
            let mut dag = dag();
            let wide = load(&mut dag, EVT::I64, 1);
            let abs = dag.get_node(op, EVT::I64, &[wide]);
            let thirty_two = dag.get_constant(32, EVT::I8);
            let high = dag.get_node(ISD::Srl, EVT::I64, &[abs, thirty_two]);
            let root = dag.get_node(ISD::Truncate, EVT::I32, &[high]);
            dag.set_root(root);
            assert_eq!(legalize(&mut dag, tli), Ok(true), "{name} {op:?}");
            assert!(all_legal(&mut dag, tli), "{name} {op:?}");
        }
    }
}
