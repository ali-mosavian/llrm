use super::super::legalize_types::TypeLegalizer;
use super::super::x86isd;
use super::*;
use crate::backend::selectiondag::{MemBase, MemInfo};

fn quotient_of(
    op: ISD,
    narrow: bool,
) -> (SelectionDAG, SDValue) {
    quotient_sized(op, narrow, false)
}

fn quotient_sized(
    op: ISD,
    narrow: bool,
    small: bool,
) -> (SelectionDAG, SDValue) {
    quotient_on(&llrm_x86_m32::M32, op, narrow, small, small)
}

fn quotient_on(
    arch: &dyn llrm_target::Target,
    op: ISD,
    narrow: bool,
    small: bool,
    size_only: bool,
) -> (SelectionDAG, SDValue) {
    quotient_in(arch, op, narrow, small, size_only, None)
}

/// `quotient_on` in a block of `frequency` over the entry's, for a target that takes the
/// speed routine in loops at size (`Some`).
fn quotient_in(
    arch: &dyn llrm_target::Target,
    op: ISD,
    narrow: bool,
    small: bool,
    size_only: bool,
    frequency: Option<f64>,
) -> (SelectionDAG, SDValue) {
    let mut tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    tli.set_small_divide(small);
    tli.set_size_only(size_only);
    tli.set_fast_divide_in_loops(frequency.is_some());
    let mut dag = dag();
    dag.set_divide_frequency(frequency.unwrap_or(0.0));
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let mut dword = |slot: i32| {
        let info = MemInfo {
            mem_vt: EVT::I32,
            align: 4,
            volatile: false,
            base: MemBase::Stack(slot),
            ext: None,
            space: 0,
            ordering: None,
        };
        dag.get_load(EVT::I32, chain, ptr, info)
    };
    let (a, b, c, d) = (dword(1), dword(2), dword(3), dword(4));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[a, b]);
    let high = if narrow { dag.get_constant(0, EVT::I32) } else { d };
    let y = dag.get_node(ISD::BuildPair, EVT::I64, &[c, high]);
    let result = dag.get_node(op, EVT::I64, &[x, y]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[result]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).with_target(arch).run().unwrap();
    let root = dag.get_root();
    (dag, root)
}

/// The shadow listed `i64` SRem/UDiv/... "expand result" (37+1 functions). A
/// division is a call to the target's inline routine: the callee is its
/// name, and the operands are copied to the registers it reads.
fn call_of(dag: &SelectionDAG) -> (String, usize) {
    let call = (0..dag.node_count() as u32)
        .map(|node| SDValue { node, res: 0 })
        .find(|&v| dag.opcode(v) == x86isd::CALL)
        .expect("a call");
    let callee = dag.operand(call, 1);
    let NodeData::Symbol(id) = dag.node(callee).data else { panic!("a symbol callee") };
    let copies = dag.node(call).ops.iter().filter(|op| dag.opcode(**op) == ISD::Register).count();
    (dag.symbol_names[id as usize].clone(), copies)
}

#[test]
fn an_i64_unsigned_divide_by_a_wide_divisor_calls_the_wide_routine() {
    let (dag, root) = quotient_of(ISD::UDiv, false);
    assert_eq!(dag.opcode(root), ISD::CopyFromReg, "the quotient's low dword is read from a register");
    assert_eq!(call_of(&dag), ("div64_u".to_owned(), 4), "four operand registers");
}

#[test]
fn a_divisor_with_a_zero_high_dword_calls_the_dword_routine() {
    let (dag, _) = quotient_of(ISD::SDiv, true);
    // a quotient read alone: the routine that fixes only its sign (SD232)
    assert_eq!(call_of(&dag), ("div64_s32q".to_owned(), 3), "no register for the divisor's high dword");
}

/// A quotient is read from the routine's quotient register, a remainder from
/// its remainder register: swapping them returns the wrong half of the pair.
#[test]
fn quotient_and_remainder_are_read_from_their_own_registers() {
    use llrm_target::Target;
    let routine = llrm_x86_m32::M32.divide_routine(false, true).expect("a routine");
    let register = |dag: &SelectionDAG, v: SDValue| dag.node(dag.operand(v, 1)).data;
    let (quotient, qroot) = quotient_of(ISD::UDiv, false);
    let (remainder, rroot) = quotient_of(ISD::URem, false);
    assert_eq!(register(&quotient, qroot), NodeData::PhysReg(routine.quotient[0]));
    assert_eq!(register(&remainder, rroot), NodeData::PhysReg(routine.remainder[0]));
}

