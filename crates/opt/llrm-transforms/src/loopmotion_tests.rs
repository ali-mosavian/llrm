//! Adapted from llrm-core's `optimize/loopmotion_tests.rs`, the port of
//! `tests/test_loopmotion.py`, each body now MIR text run by llrm-mir's
//! interpreter before and after.
//!
//! HOTLOP's counter is a global the header stores. Its observers are a
//! load, a store, a call and a volatile access; `escape` and `opaque` have
//! no rich MIR kind. Skipped, BC and OMF fixtures through the old pipeline:
//! `test_nested_accumulator_is_stored_only_after_the_outer_loop`,
//! `test_indexed_record_accumulators_store_only_after_loop` and
//! `test_a_float_loop_sinks_its_counter_store_without_an_error_handler`.

use std::rc::Rc;

use llrm_analysis::testing::{DOS, layout};
use llrm_mir::passes::{Analyses, Outer};

use super::sunk_stores;
use crate::testing::{parsed, printed, results};

/// `text`'s @f with its stores sunk: its printed form, and whether any
/// moved. @f computes what it did on `inputs`.
fn sunk(
    text: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let (layout, outer) = (layout(&module), Rc::new(Outer::of(&module, None)));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = sunk_stores(context, &layout, &callees, function, &mut Analyses::new(outer)).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    (after, changed)
}

fn kept(
    text: &str,
    inputs: &[&[i128]],
) {
    let (after, changed) = sunk(text, inputs);
    assert!(!changed, "{after}");
    assert_eq!(after, printed(&parsed(&format!("{DOS}{text}"))));
}

const TRIPS: &[&[i128]] = &[&[0], &[1], &[50], &[-3]];

/// HOTLOP: the header stores its counter, which the loop never reads back.
/// `observer` goes in the latch.
fn hotlop(observer: &str) -> String {
    format!(
        "@n = global i16 0
@m = global i16 0

define void @touch() {{
b:
  store i16 1, ptr @m
  ret void
}}

define i16 @f(i16 %k) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  store i16 %i, ptr @n
  %c = icmp slt i16 %i, %k
  br i1 %c, label %b2, label %b3

b2:
{observer}  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @n
  ret i16 %v
}}
"
    )
}

#[test]
fn test_counter_is_written_once_at_the_exit_not_every_iteration() {
    let (text, changed) = sunk(&hotlop(""), TRIPS);
    assert!(changed);
    assert!(text.contains("b1:\n  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]\n  %c = icmp"), "{text}");
    assert!(text.contains("b3:\n  store i16 %i, ptr @n\n  %v = load"), "{text}");
}

#[test]
fn test_an_observer_in_the_loop_keeps_the_store() {
    for observer in [
        "  %o = load i16, ptr @n\n",
        "  store i16 3, ptr @n\n",
        "  call void @touch()\n",
        "  %o = load volatile i16, ptr @m\n",
    ] {
        kept(&hotlop(observer), TRIPS);
    }
}

#[test]
fn test_an_exit_reachable_without_the_store_gets_no_new_write() {
    let text =
        hotlop("").replace("b0:\n  br label %b1", "b0:\n  %e = icmp eq i16 %k, 7\n  br i1 %e, label %b3, label %b1");
    kept(&text, &[&[0], &[1], &[7], &[50]]);
}

/// An accumulator the latch stores, and the entry seeds with `seeded`.
fn accumulator(seeded: &str) -> String {
    format!(
        "@acc = global i16 0

define i16 @f(i16 %k) {{
b0:
{seeded}  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %s = phi i16 [ 5, %b0 ], [ %s1, %b2 ]
  %c = icmp slt i16 %i, %k
  br i1 %c, label %b2, label %b3

b2:
  %s1 = add i16 %s, %i
  store i16 %s1, ptr @acc
  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @acc
  ret i16 %v
}}
"
    )
}

/// The cell holds the header's phi on every trip, so after the loop the
/// phi is what it holds.
#[test]
fn test_an_accumulator_stores_its_phi_once_after_the_loop() {
    let (text, changed) = sunk(&accumulator("  store i16 5, ptr @acc\n"), TRIPS);
    assert!(changed);
    assert!(
        text.contains("b2:\n  %s1 = add i16 %s, %i\n  %next") && text.contains("b3:\n  store i16 %s, ptr @acc\n"),
        "{text}"
    );
}

