//! A dump (`--dump`, `LLRM_DUMP_INPUTS`) was text LLVM's own parser refused: llrm's calling conventions written by
//! name (`watcallcc`, `sysvcc`, `cc1000`) and `declare internal`. `opt` rejected every dump it was handed; the
//! dumps now say `cc N` and no linkage on a declaration, and llrm's parser still reads both.

use llrm_mir::module::{GlobalKind, Linkage};
use llrm_mir::{parse, print};

const SOURCE: &str = "declare watcallcc i32 @external(i32)\n\n\
define internal watcallcc i32 @f(i32 %a) {\nb:\n  %r = call watcallcc i32 @external(i32 %a)\n  ret i32 %r\n}\n";

fn printed() -> String {
    let mut module = parse::module(SOURCE).expect("parses");
    for global in &mut module.globals {
        if matches!(global.kind, GlobalKind::Function(ref function) if function.is_declaration()) {
            global.linkage = Linkage::Internal;
        }
    }
    print::module(&module)
}

#[test]
fn test_a_dump_names_no_convention_and_no_linkage_llvm_does_not() {
    let text = printed();
    assert!(!text.contains("watcallcc"), "{text}");
    assert!(!text.contains("declare internal"), "{text}");
    assert!(text.contains("cc 1002"), "{text}");
    parse::module(&text).expect("llrm reads what it prints");
}

#[test]
fn test_llvm_as_reads_a_dump() {
    let Ok(llvm_as) = std::process::Command::new("llvm-as-20").arg("--version").output() else {
        eprintln!("skipped: no llvm-as-20");
        return;
    };
    assert!(llvm_as.status.success());
    let mut child = std::process::Command::new("llvm-as-20")
        .args(["-o", "/dev/null", "-"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("llvm-as runs");
    std::io::Write::write_all(child.stdin.as_mut().expect("stdin"), printed().as_bytes()).expect("writes");
    let done = child.wait_with_output().expect("finishes");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}
