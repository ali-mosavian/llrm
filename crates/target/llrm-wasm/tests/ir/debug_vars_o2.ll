target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

define i32 @f(i32 noundef %0) local_unnamed_addr !dbg !7 {
    #dbg_value(i32 %0, !12, !DIExpression(), !14)
  %2 = add nsw i32 %0, 1, !dbg !15
    #dbg_value(i32 %2, !13, !DIExpression(), !14)
  %3 = tail call i32 @g(i32 noundef %2), !dbg !16
  ret i32 %3, !dbg !17
}

declare !dbg !18 i32 @g(i32 noundef) local_unnamed_addr


!llvm.dbg.cu = !{!0}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "o2.c", directory: "/src")
!2 = !{i32 7, !"Dwarf Version", i32 4}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 1, !"wchar_size", i32 4}
!5 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!6 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!7 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !8, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !11)
!8 = !DISubroutineType(types: !9)
!9 = !{!10, !10}
!10 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!11 = !{!12, !13}
!12 = !DILocalVariable(name: "x", arg: 1, scope: !7, file: !1, line: 2, type: !10)
!13 = !DILocalVariable(name: "y", scope: !7, file: !1, line: 3, type: !10)
!14 = !DILocation(line: 0, scope: !7)
!15 = !DILocation(line: 3, column: 13, scope: !7)
!16 = !DILocation(line: 4, column: 10, scope: !7)
!17 = !DILocation(line: 4, column: 3, scope: !7)
!18 = !DISubprogram(name: "g", scope: !1, file: !1, line: 1, type: !8, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