/// SUMTHREE: a call between the seed and the loop that writes only
/// another cell leaves the seed standing, as `Accesses` says. Any writing
/// call kept the accumulator stored every trip.
#[test]
fn a_call_writing_another_cell_leaves_the_seed() {
    let seeded = accumulator("  store i16 5, ptr @acc\n  call void @w(ptr @m)\n");
    let text = format!("@m = global i16 0\n\ndeclare void @w(ptr) memory(argmem: write)\n\n{seeded}");
    let (text, changed) = sunk(
        &text
            .replace("declare", "define")
            .replace("memory(argmem: write)", "memory(argmem: write) {\nb:\n  store i16 1, ptr %0\n  ret void\n}"),
        TRIPS,
    );
    assert!(changed && text.contains("b3:\n  store i16 %s, ptr @acc\n"), "{text}");
}

#[test]
fn test_an_accumulator_without_zero_trip_initialization_stays_in_the_loop() {
    kept(&accumulator(""), TRIPS);
}

/// The latch stores `value` to @g on each of `bound` trips.
fn latch_store(
    value: &str,
    bound: &str,
) -> String {
    format!(
        "@g = global i16 0

define i16 @f(i16 %x) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = icmp slt i16 %i, {bound}
  br i1 %c, label %b2, label %b3

b2:
  store i16 {value}, ptr @g
  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @g
  ret i16 %v
}}
"
    )
}

const VALUES: &[&[i128]] = &[&[0], &[-9], &[1234]];

/// LNGMXX: an invariant the latch stores sinks only where the loop runs.
#[test]
fn test_an_invariant_store_sinks_only_when_the_loop_runs() {
    for bound in ["1", "10"] {
        let (text, changed) = sunk(&latch_store("%x", bound), VALUES);
        assert!(changed && text.contains("b3:\n  store i16 %x, ptr @g\n"), "{text}");
    }
    kept(&latch_store("%x", "0"), VALUES);
    kept(&latch_store("%x", "%x"), VALUES);
}

/// The latch's last store of its counter is the counter's last value.
#[test]
fn test_a_stored_counter_leaves_its_last_value() {
    for (bound, last) in [("1", 0), ("10", 9)] {
        let (text, changed) = sunk(&latch_store("%i", bound), VALUES);
        assert!(changed && text.contains(&format!("b3:\n  store i16 {last}, ptr @g\n")), "{text}");
    }
    kept(&latch_store("%i", "0"), VALUES);
}

/// The header stores @a[0] while the latch loads @a[`index`].
fn indexed(index: &str) -> String {
    format!(
        "@a = global [4 x i16] zeroinitializer

define i16 @f(i16 %k, i16 %j) {{
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  store i16 %i, ptr @a
  %c = icmp slt i16 %i, %k
  br i1 %c, label %b2, label %b3

b2:
  %p = getelementptr [4 x i16], ptr @a, i16 0, i16 {index}
  %o = load i16, ptr %p
  %next = add i16 %i, 1
  br label %b1

b3:
  %v = load i16, ptr @a
  ret i16 %v
}}
"
    )
}

const INDEXED: &[&[i128]] = &[&[0, 0], &[1, 0], &[20, 1], &[20, 3]];

#[test]
fn test_a_store_that_may_alias_a_load_stays() {
    kept(&indexed("%j"), INDEXED);
}

#[test]
fn test_a_store_apart_from_every_load_sinks() {
    let (text, changed) = sunk(&indexed("1"), INDEXED);
    assert!(changed && text.contains("b3:\n  store i16 %i, ptr @a\n"), "{text}");
}

/// An address only the latch computes does not reach the exit: moved
/// there, the store read a value its block does not dominate.
#[test]
fn test_an_address_the_latch_computes_keeps_its_store() {
    let text = latch_store("%x", "10")
        .replace("@g = global i16 0", "@a = global [4 x i16] zeroinitializer")
        .replace(
            "  store i16 %x, ptr @g\n",
            "  %p = getelementptr [4 x i16], ptr @a, i16 0, i16 2\n  store i16 %x, ptr %p\n",
        )
        .replace("load i16, ptr @g", "load i16, ptr getelementptr ([4 x i16], ptr @a, i16 0, i16 2)");
    kept(&text, VALUES);
}

/// A call before the loop that writes elsewhere leaves the seed standing.
/// Any writing call refused, and sum_three stored its total every trip
/// past a bound's runtime call.
#[test]
fn test_a_call_writing_elsewhere_keeps_the_seed() {
    let text = accumulator("  store i16 5, ptr @acc\n  call void @touch()\n").replace(
        "define i16 @f",
        "@m = global i16 0\n\ndefine void @touch() {\nb:\n  store i16 1, ptr @m\n  ret void\n}\n\ndefine i16 @f",
    );
    let module = parsed(&format!("{DOS}{text}"));
    let after = crate::testing::summarized(&module, super::LoopMotion, true, TRIPS);
    assert!(after.contains("b3:\n  store i16 %s, ptr @acc\n"), "{after}");
}

