use llrm_mir::module::Module;

fn text(
    align: u32,
    length: &str,
) -> String {
    format!(
        "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
         define void @f(ptr %d, ptr %s, i32 %n) {{\n\
         call void @llvm.memcpy.p0.p0.i32(ptr align {align} %d, ptr align {align} %s, i32 {length}, i1 false)\n\
         ret void\n}}\n"
    )
}

fn expanded(
    align: u32,
    length: &str,
) -> String {
    let mut module: Module = llrm_mir::parse::module(&text(align, length)).expect("parses");
    super::memcpycall::expanded(&mut module, 0, "memcpy", false, 0, false).expect("expands");
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{}", llrm_mir::print::module(&module));
    llrm_mir::print::module(&module)
}

/// A copy of a length only the program knows was `rep movs` through ESI, EDI
/// and ECX on every target (x_mergesort's six copies, +12 frame references
/// against the pin, which calls `memcpy`): with the target's `memcpy` named, it
/// is the call.
#[test]
fn test_a_variable_length_copy_is_a_call_of_memcpy() {
    let after = expanded(4, "%n");
    assert!(after.contains("call void @memcpy(ptr %d, ptr %s, i32 %n)"), "{after}");
    assert!(!after.contains("call void @llvm.memcpy"), "the intrinsic is gone: {after}");
}

/// The selector keeps what it moves in a few loads and stores: a short
/// constant copy of aligned pointers is no call.
#[test]
fn test_a_short_aligned_constant_copy_stays_for_the_selector() {
    let after = expanded(4, "16");
    assert!(after.contains("call void @llvm.memcpy"), "{after}");
    assert!(!after.contains("@memcpy("), "{after}");
}

/// `getMaxInlineSizeThreshold` is 128 and an alignment under four is a
/// call too, as the pin's `EmitTargetCodeForMemcpy` returns the generic path.
#[test]
fn test_a_long_or_misaligned_constant_copy_is_a_call() {
    assert!(expanded(4, "129").contains("call void @memcpy("), "129 bytes");
    assert!(expanded(4, "128").contains("call void @llvm.memcpy"), "128 bytes stay");
    assert!(expanded(1, "64").contains("call void @memcpy("), "alignment 1, past the inline limit");
}

/// A length of another width than the call's `i32` (a copy of 200 bytes the
/// front end sizes `i16`, a count `i64`) left the call passing `i16` as an
/// `i32` parameter: "the pipeline left invalid MIR" on every long copy in
/// tests/run/c/memcpy_libcall.c once the m32 runtime named `memcpy`.
#[test]
fn test_a_length_of_another_width_is_made_the_calls_i32() {
    let text = "declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)\n\
                define void @f(ptr %d, ptr %s, i16 %n) {\n\
                call void @llvm.memcpy.p0.p0.i16(ptr align 4 %d, ptr align 4 %s, i16 %n, i1 false)\n\
                ret void\n}\n";
    let mut module: Module = llrm_mir::parse::module(text).expect("parses");
    super::memcpycall::expanded(&mut module, 0, "memcpy", false, 0, false).expect("expands");
    let after = llrm_mir::print::module(&module);
    assert_eq!(llrm_mir::verify::verify(&module), Vec::<String>::new(), "{after}");
    assert!(after.contains("zext i16 %n to i32"), "{after}");
}

/// A 12-byte copy of pointers with no `align` was a call of `memcpy`: the pin
/// copies by loads and stores first and calls only past its limit (QCport
/// weapons.c: 16 calls where the pin has none, `_pl_spikes_tick` +55
/// instructions at -Os).
#[test]
fn test_a_short_constant_copy_of_unaligned_pointers_stays_for_the_selector() {
    assert!(!expanded(1, "12").contains("call void @memcpy("), "{}", expanded(1, "12"));
    assert!(!expanded(1, "32").contains("call void @memcpy("), "eight 4-byte moves");
    assert!(expanded(1, "36").contains("call void @memcpy("), "nine moves");
}


