//! The text of a body. The old dump printed `jl` with no target, `call` with no
//! callee, `0x0012` twice, `; inserted` on every line and a trailing `;` on
//! an empty note; none of it said what a block did at a glance.

use std::sync::Arc;

use llrm_lir::registers::RegId;
use llrm_lir::{Held, Imm, Loc, Operation, Reg, Semantics};

use super::lirtext::{Calls, Mode, Style, lir_stage_with};
use crate::model::lir::{Insn, LirBlock, LirBody, Phi};
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
        phis: vec![Phi { result: 5, incoming: vec![(0, 2), (BB1, 4)] }].into(),
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
/// still defined `v5` from `v2`.
#[test]
fn an_elided_operation_still_says_what_it_defines_and_reads() {
    let one = Insn { defines: vec![5], uses: vec![2], widths: vec![(5, 2)], ..insn(3, crate::model::lir::inert()) };
    let block = LirBlock::new(0, vec![Arc::new(one)]);
    let body = LirBody::new("f", 0, vec![block.into()], IndexMap::default(), IndexMap::default());
    let text = lir_stage_with("T", &[("f".to_owned(), body)], &Calls::default(), Mode::Compact, Style::Plain);
    assert!(text.contains("     #1  v5         = elided      v2\n"), "{text}");
}

/// Said `0x0012  jl   ; inserted` with no target, a call with neither callee
/// nor arguments, `; inserted` everywhere, and a block by address.
#[test]
fn a_body_says_where_it_goes_what_it_calls_and_nothing_else() {
    let expected = "\
fn f(v1@ax) -> @eax   ; calls clobber eax ecx
frame  slot0 [bp-4] 2 B

B0  entry  → B1
     #2  v2         = mov.i2      1
     #3               jmp         B1

    B1  ← B0 B1  → B1 B2  loop
             v5         = phi         B0: v2, B1: v4
         #4  v4         = sub.i2      v5, 1
         #5               cmp.i2      v4, -1
         #6               jne         B1 else B2

B2  ← B1  → exit
     #7  v7         = load.i2     [slot0]                               ; reload
     #8  v10@eax    = call        fib(v7@ax)
     #9  v11@eax    = call        fib(v7@ax)
    #10  v12@eax    = call        log(v7@ax)                            ; also clobbers esi
    #11               ret         v12@eax

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
    assert!(added[0].contains("#12") && added[0].contains("v9"), "{added:#?}");
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
    assert_eq!(labels, [1, 2, 3, 12], "the copy of #2 is a new instruction");
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