/// savegame.c: an accumulator loop behind a long run of if/else, loops in
/// its arms. Whether the cell holds the phi's entry value was asked once
/// per path to the entry, 2^40 here, and llrm-c ran past 300 s.
#[test]
fn test_the_entry_value_is_asked_once_per_block_not_per_path() {
    let diamonds: String = (0..40)
        .map(|at| {
            let join = if at == 0 { "b0".to_owned() } else { format!("j{}", at - 1) };
            format!("{join}:\n  br i1 %p, label %t{at}, label %e{at}\n\nt{at}:\n  br i1 %p, label %t{at}, label %j{at}\n\ne{at}:\n  br label %j{at}\n\n")
        })
        .collect();
    let text = accumulator("  %p = icmp eq i16 %k, 3\n")
        .replace("b0:\n  %p = icmp eq i16 %k, 3\n  br label %b1\n", &format!("{diamonds}j39:\n  br label %b1\n"))
        .replace("[ 0, %b0 ]", "[ 0, %j39 ]")
        .replace("[ 5, %b0 ]", "[ 5, %j39 ]");
    let text = text.replace("b0:\n  br i1 %p", "b0:\n  store i16 5, ptr @acc\n  %p = icmp eq i16 %k, 3\n  br i1 %p");
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || done.send(sunk(&text, TRIPS)).unwrap());
    assert!(finished.recv_timeout(std::time::Duration::from_secs(20)).is_ok(), "loopmotion ran past 20 s");
}

/// Each loop sunk made the next ask of Annotated prove every loop's trips
/// again, and work the edges' facts of every block again: 20 loops proved 420
/// times, 1,220 blocks and 400 loops worked: 59, 118 and 39 now. A change
/// reaches the loops and blocks that read what it touched, so the others keep
/// what they had.
#[test]
fn test_sinking_a_loops_store_proves_only_that_loop_again() {
    // The check derives them afresh to compare, and is counted.
    if std::env::var_os("LLRM_CHECK_REPLAY").is_some() {
        return;
    }
    let loops = 20;
    let mut text = String::from("define i16 @f(i16 %k) {\nb0:\n  br label %h0\n\n");
    for at in 0..loops {
        let from = if at == 0 { "b0".to_owned() } else { format!("p{at}") };
        let exit = if at + 1 == loops { "end".to_owned() } else { format!("p{}", at + 1) };
        text += &format!("@n{at} = global i16 0\n");
        text += &format!(
            "h{at}:\n  %i{at} = phi i16 [ 0, %{from} ], [ %x{at}, %l{at} ]\n  store i16 %i{at}, ptr @n{at}\n  %c{at} = icmp slt i16 %i{at}, 9\n  br i1 %c{at}, label %l{at}, label %{exit}\n\nl{at}:\n  %x{at} = add i16 %i{at}, 1\n  br label %h{at}\n\n"
        );
        if at + 1 < loops {
            text += &format!("p{}:\n  br label %h{}\n\n", at + 1, at + 1);
        }
    }
    text += "end:\n  ret i16 %k\n}\n";
    let globals: String = (0..loops).map(|at| format!("@n{at} = global i16 0\n")).collect();
    let text = text.lines().filter(|line| !line.starts_with('@')).collect::<Vec<_>>().join("\n");
    let before = (
        llrm_analysis::induction::proved(),
        llrm_analysis::ranges::blocks_solved(),
        llrm_analysis::ranges::loops_solved(),
    );
    let (after, changed) = sunk(&format!("{globals}{text}\n"), TRIPS);
    assert!(changed, "{after}");
    let proved = llrm_analysis::induction::proved() - before.0;
    let solved = llrm_analysis::ranges::blocks_solved() - before.1;
    let worked = llrm_analysis::ranges::loops_solved() - before.2;
    assert!(proved <= 4 * loops, "{proved} loops proved for {loops} loops sunk one after another");
    assert!(worked <= 4 * loops, "{worked} loops worked for the bounds of {loops} loops sunk one after another");
    assert!(
        solved <= 200,
        "{solved} blocks worked for the edges of a body of {} blocks, {loops} sunk one after another",
        2 * loops + loops
    );
}

