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
    super::memcpycall::expanded(&mut module, 0, "memcpy").expect("expands");
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
    assert!(after.contains("call ptr @memcpy(ptr %d, ptr %s, i32 %n)"), "{after}");
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
    assert!(expanded(4, "129").contains("call ptr @memcpy("), "129 bytes");
    assert!(expanded(4, "128").contains("call void @llvm.memcpy"), "128 bytes stay");
    assert!(expanded(1, "64").contains("call ptr @memcpy("), "alignment 1, past the inline limit");
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
    super::memcpycall::expanded(&mut module, 0, "memcpy").expect("expands");
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
    assert!(!expanded(1, "12").contains("call ptr @memcpy("), "{}", expanded(1, "12"));
    assert!(!expanded(1, "32").contains("call ptr @memcpy("), "eight 4-byte moves");
    assert!(expanded(1, "36").contains("call ptr @memcpy("), "nine moves");
}
