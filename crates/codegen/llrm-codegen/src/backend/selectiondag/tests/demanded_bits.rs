use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

const SAMPLES: [u32; 8] = [0, 1, 0xFF, 0x0F0F_0F0F, 0x8000_0001, 0xDEAD_BEEF, 0x1234_5678, 0xFFFF_FFFF];

/// The value of `v` when register 5 holds `x`, 6 `y`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    (x, y): (u32, u32),
) -> u32 {
    let arg = |i: usize| eval(dag, dag.operand(v, i), (x, y));
    match dag.opcode(v) {
        ISD::Register => match dag.node(v).data {
            NodeData::Register(5) => x,
            _ => y,
        },
        ISD::Constant => dag.constant(v).unwrap() as u32,
        ISD::And => arg(0) & arg(1),
        ISD::Or => arg(0) | arg(1),
        ISD::Xor => arg(0) ^ arg(1),
        ISD::Shl => arg(0).checked_shl(arg(1)).unwrap_or(0),
        ISD::Srl => arg(0).checked_shr(arg(1)).unwrap_or(0),
        ISD::Sra => ((arg(0) as i32) >> arg(1).min(31)) as u32,
        ISD::CtPop => arg(0).count_ones(),
        ISD::CtLz => arg(0).leading_zeros(),
        ISD::CtTz => arg(0).trailing_zeros(),
        ISD::Select => {
            if arg(0) != 0 {
                arg(1)
            } else {
                arg(2)
            }
        }
        ISD::Rotl => arg(0).rotate_left(arg(1)),
        ISD::SignExtendInReg => {
            let NodeData::ValueType(ext) = dag.node(dag.operand(v, 1)).data else { panic!("no type") };
            let shift = 32 - ext.size_in_bits();
            (((arg(0) << shift) as i32) >> shift) as u32
        }
        other => panic!("unexpected {other:?}"),
    }
}

fn has(
    dag: &SelectionDAG,
    v: SDValue,
    op: ISD,
) -> bool {
    dag.opcode(v) == op || (0..dag.node(v).ops.len()).any(|i| has(dag, dag.operand(v, i), op))
}

fn count(
    dag: &SelectionDAG,
    v: SDValue,
    op: ISD,
) -> usize {
    usize::from(dag.opcode(v) == op)
        + (0..dag.node(v).ops.len()).map(|i| count(dag, dag.operand(v, i), op)).sum::<usize>()
}

/// `build(x, y)` combined, the value checked for every pair of samples. Only
/// the bits `care` selects are compared: the rewrite may change the others.
fn combined(
    care: u32,
    build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue,
) -> (SelectionDAG, SDValue) {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let root = build(&mut dag, x, y);
    dag.set_root(root);
    let pairs = || SAMPLES.iter().flat_map(|&p| SAMPLES.iter().map(move |&q| (p, q)));
    let before: Vec<u32> = pairs().map(|pair| eval(&dag, root, pair) & care).collect();
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let after: Vec<u32> = pairs().map(|pair| eval(&dag, root, pair) & care).collect();
    assert_eq!(before, after, "the rewrite changed a demanded bit");
    (dag, root)
}

fn k(
    dag: &mut SelectionDAG,
    value: u128,
) -> SDValue {
    dag.get_constant(value, EVT::I32)
}

fn shift(
    dag: &mut SelectionDAG,
    op: ISD,
    x: SDValue,
    by: u128,
) -> SDValue {
    let amount = dag.get_constant(by, EVT::I8);
    dag.get_node(op, EVT::I32, &[x, amount])
}

/// `(or (srl x, 16), (shl x, 16)) & 255` read only the low byte, which the
/// `shl` never reaches: the `shl` and the `or` went to instructions anyway
/// (bench `x_popcount`: `rev32(s) & 255`, `ROL32ri` and an `OR32rr` the pin
/// does not have).
#[test]
fn a_shift_that_reaches_no_demanded_bit_is_dropped() {
    let (dag, root) = combined(u32::MAX, |dag, x, _| {
        let (hi, lo) = (shift(dag, ISD::Srl, x, 16), shift(dag, ISD::Shl, x, 16));
        let or = dag.get_node(ISD::Or, EVT::I32, &[hi, lo]);
        let low = k(dag, 255);
        dag.get_node(ISD::And, EVT::I32, &[or, low])
    });
    assert!(!has(&dag, root, ISD::Shl));
    assert!(!has(&dag, root, ISD::Or));
}

