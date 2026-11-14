//! The pipeline of `llrm-6502-llc` (`driver::assembly`): the request's platform
//! and level reach the allocator.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn module(function: &str) -> String {
    format!("target datalayout = \"{}\"\n{function}\n", datalayout())
}

/// Ten sums are live at once. Compiled for sim65 (26 zero-page bytes) they fit
/// in scratch cells; with 16 the allocator spills. A driver that kept
/// `select_function`'s own 16 spilled on every platform: 8% more bytes over the
/// hand corpus.
#[test]
fn the_platforms_zero_page_reaches_the_allocator() {
    let pairs = [
        ("a", "b"),
        ("b", "c"),
        ("c", "d"),
        ("d", "e"),
        ("e", "f"),
        ("a", "c"),
        ("b", "d"),
        ("c", "e"),
        ("d", "f"),
        ("a", "f"),
    ];
    let mut ir = String::from("define i8 @many(i8 %a, i8 %b, i8 %c, i8 %d, i8 %e, i8 %f) {\n");
    for (n, (x, y)) in pairs.iter().enumerate() {
        ir += &format!("  %t{n} = add i8 %{x}, %{y}\n");
    }
    ir += "  %x0 = xor i8 %t0, %t1\n";
    for n in 2..pairs.len() {
        ir += &format!("  %x{} = xor i8 %x{}, %t{n}\n", n - 1, n - 2);
    }
    ir += &format!("  ret i8 %x{}\n}}", pairs.len() - 2);
    let mut request = Request::new(Level::O0);
    let wide = assembly(&module(&ir), &request).unwrap();
    request.platform.zero_page = 16;
    let narrow = assembly(&module(&ir), &request).unwrap();
    assert!(!wide.contains("s_many_") && !wide.contains("pha"), "sim65 still spills or saves:\n{wide}");
    assert!(narrow.contains("s_many_0") || narrow.contains("pha"), "no pressure at 16:\n{narrow}");
}

/// A word shifted by 5 is a counted loop at the size level and `asl`/`rol`
/// pairs at speed. A driver that kept `select_function`'s speed tuning gave
/// `-Os` the same text as `-O2`.
#[test]
fn the_size_level_is_priced_in_bytes() {
    let ir = module("define i16 @shl5(i16 %x) {\n  %p = shl i16 %x, 5\n  %q = add i16 %p, %x\n  ret i16 %q\n}");
    let size = assembly(&ir, &Request::new(Level::Os)).unwrap();
    let speed = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(size.contains("dex") || size.contains("dey"), "-Os:\n{size}");
    assert!(!speed.contains("dex") && !speed.contains("dey"), "-O2:\n{speed}");
}

/// `n - 1` on a word went through the carry chain (`clc; adc #255` on each
/// byte, 22 clocks) where `lda lo; bne; dec hi; dec lo` borrows by branch (11):
/// memcmp's and strncmp's counters, a clock row against gcc. The borrow still
/// runs: 0x0100 - 1 = 0x00FF, 1 - 1 = 0, 0 - 1 = 0xFFFF.
#[test]
fn a_word_minus_one_borrows_by_branch() {
    let probe = |name: &str, start: i32| {
        format!(
            "@{name}_k = global i16 {start}\ndefine i8 @{name}() {{\n  %v = load volatile i16, ptr @{name}_k\n  \
             %d = add i16 %v, -1\n  %lo = trunc i16 %d to i8\n  %h = lshr i16 %d, 8\n  %hi = trunc i16 %h to i8\n  \
             %s = shl i8 %hi, 1\n  %r = xor i8 %lo, %s\n  ret i8 %r\n}}\n"
        )
    };
    let ir = module(&[probe("borrow", 256), probe("one", 1), probe("wrap", 0)].concat());
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("adc #255") && text.contains("dec"), "the carry chain:\n{text}");
    assert_eq!(exit_code_of(&text, "borrow"), 0xFF, "{text}");
    assert_eq!(exit_code_of(&text, "one"), 0, "{text}");
    assert_eq!(exit_code_of(&text, "wrap"), 1, "{text}");
}

