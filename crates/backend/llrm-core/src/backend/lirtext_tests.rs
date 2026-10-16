//! The text of a body. The old dump printed `jl` with no target, `call` with no
//! callee, `0x0012` twice, `; inserted` on every line and a trailing `;` on
//! an empty note; none of it said what a block did at a glance.

use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::{Held, Imm, Loc, Operation, Reg, Semantics};

use super::lirtext::{Calls, Mode, Style, lir_stage_with};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi, ValueNames};
use crate::support::hash::IndexMap;

const BB1: i64 = 0x40;
const BB2: i64 = 0x1_0000_0001;

fn held(
    value: u32,
    width: u32,
) -> Loc {
    Loc::Held(Held { value, width })
}

fn imm(value: i64) -> Loc {
    Loc::Imm(Imm { value, width: 2, address: None })
}

fn what(
    op: Operation,
    name: &str,
    dests: Vec<Loc>,
    sources: Vec<Loc>,
    target: Option<i64>,
) -> Semantics {
    Semantics { name: Some(name.to_owned()), dests, sources, target, ..Semantics::new(op) }
}

fn insn(
    at: i64,
    what: Semantics,
) -> Insn {
    Insn::new(at, Some((at, at)), Some(what), vec![], vec![])
}

/// A loop (`bb1`) that counts down, then a block that calls and returns.
fn body() -> LirBody {
    let arrival = Insn {
        delivers: vec![(Held { value: 1, width: 2 }, RegId::AX)],
        ..insn(0, Semantics { name: Some(String::new()), ..Semantics::new(Operation::Nothing) })
    };
    let entry = LirBlock {
        succ: vec![BB1].into(),
        ..LirBlock::new(
            0,
            vec![
                Arc::new(arrival),
                Arc::new(insn(0, what(Operation::Move, "mov", vec![held(2, 2)], vec![imm(1)], None))),
                Arc::new(insn(1, what(Operation::Jump, "jmp", vec![], vec![], Some(BB1)))),
            ],
        )
    };
    let looping = LirBlock {
        succ: vec![BB1, BB2].into(),
        phis: vec![Phi { result: 5, incoming: vec![(0, 2), (BB1, 4)], label: Default::default() }].into(),
        ..LirBlock::new(
            BB1,
            vec![
                Arc::new(insn(4, what(Operation::Binary, "sub", vec![held(4, 2)], vec![held(5, 2), imm(1)], None))),
                Arc::new(insn(6, what(Operation::Compare, "cmp", vec![], vec![held(4, 2), imm(-1)], None))),
                Arc::new(insn(6, what(Operation::Branch, "jne", vec![], vec![], Some(BB1)))),
            ],
        )
    };
    let reload = Insn {
        spill_reload: true,
        ..insn(
            9,
            what(
                Operation::Move,
                "mov",
                vec![held(7, 2)],
                vec![Loc::Mem(llrm_lir::Mem::new(
                    Some(llrm_omf::module::Addr::new(llrm_omf::module::Space::Frame, -4)),
                    2,
                ))],
                None,
            ),
        )
    };
    let call = |at, clobbers: &[RegId]| Insn {
        requires: vec![(Held { value: 7, width: 2 }, RegId::AX)],
        delivers: vec![(Held { value: at as u32, width: 4 }, RegId::EAX)],
        clobbers: clobbers.iter().copied().collect(),
        ..insn(at, what(Operation::Call, "call", vec![], vec![], None))
    };
    let leaving = LirBlock::new(
        BB2,
        vec![
            Arc::new(reload),
            Arc::new(call(10, &[RegId::EAX, RegId::ECX])),
            Arc::new(call(11, &[RegId::EAX, RegId::ECX])),
            Arc::new(call(12, &[RegId::EAX, RegId::ECX, RegId::ESI])),
            Arc::new(Insn {
                requires: vec![(Held { value: 12, width: 4 }, RegId::EAX)],
                ..insn(13, what(Operation::Return, "ret", vec![], vec![], None))
            }),
        ],
    );
    let mut body = LirBody::new(
        "f",
        0,
        vec![entry.into(), looping.into(), leaving.into()],
        IndexMap::default(),
        IndexMap::default(),
    );
    body.bits = 16;
    body
}