/// A constant with bits no one reads is narrowed: `(or x, 0xF0F0) & 0xFF` is
/// `(or x, 0xF0) & 0xFF`.
#[test]
fn a_constant_is_narrowed_to_the_demanded_bits() {
    let (dag, root) = combined(u32::MAX, |dag, x, _| {
        let (wide, low) = (k(dag, 0xF0F0), k(dag, 0xFF));
        let or = dag.get_node(ISD::Or, EVT::I32, &[x, wide]);
        dag.get_node(ISD::And, EVT::I32, &[or, low])
    });
    let or = dag.operand(root, 0);
    assert_eq!(dag.opcode(or), ISD::Or);
    assert_eq!(dag.constant(dag.operand(or, 1)), Some(0xF0));
}

/// An operand whose demanded bits are known is a constant: the `xor` of two
/// values masked to disjoint bits, read through one of the masks.
#[test]
fn an_operand_known_on_the_demanded_bits_is_replaced() {
    let (dag, root) = combined(u32::MAX, |dag, x, y| {
        let (hi, low) = (k(dag, 0xFF00), k(dag, 0xFF));
        let masked_x = dag.get_node(ISD::And, EVT::I32, &[x, hi]);
        let xor = dag.get_node(ISD::Xor, EVT::I32, &[masked_x, y]);
        dag.get_node(ISD::And, EVT::I32, &[xor, low])
    });
    assert!(!has(&dag, root, ISD::Xor), "the xor reads only y on the low byte");
}

/// `rotl x, 16` read through `& 255` is `srl x, 16`: a rotate was a `rol`
/// (bench `x_popcount`'s `fshl(x, x, 16) & 255`).
#[test]
fn a_rotate_read_in_its_low_bits_is_a_shift() {
    let (dag, root) = combined(255, |dag, x, _| {
        let by = dag.get_constant(16, EVT::I8);
        let rot = dag.get_node(ISD::Rotl, EVT::I32, &[x, by]);
        let low = k(dag, 255);
        dag.get_node(ISD::And, EVT::I32, &[rot, low])
    });
    assert!(!has(&dag, root, ISD::Rotl));
}

/// `(sext_in_reg (or (shl hi, 16), lo), i16)` kept the `shl` and
/// the `or`: no `SIGN_EXTEND_INREG` demanded-bits case. A call returning an
/// `i32` in dx:ax and truncated to `i16` built `shl; movzx; or; movsx` before
/// every multiply-high (recpow m16: +94 B).
#[test]
fn a_sext_in_reg_of_a_rebuilt_pair_reads_only_the_low_half() {
    let (dag, root) = combined(!0, |dag, x, y| {
        let high = shift(dag, ISD::Shl, y, 16);
        let pair = dag.get_node(ISD::Or, EVT::I32, &[high, x]);
        let ty = dag.get_value_type_node(EVT::I16);
        dag.get_node(ISD::SignExtendInReg, EVT::I32, &[pair, ty])
    });
    assert!(!has(&dag, root, ISD::Shl) && !has(&dag, root, ISD::Or), "the high half is still read");
}

/// `shld eax, edx, cl` kept the `and ecx, 31` the IR had on its count
/// (x_ll_arith -O2 m32: +1 instruction a trip against the pin): at a
/// power-of-two width a funnel shift demands only the count's low bits.
#[test]
fn a_funnel_shift_count_loses_its_mask_to_the_width() {
    let mut dag = dag();
    let (x, y, z) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6), dag.get_register(EVT::I32, 7));
    let thirty_one = dag.get_constant(31, EVT::I32);
    let count = dag.get_node(ISD::And, EVT::I32, &[z, thirty_one]);
    let root = dag.get_node(ISD::FShl, EVT::I32, &[x, y, count]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::FShl);
    assert_eq!(dag.opcode(dag.operand(root, 2)), ISD::Register, "the count kept its and");
}

/// `trunc (and x, 31)` stayed wide, so a shift count read the byte through
/// `and ecx, 31` where the pin's `movzx ecx, byte [m]` does (visitTRUNCATE,
/// before operations are legal).
#[test]
fn a_truncated_and_with_a_constant_is_the_and_of_the_truncation() {
    let mut dag = dag();
    let z = dag.get_register(EVT::I32, 7);
    let thirty_one = dag.get_constant(31, EVT::I32);
    let wide = dag.get_node(ISD::And, EVT::I32, &[z, thirty_one]);
    let root = dag.get_node(ISD::Truncate, EVT::I8, &[wide]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    assert_eq!((dag.opcode(root), dag.value_type(root)), (ISD::And, EVT::I8));
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Truncate);
}

