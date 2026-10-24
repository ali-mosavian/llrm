use super::super::dag_combiner::{CombineLevel, DagCombiner, NoCombines};
use super::*;

/// The value of `v` when register 5 holds `x`: the nodes the divide folds
/// make, over `bits`-wide values.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    x: u128,
) -> u128 {
    let bits = dag.value_type(v).size_in_bits();
    let mask = super::super::fold::mask(bits);
    let signed = |value: u128, width: u32| ((value << (128 - width)) as i128) >> (128 - width);
    let arg = |i: usize| eval(dag, dag.operand(v, i), x);
    let wide = |i: usize| dag.value_type(dag.operand(v, i)).size_in_bits();
    let sh = |i: usize| arg(i) as u32;
    let r = match dag.opcode(v) {
        ISD::Register => x,
        ISD::Constant => dag.constant(v).unwrap(),
        ISD::Add => arg(0).wrapping_add(arg(1)),
        ISD::Sub => arg(0).wrapping_sub(arg(1)),
        ISD::Mul => arg(0).wrapping_mul(arg(1)),
        ISD::And => arg(0) & arg(1),
        ISD::Truncate => arg(0),
        ISD::ZeroExtend => arg(0) & super::super::fold::mask(wide(0)),
        ISD::SignExtend => signed(arg(0), wide(0)) as u128,
        ISD::Shl => arg(0) << sh(1),
        ISD::Srl => (arg(0) & super::super::fold::mask(wide(0))) >> sh(1),
        ISD::Sra => (signed(arg(0), wide(0)) >> sh(1)) as u128,
        ISD::SMulLoHi | ISD::UMulLoHi => {
            let b = wide(0);
            let product = if dag.opcode(v) == ISD::SMulLoHi {
                (signed(arg(0), b) * signed(arg(1), b)) as u128
            } else {
                (arg(0) & super::super::fold::mask(b)) * (arg(1) & super::super::fold::mask(b))
            };
            if v.res == 1 { product >> b } else { product }
        }
        ISD::MulHS => ((signed(arg(0), bits) * signed(arg(1), bits)) >> bits) as u128,
        ISD::MulHU => (arg(0) * arg(1)) >> bits,
        ISD::Select => {
            if arg(0) & 1 == 1 {
                arg(1)
            } else {
                arg(2)
            }
        }
        ISD::SetCC => match dag.node(dag.operand(v, 2)).data {
            NodeData::CondCode(CondCode::SetEq) => u128::from(arg(0) == arg(1)),
            other => panic!("compare {other:?}"),
        },
        other => panic!("unexpected {other:?}"),
    };
    r & mask
}

fn reaches(
    dag: &SelectionDAG,
    v: SDValue,
    ops: &[ISD],
) -> bool {
    ops.contains(&dag.opcode(v)) || (0..dag.node(v).ops.len()).any(|i| reaches(dag, dag.operand(v, i), ops))
}

fn folded(
    op: ISD,
    vt: EVT,
    d: u128,
) -> (SelectionDAG, SDValue) {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(vt, 5);
    let divisor = dag.get_constant(d, vt);
    let divide = dag.get_node(op, vt, &[x, divisor]);
    dag.set_root(divide);
    DagCombiner::new(&mut dag, &mut NoCombines, CombineLevel::BeforeLegalizeTypes).with_tli(&tli).run();
    let root = dag.get_root();
    (dag, root)
}

/// `x / c` and `x % c` stayed divides, so selection emitted `idiv`/`div`
/// (the 6 SHR32ri/IMUL/CDQ/DIV differences of the m32 shadow). The fold must
/// leave no divide and give the same value for every dividend.
#[test]
fn division_by_a_constant_is_multiplies_and_shifts_with_the_same_value() {
    let samples = |bits: u32| {
        let m = super::super::fold::mask(bits);
        let mut xs = vec![0, 1, 2, 3, 5, 7, 9, 10, 11, 99, 100, 101, 127, 128, 255, 256, 1000, 12345, 65535, 65536];
        xs.extend([m, m - 1, m / 2, m / 2 + 1, m / 2 + 2, m / 3, m / 3 + 1, m / 7 * 6]);
        xs.extend((0..200).map(|i| (i * 0x9E37_79B9_7F4A_7C15u128) & m));
        xs.into_iter().map(|x| x & m).collect::<Vec<_>>()
    };
    for bits in [8u32, 16, 32] {
        let vt = EVT::integer(bits);
        let m = super::super::fold::mask(bits);
        let divisors = [
            3u128,
            5,
            6,
            7,
            9,
            10,
            12,
            25,
            100,
            125,
            641,
            1000,
            4,
            16,
            64,
            2,
            128,
            255 & m,
            m - 1,
            m - 6,
            m - 2,
            m - 3,
        ];
        for d in divisors.into_iter().filter(|&d| d > 1 && d <= m) {
            for op in [ISD::UDiv, ISD::URem, ISD::SDiv, ISD::SRem] {
                let (dag, root) = folded(op, vt, d);
                assert!(
                    !reaches(&dag, root, &[ISD::UDiv, ISD::URem, ISD::SDiv, ISD::SRem]),
                    "i{bits} {op:?} {d}: a divide is left"
                );
                for x in samples(bits) {
                    let signed = |v: u128| ((v << (128 - bits)) as i128) >> (128 - bits);
                    let want = match op {
                        ISD::UDiv => x / d,
                        ISD::URem => x % d,
                        ISD::SDiv => (signed(x).wrapping_div(signed(d))) as u128,
                        _ => (signed(x).wrapping_rem(signed(d))) as u128,
                    } & m;
                    assert_eq!(eval(&dag, root, x), want, "i{bits} {op:?} x={x} d={d}");
                }
            }
        }
    }
}

/// A divide by a constant that is not a power of two uses the multiply-high,
/// not shifts alone.
#[test]
fn a_non_power_of_two_divisor_multiplies() {
    for op in [ISD::UDiv, ISD::SDiv] {
        let (dag, root) = folded(op, EVT::I32, 7);
        assert!(reaches(&dag, root, &[ISD::UMulLoHi, ISD::SMulLoHi, ISD::MulHU, ISD::MulHS]), "{op:?}");
    }
}
