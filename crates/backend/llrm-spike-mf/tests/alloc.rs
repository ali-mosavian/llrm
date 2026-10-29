//! The allocator over a two-register file, `r0` and `r1`.

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::{Allocate, GraphColoring, Target, eliminate_phis};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi, Reg};

const R0: RegId = RegId::from_raw(0);
const R1: RegId = RegId::from_raw(1);

struct Two;

impl Target for Two {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<RegId> {
        vec![R0, R1]
    }

    fn view(
        root: RegId,
        _: (),
    ) -> RegId {
        root
    }

    fn root(register: RegId) -> RegId {
        register
    }

    fn reserved(_: RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<RegId> {
        vec![]
    }
}

fn v(n: u32) -> Reg {
    Reg::Virt(n)
}

fn op(
    name: &'static str,
    operands: Vec<Operand>,
) -> Inst {
    Inst::new(Kind::Op(name), operands)
}

fn function(
    insts: Vec<Inst>,
    registers: usize,
) -> MFunction<()> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); registers];
    function.blocks.push(Block { id: 0, insts, phis: vec![] });
    function
}

/// Two values live together get two registers; a copy whose source dies joins
/// them.
#[test]
fn interfering_values_differ_and_a_dying_copy_joins() {
    let function = function(
        vec![
            op("def", vec![Operand::Def(v(0))]),
            op("def", vec![Operand::Def(v(1))]),
            op("use2", vec![Operand::Use(v(0)), Operand::Use(v(1))]),
            Inst::copy(v(2), v(1)),
            op("use", vec![Operand::Use(v(2))]),
        ],
        3,
    );
    let assignment = GraphColoring::<Two>::default().allocate(&function).unwrap().0;
    assert_ne!(assignment[0], assignment[1]);
    assert_eq!(assignment[1], assignment[2]);
}

/// Three values live together do not fit two registers: an error, not a clash.
#[test]
fn too_many_live_values_is_an_error() {
    let function = function(
        vec![
            op("def", vec![Operand::Def(v(0))]),
            op("def", vec![Operand::Def(v(1))]),
            op("def", vec![Operand::Def(v(2))]),
            op("use3", vec![Operand::Use(v(0)), Operand::Use(v(1)), Operand::Use(v(2))]),
        ],
        3,
    );
    assert!(GraphColoring::<Two>::default().allocate(&function).is_err());
}

/// A phi becomes a copy at the end of its predecessor, before the branch.
#[test]
fn a_phi_copies_before_the_terminator() {
    let mut branch = op("br", vec![Operand::Block(1)]);
    branch.terminator = true;
    let mut function = function(vec![op("def", vec![Operand::Def(v(0))]), branch], 2);
    function
        .blocks
        .push(
            Block {
                id: 1,
                insts: vec![op("use", vec![Operand::Use(v(1))])],
                phis: vec![Phi { result: v(1), incoming: vec![(0, v(0))] }],
            },
        );
    eliminate_phis(&mut function);
    let kinds: Vec<Kind> = function.blocks[0].insts.iter().map(|inst| inst.kind).collect();
    assert_eq!(kinds, [Kind::Op("def"), Kind::Copy, Kind::Op("br")]);
    assert_eq!(function.blocks[1].insts[0].kind, Kind::Copy);
}
