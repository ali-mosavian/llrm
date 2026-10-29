//! IR to assembly text, against the pin's `llc -mtriple=m68k-linux-gnu
//! -mcpu=M68000 -relocation-model=static -O2` (directives and comments
//! dropped). The oracle is `~/scratch/llvm-build-m68k/bin/llc`, LLVM
//! d1106deb71cc built with the M68k target; not the system llc.
//!
//! The call tests failed before the reserved-copy fold (`move.l %sp, %d0` and
//! a store through `(%d0)`) and before register-class constraints (the base
//! in a data register).

use llrm_m68k::compile::{Options, text, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn compiled(
    body: &str,
    name: &str,
) -> String {
    let text_in = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let module = llrm_mir::parse::module(&text_in).expect("parses");
    text_with(&module, name, Cpu::M68000, Options::PINNED).expect("compiles")
}

#[test]
fn an_add_of_two_arguments_prints_as_llc_does() {
    let ir = "define i32 @f(i32 %a, i32 %b) {\n  %r = add i32 %a, %b\n  ret i32 %r\n}";
    assert_eq!(compiled(ir, "f"), "f:\n\tmove.l\t(4,%sp), %d0\n\tadd.l\t(8,%sp), %d0\n\trts\n");
}

const CALLS: &str = "declare i32 @g(i32, i8)\ndeclare ptr @h(ptr)\n\
define i32 @c(i32 %a, i8 %b) {\n  %r = call i32 @g(i32 %a, i8 %b)\n  ret i32 %r\n}\n\
define ptr @p(ptr %x) {\n  %r = call ptr @h(ptr %x)\n  ret ptr %r\n}";

#[test]
fn a_call_with_a_byte_argument_prints_as_llc_does() {
    let expected = "c:\n\tsuba.l\t#12, %sp\n\tmoveq\t#0, %d0\n\tmove.b\t(23,%sp), %d0\n\tmove.l\t%d0, (4,%sp)\n\
                    \tmove.l\t(16,%sp), %d0\n\tmove.l\t%d0, (%sp)\n\tjsr\tg\n\tadda.l\t#12, %sp\n\trts\n";
    assert_eq!(compiled(CALLS, "c"), expected);
}

#[test]
fn a_pointer_call_prints_as_llc_does() {
    let expected =
        "p:\n\tsuba.l\t#4, %sp\n\tmove.l\t(8,%sp), %d0\n\tmove.l\t%d0, (%sp)\n\tjsr\th\n\tadda.l\t#4, %sp\n\trts\n";
    assert_eq!(compiled(CALLS, "p"), expected);
}

const SUM: &str = "define i32 @sum(i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [0, %entry], [%i1, %loop]\n  %s = phi i32 [0, %entry], [%s1, %loop]\n  %s1 = add i32 %s, %i\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %n\n  br i1 %c, label %loop, label %exit\nexit:\n  ret i32 %s1\n}";

/// The comparison `llc` and a different allocator can meet on: registers a
/// register-to-register move joins are one name, such moves are dropped, the
/// names are numbered by first appearance, and a `movem` shows only how many
/// registers it moves.
fn modulo_names_and_copies(text: &str) -> Vec<String> {
    fn registers(line: &str) -> Vec<String> {
        line.split(|c: char| !c.is_alphanumeric() && c != '%')
            .filter(|w| w.starts_with("%d"))
            .map(str::to_owned)
            .collect()
    }
    let mut joined: Vec<Vec<String>> = Vec::new();
    for line in text.lines() {
        if let Some(operands) = line.trim_start().strip_prefix("move.l\t")
            && operands.split(", ").all(|one| one.starts_with("%d"))
        {
            let [from, to] = [0, 1].map(|at| operands.split(", ").nth(at).unwrap().to_owned());
            let (a, b) =
                (joined.iter().position(|set| set.contains(&from)), joined.iter().position(|set| set.contains(&to)));
            match (a, b) {
                (Some(a), Some(b)) if a != b => {
                    let moved = joined.remove(a.max(b));
                    joined[a.min(b)].extend(moved);
                }
                (Some(a), None) => joined[a].push(to),
                (None, Some(b)) => joined[b].push(from),
                (None, None) => joined.push(vec![from, to]),
                _ => {}
            }
        }
    }
    let mut numbered: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let bare = line.trim_start();
        if let Some(operands) = bare.strip_prefix("move.l\t")
            && operands.split(", ").all(|one| one.starts_with("%d"))
        {
            continue;
        }
        if bare.starts_with("movem.l") {
            out.push(format!("{} <{} registers>", bare.split('\t').next().unwrap(), registers(line).len()));
            continue;
        }
        let mut renamed = line.to_owned();
        for register in registers(line) {
            let name = joined.iter().find(|set| set.contains(&register)).map_or(register.clone(), |set| set[0].clone());
            if !numbered.contains(&name) {
                numbered.push(name.clone());
            }
            let at = numbered.iter().position(|one| *one == name).unwrap();
            renamed = renamed.replace(&register, &format!("%R{at}"));
        }
        out.push(renamed.replace("%R", "%r"));
    }
    out
}

