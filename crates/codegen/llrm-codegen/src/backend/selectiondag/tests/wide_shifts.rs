use super::super::legalize_dag::DagLegalizer;
use super::super::legalize_types::{LegalizeError, TypeLegalizer};
use super::super::x86_lowering::{X86Lowering, x86isd};
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn p5() -> TargetLowering {
    TargetLowering::x86_32(Features::default(), MVT::I32)
}

/// An `i64` no instruction can fold (an opaque constant), shifted, then
/// cut to a dword: `low` takes the low half, `high` the high half.
fn shifted(
    op: ISD,
    by: u128,
) -> (SelectionDAG, SDValue, SDValue) {
    let mut dag = dag();
    let x = dag.get_constant_with(0x1122_3344_5566_7788, EVT::I64, false, true);
    let amount = dag.get_constant(by, EVT::I8);
    let moved = dag.get_node(op, EVT::I64, &[x, amount]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    let thirty_two = dag.get_constant(32, EVT::I8);
    let upper = dag.get_node(ISD::Srl, EVT::I64, &[moved, thirty_two]);
    let high = dag.get_node(ISD::Truncate, EVT::I32, &[upper]);
    let both = dag.get_node(ISD::Add, EVT::I32, &[low, high]);
    dag.set_root(both);
    TypeLegalizer::new(&mut dag, &p5()).run().unwrap();
    let root = dag.get_root();
    let (l, h) = (dag.operand(root, 0), dag.operand(root, 1));
    (dag, l, h)
}

fn amount_of(
    dag: &SelectionDAG,
    value: SDValue,
) -> (Option<u128>, EVT) {
    let amount = dag.operand(value, dag.node(value).ops.len() - 1);
    (dag.constant(amount), dag.value_type(amount))
}

/// `x << 3` at 32 bits: the low dword is a shift, the high is the funnel
/// shift `shld` (legal on i486/P5), with an `i8` amount. A constant shift
/// no expansion handles was refused by name before.
#[test]
fn a_left_shift_by_a_small_constant_is_a_shift_and_a_funnel_shift() {
    let (dag, low, high) = shifted(ISD::Shl, 3);
    assert_eq!((dag.opcode(low), amount_of(&dag, low)), (ISD::Shl, (Some(3), EVT::I8)));
    assert_eq!((dag.opcode(high), amount_of(&dag, high)), (ISD::FShl, (Some(3), EVT::I8)));
}

/// A shift by 40 moves a whole dword: `x >> 40` is `hi >> 8` low, zero
/// high.
#[test]
fn a_right_shift_past_one_dword_shifts_the_high_dword() {
    let (dag, low, high) = shifted(ISD::Srl, 40);
    assert_eq!((dag.opcode(low), amount_of(&dag, low)), (ISD::Srl, (Some(8), EVT::I8)));
    assert!(dag.is_null_constant(high));
}

/// An arithmetic shift by 63 leaves only the sign: the same node in both
/// halves.
#[test]
fn an_arithmetic_shift_out_is_the_sign_in_both_halves() {
    let (dag, low, high) = shifted(ISD::Sra, 63);
    assert_eq!((dag.opcode(low), amount_of(&dag, low).0), (ISD::Sra, Some(31)));
    assert_eq!(low, high);
}

/// A variable amount is the double-width shift node: it takes both halves
/// and an `i8` amount and has two results.
#[test]
fn a_variable_shift_is_shl_parts() {
    let mut dag = dag();
    let x = dag.get_constant_with(0x1122_3344_5566_7788, EVT::I64, false, true);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(1),
        ext: None,
        space: 0,
        ordering: None,
    };
    let amount = dag.get_load(EVT::I8, chain, ptr, info);
    let moved = dag.get_node(ISD::Shl, EVT::I64, &[x, amount]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &p5()).run().unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), root.res), (ISD::ShlParts, 0));
    assert_eq!(dag.node(root).values, vec![EVT::I32, EVT::I32]);
    assert_eq!(dag.node(root).ops.len(), 3);
}

