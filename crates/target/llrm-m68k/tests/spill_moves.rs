//! A value kept across a call in a stack slot was spilled with `movem.l %d0,
//! (d,%sp)` (6 B) and reloaded with `movem.l (d,%sp), %d3` (6 B) where `move.l`
//! is 4 B: the pin keeps the codes with `movem`, and the codes are dead at
//! nearly every spill (select_of_constants: 812 spills).

use llrm_m68k::compile::{Options, items_with, text_with};
use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::object::{executable, start, write_executable};

mod support;

/// `f() = sink(g(1), ..., g(count))`, with `g(a) = (a ^ 5) + a`: every result
/// is live until `sink` sums them.
fn module(count: i32) -> llrm_mir::module::Module {
    let calls: String = (1..=count).map(|n| format!("  %c{n} = call i32 @g(i32 {n})\n")).collect();
    let params: Vec<String> = (1..=count).map(|n| format!("i32 %p{n}")).collect();
    let args: Vec<String> = (1..=count).map(|n| format!("i32 %c{n}")).collect();
    let mut sums = String::new();
    let mut last = "%p1".to_owned();
    for n in 2..=count {
        sums += &format!("  %s{n} = add i32 {last}, %p{n}\n");
        last = format!("%s{n}");
    }
    let ir = format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
define internal i32 @g(i32 %a) #0 {{
  %x = xor i32 %a, 5
  %r = add i32 %x, %a
  ret i32 %r
}}
define internal i32 @sink({}) #0 {{
{sums}  ret i32 {last}
}}
define i32 @f() {{
{calls}  %r = call i32 @sink({})
  ret i32 %r
}}
attributes #0 = {{ noinline }}",
        params.join(", "),
        args.join(", ")
    );
    llrm_mir::parse::module(&ir).expect("parses")
}

fn options() -> Options {
    Options { size: true, ..Options::default() }
}

#[test]
fn spills_are_moves_where_the_codes_are_dead() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = text_with(&module(16), "f", cpu, options()).expect("compiles");
        let single =
            text.lines().filter(|line| line.contains("movem.l") && !line.contains('/') && !line.contains('-')).count();
        let moves = text.lines().filter(|line| line.contains("move.l\t%d") && line.contains(",%sp)")).count();
        assert!(moves > 0 && single == 0, "{cpu:?}: {single} single-register movem, {moves} moves:\n{text}");
    }
}

/// The sum of `(a ^ 5) + a` for 1..=16 is 272, 16 as an exit code.
#[test]
fn a_function_with_move_spills_still_adds_up() {
    let Some(qemu) = support::qemu() else { return };
    let mut all = start("f", &[]);
    for name in ["f", "g", "sink"] {
        all.extend(items_with(&module(16), name, Cpu::M68000, options()).expect("compiles"));
    }
    let path = support::unique_path("spill_moves");
    write_executable(&path, &executable(&all).unwrap()).unwrap();
    let code = support::run(&qemu, &[], &path);
    std::fs::remove_file(&path).ok();
    assert_eq!(code, Some(16));
}
