//! IR to assembly text, against the pin's `llc -mtriple=m68k-linux-gnu
//! -mcpu=M68000 -relocation-model=static -O2` (directives and comments
//! dropped). The oracle is `$LLVM_M68K/llc`, LLVM
//! d1106deb71cc built with the M68k target; not the system llc.
//!
//! The call tests failed before the reserved-copy fold (`move.l %sp, %d0` and
//! a store through `(%d0)`) and before register-class constraints (the base
//! in a data register).

use llrm_m68k::compile::{Options, text_with};
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

/// `llc`'s loop, less its frame: it keeps `n` in the callee-saved `d2`, which
/// costs `suba`, two `movem`s and `adda`; the loop here holds `n` in `%a0`
/// (`cmp.l An, Dn` exists), a register a call clobbers anyway. The allocators
/// differ, so the data registers are renamed and the copies between them
/// dropped.
#[test]
fn a_counted_loop_is_llcs_without_the_callee_saved_frame() {
    let llc = "sum:\n\tmoveq\t#0, %d0\n\tmove.l\t(4,%sp), %a0\n\tmove.l\t%d0, %d2\n\
               .LBB0_1:\n\tadd.l\t%d2, %d0\n\tadd.l\t#1, %d2\n\tcmp.l\t%a0, %d2\n\tblt\t.LBB0_1\n\trts\n";
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
    let text =
        text_with(&module, "k", Cpu::M68000, Options { mid_end: false, ..Options::default() }).expect("compiles");
    assert!(!text.contains("movem"), "{text}");
    // block placement may lay an arm above the compare, so the sinking shows in
    // the count and the frame
    assert_eq!(text.matches("sub.l").count(), 2, "{text}");
}

/// The loop's join was a block of its own that only jumped back (`bra L5` to
/// `L5: bra L1`), a jump per trip and 4 B. `BranchFolder` skips it.
#[test]
fn a_loop_does_not_jump_through_a_block_that_only_jumps() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source("gcd")).expect("parses");
    let text =
        text_with(&module, "k", Cpu::M68000, Options { mid_end: false, ..Options::default() }).expect("compiles");
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

/// A constant held in an address register is `move.w #1, %a0` (`ExpandMOVI`),
/// not `moveq #1, %a0`, which has no encoding: `fib` returned 0 when the
/// allocator first gave its constant an address register.
#[test]
fn a_constant_in_an_address_register_is_not_a_moveq() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source("fib")).expect("parses");
    let asm = text_with(&module, "k", Cpu::M68000, Options::PINNED).expect("compiles");
    assert!(asm.contains("%a0") && !asm.lines().any(|l| l.contains("moveq") && l.contains("%a")), "{asm}");
    assert!(asm.contains("move.w\t#1, %a0"), "{asm}");
}

/// A pointer also read as an integer sat in one data register and was the
/// base of the load: `add.l (%d0), %d0`, not an instruction at all. The base
/// now comes from a copy into an address register.
#[test]
fn a_pointer_used_as_an_integer_and_as_a_base_gets_an_address_register() {
    let ir = "define i32 @k(ptr %p) {\n  %v = load i32, ptr %p\n  %i = ptrtoint ptr %p to i32\n  %r = add i32 %v, %i\n  ret i32 %r\n}";
    assert_eq!(compiled(ir, "k"), "k:\n\tmove.l\t(4,%sp), %d0\n\tmove.l\t%d0, %a0\n\tadd.l\t(%a0), %d0\n\trts\n");
}

/// Loop strength reduction prices addressing modes through the target's
/// `Tti`. With the m68k one the array walk gets a pointer induction variable
/// (a `phi ptr`, an `(An)+` access); with a scaled index (the 68020's, and
/// x86's) it stays indexed.
#[test]
fn the_m68k_tti_strength_reduces_an_array_walk_and_a_scaled_index_does_not() {
    let ir = "define i32 @k(ptr %p, i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%i1, %l]\n  %s = phi i32 [0, %entry], [%s1, %l]\n  %q = getelementptr i32, ptr %p, i32 %i\n  %v = load i32, ptr %q\n  %s1 = add i32 %s, %v\n  %i1 = add i32 %i, 1\n  %e = icmp slt i32 %i1, %n\n  br i1 %e, label %l, label %x\nx:\n  ret i32 %s1\n}";
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{ir}")).unwrap();
    let ours = llrm_mir::print::module(&llrm_m68k::compile::mid_end(&module).unwrap());
    let default = llrm_mir::print::module(
        &llrm_m68k::compile::mid_end_described_by(
            &module,
            &llrm_m68k::machine::M68k { scaled_index: true, ..Default::default() },
            None,
        )
        .unwrap(),
    );
    assert!(ours.contains("phi ptr"), "{ours}");
    assert!(!default.contains("phi ptr"), "{default}");
}

/// A guard that returns early over a loop branches forward to the cold
/// return, laid out after the loop. It used to fall into it and jump to the
/// loop: a taken `bra` before the loop on every call (`strcmp`: `bne; moveq;
/// bra`).
#[test]
fn a_guard_over_a_loop_branches_forward_to_a_cold_return() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source_in("library", "strcmp")).expect("parses");
    let text = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    let before_loop = &text[..text.find(".LBB0_2:").expect("the loop")];
    assert!(before_loop.contains("\tbeq\t.LBB0_6"), "{text}");
    assert!(!before_loop.contains("bra"), "{text}");
}

/// A `select` expanded to a diamond has equal odds on its arms, so the arm
/// that falls through stays in place (`EmitLoweredSelect` adds it first).
/// With the successors in the taken-first order the arms swapped, and `hex`
/// ran 4 instructions more (66 to 70).
#[test]
fn the_arms_of_an_expanded_select_keep_their_order() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source_in("library", "hex")).expect("parses");
    let text = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    assert!(text.contains("\tcmpi.l\t#10, %d") && text.contains("\tbcc\t.LBB0_3\n"), "{text}");
}

/// The ILP scheduler put the loop-carried copy between `hex`'s `cmpi` and
/// `bhi`; the copy is a `move.l`, which sets the flags, so they were saved
/// around it (`move.w %sr,-(%sp)` ... `move.w (%sp)+,%ccr`): 33 instructions
/// where the source order has 31.
#[test]
fn a_copy_is_not_scheduled_between_a_compare_and_its_branch() {
    let module = llrm_mir::parse::module(&llrm_m68k::kernels::source_in("library", "hex")).expect("parses");
    let text = text_with(&module, "k", Cpu::M68000, Options::default()).expect("compiles");
    assert!(!text.contains("%sr") && !text.contains("%ccr"), "{text}");
}

/// A stored `zext i8 to i32` of a load matched the store-load fragment, which
/// checks no extension (the pin's too: `llc` gives `move.l (buf,%pc), (%a0)`),
/// and copied four bytes where one was meant. Seed 68 of the differential
/// driver.
#[test]
fn a_stored_zero_extended_byte_is_not_a_long_move() {
    let ir = "@buf = external constant [16 x i8], align 1\ndefine void @f(ptr %p) {\n  %v = load i8, ptr @buf, align 1\n  %e = zext i8 %v to i32\n  store i32 %e, ptr %p, align 2\n  ret void\n}";
    let text = compiled(ir, "f");
    assert!(!text.contains("move.l\t(buf,%pc), ("), "{text}");
}
