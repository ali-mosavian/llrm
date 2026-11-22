//! Repeated instruction sequences called as routines (`outline`, the pin's
//! `MachineOutliner`): the program does the same, in fewer bytes.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::outline::{run, run_at, run_priced};

fn bytes(text: &str) -> u32 {
    llrm_6502::timing::estimate(Cpu::Mos6502, text).expect("prices").bytes
}

/// A 32-bit add of `rc20..rc23` into `rc16..rc19`.
const ADD: &str = "  clc\n  lda rc16\n  adc rc20\n  sta rc16\n  lda rc17\n  adc rc21\n  sta rc17\n  lda rc18\n  adc rc22\n  sta rc18\n  lda rc19\n  adc rc23\n  sta rc19\n";

fn probe(body: &str) -> String {
    format!(".export probe\nprobe:\n{body}  rts\n")
}

/// Two copies of a 32-bit add become one routine and two calls: fewer bytes,
/// the same answer, with the carry of the last `adc` still set when the call
/// returns (a `jsr` and its `rts` change no flag).
#[test]
fn two_identical_sequences_become_one_routine_and_the_program_computes_the_same() {
    let (a, b): (u32, u32) = (0x0403_0201, 0x0706_05FA);
    let first = a.wrapping_add(b);
    let second = first.wrapping_add(b);
    let carry = u8::from(u64::from(first) + u64::from(b) > u64::from(u32::MAX));
    let want = (second as u8) ^ ((second >> 8) as u8) ^ ((second >> 16) as u8) ^ ((second >> 24) as u8) ^ carry;
    let set = |value: u32, base: u8| {
        (0..4).map(|i| format!("  lda #{}\n  sta rc{}\n", (value >> (8 * i)) as u8, base + i)).collect::<String>()
    };
    // the carry out of each add is read by the code after it
    let body = format!(
        "{}{}{ADD}  lda #0\n  adc #0\n  sta rc24\n{ADD}  lda #0\n  adc #0\n  sta rc25\n  lda rc16\n  eor rc17\n  eor rc18\n  eor rc19\n  eor rc25\n",
        set(a, 16),
        set(b, 20)
    );
    let text = probe(&body);
    let outlined = run(&text);
    assert!(outlined.contains("jsr __outl_0"), "{outlined}");
    assert!(bytes(&outlined) < bytes(&text), "{} -> {} B:\n{outlined}", bytes(&text), bytes(&outlined));
    for (name, code) in [("plain", &text), ("outlined", &outlined)] {
        assert_eq!(
            common::run_with(&format!("outline-add-{name}"), "  jsr probe\n", code),
            i32::from(want),
            "{name}:\n{code}"
        );
    }
}

/// A sequence that ends the function is the tail form: a `jmp`, and the body
/// is the sequence with its own `rts`.
#[test]
fn a_sequence_that_ends_the_function_is_a_jump() {
    let tail =
        "  lda rc16\n  clc\n  adc rc17\n  sta rc16\n  lda rc18\n  adc rc19\n  tax\n  lda rc16\n  eor #$55\n  rts\n";
    let text = format!(
        ".export one\none:\n  lda #1\n  sta rc16\n  lda #2\n  sta rc17\n  lda #3\n  sta rc18\n  lda #4\n  sta rc19\n{tail}\
         .export two\ntwo:\n  lda #9\n  sta rc16\n  lda #8\n  sta rc17\n  lda #7\n  sta rc18\n  lda #6\n  sta rc19\n{tail}"
    );
    let outlined = run(&text);
    assert!(outlined.contains("jmp __outl_0") && outlined.matches("jmp __outl_0").count() == 2, "{outlined}");
    assert!(bytes(&outlined) < bytes(&text));
    for (name, code) in [("plain", &text), ("outlined", &outlined)] {
        // one: (1+2) ^ 0x55 in a; two: (9+8) ^ 0x55
        let call = "  jsr one\n  sta rc25\n  jsr two\n  eor rc25\n";
        let want = ((1u8 + 2) ^ 0x55) ^ ((9u8 + 8) ^ 0x55);
        assert_eq!(common::run_with(&format!("outline-tail-{name}"), call, code), i32::from(want), "{name}:\n{code}");
    }
}

