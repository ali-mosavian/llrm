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
    text.lines().filter(|line| line.trim() == format!("lda {cell}")).count()
}

/// A pointer is stored to its spill cell at entry; a path that does not write
/// its register used to load it again from the cell before the call (a
/// `lda`/`sta` pair a byte, twice, on each of two paths: 12 bytes of the 6-line
/// `f` below), because the spiller reloads before every use. Only the load
/// after the call, which took the register, is needed.
#[test]
fn a_spilled_value_is_not_reloaded_where_its_register_is_untouched() {
    llrm_6502::alloc::with_spike_allocator(|| {
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
    });
}

/// The control of the test above: a call takes the register, so the value is
/// loaded again after each one.
#[test]
fn a_spilled_value_is_reloaded_after_each_call_that_takes_its_register() {
    llrm_6502::alloc::with_spike_allocator(|| {
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
    });
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
fn crc16_is_the_88_byte_loop_and_the_bound_sees_the_table() {
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
    let dir = std::env::temp_dir().join(format!("llrm6502-run-{}-{function}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let main = format!(
        ".export _main\n_main:\n  lda #0\n  sta rc0\n  lda #$80\n  sta rc1\n  jsr {function}\n  ldx #0\n  rts\n"
    );
    std::fs::write(dir.join("t.s"), format!("{text}\n{main}")).unwrap();
    common::pipeline::tool(&dir, "cl65", &["-t", "sim6502", "-o", "t.bin", "t.s"])
        .unwrap_or_else(|e| panic!("{e}\n{text}"));
    let code = std::process::Command::new("sim65").arg(dir.join("t.bin")).status().unwrap().code().unwrap();
    std::fs::remove_dir_all(dir).ok();
    code
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

/// A region split left the complement of a loop-carried value a phi-only range
/// with no instruction naming it, and the allocator panicked ("Non-empty but
/// used interval"; 2 of 300 difftest programs). LLVM removes such an interval.
#[test]
fn a_split_that_leaves_a_range_without_uses_compiles() {
    let ir = include_str!("driver/range-left-without-uses.ll");
    let text = assembly(ir, &Request::new(Level::O2)).expect("compiles");
    assert!(text.contains("test:"), "{text}");
}
