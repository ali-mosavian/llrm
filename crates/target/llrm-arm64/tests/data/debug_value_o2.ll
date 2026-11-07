; hand-written: long inc(long n, long m) { return (id(n) ^ m) + n; } at -O2: n and m both arrive in registers and live across the call
source_filename = "inc.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define dso_local i64 @inc(i64 noundef %n, i64 noundef %m) #0 !dbg !9 {
entry:
    #dbg_value(i64 %n, !15, !DIExpression(), !17)
    #dbg_value(i64 %m, !16, !DIExpression(), !17)
    #dbg_value(i64 %n, !22, !DIExpression(), !17)
  %call = call i64 @id(i64 noundef %n), !dbg !19
    #dbg_value(i64 poison, !22, !DIExpression(), !17)
  %s = xor i64 %call, %m, !dbg !20
  %r = add nsw i64 %s, %n, !dbg !20
  ret i64 %r, !dbg !21
}

declare i64 @id(i64 noundef)

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf" }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!5, !6}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, retainedTypes: !2, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "inc.c", directory: "")
!2 = !{!3}
!3 = !DIBasicType(name: "long", size: 64, encoding: DW_ATE_signed)
!5 = !{i32 7, !"Dwarf Version", i32 5}
!6 = !{i32 2, !"Debug Info Version", i32 3}
!9 = distinct !DISubprogram(name: "inc", scope: !1, file: !1, line: 1, type: !10, scopeLine: 2, flags: DIFlagPrototyped, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !14)
!10 = !DISubroutineType(types: !11)
!11 = !{!3, !3, !3}
!14 = !{!15, !16, !22}
!15 = !DILocalVariable(name: "n", arg: 1, scope: !9, file: !1, line: 1, type: !3)
!16 = !DILocalVariable(name: "m", arg: 2, scope: !9, file: !1, line: 1, type: !3)
!17 = !DILocation(line: 0, scope: !9)
!18 = !DILocation(line: 3, column: 16, scope: !9)
!19 = !DILocation(line: 4, column: 12, scope: !9)
!20 = !DILocation(line: 4, column: 21, scope: !9)
!21 = !DILocation(line: 4, column: 5, scope: !9)
!22 = !DILocalVariable(name: "k", scope: !9, file: !1, line: 2, type: !3)