/// A stack instruction changes what the `jsr` left on the stack: a `pha` whose
/// `pla` is in the caller would be popped as half of the return address. The
/// sequences are outlined up to it, and the `pha` stays where it was.
#[test]
fn a_stack_instruction_stays_out_of_the_body() {
    let push = "  lda rc16\n  clc\n  adc rc17\n  sta rc18\n  lda rc18\n  eor #$0F\n  asl a\n  adc #1\n  sta rc19\n  lda rc19\n  pha\n";
    let body = format!(
        "  lda #3\n  sta rc16\n  lda #4\n  sta rc17\n{push}  pla\n  sta rc20\n{push}  pla\n  eor rc20\n  ora #$40\n"
    );
    let text = probe(&body);
    let outlined = run(&text);
    let (caller, bodies) = outlined.split_once("__outl_0:").expect("the arithmetic before the push is outlined");
    assert!(!bodies.contains("pha") && !bodies.contains("pla"), "{outlined}");
    assert_eq!(caller.matches("pha").count(), 2, "{outlined}");
    for (name, code) in [("plain", &text), ("outlined", &outlined)] {
        assert_eq!(common::run_with(&format!("outline-stack-{name}"), "  jsr probe\n", code), 0x40, "{name}:\n{code}");
    }
}

/// A call to a function of the program may recurse: two more bytes of the
/// return stack at each level. A call to a runtime routine may be in a body;
/// `__csave` is not (it keeps a frame on the stack). The arithmetic around a
/// refused call is still outlined, and the call stays in the caller.
#[test]
fn calls_in_a_body_are_runtime_routines_other_than_the_frame_ones() {
    let seq = |callee: &str| {
        format!(
            "  lda rc16\n  sta rc2\n  lda rc17\n  sta rc3\n  lda rc18\n  sta rc4\n  lda rc19\n  sta rc5\n  jsr {callee}\n  sta rc20\n  stx rc21\n"
        )
    };
    for (callee, in_body) in [("__mulsi3", true), ("some_function", false), ("__csave", false)] {
        let text = probe(&format!("{}{}", seq(callee), seq(callee)));
        let outlined = run(&text);
        let bodies = outlined.split_once("__outl_0:").map_or("", |(_, bodies)| bodies);
        assert_eq!(bodies.contains(&format!("jsr {callee}")), in_body, "{callee}:\n{outlined}");
    }
}

/// The same function twice: each compiles to the same text, and the module is
/// smaller with one body called by both. The answer is as the host's.
#[test]
fn identical_functions_compile_to_one_body_and_run_at_every_level() {
    let body = "  %a = add i32 %x, %y\n  %b = mul i32 %a, 3\n  %c = xor i32 %b, %y\n  %d = sub i32 %c, %x\n  %e = add i32 %d, 77777\n  %f = xor i32 %e, %a\n  ret i32 %f\n";
    let ir = format!(
        "@x = global i32 123456\n@y = global i32 -2024\n\
         define i32 @f(i32 %x, i32 %y) {{\n{body}}}\n\
         define i32 @g(i32 %x, i32 %y) {{\n{body}}}\n\
         define i8 @probe() {{\n  %x = load volatile i32, ptr @x\n  %y = load volatile i32, ptr @y\n\
           %p = call i32 @f(i32 %x, i32 %y)\n  %q = call i32 @g(i32 %y, i32 %x)\n  %r = add i32 %p, %q\n\
           %bad = icmp ne i32 %r, {}\n  %o = zext i1 %bad to i8\n  ret i8 %o\n}}\n",
        {
            let f = |x: i32, y: i32| {
                let a = x.wrapping_add(y);
                let b = a.wrapping_mul(3);
                let c = b ^ y;
                let d = c.wrapping_sub(x);
                let e = d.wrapping_add(77777);
                e ^ a
            };
            f(123456, -2024).wrapping_add(f(-2024, 123456))
        }
    );
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let code: String =
            text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
        let name = format!("outline-functions-{level:?}");
        assert_eq!(common::run_with(&name, "  jsr probe\n", &code), 0, "{level:?}:\n{text}");
    }
}