fn dump(mode: Mode) -> String {
    let calls = Calls {
        names: [(10, "fib".to_owned()), (11, "fib".to_owned()), (12, "log".to_owned())].into_iter().collect(),
        far: Default::default(),
    };
    lir_stage_with("Test", &[("f".to_owned(), body())], &calls, mode, Style::Plain)
}

/// Printed `nothing` for an operation selection had removed, hiding that it
/// still defined `t5` from `t2`.
#[test]
fn an_elided_operation_still_says_what_it_defines_and_reads() {
    let one = Insn { defines: vec![5], uses: vec![2], widths: vec![(5, 2)], ..insn(3, crate::model::lir::inert()) };
    let block = LirBlock::new(0, vec![Arc::new(one)]);
    let body = LirBody::new("f", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
    let text = lir_stage_with("T", &[("f".to_owned(), body)], &Calls::default(), Mode::Compact, Style::Plain);
    assert!(text.contains("     #1  t5         = elided      t2\n"), "{text}");
}

/// Said `0x0012  jl   ; inserted` with no target, a call with neither callee
/// nor arguments, `; inserted` everywhere, and a block by address.
#[test]
fn a_body_says_where_it_goes_what_it_calls_and_nothing_else() {
    let expected = "\
fn f(t1@ax) -> @eax   ; calls clobber eax ecx
frame  slot0 [bp-4] 2 B

B0  entry  → B1
     #2  t2         = mov.i16     1
     #3               jmp         B1

    B1  ← B0 B1  → B1 B2  loop
         #4  t5         = phi         B0: t2, B1: t4
         #5  t4         = sub.i16     t5, 1
         #6               cmp.i16     t4, -1
         #7               jne         B1 else B2

B2  ← B1  → exit
     #8  t7         = load.i16    [slot0]                               ; reload
     #9  t10@eax    = call        fib(t7@ax)
    #10  t11@eax    = call        fib(t7@ax)
    #11  t12@eax    = call        log(t7@ax)                            ; also clobbers esi
    #12               ret         t12@eax

";
    let text = dump(Mode::Compact);
    let (banner, rest) = text.split_once('\n').expect("a banner");
    assert!(banner.starts_with("═══ f · Test · m16 ═"), "{banner}");
    assert_eq!(banner.chars().count(), 80);
    assert_eq!(rest, expected);
}

/// An address in a dump moves when an unrelated instruction is added and
/// every line after it differs; a block's name is its position.
#[test]
fn no_address_prints() {
    let text = dump(Mode::Compact);
    assert!(!text.contains("0x"), "{text}");
    assert!(!text.contains("4294967297"), "{text}");
}

/// Guard: a note that says nothing leaves no `;` and no trailing space.
#[test]
fn no_line_ends_in_a_bare_semicolon_or_spaces() {
    for mode in [Mode::Compact, Mode::Verbose] {
        for line in dump(mode).lines() {
            assert!(!line.ends_with(';') && !line.ends_with(' '), "{line:?}");
        }
    }
}

#[test]
fn colour_is_in_the_text_only_when_asked() {
    let calls = Calls::default();
    let plain = lir_stage_with("T", &[("f".to_owned(), body())], &calls, Mode::Compact, Style::Plain);
    let colour = lir_stage_with("T", &[("f".to_owned(), body())], &calls, Mode::Compact, Style::Color);
    assert!(!plain.contains('\x1b'));
    assert!(colour.contains('\x1b'));
}

#[test]
fn verbose_adds_what_a_block_holds_live_and_what_a_call_needs() {
    let text = dump(Mode::Verbose);
    assert!(text.contains("live-in"), "{text}");
    assert!(text.contains("live-out"), "{text}");
}

fn _unused(_: Reg) {}

/// A block per loop level: `B1` in the outer loop, `B2` in the middle, `B3` in
/// the inner one, `B4` and `B5` on the way back out.
fn nest() -> LirBody {
    let at = |n: i64| n * 8;
    let block = |n: i64, to: &[i64]| -> crate::model::lir::Block {
        let jump = what(Operation::Jump, "jmp", vec![], vec![], to.first().map(|n| at(*n)));
        LirBlock {
            succ: to.iter().map(|n| at(*n)).collect::<Vec<_>>().into(),
            ..LirBlock::new(at(n), vec![Arc::new(insn(at(n), jump))])
        }
        .into()
    };
    let blocks = vec![
        block(0, &[1]),
        block(1, &[2, 6]),
        block(2, &[3, 5]),
        block(3, &[3, 4]),
        block(4, &[2]),
        block(5, &[1]),
        block(6, &[]),
    ];
    let mut body = LirBody::new("nest", 0, blocks, IndexMap::default(), IndexMap::default());
    body.loop_trip_counts = vec![(at(3), 100)];
    body
}

/// Three loops deep: every block is indented four spaces a loop, a loop's
/// header says `loop`, and the innermost says its trips.
#[test]
fn a_block_is_indented_by_the_loops_around_it() {
    let text = lir_stage_with("T", &[("nest".to_owned(), nest())], &Calls::default(), Mode::Compact, Style::Plain);
    let header = |name: &str| {
        text.lines().find(|line| line.trim_start().starts_with(&format!("{name}  "))).unwrap_or_default().to_owned()
    };
    let indents: Vec<usize> =
        (0..7).map(|n| header(&format!("B{n}")).len() - header(&format!("B{n}")).trim_start().len()).collect();
    assert_eq!(indents, [0, 4, 8, 12, 8, 4, 0], "{text}");
    assert!(header("B1").contains("loop") && !header("B1").contains("trips"), "{text}");
    assert!(header("B3").contains("loop 100 trips"), "{text}");
    assert!(!header("B4").contains("loop"), "{text}");
}

/// The same body with one instruction added mid-block: every other line is as
/// it was, and the new one has a number nothing had. Numbered by position, the
/// dump moved every line after the insertion, and a diff of two stages read as
/// a rewrite.
#[test]
fn an_inserted_instruction_changes_one_line_of_the_diff() {
    let before = body();
    let first = dump_of(&before);
    let mut insns: Vec<Arc<Insn>> = before.blocks[1].insns.iter().cloned().collect();
    insns.insert(1, Arc::new(insn(5, what(Operation::Move, "mov", vec![held(9, 2)], vec![held(4, 2)], None))));
    let mut blocks = before.blocks.clone();
    blocks[1] = blocks[1].with_insns(insns);
    let after = before.with_blocks(blocks);
    let second = dump_of(&after);
    let (old, new): (Vec<_>, Vec<_>) = (first.lines().collect(), second.lines().collect());
    let removed: Vec<_> = old.iter().filter(|line| !new.contains(line)).collect();
    let added: Vec<_> = new.iter().filter(|line| !old.contains(line)).collect();
    assert!(removed.is_empty(), "{removed:#?}");
    assert_eq!(added.len(), 1, "{added:#?}");
    assert!(added[0].contains("#13") && added[0].contains("t9"), "{added:#?}");
}

fn dump_of(body: &LirBody) -> String {
    lir_stage_with("T", &[("f".to_owned(), body.clone())], &Calls::default(), Mode::Compact, Style::Plain)
}

/// A clone made to be changed keeps its number; a copy of an instruction put
/// where another is gets its own.
#[test]
fn a_clone_keeps_its_number_and_a_duplicate_gets_a_new_one() {
    let body = body();
    body.number_insns();
    let original = Arc::clone(&body.blocks[0].insns[1]);
    let changed = Insn { covers: Some((7, 7)), ..(*original).clone() };
    assert_eq!(changed.label.get(), original.label.get());
    let mut insns: Vec<Arc<Insn>> = body.blocks[0].insns.iter().cloned().collect();
    insns.push(Arc::new(original.as_ref().clone()));
    let mut blocks = body.blocks.clone();
    blocks[0] = blocks[0].with_insns(insns);
    let widened = body.with_blocks(blocks);
    widened.number_insns();
    let labels: Vec<u32> = widened.blocks[0].insns.iter().map(|insn| insn.label.get()).collect();
    assert_eq!(labels, [1, 2, 3, 13], "the copy of #2 is a new instruction");
}

/// Labels are not part of what an instruction is: numbering a body for a dump
/// must not change what a pass that compares instructions or lists finds.
#[test]
fn numbering_changes_neither_equality_nor_a_list() {
    let (numbered, bare) = (body(), body());
    numbered.number_insns();
    assert_eq!(numbered, bare);
    let list = numbered.blocks[2].insns.clone();
    numbered.number_insns();
    assert!(numbered.blocks[2].insns.same_as(&list), "numbering rebuilt a list");
    assert!(Arc::ptr_eq(&numbered.blocks[2].insns[0], &list[0]));
}

/// A phi has a number like an instruction, kept when a pass rebuilds it: values
/// merge there, so it is where a diff of two stages matters most.
#[test]
fn a_phi_is_numbered_and_keeps_its_number() {
    let body = body();
    body.number_insns();
    let phi = &body.blocks[1].phis[0];
    assert_eq!(phi.label.get(), 4);
    let renamed = Phi { result: 9, ..phi.clone() };
    assert_eq!(renamed.label.get(), 4);
    assert!(dump_of(&body).contains("     #4  t5         = phi"), "{}", dump_of(&body));
}

/// Every value printed `v12`, whatever it was. A value MIR named prints as it
/// is named, a copy of it too, and a name two values share means one.
#[test]
fn a_named_value_prints_by_name_and_its_copy_keeps_it() {
    let mut body = body();
    body.names = ValueNames::of([(2, "i".to_owned()), (5, "i".to_owned())]);
    let copy = Insn::new(
        20,
        Some((20, 20)),
        Some(what(Operation::Move, "mov", vec![held(30, 2)], vec![held(2, 2)], None)),
        vec![],
        vec![],
    );
    let mut insns: Vec<Arc<Insn>> = body.blocks[0].insns.iter().cloned().collect();
    insns.insert(2, Arc::new(copy));
    let mut blocks = body.blocks.clone();
    blocks[0] = blocks[0].with_insns(insns);
    let body = body.with_blocks(blocks);
    let text = dump_of(&body);
    assert!(text.contains("  i          = mov.i16     1"), "{text}");
    assert!(text.contains("  i          = mov.i16     i"), "the copy keeps the name\n{text}");
    assert!(text.contains("i.2        = phi         B0: i, B1: t4"), "{text}");
    assert!(!text.contains("v2") && !text.contains("v30"), "{text}");
}

/// The arms of a phi are listed by block: built in another order they print as
/// the same phi, and the line matches the block header's predecessors.
#[test]
fn a_phis_arms_are_listed_by_block() {
    let body = body();
    body.number_insns();
    let phi = &body.blocks[1].phis[0];
    let swapped = Phi { incoming: phi.incoming.iter().rev().cloned().collect(), ..phi.clone() };
    let mut blocks = body.blocks.clone();
    let mut reordered = (*blocks[1]).clone();
    reordered.phis = vec![swapped].into();
    blocks[1] = reordered.into();
    let swapped = body.with_blocks(blocks);
    assert_eq!(dump_of(&swapped), dump_of(&body));
}

/// After the phis are gone a value is defined again and again; one copy of a
/// named value kept its name through all of them, so `y6.1` read as written
/// where it was a new version. A copy keeps the name only while it is defined
/// once.
#[test]
fn a_copy_keeps_a_name_only_while_it_is_defined_once() {
    let mut body = body();
    body.names = ValueNames::of([(2, "i".to_owned())]);
    let copy = |at, to, from| {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(what(Operation::Move, "mov", vec![held(to, 2)], vec![held(from, 2)], None)),
            vec![],
            vec![],
        ))
    };
    let mut insns: Vec<Arc<Insn>> = body.blocks[0].insns.iter().cloned().collect();
    insns.insert(2, copy(20, 30, 2));
    insns.insert(3, copy(21, 31, 2));
    insns.insert(4, copy(22, 31, 30));
    let mut blocks = body.blocks.clone();
    blocks[0] = blocks[0].with_insns(insns);
    let text = dump_of(&body.with_blocks(blocks));
    assert!(text.contains("  i          = mov.i16     i"), "defined once: {text}");
    assert!(!text.contains("i          = mov.i16     i\n         #5  i"), "{text}");
    assert_eq!(text.matches("t31").count(), 2, "defined twice: {text}");
}

