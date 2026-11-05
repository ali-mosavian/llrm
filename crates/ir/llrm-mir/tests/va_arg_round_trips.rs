//! `va_arg` was refused by MIR's reader ("not an instruction of MIR's
//! subset"), so any clang IR reading a `...` failed to compile. It now parses
//! and prints back as written.

use llrm_mir::{parse, print};

const SOURCE: &str = "define double @f(ptr %ap) {\nb:\n  %v = va_arg ptr %ap, double\n  ret double %v\n}\n";

#[test]
fn test_va_arg_parses_and_prints_as_written() {
    let module = parse::module(SOURCE).expect("va_arg parses");
    let text = print::module(&module);
    assert!(text.contains("%v = va_arg ptr %ap, double"), "{text}");
    parse::module(&text).expect("the print parses again");
}
