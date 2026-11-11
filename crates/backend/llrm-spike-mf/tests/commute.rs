//! `commute_tied` and the hoisted phi copy: which source of `a + b` the
//! result is tied to.

mod common;

use common::{Two, branch, op, v};
use llrm_spike_mf::commute::liveness_steps;
use llrm_spike_mf::regalloc::{eliminate_phis_with, successors};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi, commute_tied};

fn sum_into(
    result: u32,
    tied: u32,
    other: u32,
) -> Inst {
    let mut add = op("add", vec![Operand::Def(v(result)), Operand::Use(v(tied)), Operand::Use(v(other))]);
    add.ties = vec![(0, 1)];
    add.commutable = true;
    add
}

fn loop_function(insts: Vec<Inst>) -> MFunction<()> {
    let mut f = MFunction::new("f");
    f.classes = vec![(); 24];
    f.blocks.push(Block { id: 0, insts: vec![], phis: vec![] });
    f.blocks.push(Block { id: 1, insts, phis: vec![] });
    f
}

fn tied_source(f: &MFunction<()>) -> Operand {
    let add = f.blocks[1].insts.iter().find(|inst| inst.kind == Kind::Op("add")).unwrap();
    add.operands[add.ties[0].1].clone()
}

/// `s = a + b` where `s` flows on into `b`'s register: tied to `b`.
/// (v6 is b's loop register, v7 its copy read by the add; v9 is a's copy.)
fn rotating() -> Vec<Inst> {
    vec![
        Inst::copy(v(6), v(20)),
        Inst::copy(v(19), v(6)),
        Inst::copy(v(7), v(6)),
        Inst::copy(v(9), v(8)),
        sum_into(10, 9, 7),
        Inst::copy(v(11), v(10)),
        Inst::copy(v(20), v(11)),
    ]
}

/// The add took `a`'s register and the pair rotated through a temporary: a
/// move more than tying the result to `b`, the register the sum flows into.
#[test]
fn the_source_whose_register_the_sum_flows_into_is_tied() {
    let mut f = loop_function(rotating());
    commute_tied::<Two>(&mut f);
    assert_eq!(tied_source(&f), Operand::Use(v(7)));
}

/// With no copy chain from `b` back to the result the order stays.
#[test]
fn without_a_copy_chain_to_the_result_the_order_stays() {
    let mut f = loop_function(vec![
        Inst::copy(v(7), v(6)),
        Inst::copy(v(9), v(8)),
        sum_into(10, 9, 7),
        Inst::copy(v(11), v(10)),
        Inst::copy(v(21), v(11)),
    ]);
    commute_tied::<Two>(&mut f);
    assert_eq!(tied_source(&f), Operand::Use(v(9)));
}

/// The phi's copy of a value goes before that value's last read, so the read
/// ends the register; at the end of the block it kept the register live.
#[test]
fn a_phi_copy_goes_before_the_last_read_of_its_source() {
    let build = || {
        let mut f = MFunction::<()>::new("f");
        f.classes = vec![(); 4];
        let mut jump = Inst::new(Kind::Op("jump"), vec![Operand::Block(1)]);
        jump.terminator = true;
        jump.barrier = true;
        f.blocks.push(Block {
            id: 0,
            insts: vec![
                op("def", vec![Operand::Def(v(0))]),
                op("read", vec![Operand::Use(v(0))]),
                op("other", vec![Operand::Def(v(2))]),
                jump,
            ],
            phis: vec![],
        });
        f.blocks.push(Block { id: 1, insts: vec![], phis: vec![Phi { result: v(1), incoming: vec![(0, v(0))] }] });
        f
    };
    let position = |hoist| {
        let mut f = build();
        eliminate_phis_with(&mut f, hoist);
        let _ = successors(&f);
        f.blocks[0].insts.iter().position(|inst| inst.kind == Kind::Copy).unwrap()
    };
    assert_eq!(position(false), 3);
    assert_eq!(position(true), 1);
}

/// `Facts::of` solved liveness over a boolean per (block, register) with a
/// scan of every register per block per round: 3000 blocks and 30000
/// registers took 450 ms (div_constants_long at m68k spent 16% of its
/// instructions there). Bit sets visit one word per 64 registers; the bound
/// is a count, so a loaded machine cannot fail it.
#[test]
fn liveness_of_many_blocks_and_registers_is_cheap() {
    let blocks = 3000u32;
    let mut function: MFunction<()> = MFunction::new("f");
    for b in 0..blocks {
        let mut insts =
            vec![op("work", vec![Operand::Def(v(10 * b)), Operand::Use(v(10 * ((b + blocks - 1) % blocks)))])];
        if b + 1 == blocks {
            insts.push(branch(0));
        }
        function.blocks.push(Block { id: b, insts, phis: vec![] });
    }
    function.classes = vec![(); 10 * blocks as usize];
    let steps = liveness_steps(&function);
    let words = (10 * blocks as u64).div_ceil(64);
    let rounds = steps / (blocks as u64 * words);
    assert!(rounds <= 4, "{rounds} rounds of {blocks} blocks x {words} words ({steps} steps)");
}

