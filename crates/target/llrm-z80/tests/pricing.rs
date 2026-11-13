//! What a variant costs: the bytes and T-states of the printed lines.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::{Line, price_emitted, price_in_loops, set_frequencies};

fn price(text: &str) -> (u32, u64) {
    price_in_loops(&[Line::Inst(text.to_owned())])
}

/// A form the table did not recognise cost 2 B and 8 T: `ex de, hl` (1 B), a
/// conditional `jp` (3 B), the frame forms (`ld iy,#n` is 4). At -Os, where the
/// price is bytes, write_through_parameter's `main` chose a variant 3 B bigger
/// than the one it passed over (231 -> 234).
#[test]
fn every_form_the_printer_writes_is_in_the_table() {
    for (text, bytes, clocks) in [
        ("ex de, hl", 1, 4),
        ("jp nz, L0_9", 3, 10),
        ("jp z, L0_2", 3, 10),
        ("jr z, L0_2", 2, 12),
        ("ret z", 1, 11),
        ("ld sp, ix", 2, 10),
        ("ld sp, iy", 2, 10),
        ("add ix, sp", 2, 15),
        ("add iy, sp", 2, 15),
        ("ld iy, #-14", 4, 14),
    ] {
        assert_eq!(price(text), (bytes, clocks), "{text}");
    }
}

fn counted_loop(counter: &str) -> Vec<Line> {
    ["L0_1:", "inc hl", &format!("dec {counter}"), "jp nz, L0_1"]
        .iter()
        .map(|text| match text.strip_suffix(':') {
            Some(label) => Line::Label(label.to_owned()),
            None => Line::Inst((*text).to_owned()),
        })
        .collect()
}

/// A counter in `b` ends as `djnz` (2 B, 13 T), one in `c` as `dec c; jr nz`
/// (3 B, 16 T): the two were priced alike, before the jumps are relaxed.
#[test]
fn a_counter_in_b_prices_below_one_in_c() {
    let (b, c) = (price_emitted(&counted_loop("b"), true), price_emitted(&counted_loop("c"), true));
    assert!(b.0 < c.0 && b.1 < c.1, "b {b:?} c {c:?}");
}

/// byval_struct -O1 took the allocation with the 41-trip counter in `c` (`dec
/// c; jr nz`, 339 B and 11925 T) over the one with `djnz` (340 B and 11842 T):
/// +83 T for -1 B, priced as a gain. Since #9031 (LSR meets IV users in program
/// order again) that loop also stores its own index, so `b` is taken and the
/// counter is `c` (279 B, 9706 T, from 294 B, 10399 T); the counter alone, the
/// shape the price decides, keeps `djnz`.
#[test]
fn a_counted_loop_keeps_its_djnz_at_o1() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
        define void @f(ptr %p) {\nentry:\n  br label %l\nl:\n  %i = phi i8 [ 41, %entry ], [ %n, %l ]\n  \
        %q = phi ptr [ %p, %entry ], [ %q1, %l ]\n  store volatile i8 %i, ptr %q\n  \
        %q1 = getelementptr i8, ptr %q, i16 1\n  %n = sub i8 %i, 1\n  %c = icmp eq i8 %n, 0\n  \
        br i1 %c, label %e, label %l\ne:\n  ret void\n}\n";
    let out = text(ir, "f", 0, &Request::new(Level::O1)).expect("compiles");
    assert!(out.contains("djnz"), "{out}");
}

fn lines_of(text: &[&str]) -> Vec<Line> {
    text.iter()
        .map(|text| match text.strip_suffix(':') {
            Some(label) => Line::Label(label.to_owned()),
            None => Line::Inst((*text).to_owned()),
        })
        .collect()
}

/// An instruction costs its clocks times its block's frequency over the
/// entry's: the loop body the pin runs 32 times is priced 32 times.
#[test]
fn a_block_is_priced_by_its_frequency() {
    set_frequencies(&[(0, 1.0), (5, 32.0)]);
    let priced = price_in_loops(&lines_of(&["L0_0:", "inc hl", "L0_5:", "inc hl"]));
    set_frequencies(&[]);
    assert_eq!(priced.1, 6 + 6 * 32, "{priced:?}");
}

/// A block made after selection has no frequency of its own and takes that of
/// the label before it. Without that its clocks counted once, a loop's select
/// split in two blocks looked free next to the one-block form.
#[test]
fn a_block_the_passes_made_takes_the_frequency_of_the_one_before_it() {
    set_frequencies(&[(5, 32.0)]);
    let priced = price_in_loops(&lines_of(&["L0_5:", "inc hl", "L0_90:", "inc hl"]));
    set_frequencies(&[]);
    assert_eq!(priced.1, 2 * 6 * 32, "{priced:?}");
}

