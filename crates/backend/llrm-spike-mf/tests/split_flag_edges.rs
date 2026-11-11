//! 1000 compare-and-branch edges into phis (3000 blocks) took 20 s in
//! `split_flag_edges`: `successors` found each branch target by scanning all
//! blocks, and the pass counted every block's predecessors the same way, once
//! per split. They take 0.15 s.

mod common;

use common::{Two, v};
use llrm_spike_mf::regalloc::split_flag_edges;
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, Phi};

fn op(name: &'static str) -> Inst {
    Inst::new(Kind::Op(name), vec![])
}

/// `cmp; branch J` falling into `B`, which falls into the join `J` whose phi
/// reads the compare's block: the copy for that input cannot go after the
/// compare, so the edge is split.
fn diamonds(count: u32) -> MFunction<()> {
    let mut function = MFunction::new("f");
    for group in 0..count {
        let (a, b, join) = (3 * group, 3 * group + 1, 3 * group + 2);
        let mut compare = op("cmp");
        compare.sets_flags = true;
        let mut jump = common::branch(join);
        jump.reads_flags = true;
        function.blocks.push(Block { id: a, insts: vec![compare, jump], phis: vec![] });
        function.blocks.push(Block { id: b, insts: vec![op("work")], phis: vec![] });
        let (result, from_a, from_b) = (3 * group, 3 * group + 1, 3 * group + 2);
        function.blocks.push(Block {
            id: join,
            insts: vec![op("use")],
            phis: vec![Phi { result: v(result), incoming: vec![(a as usize, v(from_a)), (b as usize, v(from_b))] }],
        });
    }
    function.blocks.push(Block { id: 3 * count, insts: vec![op("ret")], phis: vec![] });
    function.classes = vec![(); 3 * count as usize + 3];
    function
}

/// The calling thread's CPU time: a loaded machine, a preempted thread or a
/// sleep does not stretch it.
fn thread_cpu() -> std::time::Duration {
    let mut found = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `found` is a valid timespec for the call to fill.
    let done = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut found) };
    assert_eq!(done, 0, "no thread CPU clock");
    std::time::Duration::new(found.tv_sec as u64, found.tv_nsec as u32)
}

/// CPU time of `run` on `count` diamonds, the construction left out; the edges
/// split are checked, or the measure would time nothing.
fn pass_cpu(
    count: u32,
    run: impl Fn(&mut MFunction<()>),
) -> std::time::Duration {
    let mut function = diamonds(count);
    let before = function.blocks.len();
    let start = thread_cpu();
    run(&mut function);
    let took = thread_cpu() - start;
    assert!(function.blocks.len() > before, "no edge was split: the test measures nothing");
    took
}

/// How much more CPU time `work` takes on 2000 diamonds than on 500: the
/// least of three runs each, on one thread's CPU clock, so the machine's load
/// and speed do not enter the ratio. 4x the input is 4x for linear work, 16x
/// for quadratic and 64x for cubic.
fn growth(work: impl Fn(u32) -> std::time::Duration) -> f64 {
    let least = |count| (0..3).map(|_| work(count)).min().expect("three runs");
    least(2000).as_secs_f64() / least(500).as_secs_f64().max(1e-4)
}

/// The pass splits one edge per round and every round is linear, so the whole
/// is quadratic: 16x. The scans this test was written for made it cubic: 64x.
/// The bound is between, 32x.
const CUBIC_FLOOR: f64 = 32.0;

/// Measured against a 1 s wall-clock bound, which failed at 1.1 s whenever the
/// machine ran 40 jobs.
#[test]
fn many_flag_edges_are_split_in_no_more_than_quadratic_work() {
    let ratio = growth(|count| pass_cpu(count, |function| split_flag_edges::<Two, _>(function)));
    assert!(ratio < CUBIC_FLOOR, "4x the edges took {ratio:.1}x the CPU time");
}

/// The measure ignores time a thread spends waiting: 50 ms of sleep is wall
/// time and no CPU time.
#[test]
fn a_wait_in_the_pass_is_not_charged() {
    let ratio = growth(|count| {
        pass_cpu(count, |function| {
            split_flag_edges::<Two, _>(function);
            std::thread::sleep(std::time::Duration::from_millis(50));
        })
    });
    assert!(ratio < CUBIC_FLOOR, "4x the edges took {ratio:.1}x the CPU time");
}

/// The measure sees work that grows with the cube.
#[test]
fn cubic_work_is_over_the_floor() {
    let ratio = growth(|count| {
        let start = thread_cpu();
        let mut sum = 0u64;
        for step in 0..(count as u64).pow(3) / 200 {
            sum = sum.wrapping_add(std::hint::black_box(step));
        }
        std::hint::black_box(sum);
        thread_cpu() - start
    });
    assert!(ratio > CUBIC_FLOOR, "4x the input took {ratio:.1}x the CPU time");
}