/// A body is never outlined again: the chain of calls is one deep, so a body
/// adds two bytes of the return stack to the code it replaces, at any depth of
/// recursion above it.
#[test]
fn an_outlined_text_is_not_outlined_again() {
    let body = format!("{ADD}{ADD}{ADD}");
    let once = run(&probe(&body));
    assert!(once.contains("__outl_"), "{once}");
    assert_eq!(run(&once), once);
    // a call of an outlined body is not in a body: the chain stays one deep
    let call = "  lda rc16\n  clc\n  adc rc17\n  sta rc18\n  jsr __outl_9\n  lda rc18\n  eor #$0F\n  sta rc19\n";
    let nested = run(&probe(&format!("{call}{call}")));
    let bodies = nested.split_once("__outl_0:").map_or("", |(_, bodies)| bodies);
    assert!(!bodies.contains("jsr __outl_9"), "{nested}");
}

/// Two recursive functions with the same arithmetic: the arithmetic is a body
/// and the recursive call is not in it, so the recursion holds the same two
/// bytes a level as before and, at its deepest, two more than the code it
/// replaced. Run to a depth that fills most of the return stack.
#[test]
fn a_recursive_function_with_an_outlined_body_runs_to_its_deepest_level() {
    let depth = 24;
    let step = |n: i32| {
        let a = n.wrapping_mul(7).wrapping_add(3);
        let b = a ^ (n << 3);
        b.wrapping_sub(n).wrapping_add(41)
    };
    let mut want = 0i32;
    for n in 1..=depth {
        want = want.wrapping_add(step(n));
    }
    let body = |me: &str| {
        format!(
            "  %z = icmp eq i32 %n, 0\n  br i1 %z, label %base, label %more\n\
             base:\n  ret i32 0\n\
             more:\n  %m = sub i32 %n, 1\n  %r = call i32 @{me}(i32 %m)\n\
               %a0 = mul i32 %n, 7\n  %a = add i32 %a0, 3\n  %s = shl i32 %n, 3\n  %b = xor i32 %a, %s\n\
               %c = sub i32 %b, %n\n  %d = add i32 %c, 41\n  %e = add i32 %d, %r\n  ret i32 %e\n"
        )
    };
    let ir = format!(
        "define i32 @f(i32 %n) {{\n{}}}\ndefine i32 @g(i32 %n) {{\n{}}}\n\
         define i8 @probe() {{\n  %r = call i32 @f(i32 {depth})\n  %q = call i32 @g(i32 {depth})\n  %t = add i32 %r, %q\n\
           %bad = icmp ne i32 %t, {}\n  %o = zext i1 %bad to i8\n  ret i8 %o\n}}\n",
        body("f"),
        body("g"),
        want.wrapping_mul(2)
    );
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        let code: String =
            text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
        let name = format!("outline-recursion-{level:?}");
        assert_eq!(common::run_with(&name, "  jsr probe\n", &code), 0, "{level:?}:\n{text}");
    }
}

/// Four copies of a 4-instruction sum in a loop were outlined for the 11 bytes
/// they save: each run of the loop added 4 calls, 48 clocks for 11 bytes, and
/// `loop_fusion` ran 31% slower (122,028 clocks against 92,923 at -O1). A place
/// in a loop runs with the loop and is given up; outside one the same four
/// copies are outlined, and at `-Os` (no rate) so are the ones in the loop.
#[test]
fn a_sequence_in_a_loop_pays_for_each_run_of_the_loop() {
    let sum = "  lda rc16\n  clc\n  adc rc17\n  sta rc18\n";
    let straight = probe(&sum.repeat(4));
    assert!(run(&straight).contains("jsr __outl_0"), "the copies are not outlined where they run once");
    let looped = probe(&format!("again:\n{}  dec rc30\n  bne again\n", sum.repeat(4)));
    assert_eq!(run(&looped), looped, "the copies are outlined inside a loop");
    assert!(run_at(&looped, None).contains("jsr __outl_0"), "bytes alone do not outline them");
}

