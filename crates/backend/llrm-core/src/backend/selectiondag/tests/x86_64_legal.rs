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
    DagLegalizer::new(&mut dag, tli, &mut X86Lowering { cmov: true, sse2: false }).run().expect("legalizes");
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
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: true, sse2: false }).run().expect("legalizes");
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

fn legalized_float_op(
    op: ISD,
    inner: Option<ISD>,
    vt: EVT,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_64(Features { sse2: true, cmov: true, ..Features::default() }, MVT::I64);
    let mut dag = SelectionDAG::new(DagConfig { pointer: EVT::I64, ..DagConfig::default() });
    let x = dag.get_register(vt, 1);
    let x = inner.map_or(x, |inner| dag.get_node(inner, vt, &[x]));
    let node = dag.get_node(op, vt, &[x]);
    dag.set_root(node);
    TypeLegalizer::new(&mut dag, &tli).run().expect("types legalize");
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: true, sse2: false }).run().expect("legalizes");
    let root = dag.get_root();
    (dag, root)
}

/// `fneg`/`fabs` of a scalar float stayed generic nodes, which no SSE
/// instruction selects: the pin xors/ands the sign bit with a 16-byte mask.
#[test]
fn fneg_and_fabs_of_a_float_are_sign_mask_logic() {
    use super::super::x86_lowering::x86isd;
    let f64v = EVT::Simple(MVT::F64);
    for (op, inner, want, mask) in [
        (ISD::FNeg, None, x86isd::FXOR, 1u128 << 63),
        (ISD::FAbs, None, x86isd::FAND, (1u128 << 63) - 1),
        (ISD::FNeg, Some(ISD::FAbs), x86isd::FOR, 1u128 << 63),
    ] {
        let (dag, root) = legalized_float_op(op, inner, f64v);
        assert_eq!((dag.opcode(root), dag.value_type(root)), (want, f64v), "{op:?}");
        assert!(dag.constant_pool.contains(&(mask, EVT::Simple(MVT::I128))), "{op:?}: mask");
    }
}

/// With SSE2 (`hasSSE2()`, L618..L706) `f32` and `f64` are done in xmm
/// registers: `fabs`, `fneg` and `fcopysign` are `andps`/`xorps` with a mask
/// (Custom), `fadd`/`fsub` are Custom, `fsin`/`fcos` are libcalls (Expand), a
/// `f32` to `f64` extension is Custom, there is no `f64` extending load, only
/// `+0.0` is a free immediate, and a bitcast between a float and an integer is
/// no longer expanded through memory. Without it the x87 arms stand.
#[test]
fn the_sse2_table_does_the_scalar_float_work_in_xmm() {
    use super::super::actions::{Action, LoadExt};
    let sse = TargetLowering::x86_64(Features { sse2: true, ..Features::default() }, MVT::I64);
    let x87 = TargetLowering::x86_64(Features::default(), MVT::I64);
    for vt in [MVT::F32, MVT::F64] {
        for op in [ISD::FAbs, ISD::FNeg, ISD::FCopySign, ISD::FAdd, ISD::FSub] {
            assert_eq!(sse.operation_action(op, EVT::from(vt)), Action::Custom, "{op:?} {vt:?}");
        }
        for op in [ISD::FSin, ISD::FCos] {
            assert_eq!(sse.operation_action(op, EVT::from(vt)), Action::Expand, "{op:?} {vt:?}");
        }
        assert_eq!(x87.operation_action(ISD::FCopySign, EVT::from(vt)), Action::Expand);
        assert_ne!(x87.operation_action(ISD::FAbs, EVT::from(vt)), Action::Custom);
        assert!(sse.is_fp_imm_legal(vt, 0) && !sse.is_fp_imm_legal(vt, 1u128 << if vt == MVT::F32 { 31 } else { 63 }));
        assert!(x87.is_fp_imm_legal(vt, 1u128 << if vt == MVT::F32 { 31 } else { 63 }));
    }
    assert_eq!(sse.operation_action(ISD::FpExtend, EVT::from(MVT::F64)), Action::Custom);
    assert_eq!(sse.load_ext_action(LoadExt::ExtLoad, MVT::F64, MVT::F32), Action::Expand);
    assert_ne!(sse.operation_action(ISD::BitCast, EVT::from(MVT::I64)), Action::Expand);
    assert_eq!(x87.operation_action(ISD::BitCast, EVT::from(MVT::I64)), Action::Expand);
}

