//! A `<8 x i16>` load and store: arm64 refused the function ("a vector value")
//! and the vectorizer had nothing to emit. Hand IR, since no pass makes a
//! vector yet (ARM64-VEC POC, scroll's copy loops).

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(body: &str) -> String {
    assembly_with(true, body)
}

fn assembly_with(
    vectors: bool,
    body: &str,
) -> String {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{body}")).unwrap();
    let mut llc = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"));
    llc.env("LLRM_ARM64_VECTORS", if vectors { "1" } else { "0" });
    let said = llc.args(["-S", "-O2", "-o"]).arg(&output).arg(&input).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8(std::fs::read(&output).unwrap()).unwrap()
}

#[test]
fn a_vector_copy_is_a_q_load_and_store() {
    let text = assembly(
        "define void @copy(ptr %d, ptr %s) {\n\
         \x20 %v = load <8 x i16>, ptr %s, align 16\n\
         \x20 store <8 x i16> %v, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    let reg = |op: &str| {
        text.lines().find_map(|l| {
            l.trim().strip_prefix(op)?.trim_start().strip_prefix('q')?.split(',').next().map(str::to_owned)
        })
    };
    let (loaded, stored) = (reg("ldr"), reg("str"));
    assert!(loaded.is_some() && loaded == stored, "{text}");
}

/// A copy loop whose store is 80 elements ahead of its load (loop-idiom leaves
/// it alone) stayed 1920 scalar `ldrh`/`strh` iterations; the vectorizer makes
/// it eight lanes per `ldr q`/`str q`.
const OVERLAPPING_COPY: &str = "define void @f(ptr noalias %a) {\n\
         entry:\n  br label %loop\n\
         loop:\n\
         \x20 %i = phi i64 [ 0, %entry ], [ %next, %loop ]\n\
         \x20 %s = getelementptr inbounds i16, ptr %a, i64 %i\n\
         \x20 %v = load i16, ptr %s, align 2\n\
         \x20 %j = add nuw nsw i64 %i, 80\n\
         \x20 %d = getelementptr inbounds i16, ptr %a, i64 %j\n\
         \x20 store i16 %v, ptr %d, align 2\n\
         \x20 %next = add nuw nsw i64 %i, 1\n\
         \x20 %done = icmp eq i64 %next, 1920\n\
         \x20 br i1 %done, label %exit, label %loop\n\
         exit:\n  ret void\n}\n";

#[test]
fn an_overlapping_copy_loop_runs_on_q_registers() {
    let text = assembly(OVERLAPPING_COPY);
    assert!(text.contains("ldr\tq") || text.contains("ldr q"), "{text}");
    assert!(!text.contains("ldrh"), "{text}");
}

/// With no flag set the pass ran nowhere: the copy loop stayed 1920 scalar
/// `ldrh` iterations (scroll: 1.19M executed instructions at -O2, 0.47M
/// vectorized). On is the default; `LLRM_ARM64_VECTORS=0` is the off-switch.
#[test]
fn the_vectorizer_runs_with_no_flag_set() {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, format!("{HEAD}{OVERLAPPING_COPY}")).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env_remove("LLRM_ARM64_VECTORS")
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    let text = std::fs::read_to_string(&output).unwrap();
    assert!(text.contains("ldr\tq") || text.contains("ldr q"), "{text}");
}

/// The same loop with the flag at `0` stays the scalar `ldrh`/`strh`
/// loop: no `q` register appears.
#[test]
fn the_flag_off_leaves_the_copy_loop_scalar() {
    let text = assembly_with(false, OVERLAPPING_COPY);
    assert!(text.contains("ldrh") && !text.contains("ldr\tq") && !text.contains("ldr q"), "{text}");
}