mod accumulator {
    //! `commute_accumulator`: `a = first; op a, a, second` where `second`
    //! arrives in `a`.
    use llrm_lir::registers::RegId;
    use llrm_spike_mf::regalloc::Target;
    use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Reg, commute_accumulator};

    /// A one-accumulator target: register 0 is the accumulator, 1 an index
    /// register, and the second source of `add` is a memory class.
    struct Acc;

    const MEM: u8 = 1;
    const ANY: u8 = 3;

    impl Target for Acc {
        type Class = u8;
        fn meet(
            one: u8,
            other: u8,
        ) -> Option<u8> {
            (one & other != 0).then_some(one & other)
        }
        fn any() -> u8 {
            ANY
        }
        fn candidates(_: u8) -> Vec<RegId> {
            vec![RegId::from_raw(0), RegId::from_raw(1)]
        }
        fn view(
            root: RegId,
            _: u8,
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
        fn operand_class(
            name: &str,
            index: usize,
        ) -> Option<u8> {
            (name == "add" && index == 2).then_some(MEM)
        }
    }

    const A: u32 = 0;
    const X: u32 = 1;

    fn phys(n: u32) -> Reg {
        Reg::Phys(RegId::from_raw(n as u8))
    }

    /// Entry block copies the arguments out of `a` and `x`; the next block
    /// moves one into `a` and adds the other.
    fn function(
        first: Reg,
        second: Reg,
    ) -> MFunction<u8> {
        let mut f = MFunction::new("f");
        f.classes = vec![ANY, ANY, ANY, ANY];
        let entry = vec![Inst::copy(Reg::Virt(0), phys(A)), Inst::copy(Reg::Virt(1), phys(X))];
        let mut add =
            Inst::new(Kind::Op("add"), vec![Operand::Def(phys(A)), Operand::Use(phys(A)), Operand::Use(second)]);
        add.commutable = true;
        let body = vec![Inst::copy(phys(A), first), add];
        f.blocks.push(Block { id: 0, insts: entry, phis: vec![] });
        f.blocks.push(Block { id: 1, insts: body, phis: vec![] });
        f.blocks[0]
            .insts
            .push(
                {
                    let mut jump = Inst::new(Kind::Op("jump"), vec![Operand::Block(1)]);
                    jump.terminator = true;
                    jump
                },
            );
        f
    }

    fn loaded(f: &MFunction<u8>) -> Reg {
        let Operand::Use(r) = f.blocks[1].insts[0].operands[1] else { panic!() };
        r
    }

    /// `a + x` read as `x + a`: `a = copy x-value; add a, a, a-value` stored
    /// the argument that is already in `a` and moved the other (`sta; txa;
    /// adc`). The one that arrives in `a` goes first, and the other becomes
    /// the cell.
    #[test]
    fn the_source_that_arrives_in_the_accumulator_is_loaded_into_it() {
        let mut f = function(Reg::Virt(1), Reg::Virt(0));
        commute_accumulator::<Acc>(&mut f);
        assert_eq!(loaded(&f), Reg::Virt(0));
        assert_eq!(f.classes[1], MEM, "the other source is the memory operand");
        assert_eq!(f.classes[0], ANY);
    }

    #[test]
    fn the_right_order_is_left_alone() {
        let mut f = function(Reg::Virt(0), Reg::Virt(1));
        let before = f.blocks[1].insts.clone();
        commute_accumulator::<Acc>(&mut f);
        assert_eq!(f.blocks[1].insts, before);
    }

    /// A write of the accumulator between the argument copy and the load: the
    /// copy no longer holds the value, so nothing is gained.
    #[test]
    fn a_clobbered_accumulator_is_not_a_source() {
        let mut f = function(Reg::Virt(1), Reg::Virt(0));
        f.blocks[0].insts.insert(2, Inst::new(Kind::Op("clobber"), vec![Operand::Def(phys(A))]));
        let before = f.blocks[1].insts.clone();
        commute_accumulator::<Acc>(&mut f);
        assert_eq!(f.blocks[1].insts, before);
    }

    /// The other source is a lane an instruction computes (`unpack`), not a
    /// copy: giving it the memory place moved it there through the
    /// accumulator the swap keeps `second` in (`phi_slot_unknown_arm`: +3 B and
    /// 22k clocks, 2026-11-08).
    #[test]
    fn a_source_an_instruction_computes_is_not_moved_into_the_place() {
        let mut f = function(Reg::Virt(1), Reg::Virt(0));
        f.blocks[0].insts[1] =
            Inst::new(Kind::Op("unpack"), vec![Operand::Def(Reg::Virt(1)), Operand::Use(Reg::Virt(2))]);
        let before = f.blocks[1].insts.clone();
        commute_accumulator::<Acc>(&mut f);
        assert_eq!(f.blocks[1].insts, before);
    }
}
