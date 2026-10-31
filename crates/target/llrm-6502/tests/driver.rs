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
/// loop.
#[test]
fn a_copy_with_nothing_live_across_it_stays_a_call() {
    let ir = module(
        r#"declare void @llvm.memcpy.p0.p0.i16(ptr noalias, ptr noalias, i16, i1)
define void @copy(ptr noalias %d, ptr noalias %s, i16 %n) {
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
/// `cpx rc2`); 163 compares of the corpus' loops did.
#[test]
fn a_loop_bound_in_x_is_compared_with_cpx() {
    let ir = module(
        "@buf = global [16 x i8] zeroinitializer\n\
         define void @f(i8 %n) {\n\
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

/// Fills `@src` with its own index, copies `%n` bytes of it plus one to `@dst`
/// by two pointers (not a `memcpy`), and folds all of `@dst` (`t * 3 + byte`)
/// into a byte.
const COUNTED_COPY: &str = "@src = global [600 x i8] zeroinitializer
@dst = global [600 x i8] zeroinitializer
define i8 @f(i16 %n) {
entry:
  br label %fill
fill:
  %i = phi i16 [ 0, %entry ], [ %i2, %fill ]
  %p = getelementptr inbounds i8, ptr @src, i16 %i
  %v = trunc i16 %i to i8
  store i8 %v, ptr %p
  %i2 = add i16 %i, 1
  %c = icmp ult i16 %i2, 600
  br i1 %c, label %fill, label %guard
guard:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %sum, label %pre
pre:
  br label %copy
copy:
  %sp = phi ptr [ @src, %pre ], [ %sp2, %copy ]
  %dp = phi ptr [ @dst, %pre ], [ %dp2, %copy ]
  %k = phi i16 [ %n, %pre ], [ %k2, %copy ]
  %x = load i8, ptr %sp
  %x1 = add i8 %x, 1
  store i8 %x1, ptr %dp
  %sp2 = getelementptr inbounds i8, ptr %sp, i16 1
  %dp2 = getelementptr inbounds i8, ptr %dp, i16 1
  %k2 = sub nuw i16 %k, 1
  %d = icmp ne i16 %k2, 0
  br i1 %d, label %copy, label %sum
sum:
  %j = phi i16 [ 0, %guard ], [ 0, %copy ], [ %j2, %sum ]
  %t = phi i8 [ 0, %guard ], [ 0, %copy ], [ %t2, %sum ]
  %q = getelementptr inbounds i8, ptr @dst, i16 %j
  %y = load i8, ptr %q
  %t3 = mul i8 %t, 3
  %t2 = add i8 %t3, %y
  %j2 = add i16 %j, 1
  %m = icmp ult i16 %j2, 600
  br i1 %m, label %sum, label %end
end:
  ret i8 %t2
}";

/// A counted copy was `lda (s),y; sta (d),y; iny; bne` only inside the page
/// bumps `pageindex` makes after selection, with a 16-bit count stepped every
/// trip beside it (`memcpy`, 40 clocks a byte). Split into windows of a page
/// by the mid-end, one index counts the window and the trip is the four
/// instructions alone (lz4, 37126 clocks against the hand-written 12870).
#[test]
fn a_counted_copy_runs_a_page_at_a_time_on_one_index() {
    let text = assembly(&module(COUNTED_COPY), &Request::new(Level::O2)).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let step = lines.windows(2).position(|pair| pair[0] == "  iny" && pair[1].starts_with("  bne __r_f_b"));
    let start = step.and_then(|at| lines[..at].iter().rposition(|line| !line.starts_with("  ")));
    let trip = start.zip(step).map(|(start, at)| &lines[start + 1..at + 2]);
    // `lda (s),y; clc; adc #1; sta (d),y; iny; bne`: no count in the trip.
    assert!(
        trip.is_some_and(|trip| trip.len() == 6
            && trip[0].starts_with("  lda (rc")
            && trip[3].starts_with("  sta (rc")),
        "the trip is not a load, an add, a store and the index:\n{text}"
    );
}

/// The windows' edges: the copy and the fold after it agree with the host's at
/// lengths around one and two pages. With the page loop's index started at
/// the wrong place the bytes landed a window off, and the fold said so.
#[test]
fn a_counted_copy_is_exact_around_the_page_edges() {
    let text = assembly(&module(COUNTED_COPY), &Request::new(Level::O2)).unwrap();
    let functions: String =
        text.lines().filter(|line| !line.starts_with("rc")).map(|line| format!("{line}\n")).collect();
    for n in [0u16, 1, 2, 255, 256, 257, 300, 511, 512, 513, 599] {
        let fold = (0..600u16)
            .fold(0u8, |t, j| t.wrapping_mul(3).wrapping_add(if j < n { (j as u8).wrapping_add(1) } else { 0 }));
        let main = format!("  lda #{}\n  sta rc2\n  lda #{}\n  sta rc3\n  jsr __r_f\n", n & 0xFF, n >> 8);
        assert_eq!(common::run_with(&format!("copy-{n}"), &main, &functions), i32::from(fold), "length {n}");
    }
}
