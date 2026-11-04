target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i486-unknown-linux-gnu"

define internal i32 @sq(i32 noundef %0) !dbg !26 {
  %2 = mul nsw i32 %0, %0, !dbg !30
  ret i32 %2, !dbg !30
}

define dso_local i32 @f(i32 noundef %0) !dbg !12 {
  %2 = call i32 @sq(i32 noundef %0), !dbg !29
  %3 = add nsw i32 %0, 1, !dbg !34
  %4 = call i32 @sq(i32 noundef %3), !dbg !32
  %5 = add nsw i32 %2, %4, !dbg !34
  ret i32 %5, !dbg !35
}

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!3, !4}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "t", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug)
!1 = !DIFile(filename: "/tmp/sc.c", directory: "/tmp")
!3 = !{i32 7, !"Dwarf Version", i32 5}
!4 = !{i32 2, !"Debug Info Version", i32 3}
!12 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !14, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0)
!14 = !DISubroutineType(types: !15)
!15 = !{!16, !16}
!16 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!26 = distinct !DISubprogram(name: "sq", scope: !1, file: !1, line: 1, type: !14, scopeLine: 1, flags: DIFlagPrototyped, spFlags: DISPFlagLocalToUnit | DISPFlagDefinition | DISPFlagOptimized, unit: !0)
!29 = !DILocation(line: 2, column: 48, scope: !12)
!30 = !DILocation(line: 1, column: 40, scope: !26)
!32 = !DILocation(line: 2, column: 68, scope: !12)
!34 = !DILocation(line: 2, column: 66, scope: !12)
!35 = !DILocation(line: 2, column: 57, scope: !12)
