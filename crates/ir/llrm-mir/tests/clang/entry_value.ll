; ModuleID = 'ev.c'
source_filename = "ev.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

; Function Attrs: nounwind uwtable
define dso_local i64 @f(i64 noundef %a, i64 noundef %b) local_unnamed_addr #0 !dbg !10 {
entry:
    #dbg_value(i64 %a, !15, !DIExpression(), !18)
    #dbg_value(i64 %b, !16, !DIExpression(), !18)
  %call = tail call i64 @g(i64 noundef %b) #2, !dbg !19
    #dbg_value(i64 %call, !17, !DIExpression(), !18)
  %mul = mul nsw i64 %a, 3, !dbg !20
  %add = add i64 %b, %mul, !dbg !21
  %add1 = add i64 %add, %call, !dbg !22
  ret i64 %add1, !dbg !23
}

declare !dbg !24 i64 @g(i64 noundef) local_unnamed_addr #1

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7, !8}
!llvm.ident = !{!9}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "Ubuntu clang version 20.1.8 (0ubuntu4)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "ev.c", directory: "/tmp/claude-1000/iv10", checksumkind: CSK_MD5, checksum: "7014dda719508bf73956bf17bb695391")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 1, !"wchar_size", i32 4}
!5 = !{i32 8, !"PIC Level", i32 2}
!6 = !{i32 7, !"PIE Level", i32 2}
!7 = !{i32 7, !"uwtable", i32 2}
!8 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!9 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!10 = distinct !DISubprogram(name: "f", scope: !1, file: !1, line: 2, type: !11, scopeLine: 2, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !14)
!11 = !DISubroutineType(types: !12)
!12 = !{!13, !13, !13}
!13 = !DIBasicType(name: "long", size: 64, encoding: DW_ATE_signed)
!14 = !{!15, !16, !17}
!15 = !DILocalVariable(name: "a", arg: 1, scope: !10, file: !1, line: 2, type: !13)
!16 = !DILocalVariable(name: "b", arg: 2, scope: !10, file: !1, line: 2, type: !13)
!17 = !DILocalVariable(name: "r", scope: !10, file: !1, line: 3, type: !13)
!18 = !DILocation(line: 0, scope: !10)
!19 = !DILocation(line: 3, column: 12, scope: !10)
!20 = !DILocation(line: 4, column: 16, scope: !10)
!21 = !DILocation(line: 4, column: 12, scope: !10)
!22 = !DILocation(line: 4, column: 20, scope: !10)
!23 = !DILocation(line: 4, column: 3, scope: !10)
!24 = !DISubprogram(name: "g", scope: !1, file: !1, line: 1, type: !25, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!25 = !DISubroutineType(types: !26)
!26 = !{!13, !13}
