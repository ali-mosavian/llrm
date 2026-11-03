use super::super::dag_combiner::{CombineLevel, DagCombiner};
use super::super::x86_lowering::X86Lowering;
use super::*;

const SAMPLES: [u32; 8] = [0, 1, 0x7F, 0x80, 0x7FFF, 0x8000, 0xFFFF, 0x1_2345];

/// The value of `v` (masked to its width) when register 5 holds `x`, 6 `y`.
fn eval(
    dag: &SelectionDAG,
    v: SDValue,
    (x, y): (u32, u32),
) -> u32 {
    let width = dag.value_type(v).size_in_bits();
    let mask = |w: u32| if w >= 32 { u32::MAX } else { (1u32 << w) - 1 };
    let from = |i: usize| dag.value_type(dag.operand(v, i)).size_in_bits();
    let arg = |i: usize| eval(dag, dag.operand(v, i), (x, y));
    mask(width)
        & match dag.opcode(v) {
            ISD::Register => match dag.node(v).data {
                NodeData::Register(5) => x,
                _ => y,
            },
            ISD::Constant => dag.constant(v).unwrap() as u32,
            ISD::Add => arg(0).wrapping_add(arg(1)),
            ISD::Sub => arg(0).wrapping_sub(arg(1)),
            ISD::Mul => arg(0).wrapping_mul(arg(1)),
            ISD::And => arg(0) & arg(1),
            ISD::Or => arg(0) | arg(1),
            ISD::Xor => arg(0) ^ arg(1),
            ISD::Shl => arg(0).checked_shl(arg(1)).unwrap_or(0),
            ISD::Srl => arg(0).checked_shr(arg(1)).unwrap_or(0),
            ISD::Sra => ((((arg(0) << (32 - width)) as i32) >> (32 - width)) >> arg(1).min(31)) as u32,
            ISD::AnyExtend | ISD::Truncate => arg(0),
            ISD::ZeroExtend => arg(0) & mask(from(0)),
            ISD::SignExtend => (((arg(0) << (32 - from(0))) as i32) >> (32 - from(0))) as u32,
            ISD::SignExtendInReg => {
                let NodeData::ValueType(inner) = dag.node(dag.operand(v, 1)).data else { panic!("a type") };
                let up = 32 - inner.size_in_bits();
                (((arg(0) << up) as i32) >> up) as u32
            }
            other => panic!("unexpected {other:?}"),
        }
}

/// `build` combined after legalization for m32 (or m16), the value checked
/// over the 16-bit samples before and after.
fn promoted(
    far: bool,
    build: impl FnOnce(&mut SelectionDAG, SDValue, SDValue) -> SDValue,
) -> (SelectionDAG, SDValue) {
    let tli = if far {
        TargetLowering::x86_16(Features::default(), MVT::I16)
    } else {
        TargetLowering::x86_32(Features::default(), MVT::I32)
    };
    let mut dag = dag();
    let (x, y) = (dag.get_register(EVT::I16, 5), dag.get_register(EVT::I16, 6));
    let root = build(&mut dag, x, y);
    dag.set_root(root);
    let pairs = || SAMPLES.iter().flat_map(|&p| SAMPLES.iter().map(move |&q| (p & 0xFFFF, q & 0xFFFF)));
    let before: Vec<u32> = pairs().map(|pair| eval(&dag, root, pair)).collect();
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::AfterLegalizeDag).with_tli(&tli).run();
    let root = dag.get_root();
    let after: Vec<u32> = pairs().map(|pair| eval(&dag, root, pair)).collect();
    assert_eq!(before, after, "the promotion changed the value");
    (dag, root)
}

/// `add i16` stayed a 16-bit `add` (`add ax, bx`: an operand-size prefix and a
/// partial-register write); the pin does it in 32 bits and truncates (bench
/// `x_switch_dense`: `urem i16 x, 14` as `imul r16`/`sub r16`).
#[test]
fn a_word_operation_is_done_in_a_dword() {
    for op in [ISD::Add, ISD::Sub, ISD::Mul, ISD::And, ISD::Or, ISD::Xor] {
        let (dag, root) = promoted(false, |dag, x, y| dag.get_node(op, EVT::I16, &[x, y]));
        assert_eq!(dag.opcode(root), ISD::Truncate, "{op:?}");
        assert_eq!(dag.value_type(dag.operand(root, 0)), EVT::I32, "{op:?}");
        assert_eq!(dag.opcode(dag.operand(root, 0)), op, "{op:?}");
    }
}

/// A right shift needs the high bits right: `srl` zero-extends the operand
/// (the pin's `srl (zero_extend x), 3`: X86 keeps the `and 0xFFFF` a `movzx`
/// matches instead of trimming it to the bits the shift reads, 0xFFF8, which
/// #4399's demanded bits did), `sra` sign-extends it (`sext_in_reg (aext x)`
/// folds to `sext x`, `visitSIGN_EXTEND_INREG`); a left shift needs neither.
#[test]
fn a_word_shift_extends_its_operand_by_its_kind() {
    for (op, extend) in [(ISD::Srl, ISD::ZeroExtend), (ISD::Sra, ISD::SignExtend), (ISD::Shl, ISD::AnyExtend)] {
        let (dag, root) = promoted(false, |dag, x, _| {
            let by = dag.get_constant(3, EVT::I8);
            dag.get_node(op, EVT::I16, &[x, by])
        });
        assert_eq!(dag.opcode(root), ISD::Truncate, "{op:?}");
        assert_eq!(dag.opcode(dag.operand(dag.operand(root, 0), 0)), extend, "{op:?}");
    }
}

/// A byte multiply by a constant is a dword multiply; a byte add is not
/// promoted, nor is anything at m16 (no wider register).
#[test]
fn only_words_and_byte_multiplies_are_promoted_and_not_at_m16() {
    let tli = TargetLowering::x86_32(Features::default(), MVT::I32);
    let mut dag = dag();
    let x = dag.get_register(EVT::I8, 5);
    let (c, y) = (dag.get_constant(5, EVT::I8), dag.get_register(EVT::I8, 6));
    let mul = dag.get_node(ISD::Mul, EVT::I8, &[x, c]);
    let add = dag.get_node(ISD::Add, EVT::I8, &[mul, y]);
    dag.set_root(add);
    let mut hooks = X86Lowering { cmov: true, sse2: false, min_size: false, i16_mul_chains: false, profile: None };
    DagCombiner::new(&mut dag, &mut hooks, CombineLevel::AfterLegalizeDag).with_tli(&tli).run();
    let root = dag.get_root();
    assert_eq!(dag.opcode(root), ISD::Add, "a byte add stays");
    assert_eq!(dag.opcode(dag.operand(root, 0)), ISD::Truncate, "the byte multiply is promoted");
    let (dag, root) = promoted(true, |dag, x, y| dag.get_node(ISD::Add, EVT::I16, &[x, y]));
    assert_eq!(dag.opcode(root), ISD::Add, "m16 keeps its word add");
}
