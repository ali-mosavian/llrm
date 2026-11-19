//! `move.l #65535, %d0` is 6 bytes where `moveq #0, %d0; not.w %d0` is 4: a
//! long constant is the cheapest `moveq` and one register operation that
//! makes it, priced on the CPU. The text, the values under qemu (a wrong
//! semantics for a step would still assemble), and the codes: `not.w` sets N
//! from the word, so a load whose codes are read keeps `move.l`.

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::consts::{made, materialize_constants};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::dagisel::local;
use llrm_m68k::mfunc::{MFunction, op_inst};
use llrm_m68k::object::{executable, start, write_executable};
use llrm_m68k::reg;
use llrm_m68k::regalloc::Assignment;
use llrm_spike_mf::{Block, Operand, Reg};

mod support;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn module(ir: &str) -> llrm_mir::module::Module {
    llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).expect("parses")
}

/// The constant `k` in a register (a select against the complement keeps it
/// from folding into an immediate operand and leaves no bit known to the
/// mid-end), its four bytes weighted 1, 3, 5, 7 and summed, cut to a byte: a
/// wrong byte changes the status.
fn byte_of(k: i64) -> String {
    format!(
        "define i32 @f(i1 %c) {{
  %k = select i1 %c, i32 {k}, i32 {}
  %b0 = and i32 %k, 255
  %s1 = lshr i32 %k, 8
  %b1 = and i32 %s1, 255
  %t1 = shl i32 %b1, 1
  %w1 = add i32 %t1, %b1
  %s2 = lshr i32 %k, 16
  %b2 = and i32 %s2, 255
  %t2 = shl i32 %b2, 2
  %w2 = add i32 %t2, %b2
  %b3 = lshr i32 %k, 24
  %t3 = shl i32 %b3, 3
  %w3 = sub i32 %t3, %b3
  %a = add i32 %b0, %w1
  %b = add i32 %a, %w2
  %r = add i32 %b, %w3
  %m = and i32 %r, 255
  ret i32 %m
}}",
        i64::from(!(k as i32))
    )
}

/// What [`byte_of`] returns.
fn checksum(k: u32) -> i32 {
    ((k & 255) + 3 * ((k >> 8) & 255) + 5 * ((k >> 16) & 255) + 7 * (k >> 24)) as i32 & 255
}

#[test]
fn a_word_of_ones_and_a_shifted_byte_are_a_moveq_and_one_step() {
    for (k, step) in [(65535, "not.w\t%d"), (196608, "swap\t%d"), (8323072, "swap\t%d")] {
        let out = text_with(&module(&byte_of(k)), "f", Cpu::M68000, Options::default()).expect("compiles");
        assert!(out.contains("moveq") && out.contains(step), "{k}: no moveq and step:\n{out}");
        assert!(!out.contains(&format!("move.l\t#{k},")), "{k}: a long immediate is left:\n{out}");
    }
}

/// Bytes first at the size level: `moveq #-128; lsl.l #8` is 4 bytes for 6.
#[test]
fn at_the_size_level_a_shift_of_a_moveq_beats_the_long_immediate() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module(&byte_of(-32768)), "f", cpu, Options { size: true, ..Options::default() })
            .expect("compiles");
        assert!(out.contains("moveq\t#-128") && out.contains("lsl.l\t#8"), "{cpu:?}: no moveq and shift:\n{out}");
    }
}

/// One constant for every step the table picks on a CPU and level, and the
/// bytes of each from the hardware: a step whose semantics the table gets
/// wrong makes another constant than it claims. The mid-end rewrites a few
/// selects (`INT_MIN` against `INT_MAX`), so a step tries up to eight of its
/// values until one reaches the backend as the `moveq` the table names.
#[test]
fn the_constant_a_sequence_makes_is_the_constant() {
    let Some(qemu) = support::qemu() else { return };
    for (cpu, flag) in [(Cpu::M68000, "m68000"), (Cpu::M68020, "m68020")] {
        for size in [false, true] {
            let options = Options { size, ..Options::default() };
            let mut steps: std::collections::BTreeMap<(&str, Option<i64>), Vec<(u32, i64)>> = Default::default();
            for (value, m, row, count) in made(cpu, size) {
                steps.entry((row, count)).or_default().push((value, m));
            }
            let (mut checked, mut missed) = (0, Vec::new());
            for ((row, count), values) in steps {
                let stride = values.len().div_ceil(8);
                let tried: Vec<_> = values.into_iter().step_by(stride).collect();
                let mut done = false;
                for (k, m) in tried {
                    let ir = byte_of(i64::from(k as i32));
                    let text = text_with(&module(&ir), "f", cpu, options).expect("compiles");
                    if !text.contains(&format!("moveq\t#{m},")) {
                        continue;
                    }
                    let mut all = start("f", &[1]);
                    all.extend(items_with(&module(&ir), "f", cpu, options).expect("compiles"));
                    let path = support::unique_path("consts");
                    write_executable(&path, &executable(&all).unwrap()).unwrap();
                    let code = support::run(&qemu, &["-cpu", flag], &path);
                    std::fs::remove_file(&path).ok();
                    assert_eq!(code, Some(checksum(k)), "{row} {count:?}: {k:#x} on {cpu:?}, size {size}");
                    done = true;
                    break;
                }
                if done {
                    checked += 1;
                } else {
                    missed.push((row, count));
                }
            }
            assert!(
                checked >= 3 && missed.len() <= checked,
                "{cpu:?} size {size}: checked {checked}, missed {missed:?}"
            );
        }
    }
}

/// A `beq` after the load reads the codes the long form set; the `not.w` form
/// sets them from the word.
#[test]
fn a_load_whose_codes_are_read_keeps_the_long_immediate() {
    let load = |reader: &str| {
        let movi = local::code_of("MOVI32ri").and_then(local::desc).expect("MOVI32ri");
        let reader_desc = local::code_of(reader).and_then(local::desc).expect("a reader");
        let mut function = MFunction::new("f");
        let name = local::name_of(u32::from(local::code_of(reader).unwrap())).unwrap();
        function
            .blocks
            .push(
                Block {
                    id: 0,
                    insts: vec![
                        op_inst("MOVI32ri", movi, vec![Operand::Def(Reg::Phys(reg::D0)), Operand::Imm(65535)]),
                        op_inst(name, reader_desc, vec![Operand::Block(0)]),
                    ],
                    phis: Vec::new(),
                },
            );
        materialize_constants(&mut function, &Assignment(Vec::new()), Cpu::M68000, false);
        function.blocks[0].insts.len()
    };
    assert_eq!(load("Beq8"), 2, "the branch reads the codes: the load must stay one instruction");
    assert_eq!(load("BRA8"), 3, "no reader of the codes: the load is a moveq and a step");
}