/// The splat the vectorizer makes for a fill (`insertelement` + `shufflevector`
/// of an argument, or a constant vector) was refused by instruction selection;
/// it is a `dup`/`movi` into a q register and one `str q`. Hand IR: the
/// pipeline's loop-idiom turns a word fill into a `memset.pattern` call (a
/// DEVIATE of the pin) before the vectorizer sees it.
#[test]
fn a_splat_store_is_a_q_register_store() {
    let argument = assembly(
        "define void @f(ptr %d, i16 %x) {\n\
         \x20 %h = insertelement <8 x i16> poison, i16 %x, i64 0\n\
         \x20 %s = shufflevector <8 x i16> %h, <8 x i16> poison, <8 x i32> zeroinitializer\n\
         \x20 store <8 x i16> %s, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(argument.contains("dup") && (argument.contains("str\tq") || argument.contains("str q")), "{argument}");
    let constant = assembly(
        "define void @g(ptr %d) {\n\
         \x20 store <8 x i16> <i16 7, i16 7, i16 7, i16 7, i16 7, i16 7, i16 7, i16 7>, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(constant.contains("movi") && (constant.contains("str\tq") || constant.contains("str q")), "{constant}");
    assert!(!constant.contains("strh"), "{constant}");
}

/// A byte-mask double (and a float with only its top byte set) with vectors on
/// stopped the compile: "xform 36/37 is not translated" (jpeg_single -O3).
/// They are `movi` forms, as in the pin.
#[test]
fn a_mask_constant_is_a_movi() {
    let text = assembly(
        "define double @d() {\n  ret double 0x00FF00FF00FF00FF\n}\n\
         define float @f() {\n  ret float 0x47E0000000000000\n}\n",
    );
    // The immediates too: the two xforms were once swapped (`movi d0, #0`).
    assert!(text.contains("movi d0, #0xff00ff00ff00ff"), "{text}");
    assert!(text.contains("#127, lsl #24"), "{text}");
}

/// A byte loaded into lane 0 of a vector (a scalarised load of the vectorizer)
/// stopped the compile: "machine opcode LDRBroX has no arm64 instruction yet"
/// (jpeg_single). It is the pin's `ldr b0, [x, x]`.
#[test]
fn a_byte_loaded_into_a_lane_is_a_b_register_load() {
    let text = assembly(
        "define void @f(ptr %p, i64 %i, ptr %q) {\n\
         \x20 %a = getelementptr i8, ptr %p, i64 %i\n\
         \x20 %b = load i8, ptr %a, align 1\n\
         \x20 %z = zext i8 %b to i64\n\
         \x20 %v = insertelement <2 x i64> poison, i64 %z, i64 0\n\
         \x20 store <2 x i64> %v, ptr %q, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(text.contains("ldr\tb") || text.contains("ldr b"), "{text}");
}

/// A splat of 1824 (no `movi` form) stopped the compile: "custom lowering of
/// BuildVector" (scroll). The pin materialises the constant and `dup`s it.
#[test]
fn a_constant_splat_without_a_movi_form_is_a_dup() {
    let text = assembly(
        "define void @f(ptr %d) {\n\
         \x20 store <8 x i16> <i16 1824, i16 1824, i16 1824, i16 1824, i16 1824, i16 1824, i16 1824, i16 1824>, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(text.contains("#1824") && text.contains("dup.8h"), "{text}");
}

/// An all-ones `<2 x i64>` panicked ("attempt to subtract with overflow" in the
/// lane mask of `lower_build_vector`; jpeg_fixed, jpeg_single).
#[test]
fn an_all_ones_i64_vector_is_a_movi() {
    let text = assembly(
        "define void @f(ptr %d) {\n\
         \x20 store <2 x i64> <i64 -1, i64 -1>, ptr %d, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(text.contains("movi") && text.contains("0xffffffffffffffff"), "{text}");
}

/// A general register in lane 0 of a vector stopped the compile ("a store of
/// something that is no register", jpeg_single): the pin's `fmov d0, x1`.
#[test]
fn a_general_register_in_lane_zero_is_an_fmov() {
    let wide = assembly(
        "define void @f(ptr %q, i64 %x) {\n\
         \x20 %v = insertelement <2 x i64> poison, i64 %x, i64 0\n\
         \x20 store <2 x i64> %v, ptr %q, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(wide.contains("fmov d") && wide.contains("x1"), "{wide}");
    let narrow = assembly(
        "define void @f(ptr %q, i32 %x) {\n\
         \x20 %v = insertelement <4 x i32> poison, i32 %x, i64 0\n\
         \x20 store <4 x i32> %v, ptr %q, align 16\n\
         \x20 ret void\n}\n",
    );
    assert!(narrow.contains("fmov s") && narrow.contains("w1"), "{narrow}");
}

/// A byte converted to a float with vectors on stopped the compile ("machine
/// opcode UCVTFv1i32 has no arm64 instruction yet", jpeg_single): the pin loads
/// it into `b0` and converts in the vector register.
#[test]
fn a_loaded_byte_converted_to_float_stays_in_the_vector_register() {
    let text = assembly(
        "define float @f(ptr %p, i64 %i) {\n\
         \x20 %a = getelementptr i8, ptr %p, i64 %i\n\
         \x20 %b = load i8, ptr %a, align 1\n\
         \x20 %f = uitofp i8 %b to float\n\
         \x20 ret float %f\n}\n",
    );
    assert!(text.contains("ldr b") && text.contains("ucvtf s0, s"), "{text}");
}

/// A splat made in the vector loop's preheader and stored in the loop (a fill
/// of an argument over a run-time count, the vectorizer's `vector.ph`): the
/// vector value crossed a block and the compile stopped with "@f: a copy of a
/// constant" (jpeg_single -O3 once the vectorizer made remainders). It is a q
/// register copy across the edge.
#[test]
fn a_splat_made_in_the_preheader_is_stored_in_the_loop() {
    let text = assembly(
        "define void @f(ptr noalias %d, i64 %v, i64 %n) {\n\
         entry:\n\
         \x20 br label %ph\n\
         ph:\n\
         \x20 %h = insertelement <2 x i64> poison, i64 %v, i64 0\n\
         \x20 %s = shufflevector <2 x i64> %h, <2 x i64> poison, <2 x i32> zeroinitializer\n\
         \x20 br label %loop\n\
         loop:\n\
         \x20 %i = phi i64 [ 0, %ph ], [ %next, %loop ]\n\
         \x20 %p = getelementptr inbounds i64, ptr %d, i64 %i\n\
         \x20 store <2 x i64> %s, ptr %p, align 8\n\
         \x20 %next = add nuw i64 %i, 2\n\
         \x20 %done = icmp eq i64 %next, %n\n\
         \x20 br i1 %done, label %exit, label %loop\n\
         exit:\n\
         \x20 ret void\n}\n",
    );
    assert!(text.contains("dup") && (text.contains("str\tq") || text.contains("str q")), "{text}");
}

/// `shl`, `lshr`, `ashr` of `<8 x i16>` by a constant refused with
/// `CannotSelect("Shl")`: the legalizer truncated the vector shift amount to a
/// scalar (`getShiftAmountOperand` leaves a vector's alone), so
/// `LowerVectorSRA_SRL_SHL` saw no splat.
#[test]
fn a_vector_shift_by_a_constant_is_one_shift_instruction() {
    let three = "<i16 3, i16 3, i16 3, i16 3, i16 3, i16 3, i16 3, i16 3>";
    for (op, mnemonic) in [("shl", "shl"), ("lshr", "ushr"), ("ashr", "sshr")] {
        let text = assembly(&format!(
            "define void @f(ptr %d, ptr %s) {{\n  %v = load <8 x i16>, ptr %s, align 16\n  %r = {op} <8 x i16> %v, {three}\n  store <8 x i16> %r, ptr %d, align 16\n  ret void\n}}\n"
        ));
        assert!(text.contains(&format!("{mnemonic} v")) && text.contains(".8h") && text.contains("#3"), "{op}: {text}");
    }
}

/// The three-register forms: one instruction each, on the lanes of the type.
#[test]
fn a_vector_binary_operation_is_one_instruction_on_its_lanes() {
    for (op, lanes, mnemonic, arrangement) in [
        ("add", "<8 x i16>", "add", ".8h"),
        ("sub", "<4 x i32>", "sub", ".4s"),
        ("mul", "<16 x i8>", "mul", ".16b"),
        ("and", "<2 x i64>", "and", ".16b"),
        ("xor", "<8 x i16>", "eor", ".16b"),
        ("or", "<4 x i32>", "orr", ".16b"),
    ] {
        let text = assembly(&format!(
            "define void @f(ptr %d, ptr %a, ptr %b) {{\n  %x = load {lanes}, ptr %a, align 16\n  %y = load {lanes}, ptr %b, align 16\n  %r = {op} {lanes} %x, %y\n  store {lanes} %r, ptr %d, align 16\n  ret void\n}}\n"
        ));
        assert!(text.contains(&format!("{mnemonic} v")) && text.contains(arrangement), "{op} {lanes}: {text}");
    }
}

/// An inner loop storing `i16` and `i64` in the same iteration was widened to
/// `<8 x i16>` and `<8 x i64>` (bandwidth maximization); the backend cannot
/// split the second, and the compile stopped with "vector with no register
/// class" (tests/run/c/pattern_fill.c at -O2). Non-constant values, so
/// loop-idiom leaves the stores.
#[test]
fn a_loop_storing_two_element_widths_compiles() {
    let text = assembly(
        "@a = global [64 x i16] zeroinitializer, align 2\n\
         @b = global [64 x i64] zeroinitializer, align 8\n\
         declare void @report(i64)\n\
         define void @f(i64 %t, i64 %m, i16 %x, i64 %y) {\n\
         entry:\n  br label %outer\n\
         outer:\n\
         \x20 %k = phi i64 [ %t, %entry ], [ %k1, %next ]\n  br label %loop\n\
         loop:\n\
         \x20 %i = phi i64 [ 0, %outer ], [ %n, %loop ]\n\
         \x20 %p = getelementptr inbounds i16, ptr @a, i64 %i\n\
         \x20 store i16 %x, ptr %p, align 2\n\
         \x20 %q = getelementptr inbounds i64, ptr @b, i64 %i\n\
         \x20 store i64 %y, ptr %q, align 8\n\
         \x20 %n = add nuw nsw i64 %i, 1\n\
         \x20 %c = icmp eq i64 %n, %m\n\
         \x20 br i1 %c, label %next, label %loop\n\
         next:\n\
         \x20 call void @report(i64 %k)\n\
         \x20 %k1 = add nsw i64 %k, -1\n\
         \x20 %more = icmp sgt i64 %k, 1\n\
         \x20 br i1 %more, label %outer, label %exit\n\
         exit:\n  ret void\n}\n",
    );
    assert!(text.contains("report"), "{text}");
}
