//! Machine opcodes the DAG path emitted as "has no arm64 instruction yet":
//! 16 of the corpus's bench kernels (queens `place`, ...) were refused by
//! `--isel=dag` and fell to the hand selector.

use std::process::Command;

const HEADER: &str = "target triple = \"arm64-apple-macosx\"\n\
target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn dag(body: &str) -> Result<String, String> {
    dag_at("--no-opt", body)
}

fn dag_at(
    level: &str,
    body: &str,
) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, format!("{HEADER}{body}")).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", level, "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if said.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&said.stderr).into_owned())
    }
}

/// A whole module through the DAG path and the shared greedy allocator.
fn full_greedy(text: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, text).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_ARM64_GREEDY", "1")
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if said.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&said.stderr).into_owned())
    }
}

/// A whole module through the DAG path.
fn full(text: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, text).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if said.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&said.stderr).into_owned())
    }
}

/// `add x, y, sext(z)` is the pin's `ADDXrx` (`add x0, x0, w1, sxtw`).
#[test]
fn an_add_of_a_sign_extended_word_is_the_extended_register_add() {
    let asm =
        dag("define i64 @f(i64 %a, i32 %b) {\n  %e = sext i32 %b to i64\n  %r = add i64 %a, %e\n  ret i64 %r\n}\n")
            .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("add x0, x0, w1, sxtw"), "{asm}");
}

