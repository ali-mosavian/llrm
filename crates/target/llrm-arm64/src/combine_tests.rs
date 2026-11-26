use super::*;

/// A block of 30,000 adds of two registers made outside it: each root looked
/// back through every instruction before it for a product and a sibling (no
/// scan finds one), 450 M operations visited; `fcmp_all` -O2 spent 52% of its
/// 3 G instructions there. With the definitions found by search it is a few
/// milliseconds; the bound is 1,000 times that.
#[test]
fn a_long_block_of_adds_is_not_scanned_back_from_every_root() {
    let mut function = MFunc::new("f");
    let (a, b) = (function.gpr(), function.gpr());
    let mut ops: Vec<Op> = (0..30_000)
        .map(|_| {
            let rd = function.gpr();
            Op::I(Inst::AddSubShifted {
                op: AddSub { subtract: false, set_flags: false },
                width: Width::W,
                rd,
                rn: a,
                rm: b,
                shift: Shift::Lsl,
                amount: 0,
            })
        })
        .collect();
    ops.push(Op::Return { uses: vec![], tail: None });
    function.blocks.push(crate::mfunc::MBlock { label: "entry".into(), ops: ops.into() });
    let started = std::time::Instant::now();
    run(&mut function, false);
    let took = started.elapsed();
    assert!(took < std::time::Duration::from_secs(5), "30,000 adds took {took:?}");
}

/// 800 `mul` + `add` pairs in one block: each root's depth trace walked every
/// read back through the block for its definition, and was made again for
/// every candidate; 800 pairs took 20 s, 1,500 took 140 s. Each
/// pair is fused (`madd`), and the whole block in well under the bound.
#[test]
fn a_long_block_of_products_is_traced_once_per_change() {
    let mut function = MFunc::new("f");
    let (a, b, c) = (function.gpr(), function.gpr(), function.gpr());
    let mut ops = vec![];
    for _ in 0..800 {
        let (product, sum) = (function.gpr(), function.gpr());
        ops.push(Op::I(Inst::ThreeSource { op: Mul3::Madd, width: Width::W, rd: product, rn: a, rm: b, ra: Gpr::R31 }));
        ops.push(Op::I(Inst::AddSubShifted {
            op: AddSub { subtract: false, set_flags: false },
            width: Width::W,
            rd: sum,
            rn: product,
            rm: c,
            shift: Shift::Lsl,
            amount: 0,
        }));
    }
    ops.push(Op::Return { uses: vec![], tail: None });
    function.blocks.push(crate::mfunc::MBlock { label: "entry".into(), ops: ops.into() });
    let started = std::time::Instant::now();
    run(&mut function, false);
    let took = started.elapsed();
    let fused = function.blocks[0]
        .ops
        .iter()
        .filter(|op| matches!(op, Op::I(Inst::ThreeSource { ra, .. }) if *ra != Gpr::R31))
        .count();
    assert_eq!(fused, 800, "premise: each pair is a madd");
    assert!(took < std::time::Duration::from_secs(5), "800 pairs took {took:?}");
}