/// A name kept from an earlier phase, when the value was a copy defined once,
/// does not follow it into the phase that defines it a second time.
#[test]
fn a_kept_copy_name_is_dropped_when_the_value_is_defined_again() {
    let mut body = body();
    body.names = ValueNames::of([(2, "i".to_owned())]);
    let copy = |at, to, from| {
        Arc::new(Insn::new(
            at,
            Some((at, at)),
            Some(what(Operation::Move, "mov", vec![held(to, 2)], vec![held(from, 2)], None)),
            vec![],
            vec![],
        ))
    };
    let with = |body: &LirBody, extra: Vec<Arc<Insn>>| {
        let mut insns: Vec<Arc<Insn>> = body.blocks[0].insns.iter().cloned().collect();
        for (at, one) in extra.into_iter().enumerate() {
            insns.insert(2 + at, one);
        }
        let mut blocks = body.blocks.clone();
        blocks[0] = blocks[0].with_insns(insns);
        body.with_blocks(blocks)
    };
    let mut once = with(&body, vec![copy(20, 30, 2)]);
    once.names = once.with_inherited_names();
    assert!(dump_of(&once).contains("  i          = mov.i16     i"), "{}", dump_of(&once));
    let twice = with(&once, vec![copy(21, 30, 5)]);
    let text = dump_of(&twice);
    assert!(text.contains("  t30        = mov.i16     i") && text.contains("  t30        = mov.i16     t5"), "{text}");
}

