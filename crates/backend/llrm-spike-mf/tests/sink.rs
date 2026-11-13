//! `machine_sink` over small diamonds.

mod common;

use common::{Two, branch, op, v};
use llrm_mir::blockfrequency::BranchProbability;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi, machine_sink, machine_sink_in_place};

/// `b0: x = add a, a; branch b2` falls into `b1`, which falls into
/// `b2: r = phi [b0: x] [b1: a]; ret r`. `x` is used by the phi only.
fn diamond(after: Vec<Inst>) -> MFunction<()> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 4];
    let mut head = vec![
        op("arg", vec![Operand::Def(v(0))]),
        op("add", vec![Operand::Def(v(1)), Operand::Use(v(0)), Operand::Use(v(0))]),
    ];
    head.extend(after);
    head.push(branch(2));
    function.blocks.push(Block { id: 0, insts: head, phis: vec![] });
    function.blocks.push(Block { id: 1, insts: vec![], phis: vec![] });
    let join = Block {
        id: 2,
        insts: vec![op("ret", vec![Operand::Use(v(2))])],
        phis: vec![Phi { result: v(2), incoming: vec![(0, v(1)), (1, v(0))] }],
    };
    function.blocks.push(join);
    function
}

fn names(block: &Block) -> Vec<&'static str> {
    block.insts.iter().map(|inst| if let Kind::Op(name) = inst.kind { name } else { "copy" }).collect()
}

/// A value only a phi on one edge reads was computed before the branch, on
/// both paths. It now sits in a block of its own on that edge.
#[test]
fn a_value_read_by_one_edges_phi_moves_onto_the_edge() {
    let mut function = diamond(vec![]);
    assert!(machine_sink::<Two>(&mut function));
    let head = &function.blocks[0];
    // `updateTerminator`: the head now jumps to the block it used to fall into
    assert_eq!(names(head), ["arg", "branch", "jump"]);
    let edge = function.blocks.iter().find(|block| block.id > 2).expect("a block on the split edge");
    assert_eq!(names(edge).first(), Some(&"add"));
    let join = function.blocks.iter().find(|block| block.id == 2).unwrap();
    let from_edge = function.blocks.iter().position(|block| block.id == edge.id).unwrap();
    assert!(join.phis[0].incoming.contains(&(from_edge, v(1))), "{:?}", join.phis[0].incoming);
}

/// A load does not cross a store below it in its block: the store may write
/// what the load reads. The same load with no store there does move.
#[test]
fn a_load_stays_above_a_store() {
    let mut store = op("store", vec![Operand::Use(v(0))]);
    store.may_store = true;
    let mut function = diamond(vec![store]);
    function.blocks[0].insts[1].may_load = true;
    assert!(!machine_sink::<Two>(&mut function));
    assert_eq!(names(&function.blocks[0]), ["arg", "add", "store", "branch"]);

    let mut free = diamond(vec![]);
    free.blocks[0].insts[1].may_load = true;
    assert!(machine_sink::<Two>(&mut free));
}

/// A value read on the path that falls through as well stays put: sinking it
/// would leave that read without a definition.
#[test]
fn a_value_read_on_both_paths_does_not_move() {
    let mut function = diamond(vec![]);
    function.blocks[1].insts.push(op("use", vec![Operand::Use(v(1))]));
    assert!(!machine_sink::<Two>(&mut function));
    assert_eq!(names(&function.blocks[0]), ["arg", "add", "branch"]);
}

/// A select made as blocks writes its result on both paths:
/// `b0: d = f; branch b2`, `b1: d = t`, `b2: ret d`. Forward coalescing of
/// the head's copy rewrote the read of `d` to `f` and left the arm's `d = t`
/// dead: 6502 `ispunct` returned 0 for 1 (2026-11-09).
#[test]
fn a_register_written_on_both_paths_is_not_forwarded_to_one_copy() {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 4];
    let head = vec![
        op("arg", vec![Operand::Def(v(0))]),
        op("arg", vec![Operand::Def(v(1))]),
        Inst::copy(v(2), v(0)),
        branch(2),
    ];
    function.blocks.push(Block { id: 0, insts: head, phis: vec![] });
    function.blocks.push(Block { id: 1, insts: vec![Inst::copy(v(2), v(1))], phis: vec![] });
    function.blocks.push(Block { id: 2, insts: vec![op("ret", vec![Operand::Use(v(2))])], phis: vec![] });
    machine_sink::<Two>(&mut function);
    let ret = function.blocks[2].insts.last().unwrap();
    assert_eq!(ret.operands, [Operand::Use(v(2))], "the read of the select's result names one arm's value");
    assert!(
        function.blocks[0].insts.iter().any(|inst| inst.kind == Kind::Copy),
        "the head's copy of the false arm went"
    );
}

/// A target with room for every register, so no sink is refused for pressure.
struct Wide;

impl llrm_spike_mf::Target for Wide {
    type Class = ();