/// A store moved to the loop's exit threw away every analysis held, so each of
/// a nest's loops with a store to move worked its ranges out again (nest 32
/// deep: 55 `bounded` runs, 9.5 G of a 35 G compile). No value, block or edge
/// moves, so what is held of them stays.
#[test]
fn test_moving_a_store_keeps_the_ranges_held() {
    use llrm_analysis::manager::Bounded;
    let mut module = parsed(&format!("{DOS}{}", hotlop("")));
    let (layout, outer) = (layout(&module), Rc::new(Outer::of(&module, None)));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    analyses.get::<Bounded>(context, &layout, function);
    assert!(sunk_stores(context, &layout, &callees, function, &mut analyses).unwrap(), "premise: a store moved");
    assert!(analyses.cached::<Bounded>().is_some(), "the ranges were thrown away by a store moving");
}

/// Each loop asked what its cells hold before the instructions of the body, a
/// fresh pass over all of it: 40 loops, none moved, 40 derivations
/// (branches(512) at -O2: 626 ms of loopmotion's 1.6 s). Nothing changed
/// between the loops, so the manager's one serves them all.
#[test]
fn what_the_cells_hold_is_derived_once_for_loops_that_move_nothing() {
    let loops = 40;
    let globals: String = (0..loops).map(|at| format!("@a{at} = global i16 0\n")).collect();
    let mut text =
        String::from("@t = global i16 0\n\ndefine i16 @f(i16 %k) {\nb0:\n  store i16 5, ptr @t\n  br label %s0\n\n");
    for at in 0..loops {
        let exit = if at + 1 == loops { "end".to_owned() } else { format!("s{}", at + 1) };
        // Seeded 5 through a load, 6 in the phi: asked what the cell holds, not
        // moved.
        text += &format!(
            "s{at}:\n  %x{at} = load i16, ptr @t\n  store i16 %x{at}, ptr @a{at}\n  br label %h{at}\n\nh{at}:\n  %i{at} = phi i16 [ 0, %s{at} ], [ %n{at}, %l{at} ]\n  %v{at} = phi i16 [ 6, %s{at} ], [ %t{at}, %l{at} ]\n  %c{at} = icmp slt i16 %i{at}, %k\n  br i1 %c{at}, label %l{at}, label %{exit}\n\nl{at}:\n  %t{at} = add i16 %v{at}, %i{at}\n  store i16 %t{at}, ptr @a{at}\n  %n{at} = add i16 %i{at}, 1\n  br label %h{at}\n\n"
        );
    }
    text += "end:\n  ret i16 %k\n}\n";
    let before = llrm_analysis::consts::cell_derivations();
    let (after, changed) = sunk(&format!("{globals}{text}"), TRIPS);
    let derived = llrm_analysis::consts::cell_derivations() - before;
    assert!(!changed, "{after}");
    assert!(derived <= 1, "{derived} derivations for {loops} loops that move nothing");
}

/// A store sunk made the next loop derive what the body's cells hold from the
/// start: 20 loops that sink a store, each followed by one that asks and keeps
/// its own, 20 derivations over the body (branches(512) at -O2: 5.5 G of a
/// loopmotion of 10.9 G). Stores moved change the cells of the loop they left
/// and what it reaches, so only those are worked again: one derivation.
#[test]
fn what_the_cells_hold_is_derived_once_for_loops_that_each_sink_a_store() {
    // The check derives them afresh to compare, and is counted.
    if std::env::var_os("LLRM_CHECK_REPLAY").is_some() {
        return;
    }
    let pairs = 20;
    let mut globals = String::from("@t = global i16 0\n");
    let mut text = String::from("define i16 @f(i16 %k) {\nb0:\n  store i16 5, ptr @t\n  br label %m0\n\n");
    for at in 0..pairs {
        globals += &format!("@m{at} = global i16 0\n@a{at} = global i16 0\n");
        let next = if at + 1 == pairs { "end".to_owned() } else { format!("m{}", at + 1) };
        // Sinks: the cell holds the phi on every trip, seeded by a constant
        // store.
        text += &format!(
            "m{at}:\n  store i16 5, ptr @m{at}\n  br label %g{at}\n\ng{at}:\n  %i{at} = phi i16 [ 0, %m{at} ], [ %n{at}, %l{at} ]\n  %s{at} = phi i16 [ 5, %m{at} ], [ %u{at}, %l{at} ]\n  %c{at} = icmp slt i16 %i{at}, %k\n  br i1 %c{at}, label %l{at}, label %s{at}x\n\nl{at}:\n  %u{at} = add i16 %s{at}, %i{at}\n  store i16 %u{at}, ptr @m{at}\n  %n{at} = add i16 %i{at}, 1\n  br label %g{at}\n\n"
        );
        // Asks and keeps: seeded through a load, the phi seeded otherwise.
        text += &format!(
            "s{at}x:\n  %x{at} = load i16, ptr @t\n  store i16 %x{at}, ptr @a{at}\n  br label %h{at}\n\nh{at}:\n  %j{at} = phi i16 [ 0, %s{at}x ], [ %o{at}, %q{at} ]\n  %v{at} = phi i16 [ 6, %s{at}x ], [ %w{at}, %q{at} ]\n  %d{at} = icmp slt i16 %j{at}, %k\n  br i1 %d{at}, label %q{at}, label %{next}\n\nq{at}:\n  %w{at} = add i16 %v{at}, %j{at}\n  store i16 %w{at}, ptr @a{at}\n  %o{at} = add i16 %j{at}, 1\n  br label %h{at}\n\n"
        );
    }
    text += "end:\n  ret i16 %k\n}\n";
    let before = llrm_analysis::consts::cell_derivations();
    let (after, changed) = sunk(&format!("{globals}{text}"), TRIPS);
    let derived = llrm_analysis::consts::cell_derivations() - before;
    assert!(changed, "{after}");
    assert!(derived <= 1, "{derived} derivations for {pairs} loops that each sink a store");
}