/// The pin's frequency of a loop's block reaches the instructions of the blocks
/// the z80 passes cut from it: the halves of a `select` in a loop are priced at
/// the loop's 32, not at 1; the join at 32, the arm made for one side at half.
#[test]
fn the_frequency_of_a_loop_reaches_the_blocks_cut_from_it() {
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
define i16 @f(i16 %n, i16 %x) {\n\
entry:\n  br label %loop\n\
loop:\n  %i = phi i16 [ 0, %entry ], [ %in, %loop ]\n  %s = phi i16 [ 0, %entry ], [ %sn, %loop ]\n\
  %c = icmp ugt i16 %i, %x\n  %t = select i1 %c, i16 %i, i16 %x\n  %sn = add i16 %s, %t\n  %in = add i16 %i, 1\n\
  %d = icmp ult i16 %in, %n\n  br i1 %d, label %loop, label %exit\n\
exit:\n  ret i16 %sn\n}";
    let lines = llrm_z80::compile::weighted_lines(ir, "f", &Request::new(Level::O1)).expect("compiles");
    let made = |name: &str| name.rsplit('_').next().and_then(|id| id.parse::<u32>().ok()).is_some_and(|id| id >= 3);
    let mut inside_made_block = false;
    let (mut checked, mut heaviest) = (0, 0.0f64);
    for (line, weight) in &lines {
        match line {
            Line::Label(name) => inside_made_block = made(name),
            Line::Inst(_) if inside_made_block => {
                assert!(
                    (weight - 32.0).abs() < 1e-6 || (weight - 16.0).abs() < 1e-6,
                    "{line:?} weighs {weight}: {lines:?}"
                );
                heaviest = heaviest.max(*weight);
                checked += 1;
            }
            _ => {}
        }
    }
    assert!(checked > 0, "no block was cut from the loop: {lines:?}");
    assert!((heaviest - 32.0).abs() < 1e-6, "no block cut from the loop weighs the loop: {lines:?}");
}

const SUM_41: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8\"\n\
@t = global [41 x i16] zeroinitializer\n\
define i16 @f() {\n\
entry:\n  br label %loop\n\
loop:\n  %i = phi i16 [ 0, %entry ], [ %in, %loop ]\n  %s = phi i16 [ 0, %entry ], [ %sn, %loop ]\n\
  %p = getelementptr [41 x i16], ptr @t, i16 0, i16 %i\n  %v = load i16, ptr %p\n  %sn = add i16 %s, %v\n\
  %in = add i16 %i, 1\n  %d = icmp ult i16 %in, 41\n  br i1 %d, label %loop, label %exit\n\
exit:\n  ret i16 %sn\n}";

fn busiest(flat_loop_trips: bool) -> f64 {
    let request = Request { passes: vec!["-fno-unroll-loops".into()], flat_loop_trips, ..Request::new(Level::O1) };
    let lines = llrm_z80::compile::weighted_lines(SUM_41, "f", &request).expect("compiles");
    lines.iter().filter(|(line, _)| matches!(line, Line::Inst(_))).map(|(_, weight)| *weight).fold(0.0, f64::max)
}

/// DEVIATE Z80-BFI-TRIPS: a loop ScalarEvolution counts runs its trips in the
/// weights. The pin's branch heuristics weigh every such loop 32, a loop of 10
/// like a loop of 41: byval_struct -O1 took a variant 3 T a trip slower in its
/// 41-trip loop for 4 T a trip in its 10-trip one.
#[test]
fn a_loop_of_forty_one_trips_weighs_forty_one() {
    let weight = busiest(false);
    assert!((weight - 41.0).abs() < 0.5, "{weight}");
}

/// The pin's own frequencies, with the deviation off: 32.
#[test]
fn flat_loop_trips_leave_the_pins_thirty_two() {
    let weight = busiest(true);
    assert!((weight - 32.0).abs() < 0.5, "{weight}");
}

/// pattern_fill -O1: the loop's join, cut from its block by a select, took the
/// weight of the label before it, the arm's 40 for the loop's 64, and the
/// variant with the counter in it (163 T a trip) was priced 144 trips short:
/// +21777 T for -41 B in the run. The decrement that ends the loop weighs the
/// loop.
#[test]
fn the_join_of_a_select_weighs_the_block_it_was_cut_from() {
    let lines =
        llrm_z80::compile::weighted_lines(include_str!("kernels/ir/sum_nonzero.ll"), "sum", &Request::new(Level::O1))
            .expect("compiles");
    let insts: Vec<(&str, f64)> = lines
        .iter()
        .filter_map(|(line, weight)| match line {
            Line::Inst(text) => Some((text.as_str(), *weight)),
            _ => None,
        })
        .collect();
    let heaviest = insts.iter().map(|(_, weight)| *weight).fold(0.0, f64::max);
    let counter = insts
        .windows(2)
        .find(|pair| pair[0].0.starts_with("dec ") && pair[1].0.starts_with("jr "))
        .map(|pair| pair[0].1)
        .expect("the loop ends in a decrement and a jump");
    assert!((counter - heaviest).abs() < 0.5, "counter {counter}, loop {heaviest}");
}

/// A block the passes made takes what flows into it. pattern_fill -O1: the
/// loop's join, cut from its block, took the weight of the label before it,
/// the arm's 40 for the loop's 64, and the variant with the counter in it
/// (163 T a trip) was priced 144 trips short: +21777 T for -41 B in the run.
/// The header (64) jumps over the arm (40) to the join: 24 + 40 = 64.
#[test]
fn a_block_made_by_a_pass_takes_the_flow_into_it() {
    set_frequencies(&[(1, 64.0), (4, 40.0)]);
    let priced =
        price_in_loops(&lines_of(&["L0_1:", "inc hl", "jr z, L0_9", "L0_4:", "inc de", "L0_9:", "inc bc", "ret"]));
    set_frequencies(&[]);
    // (6 + 12) * 64 + 6 * 40 + (6 + 10) * 64
    assert_eq!(priced.1, 2416, "{priced:?}");
}
