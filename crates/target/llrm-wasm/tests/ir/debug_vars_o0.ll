target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

define i32 @f(i32 noundef %0) !dbg !6 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
    #dbg_declare(ptr %2, !11, !DIExpression(), !12)
    #dbg_declare(ptr %3, !13, !DIExpression(), !14)
  %4 = load i32, ptr %2, align 4, !dbg !15
  %5 = add nsw i32 %4, 1, !dbg !16
  store i32 %5, ptr %3, align 4, !dbg !14
  %6 = load i32, ptr %3, align 4, !dbg !17
  %7 = call i32 @g(i32 noundef %6), !dbg !18
  ret i32 %7, !dbg !19
}

declare i32 @g(i32 noundef)


!llvm.dbg.cu = !{!0}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "dv.c", directory: "/src")
!2 = !{i32 7, !"Dwarf Version", i32 4}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 1, !"wchar_size", i32 4}
!5 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!6 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !7, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition, unit: !0, retainedNodes: !10)
!7 = !DISubroutineType(types: !8)
!8 = !{!9, !9}
!9 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!10 = !{}
!11 = !DILocalVariable(name: "x", arg: 1, scope: !6, file: !1, line: 2, type: !9)
!12 = !DILocation(line: 2, column: 11, scope: !6)
!13 = !DILocalVariable(name: "y", scope: !6, file: !1, line: 3, type: !9)
!14 = !DILocation(line: 3, column: 7, scope: !6)
!15 = !DILocation(line: 3, column: 11, scope: !6)
!16 = !DILocation(line: 3, column: 13, scope: !6)
!17 = !DILocation(line: 4, column: 12, scope: !6)
!18 = !DILocation(line: 4, column: 10, scope: !6)
!19 = !DILocation(line: 4, column: 3, scope: !6)
