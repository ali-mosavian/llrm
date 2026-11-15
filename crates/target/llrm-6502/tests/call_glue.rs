//! The 32-bit runtime routines are calls of the `mos` convention (the pin's
//! `makeLibCall` lowers a libcall through the target's calling convention): the
//! first operand in A:X:rc2:rc3, the second in rc4..rc7, the result in
//! A:X:rc2:rc3. Through `__sa`/`__sb` a call site copied 8 bytes in and 4
//! out, 50 to 60 B (div_constants_long: 23,220 B against cc65's 13,862,
//! 2026-11-15).

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::routines::cells_named;

const FAMILIES: [&str; 14] = [
    "__mulsi3",
    "__udivsi3",
    "__umodsi3",
    "__divsi3",
    "__modsi3",
    "__divmodsi4",
    "__udivmodsi4",
    "__ashlsi3",
    "__lshrsi3",
    "__ashrsi3",
    "__umulhisi3",
    "__mulhisi3",
    "__umulhisi3_byte",
    "__mulhisi3_byte",
];
const ENTRIES: [&str; 3] = ["", "_fast", "_small"];

/// A call site clobbers the cells its base routine names
/// (`call_runtime_entry`), and `emit_with` picks any variant of it: a variant
/// that named another cell would destroy a value the allocator believed kept.
#[test]
fn every_variant_names_the_cells_of_its_base() {
    for base in FAMILIES {
        let named = cells_named(base).unwrap_or_else(|| panic!("{base} names cells the scan cannot read"));
        assert!(!named.is_empty() && named.iter().all(|cell| (2..=7).contains(cell)), "{base}: {named:?}");
        for entry in
            ENTRIES.iter().flat_map(|entry| [entry.to_string(), format!("{entry}_byte"), format!("{entry}_small")])
        {
            let variant = format!("{base}{entry}");
            if let Some(cells) = cells_named(&variant) {
                assert!(cells.is_subset(&named), "{variant}: {cells:?} of {named:?}");
            }
        }
    }
}

fn asm(
    ir: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    text.split(".segment").next().unwrap().to_owned()
}

fn operation(op: &str) -> String {
    format!("define i32 @f(i32 %x, i32 %y) {{\n  %r = {op} i32 %x, %y\n  ret i32 %r\n}}\n")
}

/// `x / y` of two parameters has its operands where the routine takes them and
/// its result where the function returns it: no copy at all, a tail call. The
/// site copied both operands to `__sa`/`__sb` and the result back (about 60 B).
#[test]
fn a_long_operation_on_parameters_is_a_tail_call_with_no_copy() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        for op in ["sdiv", "udiv", "srem", "urem", "mul", "shl", "lshr", "ashr"] {
            let text = asm(&operation(op), level);
            assert!(!text.contains("__sa") && !text.contains("__sb"), "{level:?} {op}:\n{text}");
            let bytes = llrm_6502::timing::estimate(Cpu::Mos6502, &text).expect("prices").bytes;
            assert!(bytes <= 8, "{level:?} {op}: {bytes} B:\n{text}");
        }
    }
}

/// A long operation of values that are not already in place (two loaded from
/// globals) copies each byte once, to the cell the routine reads: one `lda`
/// and one `sta` for a byte in rc2..rc7, a load into A or X for the first two.
#[test]
fn a_long_operation_on_loaded_values_copies_each_byte_once() {
    let ir = "@a = global i32 0\n@b = global i32 0\n\
              define i32 @f() {\n  %x = load volatile i32, ptr @a\n  %y = load volatile i32, ptr @b\n  %r = sdiv i32 %x, %y\n  ret i32 %r\n}\n";
    for level in [Level::O2, Level::Os] {
        let text = asm(ir, level);
        let bytes = llrm_6502::timing::estimate(Cpu::Mos6502, &text).expect("prices").bytes;
        assert!(bytes <= 8 * 3 + 6 * 4 + 6, "{level:?}: {bytes} B:\n{text}");
        assert!(!text.contains("__sa") && !text.contains("__sb"), "{level:?}:\n{text}");
    }
}