/// A byte added to an accumulator from a global indexed by a byte loaded the
/// element, then added a cell (`lda g,x; clc; adc rc2; sta rc2`), where the
/// 6502 has `adc g,x` and the accumulator can stay in A: sum4 at -Os 22 B
/// against llvm-mos's 13, sum_arr, sum16, schr. The sum of 1 2 3 4 is 10.
#[test]
fn an_indexed_global_is_the_operand_of_the_add() {
    let ir = module(
        r#"@arr = global [4 x i8] c"\01\02\03\04"
@n = global i8 4
define i8 @probe() {
entry:
  %n = load volatile i8, ptr @n
  br label %loop
loop:
  %i = phi i8 [ 0, %entry ], [ %i2, %loop ]
  %s = phi i8 [ 0, %entry ], [ %s2, %loop ]
  %z = zext i8 %i to i16
  %p = getelementptr i8, ptr @arr, i16 %z
  %b = load i8, ptr %p
  %s2 = add i8 %s, %b
  %i2 = add i8 %i, 1
  %c = icmp eq i8 %i2, %n
  br i1 %c, label %done, label %loop
done:
  ret i8 %s2
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("adc arr,x") && !text.contains("lda arr,x"), "the element was loaded:\n{text}");
    assert_eq!(exit_code_of(&text, "probe"), 10, "{text}");
}

/// `t[(a + b) >> 1]` on bytes is a pointer plus an `i16` whose high byte is
/// known zero. Selected as a 16-bit add into the pointer and `ldy #0; lda
/// (rc),y` (bsearch8: 78 B against llvm-mos's 53) instead of the byte index in
/// Y.
#[test]
fn an_index_with_a_known_zero_high_byte_is_the_y_of_the_load() {
    let ir = module(
        "define i8 @ldm(ptr %t, i8 %a, i8 %b) {\n  %x = zext i8 %a to i16\n  %y = zext i8 %b to i16\n  \
         %s = add nuw nsw i16 %x, %y\n  %m = lshr i16 %s, 1\n  %p = getelementptr inbounds i8, ptr %t, i16 %m\n  \
         %v = load i8, ptr %p\n  ret i8 %v\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    // the pointer arrives in a:x and is copied to a pair once; the sum is the
    // index
    let at = text.find("lda (rc").unwrap_or_else(|| panic!("no load through a pair:\n{text}"));
    assert!(text[at..].starts_with(&format!("lda ({}),y", &text[at + 5..].split(')').next().unwrap())), "{text}");
    assert!(
        text.matches("adc").count() == 1 && text.matches("sta rc").count() == 1,
        "the pointer was added in 16 bits:\n{text}"
    );
}

/// A pointer a call returns (a and x) was copied into its cell a byte at a
/// time, and the load through it asked the cell for a pair: "a value needed in
/// two places". `*p()`, `char *q = malloc(4); *q = 1` and cc65's basic.c
/// `factor` did not compile at any level. The byte at `val` is 42.
#[test]
fn a_pointer_a_call_returns_is_loaded_through() {
    let ir = module(
        "@val = global i8 42\ndefine weak noinline ptr @p() {\n  ret ptr @val\n}\n\
         define i8 @g() {\n  %q = call ptr @p()\n  %v = load i8, ptr %q\n  ret i8 %v\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "g"), 42, "{text}");
}

/// `LLRM_6502_ZERO_PAGE` sets the platform's zero page, whichever way the
/// platform is named. It was read before the row's own `--platform=sim65`,
/// which put the 26 back: a sweep of 16..26 read flat, 4362 bytes at every
/// size.
#[test]
fn the_zero_page_override_survives_the_platform_flag() {
    let dir = std::env::temp_dir().join(format!("llrm6502-zp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = dir.join("f.ll");
    std::fs::write(&ir, module("define i8 @f(i8 %a) {\n  ret i8 %a\n}")).unwrap();
    let equates = |bytes: &str| {
        let out = dir.join(format!("f{bytes}.s"));
        let ran = std::process::Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
            .args(["--platform=sim65", "-S", "-O0", "-o", out.to_str().unwrap(), ir.to_str().unwrap()])
            .env("LLRM_6502_ZERO_PAGE", bytes)
            .status()
            .unwrap();
        assert!(ran.success());
        std::fs::read_to_string(out).unwrap().lines().filter(|line| line.starts_with("rc")).count()
    };
    assert_eq!((equates("16"), equates("26")), (16, 26));
    std::fs::remove_dir_all(dir).ok();
}

/// An unknown `-f` switch was taken as a pass and ignored: `-fno-bogus`
/// compiled (the wasm session's instrument check, 2026-11-08), so a header flag
/// the compiler did not know measured as if honored. The switches llrm-cc
/// forwards must still pass.
#[test]
fn an_unknown_f_switch_is_refused() {
    let dir = std::env::temp_dir().join(format!("llrm6502-fsw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ir = dir.join("f.ll");
    std::fs::write(&ir, module("define i8 @f(i8 %a) {\n  ret i8 %a\n}")).unwrap();
    let run = |switch: &str| {
        std::process::Command::new(env!("CARGO_BIN_EXE_llrm-6502-llc"))
            .args(["--platform=sim65", "-S", switch, "-o", dir.join("f.s").to_str().unwrap(), ir.to_str().unwrap()])
            .output()
            .unwrap()
    };
    let bogus = run("-fno-bogus");
    assert_eq!(bogus.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bogus.stderr).contains("-fno-bogus"));
    for known in ["-fno-unroll-loops", "-fno-inline=f", "-fno-second-inline", "-fwrapv", "-fexcess-precision=standard"]
    {
        assert!(run(known).status.success(), "{known} is refused");
    }
    std::fs::remove_dir_all(dir).ok();
}

/// `-Os` holds an inline to the size threshold: a callee of about forty
/// instructions called three times stays a call, where `-O2` copies it in. The
/// hand corpus and bench tables have no such callee, so their `-Os` rows
/// equal their `-O2` rows and cannot show whether `-Os` reaches the mid-end.
#[test]
fn the_size_level_keeps_a_call_that_speed_inlines() {
    let ir = module(
        "@sink = external global [8 x i8]
define internal i8 @mix(i8 %x, i8 %y) {
  %a3 = mul i8 %x, 3
  %a = add i8 %a3, %y
  %xy = xor i8 %x, %y
  %b = add i8 %xy, 5
  %c = sub i8 %a, %b
  %ab = and i8 %a, %b
  %cx = xor i8 %c, %x
  %d = or i8 %ab, %cx
  %p0 = getelementptr i8, ptr @sink, i16 0
  store i8 %a, ptr %p0
  %p1 = getelementptr i8, ptr @sink, i16 1
  store i8 %b, ptr %p1
  %p2 = getelementptr i8, ptr @sink, i16 2
  store i8 %c, ptr %p2
  %p3 = getelementptr i8, ptr @sink, i16 3
  store i8 %d, ptr %p3
  %e = add i8 %a, %d
  %p4 = getelementptr i8, ptr @sink, i16 4
  store i8 %e, ptr %p4
  %f = sub i8 %b, %c
  %p5 = getelementptr i8, ptr @sink, i16 5
  store i8 %f, ptr %p5
  %ad = xor i8 %a, %d
  %g = add i8 %ad, %x
  %p6 = getelementptr i8, ptr @sink, i16 6
  store i8 %g, ptr %p6
  %bc = or i8 %b, %c
  %h = sub i8 %bc, %y
  %p7 = getelementptr i8, ptr @sink, i16 7
  store i8 %h, ptr %p7
  %s1 = add i8 %a, %b
  %s2 = add i8 %s1, %c
  %s3 = add i8 %s2, %d
  ret i8 %s3
}
define i8 @f(i8 %p, i8 %q) {
  %m1 = call i8 @mix(i8 %p, i8 %q)
  %m2 = call i8 @mix(i8 %q, i8 %p)
  %m3 = call i8 @mix(i8 %p, i8 9)
  %r1 = add i8 %m1, %m2
  %r2 = add i8 %r1, %m3
  ret i8 %r2
}",
    );
    let calls = |level| assembly(&ir, &Request::new(level)).unwrap().matches("jsr mix").count();
    assert_eq!((calls(Level::O2), calls(Level::Os)), (0, 3));
}

/// crc16 as the mid-end leaves it: the `if` of the bit loop is a `select`.
const CRC16_SELECT_SHAPED: &str = "define i16 @crc16(ptr %0, i8 %1) {\n\
  %.not = icmp eq i8 %1, 0\n\
  br i1 %.not, label %._crit_edge, label %.lr.ph\n\
\n\
.lr.ph:                                           \n\
  %.01118 = phi i8 [ %15, %14 ], [ 0, %2 ]\n\
  %.01217 = phi i16 [ %12, %14 ], [ -1, %2 ]\n\
  %3 = zext i8 %.01118 to i16\n\
  %4 = getelementptr i8, ptr %0, i16 %3\n\
  %5 = load i8, ptr %4, align 1\n\
  %6 = zext i8 %5 to i16\n\
  %7 = shl i16 %6, 8\n\
  %8 = xor i16 %7, %.01217\n\
  br label %9\n\
\n\
9:                                                \n\
  %.016 = phi i8 [ 0, %.lr.ph ], [ %13, %9 ]\n\
  %.115 = phi i16 [ %8, %.lr.ph ], [ %12, %9 ]\n\
  %10 = shl i16 %.115, 1\n\
  %11 = xor i16 %10, 4129\n\
  %.not14 = icmp slt i16 %.115, 0\n\
  %12 = select i1 %.not14, i16 %11, i16 %10\n\
  %13 = add i8 %.016, 1\n\
  %exitcond.not = icmp eq i8 %13, 8\n\
  br i1 %exitcond.not, label %14, label %9\n\
\n\
14:                                               \n\
  %15 = add i8 %.01118, 1\n\
  %exitcond19.not = icmp eq i8 %15, %1\n\
  br i1 %exitcond19.not, label %._crit_edge, label %.lr.ph\n\
\n\
._crit_edge:                                      \n\
  %.012.lcssa = phi i16 [ -1, %2 ], [ %12, %14 ]\n\
  ret i16 %.012.lcssa\n\
}\n\
";

/// The pin's `opt -O2` makes an eight-trip CRC loop a byte-wise table lookup
/// (`LoopIdiomRecognize`, priced by the target's cost model). Compiled with a
/// machine that had no cost model the recognizer never ran: crc16 kept its
/// bit loop (25 instructions, 6497 clocks) where the pin has 19.
#[test]
fn a_crc_loop_becomes_a_table_lookup() {
    let ir = module(CRC16_SELECT_SHAPED);
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains(".segment \"RODATA\"") && text.matches(".byte").count() >= 32, "no table:\n{text}");
    assert!(!text.contains("cmp #8"), "the bit loop is still there:\n{text}");
}

/// `icmp eq %n, %i2` and `icmp eq %i2, %n` are the same compare. LSR rewrites a
/// loop's exit test with the limit on the left; the backend then kept `%i2` in
/// memory (`stx rc4; inc rc4; lda rc2; cmp rc4`, 29 lines against 26)
/// and sum_arr went from 32 bytes and 249 clocks to 38 and 366.
#[test]
fn an_equality_compare_costs_the_same_either_way_round() {
    let sum = |compare: &str| {
        let ir = module(&format!(
            "@buf = external global [256 x i8]
define i8 @f(i8 %n) {{
entry:
  %z = icmp eq i8 %n, 0
  br i1 %z, label %exit, label %loop
loop:
  %i = phi i8 [ %i2, %loop ], [ 0, %entry ]
  %s = phi i8 [ %s2, %loop ], [ 0, %entry ]
  %e = zext i8 %i to i16
  %p = getelementptr i8, ptr @buf, i16 %e
  %v = load i8, ptr %p
  %s2 = add i8 %v, %s
  %i2 = add i8 %i, 1
  %c = icmp eq {compare}
  br i1 %c, label %exit, label %loop
exit:
  %r = phi i8 [ 0, %entry ], [ %s2, %loop ]
  ret i8 %r
}}"
        ));
        assembly(&ir, &Request::new(Level::O0)).unwrap()
    };
    assert_eq!(sum("i8 %i2, %n"), sum("i8 %n, %i2"));
}

/// The machine handed to the mid-end has a cost model; with none the CRC
/// recognizer and the other passes that price by it silently did not run (see
/// `a_crc_loop_becomes_a_table_lookup`).
#[test]
fn the_machine_handed_to_the_mid_end_has_a_tti() {
    let tti = llrm_6502::driver::machine().tti().expect("a cost model");
    assert_eq!(tti.name(), "mos6502", "the generic profile prices a word like a byte");
}

fn reloads_of(
    text: &str,
    cell: &str,
) -> usize {
    text.lines().filter(|line| ["lda", "ldx", "ldy"].iter().any(|load| line.trim() == format!("{load} {cell}"))).count()
}

/// A pointer is stored to its spill cell at entry; a path that does not write
/// its register used to load it again from the cell before the call (a
/// `lda`/`sta` pair a byte, twice, on each of two paths: 12 bytes of the 6-line
/// `f` below), because the spiller reloads before every use. Only the load
/// after the call, which took the register, is needed.
#[test]
fn a_spilled_value_is_not_reloaded_where_its_register_is_untouched() {
    let ir = module(
        "declare void @g(ptr)
define ptr @fun(ptr %b, i16 %v) {
  %c = icmp slt i16 %v, 0
  br i1 %c, label %neg, label %m
neg:
  %q = getelementptr inbounds i8, ptr %b, i16 1
  store i8 45, ptr %b
  br label %m
m:
  %p = phi ptr [ %q, %neg ], [ %b, %0 ]
  call void @g(ptr %p)
  ret ptr %b
}",
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    let call = text.find("jsr g").expect("the call");
    assert_eq!(reloads_of(&text[..call], "s_fun_0"), 0, "reloaded before the call:\n{text}");
    assert_eq!(reloads_of(&text[call..], "s_fun_0"), 1, "the load after the call is the one needed:\n{text}");
}

/// The control of the test above: a call takes the register, so the value is
/// loaded again after each one.
#[test]
fn a_spilled_value_is_reloaded_after_each_call_that_takes_its_register() {
    let ir = module(
        "declare void @g(ptr)
define ptr @fun(ptr %b) {
  call void @g(ptr %b)
  call void @g(ptr %b)
  ret ptr %b
}",
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    assert_eq!(reloads_of(&text, "s_fun_0"), 2, "{text}");
}

/// A C global named `a`, `x`, `y` or `s`, or after an instruction (`inc`,
/// `ror`), or after an equate (`rc3`), was written as `lda a`: ca65 read the
/// register or the opcode and stopped with "Syntax error" or "Illegal
/// addressing mode". loop_fusion, loop_interchange, pattern_fill and idioms of
/// tests/run/c did not assemble.
#[test]
fn a_symbol_named_like_a_register_an_opcode_or_an_equate_assembles() {
    let dir = std::env::temp_dir().join(format!("llrm6502-reserved-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["a", "A", "f", "x", "y", "s", "z", "lda", "inc", "ror", "rc3", "bra", "stz"] {
        let ir = module(&format!(
            "@{name} = global i8 0
define i8 @get_{name}() {{
  %v = load i8, ptr @{name}
  ret i8 %v
}}
define void @{name}_fn() {{
  store i8 1, ptr @{name}
  ret void
}}
define ptr @addr_{name}() {{
  ret ptr @{name}
}}"
        ));
        let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
        std::fs::write(dir.join("t.s"), format!(".setcpu \"65C02\"\n{text}")).unwrap();
        common::pipeline::tool(&dir, "ca65", &["t.s", "-o", "t.o"]).unwrap_or_else(|e| panic!("`{name}`: {e}\n{text}"));
    }
    std::fs::remove_dir_all(dir).ok();
}

/// The rename of a reserved symbol reads `@name` in the text; a string constant
/// that holds `@a` (an address in a message) is data and keeps its bytes.
#[test]
fn a_string_holding_at_a_reserved_name_keeps_its_bytes() {
    let ir = module(
        "@msg = constant [5 x i8] c\"u@a\\00x\"
define ptr @fun() {
  ret ptr @msg
}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("__r_"), "the string was rewritten:\n{text}");
}

/// `ldx #<src+198` is `(<src) + 198` to ca65, not the low byte of `src+198`:
/// ld65 stopped with "Range error" (copy_runs, split_loops, memcpy_libcall of
/// tests/run/c), and where the sum fitted, `ldy #>src+198` added 198 to the
/// high byte. A word stored at an odd address through `(rc2),y` takes its
/// pointer from the symbol and the offset.
#[test]
fn the_address_of_a_global_plus_an_offset_is_one_expression_to_the_assembler() {
    let ir = module(
        "@pad = internal global [250 x i8] zeroinitializer
@src = internal global [300 x i8] zeroinitializer
@dst = internal global [300 x i8] zeroinitializer
declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
declare void @report(i32)
define void @put() {
  store i16 7, ptr @src, align 1
  store i16 9, ptr getelementptr (i8, ptr @src, i16 198), align 1
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 @dst, ptr align 1 @src, i16 200, i1 false)
  %v = load i16, ptr getelementptr (i8, ptr @dst, i16 198), align 1
  %c = icmp ne i16 %v, 9
  %z = zext i1 %c to i32
  call void @report(i32 %z)
  ret void
}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let dir = std::env::temp_dir().join(format!("llrm6502-offset-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("t.s"),
        format!("{text}\n.export _main, memcpy, report\nmemcpy:\nreport:\n_main:\n  jsr put\n  rts\n"),
    )
    .unwrap();
    common::pipeline::tool(&dir, "cl65", &["-t", "sim6502", "-o", "t.bin", "t.s"])
        .unwrap_or_else(|e| panic!("{e}\n{text}"));
    std::fs::remove_dir_all(dir).ok();
}

/// The bytes `assembly` gives `ir` at -O2, priced by `timing::estimate`.
fn bytes_at_o2(ir: &str) -> u32 {
    let request = Request::new(Level::O2);
    let text = assembly(&module(ir), &request).unwrap();
    llrm_6502::timing::estimate(request.cpu, &text).unwrap().bytes
}

/// crc16 is the small loop. Ruling: no 512-byte Sarwate table (596 B, 1275 cy
/// against 88 B, 6221 cy; cc65 -O is 259 B). The loop-shaped source (clang's
/// `-O0` IR, the `if` a branch) stays under 100 bytes. The select-shaped source
/// of `a_crc_loop_becomes_a_table_lookup` does become the table, so the bound
/// is checked to see the table when it is there.
#[test]
fn crc16_is_the_small_loop_and_the_bound_sees_the_table() {
    let looped = bytes_at_o2(
        r#"define i16 @crc16(ptr %p, i8 %n) {
  %crc = alloca i16
  %i = alloca i8
  %j = alloca i8
  store i16 -1, ptr %crc
  store i8 0, ptr %i
  br label %outer
outer:
  %iv = load i8, ptr %i
  %iz = zext i8 %iv to i16
  %nz = zext i8 %n to i16
  %c0 = icmp slt i16 %iz, %nz
  br i1 %c0, label %body, label %done
body:
  %q = getelementptr inbounds nuw i8, ptr %p, i16 %iz
  %b = load i8, ptr %q
  %bz = zext i8 %b to i16
  %sh = shl i16 %bz, 8
  %c1 = load i16, ptr %crc
  %x = xor i16 %c1, %sh
  store i16 %x, ptr %crc
  store i8 0, ptr %j
  br label %inner
inner:
  %jv = load i8, ptr %j
  %jz = zext i8 %jv to i16
  %c2 = icmp slt i16 %jz, 8
  br i1 %c2, label %step, label %next
step:
  %v = load i16, ptr %crc
  %top = and i16 %v, -32768
  %set = icmp ne i16 %top, 0
  br i1 %set, label %yes, label %no
yes:
  %v1 = load i16, ptr %crc
  %s1 = shl i16 %v1, 1
  %r1 = xor i16 %s1, 4129
  br label %join
no:
  %v2 = load i16, ptr %crc
  %r2 = shl i16 %v2, 1
  br label %join
join:
  %r = phi i16 [ %r1, %yes ], [ %r2, %no ]
  store i16 %r, ptr %crc
  %jn = add i8 %jv, 1
  store i8 %jn, ptr %j
  br label %inner
next:
  %in = add i8 %iv, 1
  store i8 %in, ptr %i
  br label %outer
done:
  %res = load i16, ptr %crc
  ret i16 %res
}"#,
    );
    assert!(looped < 100, "crc16 grew to {looped} bytes: the table is back");
    // the 88-byte loop of the ruling; 83 bytes while the bit counter kept its
    // `dec` on a cell (an opcode-keyed join rule, removed): an open row until
    // the split price sees the instruction cost
    assert!(looped <= 88, "crc16 is {looped} bytes: the loop grew");
    let table = bytes_at_o2(CRC16_SELECT_SHAPED);
    assert!(table > 500, "the select-shaped crc16 is {table} bytes: the bound cannot see the table");
}

/// Runs `main` on sim65: the equates, the compiled `text`, and a `_main` that
/// sets the soft stack and returns what `function` leaves in A as the exit
/// code.
fn exit_code_of(
    text: &str,
    function: &str,
) -> i32 {
    exit_code_with(text, function, "")
}

/// [`exit_code_of`] with `runtime` (ca65 text) after `_main`: the routines the
/// compiled code calls.
fn exit_code_with(
    text: &str,
    function: &str,
    runtime: &str,
) -> i32 {
    // Tests share the process and often the function name: one directory each.
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("llrm6502-run-{}-{run}-{function}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let main = format!(
        ".export _main\n_main:\n  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n  jsr {function}\n  ldx #0\n  rts\n"
    );
    std::fs::write(dir.join("t.s"), format!("{text}\n{main}{runtime}")).unwrap();
    common::pipeline::tool(&dir, "cl65", &["-t", "sim6502", "-o", "t.bin", "t.s"])
        .unwrap_or_else(|e| panic!("{e}\n{text}"));
    let code = std::process::Command::new("sim65").arg(dir.join("t.bin")).status().unwrap().code().unwrap();
    std::fs::remove_dir_all(dir).ok();
    code
}

/// Clocks of `strncpy(dst, src, 30)` on a 20-byte source, as `sim65 -c` counts
/// them with the program's start: `text` is the compiled function, the rest a
/// `_main` and a `memset` that fills the tail.
fn clocks_of_strncpy(text: &str) -> u64 {
    const MAIN: &str = ".export _main, memset\n.segment \"BSS\"\nsrc: .res 32\ndst: .res 40\n.segment \"CODE\"\n_main:\n  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n  ldx #0\nfill:\n  lda #65\n  sta src,x\n  inx\n  cpx #20\n  bne fill\n  lda #0\n  sta src,x\n  lda #<dst\n  sta rc2\n  lda #>dst\n  sta rc3\n  lda #<src\n  sta rc4\n  lda #>src\n  sta rc5\n  ldx #0\n  lda #30\n  jsr k_strncpy\n  ldx #0\n  rts\nmemset:\n  sta rc8\n  lda rc4\n  ora rc5\n  beq @done\n  ldy #0\n@loop:\n  lda rc8\n  sta (rc2),y\n  iny\n  bne @c\n  inc rc3\n@c:\n  lda rc4\n  bne @d\n  dec rc5\n@d:\n  dec rc4\n  lda rc4\n  ora rc5\n  bne @loop\n@done:\n  rts\n";
    let dir = std::env::temp_dir().join(format!("llrm6502-clocks-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("t.s"), format!("{text}\n{MAIN}")).unwrap();
    common::pipeline::tool(&dir, "cl65", &["-t", "sim6502", "-o", "t.bin", "t.s"])
        .unwrap_or_else(|e| panic!("{e}\n{text}"));
    let out = std::process::Command::new("sim65").arg("-c").arg(dir.join("t.bin")).output().unwrap();
    std::fs::remove_dir_all(dir).ok();
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let words: Vec<&str> = said.split_whitespace().collect();
    words
        .windows(2)
        .find(|pair| pair[1] == "cycles")
        .and_then(|pair| pair[0].parse().ok())
        .unwrap_or_else(|| panic!("no cycle count in {said:?}"))
}

/// A constant another register holds is loaded by a transfer, one byte
/// shorter in `strncpy`, and that byte tipped `emit_on` to the layout without
/// the rotated loop: 3304 clocks for the call at `-O2` where the parent took
/// 3266. A layout is kept where it wins before the transfer pass and after it.
#[test]
fn a_byte_the_transfer_saves_does_not_pick_the_slower_layout() {
    let text =
        assembly(include_str!("fixtures/strncpy_o2.ll"), &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let clocks = clocks_of_strncpy(&text);
    assert!(clocks <= 3266, "strncpy takes {clocks} clocks; the parent's layout takes 3266:\n{text}");
}

/// A global holding the address of another (`static const char *p = buf + 3`,
/// a table of pointers) was refused: "a global initializer that is not an
/// integer, array or struct". function_table, static_pointers, tail_calls and
/// ten more programs of tests/run/c did not compile. The words are relocated by
/// the assembler and run: `tab[0][1]` is 2 and `*tab[1]`, the pointer `p` read
/// through its address in the table, is `buf[3]` = 4.
#[test]
fn a_global_initialized_with_the_address_of_another_runs() {
    let ir = module(
        r#"@buf = internal global [5 x i8] c"\01\02\03\04\05"
@p = internal global ptr getelementptr (i8, ptr @buf, i16 3)
@tab = internal global [2 x ptr] [ptr @buf, ptr @p]
define i8 @both() {
  %q0 = load ptr, ptr @tab
  %a.ptr = getelementptr i8, ptr %q0, i16 1
  %a = load i8, ptr %a.ptr
  %slot = getelementptr ptr, ptr @tab, i16 1
  %q1 = load ptr, ptr %slot
  %r = load ptr, ptr %q1
  %b = load i8, ptr %r
  %hi = shl i8 %a, 4
  %sum = or i8 %hi, %b
  ret i8 %sum
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert_eq!(exit_code_of(&text, "both"), 0x24, "{text}");
}

/// A function returning an `i64` was refused: "calling.toml has a result row"
/// (6502 `pair_return`, `value_of_valueless`), then, with the row,
/// "ExtractElement I32 expand result" and "Truncate I32 expand result". The
/// halves come back in the right order: high byte 0x44 minus low byte 0x88 is
/// 0xBC.
#[test]
fn an_i64_result_comes_back_in_eight_registers_in_order() {
    let ir = module(
        r#"@a = global i32 287454020
@b = global i32 1432778632
define internal i64 @pair(i32 %x, i32 %y) #0 {
  %hi = zext i32 %x to i64
  %up = shl i64 %hi, 32
  %lo = zext i32 %y to i64
  %r = or i64 %up, %lo
  ret i64 %r
}
define i8 @pair_halves() {
  %x = load i32, ptr @a
  %y = load i32, ptr @b
  %r = call i64 @pair(i32 %x, i32 %y)
  %top = lshr i64 %r, 32
  %h = trunc i64 %top to i8
  %l = trunc i64 %r to i8
  %d = sub i8 %h, %l
  ret i8 %d
}
attributes #0 = { noinline }"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert_eq!(exit_code_of(&text, "pair_halves"), 0xBC, "{text}");
}

/// An `i64` add or subtract split into `i32` halves, for which there is no
/// carry operation, was refused ("expand add or subtract without a carry
/// operation"; sbb_wide_sub, sysv_abi). 5 - 7 borrows into the high half: the
/// high byte is 0xFF and the low 0xFE, so 0xFF - 0xFE is 1.
#[test]
fn a_64_bit_subtract_borrows_between_its_32_bit_halves() {
    let ir = module(
        r#"@a = global i32 5
@b = global i32 7
define i8 @borrow() {
  %x = load i32, ptr @a
  %y = load i32, ptr @b
  %wx = zext i32 %x to i64
  %wy = zext i32 %y to i64
  %r = sub i64 %wx, %wy
  %top = lshr i64 %r, 32
  %h = trunc i64 %top to i8
  %l = trunc i64 %r to i8
  %d = sub i8 %h, %l
  ret i8 %d
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert_eq!(exit_code_of(&text, "borrow"), 1, "{text}");
}

/// A hot loop that copies a record of unknown length each trip and keeps six
/// words live across the copy; the records are 2, 1 and 3 bytes.
const COPY_LOOP: &str = r#"declare void @llvm.memcpy.p0.p0.i16(ptr noalias, ptr noalias, i16, i1)
@src = internal global [9 x i8] c"\02\0A\0B\01\0C\03\0D\0E\0F"
@dst = internal global [6 x i8] zeroinitializer
define i16 @records() {
entry:
  %end = getelementptr i8, ptr @dst, i16 6
  br label %head
head:
  %p = phi ptr [ @src, %entry ], [ %p2, %head ]
  %q = phi ptr [ @dst, %entry ], [ %q2, %head ]
  %a = phi i16 [ 0, %entry ], [ %a2, %head ]
  %b = phi i16 [ 1, %entry ], [ %b2, %head ]
  %c = phi i16 [ 2, %entry ], [ %c2, %head ]
  %d = phi i16 [ 3, %entry ], [ %d2, %head ]
  %t = load i8, ptr %p
  %len = zext i8 %t to i16
  %from = getelementptr i8, ptr %p, i16 1
  call void @llvm.memcpy.p0.p0.i16(ptr %q, ptr %from, i16 %len, i1 false)
  %p2 = getelementptr i8, ptr %from, i16 %len
  %q2 = getelementptr i8, ptr %q, i16 %len
  %a2 = add i16 %a, %len
  %b2 = xor i16 %b, %a
  %c2 = add i16 %c, %b
  %d2 = sub i16 %d, %c
  %more = icmp ult ptr %q2, %end
  br i1 %more, label %head, label %done
done:
  %last = getelementptr i8, ptr @dst, i16 5
  %byte = load i8, ptr %last
  %wide = zext i8 %byte to i16
  %s0 = add i16 %a2, %b2
  %s1 = add i16 %s0, %c2
  %s2 = add i16 %s1, %d2
  %r = add i16 %s2, %wide
  ret i16 %r
}"#;

/// A call of `memcpy` in a loop with six words live across it left the values
/// that do not fit the preserved pairs in slots for the whole loop: 454 bytes
/// against 294 as a loop, and on lz4 1128 B and 85347 clocks against 580 B and
/// 64957 (sim65). The copy is a loop here.
#[test]
fn a_copy_in_a_loop_with_six_words_live_is_a_loop() {
    let ir = module(COPY_LOOP);
    assert!(ir.contains("@llvm.memcpy"), "the premise: the copy reaches the backend as a call");
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("jsr memcpy"), "{text}");
    let bytes = llrm_6502::timing::estimate(Request::new(Level::O2).cpu, &text).unwrap().bytes;
    assert!(bytes < 350, "{bytes} B\n{text}");
}

/// The loop made from a copy runs: the records' payloads land in `dst`, the
/// last of them (15) is added to the sums of the four words (7).
#[test]
fn the_copy_loop_made_from_a_call_runs() {
    let text = assembly(&module(COPY_LOOP), &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("jsr memcpy"), "the premise: the loop ran, not the routine\n{text}");
    assert_eq!(exit_code_of(&text, "records"), 22, "{text}");
}

/// A copy with nothing live across it is the call: 20 bytes against 78 as a
/// loop. A second caller shares the library's bytes: alone, the loop is the
/// smaller image.
#[test]
fn a_copy_with_nothing_live_across_it_stays_a_call() {
    let ir = module(
        r#"declare void @llvm.memcpy.p0.p0.i16(ptr noalias, ptr noalias, i16, i1)
define void @copy(ptr noalias %d, ptr noalias %s, i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}
define void @copy_again(ptr noalias %d, ptr noalias %s, i16 %n) {
  call void @llvm.memcpy.p0.p0.i16(ptr %d, ptr %s, i16 %n, i1 false)
  ret void
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("jsr memcpy") || text.contains("jmp memcpy"), "{text}");
}

/// lz4 with a call in its loop: 14 bytes are live across it and only four
/// pairs survive a call, so the rest sat in slots for the whole function, 1038
/// bytes against 542 without the call. Kept in a slot across the call only
/// (RAGreedy's split around a call), 929.
const LZ4_CALL: &str = include_str!("bench/lz4_call.ll");

#[test]
fn registers_live_across_a_call_are_in_a_slot_across_it_only() {
    let ir = module(LZ4_CALL);
    assert!(ir.contains("call void @touch()"), "the premise: a call in the loop");
    let request = Request::new(Level::O2);
    let text = assembly(&ir, &request).unwrap();
    let bytes = llrm_6502::timing::estimate(request.cpu, &text).unwrap().bytes;
    assert!(bytes < 1000, "{bytes} B\n{text}");
}

/// The values stored before the call and loaded after it are the ones the loop
/// reads: one literal `A`, then a 4-byte match at distance 1 makes five, and
/// the fifth is `A` (65): 5 + 65.
#[test]
fn lz4_with_a_call_still_decodes() {
    let ir = module(&format!(
        r#"{LZ4_CALL}
@inp = internal global [4 x i8] c"\10A\01\00"
@out = internal global [8 x i8] zeroinitializer
define i8 @run() {{
  %n = call i16 @lz4(ptr @inp, ptr @out, i16 5)
  %last = getelementptr i8, ptr @out, i16 4
  %b = load i8, ptr %last
  %n8 = trunc i16 %n to i8
  %r = add i8 %n8, %b
  ret i8 %r
}}"#
    ));
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert_eq!(exit_code_of(&format!("{text}\n.export touch\ntouch:\n  rts\n"), "run"), 70, "{text}");
}

/// A counted loop compares its index, which lives in `x`. It went through the
/// accumulator, `txa; cmp rc2` (two bytes and two clocks a trip over
/// `cpx rc2`); 163 compares of the corpus' loops did. The bound is the second
/// byte argument (`x`): as the first (`a`) the allocator carries the index in
/// `a` (`tax; sta buf,x; clc; adc #1; cmp rc2`), as it does for a bound loaded
/// from a global; it does not price `inx` against `clc; adc #1`.
#[test]
fn a_loop_bound_in_x_is_compared_with_cpx() {
    let ir = module(
        "@buf = global [16 x i8] zeroinitializer\n\
         define void @f(i8 %unused, i8 %n) {\n\
         entry:\n  br label %loop\n\
         loop:\n  %i = phi i8 [ 0, %entry ], [ %j, %loop ]\n  \
         %z = zext i8 %i to i16\n  %p = getelementptr i8, ptr @buf, i16 %z\n  store i8 %i, ptr %p\n  \
         %j = add i8 %i, 1\n  %c = icmp ne i8 %j, %n\n  br i1 %c, label %loop, label %exit\n\
         exit:\n  ret void\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("cpx") || text.contains("cpy"), "no cpx:\n{text}");
    assert!(!text.contains("txa\n  cmp") && !text.contains("tya\n  cmp"), "compared through a:\n{text}");
}

/// A function that returns what its last call returned was `jsr f; rts`: four
/// bytes and nine clocks over `jmp f`, and a stack level deeper (`mul8`, 12
/// clocks of 205).
#[test]
fn a_call_in_return_position_is_a_jump() {
    let ir = module("define i8 @f(i8 %a, i8 %b) {\n  %m = mul i8 %a, %b\n  ret i8 %m\n}");
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("jmp __mulqi3") && !text.contains("jsr __mulqi3"), "no tail call:\n{text}");
}

const COPY_UNTIL_ZERO: &str = "define ptr @cp(ptr %d, ptr %s) {
entry:
  br label %loop
loop:
  %d1 = phi ptr [ %d, %entry ], [ %d2, %loop ]
  %s1 = phi ptr [ %s, %entry ], [ %s2, %loop ]
  %c = load i8, ptr %s1
  store i8 %c, ptr %d1
  %s2 = getelementptr i8, ptr %s1, i16 1
  %d2 = getelementptr i8, ptr %d1, i16 1
  %z = icmp ne i8 %c, 0
  br i1 %z, label %loop, label %exit
exit:
  ret ptr %d1
}";

/// A copy up to the terminator loaded `y` twice with the same constant
/// (`ldy #0; lda (s),y; ldy #0; sta (d),y`) and then tested the byte it had
/// just loaded with `cmp #0`, because the second `ldy` stood between the load
/// and the test: 4 bytes and 6 clocks a trip (`stpcpy`, 68 B against 30).
#[test]
fn a_register_holding_the_constant_is_not_loaded_again() {
    let text = assembly(&module(COPY_UNTIL_ZERO), &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("cmp #0"), "tests a byte the load set the flags of:\n{text}");
    assert_eq!(text.matches("ldy #0").count(), 1, "y loaded with 0 more than once:\n{text}");
}

/// The sum of two globals loaded one after the other was `lda g; sta rc2; lda
/// g+1; clc; adc rc2`: a byte and three clocks over `lda g; clc; adc g+1`
/// (`sum4`, 28 bytes unrolled against 16).
#[test]
fn a_global_read_once_by_an_add_is_its_memory_operand() {
    let ir = module(
        "@g = global [4 x i8] zeroinitializer\n\
         define i8 @f() {\n  %a = load i8, ptr @g\n  %p = getelementptr i8, ptr @g, i16 1\n  \
         %b = load i8, ptr %p\n  %s = add i8 %a, %b\n  ret i8 %s\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("adc g+1") || text.contains("adc g\n"), "no memory operand:\n{text}");
    assert!(!text.contains("sta rc"), "a load went through a cell:\n{text}");
}

/// A store between the load and the add must not change what the add reads:
/// the folded add runs before the store (the load's place in the chain).
#[test]
fn a_store_between_a_load_and_its_add_does_not_change_what_the_add_reads() {
    let ir = module(
        "@g = global [4 x i8] zeroinitializer\n\
         define i8 @f(i8 %x) {\n  %a = load i8, ptr @g\n  store i8 %x, ptr @g\n  %b = add i8 %a, %x\n  ret i8 %b\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let (read, written) = (text.find("adc g"), text.find("sta g"));
    assert!(read.is_some() && read < written, "the add reads g after the store:\n{text}");
}

/// A copy up to the terminator ran its pointer bumps and then `jmp` back to
/// the test at its top: the jump is 3 clocks and 3 bytes a trip. The test
/// sits at the bottom and the loop closes with its branch; one `jmp` at the
/// entry is all that is left (`strcpy`, 40 clocks a byte).
#[test]
fn a_loop_with_its_test_at_the_top_is_laid_out_with_it_at_the_bottom() {
    let text = assembly(&module(COPY_UNTIL_ZERO), &Request::new(Level::O2)).unwrap();
    assert!(text.contains("  jmp cp_b1\ncp_b3:"), "entered by falling into the test:\n{text}");
    assert!(text.contains("  bne cp_b3\n"), "the loop is not closed by its branch:\n{text}");
}

/// A copy up to the terminator stepped both pointers with `inc lo; bne; inc hi`
/// each trip, 8 clocks a pointer on top of the 11 of the load and the store
/// (`strcpy`, 420 clocks for 11 bytes, hand-written 341). Walked by a byte
/// index in `y`, with the pointers' high bytes stepped where it wraps, the
/// trip is `lda (s),y; sta (d),y; iny; bne`: 277.
#[test]
fn a_pointer_walk_is_a_byte_index_and_a_page_bump() {
    let strcpy = COPY_UNTIL_ZERO.replace("ret ptr %d1", "ret ptr %d");
    let text = assembly(&module(&strcpy), &Request::new(Level::O2)).unwrap();
    assert!(text.contains("  iny\n"), "no byte index:\n{text}");
    assert!(!text.contains("_pa1"), "a pointer still steps by its low byte:\n{text}");
}

/// `x >> 7` of a word was seven `lsr; ror` pairs on two cells for its low byte
/// (112 clocks); the byte is the high one of `x << 1`, one `asl; rol` (the
/// hand-written `shrax7` is 28).
#[test]
fn a_word_shifted_right_by_seven_is_one_shift_left() {
    let ir = module("define i16 @shr7(i16 %x) {\n  %r = lshr i16 %x, 7\n  ret i16 %r\n}");
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("lsr rc") && text.matches("ror").count() == 0, "seven right shifts:\n{text}");
}

/// A run of shifts of a word keeps the byte that shifts alone in `a`: `asl a;
/// rol hi` is 7 clocks a step, `asl lo; rol hi` is 10.
#[test]
fn a_run_of_word_shifts_carries_one_byte_in_the_accumulator() {
    let ir = module("define i16 @shr3(i16 %x) {\n  %r = lshr i16 %x, 3\n  ret i16 %r\n}");
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("lsr a\n  ror rc2"), "the shifts go through memory:\n{text}");
}

/// gcd's `if (a > b) a -= b; else b -= a;` was folded into two selects
/// (`a -= a > b ? b : 0`), each a branch here after all: 47 bytes, 239 clocks
/// a call where llvm-mos and SDCC have 36 to 37 bytes and 127 to 140 clocks.
/// The 6502 has no conditional move, so the fold prices its selects.
#[test]
fn an_if_else_of_two_subtracts_stays_a_branch() {
    let ir = module(
        "define i8 @gcd(i8 %a0, i8 %b0) {
entry:
  %e = icmp eq i8 %a0, %b0
  br i1 %e, label %done, label %loop
loop:
  %b = phi i8 [ %b2, %latch ], [ %b0, %entry ]
  %a = phi i8 [ %a2, %latch ], [ %a0, %entry ]
  %gt = icmp ugt i8 %a, %b
  br i1 %gt, label %then, label %else
then:
  %s = sub nuw i8 %a, %b
  br label %latch
else:
  %u = sub nuw i8 %b, %a
  br label %latch
latch:
  %a2 = phi i8 [ %s, %then ], [ %a, %else ]
  %b2 = phi i8 [ %b, %then ], [ %u, %else ]
  %same = icmp eq i8 %a2, %b2
  br i1 %same, label %done, label %loop
done:
  %r = phi i8 [ %a0, %entry ], [ %a2, %latch ]
  ret i8 %r
}",
    );
    let request = Request::new(Level::O2);
    let text = assembly(&ir, &request).unwrap();
    let priced = llrm_6502::timing::estimate(request.cpu, &text).unwrap();
    assert!(priced.bytes <= 42, "{} bytes, the two-select form is 47:\n{text}", priced.bytes);
    // the equates are the platform's, `run_with` states them
    let function: String =
        text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    for (a, b, want) in [(48, 18, 6), (7, 13, 1), (9, 9, 9)] {
        let setup = format!("  ldx #{b}\n  lda #{a}\n  jsr gcd\n");
        assert_eq!(common::run_with(&format!("driver-gcd-{a}"), &setup, &function), want);
    }
}

/// A CRC-16 step tests bit 15 and shifts left. The test was `ldx rc3; bpl`
/// after the shift had already put that bit in the carry (crc16: 14 bytes and
/// 20% over a hand bit loop); the branch now reads the carry.
#[test]
fn a_crc_step_branches_on_the_carry_of_its_shift() {
    let ir = module(
        "define i16 @step(i16 %c) {\n  %t = and i16 %c, 32768\n  %b = icmp ne i16 %t, 0\n  \
         %s = shl i16 %c, 1\n  %x = xor i16 %s, 4129\n  %r = select i1 %b, i16 %x, i16 %s\n  ret i16 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("bcc") || text.contains("bcs"), "no branch on the carry:\n{text}");
    assert!(!text.contains("bpl") && !text.contains("bmi"), "a sign branch is left:\n{text}");
}

/// A Galois LFSR step tests bit 0 (`and #1`) and shifts right. The test
/// followed a `lsr`/`ror` that had put that bit in the carry; the branch reads
/// the carry, and the step still computes the register (not a corpus row).
#[test]
fn an_lfsr_step_branches_on_the_carry_of_its_right_shift() {
    let ir = module(
        "@odd = global i16 4661\n@even = global i16 4660\n\
         define i16 @lfsr(i16 %x) noinline {\n  %t = and i16 %x, 1\n  %b = icmp ne i16 %t, 0\n  \
         %s = lshr i16 %x, 1\n  %y = xor i16 %s, 46080\n  %r = select i1 %b, i16 %y, i16 %s\n  ret i16 %r\n}\n\
         define i8 @high_of_odd() {\n  %v = load volatile i16, ptr @odd\n  %r = call i16 @lfsr(i16 %v)\n  \
         %h = lshr i16 %r, 8\n  %t = trunc i16 %h to i8\n  ret i8 %t\n}\n\
         define i8 @low_of_even() {\n  %v = load volatile i16, ptr @even\n  %r = call i16 @lfsr(i16 %v)\n  \
         %t = trunc i16 %r to i8\n  ret i8 %t\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let step: String =
        text.lines().skip_while(|l| *l != "lfsr:").take_while(|l| !l.contains("rts")).collect::<Vec<_>>().join("\n");
    assert!(step.contains("bcc") || step.contains("bcs"), "no branch on the carry:\n{step}");
    assert!(!step.contains("and #1"), "the bit is tested again:\n{step}");
    assert_eq!(exit_code_of(&text, "high_of_odd"), 0xBD, "{text}");
    assert_eq!(exit_code_of(&text, "low_of_even"), 0x1A, "{text}");
}

/// `stpcpy`'s copy loop (a pointer walk with an exit phi that names the entry
/// and the loop) made the paged variant a function the selector refused ("phi
/// without an input from a predecessor"), and the whole compile failed: the
/// k_stpcpy cell of the hand corpus was BROKEN on the #6302 branch.
#[test]
fn a_copy_until_zero_loop_with_an_exit_phi_compiles() {
    let ir = module(
        "define ptr @k_stpcpy(ptr %dst, ptr %src) {\n\
         entry:\n  %c = load i8, ptr %src\n  store i8 %c, ptr %dst\n  %z = icmp eq i8 %c, 0\n  \
         br i1 %z, label %done, label %loop\n\
         loop:\n  %s = phi ptr [ %s1, %loop ], [ %src, %entry ]\n  %d = phi ptr [ %d1, %loop ], [ %dst, %entry ]\n  \
         %s1 = getelementptr inbounds i8, ptr %s, i16 1\n  %d1 = getelementptr inbounds i8, ptr %d, i16 1\n  \
         %v = load i8, ptr %s1\n  store i8 %v, ptr %d1\n  %e = icmp eq i8 %v, 0\n  br i1 %e, label %done, label %loop\n\
         done:\n  %r = phi ptr [ %dst, %entry ], [ %d1, %loop ]\n  ret ptr %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).expect("compiles");
    assert!(text.contains("k_stpcpy:"), "{text}");
}

/// A region split left the complement of a loop-carried value a phi-only range
/// with no instruction naming it, and the allocator panicked ("Non-empty but
/// used interval"; 2 of 300 difftest programs). LLVM removes such an interval.
#[test]
fn a_split_that_leaves_a_range_without_uses_compiles() {
    let ir = include_str!("driver/range-left-without-uses.ll");
    let text = assembly(ir, &Request::new(Level::O2)).expect("compiles");
    assert!(text.contains("test:"), "{text}");
}

/// The flag of a signed word overflow op, widened to a byte, sent lowering into
/// a dependence cycle: `lower_extend` made the widened flag a `select flag, 1,
/// 0` before the op was expanded, `condition_byte` had no byte for the op's
/// flag and widened it with the very node being replaced, so the `CMOV`'s flags
/// were a compare of its own result (the scheduler's "never ordered", #6429).
/// Each op's flag is right: `flags` adds the flags of four operand pairs, two
/// that overflow and two that do not.
fn flags_ir(
    op: &str,
    pairs: [(i16, i16); 4],
) -> String {
    let call = |n: usize| format!("  %f{n} = call i8 @flag(i16 {}, i16 {})\n", pairs[n].0, pairs[n].1);
    let ir = module(&format!(
        r#"declare {{ i16, i1 }} @llvm.{op}.with.overflow.i16(i16, i16)
define internal i8 @flag(i16 %a, i16 %b) #0 {{
  %r = call {{ i16, i1 }} @llvm.{op}.with.overflow.i16(i16 %a, i16 %b)
  %o = extractvalue {{ i16, i1 }} %r, 1
  %z = zext i1 %o to i8
  ret i8 %z
}}
define i8 @flags() {{
{}{}{}{}  %t = add i8 %f0, %f1
  %u = add i8 %f2, %f3
  %v = add i8 %t, %u
  ret i8 %v
}}
attributes #0 = {{ noinline }}"#,
        call(0),
        call(1),
        call(2),
        call(3),
    ));
    ir
}

fn flags_sum(
    op: &str,
    pairs: [(i16, i16); 4],
) -> i32 {
    let text = assembly(&flags_ir(op, pairs), &Request::new(Level::O2)).unwrap();
    exit_code_of(&text, "flags")
}

#[test]
fn the_flag_of_a_signed_word_add_with_overflow_widens_to_a_byte_without_a_cycle() {
    assert_eq!(flags_sum("sadd", [(30000, 30000), (-30000, -30000), (30000, -30000), (1, 2)]), 2);
}

#[test]
fn the_flag_of_a_signed_word_subtract_with_overflow_widens_to_a_byte_without_a_cycle() {
    assert_eq!(flags_sum("ssub", [(30000, -30000), (-30000, 30000), (30000, 30000), (1, 2)]), 2);
}

/// The multiply is a call to the runtime's `__mulhi3`, which the test's
/// assembler has no copy of: it compiles, with the call, and is not run.
#[test]
fn the_flag_of_a_signed_word_multiply_with_overflow_widens_to_a_byte_without_a_cycle() {
    let ir = flags_ir("smul", [(300, 300), (-300, 300), (30, 30), (-3, 4)]);
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("__mulhi3"), "{text}");
}

/// `.export` named every function and no variable: two objects with a static
/// `helper` collided at link time (softfloat's add_bits, cmp_bits, from_mag:
/// ld65 "Duplicate external identifier"), and a global another object imports
/// (`counter`) was "Unresolved external". An internal symbol is not exported,
/// an external one is, function or variable.
#[test]
fn only_external_functions_and_variables_are_exported() {
    let ir = module(
        "@counter = global i16 0\n@hidden = internal global i16 0\n\
define internal i8 @helper(i8 %a) #0 {\n  ret i8 %a\n}\n\
define i8 @pub(i8 %a) {\n  %r = call i8 @helper(i8 %a)\n  ret i8 %r\n}\n\
attributes #0 = { noinline }",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let exported: Vec<&str> = text.lines().filter_map(|l| l.strip_prefix(".export ")).collect();
    assert!(exported.contains(&"pub") && exported.contains(&"counter"), "exports {exported:?}:\n{text}");
    assert!(!exported.contains(&"helper") && !exported.contains(&"hidden"), "exports {exported:?}:\n{text}");
}

/// A pair passed to a function and returned from it by value arrives member by
/// member: `swap` returns (b, a), the caller keeps a - b = 7 - 3 of the
/// swapped pair, 3 - 7 = 0xFC.
#[test]
fn an_aggregate_passed_and_returned_by_value_runs() {
    let ir = module(
        r#"define internal { i8, i8 } @swap({ i8, i8 } %s) #0 {
  %a = extractvalue { i8, i8 } %s, 0
  %b = extractvalue { i8, i8 } %s, 1
  %x = insertvalue { i8, i8 } undef, i8 %b, 0
  %y = insertvalue { i8, i8 } %x, i8 %a, 1
  ret { i8, i8 } %y
}
define i8 @probe() {
  %p = insertvalue { i8, i8 } undef, i8 7, 0
  %q = insertvalue { i8, i8 } %p, i8 3, 1
  %r = call { i8, i8 } @swap({ i8, i8 } %q)
  %a = extractvalue { i8, i8 } %r, 0
  %b = extractvalue { i8, i8 } %r, 1
  %d = sub i8 %a, %b
  ret i8 %d
}
attributes #0 = { noinline }"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "probe"), 0xFC, "{text}");
}

/// `x << n` of a 32-bit value by a variable called the bit loop at every level:
/// 25 clocks a bit, 530 for the shift by 21 that f32 add makes on average (18%
/// of that routine). At `-O2` the call is the speed variant, whole bytes first;
/// at `-Os` it stays the 18-byte loop.
#[test]
fn a_variable_long_shift_calls_the_speed_variant_at_o2_only() {
    let ir = module(
        r#"define i32 @shl(i32 %x, i32 %n) {
  %r = shl i32 %x, %n
  ret i32 %r
}
define i32 @shr(i32 %x, i32 %n) {
  %r = lshr i32 %x, %n
  ret i32 %r
}
define i32 @sar(i32 %x, i32 %n) {
  %r = ashr i32 %x, %n
  ret i32 %r
}"#,
    );
    let fast = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    for routine in ["__ashlsi3_fast", "__lshrsi3_fast", "__ashrsi3_fast"] {
        assert!(fast.contains(&format!("{routine}\n")), "no call to {routine} at -O2:\n{fast}");
    }
    let small = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    for routine in ["__ashlsi3", "__lshrsi3", "__ashrsi3"] {
        assert!(small.contains(&format!(" {routine}\n")), "no call to {routine} at -Os:\n{small}");
    }
    assert!(!small.contains("_fast"), "a speed variant at -Os:\n{small}");
}

/// A four-trip loop over a global stayed a loop at `-O2` (22 B, 98 clocks for
/// `sum4`, against 13 B and 34 clocks unrolled): the driver turned the unroller
/// off (DEVIATE 6502-5). The pin unrolls it fully at `-O2`; `buf` holds
/// 1, 2, 3, 4 so the sum is 10.
#[test]
fn a_four_trip_loop_is_unrolled_at_o2() {
    let ir = module(
        r#"@buf = global [4 x i8] [i8 1, i8 2, i8 3, i8 4]
define i8 @sum4() {
entry:
  br label %loop
loop:
  %i = phi i16 [ 0, %entry ], [ %next, %loop ]
  %s = phi i8 [ 0, %entry ], [ %sum, %loop ]
  %p = getelementptr inbounds i8, ptr @buf, i16 %i
  %v = load volatile i8, ptr %p
  %sum = add i8 %s, %v
  %next = add nuw nsw i16 %i, 1
  %done = icmp eq i16 %next, 4
  br i1 %done, label %out, label %loop
out:
  ret i8 %sum
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("bne") && !text.contains("beq"), "the loop is still a loop:\n{text}");
    assert_eq!(exit_code_of(&text, "sum4"), 10, "{text}");
}

/// `buf[n + 3]` and `buf[n - 1]` with a byte `n` took eleven instructions, a
/// 16-bit add of the address and an indirect load: the constant had no place in
/// the indexed form (fill's `buf[n - 1]`: 46 B against llvm-mos's 22). It is
/// one `lda buf+3,x`.
#[test]
fn a_constant_offset_folds_into_the_indexed_load() {
    let ir = module(
        r#"@buf = external global [64 x i8]
define i8 @plus(i8 %n) {
  %z = zext i8 %n to i16
  %m = add i16 %z, 3
  %p = getelementptr inbounds i8, ptr @buf, i16 %m
  %v = load i8, ptr %p
  ret i8 %v
}
define i8 @minus(i8 %n) {
  %z = zext i8 %n to i16
  %m = add i16 %z, -1
  %p = getelementptr inbounds i8, ptr @buf, i16 %m
  %v = load i8, ptr %p
  ret i8 %v
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("lda buf+3,x"), "no indexed load with the offset:\n{text}");
    assert!(text.contains("lda buf-1,x"), "no indexed load with the negative offset:\n{text}");
    assert!(!text.contains("adc"), "the address was added in 16 bits:\n{text}");
}

/// A float argument, result, load, store, select and negation compiled for a
/// target with no floating-point registers: the float is the integer of its
/// width (`TypeSoftenFloat`). `float f(float x, float y) { return x; }`
/// panicked in the call lowering ("legal parts: F32"), so did every float
/// parameter.
#[test]
fn a_float_is_carried_as_the_integer_of_its_width() {
    let request = Request::new(Level::O2);
    let pass = assembly(&module("define float @f(float %x, float %y) {\n  ret float %x\n}"), &request).unwrap();
    // x arrives in a:x and rc2:rc3 and leaves in the same registers: nothing
    // moves
    assert!(!pass.contains("sta") && !pass.contains("ldy"), "the words moved:\n{pass}");
    let copy = "define void @g(ptr %p, ptr %q) {\n  %v = load float, ptr %p\n  store float %v, ptr %q\n  ret void\n}";
    let copied = assembly(&module(copy), &request).unwrap();
    assert_eq!(copied.matches("sta (").count(), 4, "four bytes stored:\n{copied}");
    let negate = "define float @n(float %x) {\n  %r = fneg float %x\n  ret float %r\n}";
    let negated = assembly(&module(negate), &request).unwrap();
    assert!(negated.contains("eor #128"), "the sign bit flipped:\n{negated}");
    let choose =
        "define float @s(i1 %c, float %x, float %y) {\n  %r = select i1 %c, float %x, float %y\n  ret float %r\n}";
    assembly(&module(choose), &request).unwrap();
}

/// A double live across a block is four i16 parts, not two: it stopped at
/// "integer wider than two registers" (five corpus cells, e.g.
/// float_in_place_update).
#[test]
fn a_double_crosses_a_block_as_four_words() {
    let body = "declare void @g()\ndefine double @f(double %x, i16 %n) {\nentry:\n  br label %l\nl:\n  %v = phi double [ %x, %entry ], [ %w, %l ]\n  %i = phi i16 [ 0, %entry ], [ %j, %l ]\n  call void @g()\n  %w = fneg double %v\n  %j = add i16 %i, 1\n  %c = icmp ult i16 %j, %n\n  br i1 %c, label %l, label %x1\nx1:\n  ret double %w\n}";
    assembly(&module(body), &Request::new(Level::O2)).unwrap();
}

/// A float or double global initializer is its bits, little-endian: it stopped
/// at "a global initializer that is not an integer, pointer, array or struct"
/// (five corpus cells, e.g. int_compared_with_float).
#[test]
fn a_float_global_is_emitted_as_its_bits() {
    let text = "@d = global double 1.5\n@f = global float 2.0\ndefine ptr @p() {\n  ret ptr @d\n}\ndefine ptr @q() {\n  ret ptr @f\n}";
    let asm = assembly(&module(text), &Request::new(Level::O2)).unwrap();
    assert!(
        asm.contains(".byte 0,0,0,0,0,0,248,63") && asm.contains(".byte 0,0,0,64"),
        "the bits of 1.5 and 2.0:\n{asm}"
    );
}

/// A float add, a multiply, an int-to-float and a compare are calls of the
/// softfloat routines (`SoftenFloatRes_Binary`, `_XINT_TO_FP`, `softenSetCC`):
/// each stopped at "the target names no routine" or, for the compare, at a
/// `CALL` the selector cannot take (the call was built for x86).
#[test]
fn a_float_operation_is_a_call_of_its_softfloat_routine() {
    let request = Request::new(Level::O2);
    let call = |body: &str, routine: &str| {
        let text = assembly(&module(body), &request).unwrap();
        assert!(
            text.contains(&format!("jsr {routine}")) || text.contains(&format!("jmp {routine}")),
            "{routine}:\n{text}"
        );
    };
    call("define float @f(float %a, float %b) {\n  %r = fadd float %a, %b\n  ret float %r\n}", "__addsf3");
    call("define float @f(float %a, float %b) {\n  %r = fmul float %a, %b\n  ret float %r\n}", "__mulsf3");
    call("define float @f(i32 %n) {\n  %r = sitofp i32 %n to float\n  ret float %r\n}", "__floatsisf");
    call("define i1 @f(float %a, float %b) {\n  %r = fcmp olt float %a, %b\n  ret i1 %r\n}", "__ltsf2");
    // An i32 result takes the `si` routines: `__fixsfdi` was called and never
    // linked.
    call("define i32 @f(float %a) {\n  %r = fptosi float %a to i32\n  ret i32 %r\n}", "__fixsfsi");
    call("define i32 @f(float %a) {\n  %r = fptoui float %a to i32\n  ret i32 %r\n}", "__fixunssfsi");
}

/// The compiled `text` without its `rcN = $..` equates, which `run_with` states
/// itself.
fn without_equates(text: &str) -> String {
    text.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect()
}

/// `srem i32` of two sign-extended words called `__modsi3`, the long routine
/// (recpow at -O1, +434 B): both operands fit a word, so it is `__modhi3`.
/// -7 % 3 = -1 (0xFF), and -32768 % -1 is 0.
#[test]
fn a_remainder_of_two_sign_extended_words_is_the_word_routine() {
    let ir = module(
        r#"define i8 @rem(i16 %a, i16 %b) {
  %x = sext i16 %a to i32
  %y = sext i16 %b to i32
  %r = srem i32 %x, %y
  %t = trunc i32 %r to i8
  ret i8 %t
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__modhi3") && !text.contains("__modsi3"), "the long routine:\n{text}");
    let function = without_equates(&text);
    for (a, b, want) in [(-7i16, 3i16, 0xFF), (-32768, -1, 0), (1000, -7, 6)] {
        let setup = format!(
            "  lda #{}\n  sta rc2\n  lda #{}\n  sta rc3\n  ldx #{}\n  lda #{}\n  jsr rem\n",
            b as u16 & 0xFF,
            (b as u16) >> 8,
            (a as u16) >> 8,
            a as u16 & 0xFF
        );
        assert_eq!(common::run_with(&format!("narrow-rem-{a}-{b}"), &setup, &function), want, "{a} % {b}");
    }
}

/// -32768 / -1 is 32768 in 32 bits and overflows 16: a divide of two
/// sign-extended words stays long (its high byte is 0x80).
#[test]
fn a_divide_of_sign_extended_words_keeps_the_long_quotient() {
    let words = module(
        r#"define i8 @high(i16 %a, i16 %b) {
  %x = sext i16 %a to i32
  %y = sext i16 %b to i32
  %q = sdiv i32 %x, %y
  %h = lshr i32 %q, 8
  %t = trunc i32 %h to i8
  ret i8 %t
}"#,
    );
    let text = assembly(&words, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__divsi3") && !text.contains("__divhi3"), "narrowed:\n{text}");
    let setup = "  lda #$FF\n  sta rc2\n  sta rc3\n  ldx #$80\n  lda #0\n  jsr high\n";
    assert_eq!(common::run_with("narrow-div-min", setup, &without_equates(&text)), 0x80);
}

/// A call result whose callee returns `range(i32 -30010, 30011)` fits a word
/// (recpow at -O1, whose `srem` of such results called `__modsi3`): the range
/// is the sign bits of the result, so the remainder is the word routine. The
/// callee is opaque (hand-written, returning -12345 in A, X, rc2, rc3) and the
/// level is -O0, where no correlated-propagation narrows it first;
/// -12345 % 1000 = -345, low byte 0xA7.
#[test]
fn a_remainder_of_a_result_with_a_signed_range_is_the_word_routine() {
    let ir = module(
        r#"declare range(i32 -30010, 30011) i32 @source()
define i8 @rem() {
  %v = call i32 @source()
  %r = srem i32 %v, 1000
  %t = trunc i32 %r to i8
  ret i8 %t
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O0)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("__modhi3") && !text.contains("__modsi3"), "the long routine:\n{text}");
    let source = "source:\n  ldx #$CF\n  lda #$FF\n  sta rc2\n  sta rc3\n  lda #$C7\n  rts\n";
    let function = format!("{}{source}", without_equates(&text));
    assert_eq!(common::run_with("narrow-range-rem", "  jsr rem\n", &function), 0xA7);
}

/// `x > 65535` and `x < 65536` are one test of the high word. The low word of
/// the first was `65535 - lo` (`ldx #255` and a swapped `cmp`), of the second
/// `lo - 0`: the pin folds `X > MAX` and `X < MIN` to false, and a half known
/// false is dropped. The code of a softfloat `sf_clz32` caller changed with the
/// polarity InstCombine happened to pick (`__divsf3` +14 lines). The arms call
/// different functions: with one callee the pin merges them into `g(2 - c)`,
/// and the `sbc` that merge leaves read as a compare chain (red on release/next
/// 5a1802c07e).
#[test]
fn a_compare_against_a_word_boundary_tests_only_the_high_word_in_both_polarities() {
    let branch = |compare: &str, then: &str, other: &str| {
        module(&format!(
            "declare i8 @g(i8)\ndeclare i8 @h(i8)\ndefine i8 @f(i32 %x) {{\n  %c = icmp {compare}\n  br i1 %c, label %{then}, label %{other}\nt:\n  %a = call i8 @g(i8 1)\n  ret i8 %a\ne:\n  %b = call i8 @h(i8 2)\n  ret i8 %b\n}}"
        ))
    };
    let instructions = |text: &str| without_equates(text).lines().filter(|l| l.starts_with("  ")).count();
    let greater = assembly(&branch("ugt i32 %x, 65535", "t", "e"), &Request::new(Level::O1)).unwrap();
    let less = assembly(&branch("ult i32 %x, 65536", "e", "t"), &Request::new(Level::O1)).unwrap();
    for text in [&greater, &less] {
        assert!(!text.contains("ldx #255") && !text.contains("sbc"), "a word compared with its bound:\n{text}");
    }
    assert_eq!(instructions(&greater), instructions(&less), "the polarity changed the code:\n{greater}\n{less}");
}

/// `side = a < b ? 1 : 2` reaches the DAG as `add (sext c), 2`. The pin turns
/// it into `sub 2, (zext c)`, which the 6502 builds as a borrow chain:
/// `phi_slot_unknown_arm` O1 read 5 bytes longer and 0.9% slower with it. The
/// add stays.
#[test]
fn an_add_of_a_sign_extended_compare_stays_an_add() {
    let ir = module(
        "define i8 @f(i8 %a, i8 %b) {\n  %c = icmp ult i8 %a, %b\n  %s = sext i1 %c to i8\n  %r = add i8 %s, 2\n  ret i8 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap();
    assert!(text.contains("adc #2") && !text.contains("sbc"), "the add of a sign-extended compare:\n{text}");
}

/// Wider than 16 bits the add of a sign-extended compare read its sign bytes
/// from a cell (`adc rc22`, 3 clk each); the pin's `sub x, (zext c)` ends in
/// `sbc #0` (2 clk). `compare_to_int64` O1 read 48 clocks slower with the add.
#[test]
fn a_wide_add_of_a_sign_extended_compare_takes_the_pins_subtract() {
    let ir = module(
        "define i64 @f(i64 %x, i8 %a, i8 %b) {\n  %c = icmp ult i8 %a, %b\n  %s = sext i1 %c to i64\n  %r = add i64 %s, %x\n  ret i64 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap();
    assert!(text.contains("sbc #0"), "the wide add of a sign-extended compare:\n{text}");
}

/// A softened `double` bitcast to `i64` takes the halves of its softened
/// integer (`ExpandIntRes_BITCAST`), not a stack slot. Guards the bytes of
/// 0x3FF8000000000001: 0x3F on top, 0xF8 next, 0x01 at the bottom.
#[test]
fn a_double_bitcast_to_i64_keeps_its_bytes() {
    let ir = module(
        r#"@d = global double 0x3FF8000000000001
define i8 @top() {
  %x = load double, ptr @d
  %i = bitcast double %x to i64
  %t = lshr i64 %i, 56
  %b = trunc i64 %t to i8
  ret i8 %b
}
define i8 @mid() {
  %x = load double, ptr @d
  %i = bitcast double %x to i64
  %t = lshr i64 %i, 48
  %b = trunc i64 %t to i8
  ret i8 %b
}
define i8 @bottom() {
  %x = load double, ptr @d
  %i = bitcast double %x to i64
  %b = trunc i64 %i to i8
  ret i8 %b
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert_eq!(exit_code_of(&text, "top"), 0x3F, "{text}");
    assert_eq!(exit_code_of(&text, "mid"), 0xF8, "{text}");
    assert_eq!(exit_code_of(&text, "bottom"), 0x01, "{text}");
}

/// A 64-bit add carried between its words by compares: the legalizer tested the
/// carry operation at the 32-bit half, which is no legal type here, instead of
/// the 16-bit piece it expands to, so `a + b` took 163 instructions with a
/// `cmp`/`bcc` pair per carry. It is an `adc` chain; `0x1_FFFF_FFFF + 1` has
/// high word 2.
#[test]
fn a_64_bit_add_is_a_carry_chain_not_compares() {
    let ir = module(
        r#"@a = internal global i64 8589934591
@b = internal global i64 1
define i8 @probe() {
  %x = load volatile i64, ptr @a
  %y = load volatile i64, ptr @b
  %s = add i64 %x, %y
  %h = lshr i64 %s, 32
  %t = trunc i64 %h to i8
  ret i8 %t
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("cmp"), "a carry made by a compare:\n{text}");
    assert_eq!(exit_code_of(&text, "probe"), 2, "{text}");
}

/// The subtract of the same: a borrow made by a compare in each limb, 165
/// instructions for `a - b` of two arguments (119 as a chain).
#[test]
fn a_64_bit_subtract_of_arguments_is_a_borrow_chain_not_compares() {
    let ir = module(
        r#"define i64 @sub(i64 %a, i64 %b) {
  %s = sub i64 %a, %b
  ret i64 %s
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("cmp"), "a borrow made by a compare:\n{text}");
}

/// The overflow flag of a 64-bit add, returned: the scheduler panicked on a
/// dependence cycle ("units [1, 2, 3, 4] are never ordered") once the add
/// expanded into a carry chain, whose last carry the return reads as a byte.
/// `2^64 - 1 + 1` overflows, `1 + 1` does not.
#[test]
fn the_overflow_flag_of_a_64_bit_add_is_read_from_the_carry_chain() {
    let ir = module(
        r#"declare { i64, i1 } @llvm.uadd.with.overflow.i64(i64, i64)
@a = internal global i64 -1
@b = internal global i64 1
define i8 @wraps() {
  %x = load volatile i64, ptr @a
  %y = load volatile i64, ptr @b
  %r = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %x, i64 %y)
  %o = extractvalue { i64, i1 } %r, 1
  %z = zext i1 %o to i8
  ret i8 %z
}
define i8 @fits() {
  %y = load volatile i64, ptr @b
  %r = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %y, i64 %y)
  %o = extractvalue { i64, i1 } %r, 1
  %z = zext i1 %o to i8
  ret i8 %z
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "wraps"), 1, "{text}");
    assert_eq!(exit_code_of(&text, "fits"), 0, "{text}");
}

/// A borrow compared with a byte was loaded back into C for a `cmp` that takes
/// a byte: `llrm-6502-llc` refused X16 BASIC's `fp_fin` at -O2 ("operand c"),
/// and with it all of mathlib.c. The fixture is that function cut to the 60
/// blocks that still refused; it compiles, and the compare reads the saved
/// 0 or 1.
#[test]
fn a_borrow_compared_with_a_byte_is_the_saved_byte_not_c() {
    let ir = module(include_str!("data/borrow_compared_with_a_byte.ll"));
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("fp_fin:"), "{text}");
}

/// The carry between the two halves of a 32-bit add was made into a byte and
/// read back (`lda #0; rol a; lsr a`), and the carry out of the last half into
/// a byte nothing read. `0x1FFFF + 1` has byte 2 equal to 2.
#[test]
fn a_wide_add_keeps_its_carry_in_c() {
    let ir = module(
        r#"@a = internal global i32 131071
@b = internal global i32 1
define i8 @probe() {
  %x = load volatile i32, ptr @a
  %y = load volatile i32, ptr @b
  %s = add i32 %x, %y
  %h = lshr i32 %s, 16
  %t = trunc i32 %h to i8
  ret i8 %t
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("rol a"), "a carry made into a byte:\n{text}");
    assert_eq!(exit_code_of(&text, "probe"), 2, "{text}");
}

/// A word sum of two bytes and a carry (`s = a + b + c; c = s >> 8`, the
/// extended-precision `adc` of a C helper) was two byte adds and a third for
/// the carry, each carry a byte of its own: 100 instructions for a four-byte
/// add, not 14. It is `adc` on the bytes. `0xFF + 1` carries into `0xFE + 1 +
/// 1` = 0x100: low byte 0, carry 1; `1 + 1` does not, and `0xFE + 1` is 0xFF.
#[test]
fn a_word_sum_of_bytes_and_a_carry_is_an_adc() {
    let probe = |name: &str, a0: u8, b0: u8, ret: &str| {
        format!(
            r#"define i8 @{name}() {{
  %a0 = load volatile i8, ptr @a0
  %b0 = load volatile i8, ptr @b0
  %a1 = load volatile i8, ptr @a1
  %b1 = load volatile i8, ptr @b1
  %s0 = add i8 %a0, %b0
  %o0 = icmp ult i8 %s0, %b0
  %c0 = zext i1 %o0 to i16
  %x = zext i8 %a1 to i16
  %y = zext i8 %b1 to i16
  %t = add nuw nsw i16 %c0, %x
  %u = add nuw nsw i16 %t, %y
  %hi = lshr i16 %u, 8
  %lo = trunc i16 %u to i8
  %c = trunc i16 %hi to i8
  ret i8 %{ret}
}}"#
        )
    };
    let globals = |a0: u8, b0: u8| {
        format!(
            "@a0 = internal global i8 {a0}\n@b0 = internal global i8 {b0}\n@a1 = internal global i8 -2\n@b1 = internal global i8 1\n"
        )
    };
    for (a0, b0, lo, carry) in [(255u8, 1u8, 0, 1), (1, 1, 255, 0)] {
        for (name, ret, want) in [("lo", "lo", lo), ("carry", "c", carry)] {
            let ir = module(&format!("{}{}", globals(a0, b0), probe(name, a0, b0, ret)));
            let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(exit_code_of(&text, name), want, "{a0}+{b0}:\n{text}");
            let body = text.split(&format!("{name}:")).nth(1).unwrap_or_default();
            let body = body.split("rts").next().unwrap_or_default();
            let count = body.lines().filter(|l| l.starts_with("  ") && !l.starts_with("  .")).count();
            assert!(count <= 30, "{count} instructions for a carry through a word sum:\n{text}");
        }
    }
}

/// A size build copied a 20-instruction callee into every caller: the pin's
/// `OptSizeThreshold` allows growth, and the 6502 expands each i16 update to a
/// dozen bytes where the call is three. On X16 BASIC, `chrget` went into nine
/// callers (84 B each) and the 24 files were 34094 B, 28730 B with the bound.
/// `step` is called from three places and stays one body.
#[test]
fn a_size_build_does_not_copy_a_callee_bigger_than_its_call() {
    let updates: String = (0..6)
        .map(|i| {
            format!("  %v{i} = load i16, ptr @g{i}\n  %w{i} = add i16 %v{i}, {}\n  store i16 %w{i}, ptr @g{i}\n", 7 + i)
        })
        .collect();
    let globals: String = (0..6).map(|i| format!("@g{i} = global i16 0\n")).collect();
    let ir = module(&format!(
        "{globals}define void @step() {{\n{updates}  ret void\n}}\ndefine void @one() {{\n  call void @step()\n  ret void\n}}\n\
         define void @two() {{\n  call void @step()\n  call void @step()\n  ret void\n}}"
    ));
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    let calls = text.matches("jsr step").count() + text.matches("jmp step").count();
    assert_eq!(calls, 3, "step was copied into its callers:\n{text}");
}

/// The bound above stood in front of an inline the pin's own cost counts as
/// shrinking (`to_double` at cost -30): `classify` kept its calls, so main
/// could not fold its constants: 185 B grew to 1708 B (tests/run/c
/// uint64_to_float). The fixture is that program's IR.
#[test]
fn a_size_build_still_inlines_what_the_cost_model_finds_negative() {
    let ir = module(include_str!("data/uint64_to_float_chain.ll"));
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    let lines = text.lines().count();
    assert!(lines < 300, "{lines} lines: classify kept its calls and main its constants unfolded:\n{text}");
}

/// A loaded `i1` that a `select` or an extension tests was selected into a
/// compare of its own zero extension: `llrm-6502-llc` ran until it had taken
/// 217 GB. A bit in memory is a byte, `x & 1`. `pick` is 7 for a stored 1 and
/// 9 for a stored 0; `mask` is a sign-extended 1; `bump` stores the bit back.
#[test]
fn a_loaded_bit_is_a_condition_a_select_and_an_extension_can_test() {
    let ir = module(
        r#"@t = global i8 1
@f = global i8 0
define i8 @pick() {
  %c = load i1, ptr @t
  %d = load i1, ptr @f
  %a = select i1 %c, i16 7, i16 1
  %b = select i1 %d, i16 100, i16 2
  %s = add i16 %a, %b
  %r = trunc i16 %s to i8
  ret i8 %r
}
define i8 @mask() {
  %c = load i1, ptr @t
  %s = sext i1 %c to i8
  %d = load i1, ptr @f
  %z = zext i1 %d to i8
  %r = add i8 %s, %z
  ret i8 %r
}
define i8 @bump() {
  %c = load i1, ptr @t
  store i1 %c, ptr @f
  %d = load i8, ptr @f
  ret i8 %d
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "pick"), 9, "{text}");
    assert_eq!(exit_code_of(&text, "mask"), 0xFF, "{text}");
    assert_eq!(exit_code_of(&text, "bump"), 1, "{text}");
}

/// `llvm.uadd.with.overflow.i16` and `usub` aborted the compiler the same way:
/// the overflow bit is an `i1` leaf. 65535 + 2 wraps to 1 and sets it.
#[test]
fn a_word_add_and_subtract_with_overflow_run() {
    let ir = module(
        r#"declare { i16, i1 } @llvm.uadd.with.overflow.i16(i16, i16)
declare { i16, i1 } @llvm.usub.with.overflow.i16(i16, i16)
@x = global i16 65535
@y = global i16 2
define i8 @add_overflow() {
  %a = load i16, ptr @x
  %b = load i16, ptr @y
  %p = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %b)
  %v = extractvalue { i16, i1 } %p, 0
  %o = extractvalue { i16, i1 } %p, 1
  %lo = trunc i16 %v to i8
  %ob = zext i1 %o to i8
  %r = add i8 %lo, %ob
  ret i8 %r
}
define i8 @sub_overflow() {
  %a = load i16, ptr @y
  %b = load i16, ptr @x
  %p = call { i16, i1 } @llvm.usub.with.overflow.i16(i16 %a, i16 %b)
  %v = extractvalue { i16, i1 } %p, 0
  %o = extractvalue { i16, i1 } %p, 1
  %lo = trunc i16 %v to i8
  %ob = zext i1 %o to i8
  %r = add i8 %lo, %ob
  ret i8 %r
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    // 1 + 1; 2 - 65535 = 3 with a borrow: 3 + 1
    assert_eq!(exit_code_of(&text, "add_overflow"), 2, "{text}");
    assert_eq!(exit_code_of(&text, "sub_overflow"), 4, "{text}");
}

/// A 64-bit multiply stays inline on the 6502: it names no size routine for the
/// product (DEVIATE in `expand_mul`, exit when the call is measured), so
/// `release/next` makes no `__muldi3` call. #6858 asserted the call and went
/// red there. The routine in the module is the shift-and-add loop a C library
/// would hold, and must stay uncalled; the inline path is `__mulsi3` calls.
#[test]
fn a_64_bit_multiply_stays_inline_until_the_call_is_measured() {
    let ir = module(
        r#"@a = internal global i64 4294967299
@b = internal global i64 4294967297
define i64 @__muldi3(i64 %a, i64 %b) {
entry:
  br label %loop
loop:
  %acc = phi i64 [ 0, %entry ], [ %acc2, %loop ]
  %x = phi i64 [ %a, %entry ], [ %x2, %loop ]
  %y = phi i64 [ %b, %entry ], [ %y2, %loop ]
  %bit = and i64 %y, 1
  %mask = sub i64 0, %bit
  %add = and i64 %x, %mask
  %acc2 = add i64 %acc, %add
  %x2 = shl i64 %x, 1
  %y2 = lshr i64 %y, 1
  %done = icmp eq i64 %y2, 0
  br i1 %done, label %out, label %loop
out:
  ret i64 %acc2
}
define i8 @probe() {
  %x = load volatile i64, ptr @a
  %y = load volatile i64, ptr @b
  %p = mul i64 %x, %y
  %h = lshr i64 %p, 32
  %t = trunc i64 %h to i8
  ret i8 %t
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("jsr __muldi3"), "the product called the routine:\n{text}");
    assert!(text.contains("jsr __mulsi3"), "the product was not expanded through 32-bit multiplies:\n{text}");
}

/// A variable `i64` shift is a call to libgcc's `__ashldi3 (u64, int)`. The
/// hook that makes the call was missing: `u64.c` (`shl`, `mul`) was refused
/// ("no library call"), so no program shifting or multiplying a `long long`
/// compiled. The routine here is the loop a C library would hold; the call
/// passes the value as four words and the count as a word, and `3 << 37 >> 32`
/// is 96.
#[test]
fn a_variable_i64_shift_calls_the_library_routine() {
    let ir = module(
        r#"@k = internal global i8 37
define i64 @__ashldi3(i64 %a, i16 %n) {
entry:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %done, label %loop
loop:
  %x = phi i64 [ %a, %entry ], [ %y, %loop ]
  %i = phi i16 [ %n, %entry ], [ %j, %loop ]
  %y = shl i64 %x, 1
  %j = sub i16 %i, 1
  %c = icmp eq i16 %j, 0
  br i1 %c, label %done, label %loop
done:
  %r = phi i64 [ %a, %entry ], [ %y, %loop ]
  ret i64 %r
}
define i8 @probe() {
  %k = load volatile i8, ptr @k
  %n = zext i8 %k to i64
  %v = shl i64 3, %n
  %h = lshr i64 %v, 32
  %t = trunc i64 %h to i8
  ret i8 %t
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("jsr __ashldi3"), "no call:\n{text}");
    assert_eq!(exit_code_of(&text, "probe"), 96, "{text}");
}

/// `__builtin_clz` and `__builtin_ctz` (`llvm.ctlz.i32`, `llvm.cttz.i32`)
/// failed to compile for the 6502 ("CtLzZeroPoison ... convert node to
/// libcall"): any C program using them was lost. `check` counts the results
/// that are wrong for 0, 1, 0x80000000 and 0xFFFFFFFF, with and without the
/// zero-is-poison flag.
#[test]
fn a_32_bit_clz_and_ctz_count_correctly() {
    let ir = module(
        r#"@v0 = global i32 0
@v1 = global i32 1
@v2 = global i32 2147483648
@v3 = global i32 4294967295
declare i32 @llvm.ctlz.i32(i32, i1)
declare i32 @llvm.cttz.i32(i32, i1)
define i8 @check() {
  %x0 = load volatile i32, ptr @v0
  %x1 = load volatile i32, ptr @v1
  %x2 = load volatile i32, ptr @v2
  %x3 = load volatile i32, ptr @v3
  %clz0 = call i32 @llvm.ctlz.i32(i32 %x0, i1 false)
  %bad0 = icmp ne i32 %clz0, 32
  %w0 = zext i1 %bad0 to i8
  %clz1 = call i32 @llvm.ctlz.i32(i32 %x1, i1 false)
  %bad1 = icmp ne i32 %clz1, 31
  %w1 = zext i1 %bad1 to i8
  %clz1p = call i32 @llvm.ctlz.i32(i32 %x1, i1 true)
  %bad2 = icmp ne i32 %clz1p, 31
  %w2 = zext i1 %bad2 to i8
  %clz2 = call i32 @llvm.ctlz.i32(i32 %x2, i1 false)
  %bad3 = icmp ne i32 %clz2, 0
  %w3 = zext i1 %bad3 to i8
  %clz2p = call i32 @llvm.ctlz.i32(i32 %x2, i1 true)
  %bad4 = icmp ne i32 %clz2p, 0
  %w4 = zext i1 %bad4 to i8
  %clz3 = call i32 @llvm.ctlz.i32(i32 %x3, i1 false)
  %bad5 = icmp ne i32 %clz3, 0
  %w5 = zext i1 %bad5 to i8
  %clz3p = call i32 @llvm.ctlz.i32(i32 %x3, i1 true)
  %bad6 = icmp ne i32 %clz3p, 0
  %w6 = zext i1 %bad6 to i8
  %ctz0 = call i32 @llvm.cttz.i32(i32 %x0, i1 false)
  %bad7 = icmp ne i32 %ctz0, 32
  %w7 = zext i1 %bad7 to i8
  %ctz1 = call i32 @llvm.cttz.i32(i32 %x1, i1 false)
  %bad8 = icmp ne i32 %ctz1, 0
  %w8 = zext i1 %bad8 to i8
  %ctz1p = call i32 @llvm.cttz.i32(i32 %x1, i1 true)
  %bad9 = icmp ne i32 %ctz1p, 0
  %w9 = zext i1 %bad9 to i8
  %ctz2 = call i32 @llvm.cttz.i32(i32 %x2, i1 false)
  %bad10 = icmp ne i32 %ctz2, 31
  %w10 = zext i1 %bad10 to i8
  %ctz2p = call i32 @llvm.cttz.i32(i32 %x2, i1 true)
  %bad11 = icmp ne i32 %ctz2p, 31
  %w11 = zext i1 %bad11 to i8
  %ctz3 = call i32 @llvm.cttz.i32(i32 %x3, i1 false)
  %bad12 = icmp ne i32 %ctz3, 0
  %w12 = zext i1 %bad12 to i8
  %ctz3p = call i32 @llvm.cttz.i32(i32 %x3, i1 true)
  %bad13 = icmp ne i32 %ctz3p, 0
  %w13 = zext i1 %bad13 to i8
  %s0 = add i8 0, %w0
  %s1 = add i8 %s0, %w1
  %s2 = add i8 %s1, %w2
  %s3 = add i8 %s2, %w3
  %s4 = add i8 %s3, %w4
  %s5 = add i8 %s4, %w5
  %s6 = add i8 %s5, %w6
  %s7 = add i8 %s6, %w7
  %s8 = add i8 %s7, %w8
  %s9 = add i8 %s8, %w9
  %s10 = add i8 %s9, %w10
  %s11 = add i8 %s10, %w11
  %s12 = add i8 %s11, %w12
  %s13 = add i8 %s12, %w13
  ret i8 %s13
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "check"), 0, "{text}");
}

#[test]
fn a_loop_right_after_an_empty_entry_block_is_not_the_entry() {
    // -O0 `while (*tp) tp++;` (basic.c st_rem): the empty entry and the loop
    // header shared block 0 and "entry block is a loop header" panicked
    let ir = module(
        "@s = global [3 x i8] c\"ab\\00\"\n@tp = internal global ptr @s\n\
         define i8 @g() {\n  br label %h\n\
         h:\n  %p = load ptr, ptr @tp\n  %c = load i8, ptr %p\n  %z = icmp ne i8 %c, 0\n  br i1 %z, label %b, label %x\n\
         b:\n  %q = getelementptr inbounds i8, ptr %p, i32 1\n  store ptr %q, ptr @tp\n  br label %h\n\
         x:\n  ret i8 7\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O0)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "g"), 7, "{text}");
}

#[test]
fn a_pointer_minus_one_borrows_by_branch() {
    // a backward copy (k_memmove): `--p` was `clc; adc #255; adc #255` through
    // a pack/unpack, 885 clocks against the hand routine's 418
    let ir = module(
        "@n = global i8 4\n\
         define i8 @g() {\n\
         \x20 %n = load volatile i8, ptr @n\n\
         \x20 %s0 = inttoptr i16 49154 to ptr\n  %d0 = inttoptr i16 45058 to ptr\n\
         \x20 %a = inttoptr i16 49150 to ptr\n  store volatile i8 1, ptr %a\n\
         \x20 %b = inttoptr i16 49151 to ptr\n  store volatile i8 2, ptr %b\n\
         \x20 %c = inttoptr i16 49152 to ptr\n  store volatile i8 3, ptr %c\n\
         \x20 %e = inttoptr i16 49153 to ptr\n  store volatile i8 4, ptr %e\n\
         \x20 br label %l\n\
         l:\n\
         \x20 %i = phi i8 [ %n, %0 ], [ %j, %l ]\n\
         \x20 %s = phi ptr [ %s0, %0 ], [ %s2, %l ]\n\
         \x20 %d = phi ptr [ %d0, %0 ], [ %d2, %l ]\n\
         \x20 %s2 = getelementptr inbounds i8, ptr %s, i32 -1\n\
         \x20 %d2 = getelementptr inbounds i8, ptr %d, i32 -1\n\
         \x20 %v = load volatile i8, ptr %s2\n  store volatile i8 %v, ptr %d2\n\
         \x20 %j = sub i8 %i, 1\n  %z = icmp ne i8 %j, 0\n  br i1 %z, label %l, label %x\n\
         x:\n\
         \x20 %lo = inttoptr i16 45054 to ptr\n  %hi = inttoptr i16 45057 to ptr\n\
         \x20 %f = load volatile i8, ptr %lo\n  %h = load volatile i8, ptr %hi\n\
         \x20 %m = mul i8 %f, 16\n  %r = add i8 %m, %h\n  ret i8 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "g"), 20, "{text}");
    assert!(!text.contains("adc #255"), "{text}");
}

#[test]
fn the_frame_address_at_offset_zero_is_the_stack_pointer_copied() {
    // k_itoa's `tmp[n]`: `clc; lda rc0; adc #0; sta; lda rc1; adc #0; sta` (13
    // B) for the address of the first byte of the frame, a copy of the pair
    // (8 B)
    let ir = module(
        "@n = global i8 5\n\
         define i8 @g() {\n\
         \x20 %a = alloca [16 x i8]\n  %n = load volatile i8, ptr @n\n\
         \x20 %p = getelementptr inbounds [16 x i8], ptr %a, i8 0, i8 %n\n\
         \x20 store volatile i8 42, ptr %p\n  %v = load volatile i8, ptr %p\n  ret i8 %v\n}",
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "g"), 42, "{text}");
    assert!(!text.contains("lda rc0\n  adc #0"), "{text}");
}

#[test]
fn a_word_plus_one_carries_by_branch() {
    // strnlen's index: `clc; lda lo; adc #1; sta lo; lda hi; adc #0; sta hi`
    // (13 B) where `inc lo; bne; inc hi` is 6
    let ir = module(
        "@n = global i16 300\n@sink = global i16 0\n\
         define i8 @g() {\n\
         \x20 %n = load volatile i16, ptr @n\n  br label %l\n\
         l:\n\
         \x20 %i = phi i16 [ 0, %0 ], [ %j, %l ]\n\
         \x20 %j = add i16 %i, 1\n  store volatile i16 %j, ptr @sink\n\
         \x20 %c = icmp ult i16 %j, %n\n  br i1 %c, label %l, label %x\n\
         x:\n\
         \x20 %r = trunc i16 %j to i8\n  ret i8 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "g"), 44, "{text}");
    assert!(!text.contains("adc #1\n"), "{text}");
}

#[test]
fn a_zero_filled_variable_is_bss_not_image_bytes() {
    // basic.c: prog[8192], tok, xv ... were `.byte 0,0,...` in DATA, 8721 B of
    // image where gcc's BSS has none
    let ir = module(
        "@buf = global [4096 x i8] zeroinitializer\n@one = global i8 1\n\
         define i8 @g() {\n  %p = getelementptr inbounds [4096 x i8], ptr @buf, i16 0, i16 4000\n\
         \x20 %v = load volatile i8, ptr %p\n  %o = load volatile i8, ptr @one\n  %r = add i8 %v, %o\n  ret i8 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O1)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "g"), 1, "{text}");
    assert!(text.contains("buf: .res 4096"), "{text}");
    assert!(!text.contains(".byte 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0\n  .byte 0,0,0,0"), "{text}");
}

/// `a / b` and `a % b` of signed words at -Os linked the short-divisor path too
/// (`__divhi3`: 149 B against SDCC's 131 for a bare divide, `k_tosdivax`): the
/// signed pair had no compact variant while the unsigned pair did. Each in its
/// own function: a divide and a remainder of the same operands are one
/// `__divhi3` and a multiply, so `__modhi3c` is only asked for by a remainder
/// alone (the first form of this test asserted both calls in one function and
/// saw no `__modhi3c`).
#[test]
fn a_signed_word_divide_calls_the_compact_routine_at_size_and_in_a_cold_module_at_speed() {
    let ir = module(
        "@a = global i16 -1000\n@b = global i16 7\n\
         define i8 @g() {\n\
         \x20 %a = load volatile i16, ptr @a\n  %b = load volatile i16, ptr @b\n\
         \x20 %q = sdiv i16 %a, %b\n  %t = lshr i16 %q, 8\n  %u = xor i16 %q, %t\n  %v = trunc i16 %u to i8\n  ret i8 %v\n}\n\
         define i8 @h() {\n\
         \x20 %a = load volatile i16, ptr @a\n  %b = load volatile i16, ptr @b\n\
         \x20 %r = srem i16 %a, %b\n  %t = lshr i16 %r, 8\n  %u = xor i16 %r, %t\n  %v = trunc i16 %u to i8\n  ret i8 %v\n}",
    );
    let size = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    let compact_at = |text: &str, routine: &str| text.contains(&format!("jsr {routine}c"));
    assert!(compact_at(&size, "__divhi3") && compact_at(&size, "__modhi3"), "{size}");
    let speed = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(compact_at(&speed, "__divhi3") && compact_at(&speed, "__modhi3"), "{speed}");
}

#[test]
fn a_word_divide_calls_the_compact_routine_at_size_and_in_a_cold_module_at_speed() {
    // k_itoa at -Os linked the divide's short-divisor path too: 89 B against
    // sdcc's 64 for a bare divide
    let ir = module(
        "@a = global i16 50000\n@b = global i16 7\n\
         define i8 @g() {\n\
         \x20 %a = load volatile i16, ptr @a\n  %b = load volatile i16, ptr @b\n\
         \x20 %q = udiv i16 %a, %b\n  %r = urem i16 %a, %b\n  %s = add i16 %q, %r\n\
         \x20 %t = lshr i16 %s, 8\n  %u = xor i16 %s, %t\n  %v = trunc i16 %u to i8\n  ret i8 %v\n}",
    );
    let size = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(size.contains("jsr __udivhi3c"), "{size}");
    let speed = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    assert!(speed.contains("jsr __udivhi3c"), "{speed}");
    // 50000 / 7 = 7142 r 6: 7148 = 0x1BEC, 0x1B ^ 0x1BEC low byte 0xEC ^ 0x1B =
    // 0xF7
    for text in [&size, &speed] {
        let linked = format!("{text}\n{}", common::pipeline::reachable_runtime(text));
        assert_eq!(exit_code_of(&linked, "g"), 0xF7, "{linked}");
    }
}

#[test]
fn a_word_step_never_holds_a_byte_in_a_on_the_plain_6502() {
    // the high byte of x + 1, returned: it came out in A and the carry step was
    // `inc a`, a 65C02 opcode ca65 --cpu 6502 refuses (x16basic-c: seven sites)
    let ir = module(
        "@n = global i16 255\n\
         define i8 @g() {\n\
         \x20 %x = load volatile i16, ptr @n\n  %y = add i16 %x, 1\n\
         \x20 %h = lshr i16 %y, 8\n  %r = trunc i16 %h to i8\n  ret i8 %r\n}",
    );
    for level in [Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        assert!(!text.contains("inc a") && !text.contains("dec a"), "{text}");
        assert_eq!(exit_code_of(&text, "g"), 1, "{text}");
    }
}

#[test]
fn a_word_global_is_loaded_and_stored_by_absolute_address() {
    // x16basic-c: 270 sites loaded a u16 global through a pointer cell, `ldx
    // #<(g); ldy #>(g); stx; sty; ldy #0; lda (p),y; ldy #1; lda (p),y` (19
    // B) where `lda g; ldx g+1` is 5
    let ir = module(
        "@w = global i16 300\n@v = global i16 0\n\
         define i8 @g() {\n\
         \x20 %a = load i16, ptr @w\n  %b = add i16 %a, 2\n  store i16 %b, ptr @v\n\
         \x20 %c = load i16, ptr @v\n  %h = lshr i16 %c, 8\n  %hl = trunc i16 %h to i8\n  %l = trunc i16 %c to i8\n\
         \x20 %r = add i8 %hl, %l\n  ret i8 %r\n}",
    );
    for level in [Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        // 302 = 0x012E: 1 + 0x2E = 47
        assert_eq!(exit_code_of(&text, "g"), 47, "{text}");
        assert!(!text.contains("#<(w)") && !text.contains("#<(v)"), "{text}");
    }
}

/// A C global named `sp` was exported as `sp`, which cc65's sim6502 library
/// defines too: `ld65` stopped with "Duplicate external identifier: 'sp'" on
/// x16basic-c's `prog.c`. On the cc65 row it is `_sp`, as cc65 names it; on
/// plain sim65 the name is unchanged.
#[test]
fn a_c_global_takes_the_platforms_symbol_prefix() {
    let ir = module("@sp = dso_local global i8 0\ndefine void @set(i8 %x) {\n  store i8 %x, ptr @sp\n  ret void\n}");
    let on = |name: &str| {
        let platform = llrm_6502::driver::platform_named(name).expect("a platform");
        assembly(&ir, &Request { platform, ..Request::new(Level::O2) }).expect("compiles")
    };
    let cc65 = on("sim65-cc65");
    assert!(cc65.contains(".export _sp") && cc65.contains("_sp: .res 1"), "{cc65}");
    assert!(cc65.contains("sta _sp") && !cc65.contains("sta sp"), "{cc65}");
    let plain = on("sim65");
    assert!(plain.contains(".export sp") && plain.contains("sta sp"), "{plain}");
}

/// `a - b + strend` copied the word global into two cells (`lda strend; sta
/// rc2; lda strend+1; sta rc3`) and added the cells: four instructions the
/// byte forms never needed. A word global read once by an add is the memory
/// operand of both halves, as a byte global is (x16 getspa, linget).
#[test]
fn a_word_global_read_once_by_an_add_is_the_memory_operand_of_both_halves() {
    let ir = module(
        "@g = global i16 0\n\
         define i16 @f(i16 %a, i16 %b) {\n  %s = sub i16 %a, %b\n  %v = load i16, ptr @g\n  %r = add i16 %s, %v\n  ret i16 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("adc g\n") && text.contains("adc g+1\n"), "no memory operand:\n{text}");
    assert!(!text.contains("lda g\n"), "the global was loaded into a cell first:\n{text}");
}

/// A store between the word load and its add must not change what the add
/// reads: the load keeps its place, so the add reads a cell.
#[test]
fn a_store_between_a_word_load_and_its_add_does_not_change_what_the_add_reads() {
    let ir = module(
        "@g = global i16 0\n\
         define i16 @f(i16 %x) {\n  %a = load i16, ptr @g\n  store i16 %x, ptr @g\n  %b = add i16 %a, %x\n  ret i16 %b\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let (read, written) = (text.find("adc g"), text.find("sta g"));
    assert!(read.is_none() || read < written, "the add reads g after the store:\n{text}");
}

/// `if (n < strend)` loaded the word global into two cells and compared the
/// cells (`lda strend; sta rc4; lda strend+1; sta rc5; cmp rc4; sbc rc5`):
/// four instructions and two cells more than `cmp strend; sbc strend+1`
/// (x16 getspa). A byte global is the same.
#[test]
fn a_global_compared_by_a_branch_is_the_memory_operand_of_the_compare() {
    for (ty, flags) in [("i16", ["cmp g\n", "sbc g+1\n"]), ("i8", ["cmp g\n", "cmp g\n"])] {
        let ir = module(&format!(
            "@g = global {ty} 0\ndeclare void @h()\n\
             define void @f({ty} %x) {{\n  %v = load {ty}, ptr @g\n  %c = icmp ult {ty} %x, %v\n  \
             br i1 %c, label %t, label %e\nt:\n  call void @h()\n  ret void\ne:\n  ret void\n}}"
        ));
        let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
        assert!(flags.iter().all(|read| text.contains(read)), "{ty}: no memory operand:\n{text}");
        assert!(!text.contains("lda g"), "{ty}: the global was loaded into a cell first:\n{text}");
    }
}

/// The flags are made where the branch reads them. A store to the global, or a
/// call, between the compare and its branch must not change what is compared:
/// the compare reads the global before the store.
#[test]
fn a_store_or_a_call_between_a_compare_and_its_branch_does_not_change_what_it_reads() {
    for between in ["store i16 %y, ptr @g", "call void @other()"] {
        let ir = module(&format!(
            "@g = global i16 0\ndeclare void @h()\ndeclare void @other()\n\
             define void @f(i16 %x, i16 %y) {{\n  %v = load i16, ptr @g\n  %c = icmp ult i16 %x, %v\n  {between}\n  \
             br i1 %c, label %t, label %e\nt:\n  call void @h()\n  ret void\ne:\n  ret void\n}}"
        ));
        let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
        let after = text.find("sta g").into_iter().chain(text.find("jsr other")).min().expect("the write");
        assert!(text.find("cmp g").is_none_or(|read| read < after), "{between}: the compare reads g after it:\n{text}");
        assert!(text.contains("lda g"), "{between}: g was never read before the write:\n{text}");
    }
}

/// `a = x; b = x;` loaded `x` from its cell twice (`lda rc2; sta a; lda rc2;
/// sta b`): the store to `a` changes neither the register nor the cell. x16
/// fin's eleven zero stores were `lda rc2; sta g` eleven times (a byte and a
/// clock each).
#[test]
fn a_byte_stored_twice_is_loaded_once() {
    let ir = module(
        "@a = global i8 0\n@b = global i8 0\n@c = global i8 0\n\
         define void @k(i8 %x) {\n  store i8 %x, ptr @a\n  store i8 %x, ptr @b\n  store i8 %x, ptr @c\n  ret void\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    // the byte arrives in `a`: it is stored three times and never loaded
    assert_eq!(text.matches("lda rc2").count(), 0, "the byte was loaded again:\n{text}");
    assert_eq!(text.matches("sta ").count(), 3, "{text}");
}

/// A zero kept in a callee-saved cell across a call was loaded and compared
/// as `lda rc8` / `cmp rc8`, two bytes each like `cmp #0`, but the compare
/// against zero after `lda cf` stayed because the cell hid the zero (x16
/// fin). The cell's immediate is read as an immediate, and the compare goes.
#[test]
fn a_zero_in_a_callee_saved_cell_is_compared_as_zero() {
    let ir = module(
        "@cf = global i8 0\n@g1 = global i8 0\n@g2 = global i8 0\ndeclare i8 @chrget(i8)\n\
         define i8 @f() {\n  store i8 0, ptr @g1\n  %c = call i8 @chrget(i8 0)\n  store i8 0, ptr @g2\n  \
         %v = load i8, ptr @cf\n  %z = icmp eq i8 %v, 0\n  %i = add i8 %c, 1\n  %r = select i1 %z, i8 %c, i8 %i\n  ret i8 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    assert!(!text.contains("cmp rc"), "a zero was compared from a cell:\n{text}");
    assert!(!text.contains("cmp #0"), "a compare against zero follows a load of the flag:\n{text}");
}

/// `UMulO` on a word was refused ("convert node to libcall",
/// `overflow_builtins.c` of tests/run/c): its flag's extension was not lowered.
/// It computes the product and the carry out of the 16 bits, at the edges (255
/// * 257 fits, 256 * 256 and 4096 * 16 do not).
#[test]
fn an_unsigned_word_multiply_with_overflow_computes_its_value_and_its_flag() {
    let pairs: [(u16, u16); 10] = [
        (0, 0),
        (1, 65535),
        (255, 257),
        (256, 256),
        (65535, 65535),
        (300, 200),
        (4096, 16),
        (255, 258),
        (2, 32768),
        (1, 32768),
    ];
    let mut ir = String::from("declare { i16, i1 } @llvm.umul.with.overflow.i16(i16, i16)\n");
    for (k, (a, b)) in pairs.iter().enumerate() {
        ir += &format!(
            "@a_um{k} = global i16 {}\n@b_um{k} = global i16 {}\n\
             define i8 @t_um{k}() {{\n  %a = load volatile i16, ptr @a_um{k}\n  %b = load volatile i16, ptr @b_um{k}\n  \
             %r = call {{ i16, i1 }} @llvm.umul.with.overflow.i16(i16 %a, i16 %b)\n  \
             %v = extractvalue {{ i16, i1 }} %r, 0\n  %o = extractvalue {{ i16, i1 }} %r, 1\n  \
             %lo = trunc i16 %v to i8\n  %h = lshr i16 %v, 8\n  %hi = trunc i16 %h to i8\n  %x = xor i8 %lo, %hi\n  \
             %m = select i1 %o, i8 -128, i8 0\n  %y = xor i8 %x, %m\n  ret i8 %y\n}}\n",
            *a as i16, *b as i16
        );
    }
    let text = assembly(&module(&ir), &Request::new(Level::O2)).unwrap();
    for (k, (a, b)) in pairs.iter().enumerate() {
        let (value, overflow) = a.overflowing_mul(*b);
        let want = (value & 0xFF) as i32 ^ (value >> 8) as i32 ^ if overflow { 0x80 } else { 0 };
        assert_eq!(
            exit_code_with(&text, &format!("t_um{k}"), llrm_6502::RUNTIME),
            want,
            "umul({a}, {b}) = {value}, overflow {overflow}"
        );
    }
}

/// A byte loaded and zero-extended to a word had its high half masked as
/// `lda #0; and #0` and a cell for it (#7165 expanded the extending load to an
/// any-extend and a mask 255): `zext(a) + zext(b)` took 30 instructions where
/// 21 do, and `struct_odd_sizes` -Os grew 74 B. A mask of zero is a zero, and
/// the high half of a word masked by 255 is known to be zero.
#[test]
fn a_zero_extended_loaded_byte_has_no_mask_of_zero() {
    let ir = module(
        "@a = global i8 0\n@b = global i8 0\n\
         define i16 @f() {\n  %1 = load i8, ptr @a\n  %2 = zext i8 %1 to i16\n  %3 = load i8, ptr @b\n  \
         %4 = zext i8 %3 to i16\n  %5 = add i16 %2, %4\n  ret i16 %5\n}",
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    assert!(!text.contains("and #0"), "a byte was masked by zero:\n{text}");
    let instructions = text.lines().filter(|line| line.starts_with("  ")).count();
    assert!(instructions <= 22, "{instructions} instructions:\n{text}");
}

/// A pointer walked by a constant and compared with its end (`for (p = vartab;
/// p != arytab; p += 7)`, x16 `vars` lookup): the pointer stayed unpacked, its
/// step built in `a`, `x` and `y` and stored back each trip, and its compare
/// read the halves through `x` and `y`: 23 instructions a trip where 14 do,
/// 490085 clocks for 120 lookups. A compare is a use a pointer pair serves, and
/// a half loaded into an index register only to be moved to `a` is loaded into
/// `a`.
#[test]
fn a_pointer_stepped_by_a_constant_and_compared_is_bumped_in_its_pair() {
    let mut table = Vec::new();
    for entry in 0..4 {
        for byte in 0..7 {
            table.push(format!("i8 {}", if byte < 2 { 1 + 2 * entry + byte } else { 0 }));
        }
    }
    let ir = module(&format!(
        "@tab = global [28 x i8] [{}]\n@vartab = global ptr @tab\n@arytab = global ptr getelementptr (i8, ptr @tab, i16 28)\n\
         @n0 = global i8 5\n@n1 = global i8 6\n\
         define ptr @find() {{\n  %1 = load ptr, ptr @vartab\n  %2 = load ptr, ptr @arytab\n  %3 = icmp eq ptr %1, %2\n  \
         br i1 %3, label %18, label %4\n\
         4:\n  %5 = load i8, ptr @n0\n  %6 = load i8, ptr @n1\n  br label %7\n\
         7:\n  %8 = phi ptr [ %1, %4 ], [ %16, %15 ]\n  %9 = load i8, ptr %8\n  %10 = icmp eq i8 %9, %5\n  \
         br i1 %10, label %11, label %15\n\
         11:\n  %12 = getelementptr inbounds i8, ptr %8, i16 1\n  %13 = load i8, ptr %12\n  %14 = icmp eq i8 %13, %6\n  \
         br i1 %14, label %18, label %15\n\
         15:\n  %16 = getelementptr inbounds i8, ptr %8, i16 7\n  %17 = icmp eq ptr %16, %2\n  br i1 %17, label %18, label %7\n\
         18:\n  %19 = phi ptr [ null, %0 ], [ null, %15 ], [ %8, %11 ]\n  ret ptr %19\n}}\n\
         define i8 @t() {{\n  %r = call ptr @find()\n  %a = ptrtoint ptr %r to i16\n  %b = ptrtoint ptr @tab to i16\n  \
         %d = sub i16 %a, %b\n  %lo = trunc i16 %d to i8\n  ret i8 %lo\n}}",
        table.join(", ")
    ));
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    assert_eq!(exit_code_with(&text, "t", llrm_6502::RUNTIME), 14, "{text}");
    let step = text.find("adc #7").expect("the step by 7");
    let trip = &text[step..text[step..].find("rts").map_or(text.len(), |at| step + at)];
    for staged in ["tax", "tay", "ldy rc"] {
        assert!(!trip.contains(staged), "`{staged}` after the step:\n{text}");
    }
}

/// A frame address (`rc0` plus 3) copied into a pointer pair went through the
/// index registers: `adc #3; tax; lda rc1; adc #0; tay; stx rc6; sty rc7`
/// (x16 `crunch` 1147 B, `flow` -57 B here): the sums are in `a`, so `sta rc6`.
#[test]
fn a_frame_address_is_stored_from_a_not_through_the_index_registers() {
    let ir = module(
        "@g = global ptr null\n@h = global ptr null\ndeclare void @use(ptr)\n\
         define void @f() {\n  %a = alloca [8 x i8]\n  %q = getelementptr inbounds i8, ptr %a, i16 3\n  \
         store volatile ptr %q, ptr @g\n  %r = getelementptr inbounds i8, ptr %a, i16 5\n  store volatile ptr %r, ptr @h\n  \
         call void @use(ptr %q)\n  ret void\n}",
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    let transfers = text.lines().filter(|line| matches!(line.trim(), "tax" | "tay")).count();
    assert!(transfers <= 4, "{transfers} transfers into the index registers:\n{text}");
}

/// `SAddO`, `SSubO` and `SMulO` on a word were refused ("convert node to
/// libcall"); they compile now, and the value and the overflow bit must be
/// right at the edges (30000 + 30000, -32768 - 1, 181 * 181), not just
/// accepted.
#[test]
fn a_signed_word_overflow_operation_computes_its_value_and_its_flag() {
    let pairs: [(i16, i16); 7] =
        [(30000, 30000), (-30000, -30000), (1, 2), (-1, 1), (32767, 1), (-32768, -1), (181, 182)];
    let ops: [(&str, fn(i16, i16) -> (i16, bool)); 3] =
        [("sadd", i16::overflowing_add), ("ssub", i16::overflowing_sub), ("smul", i16::overflowing_mul)];
    let mut ir = String::new();
    for (op, _) in ops {
        ir += &format!("declare {{ i16, i1 }} @llvm.{op}.with.overflow.i16(i16, i16)\n");
        for (k, (a, b)) in pairs.iter().enumerate() {
            ir += &format!(
                "@a_{op}{k} = global i16 {a}\n@b_{op}{k} = global i16 {b}\n\
                 define i8 @t_{op}{k}() {{\n  %a = load volatile i16, ptr @a_{op}{k}\n  %b = load volatile i16, ptr @b_{op}{k}\n  \
                 %r = call {{ i16, i1 }} @llvm.{op}.with.overflow.i16(i16 %a, i16 %b)\n  \
                 %v = extractvalue {{ i16, i1 }} %r, 0\n  %o = extractvalue {{ i16, i1 }} %r, 1\n  \
                 %lo = trunc i16 %v to i8\n  %h = lshr i16 %v, 8\n  %hi = trunc i16 %h to i8\n  %x = xor i8 %lo, %hi\n  \
                 %m = select i1 %o, i8 -128, i8 0\n  %y = xor i8 %x, %m\n  ret i8 %y\n}}\n"
            );
        }
    }
    let text = assembly(&module(&ir), &Request::new(Level::O2)).unwrap();
    for (op, rust) in ops {
        for (k, (a, b)) in pairs.iter().enumerate() {
            let (value, overflow) = rust(*a, *b);
            let want = (value as u16 & 0xFF) as i32 ^ (value as u16 >> 8) as i32 ^ if overflow { 0x80 } else { 0 };
            assert_eq!(
                exit_code_with(&text, &format!("t_{op}{k}"), llrm_6502::RUNTIME),
                want,
                "{op}({a}, {b}) = {value}, overflow {overflow}"
            );
        }
    }
}

/// `fp_bfmultt`'s arrays (`f[3 - j + k]`) lived on the soft stack: every
/// element access added `rc0`, a constant and the index in 16 bits, and the
/// function adjusted `rc0` both ways (the X16 mathlib's multiply, 77% of its
/// clocks). A function nothing re-enters has one activation, so its array is
/// global (`lda g,x`): no frame, and the values over two calls are still its
/// own.
#[test]
fn the_array_of_a_function_nothing_re_enters_is_global() {
    let ir = module(
        r#"declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture)
define i8 @fill(i8 %n) noinline norecurse {
entry:
  %a = alloca [6 x i8]
  call void @llvm.lifetime.start.p0(i64 6, ptr %a)
  br label %store
store:
  %i = phi i8 [ 0, %entry ], [ %i2, %store ]
  %x = mul i8 %i, 3
  %w = zext i8 %i to i16
  %p = getelementptr i8, ptr %a, i16 %w
  store volatile i8 %x, ptr %p
  %i2 = add i8 %i, 1
  %c = icmp ult i8 %i2, 6
  br i1 %c, label %store, label %sum
sum:
  %j = phi i8 [ 0, %store ], [ %j2, %sum ]
  %t = phi i8 [ 0, %store ], [ %t2, %sum ]
  %v = zext i8 %j to i16
  %q = getelementptr i8, ptr %a, i16 %v
  %e = load volatile i8, ptr %q
  %t2 = add i8 %t, %e
  %j2 = add i8 %j, 1
  %d = icmp ult i8 %j2, %n
  br i1 %d, label %sum, label %out
out:
  call void @llvm.lifetime.end.p0(i64 6, ptr %a)
  ret i8 %t2
}
define i8 @twice() {
  %x = call i8 @fill(i8 4)
  %y = call i8 @fill(i8 3)
  %z = add i8 %x, %y
  ret i8 %z
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("_local0") && !text.contains("sta rc0"), "the array is on the soft stack:\n{text}");
    assert_eq!(exit_code_of(&text, "twice"), 27, "{text}");
}

/// What re-enters a function keeps its stack: with the array global, the
/// recursion below returns 0 where each activation's own `a[0]` makes 6.
#[test]
fn the_array_of_a_function_that_can_be_re_entered_stays_on_the_stack() {
    let body = |name: &str, call: &str| {
        format!(
            "define i8 @{name}(i8 %n) noinline {{\n  %a = alloca [4 x i8]\n  store volatile i8 %n, ptr %a\n  \
             %z = icmp eq i8 %n, 0\n  br i1 %z, label %done, label %go\ngo:\n  %m = sub i8 %n, 1\n  {call}\n  \
             %v = load volatile i8, ptr %a\n  %s = add i8 %r, %v\n  ret i8 %s\ndone:\n  ret i8 0\n}}\n"
        )
    };
    let driver = |name: &str| format!("define i8 @run_{name}() {{\n  %r = call i8 @{name}(i8 3)\n  ret i8 %r\n}}\n");
    let direct = [body("direct", "%r = call i8 @direct(i8 %m)"), driver("direct")].concat();
    let indirect = [
        "@table = global ptr @through\n".to_owned(),
        body("through", "%f = load ptr, ptr @table\n  %r = call i8 %f(i8 %m)"),
        driver("through"),
    ]
    .concat();
    let outer = [
        "@back = global ptr @outer\ndefine i8 @mid(i8 %n) noinline {\n  %f = load ptr, ptr @back\n  \
         %r = call i8 %f(i8 %n)\n  ret i8 %r\n}\n"
            .to_owned(),
        body("outer", "%r = call i8 @mid(i8 %m)"),
        driver("outer"),
    ]
    .concat();
    for (name, ir) in [("direct", direct), ("through", indirect), ("outer", outer)] {
        let text = assembly(&module(&ir), &Request::new(Level::O2)).unwrap();
        assert!(!text.contains("_local"), "{name}: an array shared by activations:\n{text}");
        assert_eq!(exit_code_of(&text, &format!("run_{name}")), 6, "{name}:\n{text}");
    }
}

/// A call to a declaration may come back into the function (a callback): not
/// `norecurse`, so the array stays on the stack.
#[test]
fn the_array_of_a_function_that_calls_a_declaration_stays_on_the_stack() {
    let ir = module(
        "declare void @elsewhere(ptr)\ndefine void @f() {\n  %a = alloca [4 x i8]\n  \
         call void @elsewhere(ptr %a)\n  ret void\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!text.contains("_local"), "{text}");
}

/// byvalue_struct_address (tests/run/c): `bar` stores a static that `main`
/// reads, and costs more than its call at -Os until `main` has folded the read
/// away with the store. The first walk of the inliner prices it whole and
/// leaves `jsr bar` (23 bytes became 120, 62 instructions for 13); the second
/// walk finds it small.
#[test]
fn a_callee_emptied_by_a_fold_in_its_caller_is_inlined_by_the_second_walk() {
    let ir = include_str!("data/byvalue_struct_address.ll");
    let mut request = Request::new(Level::Os);
    request.passes.push("-fno-second-inline".to_owned());
    let once = assembly(ir, &request).unwrap();
    let twice = assembly(ir, &Request::new(Level::Os)).unwrap();
    assert!(once.contains("jsr bar"), "the first walk alone no longer leaves the call:\n{once}");
    assert!(!twice.contains("jsr bar"), "the callee is still called:\n{twice}");
    let size = |text: &str| text.lines().filter(|line| line.starts_with("  ")).count();
    assert!(size(&twice) * 2 < size(&once), "{} instructions against {}", size(&twice), size(&once));
}

/// The start-up zeroing of the new BSS costs 15 clocks a byte, once: a
/// function called once, outside any loop, with a 24-byte array it touches
/// twice, gave `tail_calls` 360 clocks for 46 saved. It keeps the stack where
/// clocks count (-O2) and goes static where bytes do (-Os).
#[test]
fn a_static_array_pays_for_the_zeroing_of_its_bytes() {
    let ir = module(
        r#"define i8 @once(i8 %n) noinline norecurse {
  %a = alloca [24 x i8]
  %w = zext i8 %n to i16
  %p = getelementptr i8, ptr %a, i16 %w
  store volatile i8 %n, ptr %p
  %v = load volatile i8, ptr %p
  ret i8 %v
}
define i8 @t() {
  %r = call i8 @once(i8 7)
  ret i8 %r
}"#,
    );
    let speed = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!speed.contains("_local"), "-O2 zeroes 24 bytes to save 46 clocks:\n{speed}");
    assert_eq!(exit_code_of(&speed, "t"), 7, "{speed}");
    let size = assembly(&ir, &Request::new(Level::Os)).unwrap();
    assert!(size.contains("_local0"), "-Os counts bytes:\n{size}");
    assert_eq!(exit_code_of(&size, "t"), 7, "{size}");
}

/// call_keeps (tests/run/c): `churn` is settled after the first walk, and
/// Reassociate ranks by the layout, so the second walk's Simplify list
/// reordered its xor chain for no gain: 26 bytes more at -Os. A rerun over a
/// body no call was spliced into stays only where the body gets smaller.
#[test]
fn the_second_walk_leaves_a_settled_body_as_it_was() {
    let ir = include_str!("data/call_keeps.ll");
    let mut request = Request::new(Level::Os);
    request.passes.push("-fno-second-inline".to_owned());
    let once = assembly(ir, &request).unwrap();
    let twice = assembly(ir, &Request::new(Level::Os)).unwrap();
    let size = |text: &str| text.lines().filter(|line| line.starts_with("  ")).count();
    assert!(size(&twice) <= size(&once), "{} instructions against {}", size(&twice), size(&once));
}

/// recmany_o1 (tests/run/c): the rerun over `bench_recmany` "saved" one unit of
/// the estimate (an add's operand) and kept a Reassociate reorder of a settled
/// chain: 16 bytes more at -Os. One unit does not keep a rerun.
#[test]
fn a_rerun_that_saves_one_unit_of_the_estimate_is_not_kept() {
    let ir = include_str!("data/recmany_o1.ll");
    let mut request = Request::new(Level::Os);
    request.passes.push("-fno-second-inline".to_owned());
    let once = assembly(ir, &request).unwrap();
    let twice = assembly(ir, &Request::new(Level::Os)).unwrap();
    assert_eq!(twice, once, "the second walk changed a program it has nothing to inline in");
}

/// `g++` on a global byte was `lda #1; clc; adc g; sta g` (9 bytes) where `inc
/// g` is 3 (x16basic-c: `deccnt++`, `++tenexp` in fp_fin). The carry and A are
/// dead after the store, and `inc` leaves the flags a branch reads as they
/// were.
#[test]
fn a_step_of_a_global_byte_is_an_inc_or_a_dec_of_it() {
    let ir = module(
        "@g = global i8 5\n\
         define void @up() noinline {\n  %v = load i8, ptr @g\n  %w = add i8 %v, 1\n  store i8 %w, ptr @g\n  ret void\n}\n\
         define void @down() noinline {\n  %v = load i8, ptr @g\n  %w = add i8 %v, -1\n  store i8 %w, ptr @g\n  ret void\n}\n\
         define i8 @t() {\n  call void @up()\n  call void @up()\n  call void @down()\n  %r = load i8, ptr @g\n  ret i8 %r\n}",
    );
    for level in [Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        assert!(text.contains("inc g") && text.contains("dec g"), "{text}");
        assert!(!text.contains("adc g"), "{text}");
        assert_eq!(exit_code_of(&text, "t"), 6, "{text}");
    }
    // the 65C02's `lda g; inc a; sta g`
    let mut request = Request::new(Level::Os);
    request.cpu = llrm_6502::dagisel::hooks::Cpu::W65c02;
    let text = assembly(&ir, &request).unwrap();
    assert!(text.contains("inc g") && text.contains("dec g") && !text.contains("inc a"), "{text}");
}

/// The step of the counter of `do work(); while (--n)` was `lda #255; clc; adc
/// n; sta n` too: the call after the branch writes A and takes no argument in
/// it, so A is dead at the branch. A callee that takes its argument in A keeps
/// the load.
#[test]
fn a_counter_stepped_before_a_call_that_needs_no_a_is_a_dec() {
    let ir = module(
        "@n = global i8 3\n@hits = global i8 0\n\
         define void @work() noinline {\n  %v = load i8, ptr @hits\n  %w = add i8 %v, 2\n  store i8 %w, ptr @hits\n  ret void\n}\n\
         define i8 @t() {\nentry:\n  br label %loop\nloop:\n  call void @work()\n  %c = load i8, ptr @n\n  %d = add i8 %c, -1\n  \
         store i8 %d, ptr @n\n  %z = icmp eq i8 %d, 0\n  br i1 %z, label %out, label %loop\nout:\n  %r = load i8, ptr @hits\n  ret i8 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    assert!(text.contains("dec n") && !text.contains("adc n"), "{text}");
    assert_eq!(exit_code_of(&text, "t"), 6, "{text}");
}

/// licm_graph_callee_footprint (tests/run/c): `state * 3UL` was a call of
/// `__mulsi3` (2263 clocks a trip, 10x gcc's loop). A long multiply by `2^N +-
/// 1` is a shift and an add where clocks count (-O2): the call is 59 bytes and
/// ~2200 clocks, the expansion ~75 and ~100. Tuned for size it stays a call
/// unless the shift is whole bytes (`x * 257` is 52 bytes), measured at -Os.
#[test]
fn a_long_multiply_by_a_near_power_of_two_is_shifts_where_clocks_count() {
    let ir = module(
        "define i32 @by3(i32 %x) noinline {\n  %r = mul i32 %x, 3\n  ret i32 %r\n}\n\
         define i32 @by257(i32 %x) noinline {\n  %r = mul i32 %x, 257\n  ret i32 %r\n}\n\
         define i8 @t3() {\n  %v = call i32 @by3(i32 16909060)\n  %h = lshr i32 %v, 8\n  %r = trunc i32 %h to i8\n  ret i8 %r\n}\n\
         define i8 @t257() {\n  %v = call i32 @by257(i32 16909060)\n  %h = lshr i32 %v, 8\n  %r = trunc i32 %h to i8\n  ret i8 %r\n}",
    );
    let fast = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!fast.contains("__mulsi3"), "{fast}");
    let linked = format!("{fast}\n{}", common::pipeline::reachable_runtime(&fast));
    assert_eq!(exit_code_of(&linked, "t3"), 0x09, "{fast}");
    assert_eq!(exit_code_of(&linked, "t257"), 0x07, "{fast}");
    // tuned for size: the call stays for 3, not for 257, while a variable
    // multiply keeps the routine linked (alone, `by3` is shifts: mul_image.rs)
    let ir = format!(
        "{}\n{}",
        ir.replace("noinline {", "noinline optsize {"),
        "define i32 @prod(i32 %a, i32 %b) noinline optsize {\n  %r = mul i32 %a, %b\n  ret i32 %r\n}"
    );
    let small = assembly(&ir, &Request::new(Level::Os)).unwrap();
    let (by3, by257) = (function_text(&small, "by3"), function_text(&small, "by257"));
    assert!(by3.contains("__mulsi3"), "{by3}");
    assert!(!by257.contains("__mulsi3"), "{by257}");
}

/// The lines of function `name` in `text`, up to the next label of a function.
fn function_text(
    text: &str,
    name: &str,
) -> String {
    let start = text.find(&format!("\n{name}:")).unwrap_or_else(|| panic!("no {name}:\n{text}"));
    let rest = &text[start + 1..];
    let end = rest.find("\n.export").unwrap_or(rest.len());
    rest[..end].to_owned()
}

/// `__mulsi3_fast` linked 19 bytes more into 13 of 72 programs and ran faster
/// in 11; in struct_with_i64 and frame_address_stack_args the multiply never
/// runs and the bytes were all cost. A long multiply calls `__mulsi3` at every
/// level until a priced rule picks the variant.
#[test]
fn a_long_multiply_links_the_compact_routine_at_every_level() {
    let ir = module("define i32 @f(i32 %a, i32 %b) {\n  %r = mul i32 %a, %b\n  ret i32 %r\n}");
    for level in [Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        assert!(text.contains("__mulsi3") && !text.contains("__mulsi3_fast"), "{text}");
    }
}

/// A long multiply in a 200 trip loop called the compact `__mulsi3` (32 bits
/// of multiplier a call): mul_constants 20.7M clocks against 16.8M, and
/// overflow_builtins 720k against 426k with `__mulsi3_fast`. A module links
/// the speed routine where its multiply sites count three runs
/// (`hotmultiply`): a loop, or a function called from one; at -Os, where the
/// share of the module's clocks it saves is worth the share of its bytes it
/// adds (`a_module_takes_the_speed_multiply_...`).
#[test]
fn a_long_multiply_in_a_loop_calls_the_fast_routine_at_speed() {
    let looped = module(
        "define i32 @f(i32 %a, i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%j, %l]\n  \
         %s = phi i32 [%a, %entry], [%r, %l]\n  %r = mul i32 %s, 1664525\n  %j = add i32 %i, 1\n  \
         %c = icmp ult i32 %j, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %r\n}",
    );
    let text = assembly(&looped, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("jsr __mulsi3_fast\n"), "a loop at -O2:\n{text}");
    let small = assembly(&looped, &Request::new(Level::Os)).unwrap();
    assert!(small.contains("jsr __mulsi3_fast\n"), "a loop that is the whole module at -Os:\n{small}");
}

/// `__builtin_mul_overflow` in a loop is `llvm.umul.with.overflow.i32`, no
/// `mul`: the site count saw no multiply and overflow_builtins kept the compact
/// routine (720k clocks against 426k).
#[test]
fn a_long_multiply_with_overflow_in_a_loop_calls_the_fast_routine_at_speed() {
    let looped = module(
        "declare {i32, i1} @llvm.umul.with.overflow.i32(i32, i32)\n\
         define i32 @f(i32 %a, i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%j, %l]\n  \
         %s = phi i32 [%a, %entry], [%r, %l]\n  %p = call {i32, i1} @llvm.umul.with.overflow.i32(i32 %s, i32 1664525)\n  \
         %r = extractvalue {i32, i1} %p, 0\n  %j = add i32 %i, 1\n  \
         %c = icmp ult i32 %j, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %r\n}",
    );
    let text = assembly(&looped, &Request::new(Level::O2)).unwrap();
    assert!(text.contains("jsr __mulsi3_fast\n"), "{text}");
}

/// Two functions that call through a pointer both made the label `_ci1`:
/// ca65 stopped with "Symbol '_ci1' is already defined" on any such module
/// (tail_calls_edges), so no program with two indirect calls assembled.
#[test]
fn indirect_call_return_labels_are_unique_across_functions() {
    let ir = module(
        "define i16 @f(ptr %p, i16 %x) {\n  %r = call i16 %p(i16 %x)\n  %s = add i16 %r, 1\n  ret i16 %s\n}\n\
         define i16 @g(ptr %p, i16 %x) {\n  %r = call i16 %p(i16 %x)\n  %s = add i16 %r, 2\n  ret i16 %s\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let labels: Vec<&str> = text.lines().filter(|l| l.starts_with("_ci") && l.ends_with(':')).collect();
    assert_eq!(labels.len(), 2, "{text}");
    assert_ne!(labels[0], labels[1], "{text}");
}

/// LSR left every 16- and 32-bit induction variable of the 6502 alone (its
/// datalayout is `n8`, so the pin's `IVUsers` found none): `x[i] = n * i`
/// called `__mulsi3` on every trip, 56k of shrink_wrapped_frame's 64k clocks.
/// At speed the 32-bit product is a running sum.
#[test]
fn a_32_bit_product_of_the_counter_is_a_running_sum_at_speed() {
    let ir = module(
        "define i32 @f(i32 %n) {
entry:
  %a = alloca [16 x i32], align 1
  br label %fill
fill:
  %i = phi i32 [ 0, %entry ], [ %next, %fill ]
  %product = mul nuw nsw i32 %i, %n
  %index = trunc i32 %i to i16
  %slot = getelementptr inbounds [4 x i8], ptr %a, i16 %index
  store i32 %product, ptr %slot, align 1
  %next = add nuw nsw i32 %i, 1
  %filled = icmp eq i32 %next, 16
  br i1 %filled, label %sum, label %fill
sum:
  %j = phi i32 [ 0, %fill ], [ %jn, %sum ]
  %total = phi i32 [ 0, %fill ], [ %t, %sum ]
  %jx = trunc i32 %j to i16
  %cell = getelementptr inbounds [4 x i8], ptr %a, i16 %jx
  %v = load i32, ptr %cell, align 1
  %t = add i32 %total, %v
  %jn = add nuw nsw i32 %j, 1
  %done = icmp eq i32 %jn, 16
  br i1 %done, label %exit, label %sum
exit:
  ret i32 %t
}",
    );
    let fast = assembly(&ir, &Request::new(Level::O2)).unwrap();
    assert!(!fast.contains("__mulsi3"), "{fast}");
}

/// A `byval` argument of 40 bytes is copied by a call of `memcpy` the builder
/// names itself; the 6502 left its callee an `ExternalSymbol` and refused
/// (`CannotSelect("ExternalSymbol")`: byval_struct, byval_inline and
/// byval_initializes at -O0, any caller that does not inline the callee).
#[test]
fn a_byval_argument_over_the_inline_limit_is_copied_by_a_memcpy_call() {
    let ir = module(
        r#"%big = type { [40 x i8] }
declare i32 @sum(ptr byval(%big) align 1, i32)
define i32 @f(ptr %p) {
  %r = call i32 @sum(ptr byval(%big) align 1 %p, i32 100)
  ret i32 %r
}"#,
    );
    for level in [Level::O0, Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{level:?}: {e}"));
        assert!(text.contains("jsr memcpy"), "{level:?}\n{text}");
    }
}

/// `llvm.usub.sat.i32` returned the inverse: 0 where the subtract does not
/// borrow and the wrapped difference where it does (`sat_builtins`: 4e9 - 5
/// gave 0, 5 - 4e9 gave 294967301). The borrow was read as the carry.
#[test]
fn a_32_bit_saturating_subtract_clamps_at_zero_only_on_a_borrow() {
    let ir = module(
        r#"declare i32 @llvm.usub.sat.i32(i32, i32)
@big = global i32 4000000000
@small = global i32 5
define i8 @no_borrow() {
  %a = load i32, ptr @big
  %b = load i32, ptr @small
  %r = call i32 @llvm.usub.sat.i32(i32 %a, i32 %b)
  %t = trunc i32 %r to i8
  ret i8 %t
}
define i8 @borrow() {
  %a = load i32, ptr @small
  %b = load i32, ptr @big
  %r = call i32 @llvm.usub.sat.i32(i32 %a, i32 %b)
  %t = trunc i32 %r to i8
  %h = lshr i32 %r, 24
  %ht = trunc i32 %h to i8
  %o = or i8 %t, %ht
  ret i8 %o
}"#,
    );
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        // 4000000000 - 5 = 0xEE6B27FB; 5 - 4000000000 clamps to 0
        assert_eq!(exit_code_of(&text, "no_borrow"), 0xFB, "{level:?}\n{text}");
        assert_eq!(exit_code_of(&text, "borrow"), 0, "{level:?}\n{text}");
    }
}

/// A library shift took its amount as a byte (the shift-amount type) where the
/// pin makes it the library's `int`, a 16-bit word here: a helper written in C
/// (`u64 __ashldi3(u64, int)`, compiler-rt) read a high byte the caller never
/// stored, whatever the stack held. `dirty` leaves 0xFF on the stack first.
#[test]
fn a_library_shift_passes_its_amount_as_the_c_int() {
    let ir = module(
        r#"define i64 @__ashldi3(i64 %x, i16 %n) noinline {
  %w = zext i16 %n to i64
  ret i64 %w
}
@n = global i8 3
@x = global i64 1
define void @dirty() noinline {
  %a = alloca [32 x i8]
  %p0 = getelementptr i8, ptr %a, i16 0
  store volatile i8 -1, ptr %p0
  %p1 = getelementptr i8, ptr %a, i16 1
  store volatile i8 -1, ptr %p1
  %p2 = getelementptr i8, ptr %a, i16 2
  store volatile i8 -1, ptr %p2
  %p3 = getelementptr i8, ptr %a, i16 3
  store volatile i8 -1, ptr %p3
  %p4 = getelementptr i8, ptr %a, i16 4
  store volatile i8 -1, ptr %p4
  %p5 = getelementptr i8, ptr %a, i16 5
  store volatile i8 -1, ptr %p5
  %p6 = getelementptr i8, ptr %a, i16 6
  store volatile i8 -1, ptr %p6
  %p7 = getelementptr i8, ptr %a, i16 7
  store volatile i8 -1, ptr %p7
  %p8 = getelementptr i8, ptr %a, i16 8
  store volatile i8 -1, ptr %p8
  %p9 = getelementptr i8, ptr %a, i16 9
  store volatile i8 -1, ptr %p9
  %p10 = getelementptr i8, ptr %a, i16 10
  store volatile i8 -1, ptr %p10
  %p11 = getelementptr i8, ptr %a, i16 11
  store volatile i8 -1, ptr %p11
  ret void
}
define i64 @shift() noinline {
  %v = load i64, ptr @x
  %c = load i8, ptr @n
  %z = zext i8 %c to i64
  %r = shl i64 %v, %z
  ret i64 %r
}
define i8 @high_byte_of_the_amount() {
  call void @dirty()
  %r = call i64 @shift()
  %h = lshr i64 %r, 8
  %t = trunc i64 %h to i8
  ret i8 %t
}"#,
    );
    for level in [Level::O0, Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{level:?}: {e}"));
        assert_eq!(exit_code_of(&text, "high_byte_of_the_amount"), 0, "{level:?}\n{text}");
    }
}

/// Stack arguments (the long arguments past `rc2..rc7`) were reached through a
/// frame pointer built per access: `clc; lda rc0; adc #n; sta rcA; lda rc1;
/// adc #0; sta rcB; ldy #0; lda (rcA),y` (13 B, or an 8 B copy of `rc0`; x16
/// tail_calls -Os +463 B vs gcc). The frame index belongs in the address as
/// `rc0` plus a `ldy` offset: `ldy #n; lda (rc0),y`.
#[test]
fn a_stack_argument_is_read_through_rc0_without_a_frame_pointer() {
    let ir = module(
        "@x = global [3 x i32] zeroinitializer\n\
         define i8 @sum(i32 %a, i32 %b, i32 %c) noinline {\n  %s = add i32 %a, %b\n  %t = add i32 %s, %c\n  \
         %m = lshr i32 %t, 8\n  %u = lshr i32 %t, 24\n  %p = trunc i32 %m to i8\n  %q = trunc i32 %u to i8\n  \
         %r = add i8 %p, %q\n  ret i8 %r\n}\n\
         define i8 @t() {\n  %a = load volatile i32, ptr @x\n  %pb = getelementptr i32, ptr @x, i16 1\n  \
         %b = load volatile i32, ptr %pb\n  %pc = getelementptr i32, ptr @x, i16 2\n  %c = load volatile i32, ptr %pc\n  \
         %r = call i8 @sum(i32 %a, i32 %b, i32 %c)\n  ret i8 %r\n}",
    )
    .replace(
        "@x = global [3 x i32] zeroinitializer",
        "@x = global [3 x i32] [i32 16909060, i32 270544960, i32 65538]",
    );
    for level in [Level::Os, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        let body = &text[text.find("sum:").expect("sum")..];
        let body = &body[..body.find("rts").expect("rts of sum")];
        let builds = body.matches("lda rc0\n  adc").count() + body.matches("lda rc0\n  sta").count();
        assert_eq!(builds, 0, "{builds} frame-pointer builds in sum at {level:?}:\n{body}");
        assert_eq!(exit_code_of(&text, "t"), 68, "{text}");
    }
}

/// A frame object past the 255 bytes one `(zp),Y` offset reaches still takes a
/// built address; folding frame objects into `rc0` plus `ldy` must not turn
/// such a frame into a refusal ("past the 255") or a wrapped offset.
#[test]
fn a_local_past_255_bytes_from_rc0_is_still_reached() {
    let ir = module(
        "define i8 @t() {\n  %a = alloca [300 x i8]\n  %p = getelementptr inbounds i8, ptr %a, i16 3\n  \
         %q = getelementptr inbounds i8, ptr %a, i16 290\n  store volatile i8 5, ptr %p\n  \
         store volatile i8 9, ptr %q\n  %x = load volatile i8, ptr %p\n  %y = load volatile i8, ptr %q\n  \
         %r = add i8 %x, %y\n  ret i8 %r\n}",
    );
    for level in [Level::Os, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        assert_eq!(exit_code_of(&text, "t"), 14, "{text}");
    }
}

/// A long division called the speed routine with the pin-structure wide path
/// at any site: its 141 bytes (423 clocks at 3 a byte) came into programs that
/// divide once, cold (`compound_assignment_types`: +139 B for -9 clocks). A
/// module links it only where its division sites run 3 times (`hotdivision`):
/// in a loop, or in a function called from one. Elsewhere the routine with the
/// small wide path (the compact one is 3195 clocks slower on one call); -Os
/// never has the wide one, and the small one where the module's share of clocks
/// pays for its bytes (`a_module_takes_the_speed_divide_...`) or where
/// it is smaller than the compact routine, as the signed pair is.
#[test]
fn a_long_remainder_calls_the_fast_routine_only_where_it_is_hot() {
    let cold = module("define i32 @f(i32 %a, i32 %b) {\n  %r = srem i32 %a, %b\n  ret i32 %r\n}");
    let looped = module(
        "define i32 @f(i32 %a, i32 %b, i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%j, %l]\n  \
         %s = phi i32 [%a, %entry], [%r, %l]\n  %r = srem i32 %s, %b\n  %j = add i32 %i, 1\n  \
         %c = icmp ult i32 %j, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %r\n}",
    );
    let from_a_loop = module(
        "define i32 @d(i32 %a, i32 %b) noinline {\n  %r = srem i32 %a, %b\n  ret i32 %r\n}\n\
         define i32 @f(i32 %a, i32 %b, i32 %n) {\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%j, %l]\n  \
         %s = phi i32 [%a, %entry], [%r, %l]\n  %r = call i32 @d(i32 %s, i32 %b)\n  %j = add i32 %i, 1\n  \
         %c = icmp ult i32 %j, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %r\n}",
    );
    let text = assembly(&cold, &Request::new(Level::O2)).unwrap();
    assert!(
        text.contains("jsr __modsi3_fast_small\n") && !text.contains("jsr __modsi3_fast\n"),
        "a cold site:\n{text}"
    );
    for (ir, what) in [(&looped, "a loop"), (&from_a_loop, "a function called from a loop")] {
        let text = assembly(ir, &Request::new(Level::O2)).unwrap();
        assert!(text.contains("jsr __modsi3_fast\n") && !text.contains("_small"), "{what}:\n{text}");
        let small = assembly(ir, &Request::new(Level::Os)).unwrap();
        assert!(small.contains("jsr __modsi3_fast_small\n"), "{what} at -Os:\n{small}");
    }
}

/// A long division whose divisor is known to be a byte (a constant, or a
/// zero-extended byte) called the general routine and linked the 141 B wide
/// path with it: `a % 5` cost +141 B for nothing. At speed it calls the byte
/// entry; a divisor of any width keeps its routine, and -Os never links the
/// wide path.
#[test]
fn a_long_division_by_a_known_byte_calls_the_byte_entry_at_speed() {
    for (opcode, routine) in [("udiv", "__udivsi3"), ("sdiv", "__divsi3"), ("urem", "__umodsi3"), ("srem", "__modsi3")]
    {
        let by_constant = module(&format!("define i32 @f(i32 %a) {{\n  %r = {opcode} i32 %a, 5\n  ret i32 %r\n}}"));
        let by_byte = module(&format!(
            "define i32 @f(i32 %a, i8 %b) {{\n  %d = zext i8 %b to i32\n  %r = {opcode} i32 %a, %d\n  ret i32 %r\n}}"
        ));
        let by_word = module(&format!(
            "define i32 @f(i32 %a, i32 %b, i32 %n) {{\nentry:\n  br label %l\nl:\n  %i = phi i32 [0, %entry], [%j, %l]\n  \
             %s = phi i32 [%a, %entry], [%r, %l]\n  %r = {opcode} i32 %s, %b\n  %j = add i32 %i, 1\n  \
             %c = icmp ult i32 %j, %n\n  br i1 %c, label %l, label %x\nx:\n  ret i32 %r\n}}"
        ));
        for (ir, what) in [(&by_constant, "a constant"), (&by_byte, "a zero-extended byte")] {
            let text = assembly(ir, &Request::new(Level::O2)).unwrap();
            assert!(text.contains(&format!("{routine}_fast_byte")), "{opcode} by {what}:\n{text}");
            let small = assembly(ir, &Request::new(Level::Os)).unwrap();
            assert!(!small.contains(&format!("{routine}_fast\n")), "{opcode} by {what} at -Os:\n{small}");
        }
        let text = assembly(&by_word, &Request::new(Level::O2)).unwrap();
        assert!(
            text.contains(&format!("{routine}_fast")) && !text.contains("_byte"),
            "{opcode} by a word in a loop:\n{text}"
        );
    }
}

/// A 16-bit loop counter tested with `ult` against its OLD value while the
/// stepped one is already made kept two copies of it: `stx rc3; sty rc2` at
/// the loop top and `lda rc3; cmp #63; lda rc2; sbc #0` after the step
/// (`shift_int64` -Os +5 B, `overflow_builtins` +9 B). The branch reads the
/// carry alone and `inx`/`iny` leave it, so the compare is made before the
/// step.
fn counter_loop(condition: &str) -> String {
    module(&format!(
        "@g = global i16 0\n\
         define i8 @t() {{\nentry:\n  br label %l\nl:\n  %i = phi i16 [0, %entry], [%j, %l]\n  \
         store volatile i16 %i, ptr @g\n  %j = add i16 %i, 1\n  %c = icmp {condition}\n  \
         br i1 %c, label %l, label %x\nx:\n  %v = load volatile i16, ptr @g\n  %b = trunc i16 %v to i8\n  ret i8 %b\n}}"
    ))
}

#[test]
fn a_carry_branch_on_the_old_counter_is_compared_before_the_step() {
    let ir = counter_loop("samesign ult i16 %i, 63");
    for level in [Level::Os, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        // `txa; cmp #63` is `cpx #63` where `a` is dead after it
        let (compare, step) = (text.find("cmp #63").or(text.find("cpx #63")), text.find("inx"));
        assert!(compare.is_some() && step.is_some(), "no compare or no step:\n{text}");
        assert!(compare < step, "the compare comes after the step:\n{text}");
        assert!(!text.contains("stx rc") && !text.contains("sty rc"), "a copy of the counter:\n{text}");
        assert_eq!(exit_code_of(&text, "t"), 63, "{text}");
    }
}

/// A branch on Z (`ne`) reads the flags the step sets: a compare of the old
/// counter must stay after it. Narrowing turns this loop into a `dex; bne`
/// down-counter, so only the order (when a compare remains) and the result
/// are pinned.
#[test]
fn a_zero_branch_on_the_old_counter_keeps_its_compare_after_the_step() {
    let ir = counter_loop("ne i16 %i, 63");
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap();
    if let (Some(compare), Some(step)) = (text.find("cmp #63"), text.find("inx")) {
        assert!(step < compare, "the compare moved above the step:\n{text}");
    }
    assert_eq!(exit_code_of(&text, "t"), 63, "{text}");
}

/// The compare of the NEW counter (`%j < 63`) must not move above the step:
/// it would test the old value and run one trip too many. The last value
/// stored is 62.
#[test]
fn a_compare_of_the_new_counter_stays_after_the_step() {
    let ir = counter_loop("samesign ult i16 %j, 63");
    for level in [Level::Os, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        assert_eq!(exit_code_of(&text, "t"), 62, "{text}");
    }
}

/// A down counter's step (`lda lo; bne; dec hi; dec lo`) takes the accumulator;
/// the compare that moves above it still sees the old value: the loop stores
/// 63 down to 5 and ends on 5.
#[test]
fn a_carry_branch_on_the_old_down_counter_runs_the_same_trips() {
    let ir = module(
        "@g = global i16 0\n\
         define i8 @t() {\nentry:\n  br label %l\nl:\n  %i = phi i16 [63, %entry], [%j, %l]\n  \
         store volatile i16 %i, ptr @g\n  %j = sub i16 %i, 1\n  %c = icmp samesign ugt i16 %i, 5\n  \
         br i1 %c, label %l, label %x\nx:\n  %v = load volatile i16, ptr @g\n  %b = trunc i16 %v to i8\n  ret i8 %b\n}",
    );
    for level in [Level::Os, Level::O2] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        assert_eq!(exit_code_of(&text, "t"), 5, "{text}");
    }
}

/// `select i16 -1, 1` left `lda #255; sta rc4; sta rc5` in front of the `bcs`
/// over its second arm: two stores to argument cells nothing reads, 6 B in
/// stricmp (84 against llvm-mos's 70).
#[test]
fn a_constant_pair_select_leaves_no_store_to_a_cell_nothing_reads() {
    let ir = module(
        "define i16 @f(ptr %p, i8 %b) {\n  %a = load i8, ptr %p\n  %c = icmp ult i8 %a, %b\n  %r = select i1 %c, i16 -1, i16 1\n  ret i16 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap_or_else(|e| panic!("{e}"));
    let body: String = text[text.find("__r_f:").unwrap()..].split(".segment").next().unwrap().to_owned();
    let stores: Vec<&str> = body.lines().filter(|l| l.trim_start().starts_with("st") && l.contains("rc")).collect();
    // `sta rc5` (the loaded byte kept for the compare) is read; the 255 stores
    // are not
    assert!(!body.lines().any(|l| l.trim() == "stx rc2" || l.trim() == "stx rc3"), "stores {stores:?}\n{body}");
}

/// `__cmpdf2`'s NaN test reaches the backend as a compare of `trunc i64 to i63`
/// (InstCombine's fold of `(x << 1) u> C`). The odd width went to `parts` as
/// 63/8 and refused ("integer wider than a power-of-two count of registers"),
/// so lib/fp/f64/cmp.c did not build. The pin promotes it to `i64` first.
#[test]
fn an_i63_compare_of_a_truncated_i64_compiles() {
    let ir = module(
        "define i32 @g(i64 %x, i64 %y) {\n  %a = trunc i64 %x to i63\n  \
         %c1 = icmp ult i63 %a, -4503599627370495\n  %b = trunc i64 %y to i63\n  \
         %c2 = icmp ult i63 %b, -4503599627370495\n  %o = and i1 %c1, %c2\n  \
         br i1 %o, label %t, label %f\nt:\n  %m = or i64 %x, %y\n  %z = icmp eq i64 %m, 0\n  \
         %r = zext i1 %z to i32\n  ret i32 %r\nf:\n  ret i32 1\n}\n",
    );
    assembly(&ir, &Request::new(Level::O2)).unwrap();
}

/// `store i16 (load i16 @s), @d`: the legalizer splits the load into two byte
/// loads joined by a token factor, and the split store took that token as its
/// chain, so both loads came before both stores (copy_runs +374 B / +92k clk
/// against llvm-mos: 8 instructions through X and Y, not 4). The pin's
/// `FindBetterChain` lets each store wait only for the loads it may alias.
#[test]
fn a_word_copy_between_globals_interleaves_its_byte_loads_and_stores() {
    let ir = module(
        r#"@s = global i16 0
@d = global i16 0
define void @copy() {
  %v = load i16, ptr @s
  store i16 %v, ptr @d
  ret void
}"#,
    );
    let text = assembly(&ir, &Request::new(Level::O2)).unwrap();
    let body: Vec<&str> =
        text.lines().map(str::trim).filter(|line| line.starts_with("lda") || line.starts_with("sta")).collect();
    let second_load = body.iter().rposition(|line| line.starts_with("lda")).unwrap_or(0);
    let first_store = body.iter().position(|line| line.starts_with("sta")).unwrap_or(usize::MAX);
    assert!(first_store < second_load, "both loads came before the first store:\n{text}");
}

/// `builtins_table` at -O0 failed with "intrinsic call llvm.is.constant.i16":
/// the O0 path skipped the step the pin runs at every level, so no object was
/// written (2026-11-08).
#[test]
fn is_constant_is_lowered_at_o0() {
    let ir = module(
        "@g = global i16 7\n\
         declare i1 @llvm.is.constant.i16(i16)\n\
         define i8 @t() {\nentry:\n  %v = load volatile i16, ptr @g\n  \
         %k = call i1 @llvm.is.constant.i16(i16 5)\n  %n = call i1 @llvm.is.constant.i16(i16 %v)\n  \
         %a = zext i1 %k to i8\n  %b = zext i1 %n to i8\n  %s = shl i8 %a, 1\n  %r = or i8 %s, %b\n  ret i8 %r\n}",
    );
    let text = assembly(&ir, &Request::new(Level::O0)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(exit_code_of(&text, "t"), 2, "{text}");
}

/// A call whose arguments are volatile loads of `g1`, `g2`, `g3` reads them in
/// source order at every level: every access of `g1` precedes every access of
/// `g2`, which precedes `g3` (m68k pushed them g3, g2, g1 at every level,
/// 2026-11-08). Read from the assembly; sim65 has no I/O address that logs
/// accesses, so this is an asm-order check only.
#[test]
fn volatile_arguments_are_read_in_source_order_at_every_level() {
    for (width, ty) in [("8", "i8"), ("16", "i16"), ("32", "i32")] {
        let ir = module(&format!(
            "@g1 = global {ty} 0\n@g2 = global {ty} 0\n@g3 = global {ty} 0\n\
             declare void @sink({ty}, {ty}, {ty})\n\
             define void @t() {{\nentry:\n  %a = load volatile {ty}, ptr @g1\n  %b = load volatile {ty}, ptr @g2\n  \
             %c = load volatile {ty}, ptr @g3\n  call void @sink({ty} %a, {ty} %b, {ty} %c)\n  ret void\n}}"
        ));
        for level in [Level::O0, Level::O1, Level::O2, Level::O3, Level::Os] {
            let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
            let from = text.find("\nt:").unwrap_or_else(|| panic!("no t in\n{text}"));
            let body = &text[from..];
            let body = body.split(".export").next().unwrap_or(body);
            let accesses: Vec<u8> = body
                .lines()
                .flat_map(|line| {
                    (1..=3u8).filter(move |k| {
                        let name = format!("g{k}");
                        line.split(|c: char| !c.is_alphanumeric() && c != '_').any(|word| word == name)
                    })
                })
                .collect();
            let mut sorted = accesses.clone();
            sorted.sort_unstable();
            assert!(
                sorted == accesses && (1..=3u8).all(|k| accesses.contains(&k)),
                "i{width} at {level:?}: accesses of g1, g2, g3 are {accesses:?}:\n{body}"
            );
        }
    }
}

/// A word pattern over 64 cells was a loop that scaled and added its index on
/// every trip (`asl`/`rol` of the index, `clc`/`adc` of the base: 17
/// instructions where a pointer step is 3). `fill_nest` at -O1 ran 429,137
/// clocks against gcc's 333,596; 70% of them were that loop (2026-11-10).
#[test]
fn a_pattern_fill_steps_a_pointer() {
    let ir = module(
        "declare void @llvm.experimental.memset.pattern.p0.i16.i16(ptr, i16, i16, i1)\n\
         define void @fill(ptr %p) {\n  call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr %p, i16 4660, i16 64, i1 false)\n  ret void\n}",
    );
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = assembly(&ir, &Request::new(level)).unwrap();
        assert!(!text.contains("asl") && !text.contains("rol"), "{level:?} scales an index per trip:\n{text}");
    }
}

/// The product of two values that each fit in a word was a 32 x 32 walk of
/// `__mulsi3`: fcmp_all spent 242K of its 1.055M clocks in it (2026-11-12).
/// It is one 16 x 16 -> 32 routine when both fit (GCC's widening multiply).
#[test]
fn a_product_of_two_words_calls_the_widening_multiply() {
    let product = |x: &str, y: &str| {
        module(&format!(
            "define i32 @f(i16 %a, i16 %b) {{\n  %x = {x} i16 %a to i32\n  %y = {y} i16 %b to i32\n  %m = mul i32 %x, %y\n  ret i32 %m\n}}"
        ))
    };
    for (x, y, routine) in [
        ("sext", "sext", "__mulhisi3"),
        ("zext", "zext", "__umulhisi3"),
        ("sext", "zext nneg", "__mulhisi3"),
        ("sext", "zext", "__mulsi3"),
    ] {
        let text = assembly(&product(x, y), &Request::new(Level::O2)).unwrap();
        assert!(text.contains(&format!("jsr {routine}")), "{x} * {y}:\n{text}");
        if routine != "__mulsi3" {
            assert!(!text.contains("__mulsi3"), "{x} * {y}:\n{text}");
            // the two routines add bytes: -O1 keeps the long multiply (-Os
            // prices it: widen_image.rs)
            let text = assembly(&product(x, y), &Request::new(Level::O1)).unwrap();
            assert!(!text.contains("mulhisi3"), "O1 {x} * {y}:\n{text}");
        }
    }
}

/// A multiplier whose high byte is known zero takes the entry that walks to its
/// last set bit: fcmp_all's `(F) * (k + 1)` was 414 clocks a call (2026-11-12).
#[test]
fn a_product_by_a_byte_calls_the_byte_entry() {
    let product = |x: &str| {
        module(&format!(
            "define i32 @f(i16 %a, i8 %b) {{\n  %x = {x} i16 %a to i32\n  %y = zext i8 %b to i32\n  %m = mul i32 %x, %y\n  ret i32 %m\n}}"
        ))
    };
    for (x, routine) in [("zext", "__umulhisi3_byte"), ("sext", "__mulhisi3_byte")] {
        let text = assembly(&product(x), &Request::new(Level::O2)).unwrap();
        assert!(text.contains(&format!("jsr {routine}")), "{x}:\n{text}");
    }
}

/// -Os is strict bytes: a word divide links the compact routine in every
/// module. The aligned one (16 B larger, quicker for a small quotient) made
/// k_tosudivax 82 B, a bare `jmp`, against sdcc's 64 and the compact build's
/// 58, and digits 151 B against 129.
#[test]
fn an_os_word_divide_is_the_compact_routine_in_every_module() {
    let divides = |inner: &str, outer: &str| {
        module(&format!(
            "define i16 @f(i16 %a, i16 %b, i16 %n, i16 %m) {{\nentry:\n  %d = or i16 %b, 1\n  br label %outer\n\
             outer:\n  %i = phi i16 [ 0, %entry ], [ %i1, %inner_end ]\n  %s = phi i16 [ 0, %entry ], [ %t3, %inner_end ]\n  br label %inner\n\
             inner:\n  %j = phi i16 [ 0, %outer ], [ %j1, %inner ]\n  %t = phi i16 [ %s, %outer ], [ %t3, %inner ]\n  \
             %x = add i16 %t, %a\n  {inner}\n  %t3 = add i16 %r2, 7\n  \
             %j1 = add nuw i16 %j, 1\n  %c = icmp ult i16 %j1, %m\n  br i1 %c, label %inner, label %inner_end\n\
             inner_end:\n  %i1 = add nuw i16 %i, 1\n  %o = icmp ult i16 %i1, %n\n  br i1 %o, label %outer, label %out\n\
             out:\n  {outer}\n  ret i16 %r3\n}}"
        ))
    };
    let in_loop =
        divides("%r1 = srem i16 %x, %b\n  %y = add i16 %r1, %j\n  %r2 = srem i16 %y, %d", "%r3 = add i16 %t3, 1");
    let text = assembly(&in_loop, &Request::new(Level::Os)).unwrap();
    assert!(text.contains("__modhi3c") && !text.contains("__modhi3a"), "a divide in a loop:\n{text}");
    let once = module("define i16 @g(i16 %a, i16 %b) {\n  %r = srem i16 %a, %b\n  ret i16 %r\n}");
    let text = assembly(&once, &Request::new(Level::Os)).unwrap();
    assert!(text.contains("__modhi3c") && !text.contains("__modhi3a"), "the only work of the module:\n{text}");
}

/// At -Os the long multiply is the 32-step compact walk (mul_constants: 42.0M
/// clocks in 200-trip loops, llvm-mos 13.6M); `__mulsi3_fast` stops at the
/// multiplier's last byte and is 19 B larger, linked once. A module takes
/// it where the share of its clocks it saves is worth the share of its bytes it
/// adds: multiplies in its loops, not one beside loops of other work.
#[test]
fn a_module_takes_the_speed_multiply_where_the_clocks_it_saves_are_its_own() {
    let padding: String = (0..60).map(|k| format!("  store volatile i32 %a, ptr @g{}\n", k % 3)).collect();
    let multiplies = |inner: &str, outer: &str| {
        module(&format!(
            "@g0 = global i32 0\n@g1 = global i32 0\n@g2 = global i32 0\n\
             define i32 @f(i32 %a, i32 %b, i16 %n, i16 %m) {{\nentry:\n  {padding}  br label %outer\n\
             outer:\n  %i = phi i16 [ 0, %entry ], [ %i1, %inner_end ]\n  %s = phi i32 [ 0, %entry ], [ %t3, %inner_end ]\n  br label %inner\n\
             inner:\n  %j = phi i16 [ 0, %outer ], [ %j1, %inner ]\n  %t = phi i32 [ %s, %outer ], [ %t3, %inner ]\n  \
             %x = add i32 %t, %a\n  {inner}\n  %t3 = add i32 %r2, 7\n  \
             %j1 = add nuw i16 %j, 1\n  %c = icmp ult i16 %j1, %m\n  br i1 %c, label %inner, label %inner_end\n\
             inner_end:\n  %i1 = add nuw i16 %i, 1\n  %o = icmp ult i16 %i1, %n\n  br i1 %o, label %outer, label %out\n\
             out:\n  {outer}\n  ret i32 %r3\n}}"
        ))
    };
    let in_loop = multiplies("%r1 = mul i32 %x, %b\n  %r2 = mul i32 %r1, %a", "%r3 = add i32 %t3, 1");
    let text = assembly(&in_loop, &Request::new(Level::Os)).unwrap();
    assert!(text.contains("__mulsi3_fast"), "{text}");
    let work = "store volatile i32 %x, ptr @g0\n  ".repeat(12);
    let beside = multiplies(
        &format!("{work}%r2 = shl i32 %x, 3"),
        "%p1 = mul i32 %t3, %b\n  %p2 = mul i32 %p1, %a\n  %p3 = mul i32 %p2, %b\n  %r3 = mul i32 %p3, %t3",
    );
    let text = assembly(&beside, &Request::new(Level::Os)).unwrap();
    assert!(text.contains("jsr __mulsi3\n") && !text.contains("__mulsi3_fast"), "{text}");
}

/// At -Os a long divide is the compact 32-step walk (loop_interchange: 13.9M
/// clocks, llvm-mos 3.3M); the speed routines start at the quotient's first bit
/// and are larger. A module takes them where the share of its clocks they save
/// is worth the share of its bytes they add: divides in its loops, not one
/// beside loops of other work.
#[test]
fn a_module_takes_the_speed_divide_where_the_clocks_it_saves_are_its_own() {
    let padding: String = (0..60).map(|k| format!("  store volatile i32 %a, ptr @g{}\n", k % 3)).collect();
    let divides = |inner: &str, outer: &str| {
        module(&format!(
            "@g0 = global i32 0\n@g1 = global i32 0\n@g2 = global i32 0\n\
             define i32 @f(i32 %a, i32 %b, i16 %n, i16 %m) {{\nentry:\n  {padding}  br label %outer\n\
             outer:\n  %i = phi i16 [ 0, %entry ], [ %i1, %inner_end ]\n  %s = phi i32 [ 0, %entry ], [ %t3, %inner_end ]\n  br label %inner\n\
             inner:\n  %j = phi i16 [ 0, %outer ], [ %j1, %inner ]\n  %t = phi i32 [ %s, %outer ], [ %t3, %inner ]\n  \
             %x = add i32 %t, %a\n  {inner}\n  %t3 = add i32 %r2, 7\n  \
             %j1 = add nuw i16 %j, 1\n  %c = icmp ult i16 %j1, %m\n  br i1 %c, label %inner, label %inner_end\n\
             inner_end:\n  %i1 = add nuw i16 %i, 1\n  %o = icmp ult i16 %i1, %n\n  br i1 %o, label %outer, label %out\n\
             out:\n  {outer}\n  ret i32 %r3\n}}"
        ))
    };
    let in_loop = divides("%r1 = urem i32 %x, %b\n  %r2 = urem i32 %r1, %a", "%r3 = add i32 %t3, 1");
    let text = assembly(&in_loop, &Request::new(Level::Os)).unwrap();
    assert!(text.contains("jsr __umodsi3_fast_small"), "{text}");
    let work = "store volatile i32 %x, ptr @g0\n  ".repeat(12);
    let beside = divides(
        &format!("{work}%r2 = shl i32 %x, 3"),
        "%p1 = urem i32 %t3, %b\n  %p2 = urem i32 %p1, %a\n  %p3 = urem i32 %p2, %b\n  %r3 = urem i32 %p3, %t3",
    );
    let text = assembly(&beside, &Request::new(Level::Os)).unwrap();
    assert!(text.contains("jsr __umodsi3\n") && !text.contains("_fast"), "{text}");
}