/// The target's lowering of the double-width shift: the high dword of
/// `shl_parts` is `cmov(shld, shl)` on bit 5 of the amount.
#[test]
fn shl_parts_is_a_funnel_shift_a_shift_and_a_cmov_on_bit_five() {
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let load = |dag: &mut SelectionDAG, vt: EVT, slot: i32| {
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
    };
    let (lo, hi, amount) = (load(&mut dag, EVT::I32, 1), load(&mut dag, EVT::I32, 2), load(&mut dag, EVT::I8, 3));
    let parts = dag.get_node_multi(ISD::ShlParts, &[EVT::I32, EVT::I32], &[lo, hi, amount]);
    dag.set_root(SDValue { node: parts.node, res: 1 });
    let tli = p5();
    DagLegalizer::new(
        &mut dag,
        &tli,
        &mut X86Lowering { cmov: false, sse2: false, min_size: false, i16_mul_chains: false, profile: None },
    )
    .run()
    .unwrap();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), root.res), (ISD::MergeValues, 1), "the merge of both halves; the combiner drops it");
    let high = dag.operand(root, 1);
    assert_eq!(dag.opcode(high), x86isd::CMOV);
    assert_eq!(dag.opcode(dag.operand(high, 0)), ISD::FShl, "the false value is the funnel shift");
    assert_eq!(dag.opcode(dag.operand(high, 1)), ISD::Shl, "the true value is the shift by amount & 31");
}

/// A shift with an amount and no `shl_parts` (the table empty) is refused.
#[test]
fn a_variable_shift_without_the_parts_node_is_refused() {
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::ShlParts, MVT::I32, Action::Expand);
    let mut dag = dag();
    let x = dag.get_constant_with(5, EVT::I64, false, true);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(1),
        ext: None,
        space: 0,
        ordering: None,
    };
    let amount = dag.get_load(EVT::I8, chain, ptr, info);
    let moved = dag.get_node(ISD::Shl, EVT::I64, &[x, amount]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    dag.set_root(low);
    let error = TypeLegalizer::new(&mut dag, &tli).run().unwrap_err();
    assert!(matches!(error, LegalizeError::Unsupported { op: ISD::Shl, .. }), "{error:?}");
}

/// A target with no `shl_parts` and no libcall row of its own reached
/// "expand shift to a library call" for an `i64` shifted by a variable: the
/// pin's `SHL_I64` is `__ashldi3` on every target. It was refused (6502
/// `u64.c`); it is now one call of two operands.
#[test]
fn a_variable_i64_shift_without_parts_is_the_pins_default_libcall() {
    use super::super::legalize_dag::Hooks;
    #[derive(Default)]
    struct Calls(Vec<(&'static str, usize)>);
    impl Hooks for Calls {
        fn lower_operation(
            &mut self,
            _dag: &mut SelectionDAG,
            _node: SDValue,
        ) -> Option<SDValue> {
            None
        }
        fn make_lib_call(
            &mut self,
            dag: &mut SelectionDAG,
            name: &'static str,
            args: &[SDValue],
            ret: EVT,
            _signed: bool,
        ) -> Option<SDValue> {
            self.0.push((name, args.len()));
            let (lo, hi) = (dag.get_register(EVT::I32, 90), dag.get_register(EVT::I32, 91));
            Some(dag.get_node(ISD::BuildPair, ret, &[lo, hi]))
        }
    }
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    tli.set_operation_action(ISD::ShlParts, MVT::I32, Action::Expand);
    let mut dag = dag();
    let x = dag.get_constant_with(5, EVT::I64, false, true);
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let info = MemInfo {
        mem_vt: EVT::I8,
        align: 1,
        volatile: false,
        base: MemBase::Stack(1),
        ext: None,
        space: 0,
        ordering: None,
    };
    let amount = dag.get_load(EVT::I8, chain, ptr, info);
    let moved = dag.get_node(ISD::Shl, EVT::I64, &[x, amount]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[moved]);
    dag.set_root(low);
    let mut calls = Calls::default();
    TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut calls).run().unwrap();
    assert_eq!(calls.0, vec![("__ashldi3", 2)]);
}

fn funnel_value(
    dag: &SelectionDAG,
    v: SDValue,
) -> u64 {
    let arg = |i: usize| funnel_value(dag, dag.operand(v, i));
    let bits = dag.value_type(v).size_in_bits();
    let mask = if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 };
    let funnel = |left: bool| {
        let (a, b, z) = (arg(0), arg(1), arg(2) % u64::from(bits));
        let both = (u128::from(a) << bits) | u128::from(b);
        let r = if left { both << z >> bits } else { both >> z };
        (r as u64) & mask
    };
    match dag.opcode(v) {
        ISD::Constant => dag.constant(v).unwrap() as u64 & mask,
        ISD::FShl => funnel(true),
        ISD::FShr => funnel(false),
        ISD::Truncate => arg(0) & mask,
        ISD::BuildPair => arg(0) | (arg(1) << (bits / 2)),
        other => panic!("unexpected {other:?}"),
    }
}