/// Several values merged into one number have the name they all had, else none:
/// the name of one of them would be another's.
#[test]
fn a_merged_web_keeps_a_name_only_if_every_member_has_it() {
    let names =
        ValueNames::of([(1, "sum".to_owned()), (2, "sum".to_owned()), (3, "n".to_owned()), (4, "sum".to_owned())]);
    // `ValueNames::of` renames the second `sum`; read the names it made.
    let (first, second) = (names.get(1).unwrap().0.to_owned(), names.get(2).unwrap().0.to_owned());
    assert_ne!(first, second);
    assert_eq!(names.joined(&[(1, 1)]).get(1).map(|(name, _)| name), Some("sum"), "a number joined to itself");
    assert_eq!(names.joined(&[(2, 1)]).get(1), None, "two different names");
    assert_eq!(names.joined(&[(3, 1)]).get(1), None, "two different names");
    assert_eq!(names.joined(&[(5, 1)]).get(1), None, "one member has no name");
    assert_eq!(names.joined(&[(5, 1)]).get(5), None);
    let same = ValueNames::of([(1, "i".to_owned())]);
    assert_eq!(same.joined(&[(1, 1)]).get(1).map(|(name, _)| name), Some("i"));
}

/// A number no instruction names is gone, and a pass may give it to a new
/// value, which must not read as the old one.
#[test]
fn a_name_goes_with_its_number() {
    let names = ValueNames::of([(1, "i".to_owned()), (2, "j".to_owned())]);
    let present: std::collections::BTreeSet<u32> = [1].into_iter().collect();
    let kept = names.only(&present);
    assert_eq!(kept.get(1).map(|(name, _)| name), Some("i"));
    assert_eq!(kept.get(2), None);
}