/// The high half of a 64x64 multiply is `SMULHrr`.
#[test]
fn the_high_half_of_a_signed_multiply_is_smulh() {
    let asm = dag("define i64 @f(i64 %a, i64 %b) {\n  %x = sext i64 %a to i128\n  %y = sext i64 %b to i128\n  %m = mul i128 %x, %y\n  %h = lshr i128 %m, 64\n  %r = trunc i128 %h to i64\n  ret i64 %r\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("smulh x0, x0, x1"), "{asm}");
}

/// A pointer stepped past each element is a post-index store: the pin selects
/// `STRXpost` (`str x1, [x0], #8`), a node with two results.
#[test]
fn a_store_that_steps_its_pointer_is_post_indexed() {
    let asm = dag("define ptr @f(ptr %p, i64 %v) {\n  store i64 %v, ptr %p, align 8\n  %n = getelementptr i64, ptr %p, i64 1\n  ret ptr %n\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("str x1, [x0], #8"), "{asm}");
}

/// A load whose pointer is also stepped is `LDRXpost` (`ldr x8, [x0], #8`):
/// its second result is the value, which `operand` took for the flags, so
/// the copy of the value to its register was "a copy of a constant" and the
/// function was refused.
#[test]
fn a_load_that_steps_its_pointer_is_post_indexed() {
    let asm = dag("define i64 @h(ptr %p, ptr %out) {\n  %v = load i64, ptr %p, align 8\n  %n = getelementptr i64, ptr %p, i64 1\n  store ptr %n, ptr %out, align 8\n  ret i64 %v\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("[x0], #8"), "{asm}");
}

/// A `memset` the legalizer turned into a libcall has an `ExternalSymbol`
/// callee; the DAG path refused it as `CannotSelect("ExternalSymbol")`, which
/// stopped 8 of the corpus's bench kernels (bintree, ...) at `--isel=dag`.
#[test]
fn a_libcall_the_legalizer_made_is_a_direct_call() {
    let asm = dag("declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\ndefine void @f(ptr %p, i64 %n) {\n  call void @llvm.memset.p0.i64(ptr %p, i8 0, i64 %n, i1 false)\n  ret void\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("bl _memset") || asm.contains("bl _bzero"), "{asm}");
}

/// A float multiplied by a power of two and converted is the fixed-point
/// convert (`fcvtzs x0, s0, #5`); `FCVTZSSXSri` / `SCVTFSWDri` had no row and
/// refused fpbench.
#[test]
fn a_scaled_float_conversion_is_the_fixed_point_form() {
    let asm = dag("define i64 @f(float %x) {\n  %m = fmul float %x, 32.0\n  %r = fptosi float %m to i64\n  ret i64 %r\n}\ndefine double @g(i32 %x) {\n  %f = sitofp i32 %x to double\n  %m = fmul double %f, 0.125\n  ret double %m\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("fcvtzs x0, s0, #5"), "{asm}");
    assert!(asm.contains("scvtf d0, w0, #3"), "{asm}");
}

/// `TargetOpcode::COPY` is no pattern output, so it had no name in the table
/// and the node was refused ("a machine node with no name"): sieve's
/// `m = (unsigned short)(m + i)` loop.
#[test]
fn a_selector_made_copy_node_is_a_register_copy() {
    let asm = full(include_str!("data/sieve_kernel.ll")).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("strb"), "{asm}");
    assert!(asm.contains("bench_sieve"), "{asm}");
}

/// Two compares joined by `and` are one `cmp` and one `ccmp`
/// (`ccmp x0, x8, #2, lo`); `CCMPXr` had no row and refused sieve.
#[test]
fn two_joined_compares_are_a_compare_and_a_conditional_compare() {
    let asm = dag("define i1 @f(i64 %a, i64 %b) {\n  %x = icmp ult i64 %a, %b\n  %y = icmp ult i64 %a, 32\n  %r = and i1 %x, %y\n  ret i1 %r\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("ccmp x0, x9, #2, lo"), "{asm}");
}

/// Inserting a byte into a word is the pin's `BFMWri` (`bfxil w0, w1, #0, #8`):
/// it reads the register it writes, so its result is a copy of the first
/// operand; the DAG path refused scroll with `machine opcode BFMWri has no
/// arm64 instruction yet`.
#[test]
fn inserting_a_byte_into_a_word_is_a_bitfield_move() {
    let asm = dag("define i32 @f(i32 %a, i32 %b) {\n  %m = and i32 %a, -256\n  %n = and i32 %b, 255\n  %r = or i32 %m, %n\n  ret i32 %r\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("bfm w0, w1, #0, #7"), "{asm}");
}

/// The DAG path leaves a copy per loop-carried value and one per float zero
/// (`sub w9, w0, #1; orr x0, xzr, x9`; `fmov d16, xzr; fmov d0, d16`) where the
/// hand selector writes in place: the arm64 linear scan does not coalesce, and
/// the shared PHIElimination/RegisterCoalescer do not run on arm64 yet. +6.7%
/// static instructions over the corpus at -O2.
#[test]
fn the_dag_path_leaves_no_register_copy_in_a_float_loop() {
    let asm = full_greedy(include_str!("data/float_loop_copies.ll")).unwrap_or_else(|why| panic!("{why}"));
    let body = asm.split(".Lf.2:").nth(1).and_then(|rest| rest.split("ret").next()).unwrap_or_default();
    let copies = body
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("fmov d") && l.rsplit(", ").next().is_some_and(|src| src.starts_with('d')))
        .chain(
            body.lines()
                .map(str::trim)
                .filter(|l| l.starts_with("orr x") && l.contains("xzr, x") && !l.contains("xzr, xzr")),
        )
        .count();
    assert_eq!(copies, 0, "{asm}");
}

/// A `br` on `a | b | c` is several branches in blocks the lowering made. The
/// phi copies went on the first block's edges only and `branch_targets` read
/// the branches after selection (none), so `queens`'s `place` left the phi
/// register unwritten on the other edges: the shared coalescer asserted on it,
/// the linear scan read a stale register.
#[test]
fn every_branch_of_a_split_condition_carries_the_phi_copies() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .env("LLRM_ARM64_VERIFY", "1")
        .args(["-S", "--no-opt", "--isel=dag", "-o"])
        .arg(dir.path().join("t.s"))
        .arg("tests/data/queens_place.ll")
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{said}");
}

/// `0 - zext(x)` is `SUBXrx` with a zero base. In the extended-register and
/// immediate add/sub the base is `GPR64sp`: register 31 there is `sp`, so the
/// zero written as `xzr` read the stack pointer (`queens -O2` on the Mac:
/// `sub x23, sp, w27, uxtw` and a SIGBUS).
#[test]
fn a_zero_base_of_an_extended_subtract_is_a_register_not_sp() {
    let asm = dag("define i64 @f(i32 %a) {\n  %z = zext i32 %a to i64\n  %r = sub i64 0, %z\n  ret i64 %r\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(!asm.contains("sub x0, sp,") && !asm.contains("xzr, w0, uxtw"), "{asm}");
    assert!(asm.contains("sub x0, x9, w0, uxtw"), "{asm}");
}

/// A copy that ends the function is a tail call to the library routine: the
/// DAG path refused it (`TCRETURNdi: a callee that is neither a symbol nor a
/// register`) where the hand path branches to `_memmove`.
#[test]
fn a_tail_called_library_routine_is_a_branch_to_its_symbol() {
    let asm = dag_at("-O2", "declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)\ndefine void @f(ptr %d, ptr %s, i64 %n) {\n  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)\n  ret void\n}\n")
        .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("b _memmove"), "{asm}");
}

/// A pointer stepped past each store was `strh w9, [x0]` and `add x0, x0, #2`:
/// the pin's LoadStoreOptimizer makes it `strh w9, [x0], #2`, and does not for
/// a volatile store (scroll's copy loops ran 5 instructions for the pin's 4).
#[test]
fn a_store_and_the_step_of_its_pointer_are_one_post_indexed_store() {
    let text = include_str!("data/post_index_store.ll");
    let body = &text[text.find("define").expect("a function")..];
    let asm = dag_at("-O2", body).unwrap_or_else(|why| panic!("{why}"));
    let (stepping, volatile) = asm.split_once("_step_volatile:").expect("both functions");
    assert!(stepping.contains("], #2"), "{asm}");
    assert!(!volatile.contains("], #"), "{asm}");
}

/// The update of a pointer two blocks below its load (the search loop of
/// queens, past a conditional exit) is merged as in the pin, where the scan
/// follows a successor with one predecessor.
#[test]
fn a_pointer_step_below_a_conditional_exit_is_merged_into_the_load() {
    let text = include_str!("data/queens_place.ll");
    // The function is `internal` and unused: left so, nothing is emitted.
    let body = text[text.find("define").expect("a function")..].replacen("define internal ", "define ", 1);
    let asm = dag_at("-O2", &body).unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.lines().any(|line| line.contains("ldr w") && line.contains("], #4")), "{asm}");
}

/// A double needing four moves is a pool load, as in the pin; the DAG path
/// failed with "custom lowering of ConstantPool" (nbody did not build), and
/// once lowered left the page offset in a separate `add`.
#[test]
fn a_double_constant_is_a_pool_load_with_a_page_offset() {
    let asm =
        dag_at("-O2", "define double @f(double %x) {\n  %y = fmul double %x, 0x400921FB54442D18\n  ret double %y\n}\n")
            .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("adrp x9, lCPI_f_0@PAGE"), "{asm}");
    assert!(asm.contains("ldr d16, [x9, lCPI_f_0@PAGEOFF]"), "{asm}");
    assert!(asm.contains(".quad 0x400921fb54442d18"), "{asm}");
}

/// `llvm.fmuladd` was a multiply and an add on the DAG path (nbody O2 ran
/// 18245 instructions for the pin's 16859); arm64 has
/// `isFMAFasterThanFMulAndFAdd`.
#[test]
fn fmuladd_is_one_fused_multiply_add() {
    let asm = dag_at("-O2", "declare double @llvm.fmuladd.f64(double, double, double)\ndefine double @f(double %a, double %b, double %c) {\n  %r = call double @llvm.fmuladd.f64(double %a, double %b, double %c)\n  ret double %r\n}\n").unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("fmadd d0, d0, d1, d2"), "{asm}");
    assert!(!asm.contains("fmul"), "{asm}");
}

/// A 16-bit loop counter was masked for every compare (`subs wzr, w1, w11,
/// uxth`): the pin's TypePromotion makes it an `i32` in the IR, as `crc` in
/// the bench needs (705 instructions for the pin's 506).
#[test]
fn a_narrow_loop_counter_is_compared_unmasked() {
    let body = "define i64 @f(i64 %x, i16 zeroext %lim) {\n\
b0:\n  br label %b1\n\
b1:\n  %c = phi i16 [ 0, %b0 ], [ %n, %b1 ]\n  %v = phi i64 [ %x, %b0 ], [ %w, %b1 ]\n\
  %h = lshr i64 %v, 1\n  %w = xor i64 %h, 3988292384\n\
  %n = add nuw nsw i16 %c, 1\n  %k = icmp samesign ult i16 %c, %lim\n  br i1 %k, label %b1, label %b2\n\
b2:\n  ret i64 %w\n}\n";
    let asm = dag_at("-O2", body).unwrap_or_else(|why| panic!("{why}"));
    assert!(!asm.contains("uxth"), "{asm}");
    assert!(asm.contains("subs"), "{asm}");
}

/// A constant `memcpy` of 224 bytes was a call (`b _memcpy`) on the DAG path:
/// the pin copies it in `q` pairs, 14 accesses within its limit of 16.
#[test]
fn a_constant_copy_within_the_limit_is_q_pairs_not_a_call() {
    let asm = dag_at("-O2", "declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)\ndefine void @f(ptr %d, ptr %s) {\n  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 224, i1 false)\n  ret void\n}\n").unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("ldp q") && asm.contains("stp q"), "{asm}");
    assert!(!asm.contains("_memcpy"), "{asm}");
}