/// A `shl` narrowed to 32 bits took its amount in `i32`, not the target's
/// `i8`, and no pattern matched it (`CannotSelect(Shl)`, bench `histo` at
/// m64 once the narrowing ran after legalization).
#[test]
fn a_narrowed_shift_takes_the_targets_amount_type() {
    use super::super::x86_lowering::X86Lowering;
    let mut dag = dag();
    let x = dag.get_register(EVT::I64, 5);
    let (three, mask) = (dag.get_constant(3, EVT::I8), dag.get_constant(0xffff_ffff, EVT::I64));
    let shl = dag.get_node(ISD::Shl, EVT::I64, &[x, three]);
    let root = dag.get_node(ISD::And, EVT::I64, &[shl, mask]);
    dag.set_root(root);
    let mut hooks = X86Lowering {
        multiply_early_out: None,
        imul_if_smaller: false, hot_chains: false,
        share_products: false,
        cmov: true,
        sse2: false,
        min_size: false,
        three_address_adds: false,
        i16_mul_chains: false,
        sext_load: true,
        zext_free: true,
        branch_constant_selects: false,
        profile: None,
    };
    let tli = TargetLowering::x86_64(Features::default(), MVT::I64);
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::AfterLegalizeDag).with_tli(&tli).run();
    fn narrowed_shifts(
        dag: &SelectionDAG,
        v: SDValue,
        out: &mut Vec<EVT>,
    ) {
        if dag.opcode(v) == ISD::Shl && dag.value_type(v) == EVT::I32 {
            out.push(dag.value_type(dag.operand(v, 1)));
        }
        for i in 0..dag.node(v).ops.len() {
            narrowed_shifts(dag, dag.operand(v, i), out);
        }
    }
    let mut amounts = Vec::new();
    narrowed_shifts(&dag, dag.get_root(), &mut amounts);
    assert_eq!(amounts, [EVT::I8], "no i32 shl, or its amount is not i8");
}

/// `(ashr x, 4) << 28` reads the low nibble of the shift, which no sign bit
/// reaches: a logical shift (TargetLowering.cpp#L2146). The `sra` stayed.
#[test]
fn an_arithmetic_shift_that_no_sign_bit_reaches_is_a_logical_shift() {
    let (dag, root) = combined(u32::MAX, |dag, x, _| {
        let sra = shift(dag, ISD::Sra, x, 4);
        shift(dag, ISD::Shl, sra, 28)
    });
    assert!(!has(&dag, root, ISD::Sra));
}

/// `(ashr x, 31) << 31` reads one bit, which is the sign: a logical shift even
/// for an amount the combiner cannot see.
#[test]
fn an_arithmetic_shift_read_at_one_bit_is_a_logical_shift() {
    let (dag, root) = combined(u32::MAX, |dag, x, y| {
        let mask = k(dag, 31);
        let by = dag.get_node(ISD::And, EVT::I32, &[y, mask]);
        let sra = dag.get_node(ISD::Sra, EVT::I32, &[x, by]);
        shift(dag, ISD::Shl, sra, 31)
    });
    assert!(!has(&dag, root, ISD::Sra));
}

/// The count of a 32-bit value is at most 32: `known_bits` has the bits from
/// bit 6 up zero (`computeKnownBits`, SelectionDAG.cpp#L3921), which is what
/// drops `(ctpop x) & 63` (wasm `db_ctpop`: 4 instructions, the pin 2).
#[test]
fn a_bit_count_has_its_high_bits_known_zero() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    for count in [ISD::CtPop, ISD::CtLz, ISD::CtTz] {
        let n = dag.get_node(count, EVT::I32, &[x]);
        assert_eq!(super::super::known_bits::known_bits(&dag, n).zero, !0x3Fu32 as u128 & 0xFFFF_FFFF, "{count:?}");
    }
}

/// `(select c, (x & 0xFF), (x & 0x0F)) << 28` reads the low nibble of the
/// select, so an arm loses its `and` (wasm `db_sel`: 12 instructions, the
/// pin 8).
#[test]
fn the_arms_of_a_select_are_asked_for_the_demanded_bits() {
    let (dag, root) = combined(u32::MAX, |dag, x, y| {
        let (wide, narrow) = (k(dag, 0xFF), k(dag, 0x0F));
        let a = dag.get_node(ISD::And, EVT::I32, &[x, wide]);
        let b = dag.get_node(ISD::And, EVT::I32, &[x, narrow]);
        let select = dag.get_node(ISD::Select, EVT::I32, &[y, a, b]);
        shift(dag, ISD::Shl, select, 28)
    });
    // One arm is rewritten per visit; the select's user is not asked again.
    assert!(count(&dag, root, ISD::And) < 2, "both arms kept their mask");
}