/// An `i32` funnel shift on a target whose widest register is 16 bits had no
/// expansion ("Unsupported FShl, expand result"; 6502). Each half of the
/// result must equal the funnel shift of the whole, for amounts either side
/// of the half width, at both directions.
#[test]
fn an_i32_funnel_shift_expands_to_two_half_width_funnel_shifts() {
    let tli = TargetLowering::empty(TargetLoweringInfo::new(&[MVT::I1, MVT::I8, MVT::I16], false));
    for op in [ISD::FShl, ISD::FShr] {
        for amount in [1u128, 5, 15, 16, 17, 31] {
            let (x, y) = (0x1234_5678u128, 0x9ABC_DEF0u128);
            let mut dag = dag();
            let (xn, yn) = (dag.get_constant_with(x, EVT::I32, false, true), dag.get_constant_with(y, EVT::I32, false, true));
            let zn = dag.get_constant(amount, EVT::I8);
            let funnel = dag.get_node(op, EVT::I32, &[xn, yn, zn]);
            let low = dag.get_node(ISD::Truncate, EVT::I16, &[funnel]);
            let sixteen = dag.get_constant(16, EVT::I8);
            let upper = dag.get_node(ISD::Srl, EVT::I32, &[funnel, sixteen]);
            let high = dag.get_node(ISD::Truncate, EVT::I16, &[upper]);
            let both = dag.get_node(ISD::Add, EVT::I16, &[low, high]);
            dag.set_root(both);
            TypeLegalizer::new(&mut dag, &tli).run().unwrap_or_else(|e| panic!("{op:?} by {amount}: {e:?}"));
            let root = dag.get_root();
            let got = |v: SDValue| funnel_value(&dag, v);
            let whole = {
                let both = (x << 32) | y;
                let r = if op == ISD::FShl { both << amount >> 32 } else { both >> amount };
                r as u32
            };
            let (l, h) = (got(dag.operand(root, 0)) as u32, got(dag.operand(root, 1)) as u32);
            assert_eq!((l, h), (whole & 0xFFFF, whole >> 16), "{op:?} by {amount}");
        }
    }
}

/// An `i64` product on a target with no widening multiply of its halves took
/// `force_expand_multiply` and never the pin's `MUL_I64` (`__muldi3`): 445
/// lines of assembly and four `__mulsi3` calls for one `u64 * u64` on the 6502.
/// `ExpandIntRes_MUL` tries `expandMUL`, then the library call, then the
/// forced expansion; it is now one call of two operands.
#[test]
fn a_64_bit_product_without_a_widening_multiply_is_the_pins_default_libcall() {
    use super::super::legalize_dag::Hooks;
    #[derive(Default)]
    struct Calls(Vec<(&'static str, usize)>);
    impl Hooks for Calls {
        fn lower_operation(
            &mut self,
            _dag: &mut SelectionDAG,
            _node: SDValue,
        ) -> Option<SDValue> {
            None
        }
        fn make_lib_call(
            &mut self,
            dag: &mut SelectionDAG,
            name: &'static str,
            args: &[SDValue],
            ret: EVT,
            _signed: bool,
        ) -> Option<SDValue> {
            self.0.push((name, args.len()));
            let (lo, hi) = (dag.get_register(EVT::I32, 90), dag.get_register(EVT::I32, 91));
            Some(dag.get_node(ISD::BuildPair, ret, &[lo, hi]))
        }
    }
    let mut tli = TargetLowering::empty(TargetLoweringInfo::x86_32());
    for op in [ISD::UMulLoHi, ISD::SMulLoHi, ISD::MulHU, ISD::MulHS] {
        tli.set_operation_action(op, MVT::I32, Action::Expand);
    }
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let load = |dag: &mut SelectionDAG, slot| {
        let info = MemInfo {
            mem_vt: EVT::I64,
            align: 1,
            volatile: false,
            base: MemBase::Stack(slot),
            ext: None,
            space: 0,
            ordering: None,
        };
        dag.get_load(EVT::I64, chain, ptr, info)
    };
    let (a, b) = (load(&mut dag, 1), load(&mut dag, 2));
    let product = dag.get_node(ISD::Mul, EVT::I64, &[a, b]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[product]);
    dag.set_root(low);
    let mut calls = Calls::default();
    TypeLegalizer::new(&mut dag, &tli).with_hooks(&mut calls).run().unwrap();
    assert_eq!(calls.0, vec![("__muldi3", 2)]);
}
