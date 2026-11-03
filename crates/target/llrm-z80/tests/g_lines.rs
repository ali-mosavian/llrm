//! `-g` on the z80 (`g-for-all-targets-card.md`): SDCC's `C$file$line$..`
//! symbols ahead of the instructions, which sdld lists in the `.cdb`.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

const LOCATED_LAYOUT: &str = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16";

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
        .filter(|line| !["C$", "G$", ".globl"].iter().any(|start| line.trim_start().starts_with(start)))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(kept, assembly(false));
}

/// The function had no symbol of its own and no row at the line it starts at:
/// the `.cdb` listed its first line only where its first instruction was.
#[test]
fn g_names_the_function_and_the_line_it_starts_at() {
    let text = assembly(true);
    let start = text.find("_f::").expect("the function's label");
    let head = &text[..start];
    assert!(head.contains("G$f$0$0"), "{text}");
    assert!(head.contains("C$f.c$2$"), "{text}");
}

fn adb_of(
    ret: &str,
    encoding: &str,
    size: u32,
) -> String {
    let ir = LOCATED
        .replace("i16 @f(i16 %a, i16 %b)", &format!("{ret} @f({ret} %a, {ret} %b)"))
        .replace("i16", ret)
        .replace(
            "!6 = !DISubroutineType(types: !{null})",
            "!6 = !DISubroutineType(types: !{!9})\n!9 = !DIBasicType(name: \"t\", size: SIZE, encoding: ENC)",
        )
        .replace(&LOCATED_LAYOUT.replace("i16", ret), LOCATED_LAYOUT)
        .replace("SIZE", &size.to_string())
        .replace("ENC", encoding);
    llrm_z80::driver::adb(
        &ir,
        &llrm_z80::driver::Request { debug: true, ..llrm_z80::driver::Request::new(llrm_z80::driver::Level::O2) },
    )
    .expect("an adb")
}

/// The `.adb` named no result type, or read signed and unsigned alike
/// (`SI:S` for both): sdld's `.cdb` then types `unsigned` as `int`.
#[test]
fn the_adb_types_a_function_by_its_signedness() {
    assert!(adb_of("i16", "DW_ATE_signed", 16).contains("F:G$f$0_0$0({2}DF,SI:S)"));
    assert!(adb_of("i16", "DW_ATE_unsigned", 16).contains("F:G$f$0_0$0({2}DF,SI:U)"));
    assert!(adb_of("i8", "DW_ATE_signed_char", 8).contains("DF,SC:S"));
    assert!(adb_of("i8", "DW_ATE_unsigned_char", 8).contains("DF,SC:U"));
    assert!(adb_of("i16", "DW_ATE_signed", 16).starts_with("M:f\n"));
}

/// A function the module calls and does not define had no symbol in the
/// `.adb`, so the `.cdb` could not type the call (SDCC lists `G$g` as well).
#[test]
fn the_adb_lists_a_declared_function_with_its_type() {
    let ir = LOCATED
        .replace("define i16 @f(", "declare i16 @g(i16)\ndefine i16 @f(")
        .replace("%c = add i16 %a, %b, !dbg !7", "%c = call i16 @g(i16 %a), !dbg !7")
        .replace("!6 = !DISubroutineType(types: !{null})", "!6 = !DISubroutineType(types: !{null})\n!10 = !DISubprogram(name: \"g\", scope: !1, file: !1, line: 9, type: !11, spFlags: 0)\n!11 = !DISubroutineType(types: !{!12, !12})\n!12 = !DIBasicType(name: \"int\", size: 16, encoding: DW_ATE_signed)")
        .replace("declare i16 @g(i16)", "declare i16 @g(i16) !dbg !10");
    let adb = llrm_z80::driver::adb(
        &ir,
        &llrm_z80::driver::Request { debug: true, ..llrm_z80::driver::Request::new(llrm_z80::driver::Level::O2) },
    )
    .expect("an adb");
    assert!(adb.contains("S:G$g$0_0$0({2}DF,SI:S),C,0,0"), "{adb}");
}