/// `jsr` of a long routine reads A: a load of A before it is not dead. The
/// text pass took the routine for one that reads no register
/// (`__modsi3_fast_byte` was not among the callees known to read `a`, only
/// `__modsi3_byte` and `__modsi3_byte_fast`): three `% 5`, `% 3` and `% 7` of a
/// loop, 27 where 42 (recdeep, switch_jump_table, div_constants_long at -O2).
#[test]
fn a_byte_divisor_call_keeps_the_load_of_its_first_byte() {
    let step = |a: i32, b: i32, c: i32, d: i32, e: i32| {
        let rem = |x: i32, y: i32| x.wrapping_rem(y);
        (b.wrapping_add(1), c.wrapping_add(rem(a, 5)), d ^ b, e.wrapping_add(rem(c, 3)), a.wrapping_add(rem(d, 7)))
    };
    let (mut state, trips) = ((1, 2, 3, 4, 5), 40);
    for _ in 0..trips {
        state = step(state.0, state.1, state.2, state.3, state.4);
    }
    let want = state.0.wrapping_add(state.1).wrapping_add(state.2).wrapping_add(state.3).wrapping_add(state.4);
    let ir = format!(
        "@start = global i32 1\n@trips = global i32 {trips}\n\
         define i8 @probe() {{\n\
         entry:\n  %a0 = load volatile i32, ptr @start\n  %n0 = load volatile i32, ptr @trips\n  br label %loop\n\
         loop:\n\
           %a = phi i32 [ %a0, %entry ], [ %a2, %loop ]\n\
           %b = phi i32 [ 2, %entry ], [ %b2, %loop ]\n\
           %c = phi i32 [ 3, %entry ], [ %c2, %loop ]\n\
           %d = phi i32 [ 4, %entry ], [ %d2, %loop ]\n\
           %e = phi i32 [ 5, %entry ], [ %e2, %loop ]\n\
           %n = phi i32 [ %n0, %entry ], [ %n2, %loop ]\n\
           %m5 = srem i32 %a, 5\n  %m3 = srem i32 %c, 3\n  %m7 = srem i32 %d, 7\n\
           %a2 = add i32 %b, 1\n  %b2 = add i32 %c, %m5\n  %c2 = xor i32 %d, %b\n  %d2 = add i32 %e, %m3\n  %e2 = add i32 %a, %m7\n\
           %n2 = sub i32 %n, 1\n  %go = icmp ne i32 %n2, 0\n  br i1 %go, label %loop, label %exit\n\
         exit:\n  %s1 = add i32 %a2, %b2\n  %s2 = add i32 %s1, %c2\n  %s3 = add i32 %s2, %d2\n  %s4 = add i32 %s3, %e2\n\
           %bad = icmp ne i32 %s4, {want}\n  %r = zext i1 %bad to i8\n  ret i8 %r\n}}\n"
    );
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = assembly(&format!("target datalayout = \"{}\"\n{ir}", datalayout()), &Request::new(level))
            .unwrap_or_else(|e| panic!("{e}"));
        let code: String =
            text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
        let name = format!("call-glue-mod-loop-{level:?}");
        assert_eq!(common::run_with(&name, "  jsr probe\n", &code), 0, "{level:?}: wrong result:\n{text}");
    }
}