/// The same for a fill: `movi` and `stp q`, not `_memset`.
#[test]
fn a_constant_fill_within_the_limit_is_q_stores_not_a_call() {
    let asm = dag_at("-O2", "declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)\ndefine void @f(ptr %d) {\n  call void @llvm.memset.p0.i64(ptr %d, i8 0, i64 300, i1 false)\n  ret void\n}\n").unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("stp q"), "{asm}");
    assert!(!asm.contains("_memset"), "{asm}");
}

/// An `i16` overflow intrinsic was refused ("promote result" of `SAddO`,
/// `SMulO`): the arithmetic is done in 32 bits and the flag is whether the
/// result is the extension of its low half (`overflow_builtins` fell to the
/// hand selector).
#[test]
fn an_i16_overflow_check_is_promoted() {
    for (name, narrow) in [("sadd", "i16"), ("uadd", "i16"), ("smul", "i16"), ("umul", "i16"), ("ssub", "i8")] {
        let body = format!(
            "declare {{ {narrow}, i1 }} @llvm.{name}.with.overflow.{narrow}({narrow}, {narrow})\ndefine i1 @f({narrow} %a, {narrow} %b) {{\n  %r = call {{ {narrow}, i1 }} @llvm.{name}.with.overflow.{narrow}({narrow} %a, {narrow} %b)\n  %o = extractvalue {{ {narrow}, i1 }} %r, 1\n  ret i1 %o\n}}\n"
        );
        dag_at("-O2", &body).unwrap_or_else(|why| panic!("{name}: {why}"));
    }
}

