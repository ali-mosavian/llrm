//! The object of a compile with debug info is named for the source the info
//! names, not the `.ll` the backend was handed: CodeView finds the lines'
//! file by the module's name. `llrm-cc -g` handed it `m.ll` and CodeView asked
//! for a directory of it ("Enter directory for m.ll"); with a path in the file,
//! it prefilled the build machine's directory.

mod common;

use std::process::Command;

const IR: &str = "target datalayout = \"@LAYOUT@-ni:2:3\"
target triple = \"i386-unknown-unknown-code16\"

@count = dso_local global i16 0, align 2, !dbg !0

define dso_local i16 @main() addrspace(1) {
  ret i16 0
}

!llvm.dbg.cu = !{!2}
!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: \"count\", scope: !2, file: !3, line: 1, type: !4, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C99, file: !3, producer: \"t\", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, globals: !5)
!3 = !DIFile(filename: \"/tmp/build dir/m.c\", directory: \".\")
!4 = !DIBasicType(name: \"int\", size: 16, encoding: DW_ATE_signed)
!5 = !{!0}
";

#[test]
fn an_object_with_debug_info_is_named_for_its_source_file() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("m.ll");
    let output = directory.path().join("m.obj");
    std::fs::write(&input, IR.replace("@LAYOUT@", &common::datalayout("x86-m16"))).unwrap();
    let done = Command::new(env!("CARGO_BIN_EXE_llrm-llc"))
        .args(["-O0", "-g", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let object = std::fs::read(&output).unwrap();
    // THEADR: type 0x80, a length, the name as a counted string.
    assert_eq!(object[0], 0x80);
    let name = &object[4..4 + object[3] as usize];
    assert_eq!(String::from_utf8_lossy(name), "m.c", "THEADR names the source of the debug info");
}
