//! `-g` on the z80 (`g-for-all-targets-card.md`): SDCC's `C$file$line$..`
//! symbols ahead of the instructions, which sdld lists in the `.cdb`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const LOCATED: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"
define i16 @f(i16 %a, i16 %b) !dbg !5 {
  %c = add i16 %a, %b, !dbg !7
  %d = xor i16 %c, %a, !dbg !8
  ret i16 %d, !dbg !8
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

fn assembly(debug: bool) -> String {
    text(LOCATED, "f", 0, &Request { debug, ..Request::new(Level::O2) }).expect("compiles")
}

/// `-g` named no line: sdld had no `C$` symbol to put in the `.cdb`.
#[test]
fn g_names_each_line_that_has_code() {
    let text = assembly(true);
    assert!(text.contains("C$f.c$3$"), "{text}");
    assert!(text.contains("C$f.c$4$"), "{text}");
}

/// A symbol that changed an instruction, a label or their order.
#[test]
fn g_adds_symbols_and_leaves_the_instructions_alone() {
    let kept: String = assembly(true)
        .lines()
        .filter(|line| !line.trim_start().starts_with("C$") && !line.trim_start().starts_with(".globl"))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(kept, assembly(false));
}