/// `llvm.abs` was refused ("custom lowering of Abs"; `builtins_table` fell to
/// the hand selector): the pin's `LowerABS` is `cmp x, #0; cneg`.
#[test]
fn abs_is_a_compare_and_a_conditional_negate() {
    for (ty, reg) in [("i32", "w"), ("i64", "x")] {
        let body = format!(
            "declare {ty} @llvm.abs.{ty}({ty}, i1)\ndefine {ty} @f({ty} %a) {{\n  %r = call {ty} @llvm.abs.{ty}({ty} %a, i1 false)\n  ret {ty} %r\n}}\n"
        );
        let asm = dag_at("-O2", &body).unwrap_or_else(|why| panic!("{why}"));
        assert!(asm.contains(&format!("subs {reg}zr, {reg}0, #0")) && asm.contains("csneg"), "{asm}");
    }
}

/// Atomics and fences were refused on the DAG path ("machine opcode LDARW has
/// no arm64 instruction yet"; also `DMB`): the pin's `ldar`, `ldapr`, `stlr`,
/// the LSE read-modify-writes, `cas` and `dmb`.
#[test]
fn atomics_and_fences_are_selected() {
    let text = include_str!("data/atomics.ll");
    let asm = full_greedy(text).unwrap_or_else(|why| panic!("{why}"));
    for wanted in
        ["ldar w", "ldapr w", "stlr w", "ldaddal w", "swpab w", "ldclrl w", "casal w", "dmb ish\n", "dmb ishld"]
    {
        assert!(asm.contains(wanted), "{wanted} missing:\n{asm}");
    }
}