/// `mulhu (and x, 255), 255` is below 2^16, so its high word is 0. The pin
/// asks `SimplifyDemandedBits` from `visitMULHU` ([L5821]) just to fold such a
/// node by known bits; ours never asked, and the multiply stayed (m68k, x86
/// m16: a `mul` of two masked bytes kept as a call/long multiply).
///
/// [L5821]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5821
#[test]
fn a_high_multiply_of_two_masked_bytes_is_zero() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let ff = dag.get_constant(0xFF, EVT::I32);
    let masked = dag.get_node(ISD::And, EVT::I32, &[x, ff]);
    let root = dag.get_node(ISD::MulHU, EVT::I32, &[masked, ff]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.constant(dag.get_root()), Some(0));
}

/// `(x << 16) * (y << 16)` has no bit set in 32: `visitMUL` ends in
/// `SimplifyDemandedBits` ([L5155]), which finds all 32 bits known.
///
/// [L5155]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/DAGCombiner.cpp#L5155
#[test]
fn a_product_of_two_shifted_values_whose_low_word_is_clear_is_zero() {
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I32, 5), dag.get_register(EVT::I32, 6));
    let sixteen = dag.get_constant(16, EVT::I32);
    let a = dag.get_node(ISD::Shl, EVT::I32, &[x, sixteen]);
    let b = dag.get_node(ISD::Shl, EVT::I32, &[y, sixteen]);
    let root = dag.get_node(ISD::Mul, EVT::I32, &[a, b]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    assert_eq!(dag.constant(dag.get_root()), Some(0));
}

/// `(x + 0xFFFF) & 0xFFFF`: only the low 16 bits are read, and the addend has
/// them all set, so the pin makes it `x + -1` (`ADD` case of
/// `SimplifyDemandedBits`, [L3013]). A target that did not ask kept the
/// 16-bit constant (m68k, z80, 6502, x86 m16/m32).
///
/// [L3013]:
/// https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/SelectionDAG/TargetLowering.cpp#L3013
#[test]
fn an_addend_with_every_demanded_bit_set_is_minus_one_for_any_target() {
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let low = dag.get_constant(0xFFFF, EVT::I32);
    let sum = dag.get_node(ISD::Add, EVT::I32, &[x, low]);
    let root = dag.get_node(ISD::And, EVT::I32, &[sum, low]);
    dag.set_root(root);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).run();
    let root = dag.get_root();
    let add = dag.operand(root, 0);
    assert_eq!(dag.opcode(add), ISD::Add, "root {:?}", dag.opcode(root));
    assert_eq!(dag.constant(dag.operand(add, 1)), Some(0xFFFF_FFFF));
}

/// `(x & -64) << 4` reads 28 bits of the mask. The pin's generic shrink writes
/// `0x0FFFFFC0` (an imm32 `and`, 3 B longer than `-64`); x86 keeps a mask that
/// is a sign-extended imm8 at m16 (DEVIATE SD212c). x86_16 `div_constants_ulong
/// -O2` grew 6 B when the call sites ran.
fn and_mask_under_a_shift(m16: bool) -> Option<u128> {
    use super::super::x86_lowering::X86Lowering;
    let mut dag = dag();
    let x = dag.get_register(EVT::I32, 5);
    let mask = dag.get_constant(0xFFFF_FFC0, EVT::I32);
    let and = dag.get_node(ISD::And, EVT::I32, &[x, mask]);
    let root = shift(&mut dag, ISD::Shl, and, 4);
    dag.set_root(root);
    let mut hooks = X86Lowering {
        multiply_early_out: None,
        imul_if_smaller: false, hot_chains: false,
        share_products: false,
        cmov: true,
        sse2: false,
        min_size: false,
        three_address_adds: false,
        i16_mul_chains: m16,
        sext_load: true,
        zext_free: false,
        branch_constant_selects: false,
        profile: None,
    };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::BeforeLegalizeTypes).run();
    fn mask_of(
        dag: &SelectionDAG,
        v: SDValue,
    ) -> Option<u128> {
        if dag.opcode(v) == ISD::And {
            if let Some(c) = dag.constant(dag.operand(v, 1)) {
                return Some(c);
            }
        }
        (0..dag.node(v).ops.len()).find_map(|i| mask_of(dag, dag.operand(v, i)))
    }
    mask_of(&dag, dag.get_root())
}

#[test]
fn an_x86_and_mask_that_is_an_imm8_keeps_its_short_form() {
    assert_eq!(and_mask_under_a_shift(true), Some(0xFFFF_FFC0));
}

/// m32 and m64 keep the pin's shrink.
#[test]
fn an_x86_and_mask_is_shrunk_as_the_pin_does_off_m16() {
    assert_eq!(and_mask_under_a_shift(false), Some(0x0FFF_FFC0));
}
