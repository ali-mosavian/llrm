//! `reschedule_kills`: the last read of a tied instruction's source moves above it.

mod common;

use common::{Two, op, v};
use llrm_spike_mf::{Block, Inst, Kind, MFunction, Operand, reschedule_kills};

fn tied_step(
    result: u32,
    source: u32,
) -> Inst {
    let mut step = op("step", vec![Operand::Def(v(result)), Operand::Use(v(source))]);
    step.ties = vec![(0, 1)];
    step
}

fn load(
    result: u32,
    from: u32,
) -> Inst {
    let mut load = op("load", vec![Operand::Def(v(result)), Operand::Use(v(from))]);
    load.may_load = true;
    load
}

fn store(
    value: u32,
    to: u32,
) -> Inst {
    let mut store = op("store", vec![Operand::Use(v(value)), Operand::Use(v(to))]);
    store.may_store = true;
    store
}

fn function(insts: Vec<Inst>) -> MFunction<()> {
    let mut f = MFunction::new("f");
    f.classes = vec![(); 16];
    f.blocks.push(Block { id: 0, insts, phis: vec![] });
    f
}

fn names(f: &MFunction<()>) -> Vec<&'static str> {
    f.blocks[0]
        .insts
        .iter()
        .map(|inst| match inst.kind {
            Kind::Op(name) => name,
            Kind::Copy => "copy",
        })
        .collect()
}

/// `q = p + 1; x = *p` read `p` after the step, so the step kept a copy of `p`; the load goes first.
#[test]
fn a_load_of_the_source_moves_above_the_step() {
    let mut f = function(vec![op("def", vec![Operand::Def(v(0))]), tied_step(1, 0), load(2, 0), op("use", vec![Operand::Use(v(1)), Operand::Use(v(2))])]);
    reschedule_kills::<Two>(&mut f);
    assert_eq!(names(&f), ["def", "load", "step", "use"]);
}

/// The same with a store as the last read, and a pure instruction in between.
#[test]
fn a_store_through_the_source_moves_above_the_step_and_what_is_between() {
    let mut f = function(vec![
        op("def", vec![Operand::Def(v(0))]),
        op("def", vec![Operand::Def(v(3))]),
        tied_step(1, 0),
        op("pure", vec![Operand::Def(v(4))]),
        store(3, 0),
        op("use", vec![Operand::Use(v(1)), Operand::Use(v(4))]),
    ]);
    reschedule_kills::<Two>(&mut f);
    assert_eq!(names(&f), ["def", "def", "store", "step", "pure", "use"]);
}

/// A load does not move above a store (memory order), whatever the registers.
#[test]
fn a_load_does_not_move_above_a_store() {
    let mut f = function(vec![
        op("def", vec![Operand::Def(v(0))]),
        op("def", vec![Operand::Def(v(3))]),
        tied_step(1, 0),
        store(3, 3),
        load(2, 0),
        op("use", vec![Operand::Use(v(1)), Operand::Use(v(2))]),
    ]);
    reschedule_kills::<Two>(&mut f);
    assert_eq!(names(&f), ["def", "def", "step", "store", "load", "use"]);
}

/// A reader whose own input is made after the step cannot go above it.
#[test]
fn a_reader_of_a_value_the_step_makes_stays() {
    let mut f = function(vec![
        op("def", vec![Operand::Def(v(0))]),
        tied_step(1, 0),
        op("both", vec![Operand::Def(v(2)), Operand::Use(v(0)), Operand::Use(v(1))]),
        op("use", vec![Operand::Use(v(2))]),
    ]);
    reschedule_kills::<Two>(&mut f);
    assert_eq!(names(&f), ["def", "step", "both", "use"]);
}

/// Two later reads, or a source that lives on, leave the order alone.
#[test]
fn a_source_that_lives_on_or_is_read_twice_stays() {
    let mut twice = function(vec![
        op("def", vec![Operand::Def(v(0))]),
        tied_step(1, 0),
        load(2, 0),
        load(3, 0),
        op("use", vec![Operand::Use(v(1)), Operand::Use(v(2)), Operand::Use(v(3))]),
    ]);
    reschedule_kills::<Two>(&mut twice);
    assert_eq!(names(&twice), ["def", "step", "load", "load", "use"]);
    let mut lives = function(vec![op("def", vec![Operand::Def(v(0))]), tied_step(1, 0), load(2, 0)]);
    lives.blocks.push(Block { id: 1, insts: vec![op("use", vec![Operand::Use(v(0))])], phis: vec![] });
    reschedule_kills::<Two>(&mut lives);
    assert_eq!(names(&lives), ["def", "step", "load"]);
}
