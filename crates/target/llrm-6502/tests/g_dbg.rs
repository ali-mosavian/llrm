//! `-g` on the 6502 (`g-for-all-targets-card.md` steps 2-3): `.dbg line`
//! directives, which ca65 turns into the lines of ld65's debug file.

use llrm_6502::driver::{Level, Request, assembly};

const LOCATED: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"
define i8 @f(i8 %a) !dbg !5 {
  %b = add i8 %a, 1, !dbg !7
  %c = xor i8 %b, 2, !dbg !8
  ret i8 %c, !dbg !8
}
!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2}
!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: \"t\", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug)
!1 = !DIFile(filename: \"f.c\", directory: \"/d\")
!2 = !{i32 2, !\"Debug Info Version\", i32 3}
!5 = distinct !DISubprogram(name: \"f\", scope: !1, file: !1, line: 1, type: !6, scopeLine: 2, spFlags: DISPFlagDefinition, unit: !0)
!6 = !DISubroutineType(types: !{null})
!7 = !DILocation(line: 3, column: 5, scope: !5)
!8 = !DILocation(line: 4, column: 3, scope: !5)
";

fn text(debug: bool) -> String {
    assembly(LOCATED, &Request { debug, ..Request::new(Level::O2) }).expect("compiles")
}

/// `-g` printed nothing: no line of the source reached the debug file.
#[test]
fn g_names_the_file_and_the_lines_that_have_code() {
    let text = text(true);
    assert!(text.contains(".dbg file, \"f.c\""), "{text}");
    assert!(text.contains(".dbg line, \"f.c\", 3"), "{text}");
    assert!(text.contains(".dbg line, \"f.c\", 4"), "{text}");
}

/// A debug directive that changed an instruction or its order.
#[test]
fn g_adds_directives_and_leaves_the_instructions_alone() {
    let plain = text(false);
    let without_directives: String =
        text(true).lines().filter(|line| !line.trim_start().starts_with(".dbg")).map(|line| format!("{line}\n")).collect();
    assert_eq!(without_directives, plain);
}
