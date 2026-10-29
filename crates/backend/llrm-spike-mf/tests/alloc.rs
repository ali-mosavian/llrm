//! The allocator over a two-register file, `r0` and `r1`.

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::{Allocate, GraphColoring, Target, eliminate_phis};
use llrm_spike_mf::spill::allocate_spilling;
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
    let mut inst = Inst::new(Kind::Op(name), operands);
    // a `def*` stands for an instruction that cannot run twice: it is not
    // rematerialized, so pressure spills it
    inst.side_effects = name.starts_with("def");
    inst
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

struct Spilly;

impl Target for Spilly {
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

    fn spill_store(
        _: (),
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(op("spill_store", vec![Operand::Use(register), Operand::Frame(slot)]))
    }

    fn spill_load(
        _: (),
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(op("spill_load", vec![Operand::Def(register), Operand::Frame(slot)]))
    }
}

fn count(
    function: &MFunction<()>,
    block: usize,
    name: &'static str,
) -> usize {
    function.blocks[block].insts.iter().filter(|inst| inst.kind == Kind::Op(name)).count()
}

fn use1(r: Reg) -> Inst {
    op("use", vec![Operand::Use(r)])
}

/// Three values live together on a two-register file: the one with the fewest
/// occurrences (here `v2`, live all along) goes to a slot, the rest keep
/// registers, and the allocation succeeds (it was an error before spilling
/// existed).
#[test]
fn the_cheapest_of_three_live_values_spills() {
    let mut function = function(
        vec![
            op("def", vec![Operand::Def(v(2))]),
            op("def", vec![Operand::Def(v(0))]),
            op("def", vec![Operand::Def(v(1))]),
            use1(v(0)),
            use1(v(0)),
            use1(v(1)),
            use1(v(1)),
            use1(v(2)),
        ],
        3,
    );
    let (assignment, slots) = allocate_spilling::<Spilly>(&mut function, 10).unwrap();
    assert_eq!(slots, [(10, ())]);
    assert_eq!(count(&function, 0, "spill_store"), 1);
    assert_eq!(count(&function, 0, "spill_load"), 1);
    assert_ne!(assignment.0[0], assignment.0[1], "v0 and v1 stay in registers");
    let store = function.blocks[0].insts.iter().position(|inst| inst.kind == Kind::Op("spill_store")).unwrap();
    assert_eq!(function.blocks[0].insts[store - 1].kind, Kind::Op("def"), "stored right after its definition");
}

/// Values that fit are left alone: no slot, no instruction added.
#[test]
fn nothing_spills_without_pressure() {
    let mut function = function(
        vec![
            op("def", vec![Operand::Def(v(0))]),
            op("def", vec![Operand::Def(v(1))]),
            op("use2", vec![Operand::Use(v(0)), Operand::Use(v(1))]),
        ],
        2,
    );
    let (_, slots) = allocate_spilling::<Spilly>(&mut function, 10).unwrap();
    assert!(slots.is_empty());
    assert_eq!(function.blocks[0].insts.len(), 3);
}

/// `v0` is live through a loop with `v1` and `v2`, used four times outside it;
/// `v1` and `v2` are used once each, inside it. A count of occurrences would
/// spill `v1`; weighting a use in a loop spills `v0` and the loop stays free of
/// spill code.
#[test]
fn a_value_used_in_a_loop_outlives_one_used_after_it() {
    let mut branch = op("brloop", vec![Operand::Block(1)]);
    branch.terminator = true;
    let mut function = function(
        vec![
            op("def", vec![Operand::Def(v(0))]),
            op("def", vec![Operand::Def(v(1))]),
            op("def", vec![Operand::Def(v(2))]),
        ],
        3,
    );
    function
        .blocks
        .push(
            Block {
                id: 1,
                insts: vec![op("use2", vec![Operand::Use(v(1)), Operand::Use(v(2))]), branch],
                phis: vec![],
            },
        );
    function.blocks.push(Block { id: 2, insts: vec![use1(v(0)), use1(v(0)), use1(v(0))], phis: vec![] });
    let (_, slots) = allocate_spilling::<Spilly>(&mut function, 0).unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(count(&function, 0, "spill_store"), 1, "v0 is stored where it is defined");
    assert_eq!(count(&function, 1, "spill_store") + count(&function, 1, "spill_load"), 0, "none in the loop");
    assert_eq!(count(&function, 2, "spill_load"), 3);
}

