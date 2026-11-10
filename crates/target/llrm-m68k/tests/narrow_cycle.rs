//! crc16 carried its `crc` in a long and read it through `& 0xFFFF` and a
//! test of bit 15, so every `shl`/`xor` of the bit loop was a long (8 clk on
//! the 68000, 4 as a word) and `cmpcarry` could not see a word `add`.
//! `narrowcycle` carries such a cycle at the width it is read at, where the
//! timing tables say a word is cheaper.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::machine::M68kMachine;
use llrm_mir::interpret::{self, Val};
use llrm_mir::module::Module;
use llrm_transforms::indvars::IndVars;

const LAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n\n";

/// A bit loop of `crc = bit15 ? (crc << 1) ^ 0x1021 : crc << 1`, the exit
/// reading `exit_use` of the last `crc`.
fn crc_loop(
    exit_use: &str,
    step: &str,
) -> String {
    format!(
        "{LAYOUT}define i32 @f(i32 %n) {{
entry:
  %m = or i32 %n, 3
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %crc = phi i32 [ %n, %entry ], [ %r, %loop ]
  %bit = and i32 %crc, 32768
  %c = icmp ne i32 %bit, 0
  %s = shl i32 %crc, 1
  %x = {step}
  %r = select i1 %c, i32 %x, i32 %s
  %next = add i32 %i, 1
  %t = mul i32 %next, %m
  %go = icmp ult i32 %t, 800
  br i1 %go, label %loop, label %exit
exit:
  %out = phi i32 [ %r, %loop ]
  {exit_use}
}}
"
    )
}

const MASKED: &str = "%v = and i32 %out, 65535\n  ret i32 %v";

fn parsed(text: &str) -> Module {
    llrm_mir::parse::module(text).unwrap_or_else(|error| panic!("{error}\n{text}"))
}

fn printed(module: &Module) -> String {
    let text = llrm_mir::print::module(module);
    assert_eq!(llrm_mir::verify::verify(module), Vec::<String>::new(), "{text}");
    text
}

fn results(
    module: &Module,
    inputs: &[&[i128]],
) -> Vec<Val> {
    inputs
        .iter()
        .map(|input| {
            let arguments = input.iter().map(|&value| Val::Int { bits: value as u32 as u128, width: 32 }).collect();
            interpret::run(module, "f", arguments, 100_000).unwrap_or_else(|trap| panic!("@f{input:?}: {trap:?}"))
        })
        .collect()
}

fn run(
    text: &str,
    cpu: Cpu,
) -> String {
    let mut module = parsed(text);
    let inputs: &[&[i128]] = &[&[0], &[1], &[0x8000], &[0x1234_8765], &[0xFFFF_FFFF]];
    let before = results(&module, inputs);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IndVars { lftr: true, size: false });
    manager.run_module(&mut module, std::rc::Rc::new(M68kMachine { cpu })).unwrap();
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    after
}

/// Symptom: crc16's bit loop stayed `phi i32` / `shl i32` / `xor i32` on the
/// 68000 (8 clk an operation, `.l`), so no `add.w; bcc` loop.
#[test]
fn a_cycle_read_through_a_word_mask_is_a_word_on_the_68000() {
    let after = run(&crc_loop(MASKED, "xor i32 %s, 4129"), Cpu::M68000);
    assert!(after.contains("phi i16"), "{after}");
    assert!(after.contains("shl i16"), "{after}");
    assert!(after.contains("xor i16"), "{after}");
    assert!(!after.contains("phi i32 [ %n, %entry ]"), "{after}");
}

/// A read of bits above 15 (the full value returned) needs the long.
#[test]
fn a_cycle_read_whole_stays_a_long() {
    let after = run(&crc_loop("ret i32 %out", "xor i32 %s, 4129"), Cpu::M68000);
    assert!(!after.contains("phi i16"), "{after}");
}

/// A `xor` with a constant above 16 bits changes bits the mask drops, so the
/// mask still allows the word: the word's low bits do not depend on them.
#[test]
fn a_constant_above_the_width_is_dropped_with_the_high_bits() {
    let after = run(&crc_loop(MASKED, "xor i32 %s, 268439585"), Cpu::M68000);
    assert!(after.contains("phi i16"), "{after}");
}

/// Symptom: on the 68020 and 68060 crc16's loop stayed long (`move.l; add.l;
/// tst.w; bge`, +22 and +17 clk against the best of gcc/clang) because a word
/// `add` costs what a long does: the win is the carry reading the top bit, so
/// the copy and the `tst` go.
#[test]
fn a_doubling_whose_top_bit_is_tested_is_a_word_where_a_word_add_costs_the_same() {
    for cpu in [Cpu::M68020, Cpu::M68060] {
        let after = run(&crc_loop(MASKED, "xor i32 %s, 4129"), cpu);
        assert!(after.contains("phi i16"), "{cpu:?}\n{after}");
    }
}

/// Symptom: stft_single/stft_fixed -Os grew 8 B at 68000 (+6 at -O1): a bit
/// loop was carried in a byte, saving a few clocks a trip, and each wide read
/// of it after the loop became `moveq #0; move.b`. The extensions are costed
/// against the saving, so a cycle read wide at the exits stays a long.
#[test]
fn a_cycle_read_wide_after_the_loop_stays_a_long_when_extensions_eat_the_saving() {
    let text = format!(
        "{LAYOUT}define i32 @f(i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %r = phi i32 [ %n, %entry ], [ %m, %loop ]
  %s = shl i32 %r, 1
  %o = or i32 %s, %i
  %m = and i32 %o, 255
  %next = add i32 %i, 1
  %go = icmp ult i32 %next, 8
  br i1 %go, label %loop, label %exit
exit:
  %out = phi i32 [ %m, %loop ]
  %x = mul i32 %out, %n
  %y = add i32 %x, %out
  %z = xor i32 %y, %out
  %w = sub i32 %z, %out
  ret i32 %w
}}
"
    );
    let after = run(&text, Cpu::M68000);
    assert!(!after.contains("phi i8"), "{after}");
}

/// Symptom: stft_single/stft_fixed still grew 2 B at 68000 after the exits
/// were costed: the bit-reversal loop's `x & 1` became `move.l %d3, %d4;
/// and.b #1, %d4` (6 B) from `moveq #1, %d4; and.l %d3, %d4` (4 B), because
/// `x` lives on and an immediate form overwrites its operand.
#[test]
fn a_cycle_whose_immediate_ands_copy_a_live_operand_stays_a_long() {
    let text = format!(
        "{LAYOUT}define i32 @f(i32 %n) {{
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %r = phi i32 [ 0, %entry ], [ %m, %loop ]
  %x = phi i32 [ %n, %entry ], [ %y, %loop ]
  %s = shl i32 %r, 1
  %b = and i32 %x, 1
  %o = or i32 %s, %b
  %m = and i32 %o, 255
  %y = lshr i32 %x, 1
  %next = add i32 %i, 1
  %go = icmp ult i32 %next, 8
  br i1 %go, label %loop, label %exit
exit:
  %out = phi i32 [ %m, %loop ]
  %v = and i32 %out, 255
  ret i32 %v
}}
"
    );
    let after = run(&text, Cpu::M68000);
    assert!(!after.contains("phi i8"), "{after}");
}