    fn meet(
        _: (),
        _: (),
    ) -> Option<()> {
        Some(())
    }

    fn any() {}

    fn candidates(_: ()) -> Vec<llrm_lir::registers::RegId> {
        (0..250u8).map(llrm_lir::registers::RegId::from_raw).collect()
    }

    fn view(
        root: llrm_lir::registers::RegId,
        _: (),
    ) -> llrm_lir::registers::RegId {
        root
    }

    fn root(register: llrm_lir::registers::RegId) -> llrm_lir::registers::RegId {
        register
    }

    fn reserved(_: llrm_lir::registers::RegId) -> bool {
        false
    }

    fn caller_saved() -> Vec<llrm_lir::registers::RegId> {
        vec![]
    }

    fn copy_sets_flags(
        _: llrm_lir::registers::RegId,
        _: llrm_lir::registers::RegId,
    ) -> bool {
        false
    }
}

impl llrm_spike_mf::Jump for Wide {
    fn jump(block: u32) -> Inst {
        <Two as llrm_spike_mf::Jump>::jump(block)
    }

    fn reverse(branch: &Inst) -> Option<Inst> {
        <Two as llrm_spike_mf::Jump>::reverse(branch)
    }
}

/// `count` diamonds in one loop. Each head defines a value its join alone
/// reads (so it sinks, and the liveness is solved again), and the last join
/// reads every head's other value, so hundreds of registers are live. Liveness
/// was a hash set per block: 200 diamonds took 350 ms, 20 ms now
/// (div_constants_long at m68k: 8% of its instructions in `pressure_exceeds`).
fn loop_of_diamonds(count: u32) -> MFunction<()> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 2 * count as usize];
    for g in 0..count {
        let (head, mid, join) = (3 * g, 3 * g + 1, 3 * g + 2);
        let (p, x) = (2 * g, 2 * g + 1);
        function
            .blocks
            .push(
                Block {
                    id: head,
                    insts: vec![
                        op("def", vec![Operand::Def(v(p))]),
                        op("add", vec![Operand::Def(v(x)), Operand::Use(v(p)), Operand::Use(v(p))]),
                        branch(join),
                    ],
                    phis: vec![],
                },
            );
        function.blocks.push(Block { id: mid, insts: vec![], phis: vec![] });
        let mut insts = vec![op("use", vec![Operand::Use(v(x))])];
        if g + 1 == count {
            insts.extend((0..count).map(|n| op("use", vec![Operand::Use(v(2 * n))])));
            insts.push(branch(0));
        }
        function.blocks.push(Block { id: join, insts, phis: vec![] });
    }
    function.blocks.push(Block { id: 3 * count, insts: vec![op("ret", vec![])], phis: vec![] });
    function
}

#[test]
fn sinking_in_a_loop_of_many_blocks_solves_liveness_with_bit_sets() {
    let mut function = loop_of_diamonds(200);
    let start = std::time::Instant::now();
    assert!(machine_sink::<Wide>(&mut function), "nothing sank: the test measures nothing");
    let took = start.elapsed();
    assert!(took.as_millis() < 150, "{} blocks took {took:?}", function.blocks.len());
}

/// `diamond` whose `add` is as cheap as a move, with the head's edge to the
/// join taken `percent` of the time.
fn cheap_diamond(percent: u32) -> MFunction<()> {
    let mut function = diamond(vec![]);
    function.blocks[0].insts[1].cheap_as_move = true;
    let taken = BranchProbability::new(percent, 100);
    function.odds = Some(
        [((0, 2), (taken, 0)), ((0, 1), (taken.get_compl(), 1)), ((1, 2), (BranchProbability::get_one(), 0))]
            .into_iter()
            .collect(),
    );
    function
}

/// A cheap value goes onto a split edge the pin's
/// `MachineBranchProbabilityInfo` prices at 40% or less. The sink priced every
/// edge `1 / successors`, so a 37.5% edge of a two-way branch (50%) kept its
/// constant above the branch.
#[test]
fn a_cheap_value_moves_onto_an_edge_taken_at_most_40_percent() {
    let mut function = cheap_diamond(38);
    assert!(machine_sink::<Two>(&mut function), "nothing sank: the test measures nothing");
    assert!(function.blocks.iter().any(|block| block.id > 2), "no block was made for the edge");
}

/// The control: the same function with the edge at 60% keeps the value.
#[test]
fn a_cheap_value_stays_above_an_edge_taken_over_40_percent() {
    let mut function = cheap_diamond(60);
    assert!(!machine_sink::<Two>(&mut function));
    assert!(function.blocks.iter().all(|block| block.id <= 2));
}

/// With no odds the edge is `1 / successors`, as before: 50% of a two-way
/// branch is over the threshold.
#[test]
fn a_cheap_value_stays_when_the_function_has_no_odds() {
    let mut function = cheap_diamond(38);
    function.odds = None;
    assert!(!machine_sink::<Two>(&mut function));
}