/// `llc`'s loop, with a callee-saved `d2` kept across it. The allocators
/// differ, so the registers are renamed and the copies between them dropped.
#[test]
fn a_counted_loop_is_equal_to_llc_modulo_register_names_and_copies() {
    let llc = "sum:\n\tsuba.l\t#4, %sp\n\tmovem.l\t%d2, (0,%sp)\n\tmoveq\t#0, %d0\n\tmove.l\t(8,%sp), %d1\n\tmove.l\t%d0, %d2\n\
               .LBB0_1:\n\tadd.l\t%d2, %d0\n\tadd.l\t#1, %d2\n\tcmp.l\t%d1, %d2\n\tblt\t.LBB0_1\n\
               \tmovem.l\t(0,%sp), %d2\n\tadda.l\t#4, %sp\n\trts\n";
    assert_eq!(modulo_names_and_copies(&compiled(SUM, "sum")), modulo_names_and_copies(llc));
}

/// The instrument: a different immediate, operand order or branch must not be
/// hidden by the renaming and the dropped copies.
#[test]
fn the_loop_comparison_sees_a_changed_instruction() {
    let base = "f:\n\tmoveq\t#0, %d0\n.LBB0_1:\n\tadd.l\t#1, %d0\n\tcmp.l\t%d1, %d0\n\tblt\t.LBB0_1\n\trts\n";
    for changed in [
        base.replace("#1", "#2"),
        base.replace("cmp.l\t%d1, %d0", "cmp.l\t%d0, %d1"),
        base.replace("blt", "bgt"),
        base.replace("add.l", "sub.l"),
    ] {
        assert_ne!(modulo_names_and_copies(&changed), modulo_names_and_copies(base), "{changed}");
    }
}

/// Constant and variable shifts print as `llc` does. Every shift was
/// `CannotSelect`: the legalizer cut the amount to `i8`, and m68k's amount has
/// the type of the value shifted (`getScalarShiftAmountTy`).
#[test]
fn shifts_print_as_llc_does() {
    let shift = |op: &str| compiled(&format!("define i32 @s(i32 %x) {{\n  %r = {op}\n  ret i32 %r\n}}"), "s");
    let load = "s:\n\tmove.l\t(4,%sp), %d0\n";
    assert_eq!(shift("shl i32 %x, 2"), format!("{load}\tlsl.l\t#2, %d0\n\trts\n"));
    assert_eq!(shift("lshr i32 %x, 1"), format!("{load}\tlsr.l\t#1, %d0\n\trts\n"));
    assert_eq!(shift("ashr i32 %x, 3"), format!("{load}\tasr.l\t#3, %d0\n\trts\n"));
    assert_eq!(shift("shl i32 %x, 9"), "s:\n\tmoveq\t#9, %d1\n\tmove.l\t(4,%sp), %d0\n\tlsl.l\t%d1, %d0\n\trts\n");
}