/// An instruction that writes its result before it has read every operand
/// must not get a use's register for it: a value that dies at an ordinary
/// instruction hands over its register, at an early-clobber one it does not.
#[test]
fn an_early_clobber_def_does_not_take_a_dying_uses_register() {
    let build = |early: bool| {
        let mut both = op("both", vec![Operand::Def(v(1)), Operand::Use(v(0))]);
        both.early_clobber = early;
        function(vec![op("def", vec![Operand::Def(v(0))]), both, use1(v(1))], 2)
    };
    let shared = GraphColoring::<Two>::default().allocate(&build(false)).unwrap().0;
    assert_eq!(shared[0], shared[1], "an ordinary def reuses the dying use's register");
    let apart = GraphColoring::<Two>::default().allocate(&build(true)).unwrap().0;
    assert_ne!(apart[0], apart[1]);
}

/// `r0` is the word `r0:r1`'s own low half in a three-register file: the word
/// `r2` overlaps nothing; the word `w` (id 2) overlaps both `r0` and `r1`.
struct Words;

const W: RegId = RegId::from_raw(2);
const R2: RegId = RegId::from_raw(3);

impl Target for Words {
    type Class = bool;

    fn meet(
        one: bool,
        other: bool,
    ) -> Option<bool> {
        (one == other).then_some(one)
    }

    fn any() -> bool {
        false
    }

    /// `true` is the word class.
    fn candidates(word: bool) -> Vec<RegId> {
        if word { vec![W] } else { vec![R0, R1, R2] }
    }

    fn view(
        root: RegId,
        _: bool,
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

    fn overlaps(
        one: RegId,
        other: RegId,
    ) -> bool {
        one == other || [(W, R0), (W, R1), (R0, W), (R1, W)].contains(&(one, other))
    }

    fn spill_store(
        _: bool,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(op("spill_store", vec![Operand::Use(register), Operand::Frame(slot)]))
    }

    fn spill_load(
        _: bool,
        register: Reg,
        slot: i32,
    ) -> Option<Inst> {
        Some(op("spill_load", vec![Operand::Def(register), Operand::Frame(slot)]))
    }
}

/// A word and a byte live together may not share storage: the byte does not
/// get `r0` or `r1` while the word holds `w`. Before the overlap hook the
/// colouring compared register ids only and handed out `w` and `r0` together.
#[test]
fn a_word_and_a_byte_live_together_do_not_share_storage() {
    let mut function: MFunction<bool> = MFunction::new("f");
    function.classes = vec![true, false];
    function
        .blocks
        .push(
            Block {
                id: 0,
                insts: vec![
                    op("defw", vec![Operand::Def(v(0))]),
                    op("defb", vec![Operand::Def(v(1))]),
                    op("use", vec![Operand::Use(v(0)), Operand::Use(v(1))]),
                ],
                phis: vec![],
            },
        );
    let assignment = GraphColoring::<Words>::default().allocate(&function).unwrap().0;
    assert_eq!(assignment[0], W);
    assert_eq!(assignment[1], R2, "the byte takes the register the word does not cover");
}

/// Three bytes fill `r0`, `r1` and `r2`; a word defined meanwhile needs `w`,
/// which covers `r0` and `r1`, so two bytes must go to slots. The victims are
/// bytes that overlap `w` (no byte's candidates *contain* it), and not the word
/// itself, whose range is its definition and one use. Before the overlap rule
/// the allocation failed with "nothing to spill".
#[test]
fn a_word_that_does_not_fit_spills_a_byte_it_overlaps() {
    let mut function: MFunction<bool> = MFunction::new("f");
    function.classes = vec![false, false, false, true];
    let uses = |regs: &[u32]| op("use", regs.iter().map(|r| Operand::Use(v(*r))).collect());
    function
        .blocks
        .push(
            Block {
                id: 0,
                insts: vec![
                    op("def", vec![Operand::Def(v(0))]),
                    op("def", vec![Operand::Def(v(1))]),
                    op("def", vec![Operand::Def(v(2))]),
                    op("defw", vec![Operand::Def(v(3))]),
                    uses(&[3]),
                    uses(&[0]),
                    uses(&[0]),
                    uses(&[1]),
                    uses(&[1]),
                    uses(&[2]),
                    uses(&[2]),
                ],
                phis: vec![],
            },
        );
    let (assignment, slots) = allocate_spilling::<Words>(&mut function, 0).unwrap();
    assert_eq!(slots.len(), 2);
    assert_eq!(assignment.0[3], W);
}
