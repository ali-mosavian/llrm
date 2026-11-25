//! A word loaded through `(An)+` and multiplied or divided was a load and a
//! register operation: `move.w (%a0)+, %d1; muls.w %d1, %d0` where `muls.w
//! (%a0)+, %d0` does both, 4 clk shorter on the 68000 (500 iterations of a
//! summing loop: ~2 k clk of a +3.8 k clk gap to gcc).

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn asm(ir: &str) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses");
    text_with(&module, "f", Cpu::M68000, Options::default()).expect("compiles")
}

/// `sum += cells[i] * (i + 1)` over words, the loads post-incremented.
fn sum(
    op: &str,
    load: &str,
) -> String {
    format!(
        "define i32 @f(ptr %p, i16 %n) {{
entry:
  br label %loop
loop:
  %q = phi ptr [ %p, %entry ], [ %q1, %loop ]
  %j = phi i16 [ 1, %entry ], [ %j1, %loop ]
  %s = phi i32 [ 0, %entry ], [ %s1, %loop ]
  %v = load {load}, ptr %q
  %a = {op} {load} %v to i32
  %b = {op} i16 %j to i32
  %m = mul i32 %b, %a
  %s1 = add i32 %s, %m
  %q1 = getelementptr {load}, ptr %q, i32 1
  %j1 = add i16 %j, 1
  %z = icmp eq i16 %j1, %n
  br i1 %z, label %done, label %loop
done:
  ret i32 %s1
}}"
    )
}

#[test]
fn a_signed_word_multiply_reads_its_source_through_post_increment() {
    let text = asm(&sum("sext", "i16"));
    assert!(text.contains("muls\t(%a0)+") || text.contains("muls\t(%a1)+"), "{text}");
}

#[test]
fn an_unsigned_word_multiply_reads_its_source_through_post_increment() {
    let text = asm(&sum("zext", "i16"));
    assert!(text.contains("mulu\t(%a0)+") || text.contains("mulu\t(%a1)+"), "{text}");
}

/// A long loaded through `(An)+` into the word source of a word multiply:
/// `muls.w (%a0)+` would read the high word and step by two, not four.
/// The name of the multiply says 32 (its result), so a width test on the
/// name alone let it fold.
fn load_then_multiply(load: &'static str) -> usize {
    use llrm_m68k::foldload::fold_loads;
    use llrm_m68k::mfunc::{Block, Class, Inst, Kind, MFunction, Operand, Reg};
    let virt = Reg::Virt;
    let mut loaded =
        Inst::new(Kind::Op(load), vec![Operand::Def(virt(0)), Operand::Def(virt(1)), Operand::Use(virt(2))]);
    loaded.may_load = true;
    let multiply =
        Inst::new(Kind::Op("SMULd32d16"), vec![Operand::Def(virt(3)), Operand::Use(virt(4)), Operand::Use(virt(0))]);
    let mut function = MFunction::new("f");
    function.classes = vec![Class::Xr32; 5];
    function.blocks.push(Block { id: 0, insts: vec![loaded, multiply], phis: vec![] });
    fold_loads(&mut function);
    function.blocks[0].insts.len()
}

#[test]
fn a_word_load_folds_into_the_word_multiply_and_a_long_load_does_not() {
    assert_eq!(load_then_multiply("MOV16do_WB"), 1);
    assert_eq!(load_then_multiply("MOV32do_WB"), 2);
}

/// An unpriced row makes the whole function unpriced, and a module trial
/// then fails closed. The fold must also be no larger and no slower than the
/// load and the register operation it replaces, on every CPU.
#[test]
fn the_folded_form_is_priced_and_no_dearer_than_the_two_it_replaces() {
    use llrm_m68k::isa::Cpu;
    use llrm_m68k::timing::cost_on;
    for cpu in [Cpu::M68000, Cpu::M68010, Cpu::M68020, Cpu::M68030, Cpu::M68040, Cpu::M68060] {
        for op in ["muls", "mulu", "divs", "divu"] {
            let load =
                cost_on(cpu, "move.w\t(%a0)+, %d2", 2, false).unwrap_or_else(|why| panic!("{cpu:?} load: {why}"));
            let reg =
                cost_on(cpu, &format!("{op}\t%d2, %d3"), 2, false).unwrap_or_else(|why| panic!("{cpu:?} {op}: {why}"));
            let mem = cost_on(cpu, &format!("{op}\t(%a0)+, %d3"), 2, false)
                .unwrap_or_else(|why| panic!("{cpu:?} {op} (%a0)+: {why}"));
            assert!(
                mem.clocks <= load.clocks + reg.clocks,
                "{cpu:?} {op}: {} > {} + {}",
                mem.clocks,
                load.clocks,
                reg.clocks
            );
        }
    }
}
