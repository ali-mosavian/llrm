//! Where an argument travels. A function private to its module (`fastcc`) takes up to 18 bytes in `a`, `x`, `rc2`..`rc7`,
//! `rc16`..`rc25`; one other modules can call keeps 8 bytes in registers and the rest on the soft stack, where the runtime's
//! members read theirs (`calling.toml`: `[mos]` and `[mos_private]`).

use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{self, Level, Request};
use llrm_6502::target::Mos;
use llrm_target::Target;

fn at(
    level: Level,
    ir: &str,
) -> String {
    let text = format!("target datalayout = \"{}\"\n{ir}", Mos.layout().datalayout);
    let mut request = Request::new(level);
    request.cpu = Cpu::Mos6502;
    driver::assembly(&text, &request).unwrap_or_else(|why| panic!("{level:?}: {why}"))
}

/// The lines of the function `name`.
fn function<'a>(
    text: &'a str,
    name: &str,
) -> &'a str {
    let from = text.find(&format!("\n{name}:\n")).unwrap_or_else(|| panic!("no {name} in\n{text}"));
    let rest = &text[from + 1..];
    let end = rest.find("\n.segment").unwrap_or(rest.len());
    &rest[..end]
}

const PRIVATE: &str = "define internal fastcc i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d) noinline {
  %1 = mul i32 %b, 3
  %2 = add i32 %a, %1
  %3 = mul i32 %c, 5
  %4 = add i32 %2, %3
  %5 = mul i32 %d, 7
  %6 = add i32 %4, %5
  ret i32 %6
}

define internal fastcc i32 @call4(i32 %a, i32 %b, i32 %c) noinline {
  %d = add i32 %a, %b
  %r = call fastcc i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d)
  %s = add i32 %r, %c
  ret i32 %s
}

define internal fastcc i32 @same(i32 %a, i32 %b, i32 %c, i32 %d) noinline {
  %r = tail call fastcc i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d)
  ret i32 %r
}

define i32 @entry(i32 %a) {
  %b = add i32 %a, 1
  %c = add i32 %a, 2
  %r = call fastcc i32 @call4(i32 %a, i32 %b, i32 %c)
  %s = call fastcc i32 @same(i32 %a, i32 %r, i32 3, i32 4)
  ret i32 %s
}
";

const EXTERNAL: &str = "define i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d) noinline {
  %1 = mul i32 %b, 3
  %2 = add i32 %a, %1
  %3 = add i32 %c, %d
  %4 = add i32 %2, %3
  ret i32 %4
}

define i32 @call4(i32 %a, i32 %b, i32 %c) {
  %d = add i32 %a, %b
  %r = call i32 @add4(i32 %a, i32 %b, i32 %c, i32 %d)
  ret i32 %r
}
";

/// frameless (+235 B against llvm-mos at O1) and tail_calls (+190 B): the 16 bytes of `add4(a, b, c, d)` were 8 in
/// registers and 8 stored through `(rc0),y` at the call and loaded back by the callee, a frame at every call.
#[test]
fn a_private_function_takes_sixteen_argument_bytes_in_registers() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = at(level, PRIVATE);
        for name in ["add4", "call4", "same", "entry"] {
            let body = function(&text, name);
            assert!(!body.contains("(rc0),y"), "{level:?} {name} reads or writes the soft stack\n{body}");
        }
        assert!(text.contains("rc16"), "{level:?}: the third argument is in rc16..\n{text}");
    }
}

/// tail_calls `same`: `jsr __outl_1; jsr __outl_4; jsr add3; jmp __outl_2` (12 B) for a call that is `jmp add3`.
#[test]
fn a_sibling_call_with_more_than_eight_argument_bytes_is_a_jump() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = at(level, PRIVATE);
        let body = function(&text, "same");
        assert!(body.contains("jmp add4"), "{level:?}\n{body}");
        assert!(!body.contains("jsr add4"), "{level:?}\n{body}");
    }
}

/// Negative: a function other modules can call keeps the placement the runtime's members read: 8 bytes in registers, the
/// rest on the soft stack.
#[test]
fn an_external_function_keeps_its_arguments_beyond_eight_bytes_on_the_stack() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = at(level, EXTERNAL);
        let caller = function(&text, "call4");
        assert!(caller.contains("),y"), "{level:?}: the call stores its stack arguments\n{caller}");
    }
}

/// `text` with the cells each callee is passed (`Asm::cell_callees`); a callee not listed is unknown.
fn parsed(
    text: &str,
    callees: &[(&str, &[u8])],
) -> llrm_6502::asm::Asm {
    let mut asm = llrm_6502::asm::Asm::parse(text);
    for (callee, cells) in callees {
        asm.cell_callees.insert((*callee).to_owned(), cells.iter().copied().collect());
    }
    asm
}

fn dead_stores(
    text: &str,
    callees: &[(&str, &[u8])],
) -> String {
    let mut asm = parsed(text, callees);
    llrm_6502::peephole_cells::drop_dead_stores(&mut asm);
    asm.text()
}

fn spill_copies(
    text: &str,
    callees: &[(&str, &[u8])],
) -> String {
    let mut asm = parsed(text, callees);
    llrm_6502::peephole_cells::fold_spill_copy(&mut asm);
    asm.text()
}

/// The first A/B of the private convention: call_keeps, frameless, tail_calls, recdeep and recmany_o1 printed wrong values on
/// sim65. `drop_dead_stores` and `fold_spill_copy` took the stores of the ninth to eighteenth argument bytes (`rc16`..) for
/// dead before a `jsr`, which they clobber and, they thought, read none of.
#[test]
fn a_call_of_a_program_function_reads_the_scratch_cells_its_arguments_are_in() {
    let call = "  lda #3\n  sta rc16\n  jsr f\n  rts\n";
    assert_eq!(dead_stores(call, &[("f", &[16])]), call, "f is passed rc16");
    assert_eq!(dead_stores(call, &[]), call, "a callee nothing says is passed none may read it");
    // f is passed no scratch cell: the store is as dead as before
    assert_eq!(dead_stores(call, &[("f", &[])]), "  lda #3\n  jsr f\n  rts\n");
    // a runtime routine whose text names no rc16 reads none
    assert_eq!(dead_stores("  lda #3\n  sta rc16\n  jsr __mulhi3\n  rts\n", &[]), "  lda #3\n  jsr __mulhi3\n  rts\n");
}

/// The same for `fold_spill_copy`, which dropped `lda m; sta rc16` before the call and read `m` where the callee read `rc16`.
#[test]
fn a_copy_into_an_argument_cell_stays_before_the_call_that_reads_it() {
    let copy = "  ldy rc8\n  sty rc16\n  lda rc9\n  adc rc16\n  jsr f\n  rts\n";
    assert_eq!(spill_copies(copy, &[("f", &[16])]), copy);
}

/// A platform with fewer than 26 zero-page cells has no rc16..rc25 for the arguments: refused, not a call into other memory.
#[test]
fn a_platform_without_the_argument_cells_is_refused() {
    let mut request = Request::new(Level::O1);
    request.cpu = Cpu::Mos6502;
    request.platform.zero_page = 16;
    let text = format!("target datalayout = \"{}\"\ndefine i16 @main() {{ ret i16 0 }}\n", Mos.layout().datalayout);
    let why = driver::assembly(&text, &request).expect_err("refused");
    assert!(why.contains("zero_page = 16"), "{why}");
}
