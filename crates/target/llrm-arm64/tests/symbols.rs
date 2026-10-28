use llrm_arm64::data::symbol_name;
use llrm_arm64::machine::Platform;

/// `@"\01_fputs"` (what macOS headers' `__asm("_fputs")` becomes) was emitted
/// as `__fputs`, so `extdata` failed to link; a `\01` name is the linker's
/// spelling already, on every platform.
#[test]
fn a_verbatim_name_gets_no_underscore() {
    assert_eq!(symbol_name(Platform::Darwin, "\u{1}_fputs"), "_fputs");
    assert_eq!(symbol_name(Platform::Linux, "\u{1}fputs"), "fputs");
    assert_eq!(symbol_name(Platform::Darwin, "main"), "_main");
    assert_eq!(symbol_name(Platform::Linux, "main"), "main");
}