/// Six copies of a 2-instruction move save 1 byte for 72 clocks: priced at
/// 12 clocks a byte, the first rule gave them up in straight-line code, where
/// they run once and the program is no slower for more than a byte of them.
/// A place outside every loop is never priced.
#[test]
fn a_place_outside_every_loop_is_never_priced() {
    let text = probe(&"  lda rc16\n  sta rc17\n".repeat(6));
    let outlined = run(&text);
    assert!(outlined.contains("jsr __outl_0"), "{outlined}");
    assert!(bytes(&outlined) < bytes(&text), "{} -> {} B", bytes(&text), bytes(&outlined));
}

/// Five reports of constants, each with `ldx #0; stx rc2; stx rc3` made first,
/// were five calls of one body at -O2: +12 clocks a run for 5 bytes a place, in
/// a module of 72 bytes and 100 clocks (double_against_int: 439 clocks against
/// llvm-mos's 417, 51 bytes against 72). Priced by the share of the module's
/// clocks the calls add against the share of its bytes they save, they stay
/// inline; the same text at -Os (no share) is outlined.
#[test]
fn a_run_outside_a_loop_is_outlined_where_its_bytes_are_a_larger_share_than_its_clocks() {
    let report = "  lda #5
  ldx #0
  stx rc2
  stx rc3
  jsr report
";
    let text = probe(&report.repeat(5));
    assert!(run_priced(&text, None, None).contains("jsr __outl_0"), "bytes alone outline them");
    assert!(!run_priced(&text, None, Some(1)).contains("__outl_"), "{}", run_priced(&text, None, Some(1)));
}

/// `word_compares` at -O2 on the 65C02 ran 97,736 clocks against 93,233: a
/// 3-instruction run in a 14x14 loop was outlined for 4 bytes and added 12
/// clocks a run. With a share a place in a loop is given up where its runs add
/// a larger share of the module's clocks than its bytes save of the module's;
/// without one (-Os) it is outlined.
#[test]
fn a_run_in_a_loop_is_given_up_where_its_clocks_are_a_larger_share_than_its_bytes() {
    let copy = "  lda rc16\n  sta rc17\n";
    // the moves run in the inner of two loops: each call is 12 clocks of a hot
    // loop
    let looped = probe(&format!("again:\nspin:\n{}  dec rc30\n  bne spin\n  dec rc31\n  bne again\n", copy.repeat(6)));
    assert!(run_priced(&looped, None, None).contains("jsr __outl_0"), "bytes alone outline them");
    assert!(!run_priced(&looped, None, Some(1)).contains("__outl_"), "{}", run_priced(&looped, None, Some(1)));
}

/// double_against_int, old_style_extra_args and switch_i64 at -O1 ran 5% over
/// llvm-mos (439 clocks against 417): five calls with the same constant were
/// five `jsr __outl_0` of one body, 12 clocks a run in a 72-byte module. At
/// -O1 to -O3 the share rule gives them up; at -Os (bytes alone) they stay.
#[test]
fn five_calls_with_one_constant_are_inline_at_every_speed_level() {
    let call = "  call void @report(i32 5)\n";
    let ir = format!(
        "target datalayout = \"{}\"\ndeclare void @report(i32)\ndefine void @main() {{\n{}  ret void\n}}\n",
        datalayout(),
        call.repeat(5)
    );
    for level in [Level::O1, Level::O2, Level::O3] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        assert!(!text.contains("__outl_"), "{level:?}\n{text}");
    }
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__outl_"), "-Os outlines them for bytes\n{text}");
}
