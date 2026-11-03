//! `llrm-z80-llc`: the program `llrm-cc`'s z80 row drives.

use std::process::Command;

fn llc(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_llrm-z80-llc")).args(args).output().expect("llrm-z80-llc runs")
}

#[test]
fn a_kernel_is_written_as_sdasz80_text() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let out = std::env::temp_dir().join("llrm-z80-llc-add16.s");
    let done = llc(&["-S", "-O2", "-o", out.to_str().unwrap(), &format!("{dir}/tests/kernels/ir/add16.ll")]);
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(&out).expect("the output file");
    assert!(text.contains(".area _CODE") && text.contains("_add16::") && text.contains("add hl, de"), "{text}");
}

#[test]
fn a_refused_function_fails_the_exit_status() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let source = std::env::temp_dir().join("llrm-z80-llc-refused.ll");
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i8 @g(ptr %p) {\n  %v = load double, ptr %p\n  %r = fptosi double %v to i8\n  ret i8 %r\n}\n";
    std::fs::write(&source, ir).unwrap();
    let out = std::env::temp_dir().join("llrm-z80-llc-refused.s");
    let done = llc(&["-S", "-o", out.to_str().unwrap(), source.to_str().unwrap()]);
    assert_eq!(done.status.code(), Some(1), "{dir}");
    assert!(String::from_utf8_lossy(&done.stderr).contains("REFUSED @g"));
}

/// Defined variables were dropped: a program with a table, a string and a
/// pointer into one produced code that named `_tab` and no `_tab` to link.
#[test]
fn defined_variables_are_written_as_data() {
    let source = std::env::temp_dir().join("llrm-z80-llc-data.ll");
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              @tab = constant [4 x i8] c\"\\01\\02\\03\\04\"\n\
              @.str.1 = private constant [3 x i8] c\"ab\\00\"\n\
              @p = global ptr getelementptr (i8, ptr @tab, i16 2)\n\
              @z = global [20 x i8] zeroinitializer\n\
              define i8 @f() {\n  %v = load i8, ptr @tab\n  ret i8 %v\n}\n";
    std::fs::write(&source, ir).unwrap();
    let out = std::env::temp_dir().join("llrm-z80-llc-data.s");
    let done = llc(&["-S", "-O0", "-o", out.to_str().unwrap(), source.to_str().unwrap()]);
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(&out).expect("the output file");
    for line in [
        ".area _CONST",
        "_tab::",
        ".db 1,2,3,4",
        "_$2estr$2e1::",
        ".db 97,98,0",
        ".area _DATA",
        "_p::",
        ".dw _tab+2",
        "_z::",
    ] {
        assert!(text.contains(line), "{line}: {text}");
    }
}

/// fletcher -O2 packed its sum bytes into a register that was not the return
/// pair, then moved them there: 25 instructions, a `ld e, c` before `ret`.
#[test]
fn a_pair_built_from_two_bytes_and_returned_has_its_bytes_in_the_return_pair() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let out = std::env::temp_dir().join("llrm-z80-llc-fletcher.s");
    let done = llc(&["-S", "-O2", "-o", out.to_str().unwrap(), &format!("{dir}/tests/kernels/ir/fletcher.ll")]);
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(&out).expect("the output file");
    let instructions = text.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(instructions <= 24, "{instructions} instructions:\n{text}");
}

/// `-g` was refused ("unrecognized argument -g", exit 2) and so was every
/// `llrm-cc -g` z80 row: it writes SDCC's `C$file$line` symbols.
#[test]
fn g_writes_the_line_symbols() {
    let source = std::env::temp_dir().join("llrm-z80-llc-g.ll");
    let ir = "target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16\"\n\
              define i16 @f(i16 %a, i16 %b) !dbg !5 {\n  %c = add i16 %a, %b, !dbg !7\n  ret i16 %c, !dbg !7\n}\n\
              !llvm.dbg.cu = !{!0}\n!llvm.module.flags = !{!2}\n\
              !0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: \"t\", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug)\n\
              !1 = !DIFile(filename: \"f.c\", directory: \"/d\")\n!2 = !{i32 2, !\"Debug Info Version\", i32 3}\n\
              !5 = distinct !DISubprogram(name: \"f\", scope: !1, file: !1, line: 1, type: !6, scopeLine: 2, spFlags: DISPFlagDefinition, unit: !0)\n\
              !6 = !DISubroutineType(types: !{null})\n!7 = !DILocation(line: 3, column: 5, scope: !5)\n";
    std::fs::write(&source, ir).unwrap();
    let out = std::env::temp_dir().join("llrm-z80-llc-g.s");
    let done = llc(&["-S", "-g", "-o", out.to_str().unwrap(), source.to_str().unwrap()]);
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let text = std::fs::read_to_string(&out).expect("the output file");
    assert!(text.contains("C$f.c$3$"), "{text}");
}
