//! The function built with and without the `adda`/`suba` fold: the cheaper is
//! kept. The fold is priced before allocation, which cannot see the moves its
//! address register costs (+2 B on frame_address_stack_args, word_compares,
//! sfm16_float_cmp at 68000).

use llrm_m68k::compile::{Item, Options, items_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::timing::Price;

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define i32 @f(i32 %t, i32 %n) {
entry:
  %a = alloca [8 x i32]
  br label %fill
fill:
  %i = phi i32 [0, %entry], [%i1, %fill]
  %v = mul i32 %i, 5
  %w = and i32 %v, 7
  %p = getelementptr i32, ptr %a, i32 %i
  store i32 %w, ptr %p
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, 8
  br i1 %c, label %fill, label %count
count:
  %j = phi i32 [0, %fill], [%j1, %count]
  %s = phi i32 [0, %fill], [%s1, %count]
  %q = getelementptr i32, ptr %a, i32 %j
  %x = load i32, ptr %q
  %g = icmp sgt i32 %x, %t
  %z = zext i1 %g to i32
  %s1 = add i32 %s, %z
  %j1 = add i32 %j, 1
  %d = icmp slt i32 %j1, %n
  br i1 %d, label %count, label %done
done:
  ret i32 %s1
}
";

fn build(sext_adds: bool) -> Vec<Item> {
    let module = llrm_mir::parse::module(IR).expect("parses");
    items_with(&module, "f", Cpu::M68000, Options { sext_adds, ..Options::default() }).expect("compiles")
}

fn text(items: &[Item]) -> String {
    items.iter().map(|item| format!("{item:?}")).collect::<Vec<_>>().join("\n")
}

fn position(
    items: &[Item],
    opcode: &str,
) -> usize {
    items.iter().position(|item| matches!(item, Item::Inst(inst) if inst.opcode == opcode)).expect(opcode)
}

/// A register-to-register `move.l` of the build, to be added as the cost of a
/// fold.
fn a_move(items: &[Item]) -> Item {
    items
        .iter()
        .find(|item| matches!(item, Item::Inst(inst) if inst.opcode == "MOV32rr"))
        .expect("the build has a data-register move")
        .clone()
}

#[test]
fn the_fold_is_kept_when_it_is_cheaper() {
    let (with, without) = (build(true), build(false));
    let kept = llrm_m68k::trial::cheaper(with.clone(), without, Cpu::M68000, Price::Rate);
    assert_eq!(text(&kept), text(&with));
}

/// A move added in the loop (what an address register between data neighbours
/// costs) outweighs the `ext.l` the fold saves: the build without it is kept.
/// Before, the fold stayed (+2 B).
#[test]
fn a_fold_whose_moves_cost_more_than_it_saves_is_dropped() {
    let (with, without) = (build(true), build(false));
    let at = position(&with, "SUBA16r");
    let mut dearer = with.clone();
    let extra = a_move(&without);
    dearer.insert(at, extra.clone());
    dearer.insert(at, extra);
    let kept = llrm_m68k::trial::cheaper(dearer, without.clone(), Cpu::M68000, Price::Rate);
    assert_eq!(text(&kept), text(&without));
}

/// The same move after the loop runs once, the `ext.l` saved in the loop runs
/// every time: the fold stays.
#[test]
fn a_move_outside_the_loop_does_not_outweigh_a_saving_inside_it() {
    let (with, without) = (build(true), build(false));
    let end = position(&with, "RTS");
    let mut dearer = with.clone();
    dearer.insert(end, a_move(&without));
    let kept = llrm_m68k::trial::cheaper(dearer.clone(), without, Cpu::M68000, Price::Rate);
    assert_eq!(text(&kept), text(&dearer));
}

/// `Bne8` over 130 bytes does not reach: the assembler gives it 4 bytes. The
/// trial counted 2, so a build with the far branch looked 2 bytes smaller than
/// one without it (stft_fixed -O1 grew 2 B when the other join order was kept).
#[test]
fn a_branch_that_does_not_reach_counts_four_bytes() {
    use llrm_m68k::mc::{Inst, Operand};
    use llrm_m68k::reg;
    let label = |name: &str| Item::Label(name.to_owned());
    let inst = |opcode: &'static str, operands: Vec<Operand>| Item::Inst(Inst { opcode, operands });
    let moves = |n: usize| (0..n).map(|_| inst("MOV32rr", vec![Operand::Reg(reg::D0), Operand::Reg(reg::D1)]));
    // skip: 2 + 130 + 2 = 134 B counted short, 136 laid out; plain: 134 B, the
    // same size as the short count
    let mut skip = vec![label("f"), inst("Bne8", vec![Operand::Symbol("end".into(), 0)])];
    skip.extend(moves(65));
    skip.extend([label("end"), inst("RTS", vec![])]);
    let mut plain = vec![label("f")];
    plain.extend(moves(66));
    plain.push(inst("RTS", vec![]));
    let mut small = vec![label("f")];
    small.extend(moves(60));
    small.push(inst("RTS", vec![]));
    assert!(llrm_m68k::trial::keeps(&small, &plain, Cpu::M68000, Price::Size), "premise: these build up are priced");
    assert!(
        !llrm_m68k::trial::keeps(&skip, &plain, Cpu::M68000, Price::Size),
        "the far branch is 4 bytes: the skip build is 136 B, larger than the plain 134"
    );
}
