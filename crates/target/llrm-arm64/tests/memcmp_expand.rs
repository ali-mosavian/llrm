//! `AArch64TTIImpl::enableMemCmpExpansion`: a `memcmp` of a constant size is
//! loads and compares. Without the hook arm64 kept the call (`bl bcmp`,
//! `b memcmp`) where the pin expands it.

use llrm_arm64::driver;
use llrm_arm64::machine::Platform;

const DATALAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn assembly(
    size: u32,
    zero_cmp: bool,
) -> String {
    let result =
        if zero_cmp { "  %c = icmp eq i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z\n" } else { "  ret i32 %r\n" };
    let ir = format!(
        "{DATALAYOUT}declare i32 @memcmp(ptr, ptr, i64)\n\
         define i32 @f(ptr %a, ptr %b) {{\n  %r = call i32 @memcmp(ptr %a, ptr %b, i64 {size})\n{result}}}\n"
    );
    driver::assembly(&ir, Platform::Linux, true).expect("lowers")
}

/// The sizes the review asked for: one load, three, and a tail of 7 bytes.
#[test]
fn a_constant_size_memcmp_is_loads_not_a_call() {
    for (size, zero_cmp) in [(8, true), (8, false), (24, true), (7, true), (7, false)] {
        let asm = assembly(size, zero_cmp);
        assert!(
            !asm.contains("memcmp") && !asm.contains("bcmp"),
            "{size} bytes, zero_cmp {zero_cmp} stayed a call:\n{asm}"
        );
        assert!(asm.contains("ldr"), "{size} bytes, zero_cmp {zero_cmp}:\n{asm}");
    }
}