/// `copysign` stayed a generic node no SSE instruction selects: the pin ors
/// the magnitude's low bits with the sign operand's sign bit.
#[test]
fn fcopysign_of_a_float_is_two_masks_and_an_or() {
    use super::super::x86_lowering::x86isd;
    let tli = TargetLowering::x86_64(Features { sse2: true, cmov: true, ..Features::default() }, MVT::I64);
    let mut dag = SelectionDAG::new(DagConfig { pointer: EVT::I64, ..DagConfig::default() });
    let f32v = EVT::Simple(MVT::F32);
    let (m, s) = (dag.get_register(f32v, 1), dag.get_register(f32v, 2));
    let node = dag.get_node(ISD::FCopySign, f32v, &[m, s]);
    dag.set_root(node);
    TypeLegalizer::new(&mut dag, &tli).run().expect("types legalize");
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: true, sse2: false }).run().expect("legalizes");
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (x86isd::FOR, f32v));
    for mask in [(1u128 << 31) - 1, 1 << 31] {
        assert!(dag.constant_pool.contains(&(mask, EVT::Simple(MVT::I128))));
    }
}

/// `f32` to `f64` is Custom at SSE2 (the arm is the pin's `f16` one); the
/// hook declined it, so it fell to a libcall refusal instead of `cvtss2sd`.
#[test]
fn an_f32_to_f64_extend_survives_legalization() {
    let tli = TargetLowering::x86_64(Features { sse2: true, cmov: true, ..Features::default() }, MVT::I64);
    let mut dag = SelectionDAG::new(DagConfig { pointer: EVT::I64, ..DagConfig::default() });
    let x = dag.get_register(EVT::Simple(MVT::F32), 1);
    let node = dag.get_node(ISD::FpExtend, EVT::Simple(MVT::F64), &[x]);
    dag.set_root(node);
    TypeLegalizer::new(&mut dag, &tli).run().expect("types legalize");
    DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: true, sse2: false }).run().expect("legalizes");
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::FpExtend);
}

/// Integer to float went through `fild` and a stack slot at SSE2; the pin
/// keeps `sint_to_fp` of `i32`/`i64` (`cvtsi2sd`) and widens `i16`, and turns
/// an unsigned `i32` into a signed `i64` convert.
#[test]
fn int_to_float_at_sse2_is_a_register_convert() {
    let tli = TargetLowering::x86_64(Features { sse2: true, cmov: true, ..Features::default() }, MVT::I64);
    for (op, src, shown) in [
        (ISD::SIntToFp, EVT::I32, EVT::I32),
        (ISD::SIntToFp, EVT::I64, EVT::I64),
        (ISD::SIntToFp, EVT::I16, EVT::I32),
        (ISD::UIntToFp, EVT::I32, EVT::I64),
    ] {
        let mut dag = SelectionDAG::new(DagConfig { pointer: EVT::I64, ..DagConfig::default() });
        let f64v = EVT::Simple(MVT::F64);
        let x = dag.get_register(src, 1);
        let node = dag.get_node(op, f64v, &[x]);
        dag.set_root(node);
        TypeLegalizer::new(&mut dag, &tli).run().expect("types legalize");
        DagLegalizer::new(&mut dag, &tli, &mut X86Lowering { cmov: true, sse2: true }).run().expect("legalizes");
        let root = dag.get_root();
        let from = dag.value_type(dag.operand(root, 0));
        assert_eq!((dag.opcode(root), from), (ISD::SIntToFp, shown), "{op:?} {src:?}");
    }
}