const VARIABLES: &str = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"
declare i16 @g(i16)
define i16 @f(i16 %a, i16 %b) !dbg !5 {
  %x = alloca i16, align 1
  #dbg_value(i16 %a, !11, !DIExpression(), !8)
  #dbg_value(i16 %b, !13, !DIExpression(), !8)
  #dbg_declare(ptr %x, !12, !DIExpression(), !8)
  %c = call i16 @g(i16 %a), !dbg !7
  store i16 %c, ptr %x, align 1, !dbg !7
  %d = add i16 %c, %b, !dbg !8
  ret i16 %d, !dbg !8
}
!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2}
!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: \"t\", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug)
!1 = !DIFile(filename: \"f.c\", directory: \"/d\")
!2 = !{i32 2, !\"Debug Info Version\", i32 3}
!5 = distinct !DISubprogram(name: \"f\", scope: !1, file: !1, line: 1, type: !6, scopeLine: 2, spFlags: DISPFlagDefinition, unit: !0)
!6 = !DISubroutineType(types: !{!9, !9, !10})
!7 = !DILocation(line: 3, column: 5, scope: !5)
!8 = !DILocation(line: 4, column: 3, scope: !5)
!9 = !DIBasicType(name: \"int\", size: 16, encoding: DW_ATE_signed)
!10 = !DIBasicType(name: \"unsigned\", size: 16, encoding: DW_ATE_unsigned)
!11 = !DILocalVariable(name: \"a\", arg: 1, scope: !5, file: !1, line: 1, type: !9)
!12 = !DILocalVariable(name: \"x\", scope: !5, file: !1, line: 3, type: !9)
!13 = !DILocalVariable(name: \"b\", arg: 2, scope: !5, file: !1, line: 1, type: !10)
";

fn variables_adb(level: Level) -> String {
    llrm_z80::driver::adb(VARIABLES, &Request { debug: true, ..Request::new(level) }).expect("an adb")
}

/// The `.adb` named no parameter or local, so the `.cdb` had no variable of
/// `f` for a debugger to show.
#[test]
fn the_adb_names_a_parameter_and_a_local_with_the_sign_of_its_type() {
    let adb = variables_adb(Level::O0);
    assert!(adb.contains("S:Lf.f$a$1_0$0({2}SI:S),R,0,0,[l,h]"), "{adb}");
    assert!(adb.contains("S:Lf.f$b$1_0$1({2}SI:U),R,0,0,[e,d]"), "{adb}");
    assert!(adb.contains("S:Lf.f$x$1_0$2({2}SI:S),B,1,-2"), "{adb}");
}

/// A parameter record SDCC would not write (the first cut spelled the sign
/// `UI:S`): compared with SDCC's own for the same signature, less the counter
/// at the end of the name.
#[test]
fn the_adb_writes_a_parameter_as_sdcc_does() {
    let dir = std::env::temp_dir().join(format!("z80-adb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.c"), "int g(int);\nint f(int a, unsigned b){int x=g(a);return x+b;}\n").unwrap();
    let Ok(status) =
        std::process::Command::new("sdcc").current_dir(&dir).args(["-mz80", "--debug", "-c", "f.c"]).status()
    else {
        return;
    };
    assert!(status.success());
    let parameters = |text: &str| -> Vec<String> {
        let mut lines: Vec<String> = text
            .lines()
            .filter(|line| line.starts_with("S:Lf.f$a$") || line.starts_with("S:Lf.f$b$"))
            .map(|line| line.replacen("$1_0$0(", "$1_0$N(", 1).replacen("$1_0$1(", "$1_0$N(", 1))
            .collect();
        lines.sort();
        lines
    };
    let sdcc = parameters(&std::fs::read_to_string(dir.join("f.adb")).unwrap());
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(parameters(&variables_adb(Level::O2)), sdcc);
}

/// An instruction with no location (the `ret` here) was listed under the line
/// before it; the pin gives it line 0, which SDCC spells `C$f.c$0$..`.
#[test]
fn an_unlocated_instruction_after_a_located_one_is_line_zero() {
    let ir = LOCATED.replace("  ret i16 %d, !dbg !8", "  ret i16 %d");
    let request = Request { debug: true, ..Request::new(Level::O2) };
    let on = text(&ir, "f", 0, &request).expect("compiles");
    assert!(on.contains("C$f.c$0$"), "{on}");
    let off = text(&ir, "f", 0, &Request::new(Level::O2)).expect("compiles");
    assert!(!off.contains("C$"), "{off}");
}