/// A 700-byte copy at m32 -Os was a call of `memcpy` (push x3, call, add) where
/// the pin's llc at `minsize` writes `rep movsb`: memory_expansion +1110 B of
/// the 2192 B by which 88 cells were worse. A target that takes `rep movsb` at
/// -Os, or a function marked `minsize`, leaves a constant copy to the selector
/// at any alignment and size.
#[test]
fn test_a_constant_copy_stays_for_rep_movsb_where_the_target_takes_it() {
    let run = |rep_movsb: bool| {
        let source = text(1, "700");
        let mut module: Module = llrm_mir::parse::module(&source).expect("parses");
        super::memcpycall::expanded(&mut module, 0, "memcpy", rep_movsb, 0, false).expect("expands");
        llrm_mir::print::module(&module)
    };
    assert!(run(false).contains("call void @memcpy("), "the pin's -Os: a call");
    assert!(run(true).contains("call void @llvm.memcpy"), "rep movsb at -Os where the target says so");
}

/// A 4000-byte copy of 16-aligned blocks at -O2 was a call of `memcpy` where gcc writes `rep movsq` (scroll O2,
/// +163 instructions a round). A target with a `rep movs` limit (DEVIATE REP-MOVS-SPEED) leaves it to the selector up
/// to that size; a function marked `optsize` and a copy past the limit keep the call.
#[test]
fn test_an_aligned_constant_copy_stays_for_rep_movs_up_to_the_speed_limit() {
    let run = |bytes: &str, speed_max: u64, sized: bool| {
        let source = text(16, bytes);
        let source = if sized { source.replace(") {", ") optsize {") } else { source };
        let mut module: Module = llrm_mir::parse::module(&source).expect("parses");
        super::memcpycall::expanded(&mut module, 0, "memcpy", false, speed_max, false).expect("expands");
        llrm_mir::print::module(&module)
    };
    assert!(run("4000", 0, false).contains("call void @memcpy("), "the pin's: a call");
    assert!(run("4000", 8192, false).contains("call void @llvm.memcpy"), "rep movs up to the limit");
    assert!(run("9000", 8192, false).contains("call void @memcpy("), "past the limit: a call");
    assert!(run("4000", 8192, true).contains("call void @memcpy("), "-Os keeps the call");
}

/// gcc -Os copies a length known at run time with one `rep movsb` (`decide_alg`:
/// `rep_prefix_1_byte`); the pin calls `memcpy`. A copy tuned for size outside every loop is left to
/// the selector where the tuning says so; one in a loop keeps the call, and so does a build for speed.
/// m64 -Os had 117 calls of `memcpy`/`memset` against gcc's 18 (suggest.c sugg_compose: a leaf in gcc,
/// so its caller kept values in scratch registers, 79 B in sugg_probe alone).
#[test]
fn test_a_variable_copy_at_size_outside_loops_is_rep_movsb_and_one_in_a_loop_is_a_call() {
    let cold = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
        define void @f(ptr %d, ptr %s, i32 %n) optsize {\n\
        call void @llvm.memcpy.p0.p0.i32(ptr align 1 %d, ptr align 1 %s, i32 %n, i1 false)\n\
        ret void\n}\n";
    let looped = "declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)\n\
        define void @f(ptr %d, ptr %s, i32 %n) optsize {\n\
        entry:\n  br label %loop\n\
        loop:\n  %i = phi i32 [ 0, %entry ], [ %j, %loop ]\n\
        call void @llvm.memcpy.p0.p0.i32(ptr align 1 %d, ptr align 1 %s, i32 %n, i1 false)\n\
        %j = add i32 %i, 1\n  %c = icmp ult i32 %j, 8\n  br i1 %c, label %loop, label %out\n\
        out:\n  ret void\n}\n";
    let run = |source: &str, size_cold: bool| {
        let mut module: Module = llrm_mir::parse::module(source).expect("parses");
        super::memcpycall::expanded(&mut module, 0, "memcpy", false, 0, size_cold).expect("expands");
        llrm_mir::print::module(&module)
    };
    assert!(run(cold, false).contains("call void @memcpy"), "premise: the pin's call");
    let kept = run(cold, true);
    assert!(kept.contains("call void @llvm.memcpy") && !kept.contains("@memcpy("), "{kept}");
    assert!(run(looped, true).contains("call void @memcpy"), "a copy in a loop keeps the call");
    let fast = cold.replace(" optsize {", " {");
    assert!(run(&fast, true).contains("call void @memcpy"), "not tuned for size: the call");
}