/// An `sret` parameter was refused ("a parameter attribute that moves it";
/// `make_big` of `tail_calls_edges` fell to the hand selector): the result
/// pointer is in `x8`, the argument registers start at `x0`, and the caller
/// puts the buffer in `x8`.
#[test]
fn an_sret_pointer_is_x8_and_takes_no_argument_register() {
    let asm = dag_at(
        "-O2",
        "declare void @fill(ptr sret({ i64, i64 }), i64)\n\
define void @make(ptr sret({ i64, i64 }) %out, i64 %a) {\n  store i64 %a, ptr %out\n  ret void\n}\n\
define i64 @caller(i64 %a) {\n  %t = alloca { i64, i64 }\n  call void @fill(ptr sret({ i64, i64 }) %t, i64 %a)\n  %v = load i64, ptr %t\n  ret i64 %v\n}\n",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("str x0, [x8]"), "{asm}");
    assert!(asm.contains("add x8, sp, #0") && asm.contains("bl _fill"), "{asm}");
}

/// A dynamic `alloca` and `stacksave`/`stackrestore` were refused ("custom
/// lowering of DynamicStackAlloc", "convert node to libcall" of
/// `StackRestore`): the stack pointer is read, lowered by the size and
/// written back (`add sp, x, #0`), and a restore writes the saved one.
#[test]
fn a_dynamic_alloca_moves_the_stack_pointer() {
    let asm = dag_at(
        "-O2",
        "declare void @use(ptr)\ndeclare ptr @llvm.stacksave.p0()\ndeclare void @llvm.stackrestore.p0(ptr)\n\
define void @f(i64 %n) {\n  %s = call ptr @llvm.stacksave.p0()\n  %a = alloca i8, i64 %n, align 1\n  call void @use(ptr %a)\n  call void @llvm.stackrestore.p0(ptr %s)\n  ret void\n}\n",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.matches("add sp, x").count() >= 2, "{asm}");
    assert!(asm.contains("sub sp, x29"), "{asm}");
}

const LINUX_HEADER: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn linux_dag(body: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, format!("{LINUX_HEADER}{body}")).unwrap();
    let output = dir.path().join("t.s");
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "--isel=dag", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if said.status.success() {
        Ok(std::fs::read_to_string(&output).unwrap())
    } else {
        Err(String::from_utf8_lossy(&said.stderr).into_owned())
    }
}