/// A select is a diamond: the `CMOV` pseudo of the pin, expanded as
/// `EmitLoweredSelect` does. Text equal to `llc`, labels aside.
#[test]
fn selects_print_as_llc_does() {
    let max = "define i32 @m(i32 %a, i32 %b) {\n  %c = icmp sgt i32 %a, %b\n  %r = select i1 %c, i32 %a, i32 %b\n  ret i32 %r\n}";
    assert_eq!(
        compiled(max, "m"),
        "m:\n\tmove.l\t(4,%sp), %d0\n\tmove.l\t(8,%sp), %d1\n\tcmp.l\t%d1, %d0\n\tbgt\t.LBB0_2\n\tmove.l\t%d1, %d0\n.LBB0_2:\n\trts\n"
    );
}

/// A load stored at once is one `move.b (src), (dst)` as `llc` has it
/// (`isSafeStoreLoad`). It was never folded: two moves through `%d2`, which
/// also cost a `movem` and a frame.
#[test]
fn a_loaded_byte_stored_at_once_is_one_move() {
    let ir = "define void @c(ptr %d, ptr %s) {\n  %v = load i8, ptr %s\n  store i8 %v, ptr %d\n  ret void\n}";
    assert_eq!(
        compiled(ir, "c"),
        "c:\n\tmove.l\t(8,%sp), %a0\n\tmove.l\t(4,%sp), %a1\n\tmove.b\t(%a0), (%a1)\n\trts\n"
    );
}

/// `x2 = select (x > y), x - y, x` computed both subtractions before the
/// branch, so both results stayed live and took the callee-saved `d2`/`d3`
/// (a `movem` pair, 58 B). `MachineSink` moves each into the arm that uses it.
#[test]
fn gcd_subtractions_sink_into_their_arms() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source("gcd")).expect("parses");
    let text = text(&module, "k", Cpu::M68000).expect("compiles");
    assert!(!text.contains("movem"), "{text}");
    let after_compare = text.find("ble").expect("the signed compare");
    assert!(text.matches("sub.l").count() == 2 && text.find("sub.l").unwrap() > after_compare, "{text}");
}

/// The loop's join was a block of its own that only jumped back (`bra L5` to
/// `L5: bra L1`), a jump per trip and 4 B. `BranchFolder` skips it.
#[test]
fn a_loop_does_not_jump_through_a_block_that_only_jumps() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source("gcd")).expect("parses");
    let text = text(&module, "k", Cpu::M68000).expect("compiles");
    assert!(!text.contains(":\n\tbra"), "{text}");
}

/// Twenty live incoming arguments in a loop: each is a load of its slot,
/// which runs again where the value is read instead of being stored to a
/// frame slot and reloaded. It stored 18 of them (`movem.l %d0, (N,%sp)`).
#[test]
fn spilled_incoming_arguments_are_reloaded_from_their_own_slots() {
    let params: Vec<String> = (0..20).map(|k| format!("i32 %a{k}")).collect();
    let mut ir = format!("define i32 @p({}) {{\nentry:\n  br label %l\nl:\n", params.join(", "));
    ir.push_str("  %i = phi i32 [0, %entry], [%i1, %l]\n  %s = phi i32 [0, %entry], [%s1, %l]\n");
    let mut sum = "%s".to_owned();
    for k in 0..20 {
        ir.push_str(&format!("  %m{k} = xor i32 %a{k}, %i\n  %t{k} = add i32 {sum}, %m{k}\n"));
        sum = format!("%t{k}");
    }
    ir.push_str(&format!("  %s1 = add i32 {sum}, 1\n  %i1 = add i32 %i, 1\n  %c = icmp slt i32 %i1, %a0\n"));
    ir.push_str("  br i1 %c, label %l, label %x\nx:\n  ret i32 %s1\n}\n");
    let asm = compiled(&ir, "p");
    let stores = asm.lines().filter(|line| line.starts_with("\tmovem.l\t%d") && !line.contains('-')).count();
    assert!(stores <= 1, "{asm}");
}