fn copy(
    to: u32,
    from: u32,
) -> Inst {
    let mut inst = op("copy", vec![Operand::Def(v(to)), Operand::Use(v(from))]);
    inst.kind = Kind::Copy;
    inst
}

fn jump_to(to: u32) -> Inst {
    let mut inst = op("jump", vec![Operand::Block(to)]);
    inst.terminator = true;
    inst.barrier = true;
    inst
}

/// A counting loop as the 6502 selector leaves it (phis already copies):
/// `b1: v2 = v1; inc v2; v3 = v2; test v1; exit b3; jump b2`, the latch
/// `b2: v1 = v3; jump b1`. `exit_reads` makes `b3` read `v1` too.
fn counted_loop() -> MFunction<()> {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 4];
    function.blocks.push(Block { id: 0, insts: vec![op("def", vec![Operand::Def(v(1))])], phis: vec![] });
    function.blocks.push(
        Block {
            id: 1,
            insts: vec![
                copy(2, 1),
                op("inc", vec![Operand::Def(v(2)), Operand::Use(v(2))]),
                copy(3, 2),
                op("test", vec![Operand::Use(v(1))]),
                branch(3),
                jump_to(2),
            ],
            phis: vec![],
        },
    );
    function.blocks.push(Block { id: 2, insts: vec![copy(1, 3), jump_to(1)], phis: vec![] });
    function.blocks.push(Block { id: 3, insts: vec![op("ret", vec![Operand::Use(v(1))])], phis: vec![] });
    function
}

/// 6502 `find` at -Os: `i + 1` was done before the exit test while `i` was
/// still read by it, so the step ran in Y and the back edge copied it to X
/// through memory (19 B against 13). The step is now in the latch.
#[test]
fn a_step_done_before_the_exit_test_moves_into_the_latch() {
    let mut function = counted_loop();
    assert!(machine_sink_in_place::<Two>(&mut function));
    let body = function.blocks.iter().find(|block| block.id == 1).unwrap();
    assert!(!names(body).contains(&"inc"), "{:?}", names(body));
    let latch = function.blocks.iter().find(|block| block.id == 2).unwrap();
    assert!(names(latch).contains(&"inc"), "{:?}", names(latch));
}

/// A second write of the step's register after it in the block would be
/// overwritten by the moved step: it stays.
#[test]
fn a_step_followed_by_another_write_of_its_register_stays() {
    let mut function = counted_loop();
    function.blocks[1].insts.insert(3, copy(2, 1));
    machine_sink_in_place::<Two>(&mut function);
    let body = function.blocks.iter().find(|block| block.id == 1).unwrap();
    assert!(names(body).contains(&"inc"), "{:?}", names(body));
}

/// A copy moved below a write of its source reads the new value: the count
/// is then one more than it was (found by 5 of 23 changed corpus programs
/// answering wrongly at -Os, 2026-11-13).
#[test]
fn a_copy_stays_above_a_later_write_of_its_source() {
    let mut function = counted_loop();
    function.blocks[1].insts.insert(3, op("inc", vec![Operand::Def(v(2)), Operand::Use(v(2))]));
    machine_sink_in_place::<Two>(&mut function);
    let body = function.blocks.iter().find(|block| block.id == 1).unwrap();
    assert!(body.insts.iter().any(|i| i.kind == Kind::Copy && i.operands.get(1) == Some(&Operand::Use(v(2)))), "{:?}", names(body));
}

/// `b0` sets `v1` to 8 and may leave for the join `b2`, or go through `b1`,
/// which sets `v1` again (a select made as blocks). Moved into the join, the
/// 8 overwrote `b1`'s value: 5 of 23 changed corpus programs answered wrong
/// at -Os (word_compares among them, 2026-11-13).
#[test]
fn a_value_one_arm_overwrites_stays_out_of_the_join() {
    let mut function = MFunction::new("f");
    function.classes = vec![(); 2];
    let mut eight = op("ldi", vec![Operand::Def(v(1))]);
    eight.rematerializable = true;
    function.blocks.push(
        Block {
            id: 0,
            insts: vec![op("def", vec![Operand::Def(v(0))]), eight, op("test", vec![Operand::Use(v(0))]), branch(2), jump_to(1)],
            phis: vec![],
        },
    );
    function.blocks.push(
        Block {
            id: 1,
            insts: vec![op("ldi", vec![Operand::Def(v(1))]), op("test", vec![Operand::Use(v(0))]), branch(3), jump_to(2)],
            phis: vec![],
        },
    );
    function.blocks.push(Block { id: 2, insts: vec![op("use", vec![Operand::Use(v(1))]), op("ret", vec![])], phis: vec![] });
    function.blocks.push(Block { id: 3, insts: vec![op("ret", vec![])], phis: vec![] });
    machine_sink_in_place::<Two>(&mut function);
    let head = function.blocks.iter().find(|block| block.id == 0).unwrap();
    assert!(names(head).contains(&"ldi"), "{:?}", names(head));
}
