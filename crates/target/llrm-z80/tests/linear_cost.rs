//! A function's compile time grows with its size, not with its square
//! (`select_of_constants` -O2: 412 call sites in one `main` took over a minute;
//! the pin's llc takes a second). Each test builds one block of thousands of
//! instructions and gives the pass a budget of seconds that the quadratic form
//! overran by an order of magnitude.

use std::time::{Duration, Instant};

use llrm_spike_mf::mfunc::{Block, Inst, Kind, MFunction, Operand, Reg};
use llrm_spike_mf::regalloc::Assignment;
use llrm_z80::compile::{nest_reloads, stack_slot_report_by};
use llrm_z80::reg;
use llrm_z80::regalloc::Class;

fn within(
    budget: Duration,
    what: &str,
    work: impl FnOnce(),
) {
    let start = Instant::now();
    work();
    let took = start.elapsed();
    assert!(took < budget, "{what} took {took:?}, over {budget:?}");
}

/// `sink_closed_groups` -O2 on `select_of_constants`'s `main` (6132 instructions in one block): 95% of a
/// compile spent in the fixpoint that drops the members the group cannot keep, one round for each member of a
/// dependent chain and a scan of the block for each check. A chain of 1500 whose last link is also read by the
/// branch's other successor drops one member a round: 1500 rounds, 10 s.
#[test]
fn a_long_dependent_chain_leaves_the_sink_in_linear_time() {
    let n = 1500;
    let mut insts = vec![Inst::new(Kind::Op("LDI"), vec![Operand::Def(Reg::Virt(0)), Operand::Imm(0)])];
    for v in 1..n {
        insts.push(Inst::new(Kind::Op("INCV"), vec![Operand::Def(Reg::Virt(v)), Operand::Use(Reg::Virt(v - 1))]));
    }
    let mut branch = Inst::new(Kind::Op("JPNZ"), vec![Operand::Block(2)]);
    branch.terminator = true;
    let reader = |id| {
        let mut ret = Inst::new(Kind::Op("RET"), vec![Operand::Use(Reg::Virt(n - 1))]);
        ret.terminator = true;
        ret.barrier = true;
        Block { id, insts: vec![ret], phis: Vec::new() }
    };
    insts.push(branch);
    let mut f: MFunction<Class> = MFunction::new("f");
    f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }, reader(1), reader(2)];
    within(Duration::from_secs(2), "sink_closed_groups on a chain of 1500", || {
        llrm_z80::sink_group::sink_closed_groups(&mut f);
    });
}

/// The same function with 400 call sites that save two words and read them back in the order they were saved:
/// each run's reorder was tried on its own with a report of the whole function, 400 of them, and none alone makes
/// the slot a push (the other 399 runs still cross). 3 s in release, over 10 s in debug; one report each way
/// nests all of them.
#[test]
fn many_crossed_reload_runs_are_nested_with_one_report() {
    let save = |slot, from| Inst::new(Kind::Op("SPILL16"), vec![Operand::Frame(slot), Operand::Use(Reg::Phys(from))]);
    let read = |slot, into| Inst::new(Kind::Op("RELOAD16"), vec![Operand::Def(Reg::Phys(into)), Operand::Frame(slot)]);
    let mut insts = Vec::new();
    for _ in 0..400 {
        insts.extend([save(0, reg::BC), save(1, reg::DE), read(0, reg::BC), read(1, reg::DE)]);
    }
    let mut f: MFunction<Class> = MFunction::new("f");
    f.blocks = vec![Block { id: 0, insts, phis: Vec::new() }];
    let assignment = Assignment(Vec::new());
    within(Duration::from_secs(3), "nest_reloads on 400 crossed runs", || {
        assert!(nest_reloads(&mut f, &assignment));
    });
    assert_eq!(stack_slot_report_by(&mut f, &assignment, false, false).pushed, [0, 1]);
}