/// The remainder of a word divide pair stays in `rc6:rc7`; the next word divide
/// leaves its own there (`__divhi3c` calls `__udivhi3c`). A later compare read
/// the stale cell: div_constants_short compared against the second remainder
/// and passed only because both divisors were 3.
#[test]
fn a_pair_remainder_survives_a_later_word_division() {
    let (x, y, z): (i16, i16, i16) = (10, 100, 7);
    let want = ((x / 3) ^ (y / z)) + x % 3;
    let ir = format!(
        "@x = global i16 {x}\n@y = global i16 {y}\n@z = global i16 {z}\n@k = global i16 3\n\
         define i8 @probe() {{\n  %a = load volatile i16, ptr @x\n  %b = load volatile i16, ptr @y\n  %c = load volatile i16, ptr @z\n\
           %k = load volatile i16, ptr @k\n  %q = udiv i16 %a, %k\n  %r = urem i16 %a, %k\n  %s = sdiv i16 %b, %c\n  %m = xor i16 %q, %s\n  %t = add i16 %m, %r\n\
           %bad = icmp ne i16 %t, {want}\n  %o = zext i1 %bad to i8\n  ret i8 %o\n}}\n"
    );
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = assembly(&format!("target datalayout = \"{}\"\n{ir}", datalayout()), &Request::new(level))
            .unwrap_or_else(|e| panic!("{e}"));
        let code: String =
            text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
        let name = format!("call-glue-pair-remainder-{level:?}");
        assert_eq!(common::run_with(&name, "  jsr probe\n", &code), 0, "{level:?}: stale remainder:\n{text}");
    }
}

/// The same with the later divide unsigned: `x % 3` (a pair's rc6:rc7) against
/// `x % c` read rc6 after both, so it compared the second remainder with
/// itself and said equal (10 % 3 against 10 % 7: exit 0, want 1). release/next
/// compiles div_constants_short's unsigned function so.
#[test]
fn a_pair_remainder_is_not_compared_with_a_later_remainder_of_itself() {
    let ir = "@x = global i16 10\n@c = global i16 7\n@k = global i16 3\n\
              define i8 @probe() {\n  %a = load volatile i16, ptr @x\n  %c = load volatile i16, ptr @c\n  %k = load volatile i16, ptr @k\n\
                %q = udiv i16 %a, %k\n  %r = urem i16 %a, %k\n  %s = udiv i16 %a, %c\n\
                %eq = icmp ne i16 %q, %s\n  %t = urem i16 %a, %c\n  %ne = icmp ne i16 %r, %t\n  %both = and i1 %eq, %ne\n  %o = zext i1 %both to i8\n  ret i8 %o\n}\n";
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = assembly(&format!("target datalayout = \"{}\"\n{ir}", datalayout()), &Request::new(level))
            .unwrap_or_else(|e| panic!("{e}"));
        let code: String =
            text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
        let name = format!("call-glue-pair-remainder-compare-{level:?}");
        assert_eq!(common::run_with(&name, "  jsr probe\n", &code), 1, "{level:?}: stale remainder:\n{text}");
    }
}

/// `rc2:rc3` is the high word `__umulhisi3` returns and the next call writes. A
/// copy of it from `rc2` was folded forward past the second call, so the high
/// word of a 32x32 product read the next partial product (int64_division,
/// overflow_builtins at -O2).
#[test]
fn a_wide_product_keeps_its_high_word_across_the_next_call() {
    let (a, b): (u32, u32) = (0xDEAD_BEEF, 0x1234_5678);
    let want = u64::from(a) * u64::from(b);
    let ir = format!(
        "@a = global i32 {}\n@b = global i32 {}\n\
         define i8 @probe() {{\n  %x = load volatile i32, ptr @a\n  %y = load volatile i32, ptr @b\n\
           %xw = zext i32 %x to i64\n  %yw = zext i32 %y to i64\n  %p = mul i64 %xw, %yw\n\
           %bad = icmp ne i64 %p, {want}\n  %r = zext i1 %bad to i8\n  ret i8 %r\n}}\n",
        a as i32, b as i32
    );
    for level in [Level::O2, Level::O3, Level::Os] {
        let text = assembly(&format!("target datalayout = \"{}\"\n{ir}", datalayout()), &Request::new(level))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(text.contains("__umulhisi3"), "{level:?}: no widening multiply:\n{text}");
        let code: String =
            text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
        let name = format!("call-glue-wide-product-{level:?}");
        assert_eq!(common::run_with(&name, "  jsr probe\n", &code), 0, "{level:?}: wrong product:\n{text}");
    }
}
