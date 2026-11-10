//! `minmax` kept both running values in cells, loaded both arms of each select
//! and stored one back (`lda rc6 / bcc / lda rc5 / sta rc5`), and staged the
//! compare of a value in `x` through `a` (`txa; cmp rc6`): 15 instructions a
//! trip for the 11 the selects need (2026-11-09).

mod common;

use common::pipeline::datalayout;
use llrm_6502::asm::Asm;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::peephole::compare_through_an_index;

fn through_an_index(text: &str) -> String {
    let mut asm = Asm::parse(text);
    compare_through_an_index(&mut asm);
    asm.text()
}

/// `a` is dead after the compare (the next write of it comes first): the
/// compare reads the index register itself.
#[test]
fn a_compare_of_an_index_register_reads_it_where_a_is_dead() {
    let text = "f:\n  txa\n  cmp rc6\n  bcs L\n  ldx rc6\nL:\n  lda #0\n  rts\n";
    assert_eq!(through_an_index(text), "f:\n  cpx rc6\n  bcs L\n  ldx rc6\nL:\n  lda #0\n  rts\n");
    assert_eq!(
        through_an_index("f:\n  tya\n  cmp #7\n  beq L\nL:\n  lda #0\n  rts\n"),
        "f:\n  cpy #7\n  beq L\nL:\n  lda #0\n  rts\n"
    );
}

/// `a` read after the compare keeps it, and so does a compare `cpx` cannot
/// make.
#[test]
fn a_compare_stays_when_a_is_read_or_cpx_has_no_form() {
    for text in [
        "f:\n  txa\n  cmp rc6\n  bcs L\nL:\n  rts\n",
        "f:\n  txa\n  cmp rc6,x\n  bcs L\nL:\n  lda #0\n  rts\n",
        "f:\n  txa\n  cmp (rc6),y\n  bcs L\nL:\n  lda #0\n  rts\n",
    ] {
        assert_eq!(through_an_index(text), text);
    }
}

/// The byte selects update the running values in place: the loop loads one
/// cell (`ldx rc6`) and moves nothing through `tax`.
#[test]
fn a_select_copied_back_to_its_own_register_is_one_conditional_move() {
    let ir = format!(
        "target datalayout = \"{}\"\n\
         define i8 @minmax(ptr %p, i8 %n) {{\nentry:\n  %z = icmp eq i8 %n, 0\n  br i1 %z, label %out, label %l\n\
         l:\n  %i = phi i8 [0, %entry], [%i1, %l]\n  %lo = phi i8 [255, %entry], [%lo1, %l]\n  %hi = phi i8 [0, %entry], [%hi1, %l]\n  \
         %w = zext i8 %i to i16\n  %q = getelementptr i8, ptr %p, i16 %w\n  %v = load i8, ptr %q\n  \
         %c1 = icmp ult i8 %v, %lo\n  %lo1 = select i1 %c1, i8 %v, i8 %lo\n  \
         %c2 = icmp ugt i8 %v, %hi\n  %hi1 = select i1 %c2, i8 %v, i8 %hi\n  \
         %i1 = add i8 %i, 1\n  %cc = icmp ne i8 %i1, %n\n  br i1 %cc, label %l, label %done\n\
         done:\n  %r = sub i8 %hi1, %lo1\n  ret i8 %r\nout:\n  ret i8 1\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("minmax:").unwrap();
    let body: Vec<&str> = text[from..].split(".segment").next().unwrap().lines().map(str::trim).collect();
    // the loop: from the label the back edge goes to, to that branch
    let (head, back) = (0..body.len())
        .rev()
        .find_map(|at| {
            let target = body[at].split_whitespace().nth(1)?;
            let head = body.iter().position(|l| l.strip_suffix(':') == Some(target))?;
            (head < at && body[at].starts_with('b')).then_some((head, at))
        })
        .expect("a loop");
    let body = &body[head..=back];
    let loads = body.iter().filter(|l| l.starts_with("lda rc") || l.starts_with("ldx rc")).count();
    let moves = body.iter().filter(|l| matches!(**l, "tax" | "txa")).count();
    assert!(loads <= 1 && moves == 0, "{loads} loads, {moves} register moves: {body:?}");
}