/// On Linux a variadic argument went on the stack as on Darwin (`printf(fmt,
/// a)` stored `a` and a callee reading `x1` got garbage); AAPCS64 places it
/// like any other argument.
#[test]
fn a_linux_variadic_argument_is_in_a_register() {
    let asm = linux_dag(
        "declare i32 @printf(ptr, ...)\ndefine i32 @f(ptr %fmt, i32 %a) {\n  %r = call i32 (ptr, ...) @printf(ptr %fmt, i32 %a)\n  ret i32 %r\n}\n",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(!asm.contains("str "), "{asm}");
    assert!(asm.contains("b printf"), "{asm}");
}

/// A Linux stack argument takes an 8-byte slot whatever its size and a narrow
/// one is read as its own width and extended by its attribute: the ninth `i32`
/// at `[sp]`, the `signext i8` at `[sp, #8]` with `ldrsb`.
#[test]
fn linux_stack_arguments_are_eight_byte_slots_extended_by_attribute() {
    let asm = linux_dag(
        "define i32 @f(i32 %a, i32 %b, i32 %c, i32 %d, i32 %e, i32 %g, i32 %h, i32 %i, i32 %j, i8 signext %k, i64 %l) {\n  %kk = sext i8 %k to i32\n  %t = add i32 %j, %kk\n  %u = trunc i64 %l to i32\n  %w = add i32 %t, %u\n  ret i32 %w\n}\n",
    )
    .unwrap_or_else(|why| panic!("{why}"));
    assert!(asm.contains("ldr w9, [sp]") || asm.contains("ldr w10, [sp]"), "{asm}");
    assert!(asm.contains("ldrsb w") && asm.contains("[sp, #8]"), "{asm}");
    assert!(asm.contains("[sp, #16]"), "{asm}");
}

const VARIADIC_BODY: &str = "declare void @llvm.va_start.p0(ptr)\ndeclare void @llvm.va_end.p0(ptr)\n\
declare void @llvm.va_copy.p0.p0(ptr, ptr)\n\
define i32 @f(i32 %n, ...) {\n  %ap = alloca [32 x i8], align 8\n  %cp = alloca [32 x i8], align 8\n  call void @llvm.va_start.p0(ptr %ap)\n\
  call void @llvm.va_copy.p0.p0(ptr %cp, ptr %ap)\n  %p = load ptr, ptr %cp\n  %v = load i32, ptr %p\n  call void @llvm.va_end.p0(ptr %ap)\n  ret i32 %v\n}\n";

/// A variadic callee on Linux was refused ("va_start at m64 outside the entry
/// block") and fell to the hand selector. AAPCS64 saves `x1`..`x7` and
/// `q0`..`q7` (`SaveVarArgRegisters`) and writes the five-field `va_list`;
/// `va_copy` copies its 32 bytes.
#[test]
fn a_linux_variadic_callee_saves_its_registers_and_fills_the_va_list() {
    let asm = linux_dag(VARIADIC_BODY).expect("the DAG path takes it");
    for expected in ["stp x1, x2,", "str x7,", "stp q0, q1,", "stp q6, q7,"] {
        assert!(asm.contains(expected), "no `{expected}` in:\n{asm}");
    }
    // `gr_offs` -56 (`x1`..`x7` left) and `vr_offs` -128 (`q0`..`q7` all left)
    // are the two 4-byte words of one merged store: `movn x, #55` and `movk
    // #65408, lsl #32`.
    assert!(asm.contains("#55") && asm.contains("#65408"), "no gr_offs/vr_offs words in:\n{asm}");
}

/// Darwin's `va_list` is the address of the first unnamed stack slot, which the
/// DAG path also refused.
#[test]
fn a_darwin_variadic_callee_stores_the_address_of_its_first_stack_argument() {
    let asm = dag_at("-O2", VARIADIC_BODY).expect("the DAG path takes it");
    assert!(
        asm.contains("add x9, sp, #64") && asm.contains("str x9, [sp, #32]"),
        "no stack-argument address in:\n{asm}"
    );
}

/// A phi whose entry value is `poison` exported a copy of a register nothing
/// defined, and the allocator died on it ("a unique value", `@text_write` of
/// the text driver). The pin gives the register an `IMPLICIT_DEF`.
#[test]
fn a_phi_input_that_is_poison_defines_its_register() {
    let asm = full_greedy(&format!(
        "{HEADER}define i8 @f(i32 %n) {{\n\
         entry:\n  br label %loop\n\
         loop:\n  %p = phi i8 [ poison, %entry ], [ %q, %loop ]\n\
         %i = phi i32 [ 0, %entry ], [ %j, %loop ]\n  %q = trunc i32 %i to i8\n\
         %j = add i32 %i, 1\n  %c = icmp ult i32 %j, %n\n  br i1 %c, label %loop, label %out\n\
         out:\n  ret i8 %p\n}}\n"
    ))
    .expect("compiles");
    assert!(asm.contains("ret"), "{asm}");
}