/// A store moved threw away which frames are exposed and every reference
/// annotated, and the next ask worked them out again to the same answer (82
/// `exposed-frames` runs and 74 `annotated`, all the same, in branches(512) at
/// -O2). A store moved is the same store.
#[test]
fn moving_a_store_keeps_the_exposed_frames_and_the_references_held() {
    use llrm_analysis::manager::{Annotated, ExposedFrames};
    let mut module = parsed(&format!("{DOS}{}", hotlop("")));
    let (layout, outer) = (layout(&module), Rc::new(Outer::of(&module, None)));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    analyses.get::<Annotated>(context, &layout, function);
    assert!(sunk_stores(context, &layout, &callees, function, &mut analyses).unwrap(), "premise: a store moved");
    assert!(analyses.cached::<ExposedFrames>().is_some(), "exposed frames thrown away by a store moving");
    assert!(analyses.cached::<Annotated>().is_some(), "the references thrown away by a store moving");
}

/// A store moved made the next ask build the accesses of the whole body again,
/// to what it had (95 builds in branches(512) at -O2, each one over every
/// instruction). What an instruction touches is its own, wherever it sits.
#[test]
fn moving_a_store_keeps_the_accesses_it_had() {
    use llrm_analysis::memoryssa::Accesses;
    let mut module = parsed(&format!("{DOS}{}", hotlop("")));
    let (layout, outer) = (layout(&module), Rc::new(Outer::of(&module, None)));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    let before = Accesses::managed(context, &layout, function, &mut analyses).unwrap();
    assert!(sunk_stores(context, &layout, &callees, function, &mut analyses).unwrap(), "premise: a store moved");
    let after = Accesses::managed(context, &layout, function, &mut analyses).unwrap();
    assert!(Rc::ptr_eq(&before, &after), "the accesses were built again after a store moved");
}

/// `sunk_stores` found the body's dominance and loops itself (a set of every
/// block's dominators, quadratic in a chain of diamonds: 309 M of loopmotion's
/// 1.6 G in branches(512)) though the manager held them. It asks the manager.
#[test]
fn sinking_stores_asks_the_manager_for_the_shape_of_the_body() {
    use llrm_analysis::cfg::{Shape, shapes_derived};
    let mut module = parsed(&format!("{DOS}{}", hotlop("")));
    let (layout, outer) = (layout(&module), Rc::new(Outer::of(&module, None)));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    analyses.get::<Shape>(context, &layout, function);
    let before = shapes_derived();
    assert!(sunk_stores(context, &layout, &callees, function, &mut analyses).unwrap(), "premise: a store moved");
    assert_eq!(shapes_derived(), before, "the shape of the body was derived again");
}

/// Each loop's trips were proved again for the question whether it runs at
/// all, though the manager held every loop's proofs (169 M of loopmotion's
/// 321 M in branches(512)).
#[test]
fn sinking_stores_reads_the_trips_the_manager_proved() {
    use llrm_analysis::manager::Counted;
    let mut module = parsed(&format!("{DOS}{}", accumulator("  store i16 5, ptr @acc\n")));
    let (layout, outer) = (layout(&module), Rc::new(Outer::of(&module, None)));
    let callees = llrm_mir::memory::callees(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let mut analyses = Analyses::new(outer);
    analyses.get::<Counted>(context, &layout, function);
    let before = llrm_analysis::induction::proved();
    assert!(sunk_stores(context, &layout, &callees, function, &mut analyses).unwrap(), "premise: a store moved");
    assert_eq!(llrm_analysis::induction::proved(), before, "a loop's trips were proved again");
}
