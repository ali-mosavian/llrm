; ModuleID = '/tmp/claude-1000/iv10/cs.c'
source_filename = "/tmp/claude-1000/iv10/cs.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

; Function Attrs: nounwind uwtable
define i32 @f(i32 noundef %a, i32 noundef %b) local_unnamed_addr #0 !dbg !14 {
entry:
    #dbg_value(i32 %a, !20, !DIExpression(), !23)
    #dbg_value(i32 %b, !21, !DIExpression(), !23)
  %call = tail call i32 @g(i32 noundef %a, i32 noundef 7) #2, !dbg !24
    #dbg_value(i32 %call, !22, !DIExpression(), !23)
  %add = add nsw i32 %call, %b, !dbg !25
  %call1 = tail call i32 @h(i32 noundef %add) #2, !dbg !26
  %add2 = add nsw i32 %call1, %call, !dbg !27
  ret i32 %add2, !dbg !28
}

declare !dbg !29 i32 @g(i32 noundef, i32 noundef) local_unnamed_addr #1

declare !dbg !30 i32 @h(i32 noundef) local_unnamed_addr #1

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!0}
!llvm.module.flags = !{!2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = distinct !DICompileUnit(language: DW_LANG_C11, file: !1, producer: "clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, splitDebugInlining: false, nameTableKind: None)
!1 = !DIFile(filename: "/tmp/claude-1000/iv10/cs.c", directory: "/home/alim/work/personal/llrm/.claude/worktrees/orr", checksumkind: CSK_MD5, checksum: "459591ed1d3006f42f366a63818b7ead")
!2 = !{i32 7, !"Dwarf Version", i32 5}
!3 = !{i32 2, !"Debug Info Version", i32 3}
!4 = !{i32 8, !"PIC Level", i32 2}
!5 = !{i32 7, !"PIE Level", i32 2}
!6 = !{i32 7, !"uwtable", i32 2}
!7 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = distinct !DISubprogram(name: "f", scope: !15, file: !15, line: 3, type: !16, scopeLine: 3, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !0, retainedNodes: !19, keyInstructions: true)
!15 = !DIFile(filename: "/tmp/claude-1000/iv10/cs.c", directory: "", checksumkind: CSK_MD5, checksum: "459591ed1d3006f42f366a63818b7ead")
!16 = !DISubroutineType(types: !17)
!17 = !{!18, !18, !18}
!18 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!19 = !{!20, !21, !22}
!20 = !DILocalVariable(name: "a", arg: 1, scope: !14, file: !15, line: 3, type: !18)
!21 = !DILocalVariable(name: "b", arg: 2, scope: !14, file: !15, line: 3, type: !18)
!22 = !DILocalVariable(name: "r", scope: !14, file: !15, line: 4, type: !18)
!23 = !DILocation(line: 0, scope: !14)
!24 = !DILocation(line: 4, column: 11, scope: !14, atomGroup: 1, atomRank: 2)
!25 = !DILocation(line: 5, column: 14, scope: !14)
!26 = !DILocation(line: 5, column: 10, scope: !14)
!27 = !DILocation(line: 5, column: 19, scope: !14, atomGroup: 3, atomRank: 2)
!28 = !DILocation(line: 5, column: 3, scope: !14, atomGroup: 3, atomRank: 1)
!29 = !DISubprogram(name: "g", scope: !15, file: !15, line: 1, type: !16, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!30 = !DISubprogram(name: "h", scope: !15, file: !15, line: 2, type: !31, flags: DIFlagPrototyped, spFlags: DISPFlagOptimized)
!31 = !DISubroutineType(types: !32)
!32 = !{!18, !18}