/// nbody_fixed at m16 -O1 held a 313-byte inline `i64` divide: code whose
/// bytes are strict (-O1, size) called the speed routine, though the target
/// has a shorter one.
#[test]
fn a_function_built_for_size_calls_the_size_divide() {
    let (dag, _) = quotient_sized(ISD::UDiv, false, true);
    assert_eq!(call_of(&dag).0, "div64_u_small");
    let (dag, _) = quotient_sized(ISD::UDiv, true, true);
    assert_eq!(call_of(&dag).0, "div64_u32_small", "a dword divisor has a size form, shorter than the speed one");
}

/// data_int64 at m16 -Os (Dag 441 B, Old 416): `x / 1000000` on `i64` called
/// the 85-byte dword-divisor routine, whose test spares a `div` the size code
/// can do without. Built for size it takes the shorter size form.
#[test]
fn a_function_built_for_size_divides_by_a_dword_in_the_size_form() {
    let (dag, _) = quotient_on(&llrm_x86_m16::M16, ISD::SDiv, true, true, true);
    assert_eq!(call_of(&dag).0, "div64_s32q_small", "a quotient read alone");
}

/// -O1 (bytes and clocks strict) keeps the dword divisor's speed routine: the
/// size form's second `div` is a clock cost where the dividend fits a dword.
#[test]
fn a_function_built_for_o1_divides_by_a_dword_in_the_speed_form() {
    let (dag, _) = quotient_on(&llrm_x86_m16::M16, ISD::SDiv, true, true, false);
    assert_eq!(call_of(&dag).0, "div64_s32q");
}

/// `a / d + a % d` on `i64` called the routine twice, once per op, and the
/// pipeline's remainder was `a - q * d` (a 64-bit multiply per site, x_ll_div
/// +9,534 clk over gcc). The pair is one call that returns both.
#[test]
fn an_i64_quotient_and_remainder_of_the_same_operands_are_one_call() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let ptr = dag.get_register(EVT::I32, 7);
    let chain = dag.entry_node();
    let mut dword = |slot: i32| {
        let info = MemInfo {
            mem_vt: EVT::I32,
            align: 4,
            volatile: false,
            base: MemBase::Stack(slot),
            ext: None,
            space: 0,
            ordering: None,
        };
        dag.get_load(EVT::I32, chain, ptr, info)
    };
    let (a, b, c, d) = (dword(1), dword(2), dword(3), dword(4));
    let x = dag.get_node(ISD::BuildPair, EVT::I64, &[a, b]);
    let y = dag.get_node(ISD::BuildPair, EVT::I64, &[c, d]);
    let quotient = dag.get_node(ISD::UDiv, EVT::I64, &[x, y]);
    let remainder = dag.get_node(ISD::URem, EVT::I64, &[x, y]);
    let sum = dag.get_node(ISD::Add, EVT::I64, &[quotient, remainder]);
    let low = dag.get_node(ISD::Truncate, EVT::I32, &[sum]);
    dag.set_root(low);
    TypeLegalizer::new(&mut dag, &tli).with_target(&llrm_x86_m32::M32).run().unwrap();
    let calls = (0..dag.node_count() as u32)
        .filter(|&node| dag.opcode(SDValue { node, res: 0 }) == x86isd::CALL)
        .count();
    assert_eq!(calls, 1, "one call makes the quotient and the remainder");
}

/// m32 -O1 took the 64-round divide too (clocks regress where both are strict):
/// only m16 asks for the small routine at -O1.
#[test]
fn only_m16_takes_the_size_divide_at_o1() {
    use llrm_target::Target;
    assert!(llrm_x86_m16::M16.small_divide_at_o1());
    assert!(!llrm_x86_m32::M32.small_divide_at_o1());
}

/// DEVIATE DIV-HOT: a bit-a-round `i64` divide in a loop at m32 -Os ran 2,503,700 clocks in
/// x_int64_division (libgcc's: 460,640) and 530,760 in x_ll_div (239,757). In a loop the
/// speed routine; once a call the size one.
#[test]
fn an_i64_divide_in_a_loop_at_size_takes_the_speed_routine() {
    let name = |frequency| call_of(&quotient_in(&llrm_x86_m32::M32, ISD::UDiv, false, true, true, Some(frequency)).0).0;
    assert_eq!(name(1.0), "div64_u_small", "a block that runs once a call keeps the size routine");
    assert_eq!(name(8.0), "div64_u", "a loop takes the speed routine");
}
